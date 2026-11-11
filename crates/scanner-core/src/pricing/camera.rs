use super::Cache;
use crate::Engine;
use scanner_pricing::{provider::PublicPricing, valuation};
use scanner_types::{
    camera_review::CameraSelection,
    cards::{CardGame, CardLanguage},
    error::{Result, ScannerError},
    pricing::CardValuation,
};
use std::sync::Arc;

impl Engine {
    /// Return cached guide prices immediately; refreshes never reserve recognition.
    pub async fn camera_price(&self, selection: CameraSelection) -> Result<CardValuation> {
        selection.validate()?;
        if selection.game != CardGame::Pokemon || selection.language != CardLanguage::English {
            let mut value = CardValuation::unavailable(
                1,
                "Pricing is currently available for English Pokémon cards only.",
            );
            value.card_id = Some(selection.card.id);
            return Ok(value);
        }
        let cache = self
            .pricing
            .get_or_try_init(|| async {
                Ok::<_, ScannerError>(Cache::new(
                    self.database.clone(),
                    Arc::new(PublicPricing::new()?),
                ))
            })
            .await?;
        let snapshot = cache.snapshot([selection.card.id.clone()].into()).await?;
        let identity = selection.identification(uuid::Uuid::nil(), 1, String::new());
        Ok(valuation::value(
            1,
            &identity,
            &selection.card,
            &snapshot,
            chrono::Utc::now(),
        ))
    }
}
