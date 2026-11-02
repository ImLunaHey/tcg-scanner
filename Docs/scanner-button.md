# Physical Scan-button batches

Start ScannerService with `--scan-button-device DEVICE_ID` to explicitly enable
physical Scan/Enter batches for one scanner. Omit the option to disable automatic
button handling. The generated LaunchAgent preserves the option. This installation
uses fi-8170 ID `02210000-0000-0000-0000-000004C515FF`.

The service holds an idle ADF session and waits for `ICButtonTypeScan`. The driver
must expose `scan_button_monitor` with current value `1` (On); no vendor settings
are changed. Wait for `button-monitor: ARMED` in
`~/Library/Logs/Scanner/service.jsonl`. A press with a loaded feeder closes the
idle session, then creates one ordinary queued job: **600 DPI, duplex PNG, Overscan with black backing, card
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
explicit opt-in and failed-closure protection. **The complete automatic batch
handoff is hardware-verified with the marked A4 sheet**, including loading before
pressing (see B-020 below). For further timing tests, use one expendable A4 sheet,
wait for ARMED, press Scan/Enter once, and verify one new batch with two original
PNGs, confirmed session closure and subsequent rearming.

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
Its physical scan acceptance test is recorded separately below.

## Automatic listener deployment — 2026-11-01

Installed the certificate-signed release with `--scan-button-device` persisted in
the per-user LaunchAgent. The listener reported ARMED at 10:18:50 UTC. A normal
`v1.capabilities` request then cancelled and closed the idle session successfully,
completed its own inspection without errors, and reported a loaded ADF. The
automatic listener rearmed at 10:19:10 UTC. Job history remained unchanged and
service status was idle/unpaused. Signature verification passed.

Validation: 123 Swift tests passed with warnings-as-errors; the eight focused
button tests also passed after extending the rearm-after-cancel and wake cases.
The release build and generated LaunchAgent option check passed. Local evidence
is ignored under `Diagnostics/scanner-button/automatic-*`; the binary/database/
LaunchAgent backup is under `~/Library/Application Support/Scanner/backups/` with
prefix `scan-button-20261101T101847Z`. At deployment the physical button-driven A4
scan was pending; installation and read-only inspection did not initiate a scan.

## Load-before-press check — 2026-11-01

The first automatic tests produced B-018 and B-019, each with two original PNGs,
no scan errors and confirmed session closure. Original file sizes and SHA-256
hashes matched the saved records. The user reported that pressing before loading
worked, while pressing with paper already loaded appeared ineffective. Both
recorded callbacks were accepted; neither was rejected by the empty-feeder gate.
The logs did not establish the timing of the unsuccessful presses or their cause.

A controlled retry waited until the listener was idle, loaded the marked A4 sheet,
waited three seconds, then pressed Scan/Enter. The user confirmed this order
scanned successfully. At 10:21:46 UTC the callback arrived; B-020 completed at
10:21:56 UTC with two original PNGs, no scan errors and confirmed session closure.
This verifies load-before-press and the complete button-to-batch path. The earlier
failure is not reproduced or explained by this result. Presses during processing
or listener handoff are still not queued.

Additional diagnostic logging records the driver's reported `documentLoaded`
and `scanInProgress` values at each button event, plus changes to `documentLoaded`
while the automatic or diagnostic listener waits. These are driver observations,
not independent measurements of physical loading. No triggering policy, scanner
setting or feeding behaviour was changed in response to the timing report.

## Resolution policy — 2026-11-02

Every new scan is resolved to 600 DPI in ScannerService after profile and device
defaults are applied. This includes physical-button, CLI, MCP, HTTP and native-app
jobs. Lower values in saved profiles or incoming requests cannot lower capture
resolution. The persisted job request records the effective 600 DPI; existing
scan history is unchanged. Unsupported hardware fails rather than falling back.
