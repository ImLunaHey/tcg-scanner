//! Development-only distribution assembly from already-built native and web files.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "Usage: package NATIVE_BIN_DIRECTORY VERIFIED_WEB_DIRECTORY NEW_OUTPUT_DIRECTORY"
                .into(),
        );
    }
    scanner_app::distribution::build::assemble(
        args[0].as_ref(),
        args[1].as_ref(),
        args[2].as_ref(),
    )?;
    Ok(())
}
