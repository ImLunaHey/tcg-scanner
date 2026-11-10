# Rust migration

Requested 2026-11-09: move the complete application to Rust with
[Anomaly Rune](https://github.com/anomalyco/rune) for the UI, preserving existing
scan data and workflows on macOS and adding Windows and Linux support.

This is a migration ledger, not a claim of completed feature parity. The installed
Swift service continues to own hardware until the Rust replacement passes its
acceptance checks. Development uses isolated databases and saved-image fixtures;
no automatic migration is run against the live ledger.

## Acceptance boundaries

- Wire/storage: existing v1 JSON, SQLite job ledger, stable batch/card identifiers,
  SHA-256 source integrity, additive metadata, reviewed identities and defaults.
- Service: idempotent submission, one hardware owner, ordered acquisition, bounded
  processing, cancellation/session closure, interrupted-work recovery and events.
- Images: original preservation, safe file access, edits, thumbnails, crop/deskew,
  orientation, game/side detection, camera perspective correction.
- Recognition: Pokémon English/Korean, Yu-Gi-Oh!, Star Wars Unlimited, evidence,
  ambiguous/unknown handling, catalogue retries, rarity/Energy/stamp/finish checks.
- Library: batches/continuations, inventory, reviewed finish/condition, pricing,
  missing-card/reference lookup, exports and Store transfers/resynchronization.
- Interfaces: authenticated HTTP, JSON-RPC, CLI, MCP, native Rune and remote Rune
  web UI, camera lookup, scanner settings and profiles.
- Platforms: Rust-owned macOS scanner integration, Windows TWAIN, Linux SANE,
  startup/service installers, platform transport, credentials and storage paths.
- Cutover: end-to-end parity tests, real saved-scan regressions, per-platform build
  checks and hardware verification before claiming those platforms are tested.
  No production invocation of Swift, Python, Node or scanner helper executables.

## Implementation record

- `scanner-types`: typed scan/image contracts and validation, preserving additive
  fields while their feature modules are ported. Unit tests check wire-key
  acronyms, validation and metadata preservation. `SCANNER_COMPAT_MANIFEST`
  enables a read-only round-trip check on an existing manifest.

Rust packages are currently internal workspace crates (`publish = false`); no
license or publication rights are inferred from the migration. Portable libraries
must not depend on the UI or service. Rune will be pinned to a Git revision.

- `scanner-store`: compatible SQLite tables, atomic job/image/event updates,
  stable reservations, profiles/settings, caches, continuation transactions and
  interrupted-work transitions. Existing live ledger read in read-only mode.
  Tests cover restart, rollback, event resets and Unix sidecar symlink rejection.
  Local database/sidecar creation now uses the shared native private-state
  boundary, including Windows owner/DACL validation. Native Windows execution remains outstanding.

- `scanner-core`: driver-independent queue ownership, idempotent submissions,
  profile/default resolution, ordered acquisition, cooperative cancellation,
  session-closure quarantine, continuations and shutdown. Accepted submissions
  and inspections outlive disconnected API callers. Integration tests use a
  controlled test driver with real isolated SQLite and batch files; they verify
  retry deduplication, preserved settings, closure ordering, queued cancellation,
  continuation checksums and inspection ownership.
  `NativeCardProcessor` now supplies the bounded in-process preparation path:
  verified source read, colour-managed crop/deskew, Tesseract orientation and an
  immutable PNG copy. A real B-001 original was copied into an isolated local
  batch, prepared successfully, and its source checksum reverified unchanged.
  Its public code-insert cue and quarter-turn agree with the saved Swift result.
  Native Pokémon identity now runs through the coordinator for explicit saved-image
  requests and post-scan processing. Accepted work persists beyond API disconnects;
  duplicate IDs reuse the work, language must match the batch, and shutdown drains
  publication while cancelling hardware promptly. Game/side routing preserves the
  complete duplex-pair rules and existing B-001 decisions. Unknown pairs remain
  unassessed. Other-game recognition, finish and condition integration remain open.
  Saved identification reads and manual candidate/finish/condition reviews now
  use typed lossless records through core, RPC, HTTP and MCP. Reviews require the
  current identification ID, reject unsupported candidates and in-flight work,
  preserve visual evidence, and serialize publication with other library writes.
  Existing B-001 recognition metadata passes read-only compatibility checks.
  Profile create/save/update/remove/name lookup, job summary pagination and bounded
  event long-polling are now exposed through the Rust coordinator. Accepted profile
  writes outlive caller disconnects and hold the submission reservation through
  their database commit so shutdown waits for them. Profile name uniqueness and
  partial updates are enforced transactionally, preserving additive metadata;
  tests cover concurrent writers, idempotent creates, Unicode case folding,
  invalid-update rollback and event subscriber limits/disconnection.
  Original/edit metadata, bounded chunk reads, verified PNG/JPEG thumbnails and
  manual crop/rotate/deskew edits now run through core and RPC. Immutable edit
  sidecars recover encoded output after an interrupted ledger publication; retry
  inputs must match, originals are reverified, and concurrent edits serialize
  with continuations/shutdown. Tests cover recovery, duplicate/conflicting edits,
  tampering, active-job refusal and the eight-edits-per-original limit. Thumbnails
  currently reverify sources on each request rather than using a fingerprint cache;
  decoding retains the Rust 48-megapixel bound.
  Device status and validated defaults now use backend-specific inspection
  evidence for ImageCaptureCore, SANE and TWAIN. Defaults require confirmed
  inspection closure, supported feeder/DPI/duplex observations and an idle
  coordinator; accepted writes survive caller disconnects. Unknown SANE paper
  readiness remains unknown. TWAIN inspection now copies supported-value
  containers as well as current values, with native allocation-lifetime tests.

- `scanner-device`: portable driver contracts extracted from the coordinator and
  a Rust ImageCaptureCore adapter. It includes main-thread discovery/inspection,
  explicit ADF settings with readback, ordered file callbacks, persisted-settings
  acknowledgement before feeding, cancellation deadlines, settings restoration,
  exact-device disconnect recovery and pre-enabled button monitoring. Callback
  and coordinator tests cover acknowledgement, ordering, reused URLs, limits and
  cleanup quarantine. Read-only native discovery completed with zero devices;
  inspection/capture/button/restoration are hardware-unverified. The Linux SANE
  adapter now implements serialized native discovery/inspection and ADF captures,
  copied option descriptors, settings readback/restoration, durable settings
  acknowledgement, bounded RGB frame assembly, and asynchronous cancellation
  followed by read completion and closure. A test-only native SANE library verifies
  these boundaries, including inexact writes, rejected persistence and disconnected
  callers. Linux x86_64 cross-compilation passes; real Linux hardware remains
  untested and SANE button monitoring is unsupported. Windows TWAIN 2 now has
  one-thread DSM/window ownership, stable source IDs, native discovery and
  read-only capability inspection with explicit source closure. Portable ABI tests
  check packing, scalar/enumeration decoding, fixed-point byte order, allocation
  lifetime and failed-close quarantine through a native simulated DSM. Windows
  x64/ARM64 type-checks pass. Windows acquisition now verifies/persists settings
  before enabling the ADF, pumps events, preserves bounded RGB memory strips and
  follows state-specific end/reset/disable/restore/close transitions. Ten native
  simulation scenarios cover successful duplex capture, adjusted settings,
  first/later read failures, cancellation, failed end-transfer, incomplete duplex,
  rejected persistence, cancellation before feeding and unknown-status quarantine. Actual Windows execution
  remains outstanding. Backing-colour vendor mapping and SANE/TWAIN ICC-profile
  retrieval/colour validation are also outstanding; these are not cutover-ready
  hardware claims.
  No Swift or scanner helper executable is invoked.

- `scanner-files`: pinned directory access, symlink/hard-link rejection, original
  checksum verification, bounded chunk reads, exclusive batch/image creation,
  atomic manifest publication, immutable edits and continuation numbering. Tests
  include same-size tampering, traversal, retained staging, duplicate captures,
  multipage TIFF metadata and read-only access to existing originals/edits.
  Mount policy is checked before writes: macOS `statfs` is verified against the
  live share, Linux mountinfo parsing is fixture-tested, and Windows accepts
  explicit UNC shares (mapped drives still need resolution). Windows ACL policy
  and native Windows/Linux execution remain to be validated.

- `scanner-images`: portable bounded PNG/JPEG/TIFF/WebP decoding, EXIF orientation,
  crop/rotate/deskew, thumbnails and conservative foreground geometry. Seven tests
  cover corner mapping, both tilt directions, backing transitions and multiple
  objects. Orientation recognition and camera extraction remain in progress.
  Embedded scanner ICC profiles are converted to sRGB with pure Rust colour
  management. A read-only comparison of all 128 prepared B-001 captures succeeded:
  minimum source-crop intersection-over-union 0.98898; maximum deskew difference
  0.30398°. This checks geometry, not complete orientation/recognition parity.
  Run `cargo run --release -p scanner-images --example compare_geometry -- MANIFEST`
  to repeat against a saved batch.

- `scanner-recognition`: in-process Tesseract C API and ONNX Runtime, pinned
  DINOv3 graph/weight checksums, RGB normalization and normalized CLS retrieval.
  The Rust benchmark ran all 69 existing fixtures against 121 references: all 48
  fronts with an available reference ranked correctly first, matching the earlier
  experiment. Fifteen fronts lack references and six fixtures are backs; similarity
  is not an acceptance rule. Whole-image native Tesseract ran on all 69 inputs;
  this run does not establish text-matching accuracy. Catalogue evidence, regional
  OCR and packaging the native libraries remain in progress.
  The Pokémon evidence matcher now preserves title/number/set-total checks, promo
  prefixes, Energy set codes, Korean class/set constraints and independent footer
  agreement. All candidates from the saved native baseline survive the ported
  rules. With cached metadata, native Tesseract's first whole-image pass yields
  17 unique correct front matches, eight ambiguous and 38 unresolved; all six
  backs remain unmatched. Five inputs need additional candidate metadata, so this
  is an incomplete catalogue evaluation, not a final coverage figure.
  A subsequent regional pass with the missing candidate metadata fetched yields
  38 unique correct fronts, four ambiguous and 21 unresolved, with zero incorrect
  unique matches and all six backs unmatched. Whole-only on the same complete
  metadata yields 17 correct, 12 ambiguous and 34 unresolved; regional-only yields
  29 correct, three ambiguous and 31 unresolved. These are saved upright fixtures,
  not a claim of parity with the 62 correct native baseline. Readings retain their
  separate pass provenance and full-image box coordinates. Native orientation now
  combines all four OCR directions with conservative back-layout, logo, Energy
  and public code-insert cues; ambiguous direction remains unverified. All six
  saved Pokémon back layouts pass under all four input rotations.
  End-to-end native Tesseract orientation was then checked on all 69 fixtures at
  all four rotations: 236 correct verified directions, 40 uncertain and zero
  incorrect verified directions. The 40 uncertain cases include all 24 back
  rotations because stylized logo OCR did not corroborate the colour layout;
  colour alone is deliberately insufficient. Four front fixtures account for
  the other 16 uncertain cases.

- `scanner-catalogue`: fixed-origin HTTPS TCGdex catalogue, card and set reads,
  bounded responses, validated identities, English/Korean paths, expiring caches
  and coalesced requests. Seven tests cover cache retry/coalescing, response limits,
  invalid IDs, digital-card filtering and set/detail validation. A live native
  read of `sv09-177` returned validated Salamence ex metadata. Other-game catalogue
  adapters and typed price conversion remain in progress.

- `scanner-rpc`: transport-independent v1 JSON-RPC dispatch for device discovery,
  inspection/settings reports, diagnostics, scan submission/status/cancellation,
  batch continuation, job pagination, service status, events, profile CRUD and
  image metadata/chunks/thumbnails/edits, device readiness and validated defaults.
  Existing method aliases share the same coordinator and durable operations.
  Requests are limited to 64 KiB, require a string/numeric ID and reject unknown
  parameters; errors preserve structured scanner error data. Protocol tests use
  an isolated database and controlled driver to verify malformed-request rejection,
  profile conflicts/partial updates, submission idempotency and cancellation cleanup.
  This crate opens no listener. Authentication, transport concurrency/body limits,
  The remaining library/control methods are still pending.

## Validation

`scanner-platform` provides capability-pinned private local directories,
exclusive credential creation/loading and a process-lifetime hardware lock.
Unix permissions/link checks and Windows handle owner/DACL checks apply before
using local state. Tests verify private credential round trips, replacement/link
rejection, and lock exclusion against the same native `flock` used by the Swift
service. Windows x64 type/lint checks pass; native Windows ACL behavior still
needs execution on that platform.

`scanner-http` now adapts the Rust coordinator directly to authenticated REST and
JSON-RPC, expiring/revocable bearer sessions, bounded original/thumbnail downloads
and resumable SSE events. It preserves the existing public binary-image policy
while authenticating metadata and controls. Integration tests exercise Host/Origin
and header rejection before hardware access, body limits, session authority,
REST/RPC profile sharing, image integrity, and concurrency leases through response
completion/disconnection. A native TCP listener now bounds connections, header
size/count/time, connection lifetime and graceful shutdown. Real TCP tests use
`scanner-client` to exercise authenticated RPC, structured errors and shutdown;
oversized headers are rejected before device access. Remaining domain routes
and Rune web assets are still pending.

`scanner-app` composes the native service and CLI, private credential loading,
global hardware ownership, platform scanner hosts and signal-driven shutdown.
The CLI provides scan/status/job/profile commands and generic v1 calls using the
bounded native `scanner-client`. Both binaries build; an isolated startup attempt
correctly refused the installed Swift service's hardware lock before creating a
database. Startup now reconciles interrupted jobs under the global owner lock before
constructing the coordinator. Newer manifests must match job/request/continuation
identity, retain immutable history and reviews, and pass streaming image checksum
checks. Mismatched, linked or corrupt manifests remain untouched for inspection.
Unfinished scans and OCR fail explicitly; feeding never resumes. Tests cover
newer/completed manifests, continuations, review preservation, corrupt files,
links and offline storage.
Service installation, sleep/wake handling and full friendly CLI parity remain open.

`scanner-mcp` implements MCP 2025-06-18 over bounded stdio JSON frames, with
initialization, tool discovery, structured tool results, read-only filtering, eight
concurrent calls and request cancellation. It advertises 27 completed Rust-backed
tools; finish and condition assessment tools remain pending their domain slices. Tests
cover initialization gates, argument validation, defaults, domain errors,
concurrency, duplicate IDs, cancellation and oversized/truncated frames. MCP
cancellation never sends a physical scan cancellation. The `scanner-mcp` binary
uses the same private credential loader and native HTTP client as the CLI.

The Rune library slice opens saved batches, searches cards and shows their latest
edits and identification evidence. Reads and verified thumbnails run off the UI
thread. It currently reads the ledger without writing; service-backed scan and
review controls are still being ported. Rune is pinned to `e970367bd7d2a51e2e7d58e2978ea32eac7780cb`.

```sh
cargo run -p scanner-ui
SCANNER_UI_MANIFEST=/path/to/manifest.json cargo run -p scanner-ui -- --snap /tmp/library.png --scene saved
```

`SCANNER_DATABASE` overrides the platform application-data ledger path.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
SCANNER_COMPAT_MANIFEST=/Volumes/scans/B-001/manifest.json cargo test -p scanner-types
```

Build artifacts use the standard workspace `target/` directory on the main drive.
The read-only Rune reference checkout is in the local development cache; the
application dependency is pinned in Cargo and does not require that checkout.

The native service OCR selector was replayed against 69 saved upright readings:
35 correct unique suggestions, five ambiguous, 23 unresolved fronts and six clear
backs, with no wrong unique suggestion. This selector keeps independent OCR passes
separate and retains equally supported conflicting suggestions; it is more
conservative than the earlier combined-text experiment (38 correct unique). A
real saved B-001 front produced the expected `30th-063` suggestion via native
Tesseract and live TCGdex metadata; its recognized paired back bypassed identity
OCR. The Rune acceptance UI, visual assessment and additional game adapters remain
necessary before cutover.

Explicit saved-card preparation now shares the native processor with capture
processing and accepts a stable operation UUID through core/RPC/HTTP/MCP. A
retained sidecar can recover the original prepared PNG without rerunning OCR.
Tests cover duplicate/concurrent requests, publication recovery, conflicting
sequences, original preservation and shutdown refusal.

`scanner-library` provides pure inventory aggregation over saved records: physical
copy counts, confirmed quantities, language separation, code-insert exclusion,
search/set filters, deterministic set slugs, previews, catalogue sources and
missing-card subtraction. B-001 projects to 47 playable copies in 35 entries and
17 excluded code inserts without modifying files. Synthetic tests cover rescans,
reviews, incomplete pairs and filtered totals. Non-Latin set slugs use portable
transliteration; native locale-specific ordering is not assumed across platforms.
The service projects a consistent terminal-job cursor on up to four bounded
blocking workers, then exposes inventory, missing-card checklists and registered
references through v1 RPC/HTTP; MCP includes inventory listing. HTTP tests verify
active-job exclusion, filtered totals, owned-card subtraction, bearer controls and
public-reference Host/Origin/ETag behavior without touching hardware.

The native catalogue library now supplies Pokémon set checklists, asynchronous
release dates and registered, bounded reference thumbnails. Injected-transport
tests verify request coalescing, language separation and rejection of unregistered
or off-origin images. The service now persists validated public metadata and previews in an optional
private, bounded SQLite cache; it does not store user scan originals.

Native YGOPRODeck and SWU-DB catalogue adapters now support typed printing lists,
release dates and inventory reference registration. Tests preserve IDs, deduplicate
rarity rows, reject conflicting sets/duplicate cards, and prevent Korean-to-English
fallback. Service recognition integration follows this slice.

A host-configured catalogue cache now persists validated metadata and reference
previews across restarts. Private-file checks, SHA-256, TTL/schema validation,
entry/byte caps and bounded blocking workers apply. Tests verify restart reuse,
corruption/identity rejection, expiry, eviction, linked-sidecar refusal and cache
capacity recovery after cancelled loads. Service startup configures one shared
cache for inventory and native Pokémon recognition; cache failure is optional.
The native checklist example also completed live reads of SWU `SOR` (950 catalogue
printings) and YGOPRODeck `LOB` (252 English/legacy printings), using the isolated
main-drive public-data cache. These counts validate provider integration, not card
recognition accuracy or foil/condition assessment.

Native YGO/SWU recognition now runs through the same durable core workflow as
Pokémon. It preserves exact identifiers and conservative conflict rules, records
the correct provider-specific method, and keeps finish/condition unassessed.
Printed SWU footer evidence can classify two-sided cards after verified text
orientation. Tests cover contradictory identity fields, multiple printings,
unsupported language, name/subtitle/total checks and independent-pass ambiguity.

Read-only archived fixtures expose a remaining OCR quality gap: targeted YGO field
reads identify the correct base card on B-044's four fronts, but none resolves its
set code. General regions previously identified none. B-050 yields four exact SWU
printing matches and twelve unresolved out of sixteen, with no wrong printing
selected. Threshold/segmentation probes did not establish a safe printing-code
improvement. This is not recognition parity with Apple Vision; manual review and
further portable recognition improvements remain necessary before cutover.

The shared authenticated client now compiles for wasm32 as well as native targets.
Browser Fetch explicitly refuses redirects, omits cookies/referrers, disables
caching, aborts cancelled/timed-out work and reads response streams with the same
16 MiB bound. Native TCP gateway tests continue to pass. Browser execution and the
Rune web application remain to be validated; a successful wasm check alone does
not establish browser runtime parity.

Rune now uses the Rust service for paginated batch lists, selected records and
verified PNG previews. Its secure connection screen retains credentials only in
memory; native launches can use `SCANNER_TOKEN_FILE` and `SCANNER_UI_ENDPOINT`.
The explicit `SCANNER_UI_MANIFEST` development scene remains read-only. Normal UI
startup no longer opens the live SQLite ledger directly. Cancellable async slots
work with native Tokio and browser tasks, and tests reject stale selection results,
corrupt previews and misleading/oversized PNG dimensions.

Native tests, strict Clippy, a saved B-001 headless snapshot and wasm32 checks pass.
Rune's pinned GPU cache requires wgpu's documented
[`fragile-send-sync-non-atomic-wasm`](https://docs.rs/wgpu/30.0.1/wgpu/#other)
feature in its single-threaded browser build; the UI explicitly rejects wasm atomics.
Desktop-only Rune features and file/image dependencies are target-gated. Browser
runtime rendering, hosted assets and service mutations are subsequent work.

Rune now exposes device discovery, profile selection, 600-DPI PNG defaults,
duplex/language choices, scan submission, cancellation and continuation. Card
preparation, identification and manual candidate/finish/condition reviews use the
same authenticated service operations. Status/progress refresh asynchronously;
unsaved review choices survive refreshes, and a completed write supersedes any
older pending record read. Mutations are serialized in the UI. An uncertain
response retains its original parameters/UUID for an explicit retry, and a real
TCP lost-response test verifies identical scan submissions and duplicate-click
suppression. These tests use a simulated service and never feed the live scanner.

The Rune web bundle now runs in Chromium through an isolated Tailscale fixture
service. The verified bundle includes wasm-bindgen/Rune helpers, full Inter fonts
and their licence. The HTTP gateway serves only hash-verified, bounded assets,
with a separate eight-request asset budget, ETags and a CSP that permits the Rust
WebAssembly application. API authentication and Host/Origin checks remain intact.
A browser flow loaded the synthetic card/verified thumbnail and saved a confirmed
review through native RPC with no JavaScript errors or browser token storage.
Packaging, fixture and browser-test instructions are in `Docs/rune-web.md`.

Rune inventory now supports paginated physical-copy entries, debounced searches,
set filters, missing-card checklists, bounded verified catalogue previews and
opening a copy's batch. Set/missing/copy lists are paginated rather than rendering
unbounded collections. A real Chromium fixture flow confirms one physical copy,
loads its preview and returns to its batch after saving explicit finish/condition
choices. JPEG header/digest tests reject misleading dimensions and corrupt content;
native tests, strict Clippy and wasm checks pass for this UI slice.

Rune settings now create, edit and delete stable-ID profiles, inspect the selected
scanner, show reported readiness/defaults and save validated device defaults.
Scan controls explicitly select either 600-DPI PNG settings, device defaults or a
profile; selecting a profile clears the defaults mode. Device/profile discovery
preserves selections by ID. A real browser fixture created a 300-DPI profile,
updated it to 600 DPI without changing its ID and deleted it through the service.
The ten native UI tests, native/wasm strict Clippy and structure audit pass.

Rune image editing now exposes crop percentages, deskew and clockwise rotation.
The editor shows the original and initializes from the latest saved edit's original
coordinates; every save uses the existing immutable, idempotent edit operation.
The browser fixture verified a 200×300 original becomes a separate 180×160 cropped,
rotated copy and reopening the editor still loads the original dimensions.
Eleven UI tests, native/wasm strict Clippy and the structure audit pass.

The PR now runs pinned Rust 1.98.0 CI on macOS, Linux and Windows and builds the
Rune browser WebAssembly target. Native jobs load simulated SANE/TWAIN libraries;
these exercise driver contracts without implying physical scanner validation.

The first CI run compiled every target. macOS and WebAssembly passed; Linux
exposed directory fsync on capability `O_PATH` handles, and Windows exposed private
state creation/ownership assumptions. Directory sync now opens the pinned directory
for reading. Windows creates state with a protected, inheritable owner/SYSTEM/admin
ACL in the native create operation and accepts privileged default owners only when
the DACL remains private. The finish-service checkpoint (`d3cb3fb`) now passes full native test/Clippy/build jobs on Windows, Linux and macOS, plus the WebAssembly job. Physical hardware testing is still outstanding.

Native condition analysis now follows the existing blue-back visible-mark scope:
bounded sRGB pixels, exclusion of boundary-connected backing, border components,
rotation-consistent normalized regions and at most 24 findings. It never sets a
selling grade. The service records observations automatically even when catalogue
recognition fails, and offers `images.checkCondition` through RPC, authenticated
REST (`condition-check`), MCP and Rune. Source SHA/identification IDs are checked;
manual review fields and unrelated evidence survive repeated checks.

The read-only B-001 run checked all 47 prepared Pokémon backs and excluded all 81
front/code sides, finding one candidate border mark. Removing preparation hints
produced the same 47/81 split through colour/layout evidence. The aggregate is in
`Tools/RecognitionBench/results-condition-2026-11-10.json`; these observations are
not independent confirmation of physical wear. Reproduce with
`cargo run -p scanner-core --example condition_saved -- MANIFEST`.
Saved Swift observations also flagged a tiny region on sequence 3; native analysis
did not reproduce it with area, nearest, triangle, Catmull-Rom, Gaussian or Lanczos3
resampling. The finding on sequence 51 agrees in location. Classification parity
is established on this fixture; identical small-mark sensitivity is not.

Rust now implements the ordinary printed set-code/border observations and the
pumpkin/anniversary stamp detectors. Stamp and edition contracts preserve visual
measurements, catalogue provenance and a deterministic pricing key. Standard-size
TCGdex printing variants and the existing 90-entry Trick or Trade registry feed
conservative stamped-printing decisions: conflicting variants remain uncertain
unless eligible positive border evidence resolves them, and anniversary metadata
labelled normal is not treated as reliable non-holo evidence.

Read-only stamp comparison detected all six B-027 pumpkin marks, flagged none of
71 prepared ordinary control fronts, and agreed with the saved anniversary state
on 43/47 B-001 fronts. Four anniversary results differ (including two misses);
thresholds were not retuned. Details and manifest digests are in
`Tools/RecognitionBench/results-stamps-2026-11-10.json`. Reproduce with
`cargo run -p scanner-core --example stamp_saved -- MANIFEST`.

A separate 82-identity ordinary-finish comparison held catalogue identity fixed.
Fresh Tesseract initially found no usable set-code markers. Word-level boxes and
adjacent code/language joining recovered one normal suggestion; the independent
modern-ex catalogue rule produced one holo suggestion and one Energy card was
unsupported. The remaining 79 were unresolved. Stored Vision text produced far
more usable observations. Portable finish OCR therefore remains a parity blocker;
these component results are not a claim of end-to-end recognition accuracy.

Finish assessment is now recorded during native recognition and exposed as
`images.estimateFinish`, authenticated REST `finish-estimate`, MCP and Rune's
"Assess finish" action. It checks the current identification and exact source
SHA, survives caller disconnection, and publishes before shutdown completes.
Tests verify that manual finish/condition and unrelated evidence survive,
repeated checks persist, changed sources and stale IDs fail, and REST rejects
unauthenticated or extra-field requests. The Rune control compiles natively and
for WebAssembly; suggestions require an explicit user choice.

Stamped pumpkin assessments now compare the same mark against fixed-host public
TCGdex artwork. Downloads are capped at 8 MiB, single-frame PNGs at 16 megapixels,
and decoding/caching are bounded. Reference checks retain the image SHA and never
supply physical-finish training labels. Tests cover shared fetches, persistent
reuse, corrupt-cache recovery, hostile URLs and corrupt images. The complete
workspace passed 195 tests at the finish-service checkpoint with both native
SANE/TWAIN fixture libraries enabled.

The reusable `scanner-pricing` crate ports market quote extraction, ECB GBP rate
parsing, physical-card selection and conservative per-card valuation. Decimal
arithmetic preserves JSON precision and rounds each copy before summing. Tests
cover conflicting regular/stamped products, missing reverse prices, stale rates,
DTD/duplicate-rate rejection, edition-specific quotes and per-copy rounding.
Shared JSON decimal support passes 204 workspace tests and strict WebAssembly
Clippy. Service caching and valuation UI integration follow this domain slice.

Valuation now runs through `batches.valuation`, authenticated REST and MCP. Reads
return cached/pending snapshots without waiting for public requests; two workers,
a 200-item queue and retry cooldown bound refreshes. The existing SQLite price
keys remain compatible, with bounded records and corrupt-entry handling. Snapshot
reads serialize with completion bookkeeping to prevent duplicate refreshes during
publication. Shutdown drains active cache writes; unsupported languages never
request substituted English quotes. Integration tests cover the two-fetch bound,
non-blocking reads, persistence, unsupported pairs and REST authentication.

Rune displays the batch guide and selected-card reason in a scrollable review
panel. It refreshes pending prices, cancels stale requests when selection or
review changes, and renders missing prices as unavailable. Native and WebAssembly
Clippy pass; the browser flow verifies an unpriced synthetic card remains
unpriced after review, alongside editing, inventory and profile operations.

`scanner-staging` now provides the Store protocol client and resync ownership
rules in-process. Bounded HTTPS requests reject redirects; updates and paired
photo uploads carry revisions. Tests cover lost-response recovery, mixed
acknowledged fields, preservation of reviewer edits/photos, response limits and
redacted upstream failures. Transfer planning and service orchestration follow.

Store transfer planning, frozen receipts, restart recovery and resync now run in
the Rust service through `batches.storeStatus` / `batches.sendToStore`, REST
`GET` / `POST /api/v1/batches/ID/store`, and MCP. One transfer owns its batch until
completion; other library reads remain available. Shutdown drains receipt writes.
Tests cover lost upload responses without duplicate uploads, metadata resync,
preserved reviewer notes, changed bytes, incomplete/stale preparation, stamped
retry rejection, interrupted receipts and authenticated API access.

The service accepts an optional `storeConfiguration` absolute path to the existing
private Store JSON (`baseURL`, `scannerInstanceID`, `tokenFile`). The configuration
and token are read through the portable private-directory capability. The remote
contract still cannot represent stamped editions, so those cards remain blocked.
No live Store uploads were used to validate this migration.

Rune now shows Store progress, failures, preserved reviewer cards, retry/resync
and validated review links. The browser flow checks that an unconfigured service
cannot send. Review links use native platform handling on macOS/Windows and an
in-process OpenURI desktop-portal request on Linux; no xdg-open helper is used.
The portal call compiles independently against the pinned zbus API. Native and
WebAssembly UI checks pass; a real Linux desktop portal remains a platform test.

The Rust client now verifies each image chunk's identity, offsets and checksum,
then verifies the whole saved SHA before completing a download. `scanner-export`
streams native images/batches without replacing existing files, retains verified
files after failures, and adds the batch completion manifest last. Browser ZIP
exports retain the 64 MiB image-data limit. Real TCP tests cover multi-chunk
downloads, corrupt edited images, destination races and rewritten original/edit
paths. Rune now exposes manifest, selected/original image, batch and capability
exports. Desktop save/folder dialogs support cancellation; browsers generate
downloads in Rust/WebAssembly. The browser flow downloads both images, verifies
their saved hashes and dimensions, and opens the ZIP to verify its manifest and
files. The workspace passes 225 tests plus strict native and WebAssembly Clippy.

Camera geometry now runs in `scanner-images`: JPEG metadata is bounded before
pixel allocation, EXIF/colour handling is shared, and Canny contours produce
validated four-corner outlines with perspective correction. Partial, small and
unsuitable shapes return no card. Synthetic tests verify asymmetric marks survive
without mirroring. This replaces the geometry dependency on Vision/CoreImage;
real phone-frame quality and the service/UI camera path are still in progress.

Ephemeral camera lookup now uses the Rust recognizers through
`v1.camera.identify` and public `POST /api/v1/camera/identify`. The public route
retains authority/origin validation, a 60-request/minute budget, one in-flight
request and a 768 KiB envelope / 512 KiB decoded-image bound. Other RPC methods
retain their 64 KiB limit. Camera requests cannot create sessions or access the
library. A single owned camera task reserves recognition and delays queued feeds;
caller disconnects do not strand ownership and shutdown drains the worker.

Tesseract camera OCR uses the native C deadline monitor and a shared 60-second
lookup budget. Verified orientation narrows OCR passes; known backs and code
cards return guidance. Pokémon finish estimates use camera limitations without
scanner foil thresholds; Korean never falls back to an English printing. The
real installed Tesseract deadline test, isolated service/HTTP lifecycle tests,
recognition suites and strict Clippy pass. No phone-image accuracy or physical
camera validation is claimed yet; Rune camera capture remains in progress.

Rune now owns the public `/camera` workflow in Rust/WebAssembly. Its separate
anonymous client cannot carry library credentials. Capture bounds JPEGs to
512 KiB, waits for steady frames, suppresses repeated matches, pauses in details,
and supports English/Korean selection and verified torch changes. Leaving,
backgrounding or losing connectivity stops the original camera tracks; a late
permission response is also stopped. Frames/results remain in memory only.
The HTTPS browser fixture exercises actual Rust extraction with synthetic
catalogue results, not a claim of OCR accuracy. It verifies anonymous requests,
lookup suspension, language changes, late cancellation, track release and empty
browser storage. The existing library/export flow also passes over HTTPS. Full
workspace tests and strict native/WebAssembly Clippy pass. PWA installation and
physical phone validation remain outstanding.

The pinned Rune runtime is vendored with one documented WebGPU probe fix:
unsupported adapters are tested on a temporary canvas so WebGL fallback can use
the real display. This fixes HTTPS browsers exposing WebGPU without a usable
adapter. Both camera and library flows pass that fallback. Remaining Rune sibling
crates retain the same Git revision.

A Linux CI staging retry race is fixed: final receipt publication and releasing
the active transfer now share the submission reservation with retry admission.
An immediate retry cannot observe a terminal receipt alongside stale ownership.

The coordinator now exposes owned pause/resume transitions through RPC,
authenticated REST, MCP and Rune. Pause blocks admission, cancels queued and active
feeds, drains accepted writes, camera work, inspections and Store transfers, and
survives caller disconnection. Resume waits for cleanup; cancelled feeds stay
cancelled and hardware/storage quarantine remains. Integration tests cover queue
cancellation, an in-flight submission, unknown inspection closure, shutdown and
authentication. Focused suites and strict native/WebAssembly Clippy pass. Native
sleep/wake event integration follows this coordinator slice.

Native sleep/wake is now wired through `scanner-platform` IOKit, PowrProf and
logind adapters. Hardware-service startup requires notification registration;
monitor loss closes the service. The coordinator separates manual and system
pause, preserves quarantine on wake, and never restarts cancelled feeds. Native
macOS registration/cleanup succeeds without a power-state change; synthetic
callbacks, lifecycle tests and Windows/Linux cross-target strict Clippy pass.
See [native power lifecycle](native-power.md) for OS deadline limitations and the
remaining physical suspend/resume checks.

The camera now has an install manifest, generated PNG icons and a Rust/WebAssembly
service worker. A failed `/camera` navigation renders an offline explanation;
identification/API/image requests are not intercepted. The installed worker
initializes synchronously from embedded WASM without an application cache. HTTPS
Chromium checks verify activation, icons, offline HTML, reconnect, unauthenticated
API refusal and empty CacheStorage/IndexedDB/local/session storage. Camera capture
still passes. The library browser flow now also verifies pause/resume and all
previous review, profile, inventory and checksum-verified export operations.
Physical phone installation remains untested.

Configured scan storage can now start disconnected. The local ledger recovers
interrupted jobs without accessing unavailable originals; camera recognition and
metadata remain usable. Directory capabilities open only after mount-policy
validation, and missing destinations are never created as fallback storage.
Existing handles stay pinned until explicit idle-service reconnection. Authenticated
storage status/reconnect APIs, MCP and Rune expose that distinction without
mounting shares or clearing hardware/persistence faults. Tests cover offline camera
lookup, refused feeds, reconnect, active-session refusal, unchanged directories,
required-SMB rejection of local directories, and configuration loading.
The HTTPS Rune flow also verifies the storage status and explicit reopen action.
The complete workspace passes 254 tests with native SANE/TWAIN fixtures and real
Tesseract enabled, plus strict native and WebAssembly Clippy at this checkpoint.

Opt-in automatic physical-button scanning now runs in the coordinator. Set
`scanButtonDevice` to a native scanner ID in the Rust service JSON; the driver must
already have button monitoring enabled. The service never changes vendor safety
settings to enable it. One press creates a 600-DPI prepared duplex PNG job, with a
five-second rearm delay. Scan/inspection/camera handoffs cancel the listener and
wait for confirmed native cleanup. Closure failures quarantine hardware; sleep
and shutdown drain idle listeners. A transition generation prevents old presses
from creating delayed feeds after ownership changes. Tests cover press/debounce,
client handoff, inspection, failed closure, sleep and shutdown. Periodic discovery
and recovery now run independently of power notifications. Recovery waits until
faulted queued jobs have drained, so reconnect cannot restart an earlier feed.
SANE/TWAIN explicitly report unsupported button monitoring; real button hardware
validation remains outstanding.

Explicit 30–300-second button observation is restored through the original
`diagnostics.buttonMonitor` / `diagnostics.stopButtonMonitor` methods,
authenticated REST, MCP and Rune settings. Its reservation exists before device
selection; stop/sleep/shutdown cancel startup as well as an open listener and wait
for cleanup. Diagnostic presses never create jobs. Tests verify automatic-listener
handoff, queued/late cancellation, range/authentication validation and no feeds.
Service status now exposes `buttonObservation` separately from manual pause.
