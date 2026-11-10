# Scanner HTTP

Internal in-process HTTP adapter built on Axum. `Gateway::new` takes a shared
Rust coordinator, backend label, root token and validated `Configuration`;
`Gateway::serve` accepts a TCP listener bound to `Configuration::address()`.
It limits connections to 64, headers to 32 KiB/64 fields with a ten-second
deadline, and drains connections on cancellation. `Gateway::router` is also
available for embedding. The host owns credential-file loading, coordinator
shutdown and TLS termination for an explicit public HTTPS origin.

The integration suite is a runnable hardware-free example:

```sh
cargo test -p scanner-http --test gateway
```

Implemented routes include `/rpc` (POST JSON-RPC), device inspection/defaults,
profiles, jobs, continuations, status, image metadata/edits/downloads/thumbnails,
one-hour bearer sessions, resumable SSE events, saved-card preparation/OCR/review,
inventory/checklists/reference thumbnails, pricing, visual assessment, Store
transfers, bounded public camera lookup, Rune web assets, pause/resume and storage
reconnection. The migration ledger records validation and known limitations.

Control routes require exactly one Bearer authorization header. Credentials in
URLs and cookies are not accepted. Only the root token can issue sessions;
session credentials can revoke themselves. SHA-256 root digests use constant-time
comparison, and token generation uses the operating system random source.

The existing gateway's public binary-image policy is preserved: original and
thumbnail GETs (including registered catalogue references) do not require login, but metadata and edits do. Host and Origin
checks apply to all routes. Bind validation permits loopback or explicitly enabled
Tailscale IPv4 addresses; it rejects wildcard/public addresses. Explicit public
origins must be bare HTTPS origins.

Bodies have a five-second deadline and 60 KiB REST / 64 KiB RPC limits. Response
leases retain concurrency slots until the response body is consumed or dropped
(16 total, two binary downloads, four SSE streams). Event cursors are bounded;
SSE uses fixed event names and JSON encoding. Downloads verify chunk boundaries
and the complete recorded SHA-256 before emitting the final chunk. Originals are
read from the configured capability-pinned storage root, not URL-supplied paths.

Reference APIs: [Axum](https://docs.rs/axum/0.8.9/axum/),
[bounded body collection](https://docs.rs/axum/0.8.9/axum/body/fn.to_bytes.html),
[OS randomness](https://docs.rs/getrandom/0.4.3/getrandom/fn.fill.html).
Workspace path dependencies are intentional during migration; this crate is not
currently published independently.
