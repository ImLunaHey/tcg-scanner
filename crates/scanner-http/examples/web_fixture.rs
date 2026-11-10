//! Isolated development preview: synthetic card, memory database, no hardware access.
#[path = "web_fixture/backend.rs"]
mod backend;
#[path = "web_fixture/seed.rs"]
mod seed;
use scanner_http::{Configuration, Gateway, web::WebAssets};
use std::{error::Error, path::Path, sync::Arc};
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 3 {
        return Err("Usage: web_fixture WEB_DIRECTORY TOKEN_FILE TAILSCALE_IP:PORT".into());
    }
    let token_path = Path::new(&arguments[1]);
    let token = if token_path.exists() {
        scanner_platform::load_credential(token_path)?
    } else {
        let token = scanner_http::generate_token()?;
        scanner_platform::create_credential(token_path, &token)?;
        token
    };
    let directory = tempfile::tempdir()?;
    let root = Arc::new(scanner_files::StorageRoot::open(directory.path())?);
    let database = Arc::new(scanner_store::Database::memory()?);
    seed::seed(&root, &database)?;
    let engine = scanner_core::Engine::new(
        Arc::new(backend::LibraryOnly),
        Arc::new(backend::LibraryOnly),
        database,
        root,
    );
    let configuration = Configuration::new(arguments[2].parse()?, true, None)?;
    let gateway = Gateway::new(engine.clone(), "Browser fixture", &token, configuration)?;
    gateway.configure_web_assets(WebAssets::load(Path::new(&arguments[0]))?)?;
    let listener = tokio::net::TcpListener::bind(gateway.address()).await?;
    eprintln!(
        "Synthetic browser fixture listening at http://{}",
        gateway.address()
    );
    let stop = CancellationToken::new();
    let server = gateway.serve(listener, stop.clone());
    tokio::pin!(server);
    tokio::select! { result = &mut server => { result?; }, _ = tokio::signal::ctrl_c() => { stop.cancel(); server.await?; } }
    engine.shutdown().await;
    Ok(())
}
