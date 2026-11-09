# Mobile camera identification

Open `/camera` on the scanner's HTTPS address. Sign in with the existing scanner
access key, tap **Start camera**, and allow camera access. Hold one card front
inside the guide, with all four corners visible against a contrasting background.
After roughly a second of steady video, the page automatically sends a JPEG to
ScannerService on the Mac and displays its catalogue suggestions. Move to the
next card or tap **Check again** to repeat a lookup.

On iPhone/iPad, open the address in Safari, choose **Share → Add to Home Screen**,
and open the installed Scanner app. On Android, use **Install app** or the browser's
installation menu. The manifest starts the installed app directly at `/camera`.
The Mac and Tailscale connection must stay online; this is not offline recognition.
The physical scanner and scan storage share do not need to be connected.

## Behavior and privacy

- No camera frame is written to scan storage, SQLite, a batch, inventory, browser
  storage or an application-managed cache. Only the current result stays in page
  memory. Closing the page clears it.
- Apple Vision finds the outer card rectangle; Core Image corrects perspective
  before local OCR and the existing catalogue matchers run. Images stay between
  the browser and your Mac. Catalogue requests send text/identifiers, not images.
- The page uses the rear camera where available. It stops camera tracks on Stop,
  navigation, logout, loss of connectivity or backgrounding. Returning to the page
  requires tapping Start again. Late responses from a stopped camera are ignored.
- A steady matched card is not repeatedly submitted. Unclear frames retry at most
  once per 4.5 seconds, with only one request in flight per page and one camera
  identification at a time across the service. Hardware capture and automatic
  batch identification take priority when already active.
- English uses the existing Pokémon, Yu-Gi-Oh! and Star Wars: Unlimited matchers.
  Korean uses the Korean Pokémon catalogue, whose coverage is incomplete. Card
  backs and Pokémon LIVE inserts return guidance instead of inventory entries.
- Results are suggestions, not proof of an exact printing. There is no camera
  finish, condition or price assessment. Phone glare, focus, lighting and tiny
  collector numbers affect recognition; compare the printed set and number.
- The service worker provides an offline explanation for failed page navigation.
  It creates no caches and does not intercept camera/API requests or image downloads.
  Existing images continue to use normal HTTP caching.

## API

Authenticated `POST /api/v1/camera/identify` maps to local JSON-RPC
`v1.camera.identify`. Body: `{"base64":"<JPEG bytes in base64>","language":"en"}`.
Language is `en` or `ko` (default `en`). No paths, URLs or extra fields are accepted.

JPEGs are limited to 512 KiB, one image, 240–4096 pixels per side and 8 megapixels.
JSON bodies are limited to 768 KiB on this HTTP route; ordinary routes keep their
60 KiB limit. The Unix transport's total envelope budget is 768 KiB. The service
normalizes EXIF orientation and downsizes to at most 1600 pixels before Vision.
Recognition has a 60-second deadline, checked between native processing operations;
framework calls already running finish before cancellation unwinds.

The no-store response includes `schemaVersion: 1`, `state`, `candidates`, and optional
`game`, `uncataloguedCard`, or `energySymbol`. State is `noCard`, `cardBack`, `codeCard`,
`matched`, `ambiguous`, or `noMatch`. No scan/image IDs are allocated. Malformed
parameters return 400, invalid image data 422, oversized bodies 413, busy 409,
and unauthenticated requests 401, following the gateway's existing error schema.

## Validation

Swift tests cover rectangle extraction, invalid/dimension/size rejection, the HTTP
and Unix path with a body larger than the ordinary route budget, authentication,
no database events/jobs/files or hardware capture, and concurrent/cancelled lookups.
Web unit tests cover the stability gate and viewport geometry. Browser tests use a
simulated video source on desktop Chromium, mobile Chromium and iPad WebKit;
these do not establish real phone focus, camera orientation or installation behavior.

For a physical check, open the HTTPS camera page on your phone and try a known card
upright and upside down, then another card, a blank surface, and a card back. Check
that the result changes, camera use stops on backgrounding, and the library/inventory
count stays unchanged. No ADF feeding is involved.
