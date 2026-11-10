use crate::backend::CardRecognizer;
use scanner_files::StorageRoot;
use scanner_recognition::pokemon::PokemonRecognizer;
use scanner_types::{
    cards::CardGame, error::Result, game_detection::CardGameDetection, images::OriginalImage,
    recognition::RecognitionEvidence, scan::ScanRecord,
};
use std::{path::PathBuf, sync::Arc};
pub struct NativeCardRecognizer {
    root: Arc<StorageRoot>,
    pokemon: PokemonRecognizer,
}
impl NativeCardRecognizer {
    pub fn new(root: Arc<StorageRoot>, library: PathBuf, data: PathBuf) -> Result<Self> {
        Ok(Self {
            root,
            pokemon: PokemonRecognizer::new(library, data)?,
        })
    }
    pub fn with_documents(
        mut self,
        documents: Arc<scanner_catalogue::persistent::Documents>,
    ) -> Self {
        self.pokemon = self.pokemon.with_documents(documents);
        self
    }
}
#[async_trait::async_trait]
impl CardRecognizer for NativeCardRecognizer {
    async fn recognize(
        &self,
        record: &ScanRecord,
        image: &OriginalImage,
    ) -> Result<RecognitionEvidence> {
        let pair = CardGameDetection::resolve(record)
            .into_iter()
            .find(|p| p.number == image.sequence.div_ceil(2));
        let warning = match pair {
            Some(pair)
                if pair.game == CardGame::Pokemon
                    && pair.front_sequence == Some(image.sequence) =>
            {
                None
            }
            Some(pair) if pair.game == CardGame::Pokemon => {
                Some("Recognized Pokémon back; identity runs on the paired front.")
            }
            Some(pair) if pair.game != CardGame::Unknown => Some(
                "This game's native catalogue recognizer is not configured yet; review the saved image manually.",
            ),
            _ => Some(
                "Identification requires a supported card pair and a readable front; unknown games are not assessed.",
            ),
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
        self.pokemon
            .recognize(bytes, record.request.card_language.unwrap_or_default())
            .await
    }
}
