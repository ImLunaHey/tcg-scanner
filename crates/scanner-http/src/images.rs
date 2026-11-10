use crate::{
    Gateway,
    error::{Error, Result},
    gateway::header_value,
    path::Path,
    routes::{only, uuid},
};
use axum::{
    Json,
    body::Body,
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use scanner_types::image_access::{ImageChunk, ThumbnailFormat};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

impl Gateway {
    pub(crate) async fn image_route(
        self: &Arc<Self>,
        head: &Parts,
        path: &Path,
        parts: &[&str],
        body: Map<String, Value>,
    ) -> Result<Response> {
        let ["images", id, tail @ ..] = parts else {
            return Err(Error::new(404, "notFound", "Unknown image route."));
        };
        let (job, sequence) = image_id(id)?;
        let edit = path
            .query
            .get("editID")
            .map(|s| Uuid::parse_str(s).map_err(|_| Error::params("Invalid edit ID.")))
            .transpose()?;
        if tail == ["prepare-card"] && head.method == "POST" {
            return self.prepare_card_route(path, job, sequence, body).await;
        }
        if tail == ["identification"] {
            return self
                .identification_route(head, path, job, sequence, body)
                .await;
        }
        if tail == ["finish-estimate"] && head.method == "POST" {
            return self.finish_route(path, job, sequence, body).await;
        }
        if tail == ["condition-check"] && head.method == "POST" {
            return self.condition_route(path, job, sequence, body).await;
        }
        if head.method == "POST" && tail == ["edits"] {
            path.only(&[])?;
            only(&body, &["id", "crop", "quarterTurns", "deskewDegrees"])?;
            let mut body = body;
            body.insert("jobID".into(), json!(job));
            body.insert("sequence".into(), json!(sequence));
            return Ok((
                StatusCode::CREATED,
                Json(self.call("images.edit", json!(body)).await?),
            )
                .into_response());
        }
        if head.method != "GET" {
            return Err(Error::new(
                405,
                "methodNotAllowed",
                "Method or image route not supported.",
            ));
        }
        if tail == ["thumbnail"] {
            path.only(&["editID", "format"])?;
            let format = match path
                .query
                .get("format")
                .map(String::as_str)
                .unwrap_or("png")
            {
                "png" => ThumbnailFormat::Png,
                "jpeg" => ThumbnailFormat::Jpeg,
                _ => return Err(Error::params("Thumbnail format must be png or jpeg.")),
            };
            let result = self.engine.thumbnail(job, sequence, edit, format).await?;
            let bytes = STANDARD
                .decode(result.base64)
                .map_err(|_| Error::internal())?;
            if format!("{:x}", Sha256::digest(&bytes)) != result.sha256 {
                return Err(Error::internal());
            }
            return cached(head, Body::from(bytes), &result.media_type, &result.sha256);
        }
        path.only(&["editID"])?;
        if tail == ["metadata"] {
            return Ok(Json(self.engine.image_metadata(job, sequence, edit).await?).into_response());
        }
        if !tail.is_empty() {
            return Err(Error::new(404, "notFound", "Unknown image route."));
        }
        self.download(head, job, sequence, edit).await
    }
    async fn download(
        self: &Arc<Self>,
        head: &Parts,
        job: Uuid,
        sequence: u32,
        edit: Option<Uuid>,
    ) -> Result<Response> {
        if head.headers.contains_key(header::RANGE) {
            return Err(Error::new(
                416,
                "unsupported",
                "Range requests are not supported.",
            ));
        }
        let chunk = self
            .engine
            .read_image(job, sequence, edit, 0, 262144)
            .await?;
        let image = chunk.metadata.image.clone();
        let state = Download {
            gateway: self.clone(),
            job,
            sequence,
            edit,
            first: Some(chunk),
            offset: 0,
            digest: Sha256::new(),
            deadline: Instant::now() + Duration::from_secs(300),
            sha: image.sha256.clone(),
            bytes: image.bytes,
        };
        let body = Body::from_stream(futures_util::stream::try_unfold(
            state,
            |state| async move { state.next().await },
        ));
        let format = if image.uniform_type == "public.png" {
            "png"
        } else {
            "tiff"
        };
        let mut response = cached(head, body, &format!("image/{format}"), &image.sha256)?;
        response.headers_mut().insert(
            "x-image-sha256",
            image.sha256.parse().map_err(|_| Error::internal())?,
        );
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{job}-{sequence}.{format}\"")
                .parse()
                .map_err(|_| Error::internal())?,
        );
        if response.status() != StatusCode::NOT_MODIFIED {
            response.headers_mut().insert(
                header::CONTENT_LENGTH,
                image
                    .bytes
                    .to_string()
                    .parse()
                    .map_err(|_| Error::internal())?,
            );
        }
        Ok(response)
    }
}
struct Download {
    gateway: Arc<Gateway>,
    job: Uuid,
    sequence: u32,
    edit: Option<Uuid>,
    first: Option<ImageChunk>,
    offset: u64,
    digest: Sha256,
    deadline: Instant,
    sha: String,
    bytes: u64,
}
impl Download {
    async fn next(mut self) -> std::io::Result<Option<(Vec<u8>, Self)>> {
        if self.offset == self.bytes && self.first.is_none() {
            return Ok(None);
        }
        if Instant::now() >= self.deadline {
            return Err(std::io::Error::other("Image download timed out."));
        }
        let chunk = match self.first.take() {
            Some(chunk) => chunk,
            None => self
                .gateway
                .engine
                .read_image(self.job, self.sequence, self.edit, self.offset, 262144)
                .await
                .map_err(std::io::Error::other)?,
        };
        let bytes = STANDARD
            .decode(chunk.base64)
            .map_err(std::io::Error::other)?;
        let next = self.offset + bytes.len() as u64;
        if chunk.metadata.job_id != self.job
            || chunk.metadata.image.sha256 != self.sha
            || chunk.metadata.image.bytes != self.bytes
            || chunk.offset != self.offset
            || bytes.is_empty()
            || bytes.len() > 262144
            || chunk.next_offset != next
            || next > self.bytes
            || chunk.eof != (next == self.bytes)
            || format!("{:x}", Sha256::digest(&bytes)) != chunk.chunk_sha256
        {
            return Err(std::io::Error::other("Original chunk verification failed."));
        }
        self.digest.update(&bytes);
        if chunk.eof && format!("{:x}", self.digest.clone().finalize()) != self.sha {
            return Err(std::io::Error::other(
                "Original checksum verification failed.",
            ));
        }
        self.offset = next;
        Ok(Some((bytes, self)))
    }
}
pub(crate) fn cached(head: &Parts, body: Body, media_type: &str, sha: &str) -> Result<Response> {
    if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::internal());
    }
    let tag = format!("\"{sha}\"");
    let mut response = if header_value(head, header::IF_NONE_MATCH)? == Some(&tag) {
        (StatusCode::NOT_MODIFIED, Body::empty()).into_response()
    } else {
        body.into_response()
    };
    for (key, value) in [
        (header::CONTENT_TYPE, media_type),
        (header::ETAG, tag.as_str()),
        (header::CACHE_CONTROL, "private, max-age=86400, immutable"),
    ] {
        response
            .headers_mut()
            .insert(key, value.parse().map_err(|_| Error::internal())?);
    }
    Ok(response)
}
fn image_id(value: &str) -> Result<(Uuid, u32)> {
    let (job, sequence) = value
        .split_once(':')
        .ok_or_else(|| Error::params("Image ID must be JOB_UUID:SEQUENCE."))?;
    let job = Uuid::parse_str(&uuid(job)?).map_err(|_| Error::internal())?;
    let sequence = sequence
        .parse::<u32>()
        .ok()
        .filter(|v| (1..=1000).contains(v))
        .ok_or_else(|| Error::params("Invalid image sequence."))?;
    Ok((job, sequence))
}
