# Milestone 1 validation — 2026-10-31

Debug build and release build (`swift build -c release -Xswiftc -warnings-as-errors`)
both passed. The release build produced both executables without compiler warnings.

## Automated, hardware-independent

`swift test`: 15 Swift Testing tests passed. Tests use mock scanner backends and
private temporary Unix sockets. Coverage includes:

- JSON boolean/number/null distinctions and unknown-default preservation.
- Missing/ambiguous devices, explicit selection and backend disconnection errors.
- Concurrent inspection exclusion and cancellation/failure releasing ownership.
- Capability options surviving serialization and current-settings projection.
- Driver anomaly warnings without discarding raw values.
- Malformed JSON/envelopes, unknown methods, invalid parameter types and write rejection.
- All four RPC methods through the router and real server/client transport.
- Socket permissions, duplicate endpoint ownership, unsafe directories and refusal
  to overwrite ordinary files.
- Frame size/deadline/path limits, CLI arguments and stable exit-code mapping.

These tests do not verify physical feeding, scanner reconnection, sleep/wake,
LaunchAgent context, scan cancellation or card safety.

## Physical fi-8170, logged-in command-line process

Environment: Apple Silicon, macOS 26.5.2, Swift 6.3.3, ICA package 2.4.1.
ScannerService was started without a native scanner GUI. The CLI connected over
the default Unix socket. No scan/overview requests or scanner setting writes were
made. The following checks passed:

1. `scanner devices` and `scanner devices --json` identified fi-8170 over USB.
2. `scanner capabilities --json` opened a session, inspected ADF unit 3 and eight
   vendor features, and confirmed session closure with no driver errors.
3. `scanner settings --json` returned current values with no driver errors.
4. `scanner diagnostics --json` returned OS/backend/device/session information.
5. A second service process failed startup instead of replacing the live socket.
6. A concurrent capabilities request received a structured busy response while
   diagnostics was active. Discovery remained available.
7. A nonexistent scanner ID exited 4 with JSON; an unavailable service exited 3
   with JSON. Unimplemented `scan` exited 64.
8. SIGTERM during an open hardware inspection stopped new requests, allowed the
   active CLI to complete with `sessionClosed: true`, then exited successfully.
9. An idle test service was forcibly terminated. A new instance acquired the lock,
   replaced the stale socket and discovered the fi-8170. It was then shut down.

All temporary test service processes were stopped. No LaunchAgent was installed.
Local evidence is ignored under `Diagnostics/`:
`milestone1-capabilities.json`, `milestone1-settings.json`,
`milestone1-diagnostics.json`, `milestone1-lifecycle.log`.

See [hardware findings](hardware-discovery.md) for raw capability values and
limitations. Feature availability was read from the driver; actual simplex/duplex
image output remains untested. Before milestone 2 scanning, validate nonzero scan
geometry and image preservation with ordinary paper, then separately request
approval for card-feeding experiments.
