#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source Scripts/signing.sh
scanner_load_signing_identity
swift build -c release --product ScannerMac --disable-index-store -Xswiftc -warnings-as-errors
app="$PWD/.build/Scanner.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp .build/release/ScannerMac "$app/Contents/MacOS/ScannerMac"
cp Apps/ScannerMac/Info.plist "$app/Contents/Info.plist"
scanner_sign "$app" local.scanner.mac
printf '%s\n' "Built $app"
