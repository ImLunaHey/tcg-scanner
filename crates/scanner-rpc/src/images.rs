use crate::{Failure, Router, model::value, params::Params};
use scanner_types::{image_access::ThumbnailFormat, images::ImageEditRequest};
use serde_json::Value;

impl Router {
    pub(crate) async fn images(
        &self,
        method: &str,
        params: &Params,
    ) -> Option<Result<Value, Failure>> {
        if !matches!(
            method,
            "v1.images.get" | "v1.images.read" | "v1.images.thumbnail" | "v1.images.edit"
        ) {
            return None;
        }
        Some(self.image_action(method, params).await)
    }
    async fn image_action(&self, method: &str, params: &Params) -> Result<Value, Failure> {
        if method == "v1.images.edit" {
            params.only(&[
                "id",
                "jobID",
                "sequence",
                "crop",
                "quarterTurns",
                "deskewDegrees",
            ])?;
            let crop = params
                .0
                .get("crop")
                .and_then(Value::as_object)
                .ok_or_else(|| Failure::params("Expected a normalized crop rectangle."))?;
            if crop.len() != 4
                || !["x", "y", "width", "height"]
                    .iter()
                    .all(|key| crop.contains_key(*key))
            {
                return Err(Failure::params("Expected x, y, width and height in crop."));
            }
            let request: ImageEditRequest = params.decode()?;
            request.validate().map_err(|e| Failure::params(e.message))?;
            return value(self.engine.edit_image(request).await?);
        }
        params.only(match method {
            "v1.images.read" => &["jobID", "sequence", "editID", "offset", "length"],
            "v1.images.thumbnail" => &["jobID", "sequence", "editID", "format"],
            _ => &["jobID", "sequence", "editID"],
        })?;
        let job = params.uuid("jobID")?;
        let sequence = params.integer("sequence", 0, 1, 1000)? as u32;
        if sequence == 0 {
            return Err(Failure::params("sequence is required."));
        }
        let edit = params
            .0
            .get("editID")
            .map(|_| params.uuid("editID"))
            .transpose()?;
        match method {
            "v1.images.read" => value(
                self.engine
                    .read_image(
                        job,
                        sequence,
                        edit,
                        params.integer("offset", 0, 0, 512 * 1024 * 1024)?,
                        params.integer("length", 65536, 1, 262144)? as usize,
                    )
                    .await?,
            ),
            "v1.images.thumbnail" => {
                let format = match params
                    .optional_identifier("format")?
                    .as_deref()
                    .unwrap_or("png")
                {
                    "png" => ThumbnailFormat::Png,
                    "jpeg" => ThumbnailFormat::Jpeg,
                    _ => return Err(Failure::params("Thumbnail format must be png or jpeg.")),
                };
                value(self.engine.thumbnail(job, sequence, edit, format).await?)
            }
            _ => value(self.engine.image_metadata(job, sequence, edit).await?),
        }
    }
}
