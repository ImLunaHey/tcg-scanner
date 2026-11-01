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

## Second experiment — independent border texture

The first detector required the left and right borders' fine bands to correlate.
That is unnecessarily restrictive when the visible foil pattern differs between
sides. `FoilTexture.swift` measures twelve alternative cues without requiring
matching phases: brightness bands, opponent-colour bands, mean chroma, colour
spread and brightness spread on the borders, plus illustration/body bands.
Eight-section medians also test whether a signal depends on one small area.
All pixel processing remains native Swift/CoreGraphics/ImageIO on this Mac.
`EvaluateFoil.py` is offline development analysis of JSON numbers, never a
runtime dependency of the scanner or its clients.

The baseline measures border strips at x=1.5–4% and 96–98.5%, y=12–86%.
The `inset` variant narrows those strips to 2–3.5% and 96.5–98%; it is not a
rotation or complete crop-perturbation test. The `630` variant reduces the
maximum analysis dimension from 1260 to 630 pixels; it is not an actual
300 DPI capture. Actual 300 DPI controls were evaluated separately below.
ImageIO keeps smaller inputs at their existing size. The row smoothing window
scales approximately with image height. Input and output limits prevent source
image replacement; diagnostic outputs must be new files.

Each feature's threshold is `max(calibration-negative baseline) * 1.25 + 0.001`.
All variants and additional batches use that same threshold without refitting.
All twelve results are retained, sorted by calibration positives detected only.
The original name groups/splits are reused. Because those cards have already
been inspected, these results are **exploratory**, not fresh holdout validation.
Leave-name-group-out analysis likewise does not erase dataset selection bias.

### B-024 and B-025 results

| Feature | B-024 holos flagged / 16 | B-025 non-holos flagged / 16 |
| --- | ---: | ---: |
| Minimum of independent border brightness bands | 16 | 0 |
| Maximum of independent border brightness bands | 16 | 0 |
| Median-section border brightness bands | 16 | 0 |
| Border mean chroma | 16 | 0 |
| Border colour spread | 16 | 0 |
| Border colour bands | 16 | 0 |
| Median-section border colour bands | 16 | 0 |
| Border brightness spread | 9 | 0 |
| Illustration brightness bands | 2 | 0 |
| Illustration colour bands | 1 | 0 |
| Body brightness bands | 0 | 0 |
| Body colour bands | 0 | 0 |

These are baseline counts. The minimum-border brightness cue and all four
colour cues kept the same outcomes in both variants. Maximum-border and
median-section brightness cues each missed one holo after downsampling.
Illustration/body colour or brightness sometimes falsely flagged a non-holo
after downsampling. Whole-name exclusion also produced 16 positives and zero
false positives for each of the seven strongest baseline border cues.

### Additional controls expose misleading colour cues

The user previously physically labelled B-014's 15, B-015's 14 and B-016's 22
cards as non-holo, including the corrected Volcarona. Forty-nine had an upright
prepared front opposite exactly one verified Pokémon back. B-015/C-006 and
C-009 were excluded because their companion back orientation was unknown.
These 300 DPI negative controls span eleven first-candidate sets, including
sets represented among the positives. No new scan or finish override occurred.

B-013/C-005–C-020 supplied sixteen additional 300 DPI foil-positive captures
from the user's holo-or-reverse statement. Their printings largely match B-024
in reverse order, so these are useful capture-repeatability checks, **not
sixteen additional independent positive printings**. B-024's ambiguous Kilowattrel
candidates differ from B-013's; physical foil labels do not depend on recognition.

Applying the existing thresholds to these controls:

| Feature | B-013 foil captures flagged / 16 | Older non-holos falsely flagged / 49 |
| --- | ---: | ---: |
| Minimum border brightness bands | 16 | 0 |
| Maximum border brightness bands | 16 | 1 |
| Median-section border brightness bands | 16 | 0 |
| Border mean chroma | 16 | 47 |
| Border colour spread | 16 | 49 |
| Border colour bands | 16 | 49 |
| Median-section border colour bands | 16 | 49 |

The colour-only approaches therefore fail to generalise across these captures.
The data do not isolate whether resolution, scanner processing, print texture,
colour handling or crop differences cause the failure. Do not call the colour
separation in B-024/B-025 a successful general detector.

### Most promising candidate and remaining limits

`borderMinimumBand`, threshold `0.008213026997467985`, currently performs best
when requiring the signal on both borders. It detected all 16 B-024 holos and
all 16 B-013 foil captures in all three variants. None of the 65 measured
non-holos exceeded the threshold at the baseline position. Nine older non-holos
crossed it only with the narrower strips: B-014/C-002 and C-013;
B-015/C-002, C-004, C-005, C-007, C-010 and C-014; B-016/C-006.

A conservative interpretation requires agreement across all variants: these
nine are **unstable**, not confirmed foil or confirmed non-holo. The other 56
non-holos have no foil signal in any variant. All 32 positive captures have a
stable signal, but the weakest B-024 minimum is only `0.008477184617987788`,
about 3% above the threshold. There is insufficient margin or independent data
to advertise production accuracy. Absence of this signal is never proof of
non-holo, and the check does not distinguish holo from reverse holo.

Before deployment, freeze the candidate and test a fresh mixed batch including
same-set normal/holo examples, weak-signal holos and the normal Volcarona. Include
yellow-border, full-art and nonstandard layouts in separate scope tests; the
current strips assume a conventional modern border. Preserve an uncertain
outcome for crop-sensitive or out-of-scope cards. No production detector,
saved finish estimate, profile default or Store staging record was changed.

### Reproducing the measurements

```sh
swiftc -O -warnings-as-errors Scripts/Experiments/FoilTexture.swift -o /tmp/foil-texture
/tmp/foil-texture --self-test
python3 Scripts/Experiments/EvaluateFoil.py --self-test
/tmp/foil-texture inputs.json new-measurements.json --variants
python3 Scripts/Experiments/EvaluateFoil.py new-measurements.json groups.json new-evaluation.json
python3 Scripts/Experiments/EvaluateFoil.py --apply new-evaluation.json extra-measurements.json extra-evaluation.json
```

The native synthetic checks cover flat, opposite-phase, chromatic and isolated
bands. Evaluator checks cover calibration isolation, whole-name exclusion,
fixed thresholds across variants, missing-variant rejection, all-positive
control input and unstable outcomes. `--apply` never refits a threshold and
requires the same complete variant set for every card. The real run measured
97 prepared images in all three variants (291 measurement rows); source PNGs
were only read. Complete results and control exclusions are retained in ignored
`Diagnostics/foil-experiment/` alongside the first experiment.
