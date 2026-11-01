# Overscan diagnostic

The fi-8170 macOS driver exposes `cropping=4`, labelled Overscan. Its guide says
this adds background areas above, below, left and right of the image:
https://origin.pfultd.com/downloads/IMAGE/macos/240/P2ZZ-0370-05ENZ0.pdf (page 17).
The installed feature enumeration does not expose a separate millimetre margin
or a backing-colour choice for Overscan. Values 0/1 select uncropped black/white
instead; they cannot be combined with value 4.

Local JSON-RPC scan requests accept optional `overscan:true`. This selects and
verifies cropping value 4, records `background:driverDetermined` in applied
settings, and restores the previous driver value after capture. The request must
omit `background` and disable `prepareCards`: backing colour and output geometry
must first be physically validated. Manual card preparation also rejects this
experimental mode rather than silently choosing the wrong boundary detector.

Existing requests and physical-button settings are unchanged. No brightness,
contrast, pre-pick, multifeed, paper-protection or feed-mode settings are changed.
This diagnostic does not promise the Windows screenshot's 4 mm margin.

Initial test: three user-confirmed expendable cards, 300 DPI, colour, duplex PNG.
Expected result: six originals. Inspect their background, leading/trailing edges,
dimensions and margins before deciding whether this mode improves edge capture.

## Physical result — B-023, 2026-11-01

Job `EC4C7484-7E6D-4DC0-BDA6-28F231C055D5` completed at 13:31:30 UTC after
starting at 13:31:17 UTC. Three loaded cards produced six 2656 × 3884 PNG
originals, with no scan errors and confirmed session closure. The user confirmed
all three fed cleanly with no new damage. File sizes, PNG signatures, SHA-256
hashes, sequence order and manifest agreement were verified.

The observed backing is black. The offline native card-boundary analysis found
space on all four sides for all six images, verified their upright orientation,
and produced no scan-boundary warning. Detected leading buffer was approximately
181–184 pixels (15.3–15.6 mm using nominal 300 DPI, including the detector's
conservative padding); this is an observation, not an adjustable 4 mm setting or
a calibrated physical measurement. Prepared images were not written into the
batch; it remains an original-only diagnostic for review.

After scanning, a fresh inspection confirmed restored cropping value 2 and
blank-page-skip value 3, with multifeed 1, pre-pick 1, tone 1 and sharpness 0
unchanged from the prior snapshot. ScannerService returned to idle/unpaused.
Normal scans and physical-button defaults remain unchanged. Combining explicit
white backing with Overscan is not exposed by the installed enumeration.

Validation: 130 automated Swift tests and the six-image saved-result fixture
passed; release build and signed service installation succeeded. Evidence is
ignored under `Diagnostics/overscan/`. Re-run the offline geometry check with:

```sh
SCANNER_OVERSCAN_FIXTURE=/Volumes/scans/B-023 swift test --filter savedOverscanCardsHaveBackgroundOutsideEveryEdge
```

This check reads saved originals only and never feeds the scanner.
