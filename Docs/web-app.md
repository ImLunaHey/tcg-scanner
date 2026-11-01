# Remote web workspace

The React/Vite/TypeScript application uses Tailwind CSS, TanStack Query and Wouter
browser-history routes. The existing Swift HTTP gateway serves its production assets and
forwards every scanner operation over the private service socket. Node is required
for building and tests, not for running the installed dashboard.

App URLs use paths such as `/library` and `/batches/{jobUUID}`. The gateway
serves the public sign-in/app shell for these known routes on GET/HEAD, so direct
links and reloads work. Scanner data still requires authentication. API paths,
unknown paths and missing assets never fall back to the app shell. Existing
`#/` bookmarks for supported routes are replaced with the clean path before the
router mounts; ordinary page anchors are left alone.

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

For this installation, visit **https://pulsar.tail3275e2.ts.net:8443/** while
connected to Tailscale. The scanner gateway binds to `127.0.0.1:8170`;
Tailscale Serve supplies HTTPS on port 8443. Port 443 already serves another app.
Use the root URL for the login page, rather than an `/api/v1/` endpoint.

On the scanner Mac, copy the existing private access key without printing it:

```sh
pbcopy < "$HOME/Library/Application Support/Scanner/http/token"
```

Paste it into the login form. Signing in exchanges it for a revocable one-hour
Bearer session; the root key is cleared from the form and never stored by the
application in browser storage. The HTTPS login form supports password-manager
saving with username `scanner` and the access key as its password. Your password
manager controls whether it offers to save or autofill the credential. Only the session token/expiry are stored in this tab's sessionStorage.
Sign out revokes that session and clears the query cache. Restarting the HTTP
gateway invalidates sessions, so sign in again after gateway updates. Credentials
are never placed in URLs, cookies, asset bundles or server logs.

