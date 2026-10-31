# Local diagnostic API v1

Newline-delimited UTF-8 JSON-RPC 2.0 requests over a Unix domain socket. One request
and response per connection. `id` must be a string or number and is echoed back.
This diagnostic subset does not implement notifications, batches or subscriptions.
Clients should use monotonically unique request IDs or UUIDs.

```json
{"jsonrpc":"2.0","id":"request-1","method":"v1.capabilities","params":{"deviceID":"ID from v1.devices"}}
```

Methods:

- `v1.devices`: no parameters; result contains `schemaVersion` and `devices`.
- `v1.capabilities`: optional `deviceID`; result is an `Inspection`.
- `v1.settings`: optional `deviceID`; current-value projection of an inspection,
  retaining metadata, errors and warnings but excluding allowed/default values.
- `v1.diagnostics`: optional `deviceID`; OS, backend, devices and an inspection
  when exactly one device is found or an explicit ID is supplied. With no devices
  it returns a warning without opening a session. Multiple devices require an ID
  to include an inspection.

An absent ID selects the only scanner; zero or multiple devices cause an explicit
error for capabilities/settings. Unknown parameter keys and setting-write requests
are rejected. No scan method exists. Queries other than discovery may briefly open
an exclusive diagnostic session; another inspection gets `busy` instead of
waiting in an unbounded queue. Discovery remains available during inspection.

Responses contain exactly one of `result` or `error`. Results have `schemaVersion: 1`.
Errors use standard codes `-32700` (invalid JSON), `-32600` (invalid envelope),
`-32601` (unknown method), `-32602` (invalid parameters), `-32603` (internal error),
or `-32000` with structured scanner error data including `kind` and `message`.
Driver errors preserve their domain and numeric code. A partly successful
inspection is a result with an `errors` array, allowing callers to retain any
captured data; callers must check that array before treating the query as complete.

The canonical schemas are the Codable models in `Sources/ScannerAPI`. Dates use
ISO 8601 UTC. Settings preserve numeric types, booleans, nulls and raw driver
values. Vendor `values` and `labels` are independent arrays: unequal counts must
not cause guessed labels or values to be discarded. Optional defaults are omitted
when unknown. Unknown vendor value types are marked, not stringified as usable
settings. `unknownCapabilities` is explicitly a list of unknown support, not a list
of unsupported hardware features. Unit/document/orientation numeric codes follow
ImageCaptureCore's public SDK enums. Bit depth is per channel; scan resolution is
DPI. Sizes and scan-area coordinates use the reported `measurementUnit` enum.

Limits: 64 KiB requests, 4 MiB responses, 16 in-flight connections, 5 seconds to
receive a request, 10 seconds to send a response, 90 seconds client response wait.
Oversized/incomplete frames or excess connections are closed. Socket paths are
absolute and at most 103 UTF-8 bytes. Sockets and lock files cannot replace ordinary
files or symlinks. Lock files intentionally remain after shutdown; the advisory lock,
not file existence, indicates ownership. An idle crashed service's stale socket is
replaced only after acquiring the lock.

Access is same-user only (`getpeereid` on both sides, directory 0700, socket 0600).
There is no network listener or authentication/session cookie. This is not the
future remote HTTP API. A separate `--socket` creates a separate diagnostic service;
production clients should all use the default instance. ICA prevents concurrent
hardware sessions even across separate applications.
