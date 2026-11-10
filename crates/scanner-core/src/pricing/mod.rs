mod cache;
use crate::Engine;
pub(crate) use cache::Cache;
use scanner_pricing::{
    provider::{PricingProvider, PublicPricing},
    selection,
    valuation::{self, Snapshot},
};
use scanner_types::{
    cards::CardLanguage,
    error::{Result, ScannerError},
    pricing::{BatchValuation, CardValuation},
};
use std::sync::Arc;
use uuid::Uuid;
impl Engine {
    pub fn configure_pricing(&self, provider: Arc<dyn PricingProvider>) -> Result<()> {
        self.pricing
            .set(Cache::new(self.database.clone(), provider))
            .map_err(|_| ScannerError::new("configuration", "Pricing is already configured."))
    }
    pub async fn batch_valuation(self: &Arc<Self>, id: Uuid) -> Result<BatchValuation> {
        let _reservation = self.submissions.lock().await;
        if self.state.lock().await.stopping {
            return Err(ScannerError::new("busy", "Service is stopping."));
        }
        let engine = self.clone();
        let record = tokio::task::spawn_blocking(move || engine.scan_status(id))
            .await
            .map_err(crate::image_access::worker)??;
        let selected = selection::select(&record)?;
        let english = record.request.card_language.unwrap_or_default() == CardLanguage::English;
        let snapshot = if english {
            let cache = self
                .pricing
                .get_or_try_init(|| async {
                    Ok::<_, ScannerError>(Cache::new(
                        self.database.clone(),
                        Arc::new(PublicPricing::new()?),
                    ))
                })
                .await?;
            let ids = selected
                .iter()
                .filter_map(|s| s.matched.as_ref())
                .filter(|(identity, _)| identity.request.language == CardLanguage::English)
                .map(|(_, card)| card.id.clone())
                .collect();
            cache.snapshot(ids).await?
        } else {
            Snapshot::default()
        };
        let now = chrono::Utc::now();
        let cards=selected.into_iter().map(|s| {
            if !english {
                let mut result=CardValuation::unavailable(s.number,"Pricing is not yet supported for this language; English prices are not substituted.");
                result.card_id=s.matched.map(|(_,c)|c.id);result
            } else if let Some((identity,card))=s.matched {
                valuation::value(s.number,&identity,&card,&snapshot,now)
            } else {CardValuation::unavailable(s.number,"A single card match and complete pair are needed.")}
        }).collect();
        Ok(BatchValuation::new(
            id,
            cards,
            snapshot.rate,
            !snapshot.pending.is_empty() || snapshot.rate_pending,
        ))
    }
}
