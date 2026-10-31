#!/bin/bash
# Retain credential, scanner data, logs and executable.
set -euo pipefail
scanner_domain="gui/$(id -u)"
scanner_plist="$HOME/Library/LaunchAgents/local.scanner.http.plist"
if launchctl print "$scanner_domain/local.scanner.http" >/dev/null 2>&1; then
    launchctl bootout "$scanner_domain/local.scanner.http"
    for ((scanner_attempt = 0; scanner_attempt < 60; scanner_attempt++)); do
        if ! launchctl print "$scanner_domain/local.scanner.http" >/dev/null 2>&1; then break; fi
        sleep 1
    done
    if launchctl print "$scanner_domain/local.scanner.http" >/dev/null 2>&1; then
        echo 'HTTP agent is still stopping; plist retained.' >&2
        exit 1
    fi
fi
if [[ -f "$scanner_plist" ]]; then rm "$scanner_plist"; fi
printf 'HTTP LaunchAgent removed; credential and scanner data retained.\n'
