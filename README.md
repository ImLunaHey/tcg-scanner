# Native macOS scanner platform

Initial hardware investigation for a USB-connected Ricoh fi-8170.
The scanner application, service and public CLI are not implemented yet.

## Read-only ImageCaptureCore probe

Requires macOS and a Swift 6 toolchain. From this directory:

```sh
mkdir -p .build Diagnostics
swiftc -swift-version 6 -parse-as-library Scripts/probe-imagecapture.swift -o .build/probe-imagecapture
.build/probe-imagecapture > Diagnostics/imagecapture-discovery.json
```

The development probe observes local scanner discovery for 15 seconds. JSON is
written to stdout and diagnostic messages to stderr. It does not open a device
session, change settings or scan. A successful execution with no devices is not
proof that scanner access works or that the hardware is unsupported. The report
records whether ImageCaptureCore delivered its local-enumeration callback.

This is a standalone development experiment, not the production ScannerCLI.
Production interfaces will communicate exclusively with ScannerService.
Generated local reports and binaries are ignored by Git.
