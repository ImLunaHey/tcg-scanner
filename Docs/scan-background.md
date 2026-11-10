# Scanner background comparison

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

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

Background and overscan are available on local and HTTP job requests, not as
a new web/native/MCP control or saved profile field. Card jobs and physical-button
scans now default to black Overscan (see [overscan.md](overscan.md)); an explicit
white request keeps the white comparison mode. Set `"prepareCards":true` to run cropping, deskew, upright orientation,
local identification and finish/condition checks with the selected backing.
For an existing batch, `v1.images.prepareCard` reads its original background
selection; subsequent `v1.images.identify` uses the prepared image. A duplex batch
prepared after capture is grouped/priced as card pairs even if automatic preparation
was disabled in the original scan request. Original request history is not rewritten.

The white detector estimates backing brightness from both outside margins of each
row, allowing the driver's grey-to-white transition. Darker or chromatic connected
foreground includes the grey/coloured card border. The existing component-size,
aspect-ratio and multiple-object guards, conservative enclosing crop, deskew and
orientation logic remain in force. White-on-white borders and unusually pale media
still need review; this is validated on the current card batch, not every card design.

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

## Full white pipeline and comparison — 2026-11-01

Reprocessed B-022 through ScannerService, without feeding or modifying originals.
All 38 captures received cropped/deskewed prepared PNGs and verified upright
orientation. The local recognition pipeline returned the same 19 single card IDs,
in the same paired order, as black-backed B-017. Both runs estimated normal finish
for all 19 fronts. Backs correctly have no card-name match but are recognized as
standard blue backs for condition checks. All 19 pairs in each batch are priced.

The old condition checker incorrectly flagged two long white backing strips on
each white-backed card: 38 candidate marks across 19 backs. Version
`blue-back-visible-marks-v3` excludes neutral light regions connected to the image
boundary before inspecting the blue border. Separate interior marks remain
detectable in regression tests. Light wear that merges into white backing is not
reliably distinguishable and is explicitly excluded from coverage, not declared
absent. The checker still does not assign a selling grade.

Rechecked both B-017 and B-022 using v3: 19 checked backs and zero candidate marks
in each batch; front surfaces remain outside this blue-back check's scope. All
original and prepared file sizes/hashes passed verification, and original image
records match their pre-processing snapshots. All 38 white originals touch the
leading scan boundary; their retained warnings require checking edge completeness.

Validation: 129 Swift tests, 27 web tests and production builds passed. The explicit
saved-white fixture exercised all 38 original scans without hardware access.
Desktop and 390px browser checks confirmed 19 paired panels, finish estimates and
priced cards in each batch. Browser validation uses the existing authenticated Tailscale site, and the
comparison links open those same batches. No scan images are uploaded for analysis.
Ignored evidence and the interactive sample comparison are in
`Diagnostics/white-comparison/`. White remains available per request. The subsequent B-023 test established
black Overscan as the new card-scan default; see [overscan.md](overscan.md).
