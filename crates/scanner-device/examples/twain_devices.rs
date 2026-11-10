//! Windows TWAIN discovery; opens the DSM but no source or scanner session.
#[cfg(windows)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use scanner_device::{ScannerBackend, twain::TwainBackend};
    let library = std::env::args_os()
        .nth(1)
        .ok_or("Usage: twain_devices C:\\trusted\\TWAINDSM.dll")?;
    let backend = TwainBackend::new(library.into()).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&backend.devices().await?)?
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("The TWAIN adapter requires Windows.");
}
