# Native macOS scanner platform

Milestone 1: Swift 6 discovery, driver capability enumeration and read-only
CLI diagnostics for a USB-connected Ricoh fi-8170. Verified on Apple Silicon,
macOS 26.5.2, Swift 6.3.3, with Ricoh's macOS ICA driver package 2.4.1.

Scanning, profiles, persistent jobs, launchd installation, MCP, HTTP and graphical
interfaces are not implemented in this milestone. No command feeds paper or
changes scan settings.

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
requests to finish closing their sessions, up to 120 seconds. Startup is manual;
no LaunchAgent has been installed. Run as your normal logged-in user, not root.
Other scanning applications may hold the exclusive ICA session; close their
scanner sessions before inspecting settings here.

For a release build, use `swift build -c release` and `.build/release/` binaries.

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
busy, **64** invalid command-line arguments. There is no `scanner scan` command yet.
A diagnostic query that finds no devices succeeds and reports a warning; this is
not evidence that hardware access works.

Capabilities report current values, allowed values/ranges and defaults only where
exposed. `readOnly` describes the public API; a setter's existence is not proof a
write will succeed. Scan capabilities remain `driverExposedNotTested`. Unknown
capabilities remain explicitly listed; current values never substitute for defaults.

Known fi-8170 findings: blank-page skipping is on, scan-area/physical-size fields
are zero, and two vendor enumerations have unlabelled values. These are reported
as warnings without changing anything. See [hardware findings](Docs/hardware-discovery.md).

## Architecture

- **ScannerAPI**: shared Codable/Sendable models and JSON-RPC envelopes.
- **ScannerCore**: replaceable asynchronous backend, ImageCaptureCore discovery,
  bounded session lifecycle, capability snapshots and an actor coordinating access.
- **ScannerServiceSupport**: read-only API routing and input validation.
- **ScannerTransport**: private Unix socket server/client, framing and access checks.
- **ScannerService**: sole hardware owner, main run loop and signal handling.
- **ScannerCLI**: ArgumentParser client. Does not import ScannerCore or ImageCaptureCore.

A minimal diagnostic service is included now to enforce the single-owner boundary.
The full job/persistence service remains a later milestone. The earlier development
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
CLI parsing and service/client integration. Physical checks are separate and
recorded in the validation document. No automated test requests a scan.

Build outputs, downloaded driver files and local JSON/log reports under
`Diagnostics/` are ignored. Original scan storage is not yet implemented because
this milestone does not scan.
