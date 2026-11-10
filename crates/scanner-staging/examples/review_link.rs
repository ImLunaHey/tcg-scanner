//! Validate a Store-provided review link without reading credentials or networking.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: review_link HTTPS_ORIGIN REVIEW_PATH".into());
    }
    println!("{}", scanner_staging::review_url(&args[0], &args[1])?);
    Ok(())
}
