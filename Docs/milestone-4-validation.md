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
No physical scan was started during milestone 4 implementation. Local ignored
evidence is in `Diagnostics/milestone4/`; original scans remain in their existing
locations. Codex's `scanner` stdio registration points to the installed executable;
new sessions can load it. This does not assert that an already-open session has
refreshed its available tools.

## Remaining physical acceptance check

The complete MCP workflow is verified with a mock scanner. Physical scanning through
the same service has prior CLI evidence, and real scanner inspection/image retrieval
through MCP passed. A physical job initiated through MCP itself remains untested.

For that test, have the operator load one expendable FRONT/BACK-marked A4 sheet and
confirm readiness. Use `scanner_get_device`, then explicitly call
`scanner_start_scan` with a new request UUID, `dpi: 300`, `duplex: true`,
`format: "tiff"`. Poll to completion, retrieve the manifest and both originals,
verify hashes and markings, and ask about feeding problems. Do not use cards.

Physical cancellation, unplugging during feeding, sleep/wake during feeding and
extended unattended operation remain unverified. Front/back inference, image
processing, remote HTTP, native UI and web UI belong to later milestones.
