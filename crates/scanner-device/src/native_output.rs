use image::{DynamicImage, ImageFormat};
use scanner_types::{
    error::{Result, ScannerError},
    scan::OriginalFormat,
};
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
    path::Path,
};

pub(crate) fn write_image(
    image: DynamicImage,
    format: OriginalFormat,
    incoming: &Path,
    sequence: u32,
) -> Result<String> {
    let error = |error: std::io::Error| ScannerError::new("storage", error.to_string());
    validate_incoming(incoming)?;
    let mut encoded = Cursor::new(Vec::new());
    image
        .write_to(
            &mut encoded,
            match format {
                OriginalFormat::Png => ImageFormat::Png,
                OriginalFormat::Tiff => ImageFormat::Tiff,
            },
        )
        .map_err(|error| ScannerError::new("image", error.to_string()))?;
    let filename = format!("capture-{sequence:04}.{}", format.extension());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(incoming.join(&filename)).map_err(error)?;
    file.write_all(encoded.get_ref()).map_err(error)?;
    file.sync_all().map_err(error)?;
    if encoded.get_ref().len() > 32 * 1024 * 1024 {
        return Err(ScannerError::new(
            "resourceLimit",
            "Captured image exceeds the original-file import limit; retained in incoming storage.",
        ));
    }
    Ok(filename)
}

pub(crate) fn validate_incoming(incoming: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(incoming)
        .map_err(|error| ScannerError::new("storage", error.to_string()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ScannerError::new("storage", "Invalid incoming directory."));
    }
    Ok(())
}
