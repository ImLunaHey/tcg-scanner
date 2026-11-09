//! Read-only discovery. Opens no scanner session and requests no scan.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use scanner_device::{ScannerBackend, macos::MacHost};
    let runtime = tokio::runtime::Runtime::new()?;
    let (mut host, backend) = MacHost::new(runtime.handle().clone())?;
    let task = runtime.spawn(async move { backend.devices().await });
    while !task.is_finished() {
        host.tick();
    }
    let devices = runtime.block_on(task)??;
    println!("{}", serde_json::to_string_pretty(&devices)?);
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This example currently exercises the macOS adapter.");
}
