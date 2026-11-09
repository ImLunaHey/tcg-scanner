//! Read-only SANE discovery; no device is opened and no scan is requested.
#[cfg(unix)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use scanner_device::{ScannerBackend, sane::SaneBackend};
    let library = std::env::args_os()
        .nth(1)
        .ok_or("Usage: sane_devices /trusted/path/to/libsane.so.1")?;
    let backend = SaneBackend::new(library.into()).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&backend.devices().await?)?
    );
    Ok(())
}
#[cfg(not(unix))]
fn main() {
    eprintln!("The SANE adapter requires a Unix platform.");
}
