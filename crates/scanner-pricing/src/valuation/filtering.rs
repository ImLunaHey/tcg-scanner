use rust_decimal::Decimal;
use scanner_types::{
    edition::{EditionState, PrintingVariant, StampState},
    finish::{FinishEstimate, FinishState, FinishUncertainty},
    identification::CardIdentification,
    pricing::{CardMarketPrices, MarketVariantPrice},
    recognition::{CardCandidate, CardFinish},
};
pub(super) fn edition<'a>(
    estimate: Option<&'a FinishEstimate>,
    card: &CardCandidate,
) -> Result<Option<&'a [PrintingVariant]>, &'static str> {
    let Some(estimate) = estimate else {
        return Ok(None);
    };
    let Some(edition) = estimate
        .edition
        .as_ref()
        .filter(|e| e.needs_edition_handling())
    else {
        return Ok(None);
    };
    if edition.candidate_id.as_deref() != Some(&card.id)
        || estimate.candidate_id.as_deref() != Some(&card.id)
    {
        return Err("Refresh stamped-edition evidence for the selected card before pricing.");
    }
    if edition.stamp.state != StampState::Present
        || edition.state != EditionState::Matched
        || edition.variants.is_empty()
    {
        return Err(
            "The stamped edition must be resolved before pricing; ordinary-printing prices are not substituted.",
        );
    }
    if !edition
        .variants
        .iter()
        .all(|v| v.source == "tcgdex" && !v.id.is_empty() && !v.stamps.is_empty())
    {
        return Err("No matching TCGdex stamped-variant price is available.");
    }
    Ok(Some(&edition.variants))
}
pub(super) fn finishes(
    identity: &CardIdentification,
    card: &CardCandidate,
    estimate: Option<&FinishEstimate>,
    edition: Option<&[PrintingVariant]>,
) -> (Option<CardFinish>, Vec<CardFinish>) {
    let estimate = estimate.filter(|e| e.candidate_id.as_deref() == Some(&card.id));
    let chosen = if identity.finish != CardFinish::Unverified {
        Some(identity.finish)
    } else {
        estimate
            .filter(|e| e.state == FinishState::Estimated)
            .and_then(|e| e.suggested_finish)
    };
    let finishes = if let Some(finish) = chosen {
        vec![finish]
    } else if let Some(edition) = edition {
        edition.iter().map(|v| v.finish).collect()
    } else if estimate
        .is_some_and(|e| e.uncertainty_reason == Some(FinishUncertainty::MultipleRegularFinishes))
    {
        vec![CardFinish::Normal, CardFinish::Holo]
    } else {
        card.available_finishes
            .iter()
            .copied()
            .filter(supported)
            .collect()
    };
    (chosen, finishes)
}
pub(super) fn quotes<'a>(
    prices: &'a CardMarketPrices,
    edition: Option<&[PrintingVariant]>,
    finishes: &[CardFinish],
) -> Result<Vec<&'a MarketVariantPrice>, &'static str> {
    if finishes.is_empty() || !finishes.iter().all(supported) {
        return Err("Finish needs identification before pricing.");
    }
    let quotes: Vec<_> = if let Some(edition) = edition {
        let possible: Vec<_> = edition
            .iter()
            .filter(|v| finishes.contains(&v.finish))
            .collect();
        if possible.is_empty()
            || !possible
                .iter()
                .all(|v| prices.variants.iter().any(|q| matches(q, v)))
        {
            return Err("No reliable price for every matching stamped variant.");
        }
        prices
            .variants
            .iter()
            .filter(|q| possible.iter().any(|v| matches(q, v)))
            .collect()
    } else {
        prices
            .variants
            .iter()
            .filter(|q| q.stamps.is_empty() && finishes.contains(&q.finish))
            .collect()
    };
    if quotes.is_empty()
        || !finishes
            .iter()
            .all(|f| quotes.iter().any(|q| q.finish == *f))
        || !quotes.iter().all(|q| {
            q.euros
                .is_some_and(|v| v > Decimal::ZERO && v <= Decimal::from(1_000_000))
        })
    {
        return Err(if edition.is_some() {
            "No reliable price for every matching stamped variant."
        } else {
            "No reliable trend price for every possible finish or printing."
        });
    }
    Ok(quotes)
}
fn supported(f: &CardFinish) -> bool {
    matches!(
        f,
        CardFinish::Normal | CardFinish::Holo | CardFinish::ReverseHolo
    )
}
fn matches(quote: &MarketVariantPrice, variant: &PrintingVariant) -> bool {
    let stamps = |s: &[String]| s.iter().cloned().collect::<std::collections::BTreeSet<_>>();
    quote.variant_id.as_deref() == Some(&variant.id)
        && quote.finish == variant.finish
        && stamps(&quote.stamps) == stamps(&variant.stamps)
}
