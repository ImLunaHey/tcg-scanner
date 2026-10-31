#!/bin/bash
# Deployment tool only. Runtime scanner operations never launch shell commands.
set -euo pipefail
umask 077
repo_dir="$(cd "$(dirname "$0")/.." && pwd)"
scanner_support="$HOME/Library/Application Support/Scanner"
scanner_bin="$scanner_support/bin"
scanner_agent="$HOME/Library/LaunchAgents/local.scanner.service.plist"
scanner_domain="gui/$(id -u)"
if [[ "${1:-}" != "" && "${1:-}" != "--no-start" ]]; then
    echo 'Usage: Scripts/install-launch-agent.sh [--no-start]' >&2
    exit 64
fi
# Stop via uninstall first to avoid overwriting the executable of a running owner.
if launchctl print "$scanner_domain/local.scanner.service" >/dev/null 2>&1; then
    echo 'Agent already loaded. Run Scripts/uninstall-launch-agent.sh first; data is retained.' >&2
    exit 1
fi
if [[ -e "$scanner_agent" ]]; then
    echo "Refusing to replace existing LaunchAgent: $scanner_agent" >&2
    exit 1
fi
cd "$repo_dir"
swift build -c release
mkdir -p "$scanner_bin" "$HOME/Library/LaunchAgents"
for scanner_executable in scanner scanner-service scanner-mcp; do
    install -m 700 ".build/release/$scanner_executable" "$scanner_bin/$scanner_executable.new"
    mv "$scanner_bin/$scanner_executable.new" "$scanner_bin/$scanner_executable"
done
"$scanner_bin/scanner-service" --launch-agent-plist "$scanner_agent"
plutil -lint "$scanner_agent"
if [[ "${1:-}" != "--no-start" ]]; then
    launchctl bootstrap "$scanner_domain" "$scanner_agent"
fi
printf 'Installed %s\nCLI: %s/scanner\n' "$scanner_agent" "$scanner_bin"
