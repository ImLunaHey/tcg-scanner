# Milestone 4 validation — 2026-10-31

## Implemented

Swift CLI profile create/show/partial-update/delete, device readiness, persistent
per-device defaults, batch manifests and verified original downloads. All use the
existing service. The standalone Swift MCP server uses official SDK 0.12.1 over
stdio, with 22 tools, structured results, strict schemas and enforced read-only mode.
The service gains shared operations and GRDB migration `v2-device-settings`.

## Automated evidence

`swift test -Xswiftc -warnings-as-errors`: **40 tests passed**.
`swift build -c release -Xswiftc -warnings-as-errors`: **passed**.

The MCP workflow test launches the actual compiled `scanner-mcp`, exchanges MCP
initialization and tool calls through stdin/stdout, and connects it to a real Unix
socket service backed by a mock scanner. It verifies:

- Discovery, capabilities, settings, diagnostics and readiness do not capture.
- Profile creation and partial updates; defaults and profile snapshots.
- Asynchronous scan creation, terminal polling, two saved originals and manifest.
- Repeating a request UUID does not capture again.
- Metadata and chunked image transfer with complete-file SHA-256 verification.
- Additional active/queued jobs and cancellation through MCP.
- Clean stdio EOF without stopping service-owned state.

Additional tests cover read-only tool hiding and enforcement, absent service,
invalid/unknown input fields, unsafe path/shell arguments, bounded reads, symlink
rejection, unique profile names including legacy save, CLI argument validation,
private exports, checksum failures, cleanup and refusal to overwrite files.
Existing concurrency, disconnect, restart, storage and sleep simulation tests pass.
No automated test feeds real hardware.

## Installed and live checks

The three release executables are installed under
`~/Library/Application Support/Scanner/bin/`, with the per-user LaunchAgent running.
The database was backed up before migration. Prior profiles and the completed
physical duplex job remained accessible after the updated service started.

Live CLI checks read readiness, the General documents profile and the prior batch
manifest. A CLI image download matched its original SHA-256. A separate stdio MCP
client using the installed executable in `--read-only` mode:

- Initialized ScannerMCP 0.4.0 and discovered 17 read tools.
- Found the USB fi-8170 through the running service.
- Read capabilities, settings, profiles, readiness, diagnostics, jobs and batches.
- Retrieved the prior physical duplex manifest and both TIFF originals as chunks.
- Verified every chunk and complete original against its recorded SHA-256.
- Confirmed a direct start-scan call was denied in read-only mode before forwarding.
- Confirmed job history was unchanged and the MCP process exited cleanly at EOF.

Existing physical job: `8D93D5FD-C0A7-4379-B5EC-B8EB16291611`.

| Sequence | Bytes | Verified SHA-256 |
| --- | ---: | --- |
| 1 | 747478 | `c35998f39e44bda575c943fc3a618b6d784cf75b94ad9b502b73f01669a39b05` |
| 2 | 695478 | `63dfceb6e654f79b8a0db460d06e7be817fb228d3dfa0f6060a04ee6886e0928` |

These are the unchanged images from the earlier confirmed LaunchAgent paper scan.
No physical scan was started during the initial implementation checks. A subsequent
confirmed paper test is recorded below. Local ignored evidence is in
`Diagnostics/milestone4/`; original scans remain in their existing
locations. Codex's `scanner` stdio registration points to the installed executable;
new sessions can load it. This does not assert that an already-open session has
refreshed its available tools.

## Physical MCP duplex acceptance test

After the operator confirmed the marked A4 sheet was loaded, one new job was
submitted through the installed Swift MCP executable over stdio. The current agent
session had not loaded the registered scanner tools, so a temporary diagnostic
stdio client exercised the actual MCP initialize/tools-call protocol. It did not
call the scanner hardware directly or use the CLI scan command.

- Job: `945EE3E2-F609-4C1B-9F2E-A778759B1566`.
- Started: `2026-10-31T16:33:43Z`; completed: `2026-10-31T16:33:50Z`.
- Settings: A4, duplex, 300 DPI, RGB, 8 bits per channel, TIFF.
- State: completed; errors: none; scanner session closure confirmed.
- Output: `~/Scans/validation/scan-945ee3e2-f609-4c1b-9f2e-a778759b1566/`.
- Two originals, each 2480 × 3508 pixels and one image per file.
- Visual inspection confirmed BACK in sequence 1 and FRONT in sequence 2.
  Labels describe this observed loading orientation; side metadata remains unknown.

