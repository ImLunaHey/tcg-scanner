# Scanner staging

An in-process HTTPS client for the Store scan-staging contract. The host supplies
the origin, instance ID and securely loaded token, and retains responsibility for
persisting receipts, validating image checksums and deciding when to send.

`Staging` exposes import, revision-checked metadata updates and paired PNG upload.
`ownership::can_sync` protects reviewer edits using the last acknowledged scanner
baseline. The API cannot approve staged cards or publish Store inventory.

Responses are limited to 2 MiB, metadata to 1 MiB and each PNG to 10 MiB. Requests
are serialized with a 90-second deadline; redirects and credential-bearing URLs
are rejected. Upstream bodies and transport errors are not reflected to callers.

Run the offline link-validation example:

```sh
cargo run -p scanner-staging --example review_link -- https://store.example /review/1
```

This development crate uses the versioned workspace `scanner-types` dependency
for typed errors. It is not ready for independent publication until that
dependency is published and the repository's license is specified.
