# Scanner client

Native bounded HTTP JSON-RPC client shared by application interfaces. Construct
`Client::new(origin, token)` and call `client.call("service.status", json!({}))`.
The app crate supplies a runnable consumer:

```sh
cargo run -p scanner-app --bin scanner -- --token-file /absolute/private/token status
```

Credentials stay in sensitive headers. Redirects and environment proxies are
disabled; plain HTTP is restricted to loopback/Tailscale IPv4. Requests are
limited to 64 KiB and responses to 16 MiB with transport deadlines. Response IDs
and JSON-RPC envelopes are checked, and structured scanner errors are preserved.
This crate performs no file access or process spawning. Workspace dependencies
are intentional during migration; the crate is not published independently.
