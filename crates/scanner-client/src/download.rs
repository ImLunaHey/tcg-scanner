//! Incremental verified image download, without filesystem or UI policy.
use crate::Client;
use base64::{Engine, engine::general_purpose::STANDARD};
use scanner_types::{
    error::{Result, ScannerError},
    image_access::{ImageChunk, ImageMetadata},
    images::OriginalImage,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;
const CHUNK: usize = 262_144;

/// Keep output private/incomplete until `is_complete` becomes true. Every returned
/// chunk has its own hash checked; the final chunk also verifies the whole image.
pub struct Download {
    client: Client,
    edit: Option<Uuid>,
    verifier: Verifier,
}
struct Verifier {
    metadata: ImageMetadata,
    offset: u64,
    hash: Sha256,
    complete: bool,
    failed: bool,
}
impl Client {
    /// Optionally bind a download to an immutable image from a saved manifest.
    pub async fn download(
        &self,
        job: Uuid,
        sequence: u32,
        edit: Option<Uuid>,
        expected: Option<&OriginalImage>,
    ) -> Result<Download> {
        if !(1..=1000).contains(&sequence) {
            return Err(integrity("Invalid image sequence."));
        }
        let mut params = json!({"jobID":job,"sequence":sequence});
        if let Some(edit) = edit {
            params["editID"] = json!(edit);
        }
        let metadata: ImageMetadata =
            serde_json::from_value(self.call("images.get", params).await?)
                .map_err(|_| integrity("Invalid image metadata."))?;
        let image = &metadata.image;
        if metadata.schema_version != 1
            || metadata.job_id != job
            || image.sequence != sequence
            || !(1..=512 * 1024 * 1024).contains(&image.bytes)
            || image.sha256.len() != 64
            || !image
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || expected.is_some_and(|e| !same_image(image, e))
        {
            return Err(integrity(
                "Image metadata does not match the requested saved image.",
            ));
        }
        Ok(Download {
            client: self.clone(),
            edit,
            verifier: Verifier {
                metadata,
                offset: 0,
                hash: Sha256::new(),
                complete: false,
                failed: false,
            },
        })
    }
}
impl Download {
    pub fn metadata(&self) -> &ImageMetadata {
        &self.verifier.metadata
    }
    pub fn is_complete(&self) -> bool {
        self.verifier.complete
    }
    /// A transport failure can be retried at the same offset. An integrity failure
    /// permanently invalidates this download; never publish its partial output.
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        if self.verifier.failed {
            return Err(integrity("This download failed integrity validation."));
        }
        if self.verifier.complete {
            return Ok(None);
        }
        let mut params = json!({"jobID":self.verifier.metadata.job_id,"sequence":self.verifier.metadata.image.sequence,"offset":self.verifier.offset,"length":CHUNK});
        if let Some(edit) = self.edit {
            params["editID"] = json!(edit);
        }
        let value = self.client.call("images.read", params).await?;
        let chunk =
            serde_json::from_value(value).map_err(|_| integrity("Invalid image chunk envelope."));
        let result = chunk.and_then(|chunk| self.verifier.accept(chunk));
        if result.is_err() {
            self.verifier.failed = true;
        }
        result.map(Some)
    }
}
impl Verifier {
    fn accept(&mut self, chunk: ImageChunk) -> Result<Vec<u8>> {
        if self.failed || self.complete || chunk.base64.len() > CHUNK.div_ceil(3) * 4 {
            return Err(integrity("Invalid image chunk length."));
        }
        let bytes = STANDARD
            .decode(&chunk.base64)
            .map_err(|_| integrity("Invalid image chunk encoding."))?;
        let next = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| integrity("Invalid image chunk offset."))?;
        if chunk.schema_version != 1
            || chunk.metadata.schema_version != 1
            || chunk.metadata.job_id != self.metadata.job_id
            || !same_image(&chunk.metadata.image, &self.metadata.image)
            || bytes.is_empty()
            || bytes.len() > CHUNK
            || chunk.offset != self.offset
            || chunk.next_offset != next
            || next > self.metadata.image.bytes
            || chunk.eof != (next == self.metadata.image.bytes)
            || format!("{:x}", Sha256::digest(&bytes)) != chunk.chunk_sha256
        {
            return Err(integrity(
                "Image chunk identity, offsets or checksum failed validation.",
            ));
        }
        let mut hash = self.hash.clone();
        hash.update(&bytes);
        if chunk.eof && format!("{:x}", hash.clone().finalize()) != self.metadata.image.sha256 {
            return Err(integrity("Downloaded image checksum mismatch."));
        }
        self.hash = hash;
        self.offset = next;
        self.complete = chunk.eof;
        Ok(bytes)
    }
}
fn same_image(a: &OriginalImage, b: &OriginalImage) -> bool {
    a.sequence == b.sequence
        && a.bytes == b.bytes
        && a.sha256 == b.sha256
        && a.width == b.width
        && a.height == b.height
        && a.image_count == b.image_count
        && a.uniform_type == b.uniform_type
        && a.relative_path == b.relative_path
}
fn integrity(message: &str) -> ScannerError {
    ScannerError::new("integrity", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn verifier(bytes: &[u8]) -> Verifier {
        let image:OriginalImage=serde_json::from_value(json!({"sequence":1,"relativePath":"C-001.png","bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),"width":100,"height":150,"imageCount":1,"uniformType":"public.png","side":"unknown"})).unwrap();
        Verifier {
            metadata: ImageMetadata {
                schema_version: 1,
                job_id: Uuid::new_v4(),
                image,
            },
            offset: 0,
            hash: Sha256::new(),
            complete: false,
            failed: false,
        }
    }
    fn chunk(v: &Verifier, bytes: &[u8], eof: bool) -> ImageChunk {
        ImageChunk {
            schema_version: 1,
            metadata: v.metadata.clone(),
            offset: v.offset,
            next_offset: v.offset + bytes.len() as u64,
            eof,
            base64: STANDARD.encode(bytes),
            chunk_sha256: format!("{:x}", Sha256::digest(bytes)),
        }
    }
    #[test]
    fn chunks_and_whole_image_are_verified_before_completion() {
        let mut v = verifier(b"abcdef");
        assert_eq!(v.accept(chunk(&v, b"abc", false)).unwrap(), b"abc");
        assert!(!v.complete);
        let wrong = chunk(&v, b"xyz", true);
        assert!(v.accept(wrong).is_err());
        assert_eq!(v.offset, 3);
        assert!(!v.complete);
        assert_eq!(v.accept(chunk(&v, b"def", true)).unwrap(), b"def");
        assert!(v.complete);
        assert!(v.accept(chunk(&v, b"", true)).is_err());
    }
    #[test]
    fn altered_identity_boundaries_and_chunk_hashes_cannot_advance_downloads() {
        for field in [
            "job", "sequence", "path", "bytes", "offset", "next", "eof", "hash", "empty",
        ] {
            let mut v = verifier(b"abcdef");
            let mut c = chunk(&v, b"abc", false);
            match field {
                "job" => c.metadata.job_id = Uuid::new_v4(),
                "sequence" => c.metadata.image.sequence = 2,
                "path" => c.metadata.image.relative_path = "other.png".into(),
                "bytes" => c.metadata.image.bytes = 7,
                "offset" => c.offset = 1,
                "next" => c.next_offset = 4,
                "eof" => c.eof = true,
                "hash" => c.chunk_sha256 = "0".repeat(64),
                "empty" => c.base64.clear(),
                _ => unreachable!(),
            }
            assert!(v.accept(c).is_err(), "{field}");
            assert_eq!(v.offset, 0);
            assert!(!v.complete);
        }
    }
}
