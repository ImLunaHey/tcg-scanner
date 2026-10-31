# Native macOS application

Scanner is a SwiftUI application for macOS 14+ (currently built and validated on
Apple Silicon). It talks to the per-user ScannerService through its protected
Unix socket. It has no ImageCaptureCore dependency, does not start a scanner
backend, and does not require the HTTP gateway or a bearer token. Quitting the app
does not stop the service or an accepted scanning job.

## Install and launch

Install/update ScannerService first; an older service cannot save image edits.
For an existing installation, while no physical job is active:

```sh
Scripts/uninstall-launch-agent.sh
Scripts/install-launch-agent.sh
# Quit any running Scanner app before updating it:
Scripts/install-mac-app.sh
open "$HOME/Applications/Scanner.app"
```

These scripts do not start scans. Service updates retain the database and images.
The app installer does not change HTTP/Tailscale configuration. This is a local,
ad-hoc-signed development installation; notarized distribution and App Store
sandboxing are not implemented. The same macOS user must run the service and UI.

For development:

```sh
Scripts/build-mac-app.sh
open .build/Scanner.app
open Apps/ScannerMac/ScannerMac.xcodeproj
# Reproducible Apple Silicon Xcode build:
xcodebuild -project Apps/ScannerMac/ScannerMac.xcodeproj -scheme ScannerMac \
  -configuration Debug -derivedDataPath .build/xcode \
  -clonedSourcePackagesDirPath .build CODE_SIGNING_ALLOWED=NO build
```

The Xcode application target and SwiftPM executable use the same SwiftUI library.
The normal app bundle is `Scanner.app`; its executable is `ScannerMac`.

## Workflow

- **Dashboard:** select a scanner and a shared profile or custom settings. DPI
  choices come from the selected driver's ADF capability report, intersected with
  the engine's 150–600 DPI range. Duplex is enabled only when advertised.
  A4, RGB colour and 8-bit channels remain fixed engine constraints.
- **Start:** load ordinary paper, acknowledge feeder readiness and click Start scan.
  A request UUID is retained across lost replies/app relaunches. Retry resubmits
  the same request, preventing another job with the same ID. Discovery and
  opening the app never initiate a scan.
- **Progress:** saved-image counts and previews update through the event API;
  there is no fabricated percentage when the ADF total is unknown. Cancel is
  asynchronous. Reconnection retries automatically. Service-owned jobs survive
  app closure.
- **Profiles:** create, edit, duplicate, delete and select shared presets. The
  New profile menu offers General documents, Archival and a Pokémon placeholder.
  The Pokémon preset is explicitly unvalidated and still uses A4 dimensions;
  card media handling and processing belong to the later card milestone.
- **Scanner:** inspect readiness, device defaults, all reported standard settings,
  vendor features/templates, allowed values, defaults where known, evidence and
  unknown capabilities. Unimplemented setters remain inspection-only. No media
  thickness or multifeed safety setting is written. Readiness is a snapshot;
  physical conditions can change after inspection.
- **Library:** browse paginated history, view saved originals and processed PNGs,
  compare adjacent images, inspect failures, export a manifest or an entire batch.
  Adjacent images retain acquisition order; they are not automatically labelled
  front/back. Previews are available only after an original has been saved.
- **Edit:** drag a rectangle on the original, choose clockwise rotation, and Save
  new PNG. Cropping happens before rotation. Each edit starts from the original;
  originals and prior edits remain intact. A full-image edit with zero rotation
  also converts a TIFF original to PNG.
- **Export:** choose a destination in the native file dialog. Image chunks and
  final SHA-256 digests are checked. Existing files are refused, even if the save
  panel offered replacement. A batch export writes its manifest last; interrupted
  exports retain verified files without a completion manifest. Manifest image
  paths are relative to the exported batch; `outputDirectory` describes its
  original service location.
- **Notifications:** opt in under Scanner. Completion/error notifications apply
  to jobs observed while the app runs and respect macOS permission settings.

Thumbnails are limited to 512 pixels. Editing supports inputs up to 32 MiB and
40 megapixels, two concurrent edits globally, one edit per batch at a time and
eight saved versions per original. Output PNGs are limited to 64 MiB; thumbnail
sources are limited to 32 MiB. Oversized images can still be downloaded within
the existing 512 MiB image-transfer limit.

## Additional local API contracts

`v1.images.edit` takes exactly:

```json
{
  "id": "8ED1C970-3E72-48E0-B122-1A7A2308A99A",
  "jobID": "C993DDA4-DF86-454B-BC27-63E09E64B984",
  "sequence": 1,
  "crop": {"x": 0.1, "y": 0.1, "width": 0.8, "height": 0.8},
  "quarterTurns": 1
}
```

Coordinates are normalized in the oriented original, with a top-left origin.
All values must be finite and the rectangle must lie inside the image; rotation
is 0–3 clockwise quarter turns. Only terminal jobs can be edited. Identical IDs
and inputs return the existing result; changed inputs with an existing ID fail.
No filesystem path is accepted. Processing uses Core Image and ImageIO in a
bounded background task, and source SHA-256 is verified before rendering.

The result is a `ProcessedImage` containing the request, timestamp, source digest
and generated image metadata. A nullable `processedImages` array is added to
`ScanRecord`/batch manifests, retaining compatibility with existing records.
Files are named `processed/<edit-uuid>.png`, with a JSON recovery sidecar. Files
are created exclusively, synced, recorded in SQLite, then the batch manifest is
atomically published. A repeated request can repair the manifest or recover a
complete sidecar after a crash before the ledger commit. Partial files are
retained and produce a storage error; they are never silently overwritten.

`v1.images.get`, `v1.images.read`, and `v1.images.thumbnail` accept an optional
`editID` UUID in addition to `jobID` and `sequence`. Omitting it returns the
original. Every path component is opened relative to the configured storage root
with symlinks rejected. These new edit operations are currently available through
the local API/native app; HTTP and MCP editing tools are not yet exposed.

The service remains the source of truth for profiles, jobs, image edits and
metadata. `ScannerClient` supplies asynchronous typed IPC and verified local
exports; `ScannerMacUI` supplies presentation and user intent only.

Apple references: [NavigationSplitView](https://developer.apple.com/documentation/swiftui/navigationsplitview),
[Core Image cropping](https://developer.apple.com/documentation/coreimage/ciimage/cropped(to:)),
[ImageIO destinations](https://developer.apple.com/documentation/imageio/cgimagedestination),
[notification authorization](https://developer.apple.com/documentation/usernotifications/unusernotificationcenter/requestauthorization(options:completionhandler:)).
