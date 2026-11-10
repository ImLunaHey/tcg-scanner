//! Explicit, per-user installation; never invokes an OS utility or scanner helper.
mod dependencies;
mod files;
mod install;
pub mod options;
mod startup;
use crate::configuration::{Configuration, config};
use options::{Command, Options};
use scanner_types::error::Result;
use std::path::Path;

pub fn execute(options: Options) -> Result<()> {
    match options.command {
        Command::Package {
            binaries,
            web,
            output,
        } => crate::distribution::build::assemble(&binaries, &web, &output),
        Command::Install {
            package,
            destination,
            storage,
            required_smb_share,
            library_only,
            driver_library,
            tesseract_library,
            trained_data,
        } => install::run(install::Request {
            package,
            destination,
            storage,
            required_smb_share,
            library_only,
            driver_library,
            tesseract_library,
            trained_data,
        }),
        Command::Doctor { config: path } => doctor(&path),
        Command::EnableStartup { config } => startup::set(&config, true),
        Command::DisableStartup { config } => startup::set(&config, false),
    }
}
fn doctor(path: &Path) -> Result<()> {
    let c = Configuration::load(path)?;
    scanner_platform::load_credential(&c.token_file)?;
    if let Some(web) = &c.web_directory {
        scanner_http::web::WebAssets::load(web)?;
    }
    let data = c.trained_data.as_deref();
    if let (Some(library), Some(data)) = (&c.tesseract_library, data) {
        scanner_recognition::ocr::Tesseract::new(library, data, "en")?;
        if data.join("kor.traineddata").is_file() {
            scanner_recognition::ocr::Tesseract::new(library, data, "ko")?;
        }
    }
    let report = serde_json::json!({
        "configuration": "valid", "credential": "private", "webBundle": c.web_directory.is_some(),
        "storage": scanner_files::StorageRoot::open_configured(c.storage.clone()).map(|_| "available").map_err(|e| e.message),
        "tesseractConfigured":c.tesseract_library.is_some(),
        "englishData": data.is_some_and(|p| p.join("eng.traineddata").is_file()),
        "koreanData": data.is_some_and(|p| p.join("kor.traineddata").is_file()),
        "visualIndexConfigured":c.visual_recognition.is_some(),
        "hardwareOpened":false,
    });
    println!("{}", serde_json::to_string_pretty(&report).map_err(config)?);
    Ok(())
}
