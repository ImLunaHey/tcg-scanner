# Local finish and condition assessment — evaluation gate

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

Automatic finish/condition assessment is **not enabled**: the physical finish
checks failed. Manual assessments remain authoritative. TCGdex's finish variants
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
