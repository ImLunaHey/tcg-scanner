# Scanner MCP — milestone 4

`scanner-mcp` is a Swift stdio server using the [official Swift MCP SDK](https://github.com/modelcontextprotocol/swift-sdk)
(resolved version 0.12.1). Every tool calls ScannerService over its private Unix
socket. The adapter contains no scanner backend and requires no graphical app.
Closing an MCP connection does not stop the service or cancel an accepted job.

## Installation and connection

```sh
# Stop an existing installation first; scan files and metadata are retained.
Scripts/uninstall-launch-agent.sh
Scripts/install-launch-agent.sh
"$HOME/Library/Application Support/Scanner/bin/scanner" service-status --json

# Register with Codex:
codex mcp add scanner -- "$HOME/Library/Application Support/Scanner/bin/scanner-mcp"
codex mcp get scanner --json
```

The current machine already has this registration. Start a new Codex session to
load it if the current session's tools have not refreshed. The official
[Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)
describes stdio registration and shared CLI/IDE configuration.

Other MCP clients should launch the absolute installed executable path, with no
arguments for normal control. Use `--read-only` to expose only read tools, or
`--socket /absolute/private/control.sock` for a nondefault service endpoint.
Do not run the stdio executable as a LaunchAgent: the MCP client owns its pipes.
stdout is reserved for MCP protocol messages; operational errors use stderr.

A generic client configuration, replacing the username with the actual account:

```json
{
  "mcpServers": {
    "scanner": {
      "command": "/Users/USERNAME/Library/Application Support/Scanner/bin/scanner-mcp",
      "args": []
    }
  }
}
```

## Tools

The server advertises 22 tools. Read-only mode advertises 17 and also rejects
direct calls to the five state-changing tools; annotations alone are not enforcement.
Each tool publishes an input schema, output schema and read/write annotations.

| Tool | Access | Purpose |
| --- | --- | --- |
| `scanner_list_devices` | Read | Discover scanners |
| `scanner_get_device` | Read | Readiness and configured defaults |
| `scanner_get_capabilities` | Read | Driver capabilities and unknowns |
| `scanner_get_settings` | Read | Observed settings and saved defaults |
| `scanner_set_settings` | Write | Save validated DPI/duplex/format defaults |
| `scanner_list_profiles` | Read | Profile list |
| `scanner_get_profile` | Read | Profile by UUID or unique name |
| `scanner_create_profile` | Write | Create with an explicit UUID |
| `scanner_update_profile` | Write | Update supplied fields only |
| `scanner_start_scan` | Write, physical action | Queue a scan with an explicit request UUID |
| `scanner_get_job` | Read | Job state, images and errors |
| `scanner_list_jobs` | Read | Paginated job history |
| `scanner_cancel_job` | Write | Cancel queued/active work |
| `scanner_list_batches` | Read | Paginated batch history |
| `scanner_get_batch` | Read | Complete batch record |
| `scanner_get_scan_metadata` | Read | Complete job record |
| `scanner_get_batch_manifest` | Read | JSON manifest from the service ledger |
| `scanner_get_image_metadata` | Read | Original metadata by job and sequence |
| `scanner_read_image` | Read | Bounded base64 original-image chunks |
| `scanner_get_service_status` | Read | Queue and service health |
| `scanner_get_events` | Read | Cursor-based progress, optionally long polling |
| `scanner_get_diagnostics` | Read | Driver/service diagnostics |

Inspection may return busy while hardware is in use. History, events and service
status remain available. Discovery and diagnostics never initiate a scan.

## Agent workflow

1. Discover devices, inspect capabilities, select a profile and read readiness.
   `ready` is omitted when unknown; a true result does not reserve the feeder.
2. Have the operator confirm appropriate media is loaded before a physical scan.
   Current scanning is ordinary A4, RGB, 8 bits per channel. Card feeding and
   software front/back pairing are not validated or implemented at this milestone.
3. Optionally save defaults with `scanner_set_settings` using `dpi`, `duplex`,
   `format` and optional `deviceID`. This validates and persists future defaults;
   it does not immediately write hardware properties. Busy devices reject changes.
4. Call `scanner_start_scan` with one new `requestID` UUID for the intended scan.
   Pass either a profile UUID or `useDeviceSettings: true`, or explicit scan settings.
   If omitted, explicit settings default to 300 DPI, simplex, TIFF.
5. Poll `scanner_get_job` with `id`, or use events (`after`, `waitSeconds` up to 20).
   Creation is asynchronous. Check the terminal state and errors, including when
   some images were retained from a failed or cancelled job.
6. Fetch the manifest and originals. To cancel, call `scanner_cancel_job` and poll
   until terminal; an already-feeding page may finish.
7. A further intended scan requires fresh media readiness and a new request UUID.

Example tool arguments (replace placeholders with real IDs):

```json
{
  "requestID": "NEW_UUID_FOR_THIS_INTENDED_SCAN",
  "profileID": "UUID_RETURNED_BY_GET_PROFILE"
}
```

Reuse the **same requestID and same arguments** after a lost response. Changing
inputs with that ID returns conflict; a new ID requests another physical scan.
Create-profile likewise requires an explicit `id` UUID for safe retries. Profile
updates accept a UUID or unique name; use its stable UUID when renaming.
Queued jobs keep snapshots even when profiles/defaults are changed later.

## Results and original images

Tool results provide matching JSON text and `structuredContent`:

```json
{"schemaVersion":1,"ok":true,"data":{"state":"completed"}}
```

The example shows only a result fragment; actual data follows the
[local API contracts](local-api.md). Domain/transport failures set `isError: true`
and return `{"schemaVersion":1,"ok":false,"error":{"kind":"...","message":"..."}}`.
An unknown tool returns an MCP protocol error. A successful query of a failed job
still has `ok: true`: inspect `data.state` and `data.errors` to determine scan success.
Partial diagnostic reports can also contain errors inside successful query data.

`scanner_read_image` accepts `jobID`, `sequence`, optional `offset` (default 0)
and `length` (default 65536, maximum 262144). Decode `base64`, verify `chunkSHA256`,
and follow `nextOffset` until `eof`. Verify the concatenated bytes against
`metadata.image.sha256` and `metadata.image.bytes`. No filesystem path is accepted.
The manifest preserves driver callback order and marks side identity unknown.

For a local file export, the CLI performs the same verified transfer:

```sh
"$HOME/Library/Application Support/Scanner/bin/scanner" \
  images download JOB_UUID 1 --output ./original.tiff --json
```

## Access and limits

The server uses local stdio only; there is no TCP listener or remote authentication
surface in this milestone. The private Unix socket enforces same-user access.
Read-only mode restricts this adapter, not other processes running as that user.
MCP exposes no arbitrary shell commands or arbitrary file reads. Output directories
for scans must remain inside the service's configured storage root.

At most eight tool calls run concurrently. Decoded arguments are limited to 60 KiB,
unknown fields are rejected, and individual fields have schema bounds. The SDK
buffers incoming stdio messages before these checks; this is not a pre-decode raw
frame size limit. Use trusted local MCP clients. The service separately bounds
its socket requests, job queue and image reads. Do not use all call slots for event
long polling if immediate controls are needed.

Only DPI, duplex and original format are configurable through these tools.
Multifeed, paper thickness and transport safety settings cannot be changed.
Only A4 300-DPI TIFF simplex/duplex have physical scan evidence. PNG and other
workflow branches have automated fixtures, not equivalent hardware validation.

The compiled MCP executable has passed a complete mock scan/cancel/download
workflow and read-only checks against the real fi-8170 and saved physical scans.
A new physical scan initiated through MCP remains a separate supervised paper test;
see [validation evidence](milestone-4-validation.md).
