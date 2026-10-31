#!/bin/bash
# Stop automatic startup; retain all binaries, databases, logs and original scans.
set -euo pipefail
scanner_domain="gui/$(id -u)"
scanner_agent="$HOME/Library/LaunchAgents/local.scanner.service.plist"
if launchctl print "$scanner_domain/local.scanner.service" >/dev/null 2>&1; then
    launchctl bootout "$scanner_domain/local.scanner.service"
fi
if [[ -f "$scanner_agent" ]]; then rm "$scanner_agent"; fi
printf 'LaunchAgent removed. Scanner data and binaries are retained.\n'
