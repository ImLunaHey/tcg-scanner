# Scanner compatibility patch

Source: https://github.com/anomalyco/rune/tree/e970367bd7d2a51e2e7d58e2978ea32eac7780cb/crates/rune
License: MIT (included in LICENSE).

This is the runtime crate and its assets from the pinned upstream revision.
Examples, integration tests and development artifacts are omitted. The package
manifest has explicit edition/license values, disables auto-discovered development
targets, preserves upstream Rust FFI lints while suppressing dependency-only dead
code warnings, and resolves sibling Rune crates from the same upstream Git revision.
It is excluded from the scanner workspace and patched as a dependency.

Browser compatibility changes in src/platform/web.rs: WebGPU adapter/device discovery
uses a temporary canvas, and claims the display canvas only after success. This
allows the existing WebGL2 fallback to work on HTTPS when navigator.gpu exists but
no GPU adapter can be obtained. The previous attempt permanently bound a WebGPU
context before discovering that no adapter was available.

The Chromium camera lifecycle test runs on HTTPS with a real WebGPU API but no
available adapter, and exercises the WebGL2 fallback. Remove this vendor patch
when an upstream revision includes the same behavior and passes that test.


GPU/font startup now publishes its pending result before sending `Wake::Ready`.
Winit's local user-event delivery can be synchronous; the previous ordering let
the Ready handler see an empty slot and never initialize the first frame. This
removes a possible lost-ready ordering while investigating intermittent blank
startup in cold Chromium fixture runs. It is not proof that every timeout shares
that cause.
If the event loop has closed, the already-published GPU is taken and destroyed.
The patch releases the RefCell borrow before event delivery so reentrancy is safe.


Linux font enumeration and monospace selection in `src/text/system_fonts.rs`
use Fontique's in-process system collection, replacing `fc-list` / `fc-match`
subprocesses. Standard directory scanning and bundled fonts remain fallbacks.
The application separately routes Linux Store links through its native D-Bus
portal adapter, so Rune's optional `xdg-open` link helper is not used by Scanner.
Rune's command-driven recording utilities remain development tooling.
