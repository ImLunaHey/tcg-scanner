Game routing: detected Pokémon pairs use the workflow below. Detected English
Yu-Gi-Oh! fronts use the separate [YGOPRODeck matcher](yugioh-identification.md).
Unknown pairs are not sent to either recognizer.

# Card identification and manual assessment

Automatic finish estimates now combine printed-marker/catalogue evidence with
native border-texture analysis for eligible prepared cards. See
[combined finish estimates](combined-finish.md) for decision rules, limitations,
validation and the B-026 service recheck. Estimated finish remains separate from
the physically reviewed finish field.

Card scans with `prepareCards: true` automatically identify saved images during
post-capture processing. Identification is also available as an explicit operation
on an already saved original. It never
starts the scanner, changes the original bytes or renames B/C labels. English and
Korean are supported as explicit batch settings. This feature was separately requested after the
initial scanner milestones. [Cardmarket GBP price guides](card-pricing.md) were
subsequently added; inventory creation and selling integration remain out of scope.

## Use it

Enable **Auto-crop, orient and identify cards** and select **Card language** when starting a card scan.
After capture, each image is cropped/oriented and identified in sequence; results
appear incrementally while the job is Processing. Both sides are analyzed so the
service does not assume that capture parity identifies the front. Backs normally
produce no match. General document scans with this option off do not run OCR.

### Korean batches

Choose `Korean` before starting the batch. HTTP/JSON-RPC and MCP accept
`cardLanguage: "ko"` alongside `prepareCards: true`; the CLI accepts
`scanner scan --duplex --dpi 600 --prepare-cards --card-language ko`.
That command physically starts a scan and must only be run once the loaded test
cards are confirmed ready. The physical Scan/Enter button still creates English
batches; use an explicit software-started Korean job for these tests.

Language is a batch configuration supplied by the operator, not automatically
detected. A saved identification retry uses that same language; contradictory
language requests are rejected. Existing pending requests retain their original
language during retries. The setting is separate from saved hardware DPI/sides/
format defaults.

This Mac reports Vision support for `ko-KR`; Korean OCR and orientation use it
alongside `en-US` for Latin set codes and numbers. A synthetic Korean-text image
was correctly oriented at all four quarter-turns. That software check is separate
from the B-028 physical test recorded below.

On 2026-11-01, TCGdex's `/v2/ko/cards` returned 239 named cards, restricted to
SV4K, SV4M and SV5K; its broader set list does not imply complete card coverage.
Uncovered cards remain unmatched. Korean matching requires the exact Korean
name and printed number, checks the total when available, rejects conflicting
readable set codes, and cannot drop a visible ex/V-family suffix to select an
ordinary printing. It never borrows an English catalogue entry.

English foil/stamp rules have not been validated for Korean layouts and return
`unsupported` with `languageUnsupported`. English market prices and reference
images are not requested as substitutes. Local crop/deskew and condition
observations still run; they do not establish a physical condition grade.
Store export preserves `language: "ko"`, `languageSource:
"scannerConfiguration"` and Korean suggestion language, and rejects inconsistent
identity metadata. Store already accepts the canonical `ko` language code.

The full suite passed 205 Swift tests; five targeted matching checks passed after
adding the set/suffix safeguards. Web validation passed 37 tests and a production
build. Relevant runtime research and deployment records are under ignored
`Diagnostics/korean-20261101/` and `Diagnostics/korean-cards-20261101/`.

Implementation: `a267b29` — `feat(recognition): add explicit Korean card processing`;
`8f8279e` — `feat(clients): select and preserve card language per scan`.

#### B-028 physical validation — 2026-11-01

The operator confirmed ten expendable Korean cards were loaded, then authorized
one software-started job with `cardLanguage: "ko"`, 600 DPI, duplex, PNG, black
background and overscan. Job `EBDA860A-F945-46F8-9D92-550D8053F8AF` completed
with no scanner errors. Multifeed configuration was left unchanged. The operator
reported all ten cards reached the output tray cleanly, with no jams, overlapping
feeds or new damage. This single test does not establish general media safety.

