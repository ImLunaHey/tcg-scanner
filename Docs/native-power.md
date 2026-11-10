# Native power lifecycle

The Rust hardware service subscribes before accepting HTTP clients. macOS uses
IOKit system power notifications on an owned Core Foundation run loop; Windows
uses PowrProf callback registration; Linux uses logind's system D-Bus interface.
The library-only backend does not require a power service. No helper process runs.
Failure to register prevents hardware-service startup. Losing the event stream
shuts down admission and drains the coordinator rather than silently continuing.

Sleep blocks new operations and cancels physical feeds and queued scans. The
owned coordinator transition waits for active session cleanup, accepted writes,
camera workers, inspections and Store receipts. Acknowledgement then releases
macOS's notification or Linux's delay-inhibitor file descriptor. Those mechanisms
have OS deadlines; they cannot guarantee completion before a forced sleep. Windows
notifications provide no sleep delay. On wake, cleanup must finish before new
admission; cancelled work never starts again. Manual pause and hardware/storage
quarantine survive waking. Manual resume cannot override an active system sleep.

macOS registration and cleanup were exercised without changing system power.
Synthetic callback tests verify notification ordering and deferred acknowledgement;
coordinator tests verify cancellation, closure ordering, manual pause and wake.
Windows and Linux adapters pass cross-target strict Clippy. Real suspend/resume
with scanner hardware remains an acceptance test on each platform.

```sh
cargo run -p scanner-platform --features power --example power_probe
```

Protocol references:

- [Apple IOPMLib declarations and lifecycle](https://github.com/apple-oss-distributions/IOKitUser/blob/main/pwr_mgt.subproj/IOPMLib.h)
- [Windows callback registration](https://learn.microsoft.com/en-us/windows/win32/api/powerbase/nf-powerbase-powerregistersuspendresumenotification)
- [logind signals and delay inhibitors](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html)
