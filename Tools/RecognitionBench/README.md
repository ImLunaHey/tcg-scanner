# Saved-card recognition experiment

These development tools compare native recognition, Tesseract.js OCR ablations,
and DINOv3 image retrieval. They do not change application recognition, operate
scanner hardware, save reviewed identities or upload scans. Node tooling is not
part of the shipped Swift runtime.

## Results — 2026-11-09

69 prepared images: 47 playable fronts from the current B-001 batch, the eleven
B-010 regression fronts, B-049's Binacle, four B-051 Delta Reign fronts, and six
Pokémon backs. There are **63 fronts representing 51 distinct catalogue IDs**;
repeated physical copies are not independent printing classes. Images were
checksum-verified against their manifests and resized to 840 pixels wide. Names
and printed numbers were visually cross-checked using contact sheets. This tests
base printing identity, not stamps, finish, authenticity or physical condition.

The four B-051 entries were previously missing from the catalogue. At experiment
time, TCGdex included me06-026, me06-113, me06-061 and me06-070 with the matching
names, numbers and Delta Reign metadata. `label-overrides.json` records that
correction. They are positive examples, not unknown-card negatives.

| Method | Correct unique / 63 | Wrong unique | No match | Ambiguous |
| --- | ---: | ---: | ---: | ---: |
| Current native recognition | 62 | 0 | 1 | 0 |
| Tesseract whole card | 15 | 0 | 39 | 9 |
| Tesseract regional crops | 26 | 0 | 36 | 1 |
| Whole + regional readings | 33 | 0 | 27 | 3 |
| Narrow bands / block OCR / word boxes alone | 15 | 0 | 46 | 2 |
| All Tesseract readings combined | 38 | 1 | 21 | 3 |

Every OCR variant rejected all six backs. The native failure was full-art Inkay
113/103, which also remained unresolved with Tesseract. In the current batch
alone, native recognition got 47/47; whole Tesseract got 12/47; combined whole and
regional Tesseract got 26/47. The aggressive combination got 30 correct and one
wrong out of those 47.

The wrong suggestion is important: current-004 is Grass Energy MEE 009, but one
footer read produced `008`. Combining that read with the focused crop's readable
`MEE` supplied enough evidence for the matcher to suggest mee-008. Other regions
had read 009 inside merged text that the existing parser did not accept. Thus
blindly combining observations can manufacture apparent agreement. This
experimental variant must not become automatic acceptance logic.

Tesseract 7.0.0 uses one reusable English LSTM worker. The first ablations use
sparse-text segmentation, a 300 DPI hint and preserved spaces. Regions are title,
bottom 32%, and both bottom corners; crops are resized to 820 pixels wide with
grayscale, contrast 1.28 and offset 4. The additional focused ablation uses
1000-pixel narrow bands, normalization, block segmentation and word boxes.
All readings are mapped back to card coordinates and passed through the existing
Swift `CardMatcher`, with the same top-eight detail validation and five-candidate
limit. Native recognition additionally retains its production OCR retries and
symbol analysis. This is an integration comparison with our current matcher,
whose parsing and confidence gates were developed for Vision; it is not a claim
about Tesseract's best achievable accuracy. The narrow-band ablation was added
after inspecting initial misses, so these results are exploratory, not held-out
validation. Multiple crops of one image are not independent camera frames.

### DINOv3 retrieval

Used `onnx-community/dinov3-vits16-pretrain-lvd1689m-ONNX`, revision
`48988dfe73065df8d6f5ccc0edc7c8bcf307de41`, float32, Transformers.js 4.3.2,
ONNX Runtime CPU. Normalize the CLS embedding and rank references by cosine
similarity; register tokens are not included in pooling. No training or
fine-tuning was performed on these cards.

The reference pool requests the expected printing plus up to four other
catalogue printings with the same name for the initial 59 labelled fronts. The
four formerly uncatalogued Delta Reign queries are excluded from corpus
construction, including after correcting their labels. The labels
are used to construct and score the retrieval corpus, never as query-model
inputs. This is a bounded, deliberately relevant corpus, not the full catalogue.
After excluding unavailable and empty image responses, it contained **121 public
catalogue images**, including **42 distinct expected printings covering 48
physical fronts**. All 48 had their correct printing ranked first (and therefore
in the top five). The other **15 fronts lacked their correct reference**: eleven
Energy cards and the four newly catalogued Delta Reign cards. They must not be
counted as successful identification or silently excluded from end-to-end coverage.
Public image downloads were validated before use; reference hashes and failures
are retained in the ignored diagnostic directory.

