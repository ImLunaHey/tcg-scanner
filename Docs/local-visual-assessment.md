# Local finish estimates and condition assessment

The user requires all card-image analysis to stay on this Mac. Catalogue GETs and
model-weight downloads are permitted; card images and OCR text are not uploaded.
The live application's existing OCR identifies card names/printings. It does not
establish physical foil, surface damage or a verified condition grade.

## Physical validation supplied by the user

For the saved B-009 batch, the user confirmed:

| Card | Capture sequence | Physical finish |
| --- | --- | --- |
| C-002 Larvesta 028/159 | front 4, back 3 | Reverse holo (shiny background/text area) |
| C-009 Lechonk 139/159 | front 18, back 17 | Non-holo |

These checks were performed after the scans. No new scanner job was needed.
Condition grades were not supplied; earlier reports of no new feeding damage do
not establish that a card has no pre-existing wear or hidden surface defects.

## Local Swift experiment

[Tools/LocalVisionProbe](../Tools/LocalVisionProbe) uses MLX Swift LM 3.32.3 and
Swift Transformers 1.3.5. Xcode built its Metal shaders after installing Apple's
Metal Toolchain. Inference ran locally on saved cropped PNGs; neither images nor
prompts were sent to an inference API. The main service has not gained an inference
sidecar or a runtime dependency on this development tool.

Tested model snapshots:

- `lmstudio-community/Qwen3-VL-4B-Instruct-MLX-4bit`, revision
  `552af30c9952c44f1e1a27c7c5810ded58e892bc`.
- `mlx-community/Qwen3.5-4B-4bit`, revision
  `0e7ffd5c629ef7719d4cbc04069232580bfa9d9c`.

The first model could be prompted to return reverse holo for Larvesta, but then
also invented a repeating foil pattern on the physically non-holo Lechonk. The
second model returned normal for both cards under multiple finish prompts. It
could read Larvesta's name and attack and describe flame-like background motifs,
yet did not reliably translate those observations into the correct finish.
Responses also sometimes violated the requested JSON shape or confused reverse
holo with the back of the card. Prompt wording materially changed predictions.

Paired-card runs took roughly 10–17 seconds including model loading in this local
probe. One measured run peaked around 5.8 GB of memory. These small experiments
are neither an accuracy benchmark nor production throughput measurements.

The models suggested near mint on these clean-looking scans, but that has no
independent physical-condition validation. Scans cannot establish absence of
hidden dents or fine surface scratches. A generic model's confident prose is not
proof of grade, and no predicted finish or condition was written into scan records.

## Current outcome and next gate

The generic vision-model experiment was **not enabled**: those physical finish
checks failed. A separate, constrained printed-marker estimator is now available
as described below. Automatic condition grading remains disabled. Manual
assessments remain authoritative. TCGdex's finish variants
are available printings, not the detected finish of a scanned copy. A unique
catalogue finish may support a clearly labelled inference, but must not silently
confirm the physical card or override a user's review.

Before integrating an estimator, collect a labelled scan set spanning non-holo,
holo and reverse holo plus physically inspected condition examples, including
worn cards. Separate calibration examples from held-out validation. Evaluate
visible defects, uncertainty/abstention and per-class errors, then expose tentative
results with provenance and explicit human confirmation. Do not use the two cards
above both as prompt examples and as an independent accuracy test.

A production implementation must remain in-process Swift, bound memory/time and
concurrency, preserve originals and confirmed reviews, validate structured model
outputs, and recover interrupted estimates without restarting physical scans.

