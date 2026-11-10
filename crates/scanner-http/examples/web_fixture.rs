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
    if !(3..=4).contains(&arguments.len()) {
        return Err(
            "Usage: web_fixture WEB_DIRECTORY TOKEN_FILE TAILSCALE_IP:PORT [HTTPS_ORIGIN]".into(),
        );
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
    let engine = scanner_core::Engine::with_recognizer(
        Arc::new(backend::LibraryOnly),
        Arc::new(backend::LibraryOnly),
        Some(Arc::new(backend::LibraryOnly)),
        database,
        root,
    );
    let address: std::net::SocketAddr = arguments[2].parse()?;
    let proxy_address = (std::net::Ipv4Addr::LOCALHOST, address.port()).into();
    let origin = arguments.get(3).map(String::as_str);
    let gateway = preview_gateway(
        engine.clone(),
        &token,
        Path::new(&arguments[0]),
        address,
        origin,
    )?;
    let proxy = preview_gateway(
        engine.clone(),
        &token,
        Path::new(&arguments[0]),
        proxy_address,
        origin,
    )?;
    let listener = tokio::net::TcpListener::bind(gateway.address()).await?;
    eprintln!(
        "Synthetic browser fixture listening at http://{}",
        gateway.address()
    );
    // Local reverse proxy for HTTPS preview; the primary listener stays on Tailscale.
    let loopback =
        tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, gateway.address().port()))
            .await?;
    let stop = CancellationToken::new();
    let server = async {
        tokio::try_join!(
            gateway.clone().serve(listener, stop.clone()),
            proxy.serve(loopback, stop.clone())
        )
    };
    tokio::pin!(server);
    tokio::select! { result = &mut server => { result?; }, _ = tokio::signal::ctrl_c() => { stop.cancel(); server.await?; } }
    engine.shutdown().await;
    Ok(())
}

fn preview_gateway(
    engine: Arc<scanner_core::Engine>,
    token: &str,
    web: &Path,
    address: std::net::SocketAddr,
    origin: Option<&str>,
) -> Result<Arc<Gateway>, Box<dyn Error>> {
    let configuration = Configuration::new(address, true, origin)?;
    let gateway = Gateway::new(engine, "Browser fixture", token, configuration)?;
    gateway.configure_web_assets(WebAssets::load(web)?)?;
    Ok(gateway)
}
