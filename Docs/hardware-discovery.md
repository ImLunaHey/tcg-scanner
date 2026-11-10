# fi-8170 discovery investigation — 2026-10-31

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

## Before driver installation

- macOS 26.5.2 (25F84), Apple Silicon; Swift 6.3.3 via Xcode.
- USB registry shows vendor `0x04c5`, product `0x15ff`, location `0x02210000`,
  behind an Apple USB hub, with a 5 Gbit/s link. USB product/manufacturer/serial
  string indices are zero. Identification is by USB ID, not a returned model name.
- `system_profiler SPUSBDataType -json` returned an empty array, but `ioreg`
  showed the connected device. Do not use that profiler result to infer absence.
- No matching Ricoh/Fujitsu/PFU/scanner/PaperStream/ScanSnap package receipts
  were found. `/Library/Image Capture/Devices` and
  `/Library/Image Capture/TWAIN Data Sources` were empty.
- The Swift 6 probe compiled successfully and ran in the current logged-in user
  process context without launching a GUI. Over 15 seconds it received the local
  enumeration completion callback but discovered zero scanners.
- Its JSON report is in ignored `Diagnostics/imagecapture-discovery.json`.

At this point only USB visibility was verified; the missing ICA driver was the
leading explanation for empty ImageCaptureCore discovery.

## After driver installation

The user installed the official package through macOS Installer. The downloaded
`pfufs-macOS_2.4.1.dmg` matched Ricoh's published SHA-256:
`1d5f66b8b75aa0b1f06179e7142911764b528d66ba4bdf7bdf50f01c88ff3c69`.
`pkgutil --check-signature` identified PFU LIMITED (XW4U7W2E9L), and `spctl`
accepted the package as notarized. Installed receipt `com.fujitsu.fiscanner.pkg`
reports version **2.4.1**.

At 2026-10-31T14:48:40Z the same Swift probe completed successfully and found
**one scanner**, with the following framework-reported values:

- Name: `fi-8170`
- Transport: `ICTransportTypeUSB`
- USB vendor/product: `0x04c5` / `0x15ff`
- Device module: `/Library/Image Capture/Devices/FujitsuScanner.app`
- Module version: `1.0` (distinct from installer package version `2.4.1`)

The full local report is in ignored
`Diagnostics/imagecapture-discovery-after-install.json`.
**ImageCaptureCore discovery is now hardware-verified** in this logged-in user's
command-line process. This discovery-only run did not open a session or scan.
Session inspection results follow below; scanning and LaunchAgent operation
remain unverified.

## Session and settings inspection

At 2026-10-31T14:50:58Z, the Swift capability probe successfully opened an
exclusive session, received device-ready notification, inspected the only
advertised unit (`3`, `ICScannerFunctionalUnitDocumentFeeder`) and confirmed
session closure. No errors, scan requests or setting writes occurred.
Full raw output: ignored `Diagnostics/capabilities.json`.

Hardware-tested: discovery, session opening, readiness callback, property reads
and session closure. The following values are **exposed by the driver but not
tested by scanning**:

- ADF present; feeder reports a document loaded. Sensor accuracy is not verified.
- Duplex support is true; duplex is currently disabled.
- Supported/preferred resolutions: 150, 200, 240, 300, 400, 500, 600 DPI.
  Current resolution: 300 DPI. Native resolution: 600 × 600 DPI.
- Current pixel mode: RGB (`2`); supported bit depths: 1 and 8 bits per channel,
  current 8. There is no public supported-pixel-modes list on this functional unit.
- Current document type: A4 (`1`), approximately 8.268 × 11.693 inches.
  Supported document-type IDs: 1, 2, 3, 4, 5, 10, 13, 39, 53.
- Measurement units: inches (`0`), centimetres (`1`), pixels (`5`); current inches.
- Threshold accepted; current/default 128, threshold use disabled.
- Unit state: ready (`1`). Overview scanning: reported unsupported.
- Page-order reversal: false. Odd/even page orientations: EXIF `1`.

Eight vendor settings were returned, all enumerations. Current values equal the
reported defaults in this session:

- `cropping`: `2`, Auto (Speed emphasis). Options 0–4: black background,
  white background, auto speed, auto accuracy, overscan.
- `multifeed_detection`: `1`, On (Overlapping). Options 0–3: off,
  overlapping, length, overlapping and length. **Not changed.**
