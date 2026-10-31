# Whole-card cropping and orientation

Enable **Auto-crop and orient cards** in the scan controls to create prepared PNGs
during the job's processing stage. The web and native scan controls now enable
this by default for the card workflow; turn it off for general documents. CLI/API
requests still opt in explicitly. Existing captures have a **Prepare card**
action. Neither operation discards or overwrites the driver's original image.

The web/native library shows the latest prepared card in place of the full-page
preview. **Download card** exports the prepared PNG; **Original** retrieves the
unchanged full-page capture. Copies and originals remain in the same batch folder;
there is no processed subdirectory. Disk copies use unique `edit-UUID.png` names,
with their source capture and operations recorded in the manifest. Browser card
downloads use `B-NNN/C-NNN` labels in the filename. Front/back pairing and final
per-card filenames remain separate work.

## Detection and limits

The first algorithm targets one trading card on the fi-8170's black scan
background. It finds connected foreground bounds, checks the card's aspect ratio,
rejects separate substantial objects and keeps one analysis-pixel of padding
(about three full-resolution pixels on the current 300 DPI A4 scans). The v2
method replaces the original 2% padding to reduce visible black margins. It
includes the card border instead of detecting only the artwork.
Analysis uses a bounded preview; final PNG rendering uses the original resolution.
It does not synthesize edges, remove the background, sharpen or grade the card.

Apple Vision's text quadrilateral directions vote for clockwise quarter-turn
rotation. At least four readable lines with strong agreement are required before
the orientation is marked verified. Symmetric Pokémon-back logos do not reliably
distinguish upright from upside down; uncertain backs retain their portrait
orientation and are flagged for inspection. Landscape uncertain images are turned
to portrait but still require review. No certainty is implied by a prepared copy.

If the card meets an image boundary, the copy warns that the original may have
clipped an edge. Cropping cannot recover pixels the scanner never captured. Check
all corners and edges before using the image in a listing. The first B-007 images
are flush with one scan boundary, so a complete margin cannot be added there.

This is a bounding crop and quarter-turn correction, not perspective correction
or arbitrary-angle deskew. White-background photos, sleeves, top loaders,
overlapping cards or ambiguous boundaries may require manual cropping. Inputs are
limited to one image, 32 MiB and 40 megapixels. Failed preparation retains the
original and records a warning; it does not turn a successful capture into lost
data. A cancelled processing job also retains captured originals.

## API and automation

- Set `prepareCards: true` in a scan request (REST, JSON-RPC or MCP). This controls
  cropping, orientation and automatic English card identification during processing;
  it does not change feeder or multifeed settings. See [identification](card-identification.md).
- `POST /api/v1/images/{jobID}:{sequence}/prepare-card` with `{ "id": "UUID" }`
  creates a prepared copy for a finished capture. Reuse the UUID on retry.
- JSON-RPC: `v1.images.prepareCard`, parameters `jobID`, `sequence`, `id`.
- MCP: `scanner_prepare_card`; `scanner_start_scan` accepts `prepareCards`.
- CLI: `scanner scan --duplex --prepare-cards`, or
  `scanner images prepare-card JOB_UUID SEQUENCE --request-id UUID --json`.

`processedImages[].cardPreparation` contains the normalized crop, clockwise
quarter turns, orientation verification flag, method and warnings. Existing edits
without this optional property remain ordinary manual copies. Identification
prefers the latest prepared card and records `sourceEditID` and its SHA-256.
Library tiles show a unique candidate's name, set and collector number as
**Suggested · review required**, without requiring the review dialog. Multiple
candidates and no-match/failed states are shown explicitly. Suggestions never
automatically confirm identity, finish or condition. Previously saved
identification results are not silently replaced: reject stale
suggestions and request a fresh identification when needed.

## First physical card test — B-007

