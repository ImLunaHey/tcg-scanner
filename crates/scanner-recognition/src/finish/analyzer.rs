//! In-process OCR with bounded workers and public printing metadata only.
use super::estimate;
use crate::ocr::Tesseract;
use scanner_catalogue::{TcgDex, persistent::Documents};
use scanner_types::{
    cards::CardLanguage,
    error::{Result, ScannerError},
    finish::FinishEstimate,
    images::CardPreparation,
    recognition::RecognitionEvidence,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;
pub struct FinishAnalyzer {
    library: PathBuf,
    data: PathBuf,
    catalogue: TcgDex,
    workers: Arc<Semaphore>,
}
impl FinishAnalyzer {
    pub fn new(library: PathBuf, data: PathBuf) -> Result<Self> {
        Ok(Self {
            library,
            data,
            catalogue: TcgDex::new(CardLanguage::English)?,
            workers: Arc::new(Semaphore::new(1)),
        })
    }
    pub fn with_documents(mut self, documents: Arc<Documents>) -> Self {
        self.catalogue = self.catalogue.with_documents(documents);
        self
    }
    pub async fn estimate(
        &self,
        bytes: Vec<u8>,
        mut evidence: RecognitionEvidence,
        preparation: Option<CardPreparation>,
        language: CardLanguage,
        camera: bool,
    ) -> Result<FinishEstimate> {
        let permit = self.workers.clone().acquire_owned().await.map_err(worker)?;
        let mut warning = None;
        if language == CardLanguage::English && evidence.candidates.len() == 1 {
            let card = &mut evidence.candidates[0];
            if !card.extensions.contains_key("printingVariants") {
                match self.catalogue.detail(&card.id).await {
                    Ok(detail) => {
                        card.extensions.insert(
                            "printingVariants".into(),
                            serde_json::json!(detail.printing_variants()),
                        );
                    }
                    Err(_) => {
                        warning = Some(
                            "Live printing metadata was unavailable; only retained or bundled catalogue evidence could be used.",
                        )
                    }
                }
            }
        }
        let (library, data) = (self.library.clone(), self.data.clone());
        tokio::task::spawn_blocking(move || {
            let _permit=permit;
            let mut ocr=if language==CardLanguage::English {Tesseract::new(&library,&data,"en").ok()}else{None};
            let missing=language==CardLanguage::English && ocr.is_none();
            let mut result=estimate(&bytes,&evidence,preparation.as_ref(),language,ocr.as_mut(),camera);
            if missing {result.limitations.push("Portable OCR could not initialize; only retained text and image evidence were available.".into());}
            if let Some(warning)=warning {result.limitations.push(warning.into());}
            result
        }).await.map_err(worker)
    }
}
fn worker(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("finishAnalysis", error.to_string())
}
