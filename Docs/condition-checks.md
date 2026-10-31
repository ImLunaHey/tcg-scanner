# Local visible condition checks

`blue-back-visible-marks-v1` flags candidate light marks in the blue border of an
upright, automatically prepared standard Pokémon back. It runs in-process in Swift
using ImageIO/Core Graphics. No images or prompts leave the Mac, and no model,
interpreter or helper process is involved.

This is a limited visual review aid, not a condition grader. It checks possible
edge whitening, corner whitening and elongated light marks that warrant inspection
for scratches or creases. Printed art, card fronts, the outermost paper rim and
hidden surface defects are not assessed. Dust, print variation and scan artifacts
can cause false positives. No flagged marks does not establish Near Mint.

## Use

In the web batch gallery, choose **Check condition** on a card. The panel defaults
to the back when front/back evidence is available. It shows the inspected image,
numbered candidate regions and reasons. Switch sides to see scope/limitations;
unsupported fronts and uncertain back orientations explicitly require manual
inspection. **Refresh visible checks** re-analyzes the saved source without
feeding hardware. The native scan library provides **Check condition…** per side
with the same observations and marked image.

New automatic card identifications run the check before the catalogue lookup,
including backs that produce no identity match. A catalogue outage therefore does
not suppress an available local check. The manual condition picker remains the
only way to set Near Mint/Lightly Played/etc. No estimate automatically sets a
selling grade or overwrites a review.

## Contract and provenance

`CardIdentification.conditionCheck` is optional and contains:

- `method`, `state` (`checked`, `unsupported`, `inconclusive`).
- `findings`: `kind` (`edgeWhitening`, `cornerWhitening`, `linearMark`), `region`
  and `explanation`. Regions are normalized top-left coordinates.
- `limitations` and optional `analyzedWidth`/`analyzedHeight`.

Regions refer to the identification's exact `sourceEditID` and `sourceSHA256`,
not an arbitrary later edit. Clients display that same source under the overlay.
Saved observations appear in jobs and batch manifests. They are candidates for
review, not confirmed physical defects. Original and prepared PNGs remain intact.

Available without a GUI:

- RPC `v1.images.checkCondition`: `jobID`, `sequence`, `identificationID`.
- HTTP `POST /api/v1/images/{jobID}:{sequence}/condition-check`: body
  `{ "identificationID": "UUID" }`, authenticated.
- CLI `scanner images check-condition JOB_ID SEQUENCE --identification-id UUID --json`.
- MCP `scanner_check_condition`: the same three arguments; changes saved
  observations, never hardware or manual review.

Stale identification IDs, concurrent image work and changed/missing source data
are rejected. The source is read through the storage boundary and checksum-checked.
After detached analysis, the service reloads the record and changes only
`conditionCheck`, preserving concurrent manual review. Repeating a check is
stable for the same source and algorithm version. Back/no-match and failed
catalogue-identification records can still be checked.

## Detection limits

Analysis accepts one prepared image up to 32 MiB and 40 megapixels, with at least
500 × 700 pixels, and creates at most a 1200-pixel analysis image. The preparation
must already establish a standard blue back and upright orientation. The border
must independently remain predominantly blue. Other cases abstain.

Only bright, low-saturation connected regions within a narrow blue-border band
are considered. Very small noise and the outermost rim are excluded. Elongated
regions are described as possible linear marks; they are not proof of a crease.
Up to 24 largest regions are shown with an explicit truncation note. This is a
conservative heuristic, not a trained or calibrated defect classifier. It has not
been validated for sensitivity against a physically labelled collection of worn
cards. Known worn examples are the next validation step before broader use.

## Validation and physical run B-015

The user first authorized one scan of 14 newly loaded non-holo cards. Job
`B5F95274-F58A-44FA-A79A-50C33FE72D8D` completed as B-015 at 300 DPI, duplex PNG,
with automatic preparation and identification. All 28 captures and 28 prepared
copies passed file-size/SHA-256 checks. Every card has a single front name/set
suggestion. The user confirmed all 14 fed cleanly without new damage. That does
not establish absence of pre-existing wear.

The unchanged v2 finish estimator suggested normal on 13 cards and abstained on
C-007 Volcarona (`sv09-029`) because the set-code box could not be located reliably.
The user separately confirmed Volcarona is non-holo. This label is retained in
private test notes; it was not used to force an automatic prediction. There were
no foil predictions in this new batch.

After deploying the condition feature, the saved B-015 sources were checked
without another physical scan. Twelve backs were eligible and had no candidate
marks. C-006 and C-009 backs lacked verified orientation and were unsupported;
all fourteen fronts are outside this check's scope. All non-condition-check
record fields and all 56 file checksums were preserved.

Automated validation includes synthetic positive whitening/corner/linear marks,
encoded-image overlay coordinates, exclusion of artwork/rim/noise, unsupported
sources, invalid image data, manual review preservation, stale requests,
no-match backs, catalogue failure, restart persistence, HTTP authentication and
extra-field rejection, CLI parsing and MCP tool schemas. The full Swift suite
passed 102 tests; saved-image checks ran separately. The web suite passed 18 tests,
and TypeScript/Vite and signed release builds passed.

Private requests, physical confirmation and original outcomes remain under ignored
`Diagnostics/normal-finish-test-15/`; condition before/after records and verification
are in `Diagnostics/condition-checks/`. The signed service, CLI/MCP, native app and
HTTP/web application were updated. No native UI was required for the scan or checks.

Production HTTPS browser checks verified all fourteen paired summaries, default
back selection, front/back switching, real refresh behavior and unchanged records.
Desktop and 390-pixel phone layouts passed without overflow or page errors. A
separate intercepted browser-only fixture verified numbered overlay coordinates
and explanations for a positive mark; it did not save fabricated findings or
change production data. Native compilation and signed installation passed; the
native sheet was not visually exercised during this run.

## Automatic result links and lightbox

The gallery's visible-marks result is now a clickable link to the existing
zoom/pan lightbox. The inspected source is selected from its saved edit ID and
checksum; mismatched/missing sources never receive an overlay. Numbered **red**
boxes move and scale with the image. Opening a result with findings focuses the
first mark; **Mark 1**, **Mark 2**, etc. jump to each candidate region. Clear and
unsupported results still show their scope and never imply a grade. The detailed
review panel also links its image and individual explanations to this lightbox.
The native summary opens its review panel and uses red boxes too.

Condition analysis was already part of automatic card processing. An additional
fixture-backed mock scan now verifies that a standard blue back receives and
persists a condition check without any explicit client check request, including
when the catalogue recognizer fails. These tests feed saved bytes through the
mock pipeline and do not operate the physical scanner.

Missing observations were backfilled for 146 eligible saved identification
sources across B-007–B-014. Every field other than `conditionCheck` was preserved;
no hardware scan or image rewrite was performed. Eligibility requires an existing
identification tied to a prepared image, so this is not a claim that every old
capture was assessed. B-013/C-001's back has one candidate light patch near its
top edge; this is an unconfirmed observation, not a physical damage label.

The 103-test Swift run included saved-back mock scans with and without catalogue
availability. Twenty-one web tests passed, including source-provenance guards.
Signed service/CLI/MCP/native and HTTP/web builds were installed. Production
browser checks verified the live result links, ordinary image lightboxes, the
B-013 candidate's red box, all fourteen updated B-015 normal finish summaries,
mark-to-mark focus, fixed-size number labels and phone/dark-mode layout. Positive
multi-mark tests used isolated browser fixtures and did not alter production data.
Evidence is retained under ignored `Diagnostics/condition-lightbox/`.