- Job: `81978C1D-8768-4170-B852-0A917EF85611`, 2026-10-31.
- Media: one expendable bare Dunsparce card, loaded and authorized by the user.
  The user described a removed “main roller”; the precise assembly is unverified.
- Capture: 300 DPI, RGB 8-bit, duplex, PNG, full A4 capture area. Multifeed and
  transport-safety settings were unchanged. Blank-page skipping and driver
  cropping were disabled as in prior paper scans.
- Result: completed, no reported driver errors, session closed, two originals
  saved in `/Volumes/scans/B-007` at 2480 × 3508 pixels.
- Visual inspection: C-001 is the back, initially upright; C-002 is the front,
  initially upside down. This describes this test only, not a universal pairing
  guarantee. The user reported “Fed cleanly; no new damage visible.”
- Original SHA-256: C-001 `ff18728b26f1af469c21a82f6b2bbbdc851308748f101df264b318884b6edd32`;
  C-002 `713184eb762687ad8764201729b72328eef9592a5f2d1f4df910ffd66f27b032`.
- Initial OCR read Dunsparce and 120/159, but the full-page layout caused the
  matcher to miss the collector-number region and return ambiguous name-only
  candidates. No identity, finish or condition was confirmed.
- The saved-image preparation test found the card borders, kept the back's
  orientation with an uncertainty warning, and rotated the front by 180 degrees.
  Both rendered results were visually inspected. No additional feed was needed.

Raw test evidence is ignored under `Diagnostics/card-test/`; saved-image algorithm
fixtures are opt-in via `SCANNER_CARD_PREPARATION_FIXTURE=/Volumes/scans/B-007`.
No scanned images are committed to Git.

## Installed preparation and identification

The signed service, CLI, MCP, HTTP gateway, web bundle and native application were
updated on 2026-10-31. Previous service binaries, database and native app are in
`~/Library/Application Support/Scanner/backups/card-preparation-20261031T205814Z/`.
The web installer kept its own versioned backup. Existing authentication, SMB root
and certificate-backed identities were preserved.

B-007 was prepared through the running service without feeding again. The back
copy is 777 × 1056; the front is 780 × 1056 and rotated 180 degrees. Re-identifying
the prepared front returned one candidate, **Dunsparce, Journey Together,
120/159 (`sv09-120`)**, using name, collector number and printed set total. The
previous ambiguous suggestion was rejected; the new suggestion remains pending
manual confirmation. Finish and condition were not inferred.

## Interrupted batch test — B-008

- Job `F22AF249-AB08-415C-8889-27587ECB2800`, 2026-10-31, one submission only.
- The user confirmed exactly three expendable cards before starting. Afterward,
  the user clarified **11 cards were loaded**, and reported no visible damage.
- Same 300 DPI duplex PNG A4 settings, with `prepareCards: true`. No multifeed or
  transport-safety setting was changed.
- Polling showed more than the expected six images. Cancellation was requested;
  the final state is **cancelled**, `sessionClosed: true`, with no reported driver
  errors. Eighteen 2480 × 3508 originals were retained in `/Volumes/scans/B-008`.
- Nine distinct fronts were visually inspected. Two of the 11 loaded cards are
  absent from the saved front images. The user confirmed that **all 11 reached
  the output tray**. Pending driver delivery at cancellation is a possible cause,
  not a verified diagnosis. This does not pass the intended count/order test.
- Cancellation skipped automatic preparation. The saved originals were then
  prepared through the same service API, without another feed: 18 separate PNG
  copies, all successful, approximately 777–786 × 1053–1059 pixels. All original
  SHA-256 values were verified unchanged before and after preparation.
- The nine fronts were rotated 180 degrees, verified by text orientation and
  visual inspection. The apparent backs retain portrait orientation with an
  uncertainty warning; a representative back was visually checked. All copies
  warn that a card edge reaches the original scan boundary. Arbitrary-angle
  deskew and guaranteed full-edge capture remain unvalidated.
