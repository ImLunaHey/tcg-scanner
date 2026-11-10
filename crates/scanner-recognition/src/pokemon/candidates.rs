use crate::matching::Ranked;
use scanner_catalogue::TcgDex;
use scanner_types::{
    error::Result,
    recognition::{CardCandidate, CardFinish},
};
pub(super) async fn resolve(ranks: &[Ranked], catalogue: &TcgDex) -> Result<Vec<CardCandidate>> {
    let mut results = vec![];
    for rank in ranks.iter().take(8) {
        let detail = catalogue.detail(&rank.card.id).await?;
        let total = detail.set.card_count.as_ref().and_then(|c| c.official);
        if total.is_some_and(|t| !rank.totals.is_empty() && !rank.totals.contains(&t)) {
            continue;
        }
        let mut evidence = rank.evidence.clone();
        if let Some(total) = total.filter(|t| rank.totals.contains(t)) {
            evidence.push(format!("Printed set total {total}"));
        }
        let finishes = [
            ("normal", CardFinish::Normal),
            ("holo", CardFinish::Holo),
            ("reverse", CardFinish::ReverseHolo),
        ]
        .into_iter()
        .filter(|(key, _)| detail.variants.as_ref().and_then(|v| v.get(*key)) == Some(&true))
        .map(|(_, finish)| finish)
        .collect();
        let mut extensions = scanner_types::Extensions::new();
        if let Some(rarity) = &detail.rarity {
            extensions.insert("rarities".into(), serde_json::json!([rarity]));
        }
        results.push(CardCandidate {
            id: detail.id.clone(),
            name: detail.name.clone(),
            local_id: detail.local_id.clone(),
            set_id: detail.set.id.clone(),
            set_name: detail.set.name.clone(),
            official_set_count: total,
            evidence,
            category: detail.category.clone(),
            types: detail.types.clone(),
            available_finishes: finishes,
            extensions,
        });
        if results.len() == 5 {
            break;
        }
    }
    Ok(results)
}
