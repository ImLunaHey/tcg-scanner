use scanner_staging::{
    contracts::{CardInput, Import},
    ownership::{ImageIdentity, Pair},
};
use scanner_types::{images::OriginalImage, scan::ScanRecord, staging::TransferStatus};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct PlannedCard {
    pub number: u32,
    pub front: OriginalImage,
    pub back: OriginalImage,
}
impl PlannedCard {
    pub fn pair(&self) -> Pair<'_> {
        fn image(i: &OriginalImage) -> ImageIdentity<'_> {
            ImageIdentity {
                sha256: i.sha256.as_str(),
                bytes: i.bytes,
            }
        }
        Pair {
            front: image(&self.front),
            back: image(&self.back),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Baseline {
    pub input: CardInput,
    pub card: PlannedCard,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Ledger {
    pub status: TransferStatus,
    pub origin: String,
    pub snapshot: ScanRecord,
    pub submission: Import,
    pub cards: Vec<PlannedCard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baselines: Option<Vec<Baseline>>,
}
impl Ledger {
    pub fn validate(&self, id: uuid::Uuid) -> scanner_types::error::Result<()> {
        let count = self.cards.len();
        let mut valid = self.status.schema_version == 1
            && self.status.job_id == id
            && self.snapshot.id == id
            && self.submission.source_batch_id == id
            && self.snapshot.request.request_id == id
            && (1..=50).contains(&count)
            && self.submission.cards.len() == count
            && self.status.total_cards == count
            && self.status.sent_cards <= count
            && self.submission.batch_number == self.snapshot.batch_label();
        for (index, (card, input)) in self.cards.iter().zip(&self.submission.cards).enumerate() {
            valid &= card.number == index as u32 + 1
                && input.card_number == format!("C-{:03}", card.number)
                && card.front.sequence != card.back.sequence
                && card.front.sequence.div_ceil(2) == card.number
                && card.back.sequence.div_ceil(2) == card.number;
        }
        if !valid {
            return Err(scanner_types::error::ScannerError::new(
                "storage",
                "Store receipt identities or card counts are inconsistent.",
            ));
        }
        Ok(())
    }
}
