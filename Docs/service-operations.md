Current installation: image storage is configured as `/Volumes/scans` on
`smb://lake/scans`. See [storage configuration](storage.md), including the separate
macOS network-volume permission required by the background service. Local SQLite
metadata stays in Application Support. Older examples of `~/Scans` below describe
the default when no persistent storage configuration is present.

# ScannerService operations

## Installation and process context

Run `Scripts/install-launch-agent.sh` as the normal logged-in macOS user. Do not
use sudo. The per-user Aqua LaunchAgent runs without ScannerMac or any other GUI
client. It starts at login and launchd restarts it after exit. Logout stops it;
operation before login is not claimed.

The installer refuses a loaded agent or an existing plist. Update by running
`Scripts/uninstall-launch-agent.sh` then reinstalling; metadata and scans remain.
A manual foreground service must be stopped before loading the agent. Another
owner at any socket is rejected by the fixed per-user hardware lock.

The installer deploys `scanner`, `scanner-service` and `scanner-mcp`. MCP clients
launch their own stdio adapter; closing that adapter does not stop the service or
cancel service-owned jobs. See [MCP setup](mcp.md).

Installed paths:

| Item | Location |
| --- | --- |
| Executables | `~/Library/Application Support/Scanner/bin/` |
| SQLite database | `~/Library/Application Support/Scanner/state/scanner.sqlite` |
| Unix API | `~/Library/Application Support/Scanner/run/control.sock` |
| Hardware lock | `~/Library/Application Support/Scanner/run/hardware.lock` |
| LaunchAgent | `~/Library/LaunchAgents/local.scanner.service.plist` |
| JSON line logs | `~/Library/Logs/Scanner/service.jsonl` |
| Default scan root | `~/Scans/` |

Database/socket directories are private; database/socket files use mode 0600.
Logs also go to macOS unified logging under subsystem `local.scanner.service`.
`--verbose` adds hardware callback logs. Log rotation and data retention automation
are not implemented; monitor log and scan storage during extended operation.

To configure a different root/database/socket, stop the agent and edit its
`ProgramArguments` in the plist before bootstrapping it again. Keep absolute paths,
private database/socket directories and the same database for retained history.
Changing storage roots leaves old files in place and recovery will not write into
the old root. For a manual service, pass `--storage-root`, `--database`, `--socket`.

The executable can generate a plist at a new path without touching the scanner:

```sh
"$HOME/Library/Application Support/Scanner/bin/scanner-service" \
  --storage-root /absolute/scan/root \
  --launch-agent-plist /absolute/new-agent.plist
plutil -lint /absolute/new-agent.plist
```

The generated plist references the invoking executable's absolute path. It uses
KeepAlive/RunAtLoad, a 10-second throttle and a 60-second shutdown allowance.
The service retains hardware ownership while cancelling jobs and draining clients;
its client-drain deadline is 55 seconds. Forced process termination can interrupt
cleanup, which is why startup recovery never resumes feeding.

## Status and maintenance

```sh
launchctl print "gui/$(id -u)/local.scanner.service"
"$HOME/Library/Application Support/Scanner/bin/scanner" service-status --json
"$HOME/Library/Application Support/Scanner/bin/scanner" jobs list --json
"$HOME/Library/Application Support/Scanner/bin/scanner" events --follow --json
```

Graceful restart (cancels queued and active jobs):

```sh
launchctl kill SIGTERM "gui/$(id -u)/local.scanner.service"
```

Stop automatic startup and remove the plist, retaining all data:

```sh
Scripts/uninstall-launch-agent.sh
```

Do not remove lock files to unlock a running process. Their existence is normal;
only the advisory lock indicates ownership. Do not delete the database to clear a
queue: cancel jobs through the API. Deleting a database also deletes idempotency
history and event cursors, although it does not delete original images.

For backups, stop the service first, then copy the database (and any SQLite
sidecars) plus scan directories. Resume by reinstalling or bootstrapping the
retained plist as appropriate. Do not copy only a live database and assume its
sidecars are unnecessary. Recovery can reconcile a newer valid manifest but does
not reconstruct a deleted database by crawling arbitrary folders.

## Troubleshooting

- **No scanner:** check power/USB and Ricoh ICA driver installation. Run `devices`
  and `diagnostics`. Run in the logged-in user session; do not substitute a root
  LaunchDaemon. Current driver observations are in [hardware discovery](hardware-discovery.md).
- **Service unavailable:** check `launchctl print`, the JSON log and socket path.
  A newly started agent may be throttled after repeated exits.
- **Busy:** another inspection/job may own hardware. Read `service-status`.
  Other scanning apps may hold ICA sessions; close those sessions.
- **Paused after cleanup error:** inspect the failed job, physically clear the
  feeder if needed, then disconnect/reconnect USB or power-cycle the scanner.
  The device monitor releases quarantine after macOS confirms removal of that exact
  device and its previous operation finishes. Wake alone cannot clear quarantine.
