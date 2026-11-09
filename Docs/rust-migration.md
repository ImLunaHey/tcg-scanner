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
  Disk-manifest reconciliation and Windows ACL policy remain with their owning
  platform/storage work.

## Validation

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
SCANNER_COMPAT_MANIFEST=/Volumes/scans/B-001/manifest.json cargo test -p scanner-types
```

The local internal disk is space-constrained. This migration uses
`CARGO_TARGET_DIR=/Volumes/XEX-VM/ScannerDevelopment/target/scanner` for local build
artifacts; this machine-specific path is not a build requirement or committed
Cargo configuration.
