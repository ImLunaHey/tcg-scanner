//! Offline artifact construction, separate from scanner and review mutations.
use crate::configuration::config;
use clap::Parser;
use scanner_types::{cards::CardLanguage, error::Result};
use std::{io::Write, path::PathBuf};
#[derive(Parser)]
#[command(about = "Build a pinned DINOv3 index from checksum-verified local reference images")]
pub struct Options {
    #[arg(long)]
    pub references: PathBuf,
    #[arg(long)]
    pub runtime_library: PathBuf,
    #[arg(long)]
    pub model: PathBuf,
    #[arg(long,value_parser=["en","ko"])]
    pub language: String,
    #[arg(long)]
    pub output: PathBuf,
}
pub fn execute(options: Options) -> Result<()> {
    if options.output.exists() {
        return Err(config(
            "Index output already exists; choose a new filename.",
        ));
    }
    let language = if options.language == "ko" {
        CardLanguage::Korean
    } else {
        CardLanguage::English
    };
    let mut encoder = scanner_recognition::embedding::DinoEncoder::load(
        &options.runtime_library,
        &options.model,
    )?;
    let document = scanner_recognition::embedding::build_index(
        &mut encoder,
        language,
        &options.references,
        |done, total| {
            if done % 25 == 0 || done == total {
                eprintln!("Encoded {done}/{total} reference images");
            }
        },
    )?;
    let bytes = serde_json::to_vec(&document).map_err(config)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err(config("Encoded index exceeds 256 MiB."));
    }
    let parent = options
        .output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(config)?;
    temporary.write_all(&bytes).map_err(config)?;
    temporary.as_file().sync_all().map_err(config)?;
    temporary
        .persist_noclobber(&options.output)
        .map_err(config)?;
    eprintln!(
        "Saved {} references to {}",
        document.references.len(),
        options.output.display()
    );
    Ok(())
}
