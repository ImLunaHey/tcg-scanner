use super::{
    ledger::{Ledger, PlannedCard},
    observations,
};
use scanner_staging::contracts::Import;
use scanner_types::{
    cards::CardGame,
    error::{Result, ScannerError},
    game_detection::CardGameDetection,
    identification::{CardIdentification, IdentificationState},
    images::{OriginalImage, ProcessedImage},
    pricing::BatchValuation,
    scan::{ScanRecord, ScanState},
    staging::TransferStatus,
};

pub(super) fn validate_editions(record: &ScanRecord) -> Result<()> {
    for identity in record.identifications()? {
        if observations::finish(&identity)?
            .and_then(|e| e.edition)
            .is_some_and(|e| e.needs_edition_handling())
        {
            return Err(not_ready(format!(
                "Capture {} has a detected or uncertain stamp. Store must support stamped-edition metadata before sending this card.",
                identity.request.sequence
            )));
        }
    }
    Ok(())
}
impl Ledger {
    pub fn make(
        record: ScanRecord,
        origin: String,
        instance: String,
        valuation: Option<&BatchValuation>,
    ) -> Result<Self> {
        validate_batch(&record)?;
        validate_editions(&record)?;
        let mut images: Vec<_> = record.images.iter().collect();
        images.sort_by_key(|i| i.sequence);
        if images
            .iter()
            .enumerate()
            .any(|(index, image)| image.sequence != index as u32 + 1)
        {
            return Err(not_ready("Batch capture order is incomplete."));
        }
        let identities = record.identifications()?;
        let games = CardGameDetection::resolve(&record);
        let mut cards = Vec::new();
        let mut inputs = Vec::new();
        for pair in images.as_chunks::<2>().0 {
            let number = pair[0].sequence.div_ceil(2);
            if !games
                .iter()
                .any(|g| g.number == number && g.game == CardGame::Pokemon)
            {
                return Err(ScannerError::new(
                    "unsupportedGame",
                    format!(
                        "C-{number:03} is not a detected Pokemon pair. Store export for other games is not supported yet."
                    ),
                ));
            }
            let edits = [
                prepared(&record, pair[0], &identities)?,
                prepared(&record, pair[1], &identities)?,
            ];
            let identity = edits.map(|edit| {
                identities
                    .iter()
                    .find(|i| i.request.sequence == edit.request.sequence)
            });
            let front = front(&edits, &identity)?;
            let valuation = valuation.filter(|v| v.batch_id == record.id);
            inputs.push(observations::input(
                number,
                record.request.card_language.unwrap_or_default(),
                identity[front],
                identity[1 - front],
                valuation,
            )?);
            cards.push(PlannedCard {
                number,
                front: edits[front].image.clone(),
                back: edits[1 - front].image.clone(),
            });
        }
        let mut status = TransferStatus::new(record.id, true);
        status.total_cards = cards.len();
        let submission = Import {
            scanner_instance_id: instance,
            source_batch_id: record.id,
            batch_number: record.batch_label(),
            cards: inputs,
        };
        Ok(Self {
            status,
            origin,
            snapshot: record,
            submission,
            cards,
            baselines: None,
        })
    }
}
fn prepared<'a>(
    record: &'a ScanRecord,
    original: &OriginalImage,
    identities: &[CardIdentification],
) -> Result<&'a ProcessedImage> {
    let edit = record
        .processed_images
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
        .find(|e| e.request.sequence == original.sequence && e.card_preparation.is_some())
        .ok_or_else(|| not_ready("Every capture requires a prepared image."))?;
    if !edit.card_preparation.as_ref().unwrap().orientation_verified
        || edit.source_sha256 != original.sha256
        || edit.image.uniform_type != "public.png"
        || edit.image.image_count != 1
        || !(1..=10 * 1024 * 1024).contains(&edit.image.bytes)
        || !(100..=8000).contains(&edit.image.width)
        || !(100..=8000).contains(&edit.image.height)
    {
        return Err(not_ready(
            "Prepared PNG must have verified orientation, unchanged source, one frame, at most 10 MiB and dimensions of 100–8000 pixels.",
        ));
    }
    if let Some(identity) = identities
        .iter()
        .find(|i| i.request.sequence == original.sequence)
        && (identity.state == IdentificationState::Processing
            || identity.source_sha256 != edit.image.sha256
            || identity.request.language != record.request.card_language.unwrap_or_default())
    {
        return Err(not_ready(
            "Identification checks are unfinished, outdated or for another language.",
        ));
    }
    Ok(edit)
}
fn front(
    edits: &[&ProcessedImage; 2],
    identity: &[Option<&CardIdentification>; 2],
) -> Result<usize> {
    let matches: Vec<_> = (0..2)
        .filter(|&i| {
            identity[i].is_some_and(|v| {
                matches!(
                    v.state,
                    IdentificationState::Confirmed | IdentificationState::ReviewRequired
                ) && v
                    .evidence
                    .as_ref()
                    .is_some_and(|e| !e.candidates.is_empty())
            })
        })
        .collect();
    let backs: Vec<_> = (0..2)
        .filter(|&i| {
            edits[i]
                .card_preparation
                .as_ref()
                .unwrap()
                .orientation_basis
                .as_deref()
                == Some("pokemon-back-pattern")
        })
        .collect();
    if matches.len() == 1 && !backs.contains(&matches[0]) {
        Ok(matches[0])
    } else if matches.is_empty() && backs.len() == 1 {
        Ok(1 - backs[0])
    } else {
        Err(not_ready(
            "Front/back sides are uncertain. Review the pair before sending.",
        ))
    }
}
pub(super) fn not_ready(message: impl Into<String>) -> ScannerError {
    ScannerError::new("storeNotReady", message)
}

fn validate_batch(record: &ScanRecord) -> Result<()> {
    if record.state != ScanState::Completed
        || !record.request.duplex
        || record.batch_number.is_none()
        || record.images.is_empty()
        || !record.images.len().is_multiple_of(2)
        || record.images.len() > 100
    {
        return Err(not_ready(
            "Send a completed, numbered duplex batch with both sides of every card.",
        ));
    }
    Ok(())
}
