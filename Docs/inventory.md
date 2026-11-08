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

## Visual browsing

Inventory entries include a representative `preview` with batch ID, image metadata
and the prepared edit ID when available. The service chooses the detected front or
identity-bearing side, preserving the original capture sequence. Unknown pairs
use their first capture without claiming it is a front. The web gallery loads
cached JPEG thumbnails near the viewport; full images load only
when opening the existing lightbox.

The `sets` array covers the complete inventory regardless of entry pagination or
search. Each set has its game, total copies, entry count and up to three cover
previews. The optional `set` query/RPC parameter filters by the exact set facet ID
(up to 512 bytes), and combines with search. Set tiles open the corresponding
card gallery. Quantities and grouping rules remain unchanged.

Set covers use prepared images only and prefer portrait cards when available.
Landscape covers retain their own aspect ratio. If a set has no prepared images,
its tile shows a neutral placeholder; full overscan canvases are still available
in the card gallery but are not used as decorative set covers.

The gallery displays every matching card without page controls. It automatically
fetches all metadata pages (100 entries per request), while thumbnails remain lazy
and use the existing bounded request queue. The Sets view always shows all sets.

Navigation is URL-driven: `/inventory` shows cards, `/inventory/sets` shows sets,
and `/inventory/sets/{set-id}` shows cards in a selected set. Search is stored in
`?search=...`, so direct links, refresh and browser history retain the selection.
Set IDs in the API and URLs are readable name slugs, such as `ascended-heroes`
and `delta-reign`. Internal catalogue keys still keep distinct sets separate.
Colliding slugs receive a game suffix and, if needed, a number.
Batch location links accept `?card=N` on direct navigation too.

## Missing cards

A set's **Show missing cards** checkbox (`?missing=1`) requests
`GET /api/v1/inventory/missing?set={slug}` (`v1.inventory.missing`). The service
subtracts known catalogue identities from the provider's set checklist. Rescans,
search filters and page boundaries do not create false gaps. Missing cards appear
in collector-number order with greyscale artwork and never contribute stock.
Ambiguous identities cannot establish ownership of a particular catalogue card.
Sets without catalogue data show an unavailable message rather than a complete
collection. The checklist uses the set's scanned language, preferring English when
both supported languages are represented; foil finishes are not separate gaps.

Catalogue images use token-free `/api/v1/inventory/references/{key}/thumbnail`
requests (`v1.inventory.reference`). ScannerService registers fixed-host catalogue
URLs, downloads bounded images without redirects and caches 512px JPEGs locally
(up to 256 MiB, seven days). No scans or OCR are uploaded. Thumbnails load near the
viewport through the same bounded client queue as scan previews. Provider sources:
[TCGdex sets](https://tcgdex.dev/rest/set), [SWU DB](https://www.swu-db.com/api) and
[YGOPRODeck](https://ygoprodeck.com/api-guide/).

Sets are ordered newest release first, with unknown dates last and names breaking
ties. The API adds `releaseDate` (ISO date) to set facets and
`releaseDatesPending` to inventory pages. Provider metadata refreshes in the
background with at most three concurrent requests; inventory does not wait for
network calls. Dates persist locally for a day, with failed requests retried after
five minutes. Date order uses the represented catalogue language/region.
