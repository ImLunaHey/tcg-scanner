#!/bin/bash
# Installs only the native UI. ScannerService remains a separate LaunchAgent.
set -euo pipefail
cd "$(dirname "$0")/.."
scanner_app="$HOME/Applications/Scanner.app"
if pgrep -x ScannerMac >/dev/null; then
    echo 'Quit Scanner before installing an update. Background scanning can continue.' >&2
    exit 1
fi
if [[ -e "$scanner_app" ]]; then
    scanner_bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$scanner_app/Contents/Info.plist" 2>/dev/null || true)
    if [[ "$scanner_bundle_id" != local.scanner.mac ]]; then
        echo "Refusing to replace an unrelated application: $scanner_app" >&2
        exit 1
    fi
fi
Scripts/build-mac-app.sh
mkdir -p "$HOME/Applications"
ditto .build/Scanner.app "$scanner_app"
codesign --verify --strict "$scanner_app"
printf 'Installed %s\nOpen it with: open "%s"\n' "$scanner_app" "$scanner_app"
