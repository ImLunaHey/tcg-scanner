# Scanner MCP

`scanner-mcp` is a Swift stdio server using the [official Swift MCP SDK](https://github.com/modelcontextprotocol/swift-sdk)
(resolved version 0.12.1). Every tool calls ScannerService over its private Unix
socket. The adapter contains no scanner backend and requires no graphical app.
Closing an MCP connection does not stop the service or cancel an accepted job.

## Installation and connection

The repository includes project-level connection settings:

- [`.codex/config.toml`](../.codex/config.toml) for Codex in a trusted checkout.
- [`.mcp.json`](../.mcp.json) for clients that support this project configuration format.

Both launch the installed signed `scanner-mcp` from the current user's
`~/Library/Application Support/Scanner/bin/`. The small shell command expands
`$HOME` and immediately replaces itself with the Swift executable. No username,
access key, server address or checkout path is embedded in the configuration.
ScannerService must already be installed and running; opening an MCP connection
does not install, restart or scan anything.

```sh
"$HOME/Library/Application Support/Scanner/bin/scanner" service-status --json
codex mcp get scanner --json
```

Open a new agent session from this checkout to load the project configuration.
Codex only loads project MCP settings in trusted projects; see the official
[Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp).
A host application may additionally filter which configured MCP servers it exposes;
`codex mcp get` confirms configuration, not attachment to an already-running session.
For a new installation, follow [service operations](service-operations.md).

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

The server advertises 30 tools. Read-only mode exposes 19 read tools and rejects
direct calls to state-changing tools; annotations alone are not enforcement.
Each tool publishes an input schema, output schema and read/write annotations.

| Tool | Access | Purpose |
| --- | --- | --- |
| `scanner_prepare_card` | Write | Prepare a saved card image without feeding hardware |
| `scanner_identify_card` | Write | Identify a saved card |
| `scanner_get_card_identification` | Read | Read recognition and review results |
| `scanner_review_card` | Write | Record a reviewed identity, finish and condition |
| `scanner_estimate_finish` | Write | Refresh local finish evidence |
| `scanner_check_condition` | Write | Refresh local visible-mark checks |
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
| `scanner_continue_batch` | Write, physical action | Add another feed to a completed batch, keeping settings and numbering |
| `scanner_get_job` | Read | Job state, images and errors |
| `scanner_list_jobs` | Read | Paginated job history |
| `scanner_cancel_job` | Write | Cancel queued/active work |
| `scanner_list_inventory` | Read | Card quantities, sets and copy locations; paginated search and set filters |
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

## Reading the whole inventory

Call `scanner_list_inventory` with `{"limit":100,"offset":0}`. Append each
response's `entries`, then use its `nextOffset` with the same filters until
`nextOffset` is absent. The response uses the same `InventoryPage` contract as the
web UI. `copies` preserves each copy's batch UUID, batch number and card number.
`quantity` counts scanned copies, including deliberate rescans; `confirmedQuantity`
counts reviewed copies. `status` distinguishes catalogue matches, pending catalogue
entries and unresolved cards.

Optional `search` (up to 160 UTF-8 bytes) and `set` (an exact slug from `sets[].id`)
filter entries. Omit unused filters. `matchingEntries` describes the filtered
result; `totalCopies`, classification totals, `excludedCodeCards` and `sets` describe
the complete inventory, so do not sum these repeated totals across pages. LIVE
inserts are excluded. Pagination is over current service state; avoid changing the
inventory mid-read if you need a consistent full listing.

## Agent workflow

1. Discover devices, inspect capabilities, select a profile and read readiness.
   `ready` is omitted when unknown; a true result does not reserve the feeder.
2. Have the operator confirm appropriate media is loaded before a physical scan.
   Discovery and opening this MCP connection never start a physical scan.
3. Optionally save defaults with `scanner_set_settings` using `dpi`, `duplex`,
   `format` and optional `deviceID`. This validates and persists future defaults;
   it does not immediately write hardware properties. Busy devices reject changes.
4. Call `scanner_start_scan` with one new `requestID` UUID for the intended scan.
   Pass either a profile UUID or `useDeviceSettings: true`, or explicit scan settings.
   The service enforces 600 DPI. Defaults are simplex and PNG. For cards, use
   `duplex: true`, `prepareCards: true` and the correct `cardLanguage` (`en` or `ko`);
   card preparation enables overscan unless explicitly overridden through another interface.
5. Poll `scanner_get_job` with `id`, or use events (`after`, `waitSeconds` up to 20).
   Creation is asynchronous. Check the terminal state and errors, including when
   some images were retained from a failed or cancelled job.
6. Fetch the manifest and originals. To cancel, call `scanner_cancel_job` and poll
   until terminal; an already-feeding page may finish.
7. A further feed requires fresh media readiness and a new request UUID. Use
   `scanner_continue_batch` with `id` and `requestID` to append to a completed batch,
   or `scanner_start_scan` to create a new batch. See [continuing batches](continuing-batches.md).

Example tool arguments (replace placeholders with real IDs):

```json
{
  "requestID": "NEW_UUID_FOR_THIS_INTENDED_SCAN",
  "profileID": "UUID_RETURNED_BY_GET_PROFILE"
}
```

Reuse the **same requestID and same arguments** after a lost response. Changing
inputs with that ID returns conflict; a new ID requests another physical feed.
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
The manifest preserves driver callback order; `cardGames` provides detected
front/back assignments and `processedImages` contains prepared copies.

For a local file export, the CLI performs the same verified transfer. Use the
original format’s extension; downloading does not convert historical TIFFs to PNG:

```sh
"$HOME/Library/Application Support/Scanner/bin/scanner" \
  images download JOB_UUID 1 --output ./original.png --json
```

## Access and limits

The server uses local stdio only; there is no TCP listener or remote authentication
surface. The private Unix socket enforces same-user access.
Read-only mode restricts this adapter, not other processes running as that user.
MCP exposes no arbitrary shell commands or arbitrary file reads. Output directories
for scans must remain inside the service's configured storage root.

At most eight tool calls run concurrently. Decoded arguments are limited to 60 KiB,
unknown fields are rejected, and individual fields have schema bounds. The SDK
buffers incoming stdio messages before these checks; this is not a pre-decode raw
frame size limit. Use trusted local MCP clients. The service separately bounds
its socket requests, job queue and image reads. Do not use all call slots for event
long polling if immediate controls are needed.

Scan tools expose DPI, duplex, original format, profiles, card preparation and card language.
Multifeed, paper thickness and transport safety settings cannot be changed.
The current service enforces 600 DPI. Card preparation, game detection, recognition,
finish estimates and condition checks operate on saved images and preserve originals.
Finish/condition estimates are evidence for review, not verified physical grades.

The compiled MCP executable has automated scan, continuation, cancellation,
download and read-only integration coverage using a mock backend. Earlier physical
paper tests are recorded in [milestone 4 evidence](milestone-4-validation.md).
Configuring or validating this connection does not require a physical test scan.
