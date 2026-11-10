use crate::backend::CardRecognizer;
use scanner_files::StorageRoot;
use scanner_recognition::{other_games::OtherGames, pokemon::PokemonRecognizer};
use scanner_types::{
    cards::CardGame, error::Result, game_detection::CardGameDetection, images::OriginalImage,
    recognition::RecognitionEvidence, scan::ScanRecord,
};
use std::{path::PathBuf, sync::Arc};
pub struct NativeCardRecognizer {
    camera: scanner_recognition::camera::CameraRecognizer,
    binder: Option<Arc<std::sync::Mutex<scanner_recognition::binder::BinderDetector>>>,
    root: Arc<StorageRoot>,
    pokemon: PokemonRecognizer,
    other_games: OtherGames,
    finish: scanner_recognition::finish::FinishAnalyzer,
}
impl NativeCardRecognizer {
    pub fn with_binder(
        mut self,
        detector: Arc<std::sync::Mutex<scanner_recognition::binder::BinderDetector>>,
    ) -> Self {
        self.binder = Some(detector);
        self
    }
    pub fn with_visual(
        mut self,
        visual: Arc<scanner_recognition::embedding::VisualRetriever>,
    ) -> Self {
        self.pokemon = self.pokemon.with_visual(visual.clone());
        self.camera = self.camera.with_visual(visual);
        self
    }
    pub fn new(root: Arc<StorageRoot>, library: PathBuf, data: PathBuf) -> Result<Self> {
        Ok(Self {
            binder: None,
            camera: scanner_recognition::camera::CameraRecognizer::new(
                library.clone(),
                data.clone(),
            )?,
            root,
            pokemon: PokemonRecognizer::new(library.clone(), data.clone())?,
            finish: scanner_recognition::finish::FinishAnalyzer::new(
                library.clone(),
                data.clone(),
            )?,
            other_games: OtherGames::new(library, data)?,
        })
    }
    pub fn with_documents(
        mut self,
        documents: Arc<scanner_catalogue::persistent::Documents>,
    ) -> Self {
        self.camera = self.camera.with_documents(documents.clone());
        self.pokemon = self.pokemon.with_documents(documents.clone());
        self.finish = self.finish.with_documents(documents.clone());
        self.other_games = self.other_games.with_documents(documents);
        self
    }
}
#[async_trait::async_trait]
impl CardRecognizer for NativeCardRecognizer {
    async fn detect_binder(
        &self,
        bytes: Vec<u8>,
    ) -> Result<scanner_types::binder::BinderDetection> {
        let detector = self.binder.clone().ok_or_else(|| {
            scanner_types::error::ScannerError::new(
                "configuration",
                "Automatic page detection is not configured. Use the pocket grid.",
            )
        })?;
        tokio::task::spawn_blocking(move || {
            detector
                .lock()
                .map_err(crate::image_access::worker)?
                .detect(&bytes)
        })
        .await
        .map_err(crate::image_access::worker)?
    }
    async fn recognize_camera(
        &self,
        bytes: Vec<u8>,
        language: scanner_types::cards::CardLanguage,
    ) -> Result<scanner_types::camera::CameraIdentification> {
        self.camera.recognize(bytes, language).await
    }
    fn method(&self, record: &ScanRecord, image: &OriginalImage) -> &'static str {
        match CardGameDetection::resolve(record)
            .into_iter()
            .find(|p| {
                p.front_sequence == Some(image.sequence) || p.back_sequence == Some(image.sequence)
            })
            .map(|p| p.game)
            .unwrap_or_default()
        {
            CardGame::Pokemon => "tesseract-tcgdex-rust-v1",
            CardGame::Yugioh => "tesseract-ygoprodeck-rust-v1",
            CardGame::StarwarsUnlimited => "tesseract-swu-db-rust-v1",
            CardGame::Unknown => "native-card-recognition-rust-v1",
        }
    }
    async fn estimate_finish(
        &self,
        record: &ScanRecord,
        image: &OriginalImage,
        evidence: &RecognitionEvidence,
        preparation: Option<&scanner_types::images::CardPreparation>,
        language: scanner_types::cards::CardLanguage,
    ) -> Result<scanner_types::finish::FinishEstimate> {
        let (root, snapshot, source) = (self.root.clone(), record.clone(), image.clone());
        let bytes = tokio::task::spawn_blocking(move || {
            root.read_image(&snapshot, &source, 32 * 1024 * 1024)
        })
        .await
        .map_err(crate::image_access::worker)??;
        self.finish
            .estimate(
                bytes,
                evidence.clone(),
                preparation.cloned(),
                language,
                false,
            )
            .await
    }
    async fn recognize(
        &self,
        record: &ScanRecord,
        image: &OriginalImage,
    ) -> Result<RecognitionEvidence> {
        let pair = CardGameDetection::resolve(record).into_iter().find(|p| {
            p.front_sequence == Some(image.sequence) || p.back_sequence == Some(image.sequence)
        });
        let game = pair.as_ref().map(|p| p.game).unwrap_or_default();
        let warning = if game == CardGame::Unknown {
            Some(
                "Identification requires a supported card pair and a readable front; unknown games are not assessed.",
            )
        } else if pair
            .as_ref()
            .is_some_and(|p| p.back_sequence == Some(image.sequence))
        {
            Some("Recognized card back; identity runs on the paired front.")
        } else {
            None
        };
        if let Some(warning) = warning {
            return Ok(RecognitionEvidence {
                warnings: vec![warning.into()],
                ..Default::default()
            });
        }
        let root = self.root.clone();
        let snapshot = record.clone();
        let source = image.clone();
        let bytes = tokio::task::spawn_blocking(move || {
            root.read_image(&snapshot, &source, 32 * 1024 * 1024)
        })
        .await
        .map_err(crate::image_access::worker)??;
        let language = record.request.card_language.unwrap_or_default();
        if game == CardGame::Pokemon {
            self.pokemon.recognize(bytes, language).await
        } else {
            self.other_games.recognize(game, bytes, language).await
        }
    }
}
