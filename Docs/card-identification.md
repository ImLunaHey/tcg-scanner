# Card identification and manual assessment

Automatic finish estimates now combine printed-marker/catalogue evidence with
native border-texture analysis for eligible prepared cards. See
[combined finish estimates](combined-finish.md) for decision rules, limitations,
validation and the B-026 service recheck. Estimated finish remains separate from
the physically reviewed finish field.

Card scans with `prepareCards: true` automatically identify saved images during
post-capture processing. Identification is also available as an explicit operation
on an already saved original. It never
starts the scanner, changes the original bytes or renames B/C labels. English is
the first supported language. This feature was separately requested after the
initial scanner milestones. [Cardmarket GBP price guides](card-pricing.md) were
subsequently added; inventory creation and selling integration remain out of scope.

## Use it

Enable **Auto-crop, orient and identify cards** when starting a card scan.
After capture, each image is cropped/oriented and identified in sequence; results
appear incrementally while the job is Processing. Both sides are analyzed so the
service does not assume that capture parity identifies the front. Backs normally
produce no match. General document scans with this option off do not run OCR.

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
physical card. The application does not automatically determine foil, edition,
condition, authenticity or professional grade. Inspect the actual card under
appropriate lighting; a flat scan can miss dents, fine scratches and surface wear.
These condition labels are user-entered descriptions, not a marketplace grading
certification. Reject suggestions if none agrees with the card. A rejected or
failed identification can be explicitly rerun.

## How matching works

Apple Vision reads text locally in four orientations, from a bounded image preview
up to 2400 pixels on its longest edge. It uses accurate English recognition with
language correction disabled to preserve collector numbers. Analysis creates no
processed image files. Sources are checksum-verified, limited to 32 MiB / 100
megapixels, and must be a single image in configured scan storage.

TCGdex supplies an English catalogue plus card details over HTTPS. Only catalogue
GETs and candidate IDs leave the Mac; images and OCR text are not uploaded. The
catalogue is cached in memory for 24 hours. First lookup and detail lookups need
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
| `POST /api/v1/images/{jobID}:{sequence}/identification` | `v1.images.identify` | Start; body `id`, optional `language: "en"`; returns 202 |
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
