# SMB storage migration and native scan validation — 2026-10-31

Historical paths below describe this validation run. They were subsequently
renamed to `B-001` through `B-006`; see [batch naming validation](batch-naming-validation.md).

The scanner now uses `smb://lake/scans` mounted at `/Volumes/scans`. Its SQLite
ledger remains local. `storage.json` in local Application Support records the
required share and root. The installed service LaunchAgent reads this persistent
configuration, so future installer runs do not reset the destination to `~/Scans`.

## Existing scans moved

Five existing batches were copied with exclusive destination creation:

- `A0D0EE1A-F8CC-4037-AE4B-E035D8D590BC` — one original TIFF, older simplex test.
- `487F02EC-735B-492F-AD9D-A8C80A1DF9BD` — two original TIFFs, older duplex test.
- `8D93D5FD-C0A7-4379-B5EC-B8EB16291611` — two original TIFFs.
- `945EE3E2-F609-4C1B-9F2E-A778759B1566` — two original TIFFs.
- `C993DDA4-DF86-454B-BC27-63E09E64B984` — two original PNGs and two retained
  manually generated test edits.

There were 27 files, including manifests, hidden delivery/recovery files and edit
sidecars, totalling 14,538,082 bytes at the destination. Nine original images and
two existing test edits were preserved without re-encoding. All image checksums
match their pre-migration metadata. Paths were flattened to `batch-<UUID>/` with
page files directly inside; neither `originals/` nor `processed/` exists in these
batches. Old edits are clearly named `edit-<UUID>.png` and no new processing ran.

The two older batches were imported into the local ledger using their original
IDs. Existing submission blobs were preserved for idempotent retries. Job records,
batch manifests, image metadata and historical event output paths were updated.
SQLite integrity and foreign-key checks passed.

The background service initially stalled on its first share read because macOS
requested Network Volumes permission for `scanner-service`. The user approved it.
Afterward, every original and existing edit was retrieved through the service
in chunks, SHA-256 verified, and previewed through the thumbnail API.

Only after these live checks did cleanup recheck every source/destination digest
and remove all five old canonical batch directories. Diagnostic exports and
screenshots remain ignored test artifacts. Private rollback metadata, the SQLite
backup and a complete checksum inventory are retained at:

```text
~/Library/Application Support/Scanner/backups/storage-20261031T174022Z/
```

## Real scan from the native application

The user freshly confirmed that the marked A4 sheet was loaded. Using the native
SwiftUI app, the scanner was set to fi-8170, custom settings, 300 DPI, PNG and duplex.
The loaded-paper acknowledgement was checked and **Start scan was pressed once**.

Job/batch: `3984D24B-1016-45CD-A002-3008BDF4497B`.
Started at 17:48:49 UTC. Result: **completed**, two saved images, no reported
scanner errors, and the hardware session closed successfully.

```text
/Volumes/scans/batch-3984d24b-1016-45cd-a002-3008bdf4497b/
  page-000001.png
  page-000002.png
  manifest.json
```

Hidden `.incoming/` retains the driver-delivered source files for recovery.
There is no `originals/` or `processed/` directory and no processed-image record.

| Image | Dimensions | Bytes | SHA-256 |
| --- | --- | --- | --- |
| page-000001.png | 2480 × 3508 | 979412 | `c0338c06680e1f1c01e2fc4a4be5f5639daec86f3bc81b2700d6f1d213a0164c` |
| page-000002.png | 2480 × 3508 | 951321 | `0728d56f77a545ef3db60143a2d5993d847f06e441a4cbd0ca565b3153f6e6a4` |

The native library displayed scanning progress, then completion and both previews.
The first preview visibly shows BACK; the second shows FRONT. These observations
apply to this loaded paper orientation and are not a general card-pairing rule.
Metadata therefore keeps `side: unknown`. No rotation, crop or other application
processing was applied. The user has not yet reported the physical sheet's feeding
condition for this particular test.

The authenticated HTTP manifest endpoint also returned the new flat SMB paths,
and an authenticated HTTP image download matched the first original's exact
SHA-256. The gateway's existing Tailscale LaunchAgent configuration and credential
were preserved. The service was idle with no queued job after validation.

## Automated checks

- **54 Swift Testing tests passed**, including the opt-in real-SMB storage test
  using a generated fixture image and no hardware access.
- Required-share tests reject missing mounts, local fallback directories and
  credential-bearing SMB URLs before a batch directory is created.
- Flat-layout tests verify original preservation, sequential names, batch UUIDs
  and absence of `originals/` and `processed/` folders.
- Release Swift build passed with compiler warnings treated as errors.
- HTTP regression tests: **5 passed** after adding `storageUnavailable` → HTTP 503.
- Updated service, HTTP gateway and native app binaries are installed.

Runtime evidence, including the physical job record and previews, is under ignored
`Diagnostics/storage-migration/`. No database, credentials or original images were
committed. GUI cancellation during a physical scan and notification delivery still
require separate checks; card feeding and front/back card naming remain unverified.
