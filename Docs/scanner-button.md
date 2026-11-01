# Physical Scan-button batches

Start ScannerService with `--scan-button-device DEVICE_ID` to explicitly enable
physical Scan/Enter batches for one scanner. Omit the option to disable automatic
button handling. The generated LaunchAgent preserves the option. This installation
uses fi-8170 ID `02210000-0000-0000-0000-000004C515FF`.

The service holds an idle ADF session and waits for `ICButtonTypeScan`. The driver
must expose `scan_button_monitor` with current value `1` (On); no vendor settings
are changed. Wait for `button-monitor: ARMED` in
`~/Library/Logs/Scanner/service.jsonl`. A press with a loaded feeder closes the
idle session, then creates one ordinary queued job: **300 DPI, duplex PNG, card
preparation enabled**. It uses the configured storage root and next `B-NNN` folder,
with the same cropping, orientation, recognition, finish/condition checks and
pricing pipeline as a web scan. It does not use or modify saved device defaults.
The current capture ceiling remains 100 images / 50 duplex cards.

Other buttons and browser-selection events never create jobs. Empty-feeder
presses are ignored. Repeated events are latched once; no listener runs during
scanning or processing, and a five-second cooldown prevents immediate rearming.
Busy presses are not queued. Manual client scans, settings inspection and bounded
button diagnostics cancel the idle listener and wait for closure before acquiring
the hardware. Sleep/shutdown close it; wake/reconnection allow the service monitor
to rearm it. Missing devices/drivers retry with a five-second minimum backoff.
Unconfirmed closure blocks further hardware operations until service restart.

`scanner.button.jobCreated` and `scanner.button.failed` events are stored in the
normal service event history. The listener only exists in ScannerService; no GUI,
shell process or second hardware owner is involved. Discovery itself never scans.

Automated mocks cover one job per press, busy presses, exact job settings, manual
client handoff, empty/non-Scan event filtering, sleep/shutdown, reconnect/backoff,
explicit opt-in and failed-closure protection. **The automatic batch handoff still
requires a physical A4 test**; only event delivery has been hardware-verified so far.
Load one expendable A4 sheet, wait for ARMED, press Scan/Enter once, and verify one
new batch with two original PNGs, confirmed session closure and subsequent rearming.
Do not use cards for this first test.

## Investigation history

The initial investigation logged the documented ImageCaptureCore browser selection callback in
normal service logs with the `button-monitor:` prefix. That callback remains diagnostic only:
it does not create jobs, configure the scanner, open an extra session, change the
launch application, or send vendor commands. Scanner discovery records the current
autolaunch target to help diagnose routing. The listener remains in ScannerService;
there is no competing hardware owner or GUI dependency.

Apple documents `deviceBrowser(_:requestsSelect:)` as an event of interest that
can originate from a button when the application is its target. It is not exclusively
a Scan-button event, and delivery differs when a session is open. We deliberately
do not map this callback directly to a scan until real event routing is verified.
Ricoh's macOS support table qualifies Scan-button operation as requiring Image
Capture to be running. Neither source proves delivery to our LaunchAgent.

References:
- https://developer.apple.com/documentation/imagecapturecore/icdevicebrowserdelegate/devicebrowser(_:requestsselect:)
- https://www.pfu.ricoh.com/global/scanners/fi/multios-driver/index.html

First test: with the service idle and feeder empty, press Scan/Enter once. Record
the time, scanner display and whether another application opens. Compare the
service log (`~/Library/Logs/Scanner/service.jsonl`) and batch count before/after.
A missing callback is a routing investigation result, not proof the physical button
cannot work. Do not load cards or enable automatic jobs during this test.

Physical Scan-button delivery to the headless service was hardware-verified
with an open session (see result below), before automatic batch creation was added.

## Open-session diagnostic

Empty-feeder and loaded-paper presses produced no browser-selection events and
no new jobs. This does not establish why delivery failed. The discovery listener
had no open scanner session. Ricoh's macOS driver guide, page 21, describes
selecting the scanner in Image Capture and enabling Scan button monitor:
https://origin.pfultd.com/downloads/IMAGE/macos/240/P2ZZ-0370-05ENZ0.pdf

`v1.diagnostics.buttonMonitor` with `{"seconds":300}` starts an explicit bounded
session observation in ScannerService (30–300 seconds). The response says
`opening`; wait for `button-monitor: SESSION READY` in the service log before
pressing the button. The ADF is selected and the monitor feature is logged without
changing its value. No scan command, autolaunch setting or transport-safety change
is issued. The documented open-session Objective-C button selector is logged if
ImageCaptureCore delivers it; it is absent from the current protocol header, so
its runtime delivery was subsequently verified on this installed driver. Autolaunch is only a discovery clue:
Apple describes that property as launch-on-connect, not a button-target setting.

While observation is active, service status reports paused and new scans and
hardware inspections are rejected. `v1.diagnostics.stopButtonMonitor` stops it
and waits for session closure. Timeout, sleep and service shutdown also close
the session. Unconfirmed cleanup keeps the hardware restart guard in force.
These methods are local diagnostics, not automatic discovery or remote HTTP routes.

## Physical result — 2026-11-01

With one A4 sheet loaded and the session diagnostic ready, the user pressed
Scan/Enter. At 10:12:07 UTC, ScannerService received `ICButtonTypeScan` through
`device:didReceiveButtonPress:` from the fi-8170. The service issued no scan request
and the job list remained unchanged. No GUI application was required by our
listener. This verifies button-event delivery in the running per-user service;
it does not yet verify automatic batch creation or a full button-driven scan.

The diagnostic was explicitly stopped at 10:12:23 UTC. The driver confirmed
session closure, and the service returned to unpaused/idle with no queued jobs.
Evidence is ignored under `Diagnostics/scanner-button/`, particularly
`session-after-press.jsonl` and `verified-button-event.json`.

The implementation described above adds the subsequent production handoff.
Its physical scan acceptance test remains separate from this receive-only result.