- Saved 20 original PNGs and 20 automatically cropped/oriented PNGs in
  `/Volumes/scans/B-028`. All 40 file sizes and SHA-256 checksums matched the
  persisted records; the disk manifest, local service and authenticated HTTPS job
  response agreed. Prepared images and identification records referenced the
  correct source hashes and edits. Originals remain unchanged.
- Every prepared side reported verified orientation: the ten backs used the
  Pokémon back pattern and the ten fronts used text. Consecutive captures form
  ten complete pairs, each containing one front and one back. This does not
  independently prove the identity of visually identical backs. Sampled front
  images were upright with complete edges and narrow overscan margins; no manual
  rotation or cropping was applied.
- All identification records retained `ko`. Korean OCR read names and collector
  numbers, including `조로아` / `096/187` and `로토무` / `037/106`. Some small set
  codes and titles were missed, so this is not a perfect OCR result.
- None of the ten fronts matched a catalogue entry. The scans include printed
  `sv8a` and `sv8` set codes, both absent from the queried Korean set list and
  card catalogue. Direct Korean lookups for `SV8a`, `SV8`, `SV8a-096` and
  `SV8-037` returned HTTP 404. The catalogue coverage gap prevents matching even
  when OCR reads the name and number correctly. English or Japanese identities
  were not substituted.
- Finish analysis returned `unsupported` / `languageUnsupported`, as designed;
  English foil/stamp rules were not used to label these Korean cards.

The ignored `Diagnostics/korean-20261101/` directory contains the request,
completed record, physical feed confirmation, catalogue responses and
`integrity.json`. These saved images can be reprocessed when Korean catalogue
coverage improves; another physical scan is not required to investigate matching.

### Reviewing identifications

For an older scan or an explicit retry, choose **Identify card** beneath the front
image in the web batch library, then **Identify this image**. In the native app, use **Identify / review card…**.
Identification runs in ScannerService; either interface may close while it works.

Review the proposed name, set and collector number. No candidate is selected or
confirmed automatically. Select the matching candidate and choose **Save reviewed
identity**. Finish and condition are independent, optional manual fields:

- Finish: Not verified, Non-holo, Holo, Reverse holo, Other finish.
- Condition: Not assessed, Near mint, Lightly played, Moderately played,
  Heavily played, Damaged.

Catalogue finishes describe available printings, not the finish detected on the
physical card. Supported English [finish](combined-finish.md) and
[pumpkin-stamp](stamped-editions.md) checks are estimates, not physical confirmation.
Korean finish/stamp inference is explicitly unsupported until validated. The
application does not establish authenticity or a professional grade. Inspect the actual card under
appropriate lighting; a flat scan can miss dents, fine scratches and surface wear.
These condition labels are user-entered descriptions, not a marketplace grading
certification. Reject suggestions if none agrees with the card. A rejected or
failed identification can be explicitly rerun.

## How matching works

Apple Vision reads text locally in four orientations, from a bounded image preview
up to 2400 pixels on its longest edge. It uses accurate recognition in the selected language with
language correction disabled to preserve collector numbers. Analysis creates no
processed image files. Sources are checksum-verified, limited to 32 MiB / 100
megapixels, and must be a single image in configured scan storage.

TCGdex supplies the selected language's catalogue and card details over HTTPS. Only catalogue
GETs and candidate IDs leave the Mac; images and OCR text are not uploaded. The
catalogues have separate in-memory caches for 24 hours. There is no cross-language fallback. First lookup and detail lookups need
network access. Requests have deadlines, response-size limits and a fixed host;
redirects are refused. No API key is required.

The matcher looks for a card name in the upper image area and a collector
number in the lower area, cross-checking the printed set total when readable.
When a name/number match fails catalogue validation, a bounded second Vision pass
reads the footer number region. It must independently produce a fraction matching
a previously ranked printing and its official total. The service never removes a
digit to force a match. Successful retries retain a warning identifying the extra
OCR pass. B-010 Larvesta's rarity dot was initially read as a trailing zero; the
focused pixel reread resolves it to `028/159•`.

