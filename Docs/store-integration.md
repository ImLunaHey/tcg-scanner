# Send to Store

Completed duplex card batches can be sent from the web batch toolbar to Store's
private **Scan Inbox**. The button changes to progress, then **Sent for review**
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
- The first accepted transfer freezes its source metadata and image hashes in
  SQLite. Uploads verify source checksums and Store's returned hashes. Retries
  reuse the same scanner/source/card identity and frozen files. Store's existing
  review data is never updated by this integration; different existing photos
  produce a conflict requiring review instead of replacement.
- One transfer runs at a time independently of hardware ownership. Progress is
  persistent and produces `store.transfer.updated` events. Shutdown/sleep cancel
  the worker; interrupted transfers become failed with an explicit safe retry.
  No startup scan or automatic retry is triggered. Partial staged batches remain
  private and can be completed by **Retry sending**.
- A successfully sent batch stays sent. Subsequent scanner edits do not silently
  replace photos or review data already delivered to Store.

## API

All clients use ScannerService, not Store credentials or hardware directly:

- `GET /api/v1/batches/{jobUUID}/store` → transfer status.
- `POST /api/v1/batches/{jobUUID}/store` with an empty body → start/retry; 202.
- Local JSON-RPC: `v1.batches.storeStatus` / `v1.batches.sendToStore`, each with
  `{"id":"jobUUID"}`. Typed equivalents are available in `ScannerClient`.

Status includes `configured`, `state` (`notSent`, `sending`, `sent`, `failed`),
`sentCards`, `totalCards`, optional `reviewURL`, error and update timestamp.
The gateway requires existing scanner authentication; no client-supplied remote
URL, credential or file path is accepted. Review links must remain on the
configured Store HTTPS origin.

Store contracts: [OpenAPI](https://lake.tail3275e2.ts.net:5189/api/v1/openapi.json).
Only `POST /api/v1/admin/scan-batches` and
`POST /api/v1/admin/scan-cards/{id}/photos` are used for writes. Approval and
publication are intentionally performed in Store.
