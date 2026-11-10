use crate::{cards::CardGame, images::OriginalImage, scan::ScanRecord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardGameDetection {
    pub number: u32,
    pub game: CardGame,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_sequence: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back_sequence: Option<u32>,
    #[serde(
        default,
        rename = "backSHA256",
        skip_serializing_if = "Option::is_none"
    )]
    pub back_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    pub reason: String,
}
impl CardGameDetection {
    pub fn resolve(record: &ScanRecord) -> Vec<Self> {
        if !record.request.duplex && record.request.extensions.contains_key("cameraImportSHA256") {
            return camera_fronts(record);
        }
        let mut groups: BTreeMap<u32, Vec<&OriginalImage>> = BTreeMap::new();
        for image in &record.images {
            groups
                .entry(image.sequence.div_ceil(2))
                .or_default()
                .push(image);
        }
        groups
            .into_iter()
            .map(|(number, images)| classify(record, number, &images))
            .collect()
    }
}
fn camera_fronts(record: &ScanRecord) -> Vec<CardGameDetection> {
    record.images.iter().map(|image| CardGameDetection {
        number: image.sequence,
        game: image.extensions.get("reviewedCameraGame").cloned()
            .and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default(),
        content_kind: None, front_sequence: Some(image.sequence), back_sequence: None,
        back_sha256: None, method: Some("reviewed-camera-front".into()),
        reason: "Single-sided camera capture; catalogue identity reviewed on import. No back captured.".into(),
    }).collect()
}

