# Native macOS scanner platform

Milestones 1–4: Swift scanner discovery, capability reports, basic A4 simplex/duplex
scanning, and a persistent background service for the USB Ricoh fi-8170.
The CLI and Swift MCP server use the service exclusively. No graphical application
is required. HTTP, SwiftUI, web interfaces and card processing remain later milestones.
See [MCP setup and agent workflow](Docs/mcp.md) and [milestone 4 validation](Docs/milestone-4-validation.md).

Actual hardware evidence: [paper scans](Docs/milestone-2-validation.md) and
[LaunchAgent verification](Docs/milestone-3-validation.md). Card feeding has not
been validated. Discovery and diagnostics never request a scan.

## Build and startup

Requires macOS 14+, Swift 6.1+ (GRDB's minimum) and the official ICA driver.
Hardware checks used Apple Silicon, macOS 26.5.2, Swift 6.3.3 and Ricoh driver
package 2.4.1. Other macOS versions have not been hardware-tested.

```sh
swift build
swift test
# Manual foreground service; do not run alongside the installed LaunchAgent:
.build/debug/scanner-service --verbose
```

Or install the per-user background service and automatic login startup:

```sh
Scripts/install-launch-agent.sh
"$HOME/Library/Application Support/Scanner/bin/scanner" devices --json
```

The installer builds release binaries, copies them to a stable location and
registers `local.scanner.service` in your logged-in GUI session. It does not start
a scan. `--no-start` installs without loading. To stop automatic startup:

```sh
Scripts/uninstall-launch-agent.sh
```

Uninstall retains scans, metadata, profiles, logs and binaries. For an update,
uninstall first, then install again. See [service operations](Docs/service-operations.md)
for paths, restart commands, troubleshooting and physical test steps.

## Discover and inspect

From the repository, the debug CLI can also use the installed service:

```sh
.build/debug/scanner devices
.build/debug/scanner devices --json
.build/debug/scanner device --json
.build/debug/scanner capabilities --json
.build/debug/scanner settings --json
.build/debug/scanner diagnostics --json
.build/debug/scanner service-status --json
```

Specify `--device ID` if multiple scanners are connected. Discovery includes all
local ICA scanners; only the fi-8170 ADF has been tested. Capability reports retain
unknowns and driver anomalies. A writable property is not proof that setting it
works. [Hardware findings](Docs/hardware-discovery.md) describe the observed driver.
Inspections require an idle scanner; service status, job history and events remain
available while scanning.

## Scan ordinary A4 paper

Load exactly one expendable sheet for an initial test. `scan` feeds until the ADF
empties or a capture limit is reached; it does not promise to feed only one sheet.
The default storage root is `~/Scans`. All output directories must stay inside the
service's configured `--storage-root`.

```sh
.build/debug/scanner scan --output ~/Scans/test-scans --dpi 300 --format tiff
# Reload the sheet before another scan:
.build/debug/scanner scan --duplex --dpi 300 --format tiff --no-wait --json
.build/debug/scanner jobs status JOB_UUID --json
.build/debug/scanner jobs cancel JOB_UUID --json
.build/debug/scanner jobs list --json
.build/debug/scanner events --follow --json
```

`scan-status` and `scan-cancel` remain supported aliases. A start returns a queued
job and the default CLI waits for its terminal state. Cancelling a queued job
prevents feeding; cancelling an active job is best-effort. Poll until terminal.
Ctrl-C in the waiting CLI disconnects the client; use `jobs cancel` to cancel the
service-owned job.

Pass `--request-id UUID` to deduplicate retries, including after service restart.
The same UUID with different inputs is rejected. A new UUID means a new physical
scan; never generate one merely because a response was lost.

```sh
.build/debug/scanner profiles create 'Documents' --dpi 300 --duplex --json
.build/debug/scanner profiles list --json
.build/debug/scanner profiles show Documents --json
.build/debug/scanner profiles update Documents --dpi 300 --json
.build/debug/scanner scan --profile Documents --no-wait --json
```

A selected profile supplies DPI, duplex and format, replacing those command
options. Jobs snapshot profiles when submitted; later edits cannot change queued
or active hardware configuration. Saving a profile does not touch hardware.
Names are unique without regard to case. `profiles update` changes only supplied
fields; `--no-duplex` explicitly selects simplex. `profiles delete` accepts a name
or UUID. Profiles currently cover the basic A4 workflow, not card processing.

Save per-device defaults without feeding paper or writing hardware properties:

```sh
.build/debug/scanner configure --dpi 300 --duplex --format tiff --json
# Only after paper is ready:
.build/debug/scanner scan --use-device-settings --no-wait --json
```

Defaults are validated against the idle scanner and snapshotted into each job.
Use either a profile or device defaults, not both. `device --json` reports readiness
when the driver exposes it; a ready result is a momentary observation.

Retrieve existing scans without feeding:

```sh
.build/debug/scanner batches list --json
.build/debug/scanner batches show JOB_UUID --json
.build/debug/scanner batches manifest JOB_UUID > manifest.json
.build/debug/scanner images metadata JOB_UUID 1 --json
.build/debug/scanner images download JOB_UUID 1 --output ./original.tiff --json
```

Downloads verify chunk and complete-file SHA-256 values, use private file permissions,
and refuse to overwrite an existing destination. Image access uses service-owned
job IDs and sequence numbers. The CLI export destination is local to the CLI.

Current scan scope: A4, RGB, 8 bits per channel, driver-supported 150–600 DPI,
TIFF or PNG. **Only 300-DPI TIFF simplex and duplex have been physically tested.**
The PNG path is fixture-tested. Scans require ADF/A4/geometry/bit-depth controls
plus cropping and blank-page-skip controls. Unsupported settings fail before
feeding. Multifeed, paper thickness and transport safety settings are never written.
Temporary changes are restored after the scan stops.

## Persistence and recovery

Each job creates a unique batch directory:

```text
scan-UUID/
  manifest.json
  incoming/                  # untouched driver files, including partial/invalid files
  originals/page-000001.tiff  # verified byte-identical copies
  originals/page-000002.tiff
```

Manifests update atomically after each image. GRDB/SQLite stores jobs, original
requests, profiles, image metadata, batches, capabilities, device observations and
events. Metadata migrations run at startup. Original images stay on disk and are
never overwritten by a retry. Keeping incoming and originals uses roughly twice
the image storage. Callback order is retained; side identity remains unknown.
No cropping, rotation or image processing is implemented yet.

States: queued → preparing → scanning → processing → completed, or failed/cancelled.
`processing` currently means finalizing storage, not card-image processing.
On shutdown or sleep, outstanding jobs are cancelled. At restart, unfinished jobs
are marked failed/interrupted; a newer terminal manifest can be reconciled into
the database. **Startup never resumes physical feeding.** Files not yet indexed
remain available for manual review. A disk error or unconfirmed scanner session
closure stops scheduling until the service is restarted.

Limits: 100 waiting jobs, 256 profiles, 120 seconds of capture, cancellation after
100 image callbacks, 512 MiB per accepted file and 10,000 replayable events. History
and original files have no automatic deletion policy; monitor available disk space.
A single hardware session runs at a time, including across multiple connected
scanners, matching the current backend's session limitation.

## Architecture and access

- **ScannerAPI**: shared Codable/Sendable contracts.
- **ScannerCore**: backend abstraction, ImageCaptureCore, storage and job scheduler.
- **ScannerPersistence**: GRDB transactions, migrations, history and recovery.
- **ScannerServiceSupport**: versioned JSON-RPC routing and validation.
- **ScannerTransport**: private Unix sockets; blocking CLI and asynchronous Swift client APIs.
- **ScannerService**: hardware owner, main run loop, monitoring, lifecycle and structured logs.
- **ScannerCLI**: ArgumentParser client; imports no hardware backend.
- **ScannerMCP / ScannerMCPSupport**: official Swift MCP SDK, stdio tools and schemas;
  communicates through the same private Unix socket, with no hardware backend.

Default socket: `~/Library/Application Support/Scanner/run/control.sock`.
Directory mode 0700, socket mode 0600, and same-user peer checks restrict local
access. A per-user hardware lock rejects another service even with different
socket/database paths. ICA additionally controls sessions held by other apps.
No TCP listener, shell execution endpoint or remote control is exposed.

See [local API](Docs/local-api.md) for job, profile, event and diagnostic methods.
JSON results go to stdout; logs go to stderr. Exit codes: 0 success, 1 scan/API/I/O
failure, 3 unavailable service, 4 missing/ambiguous device, 5 busy service,
6 cancelled scan, 64 argument usage error. Queries may succeed with no devices;
partial inspections include errors and exit 1.

## Validation and scope

`swift test` uses Swift Testing, mock backends and real Unix sockets. It covers
concurrent submissions, queue cancellation, immutable profile snapshots,
persistence/recovery, disconnects, simulated sleep, disk failure, transport limits,
permissions, checksums, CLI parsing, private original exports, and a complete workflow
through the compiled MCP executable over stdio. Automated tests never feed the real scanner.
Physical sleep/wake, unplugging during capture and cancellation during a real feed
remain unverified; do those controlled paper checks before unattended operation.

`Diagnostics/` and `.build/` are ignored runtime artifacts. Existing physical scans
under `Diagnostics/test-scans/` are preserved. The earlier discovery probes remain
in Git history. Inventory integration is outside the current milestone.
