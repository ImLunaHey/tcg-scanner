//! Streaming exports to explicitly selected native destinations.
use crate::{Batch, Entry, error};
use scanner_client::Client;
use scanner_types::error::Result;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use uuid::Uuid;

pub async fn image(client: &Client, job: Uuid, entry: &Entry, destination: PathBuf) -> Result<()> {
    let mut download = client
        .download(job, entry.image.sequence, entry.edit_id, Some(&entry.image))
        .await?;
    let target = destination.clone();
    let mut temporary = blocking(move || temporary(&target)).await?;
    while let Some(bytes) = download.next().await? {
        temporary = blocking(move || {
            temporary.write_all(&bytes).map_err(io)?;
            Ok(temporary)
        })
        .await?;
    }
    if !download.is_complete() {
        return Err(error("Image export is incomplete."));
    }
    blocking(move || publish(temporary, &destination)).await
}
pub async fn json(bytes: Vec<u8>, destination: PathBuf) -> Result<()> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(error("JSON export exceeds 16 MiB."));
    }
    blocking(move || {
        let mut temporary = temporary(&destination)?;
        temporary.write_all(&bytes).map_err(io)?;
        publish(temporary, &destination)
    })
    .await
}
pub async fn batch(client: &Client, batch: &Batch, parent: PathBuf) -> Result<PathBuf> {
    if !parent.is_absolute() {
        return Err(error("Choose an absolute export directory."));
    }
    let root = parent.join(&batch.name);
    let target = root.clone();
    blocking(move || {
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut private = builder;
            private.mode(0o700);
            private
        };
        builder.create(target).map_err(io)
    })
    .await?;
    match batch_files(client, batch, &root).await {
        Ok(()) => Ok(root),
        Err(e) => Err(error(format!(
            "Export could not be completed durably at {}. Verified files are retained. {}",
            root.display(),
            e.message
        ))),
    }
}
async fn batch_files(client: &Client, batch: &Batch, root: &Path) -> Result<()> {
    for entry in &batch.entries {
        image(client, batch.id, entry, root.join(&entry.name)).await?;
    }
    json(batch.manifest.clone(), root.join("manifest.json")).await
}
fn temporary(destination: &Path) -> Result<NamedTempFile> {
    if !destination.is_absolute() || destination.file_name().is_none() {
        return Err(error("Choose an absolute export filename."));
    }
    if destination.try_exists().map_err(io)? {
        return Err(error(
            "An export already exists here. Choose another filename.",
        ));
    }
    tempfile::Builder::new()
        .prefix(".scanner-export-")
        .tempfile_in(destination.parent().unwrap())
        .map_err(io)
}
fn publish(file: NamedTempFile, destination: &Path) -> Result<()> {
    file.as_file().sync_all().map_err(io)?;
    let saved = file
        .persist_noclobber(destination)
        .map_err(|e| io(e.error))?;
    saved.sync_all().map_err(io)?;
    #[cfg(unix)]
    std::fs::File::open(destination.parent().unwrap())
        .map_err(io)?
        .sync_all()
        .map_err(io)?;
    Ok(())
}
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| error("Export worker failed."))?
}
fn io(e: impl std::fmt::Display) -> scanner_types::error::ScannerError {
    error(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn destination_races_and_existing_exports_never_replace_user_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("export.json");
        json(b"first".to_vec(), path.clone()).await.unwrap();
        assert!(json(b"second".to_vec(), path.clone()).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        let raced = directory.path().join("race.json");
        let mut temp = temporary(&raced).unwrap();
        temp.write_all(b"new").unwrap();
        std::fs::write(&raced, b"existing").unwrap();
        assert!(publish(temp, &raced).is_err());
        assert_eq!(std::fs::read(&raced).unwrap(), b"existing");
        assert!(std::fs::read_dir(directory.path()).unwrap().all(|e| {
            !e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".scanner-export-")
        }));
    }
}
