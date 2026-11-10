# Milestone 3 validation — 2026-10-31

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

## Implemented

Swift background ownership with a durable GRDB/SQLite ledger, migrations, queued
jobs, request idempotency, cancellation, image/batch metadata, profile snapshots,
capability/device persistence, cursor-based progress events, periodic device
monitoring, interrupted-job recovery, structured logging, per-user hardware lock,
LaunchAgent installation and sleep/wake handling.

The current backend serializes all hardware sessions, including across scanners.
No GUI, HTTP listener, MCP server or Rust component is involved. Processing state
currently denotes storage finalization, not card processing.

## Automated evidence

`swift test -Xswiftc -warnings-as-errors`: **34 tests passed**. Tests use mock
backends, temporary storage and real Unix sockets. Important cases include:

- Twenty simultaneous identical submissions produce one job/capture.
- Twelve real socket clients submit the same request; a second job queues, queued
  cancellation prevents its capture, active cancellation retains an image, and
  status/manifest/idempotency survive reopening the database and service endpoint.
- Edits/deletion of a profile cannot alter an already queued job's settings.
- Simulated disconnects fail active/queued jobs; device transitions are recorded,
  and reconnect allows a newly requested mock scan to complete.
- Shutdown/simulated sleep cancel waiting jobs without feeding them.
- Wake does not clear an unconfirmed hardware-session quarantine.
- Storage becoming unavailable stops scheduling while retaining received originals.
- Restart recovery marks unfinished jobs interrupted, reconciles newer manifests,
  retains metadata and never feeds hardware.
- Traversal, symlink, socket permission, lock exclusivity, malformed RPC and bounded
  read checks; original checksums/order and CLI parsing.

The concurrent socket test initially exposed starvation when blocking clients ran
on Swift's cooperative executor. `UnixClient.callAsync` now dispatches blocking
socket I/O outside that executor; the test passes using the asynchronous entry point.

`swift build -c release -Xswiftc -warnings-as-errors`: **passed**.
Installer/uninstaller shell syntax and generated plist validation passed. An actual
uninstall/reinstall round trip passed after adding a bounded wait for launchd to
remove a stopping agent. Installed executable hashes match the release artifacts.

## Actual LaunchAgent checks

Machine: Apple Silicon macOS 26.5.2; Swift 6.3.3. fi-8170 USB ID 04c5:15ff;
Ricoh ICA package 2.4.1. The release service was installed and bootstrapped in
`gui/501` as `local.scanner.service`, with no scanner GUI running.

Observed:

- `devices --json` returned fi-8170, ID `02210000-0000-0000-0000-000004C515FF`.
- `capabilities --json` opened and closed a real session, enumerated the ADF unit,
  and returned no errors. No scan was requested.
- `service-status` reported no active job, zero queued jobs and no pause.
- A second service at different socket/database paths exited 1 because the per-user
  hardware owner lock was already held. It never opened hardware.
- A saved “General documents” profile survived both SIGTERM/relaunch and an idle
  SIGKILL/relaunch. After the crash, scanner discovery still succeeded.
- Events preserved cursor order across these restarts: service start, device
  connection, profile update, graceful stop, subsequent starts.
- SQLite `PRAGMA integrity_check` returned `ok`; database and socket modes were 0600.
  The live job table remained empty during these initial restart checks. A later
  explicitly confirmed physical scan is recorded below.

Ignored evidence is retained in `Diagnostics/milestone3/`: launchd device/capability
reports, service status, profile before/after restart, events and second-owner
rejection. Installed logs and metadata are outside the repository under the user's
Library directories. Existing milestone-2 originals remain untouched. The final installed service is
running with automatic login startup enabled, fi-8170 discovered, no active job,
zero queued jobs and no pause.

## Physical duplex scan through LaunchAgent

After the user confirmed that the marked A4 sheet was loaded, the installed CLI
submitted one duplex, 300-DPI TIFF job to the running LaunchAgent. No GUI was used.

- Job: `8D93D5FD-C0A7-4379-B5EC-B8EB16291611`.
- Started: `2026-10-31T15:45:19Z`; completed: `2026-10-31T15:45:26Z`.
- State: `completed`; errors: none; scanner session closure confirmed.
- Output: `~/Scans/validation/scan-8d93d5fd-c0a7-4379-b5ec-b8eb16291611/`.
- Two original TIFFs, each 2480 × 3508 pixels and one image per file.
- Visual inspection confirmed handwritten BACK in `page-000001.tiff` and FRONT in
  `page-000002.tiff`. This observation applies to this loading orientation; side
  metadata remains unknown and no general front/back pairing rule is inferred.

| Original | Bytes | SHA-256 |
| --- | ---: | --- |
| `page-000001.tiff` | 747478 | `c35998f39e44bda575c943fc3a618b6d784cf75b94ad9b502b73f01669a39b05` |
| `page-000002.tiff` | 695478 | `63dfceb6e654f79b8a0db460d06e7be817fb228d3dfa0f6060a04ee6886e0928` |

Independent checksums match the manifest and retained incoming driver files.
The manifest, API result and SQLite job record match, with two persisted image
rows. Events 11–17 record queued, preparing, scanning, each saved image, processing
and completed. The service returned to idle with no queued jobs.

Before/after settings reports match for all enumerated common settings and vendor
values. Cropping returned to 2, blank-page skipping to 3, and multifeed detection
remained 1. No driver error was reported. The user subsequently confirmed “yep it
worked!” after this test. They explicitly reported no feeding problems during the
earlier paper run; no additional claim about sheet damage is inferred.

Evidence: `Diagnostics/milestone3/physical-duplex/` (ignored); originals remain in
`~/Scans/validation/`. No application processing or rotation was applied.

## Explicit limits

The earlier milestone verified real simplex/duplex 300-DPI TIFF A4 scans through
the foreground service. This milestone additionally verifies **launchd discovery,
inspection and a completed duplex paper scan with persistent metadata**. Physical
cancellation, disconnection and sleep/wake in that process context remain untested.
Mock tests do not establish those hardware behaviors.
See [service operations](service-operations.md) for exact supervised paper tests.

Crash recovery was tested with persisted interrupted-job fixtures and newer
manifests; the actual process crash test was idle. Orphan/partial incoming files
are retained for manual review, not automatically indexed as verified originals.
Old milestone-2 manifests are not automatically imported into the new database.
Original-file/metadata retention and log rotation are manual. Queue order is FIFO
by accepted submission; jobs are not resumed after restart. No card feeding,
front/back inference, crop/deskew, HTTP, MCP or graphical interface is claimed.
