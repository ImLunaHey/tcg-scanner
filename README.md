# Scanner

A Rust application for scanning and cataloguing trading cards on macOS, Linux and
Windows. [Rune](https://github.com/anomalyco/rune) provides the native desktop
app. The original React web interface keeps its established layout and mobile
camera. The service, image processing, recognition,
library, HTTP API, CLI and MCP server run in Rust, with native libraries loaded
in-process for scanner drivers, Tesseract OCR and optional ONNX inference.

The Swift/SwiftUI implementation has been retired.
There is no macOS Vision dependency. macOS acquisition still uses the system's
ImageCaptureCore API through Rust; Linux uses SANE and Windows uses TWAIN.

## Build and run

Use Rust 1.98 and the platform's C/C++ development toolchain. Linux desktop builds
also need pkg-config, X11/Wayland, xkbcommon and udev development packages.

```sh
cargo build --workspace --bins --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

For an installable native/web distribution, dependencies, startup registration and
existing-library cutover, follow [portable installation](Docs/portable-install.md).
`scanner-setup` creates a new private installation without replacing live data.
Launch `scanner-service serve --config /absolute/service.json`, then the adjacent
`scanner-ui`. The CLI and MCP server connect to the authenticated HTTP service.
A shared per-user owner lock prevents two services from accessing scanner hardware.

## Workflows

- ADF/duplex scanning with persisted settings, ordered originals, cancellation,
  recovery, profiles, defaults and batch continuation.
- Immutable originals, SHA-256 verification, bounded crop/rotate/deskew edits,
  thumbnails, paired-side comparison and visible-condition review regions.
- Portable Tesseract recognition for Pokémon English/Korean, Yu-Gi-Oh! and Star
  Wars Unlimited, with explicit ambiguity, printed cues and catalogue retries.
- Optional local DINOv3 artwork retrieval with separate, explicitly reviewed
  suggestions. Printed identity, finish and condition remain distinct decisions.
- Inventory, catalogue/reference lookup, GBP market guides, reviewed finish and
  condition, verified exports and explicit Store transfers.
- Browser camera lookup and installable camera page, with frames kept in
  memory. Remote camera access requires HTTPS.
- Private HTTP credentials, JSON-RPC/REST/MCP, storage reconnection, pause/resume,
  native power notifications and opt-in scanner-button handling.

Recognition is conservative. Saved-card tests show lower Tesseract recall than the
former Apple Vision path; DINOv3 improves available review suggestions but does not
automatically verify a printing. The reference index must actually contain the
printing. See [recognition results and limits](Docs/visual-recognition.md) and the
sanitized reports in [Tools/RecognitionBench](Tools/RecognitionBench).

## Architecture

Twenty workspace crates separate typed contracts, persistence, safe files, image
processing, recognition, catalogue/pricing, device adapters, coordination,
application interfaces and Rune desktop presentation. SQLite and v1 saved
manifests remain
compatible. Runtime code does not invoke Swift, Python, Node, shells or scanner
helper executables. The original React web interface keeps its established
design and uses the Rust
HTTP API; Node is only a development/build dependency. The C files under
`Tools/NativeDeviceTests` simulate native driver ABIs for tests. Legacy experiments
remain in Git history.

| Area | Source / guide |
| --- | --- |
| Native application and setup | [scanner-app](crates/scanner-app), [installation](Docs/portable-install.md) |
| Rune desktop | [scanner-ui](crates/scanner-ui), [optional WASM build](Docs/rune-web.md) |
| Web workspace and camera | [Web](Web), [web build](Docs/web-app.md) |
| Driver ownership and lifecycle | [scanner-core](crates/scanner-core), [scanner-device](crates/scanner-device), [power](Docs/native-power.md) |
| Recognition and visual index | [scanner-recognition](crates/scanner-recognition), [DINOv3](Docs/visual-recognition.md) |
| API / MCP | [scanner-rpc](crates/scanner-rpc), [scanner-http](crates/scanner-http), [MCP setup](Docs/mcp.md) |
| Migration evidence | [migration ledger](Docs/rust-migration.md), [saved-card reports](Tools/RecognitionBench/README.md) |

## Validation boundaries

CI builds/tests the Rust workspace on macOS, Linux and Windows and compiles the
optional Rune UI/worker to WASM. Distributions ship the original web interface.
Native SANE/TWAIN fixtures exercise real ABI calls with
simulated devices. Saved scans test preparation and recognition without modifying
the originals. HTTPS Playwright tests exercise web review, editing, inventory,
profiles, exports, camera lifecycle and installation behavior.

Actual Linux/Windows scanner hardware, vendor backing-colour/ICC behavior,
physical suspend/resume and physical phone installation still need device testing.
These are not established by compilation or simulated drivers. The installed
legacy service and live ledger are not automatically cut over by this repository
change; [cutover instructions](Docs/portable-install.md#existing-library-cutover)
keep that operational step explicit. CI artifacts are unsigned development builds.
