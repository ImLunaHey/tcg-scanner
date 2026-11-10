//! Read-only native registration/cleanup check; never requests sleep or wake.
#[cfg(feature = "power")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = scanner_platform::power::Monitor::start().await?;
    monitor.shutdown().await;
    println!("Native power notifications registered and closed without changing power state.");
    Ok(())
}
#[cfg(not(feature = "power"))]
fn main() {
    eprintln!("Enable --features power to check native power notifications.");
}