| Sequence | Bytes | Verified SHA-256 |
| --- | ---: | --- |
| 1 | 713280 | `69e51f444f0105a2a9ce67a66a1cc647bdb471139673c87c182173c8151120bb` |
| 2 | 706700 | `28acb039e6f483acfd8afb802b7a1e6605ce2af0bba6b9435d45046ac7a7ee64` |

Both originals were retrieved through `scanner_read_image`; every chunk and the
complete image passed SHA-256 checks, and retrieved bytes matched the retained
original files. Before/after settings reports matched except for observation time.
Multifeed settings were not changed. The service returned to idle, with zero queued
jobs and no pause. The MCP process exited cleanly after the client closed stdin.

Evidence: `Diagnostics/milestone4/physical-duplex/` (ignored), including the recorded
request UUID, job, manifest, settings, retrieved TIFFs and preview copies. Originals
and incoming driver files remain in the scan directory. Preview conversion did not
modify original TIFFs. The operator subsequently confirmed the sheet looked fine,
with no feeding or damage problem reported.

Physical cancellation, unplugging during feeding, sleep/wake during feeding and
extended unattended operation remain unverified. Front/back inference, image
processing, remote HTTP, native UI and web UI belong to later milestones.

## PNG default preference

Following the successful paper test, the operator requested PNG for the selling
workflow. New Swift request/settings/profile values, CLI scan/configure/profile
creation and MCP scan defaults now select PNG. Explicit TIFF remains supported;
prior jobs and original TIFFs retain their recorded format and bytes. The installed
General documents profile was updated to PNG without changing DPI or duplex.

TIFF was used for the initial lossless hardware validation. PNG is also lossless.
[eBay currently accepts both formats](https://www.ebay.com/help/selling/listings/pictures-videos/picture-requirements?id=4148);
PNG is the chosen default for browser use. PNG decoding/storage has mock coverage;
the subsequent supervised physical test below verifies actual driver PNG output.

## Physical PNG duplex test

After the operator reloaded the marked A4 sheet and confirmed readiness, the
installed MCP server received one scan request with no format argument. This
exercised the new PNG default through the actual Swift service and ICA driver.

- Job: `C993DDA4-DF86-454B-BC27-63E09E64B984`.
- Started: `2026-10-31T16:38:17Z`; completed: `2026-10-31T16:38:24Z`.
- Applied settings: A4, duplex, 300 DPI, RGB, 8 bits per channel, PNG.
- State: completed; errors: none; scanner session closure confirmed.
- Output: `~/Scans/validation/scan-c993dda4-df86-454b-bc27-63e09e64b984/`.
- Two originals, each 2480 × 3508 pixels, ImageIO type `public.png`, PNG file signature.
- Visual inspection confirmed BACK in sequence 1 and FRONT in sequence 2.
  Side metadata remains unknown; this does not establish a general pairing rule.

| Sequence | Bytes | Verified SHA-256 |
| --- | ---: | --- |
| 1 | 851374 | `47f032f8a38c3a45b46a2b31ae50dde11af0988c0ded8614ff3a6ea800d72b4c` |
| 2 | 887397 | `22d5c9b0435d0614e224ebe851fc0e83a591407ed7eacaa3c49427cca6c24287` |

Both PNGs were retrieved through MCP in bounded chunks. Chunk and full-file hashes
passed, and downloaded bytes matched the preserved originals. No TIFF conversion
was used. Before/after settings matched except observation time; multifeed remained
unchanged. The service returned to idle, with no queued jobs and no pause. The MCP
process exited cleanly after stdin closed.

Ignored evidence is retained in `Diagnostics/milestone4/physical-png-duplex/`.
Original images and incoming driver files remain in the output directory. The operator
subsequently confirmed that this PNG run worked fine.
This verifies PNG duplex at 300 DPI; it does not establish other resolutions,
PNG simplex, card safety or marketplace upload acceptance for these test images.
