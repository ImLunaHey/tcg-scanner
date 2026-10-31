# Milestone 5 validation — 2026-10-31

## Implemented

Swift Hummingbird 2.27.1 HTTP gateway using ScannerService's private JSON-RPC socket.
Versioned REST endpoints cover devices/readiness/capabilities/settings, profiles,
asynchronous jobs/cancellation/history, batches/manifests, original image downloads,
service-generated thumbnails, diagnostics and Server-Sent Events. HTTP accepts no
arbitrary shell command or filesystem path, and never opens scanner hardware.

Authentication is mandatory even on loopback. Private root credentials create
one-hour bearer sessions; session tokens are stored hashed in memory and can be
revoked. Origin/authority validation, input bounds, connection/request/stream limits,
private token-file checks and shared service scheduling enforce access boundaries.
PNG is the default. The native and browser interfaces remain later milestones.

## Automated evidence

The final full suite of **45 tests passed** using
`swift test --disable-index-store -Xswiftc -warnings-as-errors`. Tests never feed
real hardware. Focused HTTP runs also passed during implementation.

The live HTTP integration test starts a Hummingbird TCP listener, sends requests
using independent HTTP connections, and connects the gateway to a real Unix socket
service backed by a mock scanner. It verifies:

- Missing authentication is rejected; root-to-session login and session revocation.
- Discovery/readiness/diagnostics/settings inspection do not capture.
- Profile creation, partial updates, deletion and persistent per-device defaults.
- Eight simultaneous HTTP submissions with one request ID produce one capture, including matching IDs with different letter casing.
- Reused IDs with changed input return conflict.
- PNG duplex job completion, original downloads, SHA-256 checks and batch manifests.
- ImageIO thumbnails reduce 800 × 600 fixture images to 512 × 384 without modifying originals.
- SSE progress and terminal state, Last-Event-ID resume without duplicates, expired-cursor reset.
- Busy hardware settings rejection, queued cancellation and active cancellation.
- Unrecognized Host is rejected even with an allowed X-Forwarded-Host header.

Other HTTP tests verify foreign Origin/cookies, invalid paths and UUIDs, unknown
query/JSON fields, duplicate query keys, remote output paths/shell arguments, excessive
bodies, content type, offline service, token-file modes, session expiry,
request limits, explicit Tailscale-only binding and dropped-response admission cleanup.
The service thumbnail test checks original preservation and symlink rejection.
Existing CLI/MCP/concurrency/recovery/disconnect tests continue to pass.

`swift build -c release --disable-index-store -Xswiftc -warnings-as-errors` passed.
SwiftPM emits upstream swift-distributed-tracing notices about unhandled CMakeLists
files; Swift compiler warnings were treated as errors. Early build/signing failures
coincided with disk exhaustion. Only rebuildable project outputs were removed; the
operator subsequently freed disk space. Scan files and metadata were retained.

## Installation and actual scanner reads

The updated service and all four release executables are installed under
`~/Library/Application Support/Scanner/bin/`; installed hashes matched release files.
The existing `local.scanner.service` and new `local.scanner.http` LaunchAgents are
running. Installer shell syntax and generated plists passed validation. HTTP binds
127.0.0.1:8170. A private mode-0600 credential was generated without printing it.

Authenticated live HTTP requests:

- Discovered the real USB fi-8170 and inspected readiness, capabilities and settings.
- Read profiles, diagnostics, service status and existing history.
- Retrieved the manifest and both PNG originals from job
  `C993DDA4-DF86-454B-BC27-63E09E64B984`.
- Verified content type, PNG signature, byte count and complete SHA-256 against the
  manifest and X-Image-SHA256 header.
- Retrieved two real thumbnails, both 362 × 512 pixels, respectively 32,192 and
  33,481 bytes. Originals were not converted or modified.
- Received SSE history through the live HTTP connection.
- Rejected unauthenticated job creation (401), foreign Origin (403), and a revoked
  session (401). Job history was unchanged before and after all live checks.

The verified original hashes remain:

- Sequence 1: `47f032f8a38c3a45b46a2b31ae50dde11af0988c0ded8614ff3a6ea800d72b4c`.
- Sequence 2: `22d5c9b0435d0614e224ebe851fc0e83a591407ed7eacaa3c49427cca6c24287`.

A SIGTERM/relaunch of the installed HTTP agent invalidated a previously issued
session, retained the root credential and left service job history unchanged. A live
request with an incomplete body returned HTTP 408 after 5.08 seconds, confirming
the body-read deadline without forwarding a scan request.

## Tailscale and remaining limits

A temporary second gateway bound explicitly to the currently discovered Tailscale
IPv4 `100.117.220.119`, port 8171, with `--allow-remote`. The same authenticated reads,
original/thumbnail downloads, SSE and negative access checks passed using that
address. The test client ran on the same Mac: **access from another physical device
has not been verified**. The temporary listener was stopped after testing. The
installed gateway remains loopback-only; Serve routes, HTTPS policy, Funnel and
Tailscale grants were not changed. See [remote setup](http-operations.md).

No physical scan was initiated over HTTP during this milestone. A complete HTTP
scan workflow is mock-tested; the same scanner service has prior physical PNG/TIFF
paper evidence. A supervised physical HTTP scan and a second-device/Tailscale HTTPS
check remain optional deployment acceptance tests before relying on those paths.

Bearer sessions share one scanner-control role; there are no separate user accounts,
read-only HTTP roles or persistent sessions. Range downloads, thumbnail caching,
web UI, card pairing/processing, and inventory integration are not implemented here.
Physical cancellation, sleep/wake during feeding and prolonged unattended runs
retain the earlier hardware-validation limits.

Ignored evidence: `Diagnostics/milestone5/` contains localhost/Tailscale reports,
retrieved images/thumbnails, event output and restart results. Tokens are not stored
in these reports. Root credentials and application logs live outside the repository.
