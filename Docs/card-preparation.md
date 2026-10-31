# Whole-card cropping and orientation

Enable **Auto-crop and orient cards** in the scan controls to create prepared PNGs
during the job's processing stage. This is optional and off by default for general
documents. Existing captures have a **Prepare card** action. Neither operation
discards or overwrites the driver's original image.

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
rejects separate substantial objects and keeps a small margin (about 2% of the
short edge). It includes the card border instead of detecting only the artwork.
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
  software processing only; it does not change feeder or multifeed settings.
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
Previously saved identification results are not silently replaced: reject stale
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
  and still needs a separately authorized hardware run.
