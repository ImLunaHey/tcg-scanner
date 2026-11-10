# Scanner library

Pure inventory projection from existing `ScanRecord` values. Add terminal records
once in newest-first ledger order, then request `Inventory::page`. Scans in distinct
batches are additional copies; image edits and OCR retries never add stock.

```sh
cargo run -p scanner-library --example saved_inventory -- /path/to/manifest.json
```

The builder preserves confirmed quantities separately from unique unreviewed
suggestions, languages, unresolved/incomplete pairs and excluded code inserts.
Page filters affect matching entries; totals and set summaries cover the whole
library. Preview metadata refers only to saved originals or prepared copies.

`catalogue_sources`, `catalogue_source` and `missing` let a host supply remote
checklists without coupling inventory to networking. Release dates and reference
thumbnails are supplied by that host. The crate never scans, reads files, fetches
URLs or mutates a record. The example reads explicitly supplied manifests.

Aggregation is bounded to 100,000 physical copies. Search is case insensitive;
ordering is deterministic with numeric runs. Set slugs use portable Unicode
transliteration, so non-Latin transliterations may differ from Apple's ICU rules.
Workspace path dependencies are intentional during migration; this internal crate
is not independently published.
