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
