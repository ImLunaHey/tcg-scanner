# Native macOS scanner platform

Milestones 1–2: Swift 6 discovery, driver capability enumeration, CLI diagnostics
and basic simplex/duplex A4 scanning for a USB-connected Ricoh fi-8170. Verified on Apple Silicon,
macOS 26.5.2, Swift 6.3.3, with Ricoh's macOS ICA driver package 2.4.1.

Profiles, persistent job recovery, launchd installation, MCP, HTTP and graphical
interfaces are not implemented yet. Diagnostic commands never feed paper or
change scan settings. The explicit `scan` command feeds the loaded A4 paper.

## Build and run

Requires macOS 14 or later, a Swift 6 toolchain and the scanner's official ICA
driver. Only macOS 26.5.2 has been hardware-tested. From the repository root:

```sh
swift build
swift test
.build/debug/scanner-service --verbose
```

Keep the service running. In another terminal, from the same directory:

```sh
.build/debug/scanner devices
.build/debug/scanner devices --json
.build/debug/scanner capabilities --json
.build/debug/scanner settings --json
.build/debug/scanner diagnostics --json
```

With multiple scanners, pass `--device ID` to capabilities, settings or diagnostics
using an ID from `devices`. Discovery includes all local ICA scanners; ADF-specific
fields have been implemented and tested on the fi-8170. Common functional-unit
fields are also recorded for other scanner types; they have not been hardware-tested.

Stop the service with Ctrl-C. SIGINT/SIGTERM stop new connections and allow active
diagnostic requests to finish closing their sessions. Any active scan is cancelled
and its received images retained before shutdown. Startup is manual;
no LaunchAgent has been installed. Run as your normal logged-in user, not root.
Other scanning applications may hold the exclusive ICA session; close their
scanner sessions before inspecting settings here.

For a release build, use `swift build -c release` and `.build/release/` binaries.


## Basic A4 scanning

Only ordinary A4 paper has been physically validated. Card feeding is not approved
or validated. `scan` consumes loaded sheets until the feeder empties, completion,
cancellation or a limit is reached; it does not promise to feed only one sheet.
For initial tests load exactly one expendable sheet.

The default storage root is `~/Scans`. To choose another, start the service with
`--storage-root /absolute/directory`. All client output directories must be inside
that root. In another terminal:

```sh
.build/debug/scanner scan --output ~/Scans/test-scans --dpi 300 --format tiff
# Reload the test sheet before the next command:
.build/debug/scanner scan --output ~/Scans/test-scans --dpi 300 --format tiff --duplex
```

Use `--json` for a final machine-readable result, or `--no-wait --json` to return
an ID immediately. Track or cancel it with:

```sh
.build/debug/scanner scan-status SCAN_UUID --json
.build/debug/scanner scan-cancel SCAN_UUID --json
```

Cancellation is asynchronous; poll status until terminal. Ctrl-C in the waiting
CLI disconnects that client; it does **not** cancel the service-owned scan. Use
`scan-cancel`, or stop the service to request cancellation. An explicit
`--request-id UUID` deduplicates start retries within the same service run; changed
settings with the same ID are rejected. Do not automatically retry a start with a
new UUID after a transport error because the original scan may still be running.

Current scope: A4, RGB, 8 bits per channel, driver-supported 150–600 DPI, TIFF or
PNG. Only 300-DPI TIFF simplex and duplex have been hardware-tested. Scans require
the driver's ADF, A4, inches, bit depth, resolution, blank-page-skip and cropping
controls. Unsupported configurations fail before `requestScan`. Multifeed and
paper-thickness settings are never written. Changed settings are restored after
the scan stops. The PNG path is implemented and storage-tested with fixtures but
has not been physically scanned on this device.

Each scan creates a unique `scan-UUID/` directory containing:

```text
manifest.json
incoming/                  # unmodified driver-delivered files, also retained on error
originals/page-000001.tiff  # byte-identical, checksum-verified copies
originals/page-000002.tiff
```

The manifest is atomically replaced after each preserved file and at completion.
It records settings, callback sequence, image dimensions/count, SHA-256, errors and
session closure. Files are never intentionally overwritten or deleted. Copies
are not cropped, rotated, deskewed or recompressed. Driver processing still applies:
“original” means the delivered file, not raw sensor data. Keeping both incoming
and originals intentionally uses roughly twice the image storage for now.

