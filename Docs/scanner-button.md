# Physical scan-button investigation

The service logs the documented ImageCaptureCore browser selection callback in
normal service logs with the `button-monitor:` prefix. This is diagnostic only:
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

Status: diagnostic listener implemented; physical delivery and button-triggered
batch creation are not yet hardware-verified.

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
its runtime delivery remains unverified. Autolaunch is only a discovery clue:
Apple describes that property as launch-on-connect, not a button-target setting.

While observation is active, service status reports paused and new scans and
hardware inspections are rejected. `v1.diagnostics.stopButtonMonitor` stops it
and waits for session closure. Timeout, sleep and service shutdown also close
the session. Unconfirmed cleanup keeps the hardware restart guard in force.
These methods are local diagnostics, not automatic discovery or remote HTTP routes.
