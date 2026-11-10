# Portable visual review with DINOv3

The Rust service can load a local DINOv3 index alongside Tesseract. ONNX Runtime
runs in process; scan pixels are never uploaded. The native application and Rune
WebAssembly client use the same service. The browser does not need an Apple OCR
framework or a model download.

Visual matches are stored in `evidence.visualMatches`, separately from printed
text candidates. They are nearest artwork, not verified set/collector-number
matches. Automatic pricing, finish assessment, inventory identity and Store
staging continue to use printed candidates or explicit reviewed identities.
Rune offers visual choices with a printed-detail reminder. Only an authenticated
manual review promotes the chosen visual candidate to a confirmed printing.
Camera results display the suggestions without saving an identity.

The recognizer requires a known Pokémon context for visual-only suggestions;
English references never substitute for Korean. Camera images with no reliable
Pokémon context still follow text/game recognition. One owned inference runs at
a time, and cancellation does not release the worker while native inference is
still executing. If optional visual lookup fails, text evidence is retained.

## Build an index

`scanner-index` consumes an explicit local manifest of public reference images:

```json
{"references":[{"id":"sv06.5-099","path":"/absolute/public/reference.png","sha256":"64 lowercase hexadecimal characters"}]}
```

Each image must match its checksum, fit within 8 MiB and 16 megapixels, and have
one unique catalogue ID. The index supports up to 50,000 references, stays below
256 MiB, and records the language, model revision, preprocessing and descriptor
version. The builder writes an atomic new artifact and refuses existing output.
No scan ledger is read or modified.

```sh
scanner-index --references /absolute/references.json \
  --runtime-library /absolute/libonnxruntime.so \
  --model /absolute/model.onnx --language en \
  --output /absolute/dinov3-en-v1.json
```

Use the native ONNX Runtime library for the operating system (`.dylib`, `.so`, or
`.dll`). The model and adjacent `model.onnx_data` must match the pinned checksums
in `scanner-recognition/src/embedding/onnx.rs`, revision
`48988dfe73065df8d6f5ccc0edc7c8bcf307de41`. An index built for another model or
preprocessing version is rejected. Loading a configured invalid index fails
startup; it never silently uses incompatible descriptors.

Add this optional section to the Rust service configuration, alongside the
existing `tesseractLibrary` and `trainedData` settings:

```json
{"visualRecognition":{"runtimeLibrary":"/absolute/libonnxruntime.so","model":"/absolute/model.onnx","index":"/absolute/dinov3-en-v1.json"}}
```

All paths must be absolute existing files. Scanner and camera recognition share
one loaded encoder/index. Catalogue names/details come from the existing bounded
public metadata client and cache. Updating the index requires an explicit new
artifact and service restart; no model or reference images are committed to Git.

## Saved-card validation, 10 November 2026

The production artifact contains the same 121 public artwork references used in
the earlier experiment. On 63 saved fronts, 48 have their expected printing in
that reference set. The production retrieval path puts the correct expected card
first for all 48 covered fronts. Fifteen fronts are outside reference coverage;
the nearest result for those is not evidence of a correct printing.

Replaying the retained independent Tesseract readings produces 35 correct unique
text matches. Adding separate visual review options makes the correct card
available as either that unique text result or the first visual suggestion for
51/63 fronts, including 16 without a correct unique text result. This is a
review-assistance measurement, not 51 automatic confirmations or a calibrated
confidence claim. All original printed candidate lists remain byte-for-byte
unchanged. No similarity threshold was fitted to turn missing references into
accepted identities. Wider reference coverage and independent camera/printing
validation remain necessary.

The opt-in Rust test uses `SCANNER_BENCH_DIR`, `SCANNER_VISUAL_INDEX`,
`SCANNER_ORT_LIBRARY`, and `SCANNER_DINO_MODEL` with absolute paths. It verifies
saved image checksums, uses the actual persisted index and native encoder, checks
language isolation and candidate preservation, and reuses local metadata snapshots.
Missing public metadata is fetched by card ID; images stay local.
`SCANNER_VISUAL_REPORT` optionally writes the measured results.

A separate fresh end-to-end test (`SCANNER_FRESH_VISUAL=1` plus the native
Tesseract library/data variables) passes current-010, regression-016 and hard-046
through the actual recognizer, not retained OCR. All three produce the expected
first visual option; Inkay still has no printed text candidate. This targeted
regression check is exploratory and does not establish unseen-card accuracy.
