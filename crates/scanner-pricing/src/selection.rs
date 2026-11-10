use scanner_types::{
    cards::CardGame,
    error::Result,
    game_detection::CardGameDetection,
    identification::{CardIdentification, IdentificationState},
    recognition::CardCandidate,
    scan::ScanRecord,
};
use std::collections::BTreeMap;
pub struct Selection {
    pub number: u32,
    pub matched: Option<(CardIdentification, CardCandidate)>,
}
pub fn select(record: &ScanRecord) -> Result<Vec<Selection>> {
    let paired = record.request.duplex
        && (record.request.prepare_cards == Some(true)
            || record
                .processed_images
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|e| e.card_preparation.is_some()));
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for image in &record.images {
        groups
            .entry(if paired {
                image.sequence.div_ceil(2)
            } else {
                image.sequence
            })
            .or_default()
            .push(image.sequence);
    }
    let identities = record.identifications()?;
    let games = CardGameDetection::resolve(record);
    Ok(groups
        .into_iter()
        .map(|(number, images)| {
            let pokemon = images.first().is_some_and(|sequence| {
                games.iter().any(|g| {
                    (g.front_sequence == Some(*sequence) || g.back_sequence == Some(*sequence))
                        && g.game == CardGame::Pokemon
                })
            });
            let matches: Vec<_> = identities
                .iter()
                .filter(|i| {
                    images.contains(&i.request.sequence)
                        && matches!(
                            i.state,
                            IdentificationState::Confirmed | IdentificationState::ReviewRequired
                        )
                        && i.evidence
                            .as_ref()
                            .is_some_and(|e| !e.candidates.is_empty())
                })
                .collect();
            let matched = if pokemon && matches.len() == 1 && (!paired || images.len() == 2) {
                candidate(matches[0]).map(|card| (matches[0].clone(), card.clone()))
            } else {
                None
            };
            Selection { number, matched }
        })
        .collect())
}
fn candidate(identity: &CardIdentification) -> Option<&CardCandidate> {
    let candidates = &identity.evidence.as_ref()?.candidates;
    if identity.state == IdentificationState::Confirmed {
        candidates
            .iter()
            .find(|c| Some(&c.id) == identity.selected_card_id.as_ref())
    } else if identity.state == IdentificationState::ReviewRequired && candidates.len() == 1 {
        candidates.first()
    } else {
        None
    }
}
