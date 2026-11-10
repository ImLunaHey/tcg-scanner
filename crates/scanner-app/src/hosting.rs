use crate::configuration::{Backend, Configuration, config};
use scanner_device::{CaptureEvent, CaptureOutcome, ScannerBackend};
use scanner_types::{
    device::{Device, Inspection},
    error::{Result, ScannerError},
    scan::ScanRequest,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) fn run(configuration: Configuration) -> Result<()> {
    // Acquire the same global per-user owner as Swift before native discovery,
    // opening the ledger or constructing an application engine.
    let _owner = scanner_platform::HardwareOwner::acquire()?;
    let runtime = tokio::runtime::Runtime::new().map_err(config)?;
    if matches!(configuration.backend, Backend::LibraryOnly) {
        return runtime.block_on(crate::service::run(
            configuration,
            Arc::new(LibraryOnly),
            "Library only",
        ));
    }
    native(configuration, &runtime)
}
#[cfg(target_os = "macos")]
fn native(configuration: Configuration, runtime: &tokio::runtime::Runtime) -> Result<()> {
    let (mut host, backend) = scanner_device::macos::MacHost::new(runtime.handle().clone())?;
    let task = runtime.spawn(crate::service::run(
        configuration,
        Arc::new(backend),
        "ImageCaptureCore",
    ));
    while !task.is_finished() {
        host.tick();
    }
    runtime.block_on(task).map_err(config)?
}
#[cfg(target_os = "linux")]
fn native(configuration: Configuration, runtime: &tokio::runtime::Runtime) -> Result<()> {
    runtime.block_on(async {
        let library = configuration
            .driver_library
            .clone()
            .ok_or_else(|| config("Configure an absolute trusted SANE driverLibrary."))?;
        let backend = scanner_device::sane::SaneBackend::new(library).await?;
        crate::service::run(configuration, Arc::new(backend), "SANE").await
    })
}
#[cfg(windows)]
fn native(configuration: Configuration, runtime: &tokio::runtime::Runtime) -> Result<()> {
    runtime.block_on(async {
        let library = configuration
            .driver_library
            .clone()
            .ok_or_else(|| config("Configure an absolute trusted TWAIN DSM driverLibrary."))?;
        let backend = scanner_device::twain::TwainBackend::new(library).await?;
        crate::service::run(configuration, Arc::new(backend), "TWAIN").await
    })
}
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn native(_: Configuration, _: &tokio::runtime::Runtime) -> Result<()> {
    Err(config("No native scanner adapter for this target."))
}
struct LibraryOnly;
#[async_trait::async_trait]
impl ScannerBackend for LibraryOnly {
    async fn devices(&self) -> Result<Vec<Device>> {
        Ok(vec![])
    }
    async fn inspect(&self, _: &str) -> Result<Inspection> {
        Err(unavailable())
    }
    async fn capture(
        &self,
        _: &str,
        _: &ScanRequest,
        _: PathBuf,
        _: mpsc::Sender<CaptureEvent>,
        _: CancellationToken,
    ) -> Result<CaptureOutcome> {
        Err(unavailable())
    }
}
fn unavailable() -> ScannerError {
    ScannerError::new(
        "deviceNotFound",
        "This service is configured for saved-library access only.",
    )
}
