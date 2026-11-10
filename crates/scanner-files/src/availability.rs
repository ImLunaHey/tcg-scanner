use super::*;
use scanner_types::{device::Device, scan::ScanRequest};

#[test]
fn deferred_storage_never_creates_missing_directories_and_connects_when_available() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("unmounted");
    let root = StorageRoot::deferred(configuration::StorageConfiguration {
        storage_root: path.to_string_lossy().into(),
        required_smb_share: None,
    })
    .unwrap();
    assert_eq!(
        root.check_available().unwrap_err().kind,
        "storageUnavailable"
    );
    assert!(BatchStore::create(&root, Device::default(), ScanRequest::default(), 1).is_err());
    assert!(!path.exists());
    std::fs::create_dir(&path).unwrap();
    root.check_available().unwrap();
    let batch = BatchStore::create(&root, Device::default(), ScanRequest::default(), 1).unwrap();
    assert!(path.join("B-001/manifest.json").exists());
    assert_eq!(batch.record().batch_number, Some(1));
}
#[test]
fn deferred_smb_policy_never_accepts_a_local_fallback() {
    let temporary = tempfile::tempdir().unwrap();
    let root = StorageRoot::deferred(configuration::StorageConfiguration {
        storage_root: temporary.path().to_string_lossy().into(),
        required_smb_share: Some("smb://lake/scans".into()),
    })
    .unwrap();
    assert!(root.check_available().is_err());
    assert!(root.reconnect().is_err());
    assert!(BatchStore::create(&root, Device::default(), ScanRequest::default(), 1).is_err());
    assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 0);
}
#[cfg(unix)]
#[test]
fn handles_stay_pinned_until_explicit_reconnect() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("root");
    std::fs::create_dir(&path).unwrap();
    let root = StorageRoot::open(&path).unwrap();
    std::fs::rename(&path, temporary.path().join("retained")).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(
        root.directory()
            .unwrap()
            .symlink_metadata("sentinel")
            .is_err()
    );
    std::fs::write(path.join("sentinel"), b"replacement").unwrap();
    assert!(
        root.directory()
            .unwrap()
            .symlink_metadata("sentinel")
            .is_err()
    );
    root.reconnect().unwrap();
    assert!(
        root.directory()
            .unwrap()
            .symlink_metadata("sentinel")
            .is_ok()
    );
}
