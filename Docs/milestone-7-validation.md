# Web workspace validation — 2026-10-31

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

The web dashboard is installed at **http://100.117.220.119:8170/**. Production
assets are served by the signed Swift HTTP gateway, with all scanner operations
delegated to the existing background service. No Node server runs in production.

## Automated checks

- The 59-test Swift suite passed with warnings treated as errors: 58 normal tests
  executed, with the long-running opt-in browser fixture skipped. This included
  the real-SMB mock storage test, transport/security tests, API image edits,
  original-byte preservation, public-asset boundaries, and existing CLI/MCP/native
  integration tests.
- TypeScript checking and the Vite production build passed. Four focused frontend
  contract tests passed. Public dependencies were pinned in package-lock.json.
- Six Playwright checks passed through the actual Swift HTTP gateway and Unix
  socket, backed by an isolated Swift mock: full workflow and recovery/revocation
  on desktop Chromium, phone-sized Chromium and tablet-sized WebKit.
- Workflows included profile creation/deletion, capability-derived controls,
  required readiness acknowledgement, a lost scan response followed by an
  idempotent retry, progress/completion, cancellation, thumbnail retrieval,
  comparison, reload, optional manual copy rotation and logout.
- ZIP exports were opened and every original checked against its manifest hash.
  Offline control was disabled; SSE reconnected; revoked sessions cleared browser
  state. Mobile pages had no horizontal overflow.
- WebKit rendering uses an explicit style-attribute CSP allowance while scripts
  and stylesheets remain same-origin. Screenshot capture in the WebKit test was
  omitted because Playwright injects an inline stylesheet; Chromium captures both
  desktop and phone layouts. The WebKit workflow still exercises and checks the UI
  under the production policy, including errors and downloads.

## Live installation checks

The installer preserved the existing host, port, remote-access flag, token path,
origin configuration and other LaunchAgent settings. The only plist change was
adding `--web-root`. Code-signature verification passed for the installed gateway.
The hardware service and native app were not restarted by this installation.

A headless browser signed into the real gateway and checked the live dashboard,
discovered resolution controls, B-006 previews, mobile layout and an original PNG
download. Its SHA-256 matched:

```text
c0338c06680e1f1c01e2fc4a4be5f5639daec86f3bc81b2700d6f1d213a0164c
```

The live check allowed only authentication-session mutations; all other API
mutations were blocked in its browser harness. Scan history was identical before
and after, with **six jobs**. Start remained disabled without readiness
acknowledgement. The session was revoked on sign-out. The scanner service finished
idle with storage `/Volumes/scans`. No physical scan, card feed or driver safety
setting change was performed during this milestone.

Deployment backup:

```text
~/Library/Application Support/Scanner/backups/web-20261031T190759Z-CC7EA725-04DD-49DE-BF4C-68D98A3E8DB6/
```

Ignored evidence is retained in `Diagnostics/web/`, `Web/test-results/` and
`.build/`. Credentials, scan files, databases and browser sessions are not committed.

## Remaining physical validation

These browser tests ran on this Mac through its Tailscale address. Physical
iPhone/iPad access from another device still needs the user's check. No claim is
made that an actual browser-initiated hardware scan was tested in this milestone.

The user reported removing an unidentified roller and plans to feed bare cards.
Physical tests remain paused until the feeder configuration is established.
Next: [card-test readiness](card-test-readiness.md), then card-sized capture and an
explicitly approved single expendable card test. Front/back pairing and bulk card
feeding remain unverified.
