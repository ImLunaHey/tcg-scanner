# Mobile camera identification

Open `/camera` on the scanner's HTTPS address. No login is required. Tap
**Start camera**, and allow camera access. Hold one card front
inside the guide, with all four corners visible against a contrasting background.
The camera fills the viewport. After roughly 0.4 seconds of steady video, the page
automatically sends a JPEG to ScannerService on the Mac. The last identified card
appears in a compact bottom-left tile. Tap it for a full-screen image and catalogue
details; lookups pause until the details close. The preview uses the captured
camera image, held in browser memory, rather than saving it to scan storage. Move
to the next card or tap **Check again** to repeat a lookup.

A persistent badge above the guide shows **Looking for a card**, **Hold steady**,
**Identifying card**, **Card detected**, or guidance for another view. Identification
has a spinning indicator; detection turns the badge and guide corners green.
The toolbar's **Flash** button toggles the active camera's continuous light and
shows On/Off. It shows N/A when that camera/browser does not expose torch control.
The displayed state is checked against the camera settings, and failed changes
show a message. Stopping or leaving the camera releases its track and light.
Flash behavior still needs checking on a physical phone; avoid glare on foil cards.

On iPhone/iPad, open the address in Safari, choose **Share → Add to Home Screen**,
and open the installed Scanner app. On Android, use **Install app** or the browser's
installation menu. The manifest starts the installed app directly at `/camera`.
The Mac and Tailscale connection must stay online; this is not offline recognition.
The physical scanner and scan storage share do not need to be connected. When
storage is offline at service startup, interrupted work is marked failed in the
local database without reading or writing the unavailable volume. Physical scans
remain blocked until storage validation passes; there is no local fallback.

## Behavior and privacy

- No camera frame is written to scan storage, SQLite, a batch, inventory, browser
  storage or an application-managed cache. Only the current result and one preview
  stay in page memory. Blob URLs are revoked when replaced or when the page closes.
- Apple Vision finds the outer card rectangle; Core Image corrects perspective
  before local OCR and the existing catalogue matchers run. Images stay between
  the browser and your Mac. Catalogue requests send text/identifiers, not images.
- The page uses the rear camera where available. It stops camera tracks on Stop,
  navigation, loss of connectivity or backgrounding. Returning to the page
  requires tapping Start again. Late responses from a stopped camera are ignored.
- A steady matched card is not repeatedly submitted. Changed matched cards can
  advance after a 1.2-second minimum interval; unclear frames retry after 2.5 seconds, with only one request in flight per page and one camera
  identification at a time across the service. Hardware capture and automatic
  batch identification take priority when already active.
- English uses the existing Pokémon, Yu-Gi-Oh! and Star Wars: Unlimited matchers.
  Korean uses the Korean Pokémon catalogue, whose coverage is incomplete. Card
  backs and Pokémon LIVE inserts return guidance instead of inventory entries.
- Results are suggestions, not proof of an exact printing. English Pokémon finish
  checks reuse printed set-code contrast, stamps and catalogue variants. Holo,
  reverse holo and non-holo results appear on the tile and in details, with evidence.
  Camera lighting and exposure have not been physically calibrated. The ADF's
  foil-border texture thresholds are deliberately disabled for camera input;
  missing foil texture never establishes non-holo. Ambiguous finishes stay unclear.
  Older layouts, some Energy printings, other games and Korean finishes remain
  unsupported. There is no condition or price assessment.
- The camera uses a verified text orientation first. Only a single candidate with
  agreeing name, collector number and set total can finish this faster OCR pass;
  weaker evidence falls back to all four orientations. Saved ADF recognition keeps
  its existing full-orientation behavior. Recognized Star Wars text routes directly
  to its catalogue rather than trying the Pokémon matcher first.
- Validated public card metadata is cached in memory for 15 minutes, at most 128
  entries per catalogue client. Concurrent requests share one fetch. Failed loads
  are not cached; camera frames and identification results are never cached there.
- The service worker provides an offline explanation for failed page navigation.
  It creates no caches and does not intercept camera/API requests or image downloads.
  Existing images continue to use normal HTTP caching.

## API

Public `POST /api/v1/camera/identify` maps to local JSON-RPC
`v1.camera.identify`. Body: `{"base64":"<JPEG bytes in base64>","language":"en"}`.
Language is `en` or `ko` (default `en`). No paths, URLs or extra fields are accepted.

JPEGs are limited to 512 KiB, one image, 240–4096 pixels per side and 8 megapixels.
JSON bodies are limited to 768 KiB on this HTTP route; ordinary routes keep their
60 KiB limit. The Unix transport's total envelope budget is 768 KiB. The service
normalizes EXIF orientation and downsizes to at most 1600 pixels before Vision.
Recognition has a 60-second deadline, checked between native processing operations;
framework calls already running finish before cancellation unwinds.

The no-store response includes `schemaVersion: 1`, `state`, `candidates`, and optional
`game`, `uncataloguedCard`, `energySymbol`, or `finishEstimate`. State is `noCard`, `cardBack`, `codeCard`,
`matched`, `ambiguous`, or `noMatch`. No scan/image IDs are allocated. Malformed
parameters return 400, invalid image data 422, oversized bodies 413, busy 409,
and rate/concurrency limits 429, following the gateway's existing error schema.

## Validation

Swift tests cover rectangle extraction, invalid/dimension/size rejection, the HTTP
and Unix path with a body larger than the ordinary route budget, authentication,
no database events/jobs/files or hardware capture, and concurrent/cancelled lookups.
The public camera budget is separate from authentication/control quotas; scanner,
profile, session and inventory operations still reject unauthenticated requests.
Web unit tests cover the stability gate and viewport geometry. Browser tests use a
simulated video source on desktop Chromium, mobile Chromium and iPad WebKit;
these do not establish real phone focus, camera orientation or installation behavior.

On 2026-11-09, the deployed HTTPS endpoint identified the locally cached TCGdex
Spectrier `swsh11-081` reference placed on a contrasting canvas, upright and rotated
180°, in 1.48 and 0.91 seconds respectively. Scan/job/batch database rows were
identical before and after. This is a reference-image check, not a phone-camera
accuracy or latency benchmark. The full-screen public camera workflow passes 30 simulated browser checks,
including modal focus, lookup suspension and preview cleanup.

For a physical check, open the HTTPS camera page on your phone and try a known card
upright and upside down, then another card, a blank surface, and a card back. Check
that the result changes, camera use stops on backgrounding, and the library/inventory
count stays unchanged. No ADF feeding is involved.

The live camera endpoint also returned a holo inference for the cached TCGdex
Mimikyu `sv02-097` reference: dark printed set-code evidence plus a single regular
catalogue finish. Spectrier `swsh11-081` and a camera-resolution rendering of
B-001/C-002 Energy remained unsupported. These checks validate routing and
inference output, not physical finish accuracy under phone lighting. One initial
post-deployment request timed out; subsequent live requests completed normally.
Cold catalogue loads still depend on the external catalogue service.
