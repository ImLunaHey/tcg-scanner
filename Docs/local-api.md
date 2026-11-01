# Local JSON-RPC API v1

UTF-8 newline-delimited JSON-RPC 2.0 over a private Unix socket. One request/response
per connection; string or numeric request IDs are echoed. Notifications and JSON-RPC
batch envelopes are not implemented. All clients share the service's scheduler.

```json
{"jsonrpc":"2.0","id":"request-1","method":"v1.capabilities","params":{"deviceID":"ID from v1.devices"}}
```

## Methods

| Method | Parameters | Result |
| --- | --- | --- |
| `v1.devices` | none | DeviceList |
| `v1.device.get` | optional `deviceID` | DeviceStatus with readiness and configured defaults |
| `v1.capabilities` | optional `deviceID` | Inspection |
| `v1.settings` | optional `deviceID` | SettingsReport, including optional configuredSettings |
| `v1.settings.set` | `dpi`, `duplex`, `format`, optional `deviceID` | Persisted ScanSettings |
| `v1.diagnostics` | optional `deviceID` | OS, backend, devices, inspection |
| `v1.service.status` | none | ServiceStatus, queue count, active job, pause/error state |
| `v1.scan.start`, `v1.jobs.create` | ScanRequest below | ScanRecord |
| `v1.scan.status`, `v1.scan.cancel` | `scanID` UUID | ScanRecord |
| `v1.jobs.get`, `v1.jobs.cancel` | `id` UUID | ScanRecord |
| `v1.jobs.list`, `v1.batches.list` | optional `limit` 1–100 (50), `offset` 0–1000000 (0) | JobPage with `nextOffset` |
| `v1.batches.get` | `id` UUID | ScanRecord, also the batch manifest schema |
| `v1.profiles.list` | none | `{schemaVersion, profiles}` |
| `v1.profiles.get` | `id` UUID or unique name | ScanProfile |
| `v1.profiles.create` | `id` UUID, `name`, `dpi`, `duplex`, `format` | Created ScanProfile; identical retry returns existing record |
| `v1.profiles.update` | `id` UUID or unique name; at least one of `name`, `dpi`, `duplex`, `format` | Updated ScanProfile |
| `v1.profiles.save` | `id`, `name`, `dpi`, `duplex`, `format` | Saved ScanProfile |
| `v1.profiles.delete` | `id` UUID or unique name | `{deleted: true}` |
| `v1.images.get` | `jobID` UUID, `sequence` 1–100 | ImageMetadata |
| `v1.images.thumbnail` | `jobID` UUID, `sequence` 1–100 | ImageThumbnail: base64 PNG, SHA-256, width, height |
| `v1.images.read` | `jobID`, `sequence`, optional `offset`, `length` | ImageChunk with base64 bytes and checksum |
| `v1.events` | optional `after` (0), `limit` 1–100 (100), `waitSeconds` 0–20 (0) | EventPage |

Unknown keys are rejected. UUIDs must parse; numeric pagination/cursors must be
integers within bounds. Profile names contain 1–128 UTF-8 bytes; at most 256 profiles
are stored. Names must be unique ignoring case. Save replaces a profile by ID;
create rejects conflicting IDs/names, and update changes only supplied fields.
Null fields are rejected. Deleted/edited profiles do not mutate
existing jobs. Profile hardware compatibility is checked when preparing a scan.

Capabilities/settings/diagnostics may open an exclusive session and return `busy`
while scanning. Device listing, service health, history and events remain usable.
Without `deviceID`, inspection/start requires exactly one connected scanner.
Diagnostic methods do not feed paper or write settings. Unknown capabilities and
vendor enum/label discrepancies remain explicit in reports.

## Job creation and persistence

```json
{
  "requestID": "b627ee02-f78a-4dbe-b2d0-3f3b33cfa8a0",
  "dpi": 300,
  "duplex": false,
  "format": "png"
}
```

Optional: `deviceID`, `outputDirectory`, `profileID` UUID, `useDeviceSettings` boolean. The output path must be inside
the configured storage root and contain no parent traversal or symlink escape.
A profile supplies DPI/duplex/format, overriding those request values. The exact
original submission is retained for idempotency; the record's `request` contains
the effective settings and `profile` contains the snapshot. `useDeviceSettings: true`
uses saved per-device defaults and is mutually exclusive with `profileID`. It fails
if no defaults have been saved. Later defaults changes do not change existing jobs.

`requestID` is both job ID and batch ID. Repeating the identical original submission
returns its persisted record, even after restart or deletion of its profile.
Changing any input with the same ID returns `conflict`. JSON-RPC envelope IDs are
only correlation IDs; they do not deduplicate physical scans.

Creation commits a queued job before scheduling hardware. At most 100 jobs wait;
one hardware session runs at a time with the current backend. A successful create
response does not mean a scan succeeded. The CLI waits/polls unless `--no-wait`.

States are `queued`, `preparing`, `scanning`, `processing`, `completed`, `failed`,
`cancelled`. Started/completed timestamps are optional until applicable. Detailed
records include images, errors, warnings, settings, device and output location.
JobSummary exposes scanner/profile/batch IDs and progress (`imagesSaved`;
`totalImages` is unknown). Images preserve callback order and `side: unknown`.
Processing currently finalizes storage; it does not crop or deskew.

