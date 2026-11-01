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

## Physical validation — B-022, 2026-11-01

Job `883B48A3-E246-42BD-8E51-A83793322063` scanned 19 loaded cards at 300 DPI,
duplex PNG with `background:white` and `prepareCards:false`. It completed from
13:14:58 to 13:15:45 UTC with 38 original PNGs in `/Volumes/scans/B-022`, no scan
errors and confirmed session closure. The recorded applied settings show
`cropping:1`, `background:white` and unchanged multifeed settings. The user
confirmed all 19 cards reached the tray cleanly with no new damage.

All 38 image signatures, byte counts and SHA-256 hashes were verified; sequences
1–38 are present and the manifest matches the service record. First front/back
and last front samples visibly have a pale backing with some shading/shadow,
rather than a perfectly uniform white area immediately around the card. The
unused lower A4 region is white. Originals retain their capture orientation and
full scan area. No automatic white-background crop, orientation correction,
identification, finish or condition assessment was attempted in this comparison.

The release build and 125 Swift tests passed. The signed service was installed
with its existing physical-button configuration preserved. That button and
ordinary requests without a background override still use black. Hardware-test
evidence is ignored in `Diagnostics/white-background/`; the deployment backup is
`~/Library/Application Support/Scanner/backups/white-background-20261101T131452Z`.
