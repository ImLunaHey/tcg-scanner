use super::BatchStore;
use crate::{StorageRoot, paths, storage};
use cap_fs_ext::DirExt;
use scanner_types::{
    error::Result,
    scan::{ScanRecord, ScanState},
};
use serde_json::json;
use std::path::{Path, PathBuf};
use uuid::Uuid;

impl BatchStore {
    /// Reserve the continuation in SQLite before calling publish on this prepared state.
    pub fn continuing(root: &StorageRoot, previous: &ScanRecord, request_id: Uuid) -> Result<Self> {
        root.validate()?;
        if previous.state != ScanState::Completed
            || !previous.session_closed
            || !previous.errors.is_empty()
            || previous.images.is_empty()
            || previous
                .images
                .iter()
                .enumerate()
                .any(|(i, image)| image.sequence != i as u32 + 1)
            || previous.request.duplex && !previous.images.len().is_multiple_of(2)
        {
            return Err(storage(
                "Only a completed batch with contiguous captures and complete duplex pairs can be continued.",
            ));
        }
        if previous.request.dpi != 600 || previous.images.len() > 900 {
            return Err(storage(
                "Continuing requires 600 DPI and room for another 100 images.",
            ));
        }
        let dir = root.batch_dir(previous)?;
        let incoming_name = format!(".incoming-{request_id}");
        paths::create_directory(&dir, Path::new(&incoming_name)).map_err(storage)?;
        let incoming = dir.open_dir_nofollow(&incoming_name).map_err(storage)?;
        let incoming_path = PathBuf::from(&previous.output_directory).join(incoming_name);
        let mut record = previous.clone();
        let continuation = json!({"requestID":request_id,"firstSequence":previous.images.len()+1,
            "createdAt":chrono::Utc::now(),"previousCompletedAt":previous.completed_at,"state":"queued","imagesSaved":0});
        let feeds = record
            .extensions
            .entry("continuations".into())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| storage("Invalid saved continuation metadata."))?;
        feeds.push(continuation);
        record.state = ScanState::Queued;
        record.completed_at = None;
        record.session_closed = false;
        Ok(Self {
            dir,
            incoming,
            incoming_path,
            record,
            starting_count: previous.images.len() as u32,
            policy: root.policy.clone(),
        })
    }
    pub fn publish(&self) -> Result<()> {
        self.validate()?;
        super::write_manifest(&self.dir, &self.record)
    }
    pub(super) fn validate(&self) -> Result<()> {
        self.policy.as_ref().map(|p| p.validate()).unwrap_or(Ok(()))
    }

    pub(super) fn update_continuation(&self, record: &mut ScanRecord) {
        if self.starting_count == 0 {
            return;
        }
        if let Some(last) = record
            .extensions
            .get_mut("continuations")
            .and_then(|v| v.as_array_mut())
            .and_then(|v| v.last_mut())
        {
            last["state"] = json!(record.state);
            last["completedAt"] = json!(record.completed_at);
            last["imagesSaved"] = json!(
                record
                    .images
                    .len()
                    .saturating_sub(self.starting_count as usize)
            );
            if record.state == ScanState::Preparing && last.get("startedAt").is_none() {
                last["startedAt"] = json!(chrono::Utc::now());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{device::Device, scan::ScanRequest};
    #[test]
    fn next_feed_keeps_batch_identity_and_appends_sequences() {
        let temp = tempfile::tempdir().unwrap();
        let root = StorageRoot::open(temp.path()).unwrap();
        let mut first =
            BatchStore::create(&root, Device::default(), ScanRequest::default(), 1).unwrap();
        let png = scanner_images::encode_png(image::RgbaImage::from_pixel(
            6,
            9,
            image::Rgba([12, 34, 56, 255]),
        ))
        .unwrap();
        std::fs::write(first.incoming_path().join("capture.png"), &png.bytes).unwrap();
        first.preserve("capture.png", 1).unwrap();
        first.finish(vec![], false, true).unwrap();
        let before = first.record().clone();
        let id = Uuid::new_v4();
        let mut next = BatchStore::continuing(&root, &before, id).unwrap();
        assert_eq!(next.record().id, before.id);
        assert_eq!(next.record().images.len(), 1);
        std::fs::write(next.incoming_path().join("capture.png"), &png.bytes).unwrap();
        next.publish().unwrap();
        next.preserve("capture.png", 1).unwrap();
        next.finish(vec![], false, true).unwrap();
        assert_eq!(next.record().images[1].sequence, 2);
        assert_eq!(next.record().images[0].sha256, before.images[0].sha256);
        assert_eq!(
            next.record().extensions["continuations"][0]["imagesSaved"],
            1
        );
        assert!(BatchStore::continuing(&root, &before, id).is_err());
    }
}
