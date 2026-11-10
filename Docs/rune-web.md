# Rune desktop and browser UI

The same Rust UI uses the authenticated service API on desktop and in the browser.
Browser entry code is limited to loading wasm-bindgen output; application state,
network requests, scan controls and reviews are Rust.

Build the browser bundle from the repository root:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
cargo build -p scanner-ui -p scanner-worker --lib --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir target/rune-web target/wasm32-unknown-unknown/release/scanner_ui.wasm
wasm-bindgen --target web --no-typescript --out-dir target/rune-web target/wasm32-unknown-unknown/release/scanner_worker.wasm
cargo run -p scanner-http --example web_bundle -- target/rune-web
```

Use the wasm-bindgen CLI version matching Cargo.lock. The packaging example copies
the bootstrap and full Inter faces, includes Rune's generated JavaScript helpers,
and writes a SHA-256 manifest. Inter's font licence ships with the bundle; the
fonts are copied from the pinned Rune revision's `crates/rune/assets` directory.
The browser build is single-threaded, uses WebGPU when available and includes
Rune's WebGL2 fallback. Do not enable wasm atomics.

Set `webDirectory` to the absolute bundle directory in the service configuration.
The service verifies and loads the bundle before listening. A restart is required
after replacing assets. Only manifest-listed, bounded files are served, with
ETags, MIME types, a restrictive content security policy and no credential in
URLs. API operations still require a Bearer token. The browser connects to the
same origin; it keeps its access token in memory, without cookies or browser
storage. The UI supports reconnecting with a new credential after expiry.

For the desktop, run `cargo run -p scanner-ui`. Enter a service origin and token,
or set `SCANNER_UI_ENDPOINT` and an explicit private `SCANNER_TOKEN_FILE`. The
`SCANNER_UI_MANIFEST` saved-fixture scene is a separate, read-only development mode.

An isolated browser test server uses a synthetic card, temporary storage and an
in-memory ledger. It never constructs a hardware driver:

```sh
cargo run -p scanner-http --example web_fixture -- target/rune-web ABSOLUTE_PRIVATE_TOKEN_FILE TAILSCALE_IP:8194
```

The fixture creates its private token file exclusively on first use and reuses it
on subsequent runs. Stop with Ctrl+C to drain requests and remove temporary card
storage. This development fixture does not replace normal service ownership,
startup recovery or platform hardware checks.

Run the development browser flow against that fixture with:

```sh
npm ci --prefix Tools/NativeUIWebTests
node Tools/NativeUIWebTests/flow.mjs SERVICE_ORIGIN ABSOLUTE_PRIVATE_TOKEN_FILE OPTIONAL_SCREENSHOT_DIRECTORY
```

Playwright is a test dependency only. The flow enters the token through Rune's
secure input, loads service thumbnails, changes finish/condition and confirms the
synthetic card through the real API. It checks for JavaScript errors and verifies
that neither browser storage nor visible text contains the credential. The first
runtime check used Chromium's WebGL2 fallback over the machine's Tailscale address;
it is not a WebGPU or Linux/Windows hardware test.

The camera manifest installs `/camera` with a dedicated scope and generated PNG
icons. The service worker's navigation handler is Rust/WebAssembly. Packaging
embeds its small WASM module in the registered module script so listener setup is
synchronous, including offline worker restarts. A failed camera navigation shows
an offline explanation; API requests, camera frames and image downloads are not
intercepted. No CacheStorage, IndexedDB or browser storage is created. Browser
installation policy can disable registration without blocking online camera use.

The UI embeds an unmodified, pinned Noto Sans KR fallback alongside Inter, so
Korean card names render without system fonts or a CDN. Its SIL Open Font License
ships in the verified web bundle; provenance and SHA-256 are in
`crates/scanner-ui/assets/README.md`. The synthetic browser fixture includes Korean
text, and its rendered screenshot was checked. `startup.mjs` exercises three cold
browser contexts. Startup diagnostics preserve the HTTP status, console failures
and a screenshot if the first Rune frame does not appear.
