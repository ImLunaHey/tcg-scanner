# TCG Scanner

A native macOS scanning platform for trading cards and documents, built in Swift
and initially tested with the **Ricoh fi-8170 over USB on Apple Silicon**.

Scan through a SwiftUI desktop app, a remote web dashboard, a Swift CLI, or an MCP
client. One background service owns the scanner, schedules jobs, and preserves
original images. The desktop app does not need to be open.

## What it does

- **600 DPI scanning:** simplex or duplex capture, lossless PNG by default, with
  TIFF available. The service enforces 600 DPI for every new job, including jobs
  started with the scanner's physical Scan/Enter button.
- **Card preparation:** automatic boundary detection, cropping, deskew and
  orientation, with front/back previews and manual image adjustments. Original
  captures remain unchanged.
- **Pokémon recognition:** local Apple Vision OCR with TCGdex card and set
  matching. English and Korean are selectable; Korean catalogue coverage is limited.
- **Review and valuation:** experimental finish estimates, candidate border-wear
  findings, and Cardmarket prices converted to GBP for cards and batches.
- **Batch management:** permanent batch numbers, image metadata, manifests,
  previews and exports, backed by SQLite and files on disk.
- **Remote control:** authenticated HTTP API, live events and a responsive React
  dashboard with system light/dark appearance and image zoom.
- **Store integration:** optional send/resync to a separate Store application's
  staging inbox, preserving batch/card references for review before approval.

Image analysis runs locally on the Mac. Catalogue lookups, reference-image
retrieval and pricing use external services; the optional Store integration
explicitly transfers selected batch images and metadata.

## Current scope

This is actively developed, hardware-tested software, not a claim of support for
every ICA scanner or trading-card game.

- The **fi-8170** is the tested scanner. Settings are discovered through Apple's
  ImageCaptureCore and the installed official ICA driver; Windows PaperStream
  features are not assumed to exist on macOS.
- **Game detection uses each card’s back.** Standard Pokémon and English TCG
  Yu-Gi-Oh! backs determine game and side labels automatically, without a selector.
  Unknown or ambiguous pairs skip identification. English Yu-Gi-Oh! fronts use
  local OCR and a cached YGOPRODeck catalogue for names, printed set codes and
  catalogue rarities. Yu-Gi-Oh! finish/condition assessment, pricing and Store
  export are not implemented. See [Yu-Gi-Oh! identification](Docs/yugioh-identification.md).
  See [game detection and validation](Docs/game-detection.md).
- Identity and finish results are suggestions. Condition checks inspect selected
  features of standard Pokémon backs; they do **not** establish a selling grade
  or reliably detect every scratch, dent or crease.
- Duplex captures are grouped consecutively. Front/back labels use recognition
  evidence, so uncertain sides still need review.
- Card tests have included 600 DPI duplex batches with user-confirmed clean
  feeding. They do not establish that every card, sleeve or modified feeder is
  safe. Validate new media with expendable samples. The service does not change
  multifeed detection, paper-thickness or transport-safety settings.

See the [hardware findings](Docs/hardware-discovery.md),
[card preparation](Docs/card-preparation.md),
[recognition](Docs/card-identification.md), and
[condition-check limitations](Docs/condition-checks.md).

## Requirements

- macOS 14 or later and Swift 6.2 or later; Xcode supplies the Apple frameworks.
- The scanner's official macOS ICA driver.
- Node.js and npm to build or develop the web UI. Node is not needed to serve the
  installed production dashboard.
- A [persistent signing identity](Docs/signing.md) for the installation scripts.
  Plain Swift builds and automated tests do not require one.

Hardware validation has used Apple Silicon. Other scanner models and macOS
configurations require their own validation.

## Get started

### Build and inspect

```sh
git clone https://github.com/ImLunaHey/tcg-scanner.git
cd tcg-scanner
swift build
swift test
```

Run the service in one terminal:

```sh
.build/debug/scanner-service --verbose
```

Use another terminal to inspect the scanner:

```sh
.build/debug/scanner devices --json
.build/debug/scanner capabilities --json
.build/debug/scanner device --json
.build/debug/scanner diagnostics --json
```

Do not run a foreground service alongside an installed service. Discovery and
diagnostics do not initiate scanning. Hardware inspections require an idle scanner;
job status remains available during capture.

### Scan

First validate ordinary paper feeding. Each scan feeds until the ADF is empty or a
limit is reached; it does not feed just one item unless only one is loaded.

```sh
# Loaded documents: duplex PNG, without card processing.
.build/debug/scanner scan --dpi 600 --duplex --format png --no-wait --json

# A separate run, only after loading suitable cards:
.build/debug/scanner scan --duplex --prepare-cards --card-language en --no-wait --json
```

Use `--card-language ko` for Korean cards. Monitor or cancel the returned job:

```sh
.build/debug/scanner jobs status JOB_UUID --json
.build/debug/scanner jobs cancel JOB_UUID --json
.build/debug/scanner batches list --json
.build/debug/scanner batches manifest JOB_UUID > manifest.json
```

