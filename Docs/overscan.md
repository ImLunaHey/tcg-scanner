# Overscan for card scans

The fi-8170 macOS driver exposes `cropping=4`, labelled Overscan. Its guide says
this adds background areas above, below, left and right of the image:
https://origin.pfultd.com/downloads/IMAGE/macos/240/P2ZZ-0370-05ENZ0.pdf (page 17).
The installed feature enumeration does not expose a separate millimetre margin
or a backing-colour choice for Overscan. Values 0/1 select uncropped black/white
instead; they cannot be combined with value 4.

New card jobs (`prepareCards:true`) default to Overscan with black backing,
including web/native requests and physical Scan-button batches. The service
resolves the default, preserving the original submission for idempotency and
recording `overscan:true` in the effective job request. Existing history is not
rewritten. Explicit `overscan:false` and white-background requests retain their
selected mode; document jobs do not acquire this card-specific default.

Local JSON-RPC and HTTP job requests also accept optional `overscan:true`.
The CLI exposes `scanner scan --duplex --overscan` independently of card analysis.
The service selects and verifies cropping value 4, records black backing in
applied settings, and restores the previous driver value after capture.
Automatic preparation crops and deskews a separate PNG, verifies upright
orientation, then detects the game from the paired back before running
Pokémon recognition and finish/condition checks. Other games retain prepared images
and game/side metadata without Pokémon-specific analysis. The original
retains the full captured margin. Existing Overscan batches can use the same
preparation and identification APIs without scanning again.

No brightness, contrast, pre-pick, multifeed, paper-protection or feed-mode
settings are changed. Explicit white backing and Overscan together are rejected:
the installed enumeration does not expose that combination. This does not
promise the Windows screenshot's 4 mm margin or guarantee every future card edge.

Initial test: three user-confirmed expendable cards, 300 DPI, colour, duplex PNG.
Expected result: six originals. Inspect their background, leading/trailing edges,
dimensions and margins before deciding whether this mode improves edge capture.

## Physical result — B-023, 2026-11-01

Job `EC4C7484-7E6D-4DC0-BDA6-28F231C055D5` completed at 13:31:30 UTC after
starting at 13:31:17 UTC. Three loaded cards produced six 2656 × 3884 PNG
originals, with no scan errors and confirmed session closure. The user confirmed
all three fed cleanly with no new damage. File sizes, PNG signatures, SHA-256
hashes, sequence order and manifest agreement were verified.

The observed backing is black. The offline native card-boundary analysis found
space on all four sides for all six images, verified their upright orientation,
and produced no scan-boundary warning. Detected leading buffer was approximately
181–184 pixels (15.3–15.6 mm using nominal 300 DPI, including the detector's
conservative padding); this is an observation, not an adjustable 4 mm setting or
a calibrated physical measurement. At initial capture, prepared images were not written into the
batch; subsequent automatic processing is recorded below.

After scanning, a fresh inspection confirmed restored cropping value 2 and
blank-page-skip value 3, with multifeed 1, pre-pick 1, tone 1 and sharpness 0
unchanged from the prior snapshot. ScannerService returned to idle/unpaused.
This successful result informed the card default described above.

Validation: 130 automated Swift tests and the six-image saved-result fixture
passed; release build and signed service installation succeeded. Evidence is
ignored under `Diagnostics/overscan/`. Re-run the offline geometry check with:

```sh
SCANNER_OVERSCAN_FIXTURE=/Volumes/scans/B-023 swift test --filter savedOverscanCardsHaveBackgroundOutsideEveryEdge
```

This check reads saved originals only and never feeds the scanner.

## Automatic processing — B-023, 2026-11-01

Enabled the production pipeline for Overscan and installed the signed service
and HTTP gateway. All 132 automated Swift tests and release builds passed.
Reprocessed the six saved originals through the service without scanning again.
All six received cropped, upright PNG copies with no scan-boundary warnings.
The paired identities are C-001 Claydol (`me04-047`), C-002 Deoxys (`me04-032`)
and C-003 Ferrothorn (`me04-063`). Each front received a normal-finish estimate;
each back passed the limited blue-border check with zero candidate marks.
This is not a condition grade or verification of finish. All three were priced
by the existing Cardmarket/GBP guide (7p combined at validation time).

Verified original records and hashes unchanged, all six prepared-image hashes,
identification provenance and manifest agreement. The authenticated Tailscale UI
shows three paired panels, names, finish checks and prices on desktop and 390px
mobile. All six thumbnails and lightboxes loaded. Service is idle/unpaused,
and the automatic button listener rearmed. Ignored evidence is stored under
`Diagnostics/overscan/processing/`; deployment backups use `overscan-default-`.

The user subsequently noticed slight residual tilt on C-001. Upright orientation
verification establishes quarter-turn direction, not sub-degree straightness;
fine deskew was subsequently fixed and the batch refreshed from these saved
originals (see [fine-deskew validation](card-preparation.md#fine-deskew--b-023-2026-11-01)).

## 600 DPI capture — B-024, 2026-11-01

Captured 16 user-supplied holo cards as 32 duplex PNG originals at confirmed
600 DPI, cropping value 4 (Overscan), driver-selected black backing and disabled
blank-page skipping. Multifeed settings were unchanged. The user confirmed all
16 reached the output tray without feeding problems or visible new damage.

The driver produced 5312 × 7768 pixel frames (over 41 megapixels), exceeding the
previous 40-megapixel processing input cap. Increased only card preparation and
editing input limits to 48 megapixels, retaining the 32 MiB encoded input bound,
20,000-pixel axis bound and 40-megapixel rendered-output bound. Boundary analysis
still uses a bounded thumbnail; cropped output retains the source resolution.
The original scan was allowed to finish before installing the signed service.
Reprocessing reads the preserved originals and does not feed hardware again.

148 Swift tests passed with the actual first saved 600 DPI frame enabled, followed
by a release build. The regression confirms full-resolution card preparation
and rejection of an uncropped output above the existing output cap:

```sh
SCANNER_600DPI_FIXTURE=/Volumes/scans/B-024/C-001.png swift test --filter saved600DPIOverscanCanBePreparedWithoutReducingCardResolution
```

Runtime evidence stays ignored under `Diagnostics/600dpi-holos/`. Higher input
resolution alone does not add a foil-pattern detector; finish estimates still
use set-code polarity and catalogue alternatives.

All 32 sides were subsequently cropped and verified upright, with identification
source hashes pointing at their prepared copies. Original checksums were verified
unchanged and the manifest matches the persisted records. Current results are
11 holo estimates, three normal-or-holo ambiguities (Lillie's Ribombee, Relicanth,
Mismagius), and two unresolved printing identities (Kilowattrel, Zarude). These
results do not establish a measured improvement over 300 DPI or a validated foil
classifier. No finish overrides or Store transfers were performed.