fn classify(record: &ScanRecord, number: u32, images: &[&OriginalImage]) -> CardGameDetection {
    let mut result = CardGameDetection {
        number,
        game: CardGame::Unknown,
        content_kind: None,
        front_sequence: None,
        back_sequence: None,
        back_sha256: None,
        method: None,
        reason: "A complete duplex pair with one recognized back is required.".into(),
    };
    let mut sequences: Vec<_> = images.iter().map(|i| i.sequence).collect();
    sequences.sort();
    if !record.request.duplex
        || !(1..=500).contains(&number)
        || sequences != [number * 2 - 1, number * 2]
    {
        return result;
    }
    let plans: Vec<_> = images
        .iter()
        .filter_map(|original| {
            let edit = record
                .processed_images
                .as_deref()?
                .iter()
                .rev()
                .find(|edit| {
                    edit.request.sequence == original.sequence && edit.card_preparation.is_some()
                })?;
            (edit.source_sha256 == original.sha256)
                .then_some((*original, edit.card_preparation.as_ref().unwrap()))
        })
        .collect();
    let backs: Vec<_> = plans
        .iter()
        .filter_map(|(original, plan)| {
            if !plan.orientation_verified {
                return None;
            }
            let basis = plan.orientation_basis.as_deref()?;
            let game = match basis {
                "pokemon-back-pattern" => CardGame::Pokemon,
                "yugioh-back-pattern" => CardGame::Yugioh,
                "starwars-back-pattern" => CardGame::StarwarsUnlimited,
                _ => return None,
            };
            Some((*original, game, basis))
        })
        .collect();
    let bases: Vec<_> = plans
        .iter()
        .filter_map(|(_, p)| p.orientation_basis.as_deref())
        .collect();
    if classify_content(&mut result, &bases, backs.is_empty()) {
        return result;
    }
    if backs.len() != 1 {
        if backs.len() > 1 {
            result.reason = "Both captures look like card backs; check pairing.".into();
        }
        return result;
    }
    let (back, game, basis) = backs[0];
    result.game = game;
    result.back_sequence = Some(back.sequence);
    result.front_sequence = images
        .iter()
        .find(|i| i.sequence != back.sequence)
        .map(|i| i.sequence);
    result.back_sha256 = Some(back.sha256.clone());
    result.method = Some(basis.into());
    result.reason =
        "Game and sides detected from the card back; captures paired in acquisition order.".into();
    result
}
fn classify_content(result: &mut CardGameDetection, bases: &[&str], no_backs: bool) -> bool {
    let url = bases.contains(&"pokemon-code-url");
    if url && !no_backs {
        result.reason =
            "Redemption URL and playable-card back evidence conflict; check pairing.".into();
        return true;
    }
    if no_backs && bases.contains(&"starwars-card-text") && !url {
        result.game = CardGame::StarwarsUnlimited;
        result.method = Some("starwars-card-text".into());
        result.reason="Printed SWU footer detected. No standard logo back; retain numbered sides for two-sided cards.".into();
        return true;
    }
    if no_backs && (url || bases.iter().filter(|b| **b == "pokemon-code-text").count() == 2) {
        result.content_kind = Some("pokemonCodeCard".into());
        result.method = Some("pokemon-code-evidence-v2".into());
        result.reason = if url {
            "Printed Pokémon redemption URL detected; not a playable card."
        } else {
            "Printed Pokémon online/redemption wording detected on both sides; not a playable card."
        }
        .into();
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pair(first: &str, second: &str) -> ScanRecord {
        let mut record = ScanRecord::new(
            crate::device::Device::default(),
            crate::scan::ScanRequest {
                duplex: true,
                ..Default::default()
            },
            "fixture".into(),
        );
        for (sequence, basis) in [(1, first), (2, second)] {
            let image:OriginalImage=serde_json::from_value(serde_json::json!({"sequence":sequence,"relativePath":format!("C-{sequence:03}.png"),"bytes":1,"sha256":format!("source-{sequence}"),"width":100,"height":150,"imageCount":1,"uniformType":"public.png","side":"unknown"})).unwrap();
            let edit=serde_json::from_value(serde_json::json!({"request":{"id":uuid::Uuid::new_v4(),"jobID":record.id,"sequence":sequence,"crop":{"x":0,"y":0,"width":1,"height":1},"quarterTurns":0},"createdAt":"2026-11-09T12:00:00Z","sourceSHA256":image.sha256,"image":image,"cardPreparation":{"crop":{"x":0,"y":0,"width":1,"height":1},"quarterTurns":0,"orientationVerified":true,"orientationBasis":basis,"warnings":[],"method":"fixture"}})).unwrap();
            record.images.push(image);
            record
                .processed_images
                .get_or_insert_with(Vec::new)
                .push(edit);
        }
        record
    }
    #[test]
    fn pairing_requires_matching_sources_and_rejects_conflicting_backs_or_codes() {
        let mut record = pair("text", "pokemon-back-pattern");
        let detected = CardGameDetection::resolve(&record).remove(0);
        assert_eq!(detected.game, CardGame::Pokemon);
        assert_eq!(detected.front_sequence, Some(1));
        assert_eq!(detected.back_sequence, Some(2));
        record.processed_images.as_mut().unwrap()[1].source_sha256 = "changed".into();
        assert_eq!(
            CardGameDetection::resolve(&record)[0].game,
            CardGame::Unknown
        );
        for (a, b) in [
            ("pokemon-back-pattern", "pokemon-back-pattern"),
            ("pokemon-code-url", "pokemon-back-pattern"),
        ] {
            assert_eq!(
                CardGameDetection::resolve(&pair(a, b))[0].game,
                CardGame::Unknown
            );
        }
        assert_eq!(
            CardGameDetection::resolve(&pair("pokemon-code-text", "pokemon-code-text"))[0]
                .content_kind
                .as_deref(),
            Some("pokemonCodeCard")
        );
        assert_eq!(
            CardGameDetection::resolve(&pair("starwars-card-text", "text"))[0].game,
            CardGame::StarwarsUnlimited
        );
        assert_eq!(
            CardGameDetection::resolve(&pair("text", "yugioh-back-pattern"))[0].game,
            CardGame::Yugioh
        );
        let mut record = pair("text", "pokemon-back-pattern");
        record.request.duplex = false;
        assert_eq!(
            CardGameDetection::resolve(&record)[0].game,
            CardGame::Unknown
        );
    }
    #[test]
    fn saved_pair_routing_agrees_with_existing_manifest() {
        let Some(path) = std::env::var_os("SCANNER_COMPAT_MANIFEST") else {
            return;
        };
        let record: ScanRecord = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let expected = record
            .extensions
            .get("cardGames")
            .unwrap()
            .as_array()
            .unwrap();
        let actual = CardGameDetection::resolve(&record);
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            let actual = serde_json::to_value(actual).unwrap();
            for field in [
                "number",
                "game",
                "contentKind",
                "frontSequence",
                "backSequence",
                "backSHA256",
                "method",
            ] {
                assert_eq!(
                    actual.get(field),
                    expected.get(field),
                    "Pair {} field {field}",
                    actual["number"]
                );
            }
        }
    }
}
