use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "scanner-service",
    version,
    about = "Native Rust scanner service"
)]
pub struct ServiceOptions {
    #[command(subcommand)]
    pub command: ServiceCommand,
}
#[derive(Subcommand)]
pub enum ServiceCommand {
    /// Create an exclusive private token file; never prints its contents.
    InitToken {
        #[arg(long)]
        path: PathBuf,
    },
    /// Run using an explicit JSON configuration.
    Serve {
        #[arg(long)]
        config: PathBuf,
    },
}
#[derive(Parser)]
#[command(name = "scanner", version, about = "Rust scanner service client")]
pub struct ClientOptions {
    #[arg(long, default_value = "http://127.0.0.1:8171")]
    pub endpoint: String,
    #[arg(long)]
    pub token_file: PathBuf,
    #[command(subcommand)]
    pub command: ClientCommand,
}
#[derive(Subcommand)]
pub enum ClientCommand {
    /// Call a version-one method with explicit JSON object parameters.
    Call {
        method: String,
        #[arg(default_value = "{}")]
        params: String,
    },
    Status,
    Devices,
    Jobs {
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Job {
        id: uuid::Uuid,
    },
    Cancel {
        id: uuid::Uuid,
    },
    Profiles,
    /// Submit a physical scan; reuse request-id and settings for retries.
    Scan {
        #[arg(long)]
        request_id: uuid::Uuid,
        #[arg(long)]
        device_id: Option<String>,
        #[arg(long, default_value_t = 600)]
        dpi: u32,
        #[arg(long)]
        duplex: bool,
        #[arg(long,default_value="png",value_parser=["png","tiff"])]
        format: String,
        #[arg(long)]
        prepare_cards: bool,
        #[arg(long,default_value="en",value_parser=["en","ko"])]
        card_language: String,
    },
}
