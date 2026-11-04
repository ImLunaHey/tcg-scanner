# Send to Store

Completed duplex card batches can be sent from the web batch toolbar to Store's
private **Scan Inbox**. The button changes to progress, then **Resync to Store**
and provides **Review in Store**. This does not create sellable inventory or
approve any card. Store requires an owner/admin session for approval, and the
scanner client has no approval method. Originals and prepared files stay on the
scanner's configured storage root.

## Connection

ScannerService reads optional private configuration at
`~/Library/Application Support/Scanner/store/config.json`:

```json
{
  "baseURL": "https://lake.tail3275e2.ts.net:5189",
  "scannerInstanceID": "a-stable-installation-uuid",
  "tokenFile": "/absolute/private/path/to/store/token"
}
```

Use an actual generated UUID once and preserve it in backups alongside the
configuration and scanner database. Never regenerate it on ordinary updates or
retries. The directory should have mode 0700, and both files mode 0600, owned by
the service user. Symlinks and publicly readable files are rejected. The token
is loaded only by ScannerService, never returned through the local or HTTP API,
embedded in frontend assets or recorded in the transfer ledger. Invalid or missing
configuration disables the integration while scanning remains available. Restart
the service after changing connection settings.

Only HTTPS origins without credentials, paths, queries or fragments are allowed.
The Swift URLSession client disables cookies, caching and all redirects, bounds
response sizes/timeouts, and uses normal certificate verification. No subprocess
or shell command is part of the production integration. Store error response
bodies are not relayed; they could contain reflected secrets.

## Data and retries

- `sourceBatchId` is the immutable scanner job UUID; `batchNumber` is its exact
  `B-023` label. `cardNumber` is the physical pair number, e.g. `C-001`, not a raw
  capture sequence. These stay distinct from Store's own IDs and purchase batches.
- Two consecutive captures form each card. The single recognized front or a
  uniquely detected Pokémon back determines the side order. Ambiguous/missing
  sides stop sending with a specific error rather than inventing labels.
- Each side must have an upright prepared PNG within Store's documented 10 MiB
  and 100–8000 pixel limits. Stale/in-progress checks must be refreshed first.
- Recognition names, set names and collector numbers are suggestions. OCR text
  confidence is not presented as identity confidence. Finish is an estimate;
  condition remains unknown. Back-border findings are not a selling grade.
  A current single GBP price, when available, is sent as a suggestion only.
- Each accepted transfer freezes its source metadata and image hashes in SQLite.
  Uploads verify source checksums and Store's returned hashes. An interrupted
  transfer retries the same frozen target. An explicit resync after completion
  takes the latest prepared images, recognition, finish, findings and valuation.
  Batch/card IDs stay the same; identical photos are not uploaded again.
- Per-card baselines record the last acknowledged scanner values. Pending cards
  can be updated only when remote fields and photos match those values or the
  new target (including recovery after a lost response). Writes use Store's
  optimistic revision. Reviewer notes, confirmed values, manual metadata,
  changed suggestions/photos, and approved or rejected cards are preserved.
  Preserved card labels are returned and counted separately; other cards continue.
  No force-overwrite, approval or publication operation is exposed.
- One transfer runs at a time independently of hardware ownership. Progress is
  persistent and produces `store.transfer.updated` events. Shutdown/sleep cancel
  the worker; interrupted transfers become failed with an explicit safe retry.
  No startup scan or automatic retry is triggered. Partial staged batches remain
  private and can be completed by **Retry sync**.
- Resync is explicit. Reloading the page or editing scanner results does not
  initiate a transfer. A duplicate request while sending returns current progress;
  another request after completion starts a fresh resync.

## API

All clients use ScannerService, not Store credentials or hardware directly:

- `GET /api/v1/batches/{jobUUID}/store` → transfer status.
- `POST /api/v1/batches/{jobUUID}/store` with an empty body → send, retry or resync; 202.
- Local JSON-RPC: `v1.batches.storeStatus` / `v1.batches.sendToStore`, each with
  `{"id":"jobUUID"}`. Typed equivalents are available in `ScannerClient`.

Status includes `configured`, `state` (`notSent`, `sending`, `sent`, `failed`),
`sentCards`, `totalCards`, optional `preservedCards` (labels), `reviewURL`, error
and update timestamp. `sentCards` counts synced cards; preserved cards complete
progress without being included in that count. A completed operation is `sent`
even when some or all cards were preserved.
The gateway requires existing scanner authentication; no client-supplied remote
URL, credential or file path is accepted. Review links must remain on the
configured Store HTTPS origin.

