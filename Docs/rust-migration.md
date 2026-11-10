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
  boundary, including Windows owner/DACL validation. Windows execution and
  disk-manifest reconciliation remain outstanding.

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
  Automatic identity/finish/condition processing still needs integration.
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
database. Startup currently refuses unfinished jobs pending disk reconciliation.
Service installation, sleep/wake handling and full friendly CLI parity remain open.

`scanner-mcp` implements MCP 2025-06-18 over bounded stdio JSON frames, with
initialization, tool discovery, structured tool results, read-only filtering, eight
concurrent calls and request cancellation. It advertises 23 completed Rust-backed
tools; recognition and inventory tools remain pending their domain slices. Tests
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
