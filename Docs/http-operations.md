# HTTP gateway operations

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

The per-user HTTP LaunchAgent is `local.scanner.http`. Its default listener is
127.0.0.1:8170. ScannerService must be running separately. Installation creates a
private credential once and retains it across updates. Startup never triggers scans.

```sh
Scripts/install-http-agent.sh
launchctl print "gui/$(id -u)/local.scanner.http"
# Sessions expire on gateway restart; scanner jobs continue in ScannerService:
launchctl kill SIGTERM "gui/$(id -u)/local.scanner.http"
# Remove gateway startup, retaining credential, binaries and scans:
Scripts/uninstall-http-agent.sh
```

For updates, uninstall first, then install. `--no-start` writes the plist without
loading it. Logs are `~/Library/Logs/Scanner/http-stdout.log` and `http-stderr.log`.
Authorization headers, session responses and request bodies are not logged. Root
credentials are read only from the configured private file. Log retention is manual.
The executable is `~/Library/Application Support/Scanner/bin/scanner-http`.

The plist preserves host, port, socket, token-file and optional public-origin/remote
flags. To change these, stop the agent and generate a new plist using the installed
executable's `--launch-agent-plist` option; it refuses to overwrite an existing file.
Alternatively edit the stopped plist's ProgramArguments and bootstrap it. Do not run
two gateways on the same address/port. Separate gateways have independent sessions
and rate counters, but share the same scanner-service queue and hardware owner.

HTTP shutdown drains/closes requests; ScannerService remains independent. Closing
HTTP connections never cancels an accepted scan. After an interrupted start response,
check history and retry only with the original request ID and inputs.

## Tailscale

Authentication remains mandatory in both arrangements below. Neither requires
public port forwarding or Tailscale Funnel.

### HTTPS via Tailscale Serve

Keep the gateway on loopback. Resolve the current MagicDNS name from `tailscale
status --json`, then configure the exact HTTPS origin, including a nonstandard port:

```sh
scanner-http --public-origin https://YOUR-MACHINE.YOUR-TAILNET.ts.net:8443
```

Stop the HTTP LaunchAgent before running the foreground example. For automatic
startup, add the same option to the agent's ProgramArguments. Check existing Serve
routes before configuring one so another application's route is not replaced:

```sh
tailscale serve status
tailscale serve --bg --https=8443 http://127.0.0.1:8170
```

On this Mac the CLI may be `/Applications/Tailscale.app/Contents/MacOS/Tailscale`.
Serve requires HTTPS enabled for the tailnet and may prompt to enable it. Tailnet
grants/ACLs restrict connecting devices; gateway authentication still applies.
Host and Origin must match the configured origin. Forwarded or Tailscale identity
headers cannot bypass authentication.

These are setup instructions, not a claim that Serve or tailnet policy was changed.
See official [Serve documentation](https://tailscale.com/docs/features/tailscale-serve)
and [CLI options](https://tailscale.com/docs/reference/tailscale-cli/serve).

### Direct Tailscale interface

```sh
tailscale ip -4
scanner-http --host YOUR_TAILSCALE_IPV4 --port 8170 --allow-remote
```

Use the current literal IPv4 in 100.64.0.0/10. Wildcard, LAN and public bind addresses
are rejected. HTTP between Tailscale peers travels through Tailscale; HTTPS via
Serve is preferable for browser deployment. Binding beyond loopback is explicit;
the default installed agent remains loopback-only. Tailscale IPv6 binding is not
implemented here (IPv6 loopback is supported).

From another authorized tailnet device, request `/api/v1/devices` with the bearer
header; without authentication the expected response is 401. Follow the
[HTTP API workflow](http-api.md) to obtain a session, inspect readiness and, only
after the operator confirms paper is loaded, create/monitor a scan. Events use
fetch streaming with Authorization and the last received cursor. No GUI is required.

## Troubleshooting

- **401:** missing, expired or revoked credential; gateway restart invalidates sessions.
- **403:** Host/Origin mismatch or storage access rejection. Check origin and port.
- **409:** scanner busy or conflicting idempotency input; poll existing jobs.
- **429:** request/stream/session limits; close unused streams and respect Retry-After.
- **503:** check ScannerService/socket and retained originals. A job can continue
  while HTTP is unavailable. Do not delete partial files in response to an error.
- **Incomplete download:** retry the image GET and verify SHA-256 against the manifest
  or X-Image-SHA256. Range requests are not supported.
- **Thumbnail source too large:** download the original; thumbnails have tighter bounds.
- **No remote connection:** check Tailscale connectivity/grants, bind address, Serve
  route and origin. There is no unauthenticated health bypass.
- **Disk full:** free space before scanning/rebuilding. Originals/history have no
  automatic deletion policy; build caches can be removed and regenerated.
