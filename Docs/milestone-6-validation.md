# Milestone 6 validation — native macOS app

Date: 2026-10-31. Machine: Apple Silicon, macOS 26.5.2, Xcode 26.6,
Swift 6.3.3. The native application is installed at
`~/Applications/Scanner.app`; the existing per-user ScannerService remains the
hardware owner. The app uses the Unix control socket, not the HTTP gateway.

## Automated checks

- `swift test --disable-index-store -Xswiftc -warnings-as-errors`: **51 tests pass**.
- `swift build -c release --disable-index-store -Xswiftc -warnings-as-errors`: passes.
- Release app build/installation with `Scripts/install-mac-app.sh`: passes.
- `codesign --verify --strict ~/Applications/Scanner.app`: passes (local ad-hoc signature).
- Xcode project builds for arm64. Strict compiler checking uses
  `SWIFT_TREAT_WARNINGS_AS_ERRORS=YES SWIFT_SUPPRESS_WARNINGS=NO`; disabling warning
  suppression avoids Xcode's contradictory defaults for package targets.
- Installer shell syntax and application Info.plist validation pass.

New tests cover real Unix-socket native client calls, profile CRUD, structured
errors, response correlation, crop/rotation dimensions and input validation,
original retention, processed-image thumbnails, edit idempotency/conflicts,
symlink rejection and complete-sidecar recovery after a simulated ledger gap.
Exports are exercised across multiple chunks, with checksum corruption and
existing-destination rejection. The desktop model is tested with mock hardware:
no scan without loaded-paper acknowledgement, a lost response after job creation,
app relaunch, and retry while that job is active all retain a single job ID.

Existing service, database, CLI, HTTP/authentication and compiled MCP workflow
tests continue to pass. These automated tests use temporary storage and mock
scanner backends. They do not feed physical media.

## Running application checks

The real installed service was idle before its update. The application was
launched as a native bundle and inspected using macOS accessibility and
window-scoped screenshots, without a browser or web wrapper.

Verified in the running UI:

- Dashboard discovers **fi-8170**, shows the shared service and existing history.
- Driver-derived resolutions include 150, 200, 240, 300, 400, 500 and 600 DPI.
- The start control is disabled without the explicit paper-loaded acknowledgement.
- Library opens the existing real PNG duplex batch and displays both originals.
- Profile editor and capability/readiness screens open with actual service data.
- Dragging a crop rectangle updates its selection; saving creates a new PNG.
- A 90° rotation creates another PNG; both derivatives appear in the library.
- Native export dialog writes the selected processed PNG; exported bytes have the
  exact recorded SHA-256. The test export was moved into ignored Diagnostics.
- The application reconnects after the idle service is updated.

Source batch: `C993DDA4-DF86-454B-BC27-63E09E64B984`, originally scanned earlier.
The two source PNGs are still 2480 × 3508 pixels. Their SHA-256 digests after editing:

```text
page-000001.png  47f032f8a38c3a45b46a2b31ae50dde11af0988c0ded8614ff3a6ea800d72b4c
page-000002.png  22d5c9b0435d0614e224ebe851fc0e83a591407ed7eacaa3c49427cca6c24287
```

Saved derivatives:

| Edit | Output | SHA-256 |
| --- | --- | --- |
| 90° clockwise rotation | 3508 × 2480 PNG | `60915ef21d6fad8e32d045ff226c77ece420939d55ba794200937116e8ce35bf` |
| Dragged crop, about 79% of each dimension | 1959 × 2775 PNG | `eab4fe8f0c00b98815c3a86af0d56ebd65a1a5a7442d6b1e6d592e02fd8b5bc4` |

The rotation export has the first derivative's exact digest. Originals were not
overwritten. The live ledger still contains the same three pre-existing scan
jobs, with no active job or queued job. No new physical scan was requested in
this milestone implementation. Runtime evidence is retained under ignored
`Diagnostics/milestone-6/`; image data, credentials and database files are not
committed.

The HTTP LaunchAgent configuration's SHA-256 was unchanged across the service/app
update, preserving the existing authenticated Tailscale listener.

## Physical checks after the initial implementation

A subsequent [SMB storage validation](storage-migration-validation.md) completed a
new duplex PNG scan from the native GUI, including progress, completion and both
previews. Cancellation during an active physical scan still needs validation.
Notification permission/delivery has also not been exercised; notification opt-in remains under the user's control.
Do not treat the earlier CLI/MCP paper scans as a native GUI hardware test.

The original supervised-check procedure is retained here for future regression tests:

1. Remove cards. Load one expendable A4 sheet marked FRONT/BACK.
2. Open Scanner, choose fi-8170, 300 DPI, PNG and Duplex.
3. Confirm paper is loaded using the checkbox, then deliberately click Start scan.
4. Confirm progress, saved image count, completion, both previews and feeding condition.
5. A separate cancellation check needs another freshly confirmed loaded sheet.

Card feeding, inferred front/back pairing, card boundary detection, automatic
cropping/deskew and the remote web GUI remain later milestones. Adjacent-image
comparison makes no front/back identity claim. The app exposes current engine
settings and inspects the complete driver report; unsupported engine setters are
not presented as working controls.
