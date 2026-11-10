use crate::{
    configuration::{Configuration, config},
    options::{ServiceCommand, ServiceOptions},
};
use scanner_core::{Engine, NativeCardProcessor, backend::CardProcessor};
use scanner_files::StorageRoot;
use scanner_http::Gateway;
use scanner_store::Database;
use scanner_types::{
    error::{Result, ScannerError},
    images::ProcessedImage,
    scan::ScanRecord,
};
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
    let root = Arc::new(StorageRoot::open_configured(configuration.storage)?);
    let database = Arc::new(Database::open(&configuration.database)?);
    if !database.unfinished()?.is_empty() {
        return Err(ScannerError::new(
            "recoveryRequired",
            "Interrupted jobs require manifest reconciliation before this service can start; no feeding or recovery writes were performed.",
        ));
    }
    let processor: Arc<dyn CardProcessor> =
        match (configuration.tesseract_library, configuration.trained_data) {
            (Some(library), Some(data)) => {
                Arc::new(NativeCardProcessor::new(root.clone(), library, data))
            }
            _ => Arc::new(UnconfiguredProcessor),
        };
    let listener = tokio::net::TcpListener::bind(http.address())
        .await
        .map_err(config)?;
    let engine = Engine::new(backend, processor, database, root);
    let gateway = match Gateway::new(engine.clone(), name, &token, http) {
        Ok(gateway) => gateway,
        Err(error) => {
            engine.shutdown().await;
            return Err(error);
        }
    };
    eprintln!("Rust scanner service listening on {}", gateway.address());
    let stop = CancellationToken::new();
    let server = gateway.serve(listener, stop.clone());
    tokio::pin!(server);
    let result = tokio::select! {
        result=&mut server=>result,
        signal=crate::signals::shutdown()=>{
            stop.cancel();
            engine.shutdown().await;
            let served=server.await;
            signal.and(served)
        },
    };
    stop.cancel();
    engine.shutdown().await;
    result
}
struct UnconfiguredProcessor;
#[async_trait::async_trait]
impl CardProcessor for UnconfiguredProcessor {
    async fn prepare(&self, _: &ScanRecord, _: u32) -> Result<ProcessedImage> {
        Err(ScannerError::new(
            "configuration",
            "Configure tesseractLibrary and trainedData for native automatic card preparation.",
        ))
    }
}
