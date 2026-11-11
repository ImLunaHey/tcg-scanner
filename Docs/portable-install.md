# Portable Rust installation

Scanner ships six native Rust executables plus the Rune WebAssembly browser bundle:
`scanner-service`, `scanner-ui`, `scanner`, `scanner-mcp`, `scanner-index`, and
`scanner-setup`. The same source supports macOS, Linux and Windows. Native device
integration is ImageCaptureCore, SANE and TWAIN respectively. No Swift, Node,
Python or command-line scanner helper is used by the application.

## Install a distribution

Extract the artifact for your OS/architecture. Install into a **new absolute
private directory**; existing installations and libraries are never overwritten.
The destination contains executables, web assets, `service.json`, private token,
and a separate local SQLite ledger. Scan storage must already exist or be a share
that you reconnect yourself. A disconnected required share never becomes local
fallback storage.

```sh
scanner-setup install --package /absolute/extracted-package \
  --destination /absolute/new-scanner-install \
  --storage /absolute/scans --required-smb-share smb://lake/scans
scanner-setup doctor --config /absolute/new-scanner-install/service.json
```

Use drive-qualified paths and `.exe` names on Windows. Omit
`--required-smb-share` for deliberately local storage. `--library-only` creates a
saved-library service without hardware discovery; it is useful for isolated
validation. The installer does not start a process or register startup implicitly.
An interrupted installation is left for inspection and is never used to replace
an existing install. Use another new destination after correcting the error.
Distribution hashes detect corruption; they are not publisher signatures.

Install Tesseract 5 and English `eng.traineddata`; add `kor.traineddata` for Korean.
The setup tool checks conventional Homebrew, Linux and Windows locations and the
package's `lib` / `tessdata` directories. Override discovery using absolute
`--tesseract-library`, `--trained-data` and `--driver-library` arguments. Windows
needs a matching architecture TWAIN DSM and scanner driver; Linux needs SANE and
permissions for its device. Shared-library dependencies must also be installed.
Model/runtime assets are optional and separately configured: see
[visual recognition](visual-recognition.md). DINO suggestions require a matching
local public reference index; the installer does not download models or scans.

Start the installed service explicitly after stopping any previous owner:

```sh
/absolute/new-scanner-install/scanner-service serve \
  --config /absolute/new-scanner-install/service.json
```

Launch the adjacent `scanner-ui` executable; it loads the adjacent private
configuration and credential automatically. The native UI never saves a token
in browser storage. Development builds can use `SCANNER_TOKEN_FILE` and
`SCANNER_UI_ENDPOINT`. The web library at `http://127.0.0.1:8171` uses the original web interface
and an expiring per-tab session created with the private token. Camera access from another device requires
HTTPS and an explicitly configured remote origin; see [web workspace](web-app.md).

For startup at the next login:

```sh
scanner-setup enable-startup --config /absolute/new-scanner-install/service.json
scanner-setup disable-startup --config /absolute/new-scanner-install/service.json
```

Registration uses a macOS user LaunchAgent, a Linux systemd user unit enabled in
`default.target.wants`, or the Windows current-user Run registry key. It never
invokes `launchctl`, `systemctl`, PowerShell or a shell. Linux requires a systemd
user session. Registration does not restart the running service. Disable removes
only a byte-for-byte matching registration; data and credentials are retained.
Windows logout/close cleanup is best effort within the OS termination deadline;
interrupted-work recovery remains mandatory on every platform.

## Existing library cutover

Stop the old scanner service and disable its startup before enabling Rust startup.
Keep a backup of the old database and configuration. Point Rust's `database` at
the existing compatible local ledger and `storage` at the same mounted share,
or first validate an isolated copy. Preserve Store configuration explicitly with
`storeConfiguration`; old HTTP credentials are not silently copied. The shared
hardware-owner lock prevents simultaneous Swift/Rust access. A service restart
recovers interrupted work but never resumes physical feeding automatically.

The repository migration does not perform this live cutover. Saved-image and
simulated-driver tests do not establish physical scanner, backing-colour,
ICC-profile, sleep/wake or phone parity. Real hardware validation is still needed
for each driver/platform. Tesseract's saved-card recall is lower than the former
Vision path; ambiguous readings remain unconfirmed. DINOv3 improves review choices
but does not establish collector number, finish, condition or price.

## Build a distribution

Rust 1.98 and the platform's normal C/C++ toolchain are required. Linux desktop
builds need pkg-config, X11/Wayland, xkbcommon and udev development packages.
The bundled Rune sources retain their MIT notice. The WASM binding generator must
match the locked `wasm-bindgen` version (currently 0.2.128).

```sh
cargo build --workspace --bins --release --locked
rustup target add wasm32-unknown-unknown
cargo build -p scanner-ui -p scanner-worker --lib --release \
  --target wasm32-unknown-unknown --locked
wasm-bindgen --target web --no-typescript --out-dir target/rune-web \
  target/wasm32-unknown-unknown/release/scanner_ui.wasm
wasm-bindgen --target web --no-typescript --out-dir target/rune-web \
  target/wasm32-unknown-unknown/release/scanner_worker.wasm
cargo run -p scanner-http --example web_bundle --locked -- target/rune-web
cargo run -p scanner-app --example package --locked -- \
  target/release target/rune-web target/scanner-distribution
```

The output directory must not exist. Build tooling is development-only; installed
applications never run compilers. CI produces artifacts separately for each
native platform. Artifacts are unsigned development builds; public distribution
still requires the publisher's signing/notarization/release process.
