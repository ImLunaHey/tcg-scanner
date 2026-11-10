use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "scanner-setup",
    version,
    about = "Install and inspect the portable Scanner application"
)]
pub struct Options {
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    /// Assemble a distribution from already-built native binaries and a verified web bundle.
    Package {
        #[arg(long)]
        binaries: PathBuf,
        #[arg(long)]
        web: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Copy a verified distribution into a NEW private directory and create configuration.
    Install {
        #[arg(long)]
        package: PathBuf,
        #[arg(long)]
        destination: PathBuf,
        #[arg(long)]
        storage: PathBuf,
        #[arg(long)]
        required_smb_share: Option<String>,
        #[arg(long)]
        library_only: bool,
        #[arg(long)]
        driver_library: Option<PathBuf>,
        #[arg(long)]
        tesseract_library: Option<PathBuf>,
        #[arg(long)]
        trained_data: Option<PathBuf>,
    },
    /// Check configured files and optional OCR data without opening a scanner.
    Doctor {
        #[arg(long)]
        config: PathBuf,
    },
    /// Register this installation for the next login; never starts a second owner now.
    EnableStartup {
        #[arg(long)]
        config: PathBuf,
    },
    /// Remove only this installation's matching startup registration; retain all data.
    DisableStartup {
        #[arg(long)]
        config: PathBuf,
    },
}
