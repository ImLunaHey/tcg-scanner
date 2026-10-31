#!/bin/bash
# Stop automatic startup; retain all binaries, databases, logs and original scans.
set -euo pipefail
scanner_domain="gui/$(id -u)"
scanner_agent="$HOME/Library/LaunchAgents/local.scanner.service.plist"
if launchctl print "$scanner_domain/local.scanner.service" >/dev/null 2>&1; then
    launchctl bootout "$scanner_domain/local.scanner.service"
    # bootout may return while graceful shutdown is still draining.
    for ((scanner_attempt = 0; scanner_attempt < 60; scanner_attempt++)); do
        if ! launchctl print "$scanner_domain/local.scanner.service" >/dev/null 2>&1; then break; fi
        sleep 1
    done
    if launchctl print "$scanner_domain/local.scanner.service" >/dev/null 2>&1; then
        echo 'Agent is still stopping; plist retained. Retry uninstall after shutdown completes.' >&2
        exit 1
    fi
fi
if [[ -f "$scanner_agent" ]]; then rm "$scanner_agent"; fi
printf 'LaunchAgent removed. Scanner data and binaries are retained.\n'
