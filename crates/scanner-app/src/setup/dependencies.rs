use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Dependencies {
    pub driver_library: Option<PathBuf>,
    pub tesseract_library: Option<PathBuf>,
    pub trained_data: Option<PathBuf>,
}
impl Dependencies {
    pub fn discover(package: &Path) -> Self {
        let mut directories = vec![package.join("lib"), package.to_path_buf()];
        #[cfg(target_os = "macos")]
        directories.extend(["/opt/homebrew/lib", "/usr/local/lib"].map(PathBuf::from));
        #[cfg(target_os = "linux")]
        directories.extend(
            [
                "/usr/lib/x86_64-linux-gnu",
                "/usr/lib/aarch64-linux-gnu",
                "/usr/lib64",
                "/usr/lib",
                "/usr/local/lib",
            ]
            .map(PathBuf::from),
        );
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SystemRoot") {
            directories.push(PathBuf::from(root).join("System32"));
        }
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("ProgramFiles") {
            directories.push(PathBuf::from(root).join("Tesseract-OCR"));
        }
        let file = |names: &[&str]| {
            directories
                .iter()
                .flat_map(|d| names.iter().map(move |n| d.join(n)))
                .find(|p| p.is_file())
                .and_then(|p| p.canonicalize().ok())
        };
        let mut data = vec![package.join("tessdata"), package.join("share/tessdata")];
        #[cfg(target_os = "macos")]
        data.extend(
            ["/opt/homebrew/share/tessdata", "/usr/local/share/tessdata"].map(PathBuf::from),
        );
        #[cfg(target_os = "linux")]
        data.extend(
            [
                "/usr/share/tesseract-ocr/5/tessdata",
                "/usr/share/tesseract-ocr/4.00/tessdata",
                "/usr/share/tessdata",
                "/usr/local/share/tessdata",
            ]
            .map(PathBuf::from),
        );
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("ProgramFiles") {
            data.push(PathBuf::from(root).join("Tesseract-OCR/tessdata"));
        }
        Self {
            driver_library: file(if cfg!(windows) {
                &["TWAINDSM.dll"]
            } else if cfg!(target_os = "linux") {
                &["libsane.so.1"]
            } else {
                &[]
            }),
            tesseract_library: file(if cfg!(windows) {
                &["libtesseract-5.dll", "tesseract55.dll", "libtesseract.dll"]
            } else if cfg!(target_os = "macos") {
                &["libtesseract.5.dylib", "libtesseract.dylib"]
            } else {
                &["libtesseract.so.5"]
            }),
            trained_data: data
                .into_iter()
                .find(|p| p.join("eng.traineddata").is_file())
                .and_then(|p| p.canonicalize().ok()),
        }
    }
}