Cancellation of a queued job prevents capture. Active cancellation returns the
current state; poll until terminal. Originals remain on disk. Driver failures
retain domain/code details. A partial capture can fail while retaining good images.

SIGTERM/SIGINT stop admission, cancel outstanding jobs and allow bounded cleanup.
Sleep cancels outstanding jobs; wake does not replay them. Restart reconciles newer
manifests and marks unfinished jobs interrupted. No automatic scan is triggered by
startup, diagnostics, discovery or hardware-button events. Unconfirmed hardware
closure and storage errors stop further scheduling.

## Readiness and saved defaults

`v1.device.get` reports `ready: true/false` when known; an omitted readiness value
means unknown. Busy hardware is reported as not ready without opening another
session. Readiness is a momentary observation and cannot reserve the feeder.

`v1.settings.set` validates DPI, duplex and format against the idle scanner and
persists defaults; it does not immediately write hardware configuration. It rejects
queued/active jobs. Defaults are applied only to jobs with `useDeviceSettings: true`.
`configuredSettings` is separate from currently observed driver properties.
Vendor, multifeed, paper-thickness and transport safety controls cannot be written
through this endpoint. GRDB migration `v2-device-settings` retains prior history.

## Original image retrieval

Image metadata includes the job ID and existing original image record. Reads accept
only a job UUID and sequence, never a filename. `offset` is 0–536870912 bytes;
`length` is 1–262144, default 65536. The result includes metadata, offset, nextOffset,
eof, base64 and chunkSHA256. Decode and verify each chunk, concatenate until eof,
and verify the original image's complete SHA-256 and byte count.

Files must remain within the configured storage root. Descriptor-relative opens
reject symlink components, non-regular files, hard-linked files and size mismatches.
Reads run outside the cooperative executor. Historical files under a different
storage root cannot be exported by changing a request path. Original bytes remain
unchanged. The CLI publishes a verified download at the explicitly selected local
destination, without overwriting an existing file.

Thumbnails are generated by the service using ImageIO, at most 512 pixels on the
longest side. The source must pass the same path and SHA-256 checks as originals.
Limits: two concurrent workers, 32 MiB source, 100 megapixels and 2 MiB result.
Larger originals remain downloadable. Thumbnails are generated on demand without
modifying originals or opening hardware.

## Event subscriptions

`v1.events` provides cursor-based long polling. Use `nextCursor` for the next call;
`waitSeconds: 20` waits asynchronously when caught up. A heartbeat is an empty page.
Up to eight callers may wait, leaving other server slots available for controls.
Disconnecting a subscriber cannot cancel a scan; its wait expires within 20 seconds.

Event kinds: `settings.updated`, `job.updated`, `profile.updated`, `profile.deleted`, `device.connected`,
`device.disconnected`, `service.started`, `service.stopped`, `service.sleep`,
`service.wake`. Job payloads are JobSummary values; start payloads include the
recovered-job count. Device presence is sampled roughly every 2.5 seconds, so very
brief disconnect/reconnect cycles may be missed. Driver session errors still reach
the active job independently.

Cursors persist across restarts. The last 10,000 events are retained; `oldestCursor`
and `resetRequired` identify an expired cursor or replaced database. On reset,
refresh snapshots and restart from `after: 0`. This delivers pages at least once
when clients retry a cursor; clients deduplicate by cursor. Job timestamps use
UTC ISO 8601, while event cursors provide ordering.

## Framing, errors and access

Canonical types live in `Sources/ScannerAPI`. Report/page schemas carry version 1;
individual profile and deletion results are defined by the `v1` method contract.
Optional unknown values are omitted. Current settings are not hardware defaults.
All driver enum codes retain ImageCaptureCore's numeric definitions.

Responses contain exactly one of `result` or `error`. Error codes: -32700 invalid
JSON, -32600 invalid envelope, -32601 unknown method, -32602 invalid parameters,
-32603 internal error, -32000 structured ScannerError (`kind`, `message`, optional
driver domain/code). Partial inspections are results with an `errors` array.

Limits: 64 KiB requests, 4 MiB client responses, 16 in-flight connections,
5 seconds for a request frame, 10 seconds for response writes, 90 seconds client
response timeout. Excess connections/invalid frames are closed. Socket paths must
be absolute and at most 103 UTF-8 bytes. `UnixClient.callAsync` moves blocking I/O
onto a dispatch queue; Swift clients should use it instead of blocking the
cooperative executor. Its socket deadline still applies after task cancellation.

Socket directory 0700, socket 0600, same-user peer checks in both directions. The
endpoint lock protects its path, and a separate fixed per-user hardware lock
prevents conflicting service instances at different paths. Stale socket files are
replaced only after acquiring locks; regular files/symlinks are never substituted.
No TCP API or arbitrary shell/filesystem access is provided. The MCP server uses
this service boundary; a future HTTP gateway must do the same.

## Card pricing

`v1.batches.valuation`, params `{"id":"job UUID"}`, returns a versioned GBP market
guide without accessing hardware or mutating images. Poll while `refreshing` is
true. See [card pricing](card-pricing.md) for fields, coverage and cache rules.
