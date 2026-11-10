# Camera latency benchmark

Development-only tools; the application never invokes these executables/scripts.
Use a private output directory. Do not commit user images, service configuration,
credentials or raw results. No scan records or hardware are modified.

1. Supply a JSON array of `{ "id": "label", "path": "/absolute/image.png",
   "expected": "catalogue-id", "cameraFrame": false }`. `expected: null` means
   a negative fixture. `cameraFrame: true` preserves a camera JPEG as supplied;
   otherwise the saved scan is resized and padded onto a dark 1000×1333 scene.
2. Run `cargo run --release -p scanner-recognition --example camera_components -- INPUT SERVICE_CONFIG OUTPUT_DIRECTORY`.
3. Run `python3 Tools/CameraBench/run.py OUTPUT_DIRECTORY/components.json HTTPS_SERVICE_ORIGIN OUTPUT_DIRECTORY/http.json`.

Components are measured separately, three times per fixture: extraction,
Tesseract initialization plus orientation, and DINO encoding plus index search
(including its decode). Their sum is **not** the complete recognition pipeline.
The installed service API is then tested three times per fixture in seeded shuffled
order. HTTP time includes sending the prepared JSON request and reading/parsing the
response, but excludes phone capture, JPEG/base64 encoding, and waiting for a slot.
The service stays running; these are not cold-start measurements. A first request
can still pay catalogue cache costs. Stop other camera clients during measurement.
The runner includes failures, unresolved results and wrong matches in raw results;
calculate successful-match latency separately. Nearest visual references are not
confirmed identities. Keep coverage and accuracy alongside every speed comparison.

## Baseline, 10 November 2026

Backend `b0add92`, web `9dafa15`, release build on this Mac, through its HTTPS
Tailscale service URL. Eight fixtures × three requests = 24 requests. One frame
is a guide crop derived from the user's Drowzee phone screenshot; the other seven
are synthetic camera scenes made from saved upright scans. This small diagnostic
sample is not an estimate of accuracy on arbitrary handheld camera video.

| Fixture | Median HTTP time | Outcome, all three attempts |
| --- | ---: | --- |
| Drowzee 043/103 | 1,143 ms | Correct match |
| Eevee 30th-117 | 1,286 ms | Correct match |
| Lapras 30th-017 | 1,478 ms | Correct match |
| Dunsparce sv09-120 | 1,255 ms | Correct match |
| Mewtwo 30th-063 | 1,838 ms | No match |
| Basic Grass Energy mee-009 | 743 ms | No match |
| Salamence ex sv09-177 | 60 ms | No card outline |
| Standard card back | 2,553 ms | No match; no front-card false positive |

Four of seven front fixtures matched (12/21 front requests), three remained
unresolved (9/21). No wrong confirmed matches; all three back requests returned
no match, but failed to give the more useful card-back guidance.
Successful matches: **median 1,283 ms; nearest-rank p95 1,541 ms (n=12)**.
Fast rejections are excluded from these successful-match figures.

Extracted scenes took approximately 44–48 ms to decode/extract. Per-fixture
median orientation times ranged 184–644 ms; DINO retrieval 23–26 ms. DINO model
startup was 233 ms, excluded from repeated inference times. The missing-outline
fixture took 31 ms locally and never reached either recognition stage.

The 121-reference index lacks Drowzee me06-043. It returned a different nearest
reference for Drowzee, the energy and the back. Covered Mewtwo had the correct
nearest visual reference despite failing text confirmation. Therefore a fast
nearest-neighbour result cannot replace exact-printing confirmation safely.

## Performance targets and next experiments

- Measure confirmed identity under 1,000 ms end to end, separately from first
  visual feedback around 100 ms. Neither target is achieved by this baseline.
- Test an upright-first visual path that does not wait for OCR orientation,
  retaining rotated/OCR fallbacks and labelling visual suggestions unconfirmed.
- Expand reference coverage, measure missing-reference rejection, and test
  visually identical reprints before promoting visual suggestions automatically.
- Profile identity OCR and finish estimation independently; return identity
  before optional finish analysis if that removes meaningful latency.
- Re-run these fixtures plus real phone recordings, changed-card/stale-result
  checks, rotations, glare and textured backgrounds after each experiment.

## Upright-first experiment, 10 November 2026

Backend `6454923`, web through `5105a91`. Same eight prepared frames and seeded
24-request schedule, rerun after confirming that the restarted service was ready.
A separate startup attempt encountered HTTP 502 while the service initialized;
those transport failures are retained in private startup results and are not
mixed into these warmed-service measurements.

The camera now tries independent upright whole-card and title/footer readings
before orientation, broad retries, visual supplements and finish analysis. Early
return requires one candidate with name, collector number and printed-total
support. Uncertain results still use the existing fallback. Fast responses contain
identity only, without a finish estimate; this comparison therefore measures time
to identity, with finish removed from the critical path, not faster completion of
all previous work.

Successful-match median **392 ms**, nearest-rank p95 **507 ms** (n=12).
Four of seven front fixtures still match consistently, three remain unresolved;
no wrong confirmed matches and no HTTP errors across the 24 requests. The back
still returns no match. Salamence now reaches an ambiguous fallback rather than
being rejected at outline extraction. Those unresolved paths can still take
seconds. This is progress toward subsecond identity, not evidence of 200 ms
recognition or improved overall accuracy.

Separately, browser results now retain a successful captured match even when the
live view moves, with its own captured thumbnail and the existing Last card label.
It does not mark the changed live view as matched. Reset, close, and detail-session
boundaries still ignore stale responses. Diagnostics sit below the card guide.
