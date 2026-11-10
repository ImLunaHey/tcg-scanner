use scanner_core::recover_before_start;
use scanner_files::{BatchStore, StorageRoot};
use scanner_store::Database;
use scanner_types::{
    device::Device,
    scan::{ScanRequest, ScanState},
};
use serde_json::json;
struct Fixture {
    _temp: tempfile::TempDir,
    root: StorageRoot,
    db: Database,
    batch: BatchStore,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = StorageRoot::open(temp.path()).unwrap();
        let batch =
            BatchStore::create(&root, Device::default(), ScanRequest::default(), 1).unwrap();
        let db = Database::memory().unwrap();
        db.insert(batch.record(), &batch.record().request).unwrap();
        Self {
            _temp: temp,
            root,
            db,
            batch,
        }
    }
    fn preserve(&mut self) {
        let image = scanner_images::encode_png(image::RgbaImage::from_pixel(
            6,
            9,
            image::Rgba([50, 80, 90, 255]),
        ))
        .unwrap();
        std::fs::write(self.batch.incoming_path().join("capture.png"), image.bytes).unwrap();
        self.batch.preserve("capture.png", 1).unwrap();
    }
}
#[test]
fn adopts_newer_verified_manifest_and_retains_originals_without_resuming() {
    let mut f = Fixture::new();
    f.preserve();
    f.batch
        .update(|r| {
            r.state = ScanState::Scanning;
            r.extensions.insert(
                "cardIdentifications".into(),
                json!([{"state":"processing"},{"state":"confirmed","selectedCardID":"sv09-177"}]),
            );
        })
        .unwrap();
    let original = f.batch.record().images[0].clone();
    assert_eq!(recover_before_start(&f.db, Some(&f.root)).unwrap(), 1);
    let record = f.db.job(f.batch.record().id).unwrap().unwrap();
    assert_eq!(record.state, ScanState::Failed);
    assert!(!record.session_closed);
    assert_eq!(record.images.len(), 1);
    assert_eq!(record.images[0].sha256, original.sha256);
    assert_eq!(
        record.extensions["cardIdentifications"][0]["state"],
        "failed"
    );
    assert_eq!(
        record.extensions["cardIdentifications"][1]["selectedCardID"],
        "sv09-177"
    );
    f.root.verify_image(&record, &original).unwrap();
    assert_eq!(
        f.root.read_manifest(&record).unwrap().state,
        ScanState::Failed
    );
    assert_eq!(recover_before_start(&f.db, Some(&f.root)).unwrap(), 0);
}
#[test]
fn completed_disk_state_and_interrupted_continuations_are_reconciled() {
    let mut f = Fixture::new();
    f.preserve();
    f.batch.finish(vec![], false, true).unwrap();
    recover_before_start(&f.db, Some(&f.root)).unwrap();
    let record = f.db.job(f.batch.record().id).unwrap().unwrap();
    assert_eq!(record.state, ScanState::Completed);
    assert!(record.session_closed);
    let mut record = record;
    record.state = ScanState::Scanning;
    record.extensions.insert(
        "continuations".into(),
        json!([{"requestID":uuid::Uuid::new_v4(),"firstSequence":2,"state":"scanning"}]),
    );
    f.db.save(&record).unwrap();
    f.root.save_manifest(&record).unwrap();
    recover_before_start(&f.db, Some(&f.root)).unwrap();
    let record = f.db.job(record.id).unwrap().unwrap();
    assert_eq!(record.extensions["continuations"][0]["state"], "failed");
    assert_eq!(record.extensions["continuations"][0]["imagesSaved"], 0);
}
#[test]
fn mismatched_or_corrupt_disk_metadata_is_preserved_for_inspection() {
    for mode in ["request", "continuation", "review", "checksum"] {
        let mut f = Fixture::new();
        f.preserve();
        let mut disk = f.batch.record().clone();
        match mode {
            "request" => disk.request.duplex = true,
            "continuation" => {
                disk.extensions.insert(
                    "continuations".into(),
                    json!([{"requestID":uuid::Uuid::new_v4()}]),
                );
            }
            "review" => {
                let mut record = f.db.job(disk.id).unwrap().unwrap();
                record.extensions.insert(
                    "cardIdentifications".into(),
                    json!([{"state":"confirmed","selectedCardID":"reviewed"}]),
                );
                f.db.save(&record).unwrap();
            }
            _ => {
                let path = std::path::Path::new(&disk.output_directory)
                    .join(&disk.images[0].relative_path);
                let mut bytes = std::fs::read(&path).unwrap();
                bytes[0] ^= 1;
                std::fs::write(path, bytes).unwrap();
            }
        }
        f.root.save_manifest(&disk).unwrap();
        let manifest = std::path::Path::new(&disk.output_directory).join("manifest.json");
        let before = std::fs::read(&manifest).unwrap();
        recover_before_start(&f.db, Some(&f.root)).unwrap();
        let record = f.db.job(disk.id).unwrap().unwrap();
        assert_eq!(record.state, ScanState::Failed);
        assert!(record.images.is_empty());
        assert!(
            record
                .warnings
                .iter()
                .any(|w| w.contains("not adopted or overwritten"))
        );
        assert_eq!(std::fs::read(manifest).unwrap(), before);
        if mode == "review" {
            assert_eq!(
                record.extensions["cardIdentifications"][0]["selectedCardID"],
                "reviewed"
            );
        }
    }
}
#[test]
fn unavailable_storage_and_terminal_pending_identifications_retain_history() {
    let mut f = Fixture::new();
    f.preserve();
    f.batch.finish(vec![], false, true).unwrap();
    let mut record = f.batch.record().clone();
    record.extensions.insert(
        "cardIdentifications".into(),
        json!([{"state":"processing"},{"state":"confirmed","selectedCardID":"reviewed"}]),
    );
    f.db.save(&record).unwrap();
    let manifest = std::path::Path::new(&record.output_directory).join("manifest.json");
    let before = std::fs::read(&manifest).unwrap();
    assert_eq!(recover_before_start(&f.db, None).unwrap(), 1);
    let restored = f.db.job(record.id).unwrap().unwrap();
    assert_eq!(restored.state, ScanState::Completed);
    assert_eq!(restored.images[0].sha256, record.images[0].sha256);
    assert_eq!(
        restored.extensions["cardIdentifications"][0]["state"],
        "failed"
    );
    assert_eq!(
        restored.extensions["cardIdentifications"][1]["selectedCardID"],
        "reviewed"
    );
    assert_eq!(std::fs::read(manifest).unwrap(), before);
}
#[cfg(unix)]
#[test]
fn linked_manifests_are_not_read_or_overwritten() {
    for symbolic in [true, false] {
        let f = Fixture::new();
        let record = f.batch.record();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), b"retained outside bytes").unwrap();
        let path = std::path::Path::new(&record.output_directory).join("manifest.json");
        std::fs::remove_file(&path).unwrap();
        if symbolic {
            std::os::unix::fs::symlink(outside.path(), &path).unwrap();
        } else {
            std::fs::hard_link(outside.path(), &path).unwrap();
        }
        recover_before_start(&f.db, Some(&f.root)).unwrap();
        assert_eq!(
            std::fs::read(outside.path()).unwrap(),
            b"retained outside bytes"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"retained outside bytes");
        assert_eq!(
            f.db.job(record.id).unwrap().unwrap().state,
            ScanState::Failed
        );
    }
}