The configured HTTPS address is private to the tailnet, using
[Tailscale Serve](https://tailscale.com/docs/features/tailscale-serve); Funnel is
not enabled for this scanner endpoint. The gateway's exact `--public-origin` is
`https://pulsar.tail3275e2.ts.net:8443`, so authenticated browser requests pass its
existing Host/Origin checks. The older HTTP address is no longer the entry point.

HTTPS and password-manager support do not change session lifetime: sessions still
expire after an hour, disappear when the tab session is discarded, and are
invalidated by gateway restart. Use the same access key at the HTTPS address.
The web installer preserves the configured bind address and HTTPS origin.

The Serve route was added with:

```sh
/Applications/Tailscale.app/Contents/MacOS/Tailscale serve --bg --https=8443 --yes http://127.0.0.1:8170
```

To disable only this route, use `serve --https=8443 off`; do not reset the entire
Serve configuration. The existing port-443 route remains unchanged. Prior gateway
and Serve configurations are backed up under
`~/Library/Application Support/Scanner/backups/https-20261031T212214Z/`.

Validation used the normal browser/OS certificate trust store, without bypassing
TLS verification: login, authenticated API access, paired previews, cropped PNG
download checksum, mobile layout and logout passed. The API still rejects
unauthenticated requests. Password-manager field metadata was verified; a save
prompt in the user's particular browser has not been tested.

## Available workflow

Saved card fronts can also be [identified and reviewed](card-identification.md)
using local OCR and TCGdex suggestions, with separate manual finish and condition.

- Dashboard: connection state, scanners, shared queue, recent batches and storage.
- Capture: scanner/profile selection, discovered supported DPI and duplex,
  PNG/TIFF, defaults for future jobs, readiness acknowledgement, start/cancel.
- Live updates: authenticated fetch-based SSE with cursor reconnection and reset
  handling. Polling continues during stream interruptions. Discovery/inspection
  never starts a scan.
- Profiles: create, edit, duplicate and delete, shared with native/CLI/MCP clients.
- Library: paginated history, live capture thumbnails, original/edited downloads,
  selected-image comparison, complete JSON manifests and batch ZIP export. Duplex
  card jobs group adjacent captures into a card with both sides and one identity
  summary; missing sides and interrupted batches remain flagged. The front is
  prominent, the back smaller. Card panels show identity, finish, back-border
  checks and GBP price guides. Only actual flagged marks get a lightbox link;
  scan notes remain collapsible. Per-card manual and download controls are hidden.
- Optional manual copies: percentage crop coordinates and clockwise rotation,
  processed by the existing native image service; originals remain untouched.
- [Card preparation](card-preparation.md): **Auto-crop and orient cards** starts
  enabled in the scan controls; disable it for documents. Use **Prepare card**
  on an existing capture.
  Paired card batches export through **Export ZIP** and **Manifest** in the compact
  batch toolbar. Ungrouped images retain their individual export controls.
  Unique identity suggestions show their name, set and collector number directly
  on the capture tile, clearly distinguished from confirmed identities.
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

## Appearance and image inspection

Light/dark appearance follows the system/browser `prefers-color-scheme` setting,
including changes while the page is open. There is no theme toggle or stored
appearance override. Dark mode uses black and charcoal surfaces; light mode uses
neutral white/grey surfaces. Finish pills retain green/amber evidence states in
both modes. Scan pixels retain their original colours. The sidebar slogan and
batch back link are removed. Card prices sit at the top right of each summary;
the batch value and coverage sit beside the batch title, including on phones.
See [Cardmarket GBP pricing](card-pricing.md).

Click any library preview to open an authenticated, full-resolution lightbox.
Paired cards provide front/back switching in that same dialog. Use zoom buttons,
**Fit** or **100%**; scroll or drag with a mouse to pan, or swipe on touch screens.
Close with the close button or Escape. Keyboard focus stays in the dialog and
returns to the opening preview afterward. Closing/switching revokes temporary
image URLs. This is a view operation; it does not modify saved images.

### Preview loading — 2026-10-31

The tab now runs at most two image requests concurrently, prioritizing clicked
full-resolution images over queued thumbnails. A per-session 24 MiB thumbnail
cache avoids repeat downloads when returning to a batch. Originals/full-size
images are not cached there. Logout discards that API instance and its cache;
image URLs are revoked when views close. Competing clients retain bounded retries
for the gateway's existing global download limit.

ScannerService keeps a 32 MiB derived-thumbnail cache. Every cache lookup still
opens and validates the original's safe path, file type, size and filesystem
fingerprint; changed files invalidate previews. Original downloads retain their
existing checksums. Cache eviction never removes saved files.

On this Mac, B-009's 22 sequential preview requests previously took 8.8 seconds.
With the service cache warm, the updated browser displayed all 22 in 3.9 seconds;
returning to that batch in the same tab took 14 ms and made zero thumbnail requests.
These are local Tailscale measurements, not a guarantee for other network links.

Validation: 25 core/HTTP tests, eight frontend unit tests, TypeScript/Vite build,
Chromium desktop/phone and WebKit tablet workflows. Saved-batch HTTPS checks
verified 11 pairs, 22 previews, full-resolution 100% zoom, system themes, a
checksum-matching prepared download and unchanged job history. No scanner job
was started for these checks. Generated logs/screenshots remain ignored.

### Actual browser reload caching

The first preview-cache change above covered SPA navigation only. It did **not**
fix page reloads: the gateway's blanket `no-store` policy and the client's fetch
mode prevented the browser HTTP cache from retaining images. That distinction was
missed in the initial performance report and was corrected after user feedback.

Image responses now use `Cache-Control: private, max-age=3600, immutable`,
`Vary: Authorization` and content-derived ETags. JSON, authentication errors and
control responses remain `no-store`. Authentication runs before conditional
requests can return 304. The frontend permits cache reuse for immutable image
URLs; originals use a stable job/sequence and edits have unique IDs. Session
changes use a different Authorization value and cannot reuse another session's
cached representation. Browser cache data can remain on the device, as with
other downloaded images; private cache is not a shared network cache.

Gallery requests use `/thumbnail?format=jpeg` (512-pixel maximum, quality 0.82).
The existing thumbnail API still defaults to PNG for other clients. The additive
thumbnail `mediaType` field identifies encoded bytes. Original and prepared files,
full-resolution previews and exports remain lossless PNG/TIFF as recorded.

Saved B-009 validation on the live HTTPS endpoint used **actual reloads**, with
request interception disabled (Playwright routing disables HTTP caching).
Chromium reused all 22 previews in 151 ms; a persistent WebKit context reused them
in 172–187 ms. Both transferred zero preview bytes on reload. Chromium also reused
the 1.55 MB full image without a request body transfer; WebKit revalidated it with
approximately 300 bytes of headers and no image body download. JPEG previews total
about 1.8 MB instead of roughly 8 MB. These are this Mac's measurements, not a
physical-phone or remote-link benchmark. WebKit does not cache the authenticated
images on the insecure HTTP fixture, so its cache assertion uses the production
HTTPS test instead. These checks created no scan jobs.

### Compact navigation — 2026-11-01

The breadcrumb/live-status top bar is removed; SSE subscriptions remain active.
A reconnecting notice appears in the sidebar only during a connection failure.
The sidebar is 200 px expanded or a 64 px icon rail on desktop. Its top button
collapses/expands it and stores the choice in `scanner.sidebar.collapsed` in local
browser storage. On phones the same control hides navigation labels, retaining
all four touch targets. Icon links have accessible names and desktop tooltips.
The workspace caption and sidebar brand subtitle are removed. Card summaries no
longer show “Suggested match” or the “Auto” finish sublabel; evidence and finish
colours are unchanged, and unresolved finishes remain labelled.

Production build and 27 web tests passed. Browser checks cover both sidebar
states at 1440, 768, 390 and 320 px, keyboard toggling, preference persistence,
route selection, system themes, removed labels and connection recovery. Session
screenshots and results are ignored under `Diagnostics/sidebar-ui/`.
