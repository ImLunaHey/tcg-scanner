use scanner_library::Inventory;
use scanner_types::scan::ScanRecord;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut inventory = Inventory::default();
    for path in std::env::args_os().skip(1) {
        let record: ScanRecord = serde_json::from_slice(&std::fs::read(path)?)?;
        inventory.add(&record)?;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&inventory.page(100, 0, "", "")?)?
    );
    Ok(())
}
