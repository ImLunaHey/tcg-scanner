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

## Session and capability probe

After installing the ICA driver, inspect the fi-8170 without scanning:

```sh
mkdir -p .build Diagnostics
swiftc -swift-version 6 -parse-as-library Scripts/CapabilityProbe/*.swift -o .build/probe-capabilities
.build/probe-capabilities > Diagnostics/capabilities.json
```

This development tool targets USB vendor/product `04c5:15ff`. It opens an
exclusive scanner session, waits for readiness, reads each advertised functional
unit, restores the originally selected unit if necessary, and closes the session.
Do not use another scanning application during inspection. It never requests a
scan, overview scan or setting write. Each asynchronous wait has a timeout;
failures produce structured errors and exit status 1. Success exits 0.

The JSON contains driver-reported values, enum options, defaults where exposed,
and API read-only status. A setter's existence does not prove the device accepts
all values. Missing defaults remain absent; current values are not substituted.
Capabilities are labelled `driverExposedNotScanTested` until exercised by scans.
Raw enum codes follow the installed ImageCaptureCore SDK. Vendor values and
labels are preserved independently because the driver can return different counts.
Only ADF-specific properties are currently expanded; other unit types receive
the common functional-unit fields. This is not the final capability API.