- **Storage failure:** restore writable storage/free space, inspect retained
  `incoming/` and `originals/`, then restart. Never discard partial originals merely
  because they are missing from the manifest.
- **Interrupted job:** review its manifest/files. A newly submitted UUID explicitly
  requests another physical scan; retrying the same UUID returns the existing job.
- **macOS access prompt:** approve scanner/device access for this executable in the
  logged-in user session if macOS requests it. This machine did not require a new
  prompt for the tested LaunchAgent. Do not assume identical behavior on another Mac.

## USB reconnect and power cycling

Keep the service running when turning the scanner off/on or reconnecting USB.
Discovery continues while it is offline, and the configured Scan/Enter listener
rearms after discovery (normally within a few seconds of driver readiness).
Reconnecting never submits a job. A new button press or explicit client request is
required. An interrupted scan retains received images and fails; queued jobs fail
without feeding. Retrying the interrupted request ID returns its existing result.

Both device-browser and session removal callbacks terminate the old session.
Cleanup stops waiting for cancel/close callbacks from a physically removed device;
the manifest still truthfully reports whether session closure was acknowledged.
Unknown ownership on a device that remains connected stays quarantined. A different
scanner's removal cannot release it. Recovery also works with button monitoring
disabled. Storage failures remain a separate stop condition.

For an idle hardware check, empty the feeder, note the service PID with
`launchctl print`, turn the scanner off, verify `scanner devices --json` becomes
empty, then turn it on. Verify discovery and the `button-monitor: ARMED` log return
without a changed service PID or new job. Repeat with USB unplug/replug. These
physical checks are separate from automated removal, queue and ownership tests.

### Recovery validation — 2026-11-05

- All 224 Swift tests passed with compiler warnings treated as errors. Coverage
  includes removal during opening without a close callback, exact-device quarantine
  release, active-job image preservation, queued-job failure, fresh-job acceptance,
  idle button-listener cleanup failure and retaining quarantine across wake.
- The release build passed. The service was deployed with the existing persistent
  signing identity and LaunchAgent configuration; signature verification passed.
- Live startup discovered the USB fi-8170, reported `paused: false` with no active
  or queued jobs, and armed the Scan/Enter listener. Storage remained `/Volumes/scans`.
- Physical USB unplug/replug and power-cycle checks of this build remain pending;
  automated tests and successful startup do not establish those hardware results.

## Physical paper validation

The single-sheet duplex test below passed through the LaunchAgent on 2026-10-31;
see [the recorded result](milestone-3-validation.md). Physical cancellation, unplug
and sleep/wake tests remain outstanding. These steps are deliberately manual.
Automated tests use mocks and never feed the fi-8170. Obtain confirmation that ordinary expendable A4 paper is loaded before
starting a physical scan; do not use cards for these checks.

1. With the LaunchAgent running, load one FRONT/BACK-marked A4 sheet. Run:
   `"$HOME/Library/Application Support/Scanner/bin/scanner" scan --duplex --no-wait --json`.
   Poll `jobs status JOB_UUID --json` until terminal. Verify both originals, checksum
   metadata, settings restoration and any feeding issue.
2. For a controlled cancellation test, load a small expendable paper batch, start
   another job, then run `jobs cancel JOB_UUID --json` while it is active. Poll until
   terminal and inspect every retained image. Already-feeding pages may complete.
3. Test unplugging USB only in an explicitly supervised paper run. Verify the job
   fails, partial files survive, and reconnect restores discovery. If closure is
   unconfirmed, confirmed removal releases quarantine without restarting the
   service. Pending jobs fail; submit a fresh job only after reviewing the feeder.
   Do not infer physical recovery from a mock test.
4. Separately test sleep/wake first while idle, then in a supervised paper run.
   Verify job cancellation/interruption, retained originals and no automatic feed
   after wake. Forced sleep can precede asynchronous cleanup; this behavior remains
   hardware-unverified. Do not trigger system sleep during unrelated user work.

## Design references

- [Apple: Creating Launch Daemons and Agents](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html)
  describes per-user agents and launchd lifecycle. A system daemon is not used here.
- [Apple: NSWorkspace willSleepNotification](https://developer.apple.com/documentation/appkit/nsworkspace/willsleepnotification)
  requires the workspace notification center, which the service uses along with
  `didWakeNotification`. Notification handling alone is not proof of physical cleanup.
- [GRDB 7.11.1](https://github.com/groue/GRDB.swift/tree/v7.11.1) supplies the SQLite
  queue and migration engine. DatabaseQueue serializes transactional metadata writes;
  FULL synchronous mode is requested. Files/manifests and SQLite are not one atomic
  transaction, so restart recovery reconciles the manifest before marking a job failed.