Zero padding is normalized while promo/subset prefixes are preserved. Conflicting
legible numbers rule out a printing. Title-only matches remain ambiguous. Up to
five candidates are shown after at most eight detail lookups, with the evidence used; this is not a calibrated
confidence percentage. Digital TCG Pocket entries with Pocket image paths are
excluded.

The initial version does not perform fuzzy spelling correction, visual artwork
matching or set-symbol recognition. Small cards inside a large A4 frame, unreadable
numbers, stylized titles and catalogue omissions may produce no match. Card backs
do not identify the card. Set variants/reprints can remain ambiguous even when OCR
is correct. Saved fi-8170 scans have now been checked as described below; this small batch
is not a general accuracy benchmark.

## Service contracts

Identification is asynchronous and limited to one worker per service. Jobs must
have finished before explicit identification or review requests. The automatic
processing pipeline publishes pending and completed identification records through
the same job API and manifest. It shares one recognizer with manual requests.
Cancellation stops processing while retaining images and partial results; OCR or
catalogue failures are recorded per image without failing a successful capture.
Interrupted work is marked by restart recovery and can be explicitly retried. A stable UUID makes a repeated start
request return the same result without another OCR operation. One latest result
is retained per image. Use a new UUID only for an explicit retry of a failed,
unmatched or rejected result. A review supplies the identification UUID so stale
clients cannot review a replaced result.

| HTTP | JSON-RPC | Purpose |
| --- | --- | --- |
| `POST /api/v1/images/{jobID}:{sequence}/identification` | `v1.images.identify` | Start; body `id`, optional `language: "en"` or `"ko"`; omission uses the batch language; returns 202 |
| `GET /api/v1/images/{jobID}:{sequence}/identification` | `v1.images.identification` | Poll; returns `{schemaVersion, identification}`; null before first attempt |
| `PATCH /api/v1/images/{jobID}:{sequence}/identification` | `v1.images.review` | Review using `identificationID`, optional `cardID`, `finish`, `condition`; omit `cardID` to reject |

RPC methods also take `jobID` and `sequence`. HTTP retains the same Bearer
authentication and request limits as scanner control. No method accepts paths,
arbitrary URLs or shell commands.

```sh
scanner images identify JOB_UUID 1 --request-id IDENTIFICATION_UUID --json
scanner images identification JOB_UUID 1 --json
scanner images review JOB_UUID 1 --identification-id IDENTIFICATION_UUID \
  --card-id swsh3-136 --finish reverseHolo --condition lightlyPlayed --json
```

API finish values: `unverified`, `normal`, `holo`, `reverseHolo`, `other`.
API condition values: `unassessed`, `nearMint`, `lightlyPlayed`,
`moderatelyPlayed`, `heavilyPlayed`, `damaged`.

MCP tools: `scanner_identify_card`, `scanner_get_card_identification`,
`scanner_review_card`. Read-only MCP mode exposes only retrieval. Agents must not
invent a finish or grade from OCR evidence; record the user's assessment.

## Persistence and failures

`ScanRecord.cardIdentifications` is an optional additive array in API responses,
SQLite and exported batch manifests. Each result records its request, source
SHA-256, timestamps, OCR method, state, evidence, candidates, selected TCGdex ID,
manual finish and condition. Older batches without this field remain readable.
Scan IDs, original checksums and physical side labels remain unchanged.

Identification states are `processing`, `reviewRequired`, `noMatch`, `failed`,
`confirmed`, `rejected`. OCR/network failures do not change the completed scan
state. Interrupted work becomes failed on service recovery and never restarts
automatically. A database migration indexes pending identification work. Updates
merge with concurrently saved image edits before publishing an atomic manifest.
If network storage disappears, local results remain in SQLite and the batch warns
that its disk manifest needs resynchronizing. Retrying the same start UUID after
restoring storage republishes the manifest without repeating OCR.

## Sources and validation