Store contracts: [OpenAPI](https://lake.tail3275e2.ts.net:5189/api/v1/openapi.json).
`POST /api/v1/admin/scan-batches`, revision-checked
`PUT /api/v1/admin/scan-cards/{id}` and
`POST /api/v1/admin/scan-cards/{id}/photos` are used for writes. Approval and
publication are intentionally performed in Store.

## Live validation — B-023, 2026-11-01

Validated against the deployed Store OpenAPI contract, installed the signed
Swift service/HTTP gateway and web assets, and clicked **Send to Store** on B-023
through the authenticated Tailscale dashboard. Store created one Scan Inbox batch
(`01a19783-752e-760e-8b5e-3aea1e478978`) containing C-001 Claydol, C-002 Deoxys
and C-003 Ferrothorn. All three retained their exact scanner labels, source batch
UUID and scanner instance identity. All six staged PNG hashes and byte counts
matched the correct latest prepared front/back copies.

Every card remained `pending`, with no confirmed values or inventory ID. Anonymous
requests for all six private previews were rejected. Inventory and public-product
counts stayed at zero. Repeating the scanner send command returned its saved
receipt without creating another card or uploading photos again. The entire scan
record and every original/prepared file hash remained unchanged.

The production browser check confirmed the button, completed status, same-origin
Store review link, desktop and 390px mobile layouts, and persistence after reload.
Only one browser send request occurred; browser traffic never contacted Store
with the credential. Service status returned idle/unpaused. No physical scan or
Store approval/publication was performed.

Validation: 140 Swift tests and 27 web tests passed, along with release builds and
signature verification. Regression coverage includes lost upload responses,
unchanged pair numbers, swapped capture sides, unknown sides, shutdown/restart,
private credential files, hostile review links, upstream error redaction,
reviewed-photo conflicts and gateway authentication/input boundaries. Local
runtime evidence stays ignored under `Diagnostics/store-integration/`; deployment
backups use `store-integration-` and the corresponding versioned web backup.

## Orientation errors and metadata contract — 2026-11-01

B-016/C-011 failed the orientation gate, not the upload size limit. Its back
(capture 22) was an inverted prepared PNG with unverified orientation: OCR read
`PaKCMOy`, outside the bounded logo matcher. The preparer now verifies logo OCR
again in the orientation proposed by the independent back colour/layout detector.
Both checks remain required. Export errors distinguish missing preparation,
unverified orientation, non-PNG output and actual size limits, naming the capture.

The updated Store contract requires card-level game/language metadata. New
transfers require a detected Pokémon back for every pair and include
`game:pokemon`, `gameSource:recognition`, the configured physical-card language
(`en` or `ko`), and `languageSource:scannerConfiguration`. A Pokémon pair can
be exported without a catalogue identity; unknown games and Yu-Gi-Oh! pairs
are rejected before any transfer. Suggestions include `provider:tcgdex` and
the exact catalogue candidate's `externalId`. Language remains configured,
while game is detected per pair.
Persisted transfer receipts remain readable; a retry upgrades only its frozen
local submission, never replacing Store reviewer changes. No older wire client
contract is supported.

Deployed the signed service and reran preparation/identification for B-016 capture
22. The software verified a two-quarter-turn correction using back-pattern and
logo evidence. All 44 latest prepared sides have verified orientation and matching
identification source hashes; all original file hashes remain unchanged. The
existing scan-boundary warning remains because preparation cannot recover pixels
absent from an original.

Retried B-016 successfully: Store batch `01a197b1-0958-70a9-86e7-048b8d51acf1`
contains 22 pending cards, no approvals or inventory IDs. All 44 uploaded PNG
hashes and sizes match the correct prepared sides. Every card includes canonical
game/language metadata, and recognition suggestions include provider/external ID.
C-011 authenticated previews both return images; anonymous requests are rejected.
Store nevertheless returns `photosReady:false` for C-011 in both batch and card
detail responses. This upstream readiness flag remains unexplained; successful
transfer and private preview access do not establish Store approval readiness.

Validation: 142 Swift tests passed, including a saved-image orientation regression,
and the release build succeeded. Installed with the persistent signing identity.
Runtime evidence is ignored under `Diagnostics/store-c011/`; no physical scan,
Store approval or publication was performed.

## Condition-note export — 2026-11-01

Store uses `review.borderFindings` to prefill condition notes. New exports now
include only distinct mark observations from a completed back condition check.
Scan-boundary/preparation warnings, check limitations and the no-marks summary
are not card condition descriptions and are excluded. Empty findings leave
condition notes empty; no condition grade is inferred. Scan warnings remain in
the scanner record. Inconclusive checks do not supply condition-note findings.

Validated with 143 passing Swift tests and a release build; installed the signed
service and verified it restarted idle. Removed the two exact generated warning/
summary strings from all 22 untouched B-016 pending Store drafts using optimistic
revision checks. Preserved photos, recognition, price, finish, metadata and review
state; C-011's computed condition notes are now empty. No approval or publication
was performed. Before/after evidence is ignored under `Diagnostics/store-c011/`.

## Resync validation — 2026-11-01

Installed the signed service and web update. Clicked **Resync to Store** on B-023
through the authenticated Tailscale dashboard. All three existing pending cards
received current recognition IDs, Pokémon/English metadata and scan-note cleanup.
Their card IDs and all six photo IDs, hashes and sizes stayed unchanged; no
duplicate cards or image uploads were created. A second explicit resync produced
no Store changes, including revisions. Scanner records and original hashes were
unchanged. No hardware scan, approval or publication occurred.

The browser check verified the enabled resync action after completion, progress,
Store link, desktop and 390px mobile layouts, and reload without a write request.
The browser contacted only ScannerService for syncing, never Store credentials.
Runtime evidence is ignored under `Diagnostics/store-resync/`.

147 Swift tests and 28 web tests passed; Swift release and web builds succeeded.
Regression tests cover changed prepared images and metadata, frozen retries after
a lost update response across restart, unchanged-photo reuse, revision conflicts,
continued preservation after repeated resync, reviewed/approved/rejected/manual
values, and explicit revision-checked PUT payloads. No Store API change was needed.
