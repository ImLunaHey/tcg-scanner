mod filtering;
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use scanner_types::{
    cards::CardLanguage,
    finish::FinishEstimate,
    identification::CardIdentification,
    pricing::{CardMarketPrices, CardValuation, EuroSterlingRate, ValuationState},
    recognition::CardCandidate,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub struct Snapshot {
    pub prices: BTreeMap<String, CardMarketPrices>,
    pub rate: Option<EuroSterlingRate>,
    pub pending: BTreeSet<String>,
    pub rate_pending: bool,
}
pub fn value(
    number: u32,
    identity: &CardIdentification,
    card: &CardCandidate,
    snapshot: &Snapshot,
    now: DateTime<Utc>,
) -> CardValuation {
    let mut result = CardValuation::unavailable(number, "Price unavailable.");
    result.card_id = Some(card.id.clone());
    if identity.request.language != CardLanguage::English {
        result.reason = "Identification language does not match the English pricing source.".into();
        return result;
    }
    let estimate = match identity
        .extensions
        .get("finishEstimate")
        .map(|v| serde_json::from_value::<FinishEstimate>(v.clone()))
        .transpose()
    {
        Ok(value) => value,
        Err(_) => {
            result.reason = "Refresh unsupported finish evidence before pricing.".into();
            return result;
        }
    };
    result.edition_key = estimate
        .as_ref()
        .and_then(|e| e.edition.as_ref())
        .map(|e| e.pricing_key.clone());
    let edition = match filtering::edition(estimate.as_ref(), card) {
        Ok(v) => v,
        Err(reason) => {
            result.reason = reason.into();
            return result;
        }
    };
    let Some((prices, rate)) = snapshot.prices.get(&card.id).zip(snapshot.rate.as_ref()) else {
        if snapshot.pending.contains(&card.id) || snapshot.rate_pending {
            result.state = ValuationState::Pending;
            result.reason = "Loading Cardmarket prices and GBP rate.".into();
        } else {
            result.reason =
                "Cardmarket price or GBP rate unavailable; will retry automatically.".into();
        }
        return result;
    };
    if prices.card_id != card.id {
        result.reason = "Cached catalogue price does not match this card.".into();
        return result;
    }
    if !fresh_enough(prices, rate, now) {
        result.reason = "Cached prices or exchange rate are too old or invalid.".into();
        return result;
    }
    if prices.requires_printing_review {
        result.reason = "Edition must be identified before pricing this card.".into();
        return result;
    }
    let (finish, finishes) = filtering::finishes(identity, card, estimate.as_ref(), edition);
    let quotes = match filtering::quotes(prices, edition, &finishes) {
        Ok(v) => v,
        Err(reason) => {
            result.reason = reason.into();
            return result;
        }
    };
    priced(
        result,
        quotes,
        prices,
        rate,
        finish.is_some(),
        edition.is_some(),
        now,
    )
}
fn priced(
    mut result: CardValuation,
    quotes: Vec<&scanner_types::pricing::MarketVariantPrice>,
    prices: &CardMarketPrices,
    rate: &EuroSterlingRate,
    chosen: bool,
    stamped: bool,
    now: DateTime<Utc>,
) -> CardValuation {
    let pence: Option<Vec<_>> = quotes
        .iter()
        .map(|q| {
            q.euros?
                .checked_mul(rate.pounds_per_euro)?
                .checked_mul(Decimal::ONE_HUNDRED)?
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_i64()
        })
        .collect();
    let Some(pence) = pence else {
        result.reason = "Price conversion exceeds supported bounds.".into();
        return result;
    };
    result.low_pence = pence.iter().min().copied();
    result.high_pence = pence.iter().max().copied();
    result.state = if !chosen || quotes.len() > 1 {
        ValuationState::Range
    } else {
        ValuationState::Priced
    };
    result.reason = match (stamped, result.state) {
        (true, ValuationState::Range) => "Range across matching stamped variants.",
        (true, _) => "Cardmarket trend for the matched stamped variant.",
        (false, ValuationState::Range) => "Range across possible finishes or foil patterns.",
        (false, _) => "Cardmarket trend for the matched finish.",
    }
    .into();
    result.stale = now - prices.fetched_at > chrono::Duration::hours(6)
        || now - rate.fetched_at > chrono::Duration::hours(6)
        || quotes.iter().any(|q| {
            q.updated
                .as_deref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .is_none_or(|d| {
                    now - d.with_timezone(&Utc) > chrono::Duration::days(3)
                        || d > now + chrono::Duration::days(1)
                })
        });
    result.quotes = quotes.into_iter().cloned().collect();
    result
}
fn fresh_enough(prices: &CardMarketPrices, rate: &EuroSterlingRate, now: DateTime<Utc>) -> bool {
    [prices.fetched_at, rate.fetched_at]
        .into_iter()
        .all(|date| {
            now - date < chrono::Duration::days(7) && date <= now + chrono::Duration::days(1)
        })
        && rate.pounds_per_euro > Decimal::ZERO
        && rate.pounds_per_euro < Decimal::TEN
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{pricing::MarketVariantPrice, recognition::CardFinish};
    fn fixture() -> (CardIdentification, CardCandidate, Snapshot) {
        let now: DateTime<Utc> = "2026-11-10T02:00:00Z".parse().unwrap();
        let identity=serde_json::from_value(serde_json::json!({"request":{"id":"00000000-0000-4000-8000-000000000001",
            "jobID":"00000000-0000-4000-8000-000000000002","sequence":2,"language":"en"},
            "sourceSHA256":"fixture","state":"reviewRequired","createdAt":now,"method":"fixture","assessmentSource":"manual-review","finish":"normal"})).unwrap();
        let card=serde_json::from_value(serde_json::json!({"id":"sv01-104","localID":"104","name":"Greavard","setID":"sv01",
            "setName":"Scarlet & Violet","evidence":["Collector number 104"],"availableFinishes":["normal","reverseHolo"]})).unwrap();
        let quote = MarketVariantPrice {
            variant_id: None,
            stamps: vec![],
            finish: CardFinish::Normal,
            pattern: None,
            product_id: Some(1),
            euros: Some(Decimal::new(125, 2)),
            updated: Some(now.to_rfc3339()),
        };
        let mut snapshot = Snapshot {
            rate: Some(EuroSterlingRate {
                pounds_per_euro: Decimal::new(90, 2),
                date: "2026-11-09".into(),
                fetched_at: now,
            }),
            ..Default::default()
        };
        snapshot.prices.insert(
            "sv01-104".into(),
            CardMarketPrices {
                card_id: "sv01-104".into(),
                fetched_at: now,
                variants: vec![quote],
                requires_printing_review: false,
            },
        );
        (identity, card, snapshot)
    }
    #[test]
    fn money_rounds_per_physical_card_and_missing_quotes_are_never_zero() {
        let (mut identity, card, mut snapshot) = fixture();
        let now = snapshot.rate.as_ref().unwrap().fetched_at;
        let priced = value(1, &identity, &card, &snapshot, now);
        assert_eq!(priced.low_pence, Some(113));
        assert_eq!(priced.state, ValuationState::Priced);
        assert!(!priced.stale);
        let batch = scanner_types::pricing::BatchValuation::new(
            identity.request.job_id,
            vec![priced.clone(), priced],
            snapshot.rate.clone(),
            false,
        );
        assert_eq!(batch.low_pence, Some(226)); // Two 112.5p copies round separately.
        identity.finish = CardFinish::Unverified;
        assert!(
            value(1, &identity, &card, &snapshot, now)
                .low_pence
                .is_none()
        );
        let mut reverse = snapshot.prices[&card.id].variants[0].clone();
        reverse.finish = CardFinish::ReverseHolo;
        reverse.euros = Some(Decimal::from(5));
        snapshot
            .prices
            .get_mut(&card.id)
            .unwrap()
            .variants
            .push(reverse);
        let range = value(1, &identity, &card, &snapshot, now);
        assert_eq!((range.low_pence, range.high_pence), (Some(113), Some(450)));
        assert_eq!(range.state, ValuationState::Range);
        assert!(
            value(
                1,
                &identity,
                &card,
                &snapshot,
                now + chrono::Duration::days(8)
            )
            .low_pence
            .is_none()
        );
        identity.request.language = CardLanguage::Korean;
        assert!(
            value(1, &identity, &card, &snapshot, now)
                .low_pence
                .is_none()
        );
    }
    #[test]
    fn stamped_identity_cannot_receive_ordinary_or_wrong_variant_quotes() {
        use scanner_types::edition::*;
        let (mut identity, card, mut snapshot) = fixture();
        let now = snapshot.rate.as_ref().unwrap().fetched_at;
        let stamp = StampEvidence {
            state: StampState::Present,
            kind: "pikachu-pumpkin".into(),
            method: "fixture".into(),
            region: None,
            source_quarter_turns: 0,
            similarity: None,
            baseline_similarity: None,
            downsampled_similarity: None,
            evidence: vec![],
        };
        let variant = PrintingVariant {
            id: "stamped-1".into(),
            source: "tcgdex".into(),
            finish: CardFinish::Normal,
            stamps: vec!["trick-or-trade".into()],
            years: vec![],
        };
        let edition = EditionEvidence::new(
            EditionState::Matched,
            Some(card.id.clone()),
            stamp,
            None,
            vec![variant],
            vec![],
        );
        let mut estimate = FinishEstimate::new(
            scanner_types::finish::FinishState::Uncertain,
            Some(card.id.clone()),
            vec![],
            None,
        );
        estimate.edition = Some(edition);
        identity
            .extensions
            .insert("finishEstimate".into(), serde_json::json!(estimate));
        assert!(
            value(1, &identity, &card, &snapshot, now)
                .low_pence
                .is_none()
        );
        let quote = &mut snapshot.prices.get_mut(&card.id).unwrap().variants[0];
        quote.variant_id = Some("stamped-1".into());
        quote.stamps = vec!["trick-or-trade".into()];
        let priced = value(1, &identity, &card, &snapshot, now);
        assert_eq!(priced.low_pence, Some(113));
        assert!(priced.edition_key.is_some());
        estimate.candidate_id = Some("different".into());
        identity
            .extensions
            .insert("finishEstimate".into(), serde_json::json!(estimate));
        assert!(
            value(1, &identity, &card, &snapshot, now)
                .low_pence
                .is_none()
        );
    }
}
