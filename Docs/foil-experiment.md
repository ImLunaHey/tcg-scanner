# Local foil appearance experiment

This is an offline measurement experiment, not the production finish detector.
It reads already upright prepared front PNGs with native Swift/ImageIO and never
scans hardware, contacts an external API, or changes recognition/review values.

```sh
swift Scripts/Experiments/FoilFeatures.swift --self-test
swift Scripts/Experiments/FoilFeatures.swift inputs.json measurements.json
```

Input is an array of objects with `id`, absolute `path`, physical `label`, and
`printingIDs` (an array of catalogue candidate IDs). Labels come from the user's
physical checks, never the existing finish estimator. Image limits are 32 MiB,
40 megapixels, one frame and 20,000 pixels per axis; manifests contain at most
200 entries. Output must be a separate diagnostic JSON file. Original scans and
service metadata are never written.

Measurements cover left/right borders, artwork and text-body regions. They
include brightness, chroma, high-frequency row variation and correlation between
border rows. The initial candidate signal is the smaller border band energy
multiplied by positive left/right correlation. This is an observable image
feature, not a direct reflectivity measurement. Printed textures, scanner banding,
skew and card design can mimic it. These fixed regions target ordinary modern
card layouts, not full-art or every historical printing.

## Frozen pilot evaluation protocol

Positive set: B-024, 16 physically reported holo cards at 600 DPI. Negative set:
the subsequent batch of 16 physically reported non-holos, with the same scanner,
resolution, background and overscan settings. Preserve the user's distinction
between physical labels and predictions. Ambiguous identity does not erase a
physical foil label, but makes printing-specific evaluation less certain.

Before inspecting the negative measurements: group by the first candidate's
lowercase Pokémon name (use the card label if no name was identified). Put all
copies of a name in the same split. Use the first SHA-256 byte modulo two: zero
is calibration, one is holdout. Set the candidate threshold to the maximum
calibration-negative signal plus 0.002; require at least one calibration negative.
Report false positives and detected positives separately for each split. Do not
retune against holdout results or advertise this small pilot as production
accuracy. The batches may have set/artwork confounding and only a few examples
per foil treatment; independent matched-printing validation is still needed.

Measurements, manifests and outputs belong in ignored `Diagnostics/foil-experiment/`.

## Pilot result — 2026-11-01

B-025 (`A2DAAF14-4915-4998-85C9-1AE09A4C3141`) captured the 16 known
non-holos as 32 PNGs at 600 DPI, duplex, black backing and overscan. The user
confirmed all 16 fed cleanly without jams, overlapping feeds or visible new
damage. Automatic processing completed with no job errors: 32 prepared images,
32 identification records and 32 software-verified orientations. All original
files matched their recorded SHA-256 hashes. A standard Pokémon back was found
in each consecutive capture pair; the opposite prepared image supplied the
front measurement. Original files were not modified.

The frozen name-group split produced 13 positive / 8 negative calibration cards
and 3 positive / 8 negative held-out cards. The threshold was
`0.0021377615316729875` (maximum calibration-negative signal plus 0.002).

| Split | Known holos flagged | Known non-holos incorrectly flagged |
| --- | ---: | ---: |
| Calibration | 1 / 13 | 0 / 8 |
| Holdout | 2 / 3 | 0 / 8 |

Only B-024/C-001 Tornadus, C-003 Emboar and C-004 Lillie's Ribombee exceeded
the threshold. The other 13 physically reported holos did not. This rejects the
initial paired-border band signal as a sufficient general foil detector. Zero
false positives in this small sample does not establish reliability, and the
three-positive holdout is too small to estimate sensitivity meaningfully.
The threshold was not retuned after examining holdout results. Production
finish estimates and manual reviews were not changed by this experiment.

All negative cards have first-candidate IDs from the same set (`me04`), whereas
the positives span other sets. That is a substantial design confound: separation
could reflect printed borders rather than foil. A future candidate signal needs
independent, physically checked examples including matching printings with both
finishes. Absence of visible foil evidence must not be treated as proof of
non-holo. This experiment also does not show that foil is impossible to detect
from these scans; it tests one specific visual cue.

Validation: the Swift synthetic flat/banded image checks passed, and the native
measurement script successfully read all 32 prepared fronts. Local input
manifests, name groups, feature measurements and the frozen-threshold evaluation
are retained in `Diagnostics/foil-experiment/`. No card images left the Mac.

## Observed resolution tradeoff

Capture timings below use persisted service events from scanning to processing;
the median interval measures consecutive completed duplex pairs. Prepared size
counts only the latest front/back output per card, excluding preserved originals.

| Batch | DPI | Cards | Capture seconds | Median seconds/card | Prepared MB/card |
| --- | ---: | ---: | ---: | ---: | ---: |
| B-023 | 300 | 3 | 10.77 | 2.58 | 3.70 |
| B-024 | 600 | 16 | 72.89 | 4.08 | 14.61 |
| B-025 | 600 | 16 | 71.47 | 3.88 | 15.31 |

All three used overscan. These are different cards and the 300 DPI sample is
small, so this is indicative rather than a controlled benchmark. B-025 took
about three minutes including automatic processing. Prefer 300 DPI for bulk
throughput; 600 DPI is an image-quality choice for detailed listings, with
roughly four times the prepared storage in these samples. This pilot provides
no justification for selecting 600 DPI solely to obtain reliable foil detection.
The default profile was not changed.
