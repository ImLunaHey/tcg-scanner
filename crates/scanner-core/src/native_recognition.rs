use crate::backend::CardRecognizer;
use scanner_files::StorageRoot;
use scanner_recognition::{other_games::OtherGames, pokemon::PokemonRecognizer};
use scanner_types::{
    cards::CardGame, error::Result, game_detection::CardGameDetection, images::OriginalImage,
    recognition::RecognitionEvidence, scan::ScanRecord,
};
use std::{path::PathBuf, sync::Arc};
pub struct NativeCardRecognizer {
    root: Arc<StorageRoot>,
    pokemon: PokemonRecognizer,
    other_games: OtherGames,
}
impl NativeCardRecognizer {
    pub fn new(root: Arc<StorageRoot>, library: PathBuf, data: PathBuf) -> Result<Self> {
        Ok(Self {
            root,
            pokemon: PokemonRecognizer::new(library.clone(), data.clone())?,
            other_games: OtherGames::new(library, data)?,
        })
    }
    pub fn with_documents(
        mut self,
        documents: Arc<scanner_catalogue::persistent::Documents>,
    ) -> Self {
        self.pokemon = self.pokemon.with_documents(documents.clone());
        self.other_games = self.other_games.with_documents(documents);
        self
    }
}
#[async_trait::async_trait]
impl CardRecognizer for NativeCardRecognizer {
    fn method(&self, record: &ScanRecord, image: &OriginalImage) -> &'static str {
        match CardGameDetection::resolve(record)
            .into_iter()
            .find(|p| p.number == image.sequence.div_ceil(2))
            .map(|p| p.game)
            .unwrap_or_default()
        {
            CardGame::Pokemon => "tesseract-tcgdex-rust-v1",
            CardGame::Yugioh => "tesseract-ygoprodeck-rust-v1",
            CardGame::StarwarsUnlimited => "tesseract-swu-db-rust-v1",
            CardGame::Unknown => "native-card-recognition-rust-v1",
        }
    }
    async fn recognize(
        &self,
        record: &ScanRecord,
        image: &OriginalImage,
    ) -> Result<RecognitionEvidence> {
        let pair = CardGameDetection::resolve(record)
            .into_iter()
            .find(|p| p.number == image.sequence.div_ceil(2));
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
