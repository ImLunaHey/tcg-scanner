//! Decimal public market quotes, separate from immutable scan/recognition records.
use crate::recognition::CardFinish;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketVariantPrice {
    #[serde(default, rename = "variantID", skip_serializing_if = "Option::is_none")]
    pub variant_id: Option<String>,
    #[serde(default)]
    pub stamps: Vec<String>,
    pub finish: CardFinish,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, rename = "productID", skip_serializing_if = "Option::is_none")]
    pub product_id: Option<u64>,
    #[serde(
        default,
        with = "rust_decimal::serde::arbitrary_precision_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub euros: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardMarketPrices {
    #[serde(rename = "cardID")]
    pub card_id: String,
    pub fetched_at: DateTime<Utc>,
    pub variants: Vec<MarketVariantPrice>,
    pub requires_printing_review: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuroSterlingRate {
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub pounds_per_euro: Decimal,
    pub date: String,
    pub fetched_at: DateTime<Utc>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValuationState {
    Priced,
    Range,
    Pending,
    #[default]
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardValuation {
    pub number: u32,
    #[serde(default, rename = "cardID", skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition_key: Option<String>,
    pub state: ValuationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_pence: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_pence: Option<i64>,
    pub stale: bool,
    pub reason: String,
    pub quotes: Vec<MarketVariantPrice>,
}
impl CardValuation {
    pub fn unavailable(number: u32, reason: impl Into<String>) -> Self {
        Self {
            number,
            reason: reason.into(),
            state: ValuationState::Unavailable,
            card_id: None,
            edition_key: None,
            low_pence: None,
            high_pence: None,
            stale: false,
            quotes: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchValuation {
    pub schema_version: u32,
    #[serde(rename = "batchID")]
    pub batch_id: Uuid,
    pub currency: String,
    pub source: String,
    pub metric: String,
    pub exchange_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange_rate: Option<EuroSterlingRate>,
    pub cards: Vec<CardValuation>,
    pub priced_count: usize,
    pub total_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_pence: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_pence: Option<i64>,
    pub refreshing: bool,
    pub stale: bool,
    pub warning: String,
}
impl BatchValuation {
    pub fn new(
        batch_id: Uuid,
        cards: Vec<CardValuation>,
        exchange_rate: Option<EuroSterlingRate>,
        refreshing: bool,
    ) -> Self {
        let priced: Vec<_> = cards
            .iter()
            .filter_map(|c| c.low_pence.zip(c.high_pence))
            .collect();
        let sum = |high: bool| {
            (!priced.is_empty())
                .then(|| {
                    priced.iter().try_fold(0i64, |total, (lo, hi)| {
                        total.checked_add(if high { *hi } else { *lo })
                    })
                })
                .flatten()
        };
        Self {schema_version:1,batch_id,currency:"GBP".into(),source:"Cardmarket via TCGdex".into(),metric:"trend".into(),
            exchange_source:"European Central Bank".into(),exchange_rate,priced_count:priced.len(),total_count:cards.len(),
            low_pence:sum(false),high_pence:sum(true),refreshing,stale:cards.iter().any(|c|c.stale),cards,
            warning:"Market guide for the matched catalogue printing; not condition-adjusted. Excludes fees and postage.".into()}
    }
}
