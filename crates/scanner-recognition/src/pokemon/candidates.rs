use crate::matching::Ranked;
use scanner_catalogue::TcgDex;
use scanner_types::{
    error::Result,
    recognition::{CardCandidate, CardFinish},
};
pub(super) async fn resolve(
    ranks: &[Ranked],
    catalogue: &TcgDex,
    codes: &std::collections::BTreeSet<String>,
) -> Result<Vec<CardCandidate>> {
    let mut results = vec![];
    for rank in ranks.iter().take(8) {
        let detail = catalogue.detail(&rank.card.id).await?;
        let total = detail.set.card_count.as_ref().and_then(|c| c.official);
        if total.is_some_and(|t| !rank.totals.is_empty() && !rank.totals.contains(&t)) {
            continue;
        }
        let mut evidence = rank.evidence.clone();
        if !codes.is_empty() {
            // A confident code is a constraint, never an invented catalogue ID.
            let set = catalogue.set(&detail.set.id).await?;
            let Some(code) = super::set_code::agrees(codes, set.abbreviation.as_ref()) else {
                continue;
            };
            evidence.push(format!("Printed set code {code}"));
        }
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
        extensions.insert(
            "printingVariants".into(),
            serde_json::json!(detail.printing_variants()),
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_catalogue::JsonTransport;
    use scanner_types::{cards::CardLanguage, recognition::RecognizedLine};
    use std::{collections::BTreeSet, sync::Arc};
    struct Printings;
    #[async_trait::async_trait]
    impl JsonTransport for Printings {
        async fn get(&self, path: &str, _: usize) -> Result<Vec<u8>> {
            let value = match path {
                "en/cards" => serde_json::json!([
                    {"id":"me04-025","localId":"025","name":"Wimpod"},
                    {"id":"fixture-025","localId":"025","name":"Wimpod"}
                ]),
                "en/sets/me04" => {
                    serde_json::json!({"id":"me04","cards":[{"id":"me04-025","localId":"025","name":"Wimpod"}],"abbreviation":{"official":"CRI"}})
                }
                "en/sets/fixture" => {
                    serde_json::json!({"id":"fixture","cards":[{"id":"fixture-025","localId":"025","name":"Wimpod"}],"abbreviation":{"official":"XYZ"}})
                }
                "en/cards/me04-025" | "en/cards/fixture-025" => {
                    let id = path.strip_prefix("en/cards/").unwrap();
                    let set = id.rsplit_once('-').unwrap().0;
                    serde_json::json!({"id":id,"localId":"025","name":"Wimpod","set":{"id":set,"name":set,"cardCount":{"official":86}}})
                }
                _ => panic!("Unexpected catalogue request: {path}"),
            };
            Ok(serde_json::to_vec(&value).unwrap())
        }
    }
    #[tokio::test]
    async fn wimpod_printed_code_disambiguates_same_number_and_total_across_ocr_passes() {
        let catalogue = TcgDex::with_transport(CardLanguage::English, Arc::new(Printings));
        let line = |text: &str, y| RecognizedLine {
            text: text.into(),
            y,
            x: 0.1,
            width: 0.4,
            height: 0.04,
            confidence: 0.95,
        };
        let plain = vec![line("Wimpod", 0.9), line("025/086", 0.04)];
        let read = |lines| crate::readings::Reading { turns: 0, lines };
        let baseline =
            super::super::resolve(vec![read(plain.clone())], CardLanguage::English, &catalogue)
                .await
                .unwrap();
        assert_eq!(baseline.candidates.len(), 2);
        let mut coded = plain.clone();
        coded.push(line("CRI EN", 0.04));
        let result = super::super::resolve(
            vec![read(plain), read(coded)],
            CardLanguage::English,
            &catalogue,
        )
        .await
        .unwrap();
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].id, "me04-025");
        assert!(
            result.candidates[0]
                .evidence
                .contains(&"Printed set code CRI".into())
        );
        let ranks = crate::matching::PokemonMatcher::new(
            catalogue.catalogue().await.unwrap().as_ref().clone(),
            CardLanguage::English,
        )
        .rank(&[line("Wimpod", 0.9), line("025/086", 0.04)]);
        assert!(
            resolve(&ranks, &catalogue, &BTreeSet::from(["ZZZ".into()]))
                .await
                .unwrap()
                .is_empty()
        );
    }
}
