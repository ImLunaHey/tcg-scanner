# Scanner applications

Native Rust composition for `scanner-service` and the `scanner` HTTP client.
Build with `cargo build -p scanner-app --bins`. No helper executables are invoked.
The installed Swift service must be stopped before starting Rust: both use the
same per-user hardware lock, acquired before opening the database or driver.
Do not cut over the live ledger until the migration acceptance checks pass.

Create a private credential without printing its value:

```sh
scanner-service init-token --path /absolute/private/http/token
scanner-service serve --config /absolute/service.json
scanner --token-file /absolute/private/http/token status
scanner --token-file /absolute/private/http/token call profiles.list
```

Example isolated configuration (replace paths for the current platform):

```json
{
  "database": "/absolute/private/state/scanner.sqlite",
  "storage": { "storageRoot": "/absolute/scans" },
  "tokenFile": "/absolute/private/http/token",
  "listen": "127.0.0.1:8171",
  "backend": "libraryOnly"
}
```

`backend: native` selects ImageCaptureCore on macOS, SANE on Linux, or TWAIN on
Windows. Linux and Windows require an absolute trusted `driverLibrary` path.
Automatic preparation requires both `tesseractLibrary` and `trainedData` paths.
The `libraryOnly` backend exposes saved data without discovering scanner devices.
Use absolute paths, including drive-qualified paths on Windows. Local state and
credential directories must satisfy the platform's private ownership checks.

SIGINT/SIGTERM stop acceptance and wait for owned writes and scanner session
closure. Interrupted jobs reconcile verified manifests before accepting clients; incomplete
work fails explicitly and physical feeding never resumes automatically. Installers, platform
sleep/wake handling and the rest of the friendly CLI command tree remain pending.
The generic `call` command exposes every currently implemented v1 RPC method.
Physical scan commands require an explicit request UUID for safe retries.
