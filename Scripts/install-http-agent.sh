#!/bin/bash
# Deployment only. The HTTP gateway never invokes shell commands at runtime.
set -euo pipefail
umask 077
scanner_repo="$(cd "$(dirname "$0")/.." && pwd)"
scanner_support="$HOME/Library/Application Support/Scanner"
scanner_bin="$scanner_support/bin"
scanner_plist="$HOME/Library/LaunchAgents/local.scanner.http.plist"
scanner_domain="gui/$(id -u)"
if [[ "${1:-}" != "" && "${1:-}" != "--no-start" ]]; then
    echo 'Usage: Scripts/install-http-agent.sh [--no-start]' >&2
    exit 64
fi
if launchctl print "$scanner_domain/local.scanner.http" >/dev/null 2>&1 || [[ -e "$scanner_plist" ]]; then
    echo 'HTTP agent already installed. Run Scripts/uninstall-http-agent.sh before updating.' >&2
    exit 1
fi
cd "$scanner_repo"
source Scripts/signing.sh
scanner_load_signing_identity
swift build -c release --disable-index-store --product scanner-http
mkdir -p "$scanner_bin" "$HOME/Library/LaunchAgents"
install -m 700 .build/release/scanner-http "$scanner_bin/scanner-http.new"
scanner_sign "$scanner_bin/scanner-http.new" local.scanner.http
mv "$scanner_bin/scanner-http.new" "$scanner_bin/scanner-http"
if [[ ! -e "$scanner_support/http/token" ]]; then
    "$scanner_bin/scanner-http" --init-token
fi
"$scanner_bin/scanner-http" --launch-agent-plist "$scanner_plist"
plutil -lint "$scanner_plist"
if [[ "${1:-}" != "--no-start" ]]; then
    launchctl bootstrap "$scanner_domain" "$scanner_plist"
fi
printf 'Installed authenticated localhost HTTP gateway on port 8170. ScannerService must be running.\n'