- `prepick`: `1`, On. Values 0, 1, 2, but only Off/On labels were returned.
- `dropout_color`: `4`, None. Options 0–4: white, red, green, blue, none.
  Driver tooltip says this setting is ignored in colour mode.
- `sharpness` (Text clarity): `0`, None. Options 0–8: none, smoothing,
  three emphasis levels, four descreen levels.
- `tone_adjustment`: `1`, Normal. Options 0–4: none, normal, soft, sharp, bright.
- `blank_page_skip`: `3`, On (Default). Options 0–5: off, then five
  sensitivity levels from low to high. **Not changed.**
- `scan_button_monitor`: `1`, On. Values 0, 1, 2, but only Off/On labels returned.

### Findings to resolve before scanning

Blank-page skipping can omit an image and break pairing based on alternating
front/back callbacks. The first scanning implementation must explicitly preserve
both sides and verify ordering; it must not silently inherit this setting.

Physical size and scan area were returned as zeros; scaling was zero with empty
supported/preferred lists. These are raw observations, not usable scan dimensions
or proof that the scanner supports zero-sized media. Configuration needs validation
before a scan. A4 document size was nonzero despite these fields.

The extra unlabelled value `2` in Prepick and Scan button monitor must remain
uninterpreted. Preserve the raw values and labels rather than inventing labels.

No separate brightness, contrast, gamma, deskew, automatic orientation, paper
thickness, maintenance-counter or firmware controls were exposed in these vendor
features. Tone presets must not be treated as verified equivalents. Their broader
availability is unknown. No software processing is implemented yet. Continuous
feeding, error recovery, actual image output and card safety remain untested.

The probes have now been migrated into the Swift package and service-backed
CLI. See [validation](milestone-1-validation.md). The standalone scripts were
removed to leave one supported hardware owner; their source remains in Git history.

## Official support information

Ricoh lists the fi-8170 for its macOS ICA driver and specifies USB-only support
for this model in the macOS driver. This matches the intended connection.
See [Ricoh's multi-OS driver documentation](https://www.pfu.ricoh.com/global/scanners/fi/multios-driver/index.html).

The [OS compatibility matrix](https://fi-faq.pfu.ricoh.com/hc/en-us/articles/15388075697945-fi-8950-fi-8930-fi-8820-fi-8190-fi-8170-fi-8150-fi-8290-fi-8270-fi-8250-fi-800R-fi-8040-fi-7900-fi-7800-fi-7700-fi-7600-fi-7700S-fi-7480-fi-7460-fi-7300NX-fi-70F)
lists 2.4.1 as the minimum macOS driver version for the fi-8170 on Tahoe 26.
[PFU's Apple Silicon FAQ](https://faq.pfu.jp/faq/show/3710?site_domain=fi)
also lists the fi-8170 and recommends the latest driver.

The driver comparison documents simplex/duplex, resolution options and several
image adjustments, but these are vendor claims, not capabilities observed from
our device. They must not populate a hardware-tested capability report.
Windows PaperStream features must not be assumed available through ICA.
There is no evidence yet that an alternative backend is necessary.

## Driver installation and discovery reproduction

1. Open the [fi-8170 download page](https://www.pfu.ricoh.com/global/scanners/fisp/dl/fi-8170.html).
2. Select **macOS Tahoe 26**, then **Scanner Driver** under Software.
3. Download **Image Scanner Driver for macOS**, version **2.4.1 or newer**
   that explicitly supports Tahoe. Follow the vendor installer instructions,
   including any requested administrator authentication or restart.
4. Keep the scanner powered on and USB-connected, then run from this repository:

   ```sh
   mkdir -p .build Diagnostics
   swift build
   .build/debug/scanner-service --verbose
   ```

   In another terminal from the repository:

   ```sh
   .build/debug/scanner devices --json
   .build/debug/scanner capabilities --json > Diagnostics/capabilities.json
   ```

If discovery is empty on another installation,
check whether Apple's Image Capture application lists the device. That separates
system/driver visibility from issues specific to our command-line process. Do not
press Scan. Capture the installed driver version and any errors before changing
permissions or considering another backend.

## API basis

The probe uses Apple's [ICDeviceBrowser](https://developer.apple.com/documentation/imagecapturecore/icdevicebrowser)
with a scanner/local mask, a retained delegate and the main run loop. The installed
Xcode SDK headers document asynchronous discovery and the local-enumeration
callback. Device identity and module version are recorded only if returned by the
framework. Discovery does not establish that opening a scanner session will work.
