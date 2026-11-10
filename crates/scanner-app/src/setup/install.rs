use super::{dependencies::Dependencies, files};
use crate::{
    configuration::{Configuration, config},
    distribution::Distribution,
};
use scanner_types::error::Result;
use std::path::PathBuf;

pub(super) struct Request {
    pub package: PathBuf,
    pub destination: PathBuf,
    pub storage: PathBuf,
    pub required_smb_share: Option<String>,
    pub library_only: bool,
    pub driver_library: Option<PathBuf>,
    pub tesseract_library: Option<PathBuf>,
    pub trained_data: Option<PathBuf>,
}
pub(super) fn run(r: Request) -> Result<()> {
    if !r.destination.is_absolute() || r.destination.exists() || !r.storage.is_absolute() {
        return Err(config(
            "Use a NEW absolute installation directory and an absolute scan storage path.",
        ));
    }
    scanner_files::configuration::StorageConfiguration {
        storage_root: r.storage.to_string_lossy().into(),
        required_smb_share: r.required_smb_share.clone(),
    }
    .validate_shape()?;
    let package = r.package.canonicalize().map_err(config)?;
    let distribution = Distribution::load(&package)?;
    let mut dependencies = Dependencies::discover(&package);
    dependencies.driver_library = r.driver_library.or(dependencies.driver_library);
    dependencies.tesseract_library = r.tesseract_library.or(dependencies.tesseract_library);
    dependencies.trained_data = r.trained_data.or(dependencies.trained_data);
    validate_dependencies(&dependencies, r.library_only)?;
    // Check every byte before creating private state. A second check during copy
    // also detects a package modified between validation and installation.
    for name in distribution.manifest.files.keys() {
        distribution.read(name)?;
    }
    scanner_platform::PrivateDirectory::open(&r.destination, true)?;
    for (name, entry) in &distribution.manifest.files {
        files::write(
            &r.destination.join(name),
            &distribution.read(name)?,
            entry.executable,
        )?;
    }
    let relocate = |path: Option<PathBuf>| {
        path.map(|p| {
            p.strip_prefix(&package)
                .map(|s| r.destination.join(s))
                .unwrap_or(p.clone())
        })
    };
    let token = r.destination.join("private/token");
    scanner_platform::create_credential(&token, &scanner_http::generate_token()?)?;
    let configuration = serde_json::json!({
        "database":r.destination.join("state/scanner.sqlite"), "tokenFile":token,
        "webDirectory":r.destination.join("web"), "listen":"127.0.0.1:8171",
        "backend":if r.library_only { "libraryOnly" } else { "native" },
        "storage":{"storageRoot":r.storage,"requiredSMBShare":r.required_smb_share},
        "driverLibrary":relocate(dependencies.driver_library),
        "tesseractLibrary":relocate(dependencies.tesseract_library),
        "trainedData":relocate(dependencies.trained_data),
    });
    let path = r.destination.join("service.json");
    files::write(
        &path,
        &serde_json::to_vec_pretty(&configuration).map_err(config)?,
        false,
    )?;
    Configuration::load(&path)?;
    println!(
        "Installed Scanner at {}\nConfiguration: {}\nNo service started; startup registration is a separate explicit command.",
        r.destination.display(),
        path.display()
    );
    Ok(())
}
fn validate_dependencies(d: &Dependencies, library_only: bool) -> Result<()> {
    if !library_only && !cfg!(target_os = "macos") && d.driver_library.is_none() {
        return Err(config(
            "Provide --driver-library pointing to SANE or the TWAIN DSM.",
        ));
    }
    if !library_only && (d.tesseract_library.is_none() || d.trained_data.is_none()) {
        return Err(config(
            "Provide --tesseract-library and --trained-data for portable OCR.",
        ));
    }
    if d.tesseract_library.is_some() != d.trained_data.is_some() {
        return Err(config(
            "Tesseract library and trained data must be configured together.",
        ));
    }
    for path in [&d.driver_library, &d.tesseract_library, &d.trained_data]
        .into_iter()
        .flatten()
    {
        if !path.is_absolute() || !path.exists() {
            return Err(config("Dependency paths must be absolute and exist."));
        }
    }
    if d.trained_data
        .as_ref()
        .is_some_and(|p| !p.join("eng.traineddata").is_file())
    {
        return Err(config(
            "English trained data is required. Add kor.traineddata for Korean cards.",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "install_test.rs"]
mod tests;