When the correct reference is removed from each ranking, the nearest wrong
printing reaches **0.868 similarity** (Iris's Fighting Spirit), and an Eevee
alternative reaches **0.841**. These are not probabilities. A universal similarity
cutoff would need an independent validation set and explicit unknown handling.
DINOv3 is promising for candidate retrieval; these results do not justify
confirming exact printings from similarity alone. It did not improve the native
baseline here: its covered cards were already recognized by native OCR, and the
remaining native miss had no catalogue reference image.

### Runtime and browser check

Observed desktop medians were approximately 0.335 s for native recognition,
0.343 s for whole-card Tesseract OCR, 0.763 s for whole+regional OCR, and 0.035 s
for DINOv3 query preprocessing/embedding/search. These are different stage costs,
not a controlled end-to-end speed comparison. Native includes matching and
occasionally network access; Tesseract timings exclude catalogue matching and
image preprocessing; embedding timing excludes index construction and model
startup/download. The model cache is about 82 MiB. An interrupted first embedding
run populated the reference cache before the successful run; its cached startup
time is not a cold-start benchmark.

`browser-smoke.mjs` successfully ran Tesseract's actual Web Worker + WASM build in
headless desktop Chromium against Mewtwo, Dunsparce and the difficult Inkay. All
assets and images were served by intercepted local routes with external requests
blocked; no test server was started. It read Mewtwo and Dunsparce titles, while
Inkay remained difficult. That proves browser execution, not mobile accuracy or
speed. DINOv3 ran in Node's ONNX CPU runtime, not browser WASM/WebGPU. SigLIP2,
combined DINOv3/SigLIP2 embeddings, live multi-frame aggregation, camera rectangle
finding, mobile memory use, glare and binder pages were not tested. All inputs
were already cropped/oriented by the existing Mac pipeline.

## Reproduce

Requires Swift/Xcode, Node, the saved batches and public catalogue/model access.
Private images, raw OCR, metadata snapshots and model caches stay under ignored
`Diagnostics/recognition-bench/`; only aggregate results are committed. Keep the
catalogue snapshot and reference list with the run: live metadata/images can change.

```sh
npm ci --prefix Tools/RecognitionBench
node Tools/RecognitionBench/prepare.mjs /Volumes/scans/B-001 \
  /Volumes/scans/test-archive/20261108T105759 Diagnostics/recognition-bench \
  Tools/RecognitionBench/label-overrides.json

SCANNER_BENCH_INPUT="$PWD/Diagnostics/recognition-bench/input.json" \
SCANNER_BENCH_OUTPUT="$PWD/Diagnostics/recognition-bench/native.json" \
  swift test --disable-index-store --filter benchmarkSavedRecognition

node Tools/RecognitionBench/ocr.mjs Diagnostics/recognition-bench/input.json \
  Diagnostics/recognition-bench/tesseract.json
node Tools/RecognitionBench/focused.mjs Diagnostics/recognition-bench/input.json \
  Diagnostics/recognition-bench/tesseract.json Diagnostics/recognition-bench/focused.json
node Tools/RecognitionBench/references.mjs Diagnostics/recognition-bench/input.json \
  Diagnostics/recognition-bench

SCANNER_BENCH_INPUT="$PWD/Diagnostics/recognition-bench/input.json" \
SCANNER_BENCH_OUTPUT="$PWD/Diagnostics/recognition-bench/tesseract-matches.json" \
SCANNER_BENCH_OCR="$PWD/Diagnostics/recognition-bench/focused.json" \
  swift test --disable-index-store --filter benchmarkSavedRecognition

node Tools/RecognitionBench/embeddings.mjs Diagnostics/recognition-bench/input.json \
  Diagnostics/recognition-bench/references.json Diagnostics/recognition-bench/dino.json
# A Playwright Chromium installation is required for this optional browser check.
node Tools/RecognitionBench/browser-smoke.mjs Diagnostics/recognition-bench/input.json \
  Diagnostics/recognition-bench/browser.json
node Tools/RecognitionBench/summarize.mjs Diagnostics/recognition-bench
npm test --prefix Tools/RecognitionBench
```

References: [Tesseract.js API](https://github.com/naptha/tesseract.js/blob/master/docs/api.md),
[DINOv3 implementation](https://github.com/facebookresearch/dinov3),
[ONNX export](https://huggingface.co/onnx-community/dinov3-vits16-pretrain-lvd1689m-ONNX).
The export uses the DINOv3 license; this experiment does not add model distribution
to the application.
