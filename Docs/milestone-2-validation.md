# Milestone 2 — basic scanning validation, 2026-10-31

## Implemented

Swift ImageCaptureCore ADF scanning through the single ScannerService owner;
explicit A4/RGB/8-bit configuration, simplex/duplex, TIFF/PNG selection, asynchronous
start/status/cancel, bounded capture, original-file preservation and atomic batch
manifests. No subprocess provides runtime scanner functionality. The CLI is only
a Unix API client. There is no graphical application dependency.

The shared connection lifecycle handles session opening, readiness, functional-unit
selection and closure for both diagnostics and scanning. Enum objects assigned to
the driver's non-retaining `currentValue` property are retained for the session.
Scan configuration is read back before feed. Cropping and blank-page skipping are
temporarily disabled; multifeed detection and transport-safety settings are untouched.

## Automated checks

21 Swift Testing tests passed, including new tests for:

- Non-overwriting unique storage and byte-identical original copies.
- Callback order preserved even if file handling arrives out of order.
- Traversal/symlink rejection and invalid-image retention.
- Scan request idempotency/conflicts and exclusion against inspection/other scans.
- Cancellation retaining received images and releasing ownership.
- Invalid requests rejected before hardware access; shutdown rejects new scans.
- Partial duplex failure persisting the received side and a review warning.
- Malformed scan RPC input and CLI scan option validation.

Existing discovery, JSON schema, socket access, API and CLI tests also pass.
Debug and release builds both passed with warnings treated as errors
(`swift build -Xswiftc -warnings-as-errors` and
`swift build -c release -Xswiftc -warnings-as-errors`).

## Physical tests

Environment: Apple Silicon Mac, macOS 26.5.2, Swift 6.3.3, fi-8170 over USB with
ICA package 2.4.1. User confirmed ordinary expendable A4 paper marked FRONT/BACK;
no trading cards were used. Tests were initiated through `scanner scan`, with the
native GUI absent. The user reported no feeding problem during the simplex pass.

**Simplex:** scan `a0d0ee1a-f8cc-4037-ae4b-e035d8d590bc`, completed at
15:16:28 UTC. One TIFF, 2480 × 3508 pixels, 699,068 bytes. Visually inspected:
shows the handwritten BACK marking. Driver completion reported no error; session
closure confirmed. Original checksum:
`425d09981eac84125fd4ef931ebf4ae8ebe3c16a3bf75404ae2e66e698a986c1`.

**Duplex:** after the user reloaded the same sheet in the same orientation, scan
`487f02ec-735b-492f-ad9d-a8c80a1df9bd` completed at 15:18:07 UTC. Two TIFFs,
both 2480 × 3508 pixels. Visually inspected in preserved callback order:

1. BACK, 756,148 bytes, SHA-256
   `24d208762413ac6c1f89eb3535f7e81da1026c9cf0ac83720f2dde50b0847476`.
2. FRONT, 697,236 bytes, SHA-256
   `60a609caf4f93b9e8358c46fece45d71c0db2c8089a9fb14450717f81896c3ae`.

Both passed ImageIO decoding and independent checksum verification. All three TIFFs
report 300 × 300 DPI, RGB, depth 8, TIFF compression 5 (LZW), and Scanner RGB Profile.
No application rotation/cropping/recompression was applied. Thin black margins are
visible with automatic cropping disabled. The handwritten labels are sideways
because of the original page orientation; this milestone preserves it.

Both scans used A4 bounds 8.267716535433072 × 11.692913385826772 inches,
blank-page-skip 0, cropping 0. Post-scan diagnostic reads confirmed original
cropping 2, blank-page-skip 3 and duplex false were restored. Multifeed detection
remained 1 (overlapping) throughout the observed before/after settings.

The BACK/FRONT order describes this particular loading orientation, not a universal
scanner-side convention. Manifests deliberately retain side=unknown until a
separately validated pairing policy is implemented. Multi-sheet ordering remains
untested; this validates one duplex sheet, not a production card batch.

## Retained evidence and limits

Local files are retained under ignored `Diagnostics/test-scans/scan-UUID/`, including
incoming driver files, originals and manifests. CLI reports:
`Diagnostics/milestone2-simplex.json`, `Diagnostics/milestone2-duplex.json`.
Restored settings reports:
`Diagnostics/milestone2-settings-after-simplex.json`,
`Diagnostics/milestone2-settings-after-duplex.json`.

The test service was shut down after verification. No LaunchAgent was installed.
PNG output, other resolutions, multi-sheet feed, physical cancellation, unplugging
during scans, jams, sleep/wake and all card media remain hardware-untested. Mock
cancellation/disconnection tests are not represented as physical tests. No unsafe
media experiment or multifeed-disable operation was performed.

Persistent queues, SQLite history, crash recovery, durable idempotency, calibrated
front/back pairing, image processing and inventory integration remain later work.
