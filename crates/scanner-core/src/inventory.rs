use crate::Engine;
use scanner_catalogue::library::LibraryCatalogue;
use scanner_library::Inventory;
use scanner_types::{
    error::{Result, ScannerError},
    image_access::ImageThumbnail,
    inventory::{InventoryMissingCards, InventoryPage},
};
use std::sync::Arc;
impl Engine {
    /// Supply an offline or customized public catalogue before the first library
    /// request. Configuration is fixed once initialized; it never controls hardware.
    pub fn configure_library_catalogue(&self, catalogue: Arc<LibraryCatalogue>) -> Result<()> {
        self.library_catalogue
            .set(catalogue)
            .map_err(|_| ScannerError::new("conflict", "Library catalogue is already configured."))
    }
    async fn catalogue(&self) -> Result<&Arc<LibraryCatalogue>> {
        self.library_catalogue
            .get_or_try_init(|| async { LibraryCatalogue::new() })
            .await
    }
    async fn project_inventory<T: Send + 'static>(
        &self,
        project: impl FnOnce(Inventory) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let permit = self
            .inventory_workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| ScannerError::new("resourceLimit", "Inventory is busy; retry shortly."))?;
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut inventory = Inventory::default();
            database.read_inventory_records(|record| inventory.add(&record))?;
            project(inventory)
        })
        .await
        .map_err(|e| ScannerError::new("inventory", e.to_string()))?
    }
    pub async fn inventory(
        &self,
        limit: usize,
        offset: usize,
        search: &str,
        set: &str,
    ) -> Result<InventoryPage> {
        // Validate cheap bounds before reading the ledger.
        Inventory::default().page(limit, offset, search, set)?;
        let (search, set) = (search.to_owned(), set.to_owned());
        let (mut page, sources) = self
            .project_inventory(move |inventory| {
                Ok((
                    inventory.page(limit, offset, &search, &set)?,
                    inventory.catalogue_sources(),
                ))
            })
            .await?;
        if !sources.is_empty() {
            let dates = self
                .catalogue()
                .await?
                .release_dates(&sources.values().cloned().collect::<Vec<_>>());
            page.release_dates_pending = dates.pending;
            for set in &mut page.sets {
                set.release_date = sources
                    .get(&set.id)
                    .and_then(|s| dates.dates.get(s))
                    .cloned();
            }
        }
        Ok(page)
    }
    pub async fn missing_inventory_cards(&self, set: &str) -> Result<InventoryMissingCards> {
        if set.is_empty() || set.len() > 512 || set.contains('\0') {
            return Err(ScannerError::new("invalidParams", "Invalid inventory set."));
        }
        let set = set.to_owned();
        let (inventory, source) = self
            .project_inventory(move |inventory| {
                let source = inventory.catalogue_source(&set)?;
                Ok((inventory, source))
            })
            .await?;
        let Some(source) = source else {
            return Ok(InventoryMissingCards {
                schema_version: 1,
                cards: vec![],
                catalogue_count: None,
                note: Some("This set has no supported catalogue source.".into()),
            });
        };
        let cards = self.catalogue().await?.cards(&source).await?;
        Ok(inventory.missing(cards.as_ref().clone(), &source))
    }
    pub async fn inventory_reference(&self, key: &str) -> Result<ImageThumbnail> {
        self.catalogue().await?.thumbnail(key).await
    }
}