- OCR on the nine prepared fronts returned eight specific candidates matching
  their visible names and collector numbers: Salamence ex 177, Larvesta 028,
  Pangoro 099, Virizion 015, Iris's Fighting Spirit 149, Lokix 100, Cubone 076,
  Lechonk 139, all Journey Together. Basic Fire Energy 010 returned `noMatch`.
  No candidate, finish or condition was automatically confirmed.
- Images keep callback sequence and `side: unknown`. Back/front alternation is an
  observation, not verified physical pairing or a completed batch manifest.

Evidence and request IDs are ignored under `Diagnostics/card-batch-test/`.
No further physical scan was initiated.

## Verification

- Swift suite: 70 executed tests passed with SMB integration enabled; three
  opt-in fixtures skipped (73 reported total). Two saved-image crop/orientation
  tests also passed against B-007.
- Web: TypeScript/Vite production build, four unit tests and six browser tests
  passed across desktop, phone and tablet configurations. Browser exports include
  both originals and prepared copies, with checksums verified.
- Production browser verification through Tailscale passed: prepared downloads,
  original checksum, crop/rotation prefill, mobile layout and unchanged job count.
  Only login/logout changed state. Production native accessibility inspection
  confirmed preparation controls and per-image export/warnings.
- Release binaries built and installed with persistent certificate-backed signing.
- B-008 verifies saved-image processing after cancellation, not uninterrupted
  hardware scanning followed by automatic processing; that path has mock tests
  and needed the separately authorized hardware run recorded below.

## Full-batch retry — B-009

The user explicitly reloaded and authorized all 11 cards for one full retry.
Job `C4FAD134-D129-4DE4-A542-EFC6E9BEE95E` completed normally at 300 DPI, duplex PNG,
with 22 originals and 22 automatically cropped/oriented copies. The session closed
and no driver errors were reported. All 22 original checksums were verified.
The user confirmed all 11 fed cleanly with no new marks or damage.

This run used `black-background-card-v2`, whose smaller margin produces copies
750–765 pixels wide and 1041–1047 pixels high. All 11 even-numbered fronts were
turned upright by 180 degrees; back orientation remains unverified by the
algorithm. Representative front images were visually inspected, including the
full-art Salamence ex and Dunsparce. The existing scan-boundary warning remains.

OCR was requested on the 11 saved fronts after completion. Nine returned unique
Journey Together candidates: Larvesta, Pangoro, Virizion, Iris's Fighting Spirit,
Lokix, Cubone, Lechonk, Impidimp and Dunsparce. Basic Fire Energy returned no match.
Salamence ex returned ambiguous name-only candidates for other printings this time;
these are not a verified match. Nothing was automatically confirmed. OCR accuracy
is not established by the successful image count.

Raw requests, scan results and recognition evidence are ignored under
`Diagnostics/card-full-retry/`. B-008 remains cancelled and its original files
are retained separately; the retry did not overwrite or complete that older job.

### Paired web library

