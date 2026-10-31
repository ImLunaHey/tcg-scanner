# Remote web workspace

The React/Vite/TypeScript application uses Tailwind CSS, TanStack Query and Wouter
hash routes. The existing Swift HTTP gateway serves its production assets and
forwards every scanner operation over the private service socket. Node is required
for building and tests, not for running the installed dashboard.

## Installation and access

Install ScannerService and the authenticated HTTP LaunchAgent first, configure
[stable signing](signing.md), then run from the repository root:

```sh
Scripts/install-web.sh
```

This builds the web bundle and signs/updates only the HTTP gateway. It preserves
the installed gateway's bind address, port, token file, public origin and remote
access flags. Static assets are installed in a new versioned directory under
Application Support. The previous executable and plist are backed up before the
gateway restarts. ScannerService, the native app and active scan jobs are not
restarted. Keep the old asset directories for rollback; they contain no scans.

For this installation, visit **http://100.117.220.119:8170/** while connected to
Tailscale. Resolve the current address with `tailscale ip -4` if it changes.
Use the root URL for the login page, rather than an `/api/v1/` endpoint.

On the scanner Mac, copy the existing private access key without printing it:

```sh
pbcopy < "$HOME/Library/Application Support/Scanner/http/token"
```

Paste it into the login form. Signing in exchanges it for a revocable one-hour
Bearer session; the root key is cleared from the form and never stored in browser
storage. Only the session token/expiry are stored in this tab's sessionStorage.
Sign out revokes that session and clears the query cache. Restarting the HTTP
gateway invalidates sessions, so sign in again after gateway updates. Credentials
are never placed in URLs, cookies, asset bundles or server logs.

The configured HTTP address is carried inside the encrypted Tailscale connection.
Do not expose the port publicly. For browser HTTPS, the existing gateway also
supports Tailscale Serve with an exact configured HTTPS public origin; see
[HTTP deployment](http-api.md). Tailscale Serve is not required or enabled by
the web installer.

## Available workflow

- Dashboard: connection state, scanners, shared queue, recent batches and storage.
- Capture: scanner/profile selection, discovered supported DPI and duplex,
  PNG/TIFF, defaults for future jobs, readiness acknowledgement, start/cancel.
- Live updates: authenticated fetch-based SSE with cursor reconnection and reset
  handling. Polling continues during stream interruptions. Discovery/inspection
  never starts a scan.
- Profiles: create, edit, duplicate and delete, shared with native/CLI/MCP clients.
- Library: paginated history, live capture thumbnails, original/edited downloads,
  selected-image comparison, complete JSON manifests and batch ZIP export.
- Optional manual copies: percentage crop coordinates and clockwise rotation,
  processed by the existing native image service. No automatic image processing
  runs; originals remain untouched. The default workflow is scan-only.
- Scanner details: capability values, ranges, defaults, evidence, vendor fields,
  unknowns and read-only status; report download. Unsupported driver fields remain
  inspection-only rather than pretending the engine can configure them.

Start requests use stable UUID idempotency keys. A lost/ambiguous response retains
the exact request in sessionStorage and offers an explicit retry of that request.
The UI never retries a scan automatically. Repeated submissions resolve to the
same service job. Configuration controls are disabled while the scanner is busy.

Current engine limitations remain A4 RGB 8-bit capture, unverified card feeding,
and unassigned physical sides. Comparison does not infer front/back pairs.
Files use `B-001/C-001.png` until pairing is validated. The browser ZIP export
includes originals, retained manual edits and the manifest; it preserves file
bytes and runs archive creation in a worker. Its 64 MiB input limit prevents
excessive mobile memory use. Download individual files or use native export for
larger batches. Thumbnails are fetched serially per tab to respect gateway limits.

## Security boundaries

Only `/`, `/index.html`, `/favicon.svg` and the installed `assets/` files can be
served without authentication. Assets are read into a bounded immutable snapshot
at startup; the public file handler cannot browse scan storage, tokens or arbitrary
filesystem paths. Symlinks, unexpected files, oversized assets and traversal paths
are rejected. Host/origin checks also apply to public assets. All APIs, original
images, edits, thumbnails and SSE retain Bearer authentication and resource limits.

The web shell has a restrictive CSP: scripts/stylesheets/workers only from the same
origin, renderer style attributes for WebKit compatibility, blob image previews,
no external scripts, no framing and no inline script
execution. API and asset responses use `no-store`; there is no service worker or
offline image cache. Cookie credentials are not accepted, avoiding cookie-based
CSRF. CORS is not enabled.

## Build and tests

Use a current supported Node release (this installation uses Node 24) and Swift 6:

```sh
npm --prefix Web ci
npm --prefix Web run build
npm --prefix Web test
swift test --disable-index-store -Xswiftc -warnings-as-errors
```

The browser fixture is opt-in and uses a Swift mock backend through the actual
Unix socket and HTTP gateway. It cannot access scanner hardware. In one terminal:

```sh
export SCANNER_BROWSER_HOST="$(/Applications/Tailscale.app/Contents/MacOS/Tailscale ip -4)"
SCANNER_BROWSER_FIXTURE=1 SCANNER_BROWSER_ASSETS="$PWD/Web/dist" \
  swift test --filter serveBrowserFixture --disable-index-store
```

After its port 8171 responds, run in another terminal with the same host variable:

```sh
cd Web
npx playwright install chromium webkit
npm run test:e2e
```

Stop the fixture with Ctrl-C when finished. Its `eeee…` fixture credential is
deliberately separate from production. Screenshots/traces and build outputs are
ignored. The fixture tests cover desktop, phone and tablet layouts, Chromium and
WebKit, authentication, CRUD, lost-response idempotency, cancellation, thumbnails,
manual edits, comparison, byte-verified ZIP exports, reconnection and revocation.
Viewport emulation is not a claim of testing on physical iPhones/iPads.
