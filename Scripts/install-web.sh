#!/bin/bash
# Deployment only: install static React assets and update the signed HTTP gateway.
set -euo pipefail
umask 077
cd "$(dirname "$0")/.."
source Scripts/signing.sh
scanner_load_signing_identity
scanner_support="$HOME/Library/Application Support/Scanner"
scanner_plist="$HOME/Library/LaunchAgents/local.scanner.http.plist"
scanner_http="$scanner_support/bin/scanner-http"
scanner_domain="gui/$(id -u)"
if [[ ! -f "$scanner_plist" || ! -f "$scanner_http" ]]; then
    echo 'Install the HTTP LaunchAgent first with Scripts/install-http-agent.sh.' >&2
    exit 1
fi
npm --prefix Web ci
npm --prefix Web run build
swift build -c release --product scanner-http --disable-index-store -Xswiftc -warnings-as-errors
scanner_release="$(date -u +%Y%m%dT%H%M%SZ)-$(uuidgen)"
scanner_web="$scanner_support/http/web-builds/$scanner_release"
scanner_backup="$scanner_support/backups/web-$scanner_release"
mkdir -p "$scanner_support/http/web-builds" "$scanner_support/backups"
mkdir "$scanner_web" "$scanner_backup"
ditto Web/dist "$scanner_web"
cp "$scanner_plist" "$scanner_backup/http.plist"
cp "$scanner_http" "$scanner_backup/scanner-http"
install -m 700 .build/release/scanner-http "$scanner_http.new"
scanner_sign "$scanner_http.new" local.scanner.http
# Validate the public asset snapshot before stopping the installed gateway.
"$scanner_http.new" --web-root "$scanner_web" --launch-agent-plist "$scanner_backup/asset-check.plist"
python3 - "$scanner_plist" "$scanner_backup/updated.plist" "$scanner_web" "$scanner_http" <<'PY'
import os, plistlib, sys
source, target, webroot, binary = sys.argv[1:]
with open(source, 'rb') as file: value = plistlib.load(file)
assert value['Label'] == 'local.scanner.http'
args = value['ProgramArguments']
assert args[0] == binary
if '--web-root' in args:
    i = args.index('--web-root'); args[i + 1] = webroot
else: args += ['--web-root', webroot]
with open(target, 'xb') as file: plistlib.dump(value, file)
os.chmod(target, 0o600)
PY
plutil -lint "$scanner_backup/updated.plist"
scanner_was_loaded=false
if launchctl print "$scanner_domain/local.scanner.http" >/dev/null 2>&1; then
    scanner_was_loaded=true
    launchctl bootout "$scanner_domain/local.scanner.http"
fi
scanner_rollback() {
    trap - ERR
    launchctl bootout "$scanner_domain/local.scanner.http" >/dev/null 2>&1 || true
    install -m 700 "$scanner_backup/scanner-http" "$scanner_http.rollback"
    mv "$scanner_http.rollback" "$scanner_http"
    cp "$scanner_backup/http.plist" "$scanner_plist.rollback"
    mv "$scanner_plist.rollback" "$scanner_plist"
    if [[ "$scanner_was_loaded" == true ]]; then launchctl bootstrap "$scanner_domain" "$scanner_plist"; fi
    echo 'HTTP update failed; previous binary and configuration restored.' >&2
}
trap scanner_rollback ERR
mv "$scanner_http.new" "$scanner_http"
cp "$scanner_backup/updated.plist" "$scanner_plist.new"
mv "$scanner_plist.new" "$scanner_plist"
launchctl bootstrap "$scanner_domain" "$scanner_plist"
trap - ERR
printf 'Web dashboard installed. Existing HTTP address and authentication preserved.\nAssets: %s\nBackup: %s\n' "$scanner_web" "$scanner_backup"
