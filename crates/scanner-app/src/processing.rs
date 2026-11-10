use crate::configuration::{Configuration, config};
use scanner_catalogue::{library::LibraryCatalogue, persistent::Documents};
use scanner_core::{
    NativeCardProcessor, NativeCardRecognizer,
    backend::{CardProcessor, CardRecognizer},
};
use scanner_files::StorageRoot;
use scanner_types::{
    error::{Result, ScannerError},
    images::ProcessedImage,
    scan::ScanRecord,
};
use std::sync::Arc;
pub(crate) struct Processing {
    pub processor: Arc<dyn CardProcessor>,
    pub recognizer: Option<Arc<dyn CardRecognizer>>,
    pub catalogue: Arc<LibraryCatalogue>,
    pub pricing: Arc<dyn scanner_pricing::provider::PricingProvider>,
}
pub(crate) async fn configure(
    configuration: &Configuration,
    root: Arc<StorageRoot>,
) -> Result<Processing> {
    let path = configuration.catalogue_cache.clone().unwrap_or_else(|| {
        configuration
            .database
            .parent()
            .unwrap()
            .join("catalogue/public.sqlite")
    });
    let documents = match tokio::task::spawn_blocking(move || Documents::open(&path))
        .await
        .map_err(config)?
    {
        Ok(cache) => Some(cache),
        Err(error) => {
            eprintln!("Optional public catalogue cache unavailable: {error}");
            None
        }
    };
    let catalogue = match &documents {
        Some(cache) => LibraryCatalogue::cached(cache.clone())?,
        None => LibraryCatalogue::new()?,
    };
    let mut pricing = scanner_pricing::provider::PublicPricing::new()?;
    if let Some(cache) = &documents {
        pricing = pricing.with_documents(cache.clone());
    }
    let (processor, recognizer): (Arc<dyn CardProcessor>, Option<Arc<dyn CardRecognizer>>) = match (
        &configuration.tesseract_library,
        &configuration.trained_data,
    ) {
        (Some(library), Some(data)) => {
            let processor = Arc::new(NativeCardProcessor::new(
                root.clone(),
                library.clone(),
                data.clone(),
            ));
            let mut recognizer = NativeCardRecognizer::new(root, library.clone(), data.clone())?;
            if let Some(cache) = documents {
                recognizer = recognizer.with_documents(cache);
            }
            (processor, Some(Arc::new(recognizer)))
        }
        _ => (Arc::new(UnconfiguredProcessor), None),
    };
    Ok(Processing {
        processor,
        recognizer,
        catalogue,
        pricing: Arc::new(pricing),
    })
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