- [Apple Vision text recognition](https://developer.apple.com/documentation/vision/vnrecognizetextrequest)
- [TCGdex cards endpoint](https://tcgdex.dev/rest/cards)
- [TCGdex card/set/variant fields](https://tcgdex.dev/reference/card)

The live catalogue/reference-image check uses Furret `swsh3-136` (Darkness Ablaze,
136/189). It verified name, collector number and set total with local Vision and
the real TCGdex API. This is a reference-image test, not a scanner-card accuracy
benchmark. The external reference image is not committed.

```sh
swift test --filter ScannerRecognitionTests --disable-index-store
# Optional network/reference test; provide the downloaded TCGdex swsh3-136 image:
SCANNER_TEST_TCGDEX_IMAGE=/absolute/path/furret.png \
  swift test --filter recognizeKnownCatalogueImage --disable-index-store
```

Automated tests cover number normalization, ambiguous printings, conflicting
numbers, input/path boundaries, asynchronous idempotency, manual assessment,
concurrent image edits, restart/shutdown, network failure, original integrity,
authenticated HTTP access, CLI parsing and MCP policy. Browser fixtures return
explicit mock recognition evidence and never access scanner hardware or TCGdex.

## Installed validation — 2026-10-31

- Full Swift suite: 69 tests reported passing; 67 executed and two opt-in fixtures
  skipped. The reference-image network test passed separately (about eight seconds
  for that single sample; not a throughput benchmark).
- Four frontend contract tests and the TypeScript/Vite build passed.
- Six browser tests passed across desktop Chromium, phone-sized Chromium and
  tablet-sized WebKit. Identification, explicit review, finish/condition, reload
  and exported manifest metadata were checked against an isolated Swift mock.
  A premature first browser run began before its fixture listener was ready;
  the complete run passed after checking readiness.
- Release Swift builds and installed code signatures passed. All native services
  retain their certificate-backed identities. The web installer now retries the
  brief launchd unload/bootstrap race, including during rollback.
- Live authenticated browser check opened the identification panel, verified the
  existing original PNG download by SHA-256, and found no JavaScript/CSP errors
  or mobile horizontal overflow. Only login/logout mutated live service state.
- The updated native app connected to the service and opened the identification
  sheet for B-006/C-001. The OCR start button was not pressed.
- The updated service generated an existing SMB image thumbnail without another
  permission prompt. All six live scan jobs remained unchanged; service idle,
  queue empty. No physical feed or new recognition metadata was created in live
  batches during deployment checks.

Backups are under Application Support/Scanner/backups:
`recognition-20261031T192816Z` (previous service/CLI/MCP, native app and SQLite) and
`web-20261031T192933Z-F3A47280-2FDF-4533-91FC-0E92CAFE1D75` (gateway/configuration).
Ignored evidence is in `Diagnostics/recognition/`, `Web/test-results/` and `.build/`.
Physical phone/tablet testing and actual scanner-card recognition accuracy remain
unverified. See [card-test readiness](card-test-readiness.md).

## B-009 recognition corrections — 2026-10-31

The initial run suggested one printing for nine of eleven fronts. Fire Energy
remained ambiguous and Salamence ex had no match. Vision read the latter's
stylized suffix as `@X` (and `CX` in another orientation). The matcher now accepts
these narrow suffix variations only with collector-number agreement. Tall rotated
text boxes no longer qualify as title/footer evidence.

Basic Energy cards can match using an Energy title, the exact SVE set code and
one unambiguous three-digit footer number, checked against SVE catalogue entries.
An Energy title alone is insufficient. This resolved Fire Energy `sve-010`;
collector 177/159 resolved Salamence ex `sv09-177`. The two saved fronts were
reprocessed without feeding cards, changing original files or confirming identities.
All eleven B-009 fronts now have a unique **suggestion**, awaiting manual review.

Twelve recognition tests passed with the saved-batch fixture enabled (one unrelated
reference fixture skipped). Tests include rejection of suffix-only matches,
missing set codes and competing standalone numbers. Repeat the saved-image check:

```sh
SCANNER_RETRY_FIXTURE=/Volumes/scans/B-009 \
  swift test --filter ScannerRecognitionTests --disable-index-store
```
