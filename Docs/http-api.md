# HTTP API v1

`scanner-http` is a Swift 6.2+ [Hummingbird 2.27.1](https://github.com/hummingbird-project/hummingbird/releases/tag/2.27.1) gateway. It communicates exclusively
with ScannerService over the private Unix socket. No HTTP process opens a scanner,
executes a shell command, reads an arbitrary file, or schedules hardware itself.
SwiftUI and web applications are not needed for headless operation. The optional
[browser UI](web-app.md) is served by this gateway using `--web-root`.

## Installation

Update the service first to obtain the thumbnail method, then install the gateway:

```sh
Scripts/uninstall-launch-agent.sh
Scripts/install-launch-agent.sh
Scripts/install-http-agent.sh
```

Updating an existing gateway requires `Scripts/uninstall-http-agent.sh` first.
The default is **127.0.0.1:8170**, with authentication required on every API route.
When web assets are configured, only the static login shell/assets are public.
The HTTP LaunchAgent is `local.scanner.http`; the executable is installed beside
`scanner`, under `~/Library/Application Support/Scanner/bin/`. Automatic startup
runs in the logged-in user session. ScannerService must also be running.

For a foreground development gateway:

```sh
swift build --product scanner-http
.build/debug/scanner-http --init-token  # only once; refuses an existing file
.build/debug/scanner-http
```

The generated root credential is 32 random bytes encoded as 64 lowercase hex
characters. It is stored in `~/Library/Application Support/Scanner/http/token`,
mode 0600, in a private 0700 directory. Initialization never prints it. The process
refuses an unsafe token file or missing credential. `--token-file` and `--socket`
accept alternate private paths. No credentials belong in Git, URLs, logs or plist
arguments. Protect the root credential as scanner-control access.

## Authentication and sessions

Every API request needs `Authorization: Bearer TOKEN`. Cookies and URL tokens are not
accepted. The root credential can call APIs directly or create an expiring session:

| Route | Behavior |
| --- | --- |
| `POST /api/v1/session` | Root credential required. Returns a random bearer `token`, `expiresAt` and schemaVersion; expires after one hour. |
| `DELETE /api/v1/session` | Revoke the current session token. Root tokens cannot be revoked through this endpoint. |

Session tokens have the same scanner-control privileges as the root token but
cannot create further sessions. Sessions are kept hashed in memory, capped at 64,
and invalidated by gateway restart. To rotate the root, stop the gateway, move the
old credential to a private backup or remove it deliberately, initialize a new one,
and restart. All clients must then authenticate again. The root has no automatic
expiry. There are no user accounts or separate read/write roles in this milestone.

Browser clients should obtain a session over HTTPS or the configured encrypted
Tailscale connection. The shipped UI keeps its expiring token in tab sessionStorage
to support reloads; it never stores the root credential. Clients
send the Authorization header on fetch requests with `credentials: "omit"`. No cookie session means
state-changing requests do not rely on ambient browser credentials. The server
also rejects mismatched Origin and Host/authority values. CORS is not enabled;
use one origin for the UI and API. Forwarded headers and Tailscale identity
headers never bypass authentication or change the configured origin.

A local diagnostic request can read the token without putting it in process
arguments, command history or output (development example; not runtime code):

```sh
python3 - <<'PY'
from pathlib import Path
from urllib.request import Request, urlopen
credential = (Path.home() / 'Library/Application Support/Scanner/http/token').read_text().strip()
request = Request('http://127.0.0.1:8170/api/v1/devices', headers={'Authorization': 'Bearer ' + credential})
print(urlopen(request).read().decode())
PY
```

## Routes and contracts

JSON responses use the shared Codable [local API schemas](local-api.md). Successful
results are direct objects, rather than JSON-RPC envelopes. Timestamps use UTC ISO
8601. Unknown fields, invalid identifiers, duplicate query parameters and oversized
requests are rejected. All HTTP profile IDs are UUIDs; resolve names through the list.

| Method and route | Input / response |
| --- | --- |
| `GET /api/v1/devices` | DeviceList |
| `GET /api/v1/devices/{id}` | DeviceStatus, including observed readiness |
| `GET /api/v1/devices/{id}/capabilities` | Inspection |
| `GET /api/v1/devices/{id}/settings` | SettingsReport |
| `PATCH /api/v1/devices/{id}/settings` | Complete `dpi`, `duplex`, `format` object; validates/persists defaults without feeding. Jobs use these only with `useDeviceSettings: true`. |
| `GET /api/v1/profiles` | Profile list |
| `POST /api/v1/profiles` | Required stable UUID `id` and `name`; optional dpi/duplex/format default 300/false/png. Returns 201. |
| `GET /api/v1/profiles/{id}` | ScanProfile |
| `PATCH /api/v1/profiles/{id}` | At least one of name/dpi/duplex/format; retains omitted fields |
| `DELETE /api/v1/profiles/{id}` | Delete profile, preserving existing job snapshots |
| `GET /api/v1/jobs` | JobPage; optional limit 1–100, offset 0–1000000 |
| `POST /api/v1/jobs` | Scan request described below; returns 202 and queued/current ScanRecord |
| `GET /api/v1/jobs/{id}` | ScanRecord |
| `POST /api/v1/jobs/{id}/cancel` | Empty body; current ScanRecord; poll until terminal |
| `GET /api/v1/batches` | Same pagination as jobs; one batch per job |
| `GET /api/v1/batches/{id}` | ScanRecord/manifest |
| `GET /api/v1/batches/{id}/manifest` | JSON manifest with attachment disposition |
| `GET /api/v1/images/{jobUUID}:{sequence}` | Original image bytes, download disposition, content type, ETag and X-Image-SHA256 |
| `GET /api/v1/images/{jobUUID}:{sequence}/metadata` | ImageMetadata |
| `GET /api/v1/images/{jobUUID}:{sequence}/thumbnail` | Service-generated PNG, maximum 512 pixels on longest side |
| `POST /api/v1/images/{jobUUID}:{sequence}/edits` | `id` UUID, `quarterTurns` 0–3, and normalized `crop` (`x`, `y`, `width`, `height`). Optional `deskewDegrees` (clockwise, −12…12) expands/rotates the original canvas before that crop; quarter turns apply afterward. Creates a separate PNG via ScannerService, preserving the original. |
| `POST /api/v1/images/{jobUUID}:{sequence}/prepare-card` | Stable `id` UUID; creates a cropped/oriented PNG copy. Returns 422 when no unambiguous card is found. See [preparation contracts](card-preparation.md). |
| `GET /api/v1/events` | Server-Sent Events; see below |
| `GET /api/v1/diagnostics` | Diagnostics; optional deviceID query |
| `GET /api/v1/status` | ServiceStatus |

Image IDs combine the persisted job UUID and sequence 1–100; they are stable across
restarts and do not require renaming originals or a new metadata table. Sequence
preserves callback order; physical front/back identity remains unknown.
The three image GET routes accept an optional `editID` UUID query parameter to
retrieve a retained manual copy belonging to that job and source sequence.
Omitting it always selects the original. Editing is idempotent by the supplied
edit UUID and delegates validation/processing to ScannerService.

## Creating jobs safely

Only POST jobs requests physical feeding. Discovery, diagnostics, settings and
thumbnail endpoints never feed paper. Confirm loaded appropriate media before
starting. The current validated scope is A4 paper; card safety is not established.

Send `Content-Type: application/json` and a stable `requestID` UUID, or the same UUID
in `Idempotency-Key`. If both are present they must match. No ID is generated on
behalf of a remote caller. Retry the same ID and input after a lost response. A
new UUID explicitly requests another scan. Conflicting reused IDs return 409.

```json
{
  "requestID": "de51f702-8a9c-4784-b98c-d3c5c04b435a",
  "duplex": true,
  "format": "png",
  "dpi": 300
}
```

Optional `deviceID`, `profileID` UUID, and `useDeviceSettings` are accepted. Choose
either a profile or device defaults; omitted explicit settings are 300 DPI, simplex,
PNG. The service validates capabilities, snapshots settings and owns the queue.
Remote requests cannot choose an output path; scans use the configured service
storage root. Profile creation requires a stable caller-supplied UUID for the same
retry reason. Changing a profile never changes a queued job's snapshot.

A successful 202 response means accepted, not scanned. Poll job state or subscribe
to events. A failed/cancelled job can still contain retained images. Cancelling an
active job is best-effort; polling must continue until terminal. Disconnecting HTTP
or restarting the gateway does not cancel jobs. Gateway retries never resume
interrupted physical feeding automatically.

## Images and events

Original downloads stream service-provided chunks, with per-chunk and final SHA-256
verification. No arbitrary path is accepted. Source path/symlink/size checks stay
inside ScannerCore. Compare the downloaded file hash with X-Image-SHA256. If an
error occurs after headers, the download is terminated; discard incomplete downloads
and retry the same read. Range requests return 416; downloads currently restart
from the beginning. Originals remain unmodified.

Thumbnails run off the scanner actor through ImageIO, with two workers maximum,
32 MiB/100-megapixel source limits and a 2 MiB output limit. Larger originals remain
downloadable. Thumbnails are generated on demand without a persistent cache.

Events use fetch streaming with an Authorization header (native EventSource cannot
set that header). Responses have `text/event-stream`, no-store and buffering disabled.
The stream emits:

- `event: scanner`, `id: CURSOR`, JSON data containing ServiceEvent (kind, timestamp,
  payload). Kinds include job.updated and device connected/disconnected events.
- Heartbeat comment lines while idle.
- `event: reset` if the cursor expired or the service database changed. Refresh
  snapshots and reconnect from after=0.
- `event: error` for a later service/authentication failure. Reauthenticate or retry
  according to the error; preserve the last delivered cursor.

Pass `Last-Event-ID` or `?after=CURSOR`, not both. Connections end after about 25
seconds (optional `seconds=1…25`); reconnect with the last received event ID.
Events are replayable, so deduplicate by cursor. Session revocation/expiry is checked
while streaming. A service interruption does not lose persisted history.

## Errors and resource limits

Gateway errors return `{ "schemaVersion": 1, "error": { "kind": "…", "message": "…" } }`.
HTTP status: 400 malformed input, 401 missing/expired authentication, 403 access/origin
rejection, 404 missing record/route, 409 busy/conflict, 413 body too large, 415 unsupported
content type/encoding, 416 unsupported Range, 422 invalid/unsupported settings,
429 rate/resource limit, 503 unavailable service/storage. Unexpected failures use 500.
Partial inspections can return 200 with errors in their diagnostic payload.
Malformed HTTP rejected by the framework before routing may have an empty response.

Limits per gateway process: 32 connections, 16 active responses, 4 SSE streams,
2 image responses, 600 authenticated requests/minute and 30 invalid authentication
attempts/minute. Rate limits are shared across clients; 429 includes Retry-After.
Requests allow 60 KiB bodies with a five-second body deadline, 16 KiB total headers,
32 headers, 8 KiB header fields/request target, and a 30-second idle connection timeout.
Image transfers have a five-minute processing deadline checked between chunks. The service independently
limits its queue, IPC connections and original file sizes. Authentication checks
precede body decoding and IPC. A dropped response releases its admission slot.

These are defensive bounds for authenticated local/tailnet control, not a claim of
public-internet hardening. No public bind or wildcard address is accepted.

## Card identification extension

Authenticated asynchronous identification and manual review are available at
`/api/v1/images/{jobID}:{sequence}/identification` using GET, POST and PATCH.
See [contracts, states and limitations](card-identification.md). Identification
operates on saved images (preferring prepared card copies) and never starts a
physical scan.

Scan requests also accept optional `prepareCards: true` to prepare captured images
during processing. Defaults to false; feeder configuration is unaffected.

## This installation's HTTPS entry point

The current dashboard/API origin is `https://pulsar.tail3275e2.ts.net:8443`.
Tailscale Serve proxies it privately to the gateway at `127.0.0.1:8170`, with
that exact HTTPS value configured as `--public-origin`. Authentication remains
required and forwarded headers do not grant access. See [deployment and rollback
notes](web-app.md). The installation defaults above still apply to new setups.


Finish estimates: `POST /api/v1/images/{jobUUID}:{sequence}/finish-estimate` accepts
`identificationID` and returns the current `CardIdentification` with an optional
`finishEstimate` (`state`, optional `suggestedFinish`, `candidateID`, method,
evidence and limitations). This authenticated operation analyzes the original
identification's saved source locally and never feeds hardware or changes reviewed
finish/condition fields. New identifications compute the estimate automatically.
See [finish estimate scope and validation](local-visual-assessment.md).

Visible condition checks: `POST /api/v1/images/{jobUUID}:{sequence}/condition-check`
accepts `identificationID` and returns the current `CardIdentification` with
`conditionCheck`. The authenticated operation analyzes the saved source locally,
including no-match backs, and preserves manual finish/condition reviews. Returned
regions are candidate marks, not verified damage or a grade. See
[scope, schema and validation](condition-checks.md).
