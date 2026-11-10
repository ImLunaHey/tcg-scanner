use scanner_types::{
    error::{Result, ScannerError},
    game_detection::CardGameDetection,
    inventory::{
        InventoryCatalogueCard, InventoryCatalogueSource, InventoryEntry, InventoryMissingCards,
    },
    scan::ScanRecord,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub struct Inventory {
    pub(crate) entries: BTreeMap<String, InventoryEntry>,
    pub(crate) sources: BTreeMap<String, BTreeSet<InventoryCatalogueSource>>,
    pub(crate) owned: BTreeMap<InventoryCatalogueSource, BTreeSet<String>>,
    pub(crate) excluded: usize,
    copies: usize,
}
impl Inventory {
    /// Add one persisted job once, in newest-first ledger order. Rescans in
    /// distinct batches remain additional copies; edits and retries do not.
    pub fn add(&mut self, record: &ScanRecord) -> Result<()> {
        if !record.state.terminal()
            || !(record.request.prepare_cards == Some(true)
                || record
                    .processed_images
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .any(|e| e.card_preparation.is_some()))
        {
            return Ok(());
        }
        let identities = record.identifications()?;
        let detections = CardGameDetection::resolve(record);
        let mut groups: BTreeMap<u32, Vec<_>> = BTreeMap::new();
        for image in &record.images {
            groups
                .entry(if record.request.duplex {
                    image.sequence.div_ceil(2)
                } else {
                    image.sequence
                })
                .or_default()
                .push(image);
        }
        for (number, images) in groups {
            if self.copies + self.excluded >= 100_000 {
                return Err(ScannerError::new(
                    "resourceLimit",
                    "Inventory exceeds 100,000 physical copies.",
                ));
            }
            let detection = if record.request.duplex {
                detections.iter().find(|d| d.number == number)
            } else {
                None
            };
            if detection.is_some_and(|d| d.content_kind.as_deref() == Some("pokemonCodeCard")) {
                self.excluded += 1;
                continue;
            }
            let entry = self.entry(record, number, &images, &identities, detection)?;
            self.copies += 1;
            match self.entries.entry(entry.id.clone()) {
                std::collections::btree_map::Entry::Occupied(mut current) => {
                    let current = current.get_mut();
                    current.quantity += entry.quantity;
                    current.confirmed_quantity += entry.confirmed_quantity;
                    current.copies.extend(entry.copies);
                }
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(entry);
                }
            }
        }
        Ok(())
    }
    pub fn catalogue_sources(&self) -> BTreeMap<String, InventoryCatalogueSource> {
        let slugs = self.slugs();
        self.sources
            .iter()
            .filter_map(|(key, sources)| {
                Some((
                    slugs.get(key)?.clone(),
                    sources.iter().min_by_key(|s| s.language.as_str())?.clone(),
                ))
            })
            .collect()
    }
    pub fn catalogue_source(&self, set: &str) -> Result<Option<InventoryCatalogueSource>> {
        let key = self
            .slugs()
            .into_iter()
            .find(|(_, slug)| slug == set)
            .map(|(key, _)| key)
            .ok_or_else(|| ScannerError::new("notFound", "Inventory set not found."))?;
        Ok(self
            .sources
            .get(&key)
            .and_then(|s| s.iter().min_by_key(|s| s.language.as_str()))
            .cloned())
    }
    pub fn missing(
        &self,
        cards: Vec<InventoryCatalogueCard>,
        source: &InventoryCatalogueSource,
    ) -> InventoryMissingCards {
        let count = cards.len();
        let owned = self.owned.get(source);
        InventoryMissingCards {
            schema_version: 1,
            cards: cards
                .into_iter()
                .filter(|c| !owned.is_some_and(|ids| ids.contains(&c.id)))
                .collect(),
            catalogue_count: Some(count),
            note: None,
        }
    }
}
