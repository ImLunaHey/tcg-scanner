use crate::Inventory;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use scanner_types::{
    cards::CardGame,
    error::Result,
    game_detection::CardGameDetection,
    identification::{CardIdentification, IdentificationState},
    images::OriginalImage,
    inventory::{InventoryCatalogueSource, InventoryCopy, InventoryEntry, InventoryPreview},
    recognition::CardCandidate,
    scan::ScanRecord,
};
pub fn selected_candidate(identity: &CardIdentification) -> Option<&CardCandidate> {
    let candidates = &identity.evidence.as_ref()?.candidates;
    match identity.state {
        IdentificationState::Confirmed => candidates
            .iter()
            .find(|c| Some(&c.id) == identity.selected_card_id.as_ref()),
        IdentificationState::ReviewRequired if candidates.len() == 1 => candidates.first(),
        _ => None,
    }
}
impl Inventory {
    pub(crate) fn entry(
        &mut self,
        record: &ScanRecord,
        number: u32,
        images: &[&OriginalImage],
        identities: &[CardIdentification],
        detection: Option<&CardGameDetection>,
    ) -> Result<InventoryEntry> {
        let game = detection.map_or(CardGame::Unknown, |d| d.game);
        let language = record.request.card_language.unwrap_or_default();
        let identities: Vec<_> = identities
            .iter()
            .filter(|i| {
                images
                    .iter()
                    .any(|image| image.sequence == i.request.sequence)
                    && i.request.language == language
                    && detection
                        .and_then(|d| d.front_sequence)
                        .is_none_or(|seq| seq == i.request.sequence)
            })
            .collect();
        let matched: Vec<_> = identities
            .iter()
            .filter_map(|i| selected_candidate(i).map(|card| (*i, card)))
            .collect();
        let waiting: Vec<_> = identities
            .iter()
            .filter(|i| {
                i.state == IdentificationState::AwaitingCatalogue
                    && i.evidence
                        .as_ref()
                        .is_some_and(|e| e.extensions.contains_key("uncataloguedCard"))
            })
            .collect();
        let complete = !record.request.duplex || images.len() == 2;
        let copy = InventoryCopy {
            batch_id: record.id,
            batch_number: record.batch_number,
            card_number: number,
        };
        let mut entry = InventoryEntry {
            id: key(&[
                "unresolved",
                &record.id.to_string().to_uppercase(),
                &number.to_string(),
            ]),
            game,
            language: language.as_str().into(),
            name: if complete {
                "Needs identification"
            } else {
                "Incomplete card pair"
            }
            .into(),
            set_name: None,
            number: None,
            status: "unresolved".into(),
            quantity: 1,
            confirmed_quantity: 0,
            preview: None,
            set_key: format!("{}-unidentified", game.as_str()),
            copies: vec![copy],
        };
        if complete && game != CardGame::Unknown {
            if let [(identity, card)] = matched.as_slice() {
                self.catalogued(&mut entry, identity, card, language);
            } else if matched.is_empty() && waiting.len() == 1 {
                printed(&mut entry, waiting[0]);
            }
        }
        let sequence = detection
            .and_then(|d| d.front_sequence)
            .or_else(|| matched.first().map(|(i, _)| i.request.sequence))
            .or_else(|| waiting.first().map(|i| i.request.sequence))
            .or_else(|| images.iter().map(|i| i.sequence).min());
        entry.preview = preview(record, images, sequence);
        Ok(entry)
    }
    fn catalogued(
        &mut self,
        entry: &mut InventoryEntry,
        identity: &CardIdentification,
        card: &CardCandidate,
        language: scanner_types::cards::CardLanguage,
    ) {
        let provider = card
            .extensions
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("tcgdex");
        let set = if provider == "ygoprodeck" {
            card.set_id.split('-').next().unwrap_or("unidentified")
        } else {
            &card.set_id
        };
        let source = InventoryCatalogueSource {
            provider: provider.into(),
            set_id: set.into(),
            language,
        };
        let set_key = format!("{}-{provider}-{set}", entry.game.as_str());
        self.sources
            .entry(set_key.clone())
            .or_default()
            .insert(source.clone());
        self.owned
            .entry(source)
            .or_default()
            .insert(card.id.clone());
        entry.id = key(&[
            entry.game.as_str(),
            &entry.language,
            "catalogue",
            provider,
            &card.id,
        ]);
        entry.name = card.name.clone();
        entry.set_name = Some(card.set_name.clone());
        entry.number = Some(format!(
            "{}{}",
            card.local_id,
            card.official_set_count
                .map(|n| format!("/{n}"))
                .unwrap_or_default()
        ));
        entry.status = "catalogue".into();
        entry.set_key = set_key;
        entry.confirmed_quantity = usize::from(identity.state == IdentificationState::Confirmed);
    }
}
fn preview(
    record: &ScanRecord,
    images: &[&OriginalImage],
    sequence: Option<u32>,
) -> Option<InventoryPreview> {
    let original = images.iter().find(|i| Some(i.sequence) == sequence)?;
    let prepared = record
        .processed_images
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
        .find(|e| {
            e.request.sequence == original.sequence
                && e.card_preparation.is_some()
                && e.source_sha256 == original.sha256
        });
    Some(InventoryPreview {
        batch_id: record.id,
        image: prepared.map(|e| &e.image).unwrap_or(original).clone(),
        edit_id: prepared.map(|e| e.request.id),
    })
}

fn printed(entry: &mut InventoryEntry, identity: &CardIdentification) {
    let Some(printed) = identity
        .evidence
        .as_ref()
        .and_then(|e| e.extensions.get("uncataloguedCard"))
    else {
        return;
    };
    let (Some(name), Some(set), Some(number)) = (
        printed["name"].as_str(),
        printed["setCode"].as_str(),
        printed["collectorNumber"].as_str(),
    ) else {
        return;
    };
    entry.id = key(&[
        entry.game.as_str(),
        &entry.language,
        "awaitingCatalogue",
        set,
        number,
        &name.to_lowercase(),
    ]);
    entry.name = name.into();
    entry.set_name = Some(printed["setName"].as_str().unwrap_or(set).into());
    entry.number = Some(number.into());
    entry.status = "awaitingCatalogue".into();
    entry.set_key = format!("{}-printed-{set}", entry.game.as_str());
}
fn key(parts: &[&str]) -> String {
    STANDARD.encode(
        serde_json::to_string(parts)
            .expect("String array")
            .replace('/', "\\/"),
    )
}
