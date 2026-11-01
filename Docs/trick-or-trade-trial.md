# B-027 Trick or Trade scan — 2026-11-01

The user loaded six Trick or Trade cards and explicitly requested this scan.
B-027 (`DB3A322C-FE37-4A1D-9EFF-C387FCEF113A`) completed with twelve original
PNGs and twelve automatically prepared images in `/Volumes/scans/B-027`.
Driver-reported settings: 600 DPI, duplex, RGB 8-bit, black background and
overscan (cropping value 4). Multifeed settings were left unchanged.

The user confirmed all six cards fed cleanly with no new damage. Each consecutive
capture pair has one verified upright text/front preparation and one verified
Pokémon-back preparation. All six fronts have a single base-card candidate.
Back images appropriately have no catalogue match.

## Frozen automatic predictions

Predictions were saved before any physical finish labels were supplied. The
software revision was `422ee9b`, using `modern-set-code-and-border-v5` and the
separate modern ex catalogue rule. No thresholds or rules were changed for this
batch, and no labels were inferred from the product name.

| Card | Base-card match | Catalogue ID | Automatic finish |
| --- | --- | --- | --- |
| C-001 | Greavard | sv01-104 | Normal |
| C-002 | Mimikyu | sv02-097 | Holo |
| C-003 | Dusclops | swsh4-70 | Unsupported |
| C-004 | Gastly | swsh11-064 | Unsupported |
| C-005 | Drifblim | sv01-090 | Normal |
| C-006 | Spectrier | swsh11-081 | Unsupported |

The three Sword & Shield layouts fall outside the current modern English
set-code detector. Mimikyu has stable positive border evidence in all three
measurement variants. The two normal estimates retain independent marker and
catalogue evidence; absent texture itself does not establish normal.

Physical finish labels were not available when these predictions were recorded.
The sealed prediction SHA-256 is
`2433687d08a22f0d9412736669e0870881e4261ca63d3ad2b516c75ac64d6cda`.
Snapshots, predictions, physical feed confirmation and integrity results are
retained in ignored `Diagnostics/trick-or-trade-20261101T205046Z/`.

## Physical scoring

The user subsequently confirmed that C-002 Mimikyu and C-006 Spectrier are the
only holos. C-001 Greavard, C-003 Dusclops, C-004 Gastly and C-005 Drifblim are
normal; no reverse holo was reported. These labels are stored separately in
`physical-labels.json`; `physical-scoring.json` scores the unchanged sealed file.

- Three cards received estimates, all correct: Greavard normal, Mimikyu holo and
  Drifblim normal. No incorrect finish was issued.
- Three cards remained unassessed: Dusclops, Gastly and Spectrier. This is 3/6
  classification coverage (50%), with 3/3 correct among issued estimates.
- One of the two physical holos was detected. Spectrier was unsupported, not
  incorrectly called normal. There were no false holo calls among four normals.
- With no physical reverse holos, this batch cannot evaluate reverse-holo detection.
- The sample is too small for a general accuracy claim. The three abstentions
  follow the detector's unsupported Sword & Shield scope, not a threshold that
  was retuned after receiving the labels.

The sealed prediction checksum remains unchanged. No scanner operation, detector
change, catalogue-identity confirmation or manual finish/condition override was
made while recording these physical test labels. The next coverage gap is older
layouts and stamped-edition recognition, with their distinct catalogue variants.

## Edition limitation

The current recognizer matches name, collector number and printed set total. It
does not detect the Trick or Trade pumpkin stamp or select stamped catalogue
variants. The stamp is visible in the two prepared fronts spot-checked here,
Greavard and Mimikyu. The six returned IDs therefore identify the underlying
cards, not verified Trick or Trade editions.

The pricing provider excludes stamped variants when choosing ordinary-printing
prices. Displayed prices must not be treated as verified Trick or Trade values.
Resolving the edition requires explicit stamp/variant recognition before applying
edition-specific finish or pricing rules. Neither a base-card match nor the
batch's product description silently confirms the edition.

## Integrity verification

All twelve originals and twelve prepared images matched recorded file sizes and
SHA-256 values. All paths remained under the configured B-027 directory. The
SMB manifest and authenticated HTTPS job response matched the completed service
record. Reviewed finish and condition values remained unverified/unassessed.
The scanner returned no job errors. No code or detector settings were changed
and no Store send was requested.