Duplex jobs requested with card preparation show consecutive captures together
under one card heading. The first implementation labelled even captures Front and odd captures Back
for the fi-8170. B-011's reversed-feed test below supersedes that assumption:
current labels use recognition evidence and otherwise remain Side 1/Side 2. The pair shares one identity summary, including
catalogue category and elemental types when available. [TCGdex card fields](https://tcgdex.dev/rest/card) are
catalogue attributes of the suggestion, not proof of a physical printing. Phone and
tablet layouts keep the two previews side by side.

This is a presentation grouping by immutable capture sequence, not persisted
hardware side metadata or a universal correspondence guarantee. Missing sequence
slots remain empty rather than shifting later pairs; incomplete groups and
interrupted batches are labelled. Original capture filenames, IDs and manifest
side fields stay unchanged. Card loading direction and interrupted feed still
require correspondence review before treating pair labels as inventory IDs.

### Follow-up validation and deployment

The v2 crop tests passed against saved B-007 files without feeding. The Swift
suite passed with SMB integration enabled (74 reported tests, three opt-in
fixtures skipped). Five web unit tests passed. Existing browser workflows passed
on desktop Chromium, phone Chromium and tablet WebKit. The new paired-layout
checks passed on all three, including category/type visibility, a single identity
summary and side-by-side previews. The first pairing check used a five-second
completion deadline and timed out during legitimate image processing; a bounded
15-second deadline passed without changing the application behavior.

The signed update was installed. The HTTP gateway's prior process took longer than
the installer's ten-second restart window to exit; bootstrap succeeded once it
finished. The installer now allows 45 seconds for that graceful shutdown. No
scanner restart or additional feed was used to resolve the HTTP deployment.

Production browser verification through Tailscale confirmed 11 groups, 22 image
previews, Larvesta's name/set/number/Pokémon–Fire metadata, a prepared download's
SHA-256, a mobile layout with both sides together, and unchanged job history.
B-009's unconfirmed identification requests were refreshed to add catalogue
category/types; no user-confirmed identity was replaced. Nine unique matches,
one ambiguous result and one no-match remained. Native inspection confirmed the
new catalogue fields and cropped-image labels as well.

Cropping is automatic when enabled. Identification is still a separate service
operation; the B-009 requests were initiated after capture by the test operator.
For another batch, use **Identify card** on its front image to start OCR; results
then appear directly on the pair. Confirmation, foil finish and condition remain
manual. Grouping does not rename the original files or establish permanent
physical-side metadata.

## Reversed-feed validation — B-011, 2026-10-31

The user reloaded the same eleven cards back-to-front and authorized one test.
Job `1E0CF640-DA93-4915-ADC6-3DD9A539FD8D` retained the existing 300 DPI duplex PNG
configuration and card processing; multifeed and other feeder safety settings
were unchanged. It completed with 22 originals, 22 prepared copies, no job errors
and a closed scanner session. The user reported all eleven fed cleanly without
jams, overlap or new damage. All 44 saved PNG files passed size/SHA-256 checks.

Automatic recognition found the same eleven printings as B-010, now on odd
capture sequences. Every front was automatically rotated two quarter turns and
had text-supported orientation. Card backs remain orientation-unverified; this
test does not establish arbitrary-orientation support or automatic back rotation.

In pair-number order, the suggestions were Dunsparce, Impidimp, Lechonk,
Salamence ex, Cubone, Lokix, Iris's Fighting Spirit, Pangoro, Larvesta, Virizion,
and Fire Energy. This is the observed order, not a promise that every reload
preserves card position. Identifications remain reviewable suggestions.

The test exposed a web presentation bug: the paired gallery assumed the even
capture was always the front and could select its no-match result instead of
the recognized front. It now displays whichever side has recognition evidence
first. If both sides match, neither matches, or a side is missing, it keeps
Side 1 / Side 2 labels; ambiguous matches have a warning and each side can be
reviewed separately. This is a presentation inference, not a change to original
capture order, image metadata, permanent IDs or verified physical pairing.

Web unit tests cover either front sequence, missing sides, ambiguous matches,
no matches, rejected matches and pending recognition. Private scan requests,
records, checksums and physical confirmation are in `Diagnostics/reversed-feed/`.
The browser validation does not create or alter scans. A separately authorized
mixed-orientation physical test follows this batch.

Read-only Chromium checks over the production Tailscale HTTPS address found
11 pairs and 11 suggestions in both B-011 and B-010. Larvesta's front download
matched the prepared PNG checksum at sequence 17 and 4 respectively; each
identity dialog opened the correct capture. Both layouts fit a 390-pixel phone
viewport, with no page errors or changes to job history. During validation,
image downloads were found to consume the same admission slots as image
metadata/identification controls. Only binary GETs now use the two download
slots; the normal authenticated request limit still covers controls. All nine
HTTP tests passed, including a regression with both download slots occupied.
