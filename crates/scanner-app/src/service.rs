use crate::{
    configuration::{Configuration, config},
    options::{ServiceCommand, ServiceOptions},
};
use scanner_core::Engine;
use scanner_files::StorageRoot;
use scanner_http::Gateway;
use scanner_store::Database;
use scanner_types::error::Result;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub fn execute(options: ServiceOptions) -> Result<()> {
    match options.command {
        ServiceCommand::InitToken { path } => {
            scanner_platform::create_credential(&path, &scanner_http::generate_token()?)
        }
        ServiceCommand::Serve { config: path } => crate::hosting::run(Configuration::load(&path)?),
    }
}
pub(crate) async fn run(
    configuration: Configuration,
    backend: Arc<dyn scanner_device::ScannerBackend>,
    name: &str,
) -> Result<()> {
    let http = scanner_http::Configuration::new(
        configuration.listen,
        configuration.allow_remote,
        configuration.public_origin.as_deref(),
    )?;
    let token = scanner_platform::load_credential(&configuration.token_file)?;
    let web = match configuration.web_directory.clone() {
        Some(path) => Some(
            tokio::task::spawn_blocking(move || scanner_http::web::WebAssets::load(&path))
                .await
                .map_err(config)??,
        ),
        None => None,
    };
    let root = Arc::new(StorageRoot::deferred(configuration.storage.clone())?);
    let database = Arc::new(Database::open(&configuration.database)?);
    let recovery_database = database.clone();
    let recovery_root = root.clone();
    let recovered = tokio::task::spawn_blocking(move || {
        let available = recovery_root.check_available();
        if let Err(error) = &available {
            eprintln!("Scan storage unavailable; camera and metadata remain available: {error}");
        }
        scanner_core::recover_before_start(
            &recovery_database,
            available.is_ok().then_some(recovery_root.as_ref()),
        )
    })
    .await
    .map_err(config)??;
    eprintln!("Recovered {recovered} interrupted job(s); physical scans were not resumed.");
    let staging = crate::staging::load(configuration.store_configuration.as_deref())?;
    let processing = crate::processing::configure(&configuration, root.clone()).await?;
    let listener = tokio::net::TcpListener::bind(http.address())
        .await
        .map_err(config)?;
    let engine = build_engine(backend, processing, database, root, staging).await?;
    let gateway = match Gateway::new(engine.clone(), name, &token, http) {
        Ok(gateway) => gateway,
        Err(error) => {
            engine.shutdown().await;
            return Err(error);
        }
    };
    if let Some(web) = web
        && let Err(error) = gateway.configure_web_assets(web)
    {
        engine.shutdown().await;
        return Err(error);
    }
    eprintln!("Rust scanner service listening on {}", gateway.address());
    let monitor = if matches!(configuration.backend, crate::configuration::Backend::Native) {
        match scanner_platform::power::Monitor::start().await {
            Ok(monitor) => Some(monitor),
            Err(error) => {
                engine.shutdown().await;
                return Err(error);
            }
        }
    } else {
        None
    };
    serve(engine, gateway, listener, monitor).await
}

async fn serve(
    engine: Arc<Engine>,
    gateway: Arc<Gateway>,
    listener: tokio::net::TcpListener,
    monitor: Option<scanner_platform::power::Monitor>,
) -> Result<()> {
    let stop = CancellationToken::new();
    let power = crate::power::run(engine.clone(), monitor, stop.clone());
    tokio::pin!(power);
    let server = gateway.serve(listener, stop.clone());
    tokio::pin!(server);
    let result = tokio::select! {
        result=&mut server=>{
            stop.cancel();
            engine.shutdown().await;
            let monitored = power.await;
            result.and(monitored)
        },
        monitored=&mut power=>{
            stop.cancel();
            engine.shutdown().await;
            let served = server.await;
            monitored.and(served)
        },
        signal=crate::signals::shutdown()=>{
            stop.cancel();
            engine.shutdown().await;
            let served=server.await;
            let monitored = power.await;
            signal.and(served).and(monitored)
        },
    };
    stop.cancel();
    engine.shutdown().await;
    result
}

async fn build_engine(
    backend: Arc<dyn scanner_device::ScannerBackend>,
    processing: crate::processing::Processing,
    database: Arc<Database>,
    root: Arc<StorageRoot>,
    staging: Option<Arc<dyn scanner_staging::Staging>>,
) -> Result<Arc<Engine>> {
    let engine = Engine::with_recognizer(
        backend,
        processing.processor,
        processing.recognizer,
        database,
        root,
    );
    if let Err(error) = engine.configure_library_catalogue(processing.catalogue) {
        engine.shutdown().await;
        return Err(error);
    }
    if let Err(error) = engine.configure_pricing(processing.pricing) {
        engine.shutdown().await;
        return Err(error);
    }
    if let Some(staging) = staging
        && let Err(error) = engine.configure_staging(staging)
    {
        engine.shutdown().await;
        return Err(error);
    }
    Ok(engine)
}
