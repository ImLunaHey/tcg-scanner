# Web workspace

The original React interface is the production web UI. Rune is the desktop app;
the optional Rune WASM build is not installed as the public web interface.
The Rust service serves the web assets and implements the existing REST API.
The original dashboard, navigation, card grids, styling, responsive layout and
fullscreen camera are preserved. A persistent native `<video>` displays the camera
stream; recognition samples a separate canvas without replacing the preview.

Build and verify from the repository root:

```sh
cd Web
npm ci --ignore-scripts
npm test
npm run build
cd ..
cargo run -p scanner-http --example web_bundle --locked -- Web/dist --existing
```

Point the service's `webDirectory` at that verified output. Distribution packaging
uses `scanner-setup package --binaries /absolute/binaries --web /absolute/Web/dist
--output /absolute/new-package`. The running service loads immutable verified
assets on startup, so changing the bundle requires a service restart.

The library uses an expiring per-tab session; the public camera requires no token.
Successful camera matches are retained in on-device IndexedDB as Recent scans.
Prices and binder-page detection are public lookups; adding reviewed captures to
inventory requires a session. See [camera library](camera-library.md) for storage,
pricing and binder details. HTTPS is required for remote camera access. Node is
needed only to build/test assets.

For browser integration tests, run the Rust `web_fixture` example with a private
token containing 64 `e` characters and set `SCANNER_BROWSER_URL` to its Tailscale
URL, then run `npm run test:e2e` in `Web`. The fixture has synthetic saved cards
and no physical scanner. Never run the mutating fixture suite on the live service.
