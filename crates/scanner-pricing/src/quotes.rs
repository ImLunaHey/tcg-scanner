//! Regular and reverse market series are never substituted for each other.
use rust_decimal::Decimal;
use scanner_types::{
    pricing::{CardMarketPrices, MarketVariantPrice},
    recognition::CardFinish,
};
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Clone, Deserialize)]
struct Prices {
    cardmarket: Option<Market>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Market {
    unit: Option<String>,
    id_product: Option<u64>,
    updated: Option<String>,
    #[serde(default, with = "rust_decimal::serde::arbitrary_precision_option")]
    trend: Option<Decimal>,
    #[serde(
        default,
        rename = "trend-holo",
        with = "rust_decimal::serde::arbitrary_precision_option"
    )]
    reverse_trend: Option<Decimal>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Variant {
    variant_id: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    size: Option<String>,
    foil: Option<String>,
    stamp: Option<Vec<String>>,
    subtype: Option<String>,
    pricing: Option<Prices>,
}
#[derive(Deserialize)]
pub struct PricingDetail {
    pub id: String,
    variants: Option<BTreeMap<String, bool>>,
    variants_detailed: Option<Vec<Variant>>,
    pricing: Option<Prices>,
}
impl PricingDetail {
    pub fn quotes(&self, now: chrono::DateTime<chrono::Utc>) -> CardMarketPrices {
        let details: Vec<_> = self
            .variants_detailed
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|v| {
                v.size.as_deref() == Some("standard")
                    && v.subtype.is_none()
                    && finish(&v.kind).is_some()
                    && v.variant_id.as_ref().is_none_or(|id| id.len() <= 128)
                    && v.stamp.as_ref().is_none_or(|stamps| {
                        stamps.len() <= 8 && stamps.iter().all(|s| !s.is_empty() && s.len() <= 80)
                    })
            })
            .collect();
        let mut variants = vec![];
        for variant in &details {
            let market = variant.pricing.as_ref().and_then(|p| p.cardmarket.as_ref());
            let mut quote = quote(&variant.kind, market);
            if variant.kind != "reverse"
                && let Some(product) = market.and_then(|m| m.id_product)
                && details.iter().any(|other| {
                    other.kind != "reverse"
                        && other
                            .pricing
                            .as_ref()
                            .and_then(|p| p.cardmarket.as_ref())
                            .and_then(|m| m.id_product)
                            == Some(product)
                        && (other.kind != variant.kind
                            || stamps(&other.stamp) != stamps(&variant.stamp))
                })
            {
                quote.euros = None;
            }
            quote.variant_id = variant.variant_id.clone();
            quote.stamps = variant.stamp.clone().unwrap_or_default();
            quote.pattern = variant.foil.clone();
            variants.push(quote);
        }
        if self.variants_detailed.as_ref().is_none_or(|v| v.is_empty()) {
            let enabled =
                |key: &str| self.variants.as_ref().and_then(|v| v.get(key)) == Some(&true);
            let regular = ["normal", "holo"].iter().filter(|k| enabled(k)).count();
            for kind in ["normal", "holo", "reverse"]
                .into_iter()
                .filter(|k| enabled(k))
            {
                let mut quote = quote(
                    kind,
                    self.pricing.as_ref().and_then(|p| p.cardmarket.as_ref()),
                );
                if kind != "reverse" && regular != 1 {
                    quote.euros = None;
                }
                variants.push(quote);
            }
        }
        CardMarketPrices {
            card_id: self.id.clone(),
            fetched_at: now,
            variants,
            requires_printing_review: self.variants.as_ref().and_then(|v| v.get("firstEdition"))
                == Some(&true),
        }
    }
}
fn stamps(value: &Option<Vec<String>>) -> std::collections::BTreeSet<&str> {
    value
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect()
}
fn finish(kind: &str) -> Option<CardFinish> {
    match kind {
        "normal" => Some(CardFinish::Normal),
        "holo" => Some(CardFinish::Holo),
        "reverse" => Some(CardFinish::ReverseHolo),
        _ => None,
    }
}
fn quote(kind: &str, market: Option<&Market>) -> MarketVariantPrice {
    let euros = market
        .filter(|m| m.unit.as_deref() == Some("EUR"))
        .and_then(|m| {
            if kind == "reverse" {
                m.reverse_trend
            } else {
                m.trend
            }
        })
        .filter(|v| *v > Decimal::ZERO && *v <= Decimal::from(1_000_000));
    MarketVariantPrice {
        finish: finish(kind).unwrap(),
        euros,
        variant_id: None,
        stamps: vec![],
        pattern: None,
        product_id: market.and_then(|m| m.id_product),
        updated: market.and_then(|m| m.updated.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn quotes(input: serde_json::Value) -> CardMarketPrices {
        serde_json::from_value::<PricingDetail>(input)
            .unwrap()
            .quotes(chrono::Utc::now())
    }
    #[test]
    fn ambiguous_regular_series_and_missing_reverse_series_never_substitute() {
        let result = quotes(
            serde_json::json!({"id":"x","variants":{"normal":true,"holo":true,"reverse":true},
            "pricing":{"cardmarket":{"unit":"EUR","trend":2.50,"trend-holo":7.12}}}),
        );
        assert!(result.variants[0].euros.is_none());
        assert!(result.variants[1].euros.is_none());
        assert_eq!(result.variants[2].euros, Some(Decimal::new(712, 2)));
        let result = quotes(
            serde_json::json!({"id":"x","variants":{"normal":true,"reverse":true},
            "pricing":{"cardmarket":{"unit":"EUR","trend":2.50}}}),
        );
        assert_eq!(result.variants[0].euros, Some(Decimal::new(250, 2)));
        assert!(result.variants[1].euros.is_none());
    }
    #[test]
    fn detailed_variant_gaps_and_shared_products_cannot_fall_back_to_root_prices() {
        let result = quotes(serde_json::json!({"id":"x","variants":{"normal":true},
            "pricing":{"cardmarket":{"unit":"EUR","trend":50}},"variants_detailed":[
                {"variantId":"1","type":"normal","size":"standard","stamp":[],"pricing":{"cardmarket":{"idProduct":10,"unit":"EUR","trend":2}}},
                {"variantId":"2","type":"normal","size":"standard","stamp":["trick-or-trade"],"pricing":{"cardmarket":{"idProduct":10,"unit":"EUR","trend":2}}},
                {"variantId":"3","type":"holo","size":"standard"},
                {"variantId":"4","type":"holo","size":"jumbo"}]}));
        assert_eq!(result.variants.len(), 3);
        assert!(result.variants.iter().all(|v| v.euros.is_none()));
        assert_eq!(result.variants[1].stamps, ["trick-or-trade"]);
    }
    #[test]
    fn decimal_quotes_keep_exact_json_precision_and_invalid_currency_has_no_value() {
        let detail:PricingDetail=serde_json::from_str(r#"{"id":"x","variants":{"normal":true},"pricing":{"cardmarket":{"unit":"EUR","trend":123.4567890123456789}}}"#).unwrap();
        let result = detail.quotes(chrono::Utc::now());
        assert_eq!(
            result.variants[0].euros.unwrap().to_string(),
            "123.4567890123456789"
        );
        let encoded = serde_json::to_string(&result).unwrap();
        assert!(encoded.contains("\"euros\":123.4567890123456789"));
        for (unit, value) in [("USD", 1), ("EUR", 0), ("EUR", 1_000_001)] {
            let result = quotes(
                serde_json::json!({"id":"x","variants":{"normal":true},"pricing":{"cardmarket":{"unit":unit,"trend":value}}}),
            );
            assert!(result.variants[0].euros.is_none());
        }
    }
}
