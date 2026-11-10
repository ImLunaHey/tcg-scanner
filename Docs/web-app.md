# Rune browser application

The browser interface is the Rust Rune UI compiled to WebAssembly. The former
React app has been removed. Build and configuration instructions are in
[Rune web](rune-web.md); native distribution setup is in [portable installation](portable-install.md).

Library operations require a token held only in memory for the active connection.
The separate `/camera` workflow makes bounded anonymous identification requests;
it cannot inspect saved scans. Camera tracks stop on leave/background/disconnect,
and captured frames never enter persistent browser storage. Its Rust worker
provides an offline explanation without caching private images or API responses.

Use an explicitly configured HTTPS origin for remote cameras. Serving the bundle
does not grant access to the authenticated library or mutate scanner ownership.
