use crate::{Batch, Entry, MEMORY_IMAGE_LIMIT, error};
use scanner_client::Client;
use scanner_types::error::Result;
use std::io::{Cursor, Write};
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

pub async fn image_bytes(client: &Client, job: Uuid, entry: &Entry) -> Result<Vec<u8>> {
    if entry.image.bytes > MEMORY_IMAGE_LIMIT {
        return Err(error(
            "This image exceeds the 64 MiB browser memory limit. Export it from the desktop app.",
        ));
    }
    let mut download = client
        .download(job, entry.image.sequence, entry.edit_id, Some(&entry.image))
        .await?;
    let mut bytes = Vec::with_capacity(entry.image.bytes as usize);
    while let Some(chunk) = download.next().await? {
        bytes.extend_from_slice(&chunk);
    }
    if !download.is_complete() {
        return Err(error("Image export is incomplete."));
    }
    Ok(bytes)
}
/// Store already-compressed PNG/TIFF bytes in a ZIP, adding the completion
/// manifest last. Nothing is returned when any image fails integrity validation.
pub async fn archive(client: &Client, batch: &Batch) -> Result<Vec<u8>> {
    if batch.total > MEMORY_IMAGE_LIMIT {
        return Err(error(
            "This batch exceeds the 64 MiB browser export limit. Export individual images or use the desktop app.",
        ));
    }
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o600);
    for entry in &batch.entries {
        zip.start_file(format!("{}/{}", batch.name, entry.name), options)
            .map_err(|e| error(e.to_string()))?;
        let mut download = client
            .download(
                batch.id,
                entry.image.sequence,
                entry.edit_id,
                Some(&entry.image),
            )
            .await?;
        while let Some(chunk) = download.next().await? {
            zip.write_all(&chunk).map_err(|e| error(e.to_string()))?;
        }
        if !download.is_complete() {
            return Err(error("Batch export is incomplete."));
        }
    }
    zip.start_file(format!("{}/manifest.json", batch.name), options)
        .map_err(|e| error(e.to_string()))?;
    zip.write_all(&batch.manifest)
        .map_err(|e| error(e.to_string()))?;
    Ok(zip.finish().map_err(|e| error(e.to_string()))?.into_inner())
}
