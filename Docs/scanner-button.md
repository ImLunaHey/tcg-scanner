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
