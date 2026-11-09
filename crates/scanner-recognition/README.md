# Portable recognition

This library calls Tesseract and ONNX Runtime in-process. It does not launch an
interpreter, command-line program or sidecar. Native library paths and Tesseract
trained-data paths are explicit inputs; packaging must supply each platform's
libraries. Windows and Linux execution still require validation on those systems.

`DinoEncoder` accepts only the checked graph and external weights for
`onnx-community/dinov3-vits16-pretrain-lvd1689m-ONNX`, revision
`48988dfe73065df8d6f5ccc0edc7c8bcf307de41`. It produces a normalized 384-dimensional
CLS vector after sRGB, 224×224 triangle resampling and ImageNet normalization.
That preprocessing version is part of the index key: embeddings from other
pipelines must be regenerated. Model weights are not committed or redistributed.

The initial exact cosine index exposes candidates only. A high score cannot prove
identity when the real printing is absent, distinguish foil finish, or confirm
condition. Catalogue/text checks and user review remain required.

Read-only development regression:

```sh
cargo run --release -p scanner-recognition --example benchmark_saved -- \
  INPUT_JSON REFERENCES_JSON MODEL_ONNX ONNX_RUNTIME_LIBRARY \
  TESSERACT_LIBRARY TESSDATA_DIRECTORY OUTPUT_JSON
```

On 2026-11-09, the 69 saved fixtures and 121 public references from the earlier
benchmark produced 48/48 correct first-ranked matches among reference-covered
fronts. The remaining inputs were 15 reference-missing fronts and six backs.
All 69 also ran through native Tesseract 5.5.1 with the installed English data;
OCR text output is retained locally for matching comparisons. No saved scan or
OCR text is uploaded by these engines or the benchmark.

Regional OCR and orientation regression commands:

```sh
cargo run --release -p scanner-recognition --example ocr_saved -- INPUT_JSON TESSERACT_LIBRARY TESSDATA_DIRECTORY OUTPUT_JSON
cargo run --release -p scanner-recognition --example match_saved -- CATALOGUE_JSON METADATA_DIRECTORY OUTPUT_JSON combined
cargo run --release -p scanner-recognition --example orient_saved -- INPUT_JSON TESSERACT_LIBRARY TESSDATA_DIRECTORY ORIENTATION_JSON
```

The regional benchmark retains whole/title/footer/left/right reads independently.
Its combined variant is an evaluation, not an automatic acceptance policy. With
complete metadata for the evaluated candidates it yields 38 unique correct fronts,
four ambiguous and 21 unresolved, with no incorrect unique match; all six backs
remain unmatched. The same whole-only pass yields 17 correct fronts. Aggregate
results are in `Tools/RecognitionBench/results-rust-2026-11-09.json`.
