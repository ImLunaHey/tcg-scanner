# Scan inventory

The web Inventory page reads `GET /api/v1/inventory` through the authenticated
HTTP gateway. The corresponding service method is `v1.inventory.list`.
Both accept `limit` (1–100, default 50), `offset` and optional `search` (160 bytes).

Quantities count saved card captures in terminal batches, including partial failed
batches. Duplex front/back captures count once. Edits and repeated identification
attempts add no copies. Rescans in different batches intentionally count again.
General document batches and detected online redemption/code cards are excluded.

A single catalogue match groups by game, physical language, provider and card ID.
Suggested matches contribute to the count but remain labelled as suggestions;
`confirmedQuantity` counts human confirmations. All finishes and conditions are
combined within a catalogue card. Awaiting-catalogue records group separately by
printed name, set code, number and language. Ambiguous, unidentified and incomplete
pairs remain individual unresolved entries. This is scan inventory, not an
available-to-sell stock ledger; sending a card to Store does not deduct it.

Each entry contains its original batch and card locations. Search and paging apply
to entries; summary quantities describe the whole inventory. Reads use a single
SQLite snapshot and stream records into summaries without opening image files or
requesting prices. Work runs off the coordinator actor, with at most four readers.
Daily catalogue updates regroup previously unresolved printings on the next read.
