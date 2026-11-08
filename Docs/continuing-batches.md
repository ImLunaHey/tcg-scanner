# Continuing a batch

A completed batch can receive another physical feed without changing its batch UUID, directory or existing card numbers. For example, 25 duplex cards in B-001 are followed by C-026 onwards. The original request, settings, created/started timestamps, images, prepared copies and identification records are retained. Only the newly captured pairs run through automatic preparation and identification.

The CLI starts feeding immediately; first load suitable media and leave Scan/Enter unpressed:

```sh
scanner batches continue BATCH_UUID --request-id NEW_FEED_UUID --no-wait --json
scanner scan-status BATCH_UUID --json
```

JSON-RPC: `v1.batches.continue`, parameters `id` (batch UUID) and `requestID` (new feed UUID).
HTTP: authenticated `POST /api/v1/batches/{id}/continue`, JSON `{"requestID":"NEW_FEED_UUID"}`. Returns 202 and the same batch record.
MCP: `scanner_continue_batch`, parameters `id` and `requestID`; a state-changing physical scan tool excluded in read-only mode.

Reuse the same request ID after a timeout or restart. A retry returns current state without feeding again. A different request ID means another feed. The original batch settings are reused exactly; continuation does not accept a profile, output path or settings overrides. A new scan or physical Scan/Enter press still creates a new batch.

Each feed retains the 100-image callback limit (50 duplex cards); batches have a separate 1000-image limit. A continuation requires space for a full 100-image feed. Image access, preparation and recognition accept the cumulative image sequence. Responses and recovery manifests remain bounded to 32 MiB. The external Store export currently retains its own 50-card limit and stamped-edition restrictions.

Only successfully completed 600 DPI batches with a closed scanner session, contiguous image numbers and complete duplex pairs can continue. Failed, cancelled or interrupted feeds retain every received original but require inspection; they cannot silently attach a new feed across an incomplete pair. Image analysis/editing or Store transfers must finish before a continuation is accepted. No physical feed resumes automatically after a service restart.

Each continuation has an immutable request ID and starting sequence, with its own timestamps, state and saved-image count in `continuations`. It uses a unique `.incoming-<requestID>` staging directory. Original writes refuse existing files. The database reserves the feed before hardware is scheduled. Recovery refuses an older completed manifest from the previous feed when the database has already accepted a continuation.