Callback order is preserved; filenames do not claim front/back identity. An odd
duplex image count is flagged for review. Basic limits are 120 seconds of capture,
cancellation after 100 image callbacks, 512 MiB per file accepted into originals,
and 100 records per service run. Extra/invalid driver files remain in incoming.
Cancellation is best-effort and may allow already-feeding sheets to complete.

On service restart, files/manifests remain but status IDs are not reloaded and
interrupted manifests can still say preparing/scanning. Crash recovery, queued
jobs, durable idempotency and long-running batch reliability belong to milestone 3
and later. Do not use this milestone unattended for valuable media.

## Reports and diagnostics

```sh
mkdir -p Diagnostics
.build/debug/scanner capabilities --json > Diagnostics/capabilities.json
.build/debug/scanner diagnostics --json --verbose > Diagnostics/diagnostics.json
```

JSON goes to stdout; verbose client/service logs go to stderr. Runtime failures
with `--json` return a structured `error`. If an inspection partly succeeds, its
partial report includes `errors` and the CLI exits 1. ArgumentParser usage errors
remain text on stderr. Avoid overwriting a report you need to preserve.

Exit codes: **0** successful query (possibly no devices), **1** inspection/API/I/O
failure, **3** service unavailable, **4** missing or ambiguous device, **5** service
busy, **6** scan cancelled, **64** invalid command-line arguments.
A diagnostic query that finds no devices succeeds and reports a warning; this is
not evidence that hardware access works.

Diagnostic capabilities report current values, allowed values/ranges and defaults only where
exposed. `readOnly` describes the public API; a setter's existence is not proof a
write will succeed. The generic capability snapshot uses `driverExposedNotTested`; the separate
[hardware validation record](Docs/milestone-2-validation.md) records exactly which
scan configurations have been tested. Unknown
capabilities remain explicitly listed; current values never substitute for defaults.

Known fi-8170 findings: blank-page skipping is on, scan-area/physical-size fields
are zero, and two vendor enumerations have unlabelled values. These are reported
as warnings without changing anything during diagnostics. Scans explicitly set
A4 geometry and temporarily disable blank-page skipping and automatic cropping. See [hardware findings](Docs/hardware-discovery.md).

## Architecture

- **ScannerAPI**: shared Codable/Sendable models and JSON-RPC envelopes.
- **ScannerCore**: replaceable asynchronous backend, ImageCaptureCore discovery,
  bounded session lifecycle, capability snapshots, scan storage and an actor coordinating access.
- **ScannerServiceSupport**: API routing and input validation.
- **ScannerTransport**: private Unix socket server/client, framing and access checks.
- **ScannerService**: sole hardware owner, main run loop and signal handling.
- **ScannerCLI**: ArgumentParser client. Does not import ScannerCore or ImageCaptureCore.

A minimal service enforces the single-owner boundary. Scan records are available
in memory during its run and persisted as per-batch manifests; the full queued
job/SQLite recovery service remains a later milestone. The earlier development
probes are preserved in Git history and have been removed from the working tree.

The socket defaults to `~/Library/Application Support/Scanner/run/control.sock`.
Both executables accept `--socket /absolute/path` for isolated development/test
endpoints. The parent directory must be owned by the current user with mode 0700;
the socket is mode 0600, peers must have the same user ID, and an advisory lock
prevents conflicting instances at the same endpoint. All clients for your scanner
should use the same endpoint. ICA additionally enforces session exclusivity across
applications. No TCP listener, shell execution or remote control is exposed.

See [local API contract](Docs/local-api.md) and [validation record](Docs/milestone-1-validation.md).

## Tests and scope

`swift test` runs hardware-independent Swift Testing tests with mock backends and
real Unix sockets. They cover selection, capability serialization, concurrency,
cancellation, backend failure recovery, RPC validation, private paths, framing,
CLI parsing, service/client integration, scan ownership/cancellation, original-file
checksums, collision/path protection and partial-duplex retention. Physical checks are separate and
recorded in the validation document. No automated test requests a scan.

Build outputs, downloaded driver files and local JSON/log reports under
`Diagnostics/` are ignored. Real scan batches are written under the configured storage root; they are never
removed by tests or cleanup. Test scans under `Diagnostics/test-scans/` are ignored
but retained locally.
