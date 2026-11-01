# Scanner background comparison

The installed fi-8170 ICA driver exposes these `cropping` enumeration values:
`0` = None (with black background), `1` = None (with white background).
Ricoh documents both on page 16 of its macOS driver guide:
https://origin.pfultd.com/downloads/IMAGE/macos/240/P2ZZ-0370-05ENZ0.pdf

Local JSON-RPC `v1.jobs.create` / `v1.scan.start` accepts optional
`"background":"white"` or `"background":"black"`. Omission preserves the previous
black default and compatibility with existing saved requests. The choice is saved
in the request and applied settings; reusing an idempotency key with a different
background is rejected. The service checks that the driver exposes and accepts
the selected enumeration value, and restores its previous setting after capture.
No multifeed or transport-safety settings are changed.

This is initially a diagnostic option on the local API, not a new web/native/MCP
control or saved profile field. Physical-button scans retain their existing black
default. The card boundary detector currently assumes a dark background: use
`"prepareCards":false` for the initial white comparison to preserve full captures
without applying that detector. White-background cropping needs separate validation.

White is driver-exposed, but a physical white scan has not yet been validated.
