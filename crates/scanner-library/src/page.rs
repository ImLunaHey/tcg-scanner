use crate::{Inventory, names::natural};
use scanner_types::{
    error::{Result, ScannerError},
    inventory::{InventoryEntry, InventoryPage, InventorySet},
};
use std::collections::BTreeMap;
impl Inventory {
    pub fn page(
        &self,
        limit: usize,
        offset: usize,
        search: &str,
        set: &str,
    ) -> Result<InventoryPage> {
        if !(1..=100).contains(&limit)
            || offset > 1_000_000
            || search.len() > 160
            || set.len() > 512
        {
            return Err(ScannerError::new(
                "invalidParams",
                "Invalid inventory pagination or filters.",
            ));
        }
        let slugs = self.slugs();
        let all: Vec<_> = self
            .entries
            .values()
            .map(|e| {
                let mut e = e.clone();
                e.set_key = slugs[&e.set_key].clone();
                e
            })
            .collect();
        let query = search.trim().to_lowercase();
        let mut filtered: Vec<_> = all
            .iter()
            .filter(|e| {
                (set.is_empty() || e.set_key == set)
                    && (query.is_empty()
                        || format!(
                            "{} {} {} {} {}",
                            e.name,
                            e.set_name.as_deref().unwrap_or(""),
                            e.number.as_deref().unwrap_or(""),
                            e.game.as_str(),
                            e.language
                        )
                        .to_lowercase()
                        .contains(&query))
            })
            .cloned()
            .collect();
        filtered.sort_by(|a, b| {
            b.quantity
                .cmp(&a.quantity)
                .then_with(|| natural(&a.name, &b.name))
                .then(a.id.cmp(&b.id))
        });
        let matching_entries = filtered.len();
        let count = |status| {
            all.iter()
                .filter(|e| e.status == status)
                .map(|e| e.quantity)
                .sum()
        };
        let sets = sets(&all);
        Ok(InventoryPage {
            schema_version: 1,
            sets,
            release_dates_pending: false,
            entries: filtered.into_iter().skip(offset).take(limit).collect(),
            total_copies: all.iter().map(|e| e.quantity).sum(),
            catalogue_copies: count("catalogue"),
            awaiting_catalogue_copies: count("awaitingCatalogue"),
            unresolved_copies: count("unresolved"),
            excluded_code_cards: self.excluded,
            matching_entries,
            next_offset: (offset + limit < matching_entries).then_some(offset + limit),
        })
    }
}
fn sets(all: &[InventoryEntry]) -> Vec<InventorySet> {
    let mut grouped: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    for entry in all {
        grouped.entry(&entry.set_key).or_default().push(entry);
    }
    let mut result: Vec<_> = grouped
        .into_iter()
        .map(|(id, mut entries)| {
            entries.sort_by(|a, b| a.id.cmp(&b.id));
            let mut previews: Vec<_> = entries
                .iter()
                .filter_map(|e| e.preview.clone())
                .filter(|p| p.edit_id.is_some())
                .collect();
            previews.sort_by(|a, b| {
                (a.image.width > a.image.height)
                    .cmp(&(b.image.width > b.image.height))
                    .then(a.batch_id.cmp(&b.batch_id))
                    .then(a.image.sequence.cmp(&b.image.sequence))
            });
            previews.truncate(3);
            InventorySet {
                id: id.into(),
                name: entries[0].set_name.clone().unwrap_or("Unidentified".into()),
                game: entries[0].game,
                quantity: entries.iter().map(|e| e.quantity).sum(),
                entries: entries.len(),
                release_date: None,
                previews,
            }
        })
        .collect();
    result.sort_by(|a, b| natural(&a.name, &b.name).then(a.id.cmp(&b.id)));
    result
}
