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
the orientation is marked verified. The v3 method additionally recognizes standard
international blue Pokémon backs using a bounded fuzzy OCR logo check, blue border,
yellow end logos and central red/white Poké Ball layout. The red half establishes
the top; repeated logos alone do not. All layout checks must agree on exactly one
rotation. Unknown/ambiguous backs retain their portrait orientation and are flagged
for inspection. This does not authenticate the card or assess its condition. Landscape uncertain images are turned
to portrait but still require review. No certainty is implied by a prepared copy.

If the card meets an image boundary, the copy warns that the original may have
clipped an edge. Cropping cannot recover pixels the scanner never captured. Check
all corners and edges before using the image in a listing. The first B-007 images
are flush with one scan boundary, so a complete margin cannot be added there.

The v3 method fits a minimum-area enclosing box to the complete outer foreground
within ±12°, rotates the full original canvas, then crops with a protective margin
and applies the upright quarter-turn correction. It skips negligible corrections
and warns if the best angle reaches the supported range limit. This is rigid
small-angle deskew, not perspective correction or reconstruction of missing edges. White-background photos, sleeves, top loaders,
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
quarter turns, orientation verification flag, method and warnings. V3 adds optional
`deskewDegrees`, `orientationBasis` (`text`, `pokemon-back-pattern`, or `unknown`)
and `sourceCrop` (the original-image bounding crop for manual editing). `crop` in
the recorded edit applies after deskew; old records with no deskew field retain
their previous meaning. Manual editors operate on the original using `sourceCrop`,
not coordinates from the rotated canvas. Automatic processing runs in Swift with
[Core Image transforms](https://developer.apple.com/documentation/coreimage/ciimage/transformed(by:)); no images leave this Mac for orientation analysis. Existing edits
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

## Mixed-orientation validation — B-012, 2026-10-31

The user mixed the eleven cards' orientations, loaded them and authorized one
additional scan after the B-011 checks. Read-only inspection found the ADF loaded
and the service idle. Job `9D3E61D1-21F5-47DA-8338-971EDB02529B` completed using
300 DPI duplex PNG and card preparation, with unchanged feeder safety settings.
It saved 22 originals and 22 prepared images, closed the hardware session and
reported no job errors. The user confirmed all eleven fed cleanly with no new
damage. Size and SHA-256 verification passed for all 44 saved PNG files.

Each consecutive pair has exactly one recognized front. The eleven suggested
printing IDs equal the B-010 baseline, with no missing or duplicate printings.
In pair order: Fire Energy, Virizion, Larvesta, Pangoro, Iris's Fighting Spirit,
Lokix, Cubone, Salamence ex, Lechonk, Impidimp and Dunsparce. The first three
fronts required 180-degree correction; the remaining eight required none.
All eleven fronts had text-supported orientation and appeared upright in the
prepared gallery. The front captures in this run are all even-numbered: it
validates mixed upright/upside-down loading, while B-011 separately validates
fronts arriving on the other scanner side. Neither test establishes arbitrary
sideways physical feeding or every possible mixed-front/back stack.

Read-only Chromium validation over the production Tailscale HTTPS address
checked all eleven pair summaries, front downloads and identity dialogs. Every
download matched that pair's prepared front PNG checksum, and every dialog
targeted that front's actual capture sequence. Desktop and 390-pixel phone
layouts were checked, with no horizontal phone overflow, page errors or job
history changes. Recognition remains a suggestion requiring review; no finish
or condition estimates were added by this test.

Remaining limitations: backs have uncertain orientation and may remain rotated;
automatic upright back correction is not implemented. Scan-boundary crop warnings
remain visible where a card reaches the scan area's edge; cropping cannot restore
uncaptured pixels. The original PNGs are preserved. Private requests, final job
records, physical confirmation, file verification and browser-check evidence are
under `Diagnostics/mixed-orientation/` (ignored by Git). No further scan was
started after B-012.

## Automatic deskew and back orientation follow-up — 2026-10-31

The v3 processing update supersedes the missing deskew/back-orientation behavior
reported in the preceding physical tests. It runs automatically in ScannerService
when card preparation is enabled; no operator rotation is needed for recognized
fronts or standard international blue Pokémon backs. Uncertain images retain a
review warning. The implementation uses native Swift, Vision and Core Image.

Validation used saved B-012 originals; the feeder was not operated:

- All 22 captures passed expected upright orientation checks (11 text-based fronts
  and 11 pattern-based backs). Six captures received small-angle corrections,
  ranging from −1° to +1°; effectively straight captures avoided interpolation.
- Digitally transformed front/back examples passed every quarter-turn orientation
  with additional −5° and +5° tilt (16 cases). These are image-processing tests,
  not evidence that sideways physical card feeding is safe.
- Geometry tests check enclosing dimensions and margins at several angles. A mock
  scanner job verifies automatic deskew is rendered and persisted before OCR.
  All eleven real fronts were rejected by the back-pattern detector even when
  supplied with a matching logo token, guarding against false back classification.
- A separate read-only recognition regression on the new PNGs retained all eleven
  expected printing IDs, including Larvesta; every chosen OCR orientation was 0°.
- The full Swift suite, the 14-test image-processing run with saved fixtures,
  nine HTTP tests (including valid/invalid deskew requests through the service),
  eleven web tests, TypeScript/Vite build and signed release build passed.

The signed service, CLI/MCP clients, HTTP gateway, web assets and native app were
updated. Native installation followed the user's confirmation that the app was
closed. B-012 was reprocessed through `v1.images.prepareCard` using saved operation
UUIDs: it now has 22 original PNGs, 22 earlier prepared copies and 22 new v3 copies.
All 66 file sizes/checksums were verified. Installed-service output exactly matched
the saved-fixture output. Stored identification/review records remain unchanged,
including their original source provenance; fresh OCR was tested separately rather
than clearing review state.

Read-only browser checks verified all 22 current front/back downloads by checksum,
all eleven identity dialogs, original-coordinate manual-crop prefill, desktop and
390-pixel phone layouts, and unchanged job history. The gallery uses the new
prepared copies after refresh. Evidence is ignored under
`Diagnostics/orientation-refresh/` and `Diagnostics/orientation-validation/`.

Original scan-boundary warnings still apply: deskew cannot reconstruct an edge
that the scanner did not capture. The standard-back detector is not an authenticity
check, and unrecognized back designs remain subject to manual review.


## New bulk-card physical test — B-013, 2026-10-31

The user loaded 20 different bulk-bin cards and said they should all be holo.
That is an expected finish, not verified per-card ground truth. One authorized
300 DPI duplex PNG job, `C4E36EC5-11C3-4A9C-A33A-475B1187EF4E`, ran with automatic
card preparation and identification. Feeder safety settings were unchanged.
The user reported all 20 fed cleanly with no new damage. No second scan was run.

The job completed with 40 originals, 40 prepared copies, no errors and a closed
scanner session. All 80 PNG file sizes and SHA-256 values were verified. The v3
pipeline found 20 standard blue backs and 20 text-bearing fronts, with upright
orientation evidence for all 40 images. All required 180-degree correction;
13 images also received deskew corrections between −0.9° and +0.8°. Scan-boundary
warnings remain where the captured edge meets the scan area.

Every consecutive pair has one front with a single catalogue suggestion, backed
by a recognized name and collector number. The twenty suggestions span eight
sets and remain `reviewRequired`; these new cards do not have independently
confirmed identity labels. Spectrier appears twice in the captured sequence.
All finishes remain unverified and conditions unassessed. Catalogue availability
of holo/reverse-holo variants does not establish the scanned card's finish.

| Pair | Suggested card | Catalogue ID |
| --- | --- | --- |
| C-001 | Gwynn | `me05-078` |
| C-002 | Gladion's Final Battle | `me05-077` |
| C-003 | Rust Syndicate Grunt | `me05-081` |
| C-004 | Team Rocket's Giovanni | `sv10-174` |
| C-005 | Vileplume | `me02-003` |
| C-006 | Spectrier | `me02.5-098` |
| C-007 | Zamazenta | `sv10-146` |
| C-008 | Okidogi | `me02.5-122` |
| C-009 | Indeedee | `sv01-153` |
| C-010 | Zarude | `me05-056` |
| C-011 | Genesect | `me02-008` |
| C-012 | Mismagius | `me02.5-086` |
| C-013 | Tapu Koko | `me02.5-067` |
| C-014 | Spectrier | `me02.5-098` |
| C-015 | Iono's Kilowattrel | `me02.5-072` |
| C-016 | Relicanth | `sv05-084` |
| C-017 | Lillie's Ribombee | `sv09-067` |
| C-018 | Emboar | `sv10.5w-013` |
| C-019 | Armarouge | `me05-012` |
| C-020 | Tornadus | `sv10.5w-078` |

Read-only browser validation over the production Tailscale HTTPS address checked
20 paired summaries, 20 correct identity dialogs and every prepared front download
against its stored checksum. Desktop and 390-pixel phone layouts passed without
page errors or horizontal overflow. Job history was unchanged by browser checks.
Private request IDs, expectations, physical confirmation, records, checksums and
screenshots are retained under ignored `Diagnostics/bulk-holo-test/`.

The existing capture guard stops the job at 100 image callbacks, so this test
used fewer than 50 duplex cards. Raising that limit and validating larger batches
were outside this run.

## Fine deskew — B-023, 2026-11-01

C-001's front retained a small tilt despite verified upright orientation. The old
minimum-area search found −0.3° but discarded it: the enclosing area improved by
only 0.165%, below its 0.5% cutoff. Rounded, pixelated corners can also make the
minimum-area box choose zero while the straight card edges are tilted.

`card-boundary-v4` refines the coarse angle using the middle 80% of two long outer
edges. It rejects isolated edge noise and requires sufficient edge coverage,
low residual error and agreement between opposite edges. Both row and column
endpoints are sampled so sideways cards have the same evidence. Refinement is
bounded to 0.6° from the coarse result; the existing ±12° limit and 0.15° noise
floor remain. The fallback area-gain cutoff is 0.1%. Crops still enclose every
observed foreground point with padding, rather than trimming to the fitted lines.

Regression tests cover rounded rasterized cards at ±0.3° and zero, each in all
four orientations, plus original corner enclosure, isolated edge noise and the
saved six-image B-023 fixture. The native pipeline performs this correction
automatically for future scans; it is not a manually entered card rotation.

Deployed the signed release and regenerated all six B-023 preparations from the
unchanged originals. C-001 now receives approximately +0.321° on its back and
−0.314° on its front, automatically calculated from the edges. C-002 retains zero
correction; C-003 receives +0.530° / −0.463°. All six remain upright with no boundary
warnings. Refreshed identification, finish and condition checks against the new
prepared-image hashes: the same three identities, normal-finish estimates and
zero back-border findings remain. Previous prepared copies are also retained.

Validation: the 133-test suite (including the explicitly enabled six-image
hardware-result fixture), a separate added edge-noise regression and release
build passed. Original and all prepared-image hashes, source provenance and
manifest agreement passed. All six production thumbnails/lightboxes and the
three paired panels, finish checks and prices passed desktop/mobile browser
checks. The service returned to idle/unpaused. Private evidence remains ignored
under `Diagnostics/overscan/fine-deskew/`; signed deployment backups use
`fine-deskew-`. This validation used saved images and did not feed the scanner.
