# Scanner RPC

Internal Rust adapter for the scanner's existing version-one JSON-RPC protocol.
The caller constructs a `scanner_core::Engine` with its backend, processor,
database and storage root, then passes it to `scanner_rpc::Router::new`.
`Router::respond` accepts UTF-8 JSON bytes and returns a serializable `Response`.
The public integration test in `tests/protocol.rs` is a runnable, hardware-free
example: `cargo test -p scanner-rpc --test protocol`.

Supported operations cover device discovery/inspection, settings reports,
diagnostics, scans, batch continuation, job history, service status, events and
profiles. The migration ledger lists remaining operations and transport work.

This library opens no listener and performs no authentication. A transport must
authenticate before dispatch, limit body reads to 64 KiB before allocation, bound
concurrency, and enforce its origin/access policy. The router also enforces the
request size, envelope, per-method parameter keys and typed domain validation.
It preserves the scanner v1 requirement for string/numeric request IDs; batch
requests and notifications are unsupported. Camera uploads need a separate
bounded interface. Engine shutdown remains the host application's responsibility.

Workspace path dependencies are intentional during migration; this crate is not
currently published independently.
