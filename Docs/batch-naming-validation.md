# Permanent batch numbering — 2026-10-31

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

The selected layout is `B-001/C-001-front.png`, with dates in metadata only.
This change implements persistent batch numbers and sequential capture names.
Front/back card pairing is still unverified; existing paper images use `C-001.png`
and `C-002.png` (or retain their original TIFF extension), with `side: unknown`.
No physical scan or card-feeding experiment was performed during this change.

## Naming and persistence

- SQLite migration `v3-batch-numbers` creates a permanent AUTOINCREMENT reservation
  table. Allocations are transactional and idempotent by job UUID.
- Numbers are global to this installation, never reset daily, and are not recycled
  after failure. Padding expands beyond 999. Back up the ledger with the scans.
- Job and batch records expose optional `batchNumber`; UUID API routes and original
  idempotency submissions are unchanged.
- The native dashboard, library and exports use the new labels. Dates remain visible
  in the app and recorded in manifests.

## Existing batches

Six completed batches were numbered oldest-first:

| Folder | Existing batch UUID | Originals |
| --- | --- | --- |
| B-001 | A0D0EE1A-F8CC-4037-AE4B-E035D8D590BC | 1 TIFF |
| B-002 | 487F02EC-735B-492F-AD9D-A8C80A1DF9BD | 2 TIFF |
| B-003 | 8D93D5FD-C0A7-4379-B5EC-B8EB16291611 | 2 TIFF |
| B-004 | 945EE3E2-F609-4C1B-9F2E-A778759B1566 | 2 TIFF |
| B-005 | C993DDA4-DF86-454B-BC27-63E09E64B984 | 2 PNG |
| B-006 | 3984D24B-1016-45CD-A002-3008BDF4497B | 2 PNG |

The service was idle and unloaded during migration. The maintenance process held
the hardware-owner lock. SMB rejected `renamex_np(RENAME_EXCL)` with ENOTSUP;
that attempt changed no batches. The successful migration created new directories
exclusively, copied all files, verified SHA-256, and transactionally updated the
local ledger. All original images, delivery files and two historical manual edits
retain their exact bytes. Only manifests and ledger paths/labels changed.
The next reserved batch will be **B-007**; no test reservation consumed it.

Backup ledger, old manifests, full checksum inventory and migration completion
record are private files under:

```text
~/Library/Application Support/Scanner/backups/numbering-20261031T180749Z/
```

## Validation

- **57 Swift Testing tests passed**, including real-SMB mock storage, concurrent
  reservations, retry/restart stability, historical-ledger migration, and existing
  CLI/service/MCP/HTTP integration tests.
- Release build passed with warnings treated as errors. Updated service, CLI, MCP,
  HTTP gateway and native app were installed. App signing verification passed.
- SQLite integrity and foreign-key checks passed; original job submission bytes
  remain unchanged.
- The live native dashboard shows permanent batch labels; authenticated HTTP
  manifest retrieval returns `batchNumber: 6`, `/Volumes/scans/B-006`, and the new
  capture names. Existing HTTP configuration and authentication were preserved.
- The user approved the renewed macOS network-volume permission for the updated
  service executable. The live service then returned all **11 original images**
  and **2 retained edits** with matching SHA-256 hashes; PNG thumbnails worked for
  all 13 images. Authenticated HTTP also returned both B-006 PNG originals with
  matching hashes.
- After those checks, all six obsolete UUID folders were removed. Each of their
  32 files was checked against the migration inventory immediately before removal;
  image and delivery bytes were also checked at the destination. Original manifests
  remain in the private backup. Unknown files would have stopped cleanup.
- `live-verification.json`, `http-verification.json` and
  `source-cleanup-complete.json` in the backup directory record the completed
  checks. Canonical batches now exist only under `B-001` through `B-006`.
