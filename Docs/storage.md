# Scan storage

The configured destination for this installation is `smb://lake/scans`, mounted
at `/Volumes/scans`. All clients use ScannerService's configured storage root.
The SQLite database remains in local Application Support; it is not moved to SMB.

The user's workflow is **scan-only**: preserve the driver-delivered scan, with
PNG as the default for new jobs. Do not add automatic cropping, rotation,
background removal or other processing. Keep image files directly in a batch
folder, without `originals/` or `processed/` subdirectories.

```text
/Volumes/scans/
  B-001/
    C-001.png
    C-002.png
    manifest.json
```

Batch numbers increase across the installation's lifetime, not per day. Dates
stay in metadata and the app. Label a box `B-001` and a sleeve `B-001 / C-001`.
The ledger reserves numbers transactionally before job creation, never recycles
them, and preserves reservations across restarts and idempotent retries. Failed
job creation can leave a gap. Keep the local SQLite ledger in backups: the share
alone is not the numbering authority. Existing folders are never overwritten.
Padding is a minimum of three digits; `B-1000` follows `B-999`.

UUIDs remain the API and idempotency identifiers; `batchNumber` is an additive,
optional metadata field. The six existing batches were assigned numbers oldest
first. Existing TIFF files retain their encoding; new jobs default to PNG.

The intended card layout is `B-001/C-001-front.png` and
`B-001/C-001-back.png`, then the next card number. **Verified card pairing is still
pending**, so current paper captures use `C-001.png`, `C-002.png` in acquisition
order, with `side: unknown`. These capture indices are not yet shared card IDs
across duplex sides. Do not label card sleeves from them until pairing is verified.

`.incoming/` is hidden internal staging for driver delivery and recovery. It may
retain duplicate delivery files or partial captures; it is not a processed-image
folder. Older manually generated test edits are preserved as flat `edit-<UUID>.png`
files and hidden `.edit-<UUID>.json` recovery metadata. No new edits are generated
during scanning. The optional manual editor also uses these flat names.

## Persistent configuration

The service reads the private, non-secret file:

```text
~/Library/Application Support/Scanner/storage.json
```

For this installation:

```json
{"storageRoot":"/Volumes/scans","requiredSMBShare":"smb://lake/scans"}
```

Without a configuration file, the default remains `~/Scans`. Explicit
`--storage-root` replaces that selection; when explicitly choosing SMB, also
supply `--required-smb-share smb://lake/scans`. Credentials must never be embedded
in this configuration or the share URL.

The service verifies the filesystem type, exact mount point and server/share
identity at startup and before batch creation and storage writes. A plain local
folder at `/Volumes/scans` is rejected. The service does not recreate a missing
network mount or silently fall back to local image storage. If the share is
unavailable, mount it first; launchd retries service startup. An interruption
during an active transfer still produces a storage failure and may retain partial
files; it does not automatically rescan physical media.

The LaunchAgent installer preserves this location through updates by leaving
storage selection in the persistent configuration rather than writing a hardcoded
`~/Scans` argument into every generated plist. Older installed plists containing
an explicit `--storage-root` must be regenerated when adopting the configuration.

## macOS network-volume permission

SMB authentication and macOS application privacy permissions are separate.
`scanner-service`, running as a LaunchAgent, needs permission to access network
volumes even when Terminal or the coding application can read the mount.
Approve the macOS network-volume access request specifically for `scanner-service`.
Updates now use a [persistent signing identity](signing.md) rather than ad-hoc
signatures. Switching to that identity can require one final approval; subsequent
builds must keep the same identity and identifier to preserve permission matching.
If necessary, review its entry in System Settings → Privacy & Security → Files
and Folders → Network Volumes. Other applications' permission requests are
independent and are not required scanner permissions.

Until permission is resolved, the system can hold the first filesystem open and
an API read can time out. Do not feed paper to diagnose that situation. Complete
a read/thumbnail check against an existing batch before the next physical scan.

## Migration procedure

1. Confirm no active/queued hardware jobs and unload the service LaunchAgent.
2. Back up the local SQLite ledger and service configuration.
3. Inventory all batch files, including pre-ledger hardware-test batches.
4. Create each destination exclusively. Copy files and verify SHA-256; preserve
   originals' exact bytes. Flatten image paths and keep delivery files in hidden
   staging. Update manifests and existing edit recovery metadata.
5. Transactionally update job records, batch manifests, image records and historical
   event paths in the local ledger. Preserve existing idempotency submission bytes.
   Import older batches by their original IDs rather than creating scanning jobs.
6. Set the storage configuration, update/restart the service, and verify retrieval
   and thumbnails for every original and retained edit through its API.
7. Only then remove source files whose source and destination checksums still match
   the migration inventory. Preserve unrecognized or changed source files for review.

Migration backups and checksum inventories are stored privately under
`~/Library/Application Support/Scanner/backups/`. Diagnostic exports/screenshots
in the repository are test artifacts rather than canonical scan batches and stay
outside the live scan library.

Migration and the subsequent physical native-app scan are documented in
[storage validation](storage-migration-validation.md). The later permanent-number
migration is recorded in [batch naming validation](batch-naming-validation.md).

## Checks

```sh
# Mock fixture writes on the real mount; no scanner hardware access:
SCANNER_TEST_SMB_ROOT=/Volumes/scans swift test --disable-index-store -Xswiftc -warnings-as-errors
# Current configured destination:
cat "$HOME/Library/Application Support/Scanner/storage.json"
# Normal scan command uses the service destination; load/confirm paper first:
scanner scan --duplex --format png
```

The configuration test rejects local fallback directories and credential-bearing
URLs. The opt-in mounted-volume test creates a private temporary fixture batch,
verifies PNG preservation and the flat layout, and removes its own fixture files.

The Rust Windows storage adapter resolves connected mapped drives through
`WNetGetUniversalNameW` after canonicalization. It accepts the configured UNC
share only; a disconnected mapping or local drive cannot become fallback scan
storage. It never connects or authenticates a share. Open scan files are checked
by native handle for reparse points and multiple hard links on Windows, as well
as the existing Unix link-count checks. The mapped-drive API is documented by
[Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/winnetwk/nf-winnetwk-wnetgetuniversalnamew).
