# Native scanner access

This crate owns the portable driver contract and native adapters. It depends on
shared scanner data, not the service, database, UI or recognition engine. It calls
platform APIs in-process and invokes no helper executable.

The macOS adapter uses Rust `objc2` bindings to ImageCaptureCore. `MacHost` must
live on the process main thread and be pumped with `tick`; a live Tokio runtime
supplies timers. `MacBackend` is a sendable command handle. Shut down the portable
coordinator before dropping the host, and drop the host before its runtime.

Driver settings must be acknowledged as persisted before requesting a feed.
Captures retain ordered output paths, reject reused URLs, bound image callbacks,
wait for cancellation/closure, restore changed settings after acquisition stops,
and quarantine uncertain sessions until the exact native device is removed.
Button monitoring reads the official driver's already-enabled feature and
requires a loaded feeder plus a Scan-button event. It never enables or changes
transport safety settings.

Read-only discovery (does not open a session or request a scan):

```sh
cargo run -p scanner-device --example devices
```

The adapter builds and its contract tests run on macOS. Discovery completed with
no devices reported in the development environment; real inspection, capture,
button delivery and settings restoration remain hardware-unverified.

The Unix SANE adapter dynamically loads a trusted SANE 1 library on one dedicated
worker. It owns process-wide initialization, serializes normal API calls, copies
option descriptors before reloading them, verifies requested ADF/colour/geometry
settings, and waits for persisted-settings acknowledgement before feeding. Only
SANE's explicitly asynchronous cancellation call runs on a second thread; the
worker waits for reads to return before restoring settings and closing.
It preserves complete interleaved RGB8 frames as PNG/TIFF, including padded rows
and unknown heights, and stops on incomplete duplex pairs or capture limits.
Unsupported sources, image formats and requested vendor controls fail explicitly.
Button events are unsupported through this adapter. Install SANE with a backend
for the actual scanner; backend support is separate from API compatibility.

```sh
cargo run -p scanner-device --example sane_devices -- /trusted/path/to/libsane.so.1
```

Native ABI simulations pass on macOS and all targets type-check for Linux x86_64.
See [the native fixture instructions](../../Tools/NativeDeviceTests/README.md).
Physical SANE discovery/inspection/acquisition remains untested. The API contract
is documented in the [SANE standard](https://sane-project.gitlab.io/standard/1.06/api.html).
Windows is still being implemented. The crate and `scanner-types`
path dependency remain internal (`publish = false`); no license is inferred.
