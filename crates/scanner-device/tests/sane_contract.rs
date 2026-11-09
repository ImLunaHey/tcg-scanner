//! Optional native ABI test: build Tools/NativeDeviceTests/sane.c and set
//! SCANNER_SANE_TEST_LIBRARY to that trusted test library's absolute path.
#![cfg(unix)]
use scanner_device::{CaptureEvent, ScannerBackend, sane::SaneBackend};
use scanner_types::scan::ScanRequest;
use std::{path::PathBuf, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct Fixture {
    library: libloading::Library,
}
impl Fixture {
    fn configure(&self, scenario: i32) {
        // SAFETY: caller explicitly selects the test-only library above.
        unsafe {
            self.library
                .get::<unsafe extern "C" fn(i32)>(b"fixture_configure\0")
                .unwrap()(scenario);
        }
    }
    fn counter(&self, index: i32) -> i32 {
        // SAFETY: fixture counters are read atomically except resolution, which
        // this test reads only after capture cleanup has returned.
        unsafe {
            self.library
                .get::<unsafe extern "C" fn(i32) -> i32>(b"fixture_counter\0")
                .unwrap()(index)
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_settings_capture_restore_and_cancellation_contract() {
    let Some(path) = std::env::var_os("SCANNER_SANE_TEST_LIBRARY") else {
        return;
    };
    let path = PathBuf::from(path);
    let fixture = Fixture {
        library: unsafe { libloading::Library::new(&path).unwrap() },
    };
    fixture.configure(0);
    let backend = SaneBackend::new(path).await.unwrap();
    let device = backend.devices().await.unwrap().remove(0);
    assert_eq!(device.id, "sane:test:cards");
    let inspection = backend.inspect(&device.id).await.unwrap();
    assert!(inspection.session_opened && inspection.session_closed);
    assert_eq!(fixture.counter(0), 0);
    normal_capture(&backend, &fixture, &device.id).await;
    failed_settings(&backend, &fixture, &device.id).await;
    rejected_persistence(&backend, &fixture, &device.id).await;
    cancelled_read(&backend, &fixture, &device.id).await;
    dropped_caller(&backend, &fixture, &device.id).await;
}
async fn normal_capture(backend: &SaneBackend, fixture: &Fixture, id: &str) {
    fixture.configure(0);
    let directory = tempfile::tempdir().unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    let backend = backend.clone();
    let id = id.to_owned();
    let incoming = directory.path().to_path_buf();
    let task = tokio::spawn(async move {
        backend
            .capture(
                &id,
                &ScanRequest {
                    duplex: true,
                    ..ScanRequest::default()
                },
                incoming,
                tx,
                CancellationToken::new(),
            )
            .await
            .unwrap()
    });
    let CaptureEvent::Settings {
        settings,
        acknowledged,
    } = rx.recv().await.unwrap()
    else {
        panic!("settings first");
    };
    assert_eq!(fixture.counter(0), 0);
    assert_eq!(settings["source"], "ADF Duplex");
    assert_eq!(settings["resolution"], 600);
    assert_eq!(settings["swskip"], 0.0);
    acknowledged.send(Ok(())).unwrap();
    for expected in 1..=2 {
        let CaptureEvent::Image { filename, sequence } = rx.recv().await.unwrap() else {
            panic!("image expected");
        };
        assert_eq!(sequence, expected);
        let image = image::open(directory.path().join(filename))
            .unwrap()
            .into_rgb8();
        assert_eq!(image.dimensions(), (2, 2));
        assert_eq!(image.as_raw(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    }
    let result = task.await.unwrap();
    assert!(result.session_closed && result.errors.is_empty());
    assert_eq!(fixture.counter(1), 1);
    assert_eq!(fixture.counter(2), 300);
}
async fn failed_settings(backend: &SaneBackend, fixture: &Fixture, id: &str) {
    fixture.configure(2);
    let directory = tempfile::tempdir().unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    let result = backend
        .capture(
            id,
            &ScanRequest::default(),
            directory.path().into(),
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(result.session_closed && !result.errors.is_empty());
    assert_eq!(fixture.counter(0), 0);
    assert_eq!(fixture.counter(2), 300);
    assert!(rx.recv().await.is_none());
}
async fn cancelled_read(backend: &SaneBackend, fixture: &Fixture, id: &str) {
    fixture.configure(1);
    let directory = tempfile::tempdir().unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    let cancellation = CancellationToken::new();
    let token = cancellation.clone();
    let backend = backend.clone();
    let id = id.to_owned();
    let incoming = directory.path().to_path_buf();
    let task = tokio::spawn(async move {
        backend
            .capture(&id, &ScanRequest::default(), incoming, tx, token)
            .await
            .unwrap()
    });
    let CaptureEvent::Settings { acknowledged, .. } = rx.recv().await.unwrap() else {
        panic!("settings first");
    };
    acknowledged.send(Ok(())).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.counter(3) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    cancellation.cancel();
    let result = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.cancelled && result.session_closed);
    assert_eq!(fixture.counter(1), 1);
    assert_eq!(fixture.counter(2), 300);
    assert_eq!(fixture.counter(4), 0);
    assert!(fixture.counter(5) > 0);
}

async fn rejected_persistence(backend: &SaneBackend, fixture: &Fixture, id: &str) {
    fixture.configure(0);
    let directory = tempfile::tempdir().unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    let backend = backend.clone();
    let id = id.to_owned();
    let incoming = directory.path().to_path_buf();
    let task = tokio::spawn(async move {
        backend
            .capture(
                &id,
                &ScanRequest::default(),
                incoming,
                tx,
                CancellationToken::new(),
            )
            .await
            .unwrap()
    });
    let CaptureEvent::Settings { acknowledged, .. } = rx.recv().await.unwrap() else {
        panic!("settings first");
    };
    acknowledged
        .send(Err(scanner_types::error::ScannerError::new(
            "storage",
            "Simulated disk failure",
        )))
        .unwrap();
    let result = task.await.unwrap();
    assert!(result.session_closed);
    assert_eq!(result.errors[0].kind, "storage");
    assert_eq!(fixture.counter(0), 0);
    assert_eq!(fixture.counter(2), 300);
}
async fn dropped_caller(backend: &SaneBackend, fixture: &Fixture, id: &str) {
    fixture.configure(1);
    let directory = tempfile::tempdir().unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    let cancellation = CancellationToken::new();
    let token = cancellation.clone();
    let backend = backend.clone();
    let id = id.to_owned();
    let incoming = directory.path().to_path_buf();
    let task = tokio::spawn(async move {
        backend
            .capture(&id, &ScanRequest::default(), incoming, tx, token)
            .await
            .unwrap()
    });
    let CaptureEvent::Settings { acknowledged, .. } = rx.recv().await.unwrap() else {
        panic!("settings first");
    };
    acknowledged.send(Ok(())).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.counter(3) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    cancellation.cancel();
    tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.counter(1) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(fixture.counter(4), 0);
    assert_eq!(fixture.counter(2), 300);
}
