# Camera prices, recent scans and binder pages

Open `/camera`. The history button opens **Recent scans**; the grid button opens
**Binder page**. These additions keep the existing camera layout and preview.

Matched cards appear in a local IndexedDB list (up to 100 captures) with their
image, time, language, quantity and selected finish. Repeated video matches do
not add quantities. Adjust quantity for additional physical copies. Separate
binder pockets remain separate captures even when their identities match.
Removing a recent entry does not remove an inventory card. Browser storage can
be cleared or evicted; use **Add to inventory** to keep reviewed captures in the
scanner's saved library.

Prices load separately from recognition using the existing Cardmarket/TCGdex
cache and ECB conversion to GBP. English Pokémon is supported. Unknown finishes
show a range only when all applicable variants have usable prices. Stamps,
missing quotes and unsupported languages can leave the price unavailable.
These are dated market guides, not condition grades or guaranteed sale prices.

Select entries, check the printed set/number, finish and quantity, then choose
**Add to inventory**. This requires the normal scanner session. Each capture is
an idempotent import: retrying after a lost response cannot add it twice. Saved
images and confirmed identifications use single-sided batches in the existing
ledger and storage root. Interrupted storage writes remain visible as failed
batches and are excluded from inventory; they require recovery before that
capture ID can be reused. Bulk saves retain successful entries if a later entry
fails.

Existing inventory records, reviews, images and IDs are unchanged. Camera
recognition and saved-scan recognition share catalogue/matching components, but
the upright identity fast path is currently camera-specific. Existing records
are not silently reidentified. A printed card already in the catalogue is
eligible for camera recognition, but being in inventory is not an accuracy
guarantee and does not automatically add it to the visual reference index.

## Binder workflow

Take or choose a page photo. The native detector proposes up to 24 card boxes.
Check them, skip empty pockets, then **Identify selected cards**. Each crop is
submitted sequentially to the same camera recognizer, with progress and a retry
button. Ambiguous candidates require selection using the printed set and number.
Keep reviewed matches in Recent scans, then use the same inventory save flow.
A 2×2, 3×3, 3×4 or 4×4 pocket grid with an adjustable page margin is available
when automatic detection is unavailable or misses a pocket.

The browser retains up to 4096 pixels on the long side for crops and sends a
smaller detection image. Use the original photo: reducing an entire page to a
small image discards the tiny collector numbers that OCR needs. This first
version has been checked with synthetic pages made from saved scans; real
sleeves, glare and perspective still need a representative binder-photo set.

## Optional native detector

Production uses `scanner-recognition::binder::BinderDetector` with ONNX Runtime,
RGB letterboxing to 640×640, a one-class YOLOv8 `[1,5,8400]` output, confidence
threshold 0.35, and IoU suppression 0.45. Detection is not identity evidence.
The graph is checked against a configured SHA-256 at startup. DINO and binder
sessions share one explicit `ModelRuntime` environment and must use the same
ONNX Runtime library. The real-model integration test can be enabled with
`SCANNER_MODEL_TEST_CONFIG=/absolute/service.json cargo test -p scanner-recognition --test native_models`. No Python process
runs in the application. Without this configuration, the grid workflow works.

The evaluated weights are the `pokemon_detector4` checkpoint from
[ShreyShingala/Pokemon-Card-Scanning-Webapp](https://github.com/ShreyShingala/Pokemon-Card-Scanning-Webapp/tree/c75f8e721f3eb86b3081ea8ddca81f2e333bc8e4).
Weights and private test photos are not included in this repository.
Development conversion follows [Ultralytics ONNX export](https://docs.ultralytics.com/modes/export/):

```sh
python3 -m venv /path/to/development-venv
/path/to/development-venv/bin/pip install ultralytics==8.3.228 onnx==1.19.1
/path/to/development-venv/bin/python Tools/BinderModel/export.py /path/to/models
```

Add the printed graph hash and absolute paths to service configuration:

```json
{
  "binderDetection": {
    "runtimeLibrary": "/absolute/path/to/libonnxruntime.dylib",
    "model": "/absolute/path/to/best.onnx",
    "sha256": "<exported graph SHA-256>"
  }
}
```

Use the appropriate ONNX Runtime library for Linux/Windows. OCR configuration
is also required. Export tested with Torch 2.15.0, Ultralytics 8.3.228 and ONNX
1.19.1; that graph hashes to
`5977c196f195583a492adbe1e8d4c48213dec8fb1929fbdb55e9bca67acd5d46`.

`binder_probe` reports native detector timing and normalized boxes for bounded
JPEGs without writing inventory. On Apple M4, a synthetic nine-card page was
localized in 26–33 ms (nine boxes, three runs). Replaying original-resolution
crops through the existing camera endpoint returned five exact matches, one
ambiguous result and three no-matches, with no wrong automatic matches. The nine
identity requests took about 13.5 seconds total; locating all boxes does not mean
identifying the whole page in 30 ms. Basic Energy, a full-art Salamence and one
Mewtwo crop remained unresolved. A downsampled page performed substantially
worse. These results justify keeping per-pocket review/retry and do not establish
real binder accuracy.
