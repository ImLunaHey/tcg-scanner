//! Development-only detector timing and normalized boxes for saved JPEGs.
use scanner_recognition::binder::BinderDetector;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        return Err("Usage: binder_probe RUNTIME MODEL SHA256 JPEG...".into());
    }
    let detector = BinderDetector::load(
        std::path::Path::new(&args[0]),
        std::path::Path::new(&args[1]),
        &args[2],
    )?;
    for path in &args[3..] {
        let bytes = std::fs::read(path)?;
        for round in 0..3 {
            let start = std::time::Instant::now();
            let result = detector.lock().unwrap().detect(&bytes)?;
            println!(
                "{}",
                serde_json::json!({"path":path,"round":round,"milliseconds":start.elapsed().as_secs_f64()*1000.,"result":result})
            );
        }
    }
    Ok(())
}
