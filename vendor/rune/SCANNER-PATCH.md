# Scanner compatibility patch

Source: https://github.com/anomalyco/rune/tree/e970367bd7d2a51e2e7d58e2978ea32eac7780cb/crates/rune
License: MIT (included in LICENSE).

This is the runtime crate and its assets from the pinned upstream revision.
Examples, integration tests and development artifacts are omitted. The package
manifest has explicit edition/license values, disables auto-discovered development
targets, preserves upstream Rust FFI lints while suppressing dependency-only dead
code warnings, and resolves sibling Rune crates from the same upstream Git revision.
It is excluded from the scanner workspace and patched as a dependency.

The sole runtime change is in src/platform/web.rs: WebGPU adapter/device discovery
uses a temporary canvas, and claims the display canvas only after success. This
allows the existing WebGL2 fallback to work on HTTPS when navigator.gpu exists but
no GPU adapter can be obtained. The previous attempt permanently bound a WebGPU
context before discovering that no adapter was available.

The Chromium camera lifecycle test runs on HTTPS with a real WebGPU API but no
available adapter, and exercises the WebGL2 fallback. Remove this vendor patch
when an upstream revision includes the same behavior and passes that test.
