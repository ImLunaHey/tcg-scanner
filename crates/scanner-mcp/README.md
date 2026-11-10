# Scanner MCP

In-process MCP 2025-06-18 stdio protocol with an injected `Service` boundary.
The native `scanner-client` implements that boundary. No scanner access occurs
until a tool is called; the server never launches helpers. A runnable application:

```sh
cargo run -p scanner-app --bin scanner-mcp -- --token-file /absolute/private/token --read-only
```

The caller supplies the endpoint (default `http://127.0.0.1:8171`) and private
credential path. Stdout carries protocol messages only; errors go to stderr.
The server implements initialization, ping, tools/list, tools/call and cancellation.
It advertises only implemented operations; read-only mode removes every mutation.
Tool argument validation and schemas share one definition. Tool failures include
both text and identical structured JSON with `schemaVersion`, `ok` and `error`.

Frames are bounded to 64 KiB, arguments to 60 KiB, active tool calls to eight and
buffered responses to eight. Output has a 30-second write deadline. Requests have
unique active IDs. Oversized or incomplete frames close the session. Dropping an
MCP call cancels the proxy request, never an already accepted scanner job: use the
explicit cancel-job tool. Stdio EOF drops outstanding proxy requests.

Finish, condition and inventory tools await their Rust domain implementations. The
workspace path dependencies are intentional during migration; this internal crate
is not published independently.

Protocol references: [lifecycle](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle),
[stdio](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports),
[tools](https://modelcontextprotocol.io/specification/2025-06-18/server/tools).