Use `--request-id UUID` with identical inputs when retrying a lost response. A new
UUID creates a new physical scan. Closing a client or interrupting its status wait
does not cancel the service-owned job.

Profiles and device defaults supply scan options; the service applies the 600 DPI
policy after resolving them. Run `scanner --help` or a subcommand's `--help` for
profiles, configuration, image downloads and other operations.

### Install the background service and interfaces

After configuring [signing](Docs/signing.md), a fresh installation can use:

```sh
Scripts/install-launch-agent.sh

# Optional native SwiftUI application:
Scripts/install-mac-app.sh
open "$HOME/Applications/Scanner.app"

# Optional HTTP gateway and web dashboard:
Scripts/install-http-agent.sh
Scripts/install-web.sh
```

The service runs as a per-user LaunchAgent and installs the CLI at
`~/Library/Application Support/Scanner/bin/scanner`. Installation does not start a
scan. For existing installations, follow the
[service operations](Docs/service-operations.md) and interface-specific update
instructions rather than rerunning fresh-install commands over running services.

The web dashboard is served at `http://127.0.0.1:8170` by default. Its application
shell is public; scanner data and controls require authentication. The installer
creates a private access key. To copy it on the scanner Mac:

```sh
pbcopy < "$HOME/Library/Application Support/Scanner/http/token"
```

For remote use, configure authenticated HTTPS access through Tailscale as described
in [HTTP operations](Docs/http-operations.md). Do not expose unrestricted control
to the public internet.

| Interface | Setup and reference |
| --- | --- |
| Native SwiftUI app | [Native application](Docs/native-app.md) |
| React web dashboard | [Web application](Docs/web-app.md) |
| CLI and background service | [Service operations](Docs/service-operations.md) |
| MCP over stdio | [MCP configuration and tools](Docs/mcp.md) |
| Physical Scan/Enter button | [Button configuration](Docs/scanner-button.md) |
| HTTP and local JSON-RPC | [HTTP API](Docs/http-api.md) · [Local API](Docs/local-api.md) |

## Storage and recovery

The default image root is `~/Scans`. A configured SMB share is also supported;
SQLite metadata remains local. All clients use the service's configured storage
root, and remote clients cannot write to arbitrary filesystem paths.

```text
Scans/
  B-001/
    manifest.json
    C-001.png
    C-002.png
    edit-<UUID>.png
    .incoming/
```

Batch numbers increase permanently; dates stay in metadata. Original filenames
refer to acquisition order, while prepared copies and front/back relationships
are recorded in metadata. Images remain directly inside the batch folder rather
than separate `originals/` and `processed/` directories. `.incoming/` retains
original driver deliveries and may contain duplicates or partial files.

Manifests update atomically and saved originals are checksum-verified. Interrupted
jobs are recovered or marked failed; startup never resumes physical feeding.
Failed scans retain received files, but a partial batch must not be mistaken for a
complete one. Back up both the images and the local database, which holds the
batch-number ledger and scan history. See [storage configuration](Docs/storage.md).

Current limits include 100 image callbacks per scan, 120 seconds without a new
image, and a 30-minute capture limit. The callback ceiling cancels the job when
reached, so keep batches **below 50 duplex cards**. Only one hardware session is
active at a time across the current backend. There is no automatic deletion of
scan history or originals.

## Architecture

All interfaces communicate with ScannerService. The native app, CLI and MCP server
use a private Unix-domain socket; the Hummingbird HTTP gateway forwards web/API
requests through that same service. Clients never open their own scanner sessions.

| Component | Responsibility |
| --- | --- |
| `ScannerAPI`, `ScannerClient`, `ScannerTransport` | Shared contracts and local IPC |
| `ScannerCore`, `ScannerService`, `ScannerServiceSupport` | Hardware access, queue, lifecycle and API routing |
| `ScannerPersistence` | GRDB/SQLite metadata, migrations and recovery |
| `ScannerImageProcessing`, `ScannerRecognition` | Native image preparation, OCR and card analysis |
| `ScannerCLI`, `ScannerMCP`, `ScannerMCPSupport` | Headless and agent interfaces |
| `ScannerHTTP`, `ScannerHTTPSupport` | Authenticated HTTP, downloads and live events |
| `ScannerMacUI`, `Apps/ScannerMac` | Native SwiftUI desktop application |
| `Web` | React, Vite, TypeScript, Tailwind CSS and TanStack Query |

The native stack uses Swift concurrency, ImageCaptureCore, Core Image, Vision and
ImageIO. The macOS app is native SwiftUI, with no Electron, Tauri or web wrapper.

## Development and testing

```sh
swift test --disable-index-store -Xswiftc -warnings-as-errors
npm --prefix Web ci
npm --prefix Web test
npm --prefix Web run build
```

Swift Testing covers mock scanners, scheduling, persistence, local sockets, CLI,
MCP and HTTP integration. Automated tests do not feed the physical scanner.
Hardware validation and experimental findings are documented separately in
[`Docs/`](Docs/). Some documents are historical milestone reports; use current
source and this README for current defaults and limits.

Build outputs, local diagnostics, databases, credentials and scanned card images
are not part of the repository.
