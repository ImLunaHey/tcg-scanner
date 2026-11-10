# Native Rune application

`scanner-ui` is the Rust desktop application on macOS, Linux and Windows. It uses
the same Rune components and authenticated API as the WASM browser interface.
There is no SwiftUI, React, Electron or Tauri application runtime.

The desktop provides batches, scanning/continuation, saved-card preparation and
identification, explicit identity/finish/condition review, source-bound visible
mark regions, paired-side comparison, image editing, inventory, catalogue and
reference lookup, pricing, exports, Store transfers, profiles and scanner settings.
Inspection reports preserve unknown/unsupported capabilities and driver evidence.

An installed executable reads the adjacent private `service.json` and token.
Development builds accept `SCANNER_UI_ENDPOINT` and `SCANNER_TOKEN_FILE`; without
those, the connection screen accepts a service URL and in-memory token. The
`SCANNER_UI_MANIFEST` fixture scene is read-only development support.

Run `cargo run -p scanner-ui` for development. Follow [portable installation](portable-install.md)
for packaged builds and startup. The service must already be running; the UI
never launches it as a helper. Camera capture is the browser `/camera` route.
