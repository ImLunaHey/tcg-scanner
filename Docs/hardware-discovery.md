# fi-8170 discovery investigation — 2026-10-31

## Observed on this Mac

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

USB visibility is verified. ImageCaptureCore scanner access, functional units,
capabilities, settings and scanning are **not verified**. No session was opened,
settings changed or paper fed. LaunchAgent operation has not been tested.
The missing ICA driver is the leading explanation, not a proven exclusive cause.

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

## Next step: install the official driver and repeat discovery

1. Open the [fi-8170 download page](https://www.pfu.ricoh.com/global/scanners/fisp/dl/fi-8170.html).
2. Select **macOS Tahoe 26**, then **Scanner Driver** under Software.
3. Download **Image Scanner Driver for macOS**, version **2.4.1 or newer**
   that explicitly supports Tahoe. Follow the vendor installer instructions,
   including any requested administrator authentication or restart.
4. Keep the scanner powered on and USB-connected, then run from this repository:

   ```sh
   mkdir -p .build Diagnostics
   swiftc -swift-version 6 -parse-as-library Scripts/probe-imagecapture.swift -o .build/probe-imagecapture
   .build/probe-imagecapture > Diagnostics/imagecapture-discovery.json
   cat Diagnostics/imagecapture-discovery.json
   ```

No driver has been installed by this investigation. If discovery is still empty,
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