Sources: [MLX Swift LM](https://github.com/ml-explore/mlx-swift-lm),
[TCGdex variant fields](https://tcgdex.dev/reference/card#variants),
[TCGplayer condition overview](https://help.tcgplayer.com/hc/en-us/articles/221430307-Card-Conditioning-Overview).
Model outputs and screenshots are private local diagnostics, not committed data.

## Next local experiments proposed after B-010

These are proposed validation paths, not implemented classifiers:

- **Finish:** after identifying the printing, align the scanned front with a
  trusted reference and inspect the illustration and lower background separately.
  A small classifier trained on physically labelled scanner images could learn
  reverse-holo motifs for a particular layout. Do not infer foil solely from
  colourful artwork or a catalogue's list of variants. Split evaluation by
  physical card, not by repeat scan, to avoid testing on another scan of the same
  training card. Measure abstentions and false positives on non-holo cards.
- **Condition:** start with visible defects, not a whole-card grade. Inspect the
  back border and corners for whitening/chips and both faces for candidate
  creases. Show marked regions for review and record what was actually observed.
  Printed white areas, rounded corners, cropping errors and reflections need
  negative examples. No detected defect does not establish Near Mint.
- **Additional evidence:** an optional angled-light photo of the physical card
  could provide surface evidence absent from the flatbed scan; that photo would
  also be analyzed locally. This is not needed to scan or identify cards. There
  are no new hardware changes or automatic feeding experiments in this proposal.

Apple supplies [image registration](https://developer.apple.com/documentation/vision/aligning-similar-images)
and [contour detection](https://developer.apple.com/documentation/vision/vndetectcontoursrequest)
for the proposed in-process Swift implementation. Their availability does not
validate this classifier. [TCGplayer's defect definitions](https://help.tcgplayer.com/hc/en-us/articles/26141121045143-Understanding-Card-Condition-Imperfections-A-Comprehensive-Guide)
provide concrete review categories including edgewear, scratches and indentation;
physical inspection is still needed for a final selling condition.


## Modern printed-marker estimator — 2026-10-31

`modern-set-code-polarity-v1` is an experimental, deterministic Swift estimator.
It reads the printed set-code box locally and combines its contrast with the
identified printing's `availableFinishes`. No model or image upload is used.
Catalogue versions narrow the inference; they are not a measurement of foil.

The scope is ordinary modern English `svNN` / `meNN` numbered expansions,
including supported decimal subset IDs, with a single candidate, a readable
collector number within the official set total, and a listed reverse-holo variant.
Promos, basic Energy, older layouts, full-art/secret numbers beyond the official
set total, ambiguous identities and unlisted variants are excluded.

- A light code box with dark letters suggests reverse holo.
- A dark code box suggests the regular printing. If the catalogue lists only one
  regular finish (normal or holo), that becomes the tentative estimate.
- If both normal and holo regular versions are listed, the estimator abstains.
  It does not distinguish those by rarity, colourful artwork or an assumed batch
  finish. Missing, tiny, low-contrast or ambiguous markers also produce no guess.

Vision character-range boxes help separate merged regulation marks from the code.
The analyzer uses bounded image dimensions, a small code region, contrast/ink
checks and a gap between light/dark thresholds. It records the region, orientation,
median luminance, polarity, candidate ID, method, reasons and limitations. These
are evidence diagnostics, not calibrated confidence scores. The family/layout rule
is a hypothesis checked on this small local sample; it is not a guarantee for all
print runs or special products. Counterfeits and incomplete catalogue variants
are outside this estimator's validation.

### Physical labels and limitations of validation

In addition to the B-009 labels above, the user physically confirmed:

| B-013 pair | Card | Finish |
| --- | --- | --- |
| C-001 | Gwynn | Non-holo |
| C-002 | Gladion's Final Battle | Non-holo |
| C-003 | Rust Syndicate Grunt | Non-holo |
| C-004 | Team Rocket's Giovanni | Non-holo |
| C-005 | Vileplume | Holo (illustration) |
| C-009 | Indeedee | Holo (illustration) |

The other B-013 Pokémon were described only as holo or reverse holo and are not
exact validation labels. Initial predictions were saved before the clarification;
localization/padding and thresholds were then refined using this sample. These
are development/calibration checks, **not held-out accuracy measurements**.
The final checks allow abstention but reject a wrong emitted estimate. Seven of
eight exact labelled examples receive agreeing estimates; Rust Syndicate Grunt's
code remains unreadable to this routine and is left uncertain. Repeated scans of
Larvesta/Lechonk do not count as independent physical-card examples.

### Service and review behavior

New single-candidate identifications automatically include an optional
`CardIdentification.finishEstimate`. The existing `finish`, `condition`,
`assessmentSource`, identity selection and review timestamps are never automatically
changed. API consumers must not treat `suggestedFinish` as confirmed inventory data.
Original files and identification source hashes/edit IDs remain authoritative.

Existing identifications can be evaluated without rejection, OCR replacement or
hardware access:

- JSON-RPC: `v1.images.estimateFinish` with `jobID`, `sequence`, `identificationID`.
- HTTP: `POST /api/v1/images/{jobID}:{sequence}/finish-estimate` with
  `{ "identificationID": "UUID" }` (authenticated).
- CLI: `scanner images estimate-finish JOB_ID SEQUENCE --identification-id UUID --json`.
- MCP: `scanner_estimate_finish` with the same three parameters.

Repeated requests recompute the same source evidence. Stale identification IDs,
changed source hashes, active image work and unsupported request fields are rejected.
Refreshing preserves human review fields, including a review completed while the
analysis was in progress. Source changes/identity changes cannot overwrite a newer
identification. Storage failures remain errors rather than silent success.

Web and SwiftUI summaries show **Holo / Non-holo / Reverse holo (estimated)** or
**Needs review**. Reviewed values take precedence. Review panels expose reasons and
limitations. **Use finish estimate in review** only fills the form; saving still
requires an explicit review action. Condition grading remains manual.

Sources for the underlying APIs/data:
[Vision recognized-text range boxes](https://developer.apple.com/documentation/vision/vnrecognizedtext/boundingbox(for:)),
[TCGdex card variants](https://tcgdex.dev/reference/card#variants).
Private labels, predictions and validation output are in ignored
`Diagnostics/finish-estimator/`. Saved-batch refreshes did not start hardware.
The separately authorized new-card run is recorded below.


### Deployment and new-card test — B-014

The signed service, CLI, MCP, native application and HTTP/web application were
updated. Existing B-009, B-012 and B-013 identifications received estimates without
changing identity, manual finish/condition, review timestamps or source provenance.
Original file checksums were verified. Browser checks exercised refreshing an
estimate and filling a review form without saving it; neither changed reviewed
fields. Desktop and 390-pixel phone layouts passed. The full Swift suite reported
96 passing tests; opt-in fixture checks were also run separately. All 15 web tests,
TypeScript/Vite and signed release builds passed.

After this implementation was frozen, the user loaded 15 additional cards reported
as normal. One authorized duplex PNG scan produced B-014, job
`61EBDD86-F1A2-4F18-8E65-F8E104F2EB63`. All 30 sides were captured and automatically
prepared, oriented and analyzed, with no job errors. The user confirmed that all
15 cards reached the tray cleanly with no new damage. All 60 original/prepared
file sizes and checksums passed. Every pair has one single-candidate front match;
identities remain suggestions pending review.

Nine fronts received a non-holo estimate; six abstained due to inconclusive
set-code contrast. No holo or reverse-holo estimate was emitted. The six uncertain
cards are C-001 Magnemite, C-002 Magnezone, C-003/C-004 Pachirisu, C-005 Heliolisk
and C-011 Seedot, all from Mega Evolution. Their median set-code luminance was
approximately 0.455–0.495, outside the dark-box threshold. The estimator was not
retuned on these cards, and the user's normal label was not supplied to inference.
This is a small new-card negative-control test with duplicate printings, not a
benchmark for general finish accuracy or sensitivity to foil.

Read-only production browser checks verified all 15 paired identity summaries,
review dialogs and prepared front downloads against checksums, without changing
job history. Desktop and phone layouts passed with no page errors or overflow.
Private requests, records, physical confirmation, initial estimates and validation
are retained under ignored `Diagnostics/normal-finish-test/`.

### Dense set-code lettering correction — v2

B-014 exposed a weakness in whole-box median brightness: dense white `MEG EN`
lettering occupied enough of a black box to put six normal cards into the
uncertain range. Version `modern-set-code-polarity-v2` additionally measures broad,
uninterrupted dark/light background rectangles. It excludes the side border,
requires patches wider than narrow glyph stems and taller than thin outlines,
and requires one polarity to dominate. Blank patches, competing backgrounds,
contradiction with the original contrast measurement and ambiguous catalogue
variants still cannot produce a verified finish.

The bounded Swift routine runs in-process and adds optional
`darkBackgroundAreaFraction` and `lightBackgroundAreaFraction` diagnostics.
All fifteen saved B-014 fronts now suggest normal, without using their batch ID,
card name or supplied finish labels as inference inputs. Existing B-009/B-013
estimates, including the physically checked reverse-holo Larvesta and holo
Vileplume/Indeedee, are preserved in fixture checks. B-014 was used to develop this
correction and is now a regression/calibration sample, not independent validation
of v2. Its original v1 predictions remain in the earlier diagnostic snapshot.

The full Swift suite passed 98 tests; a separate fixture-enabled run covered
B-009, B-013 and B-014, plus synthetic dense lettering, inverse polarity, thin
outlines, blank regions, competing patches and contradictory evidence. All
physical labels remain separate from automated estimates. No new scan is needed
for this correction.

The signed service and CLI/MCP binaries were deployed, and B-014 was refreshed
through the existing finish-estimate API. All fifteen live results are v2 normal
estimates. The refresh preserved every non-estimate field and all 60 original and
prepared PNG checksums. A production HTTPS browser check verified all fifteen
`Non-holo (estimated)` summaries, unchanged manual review, unchanged job data and
phone layout. New scans use v2 automatically. The earlier saved batches were used
for read-only fixture validation and were not rewritten. Private before/after
records and verification are in ignored `Diagnostics/finish-estimator-v2/`.

### Visible condition review aid

A separate local [blue-back border check](condition-checks.md) now flags candidate
whitening and elongated marks for image/physical review. It runs automatically
with card processing and can be refreshed on saved sources. Automatic condition
grading remains disabled; a clean result is not a Near Mint assessment. See the
linked scope and B-015 validation limits before interpreting these observations.
