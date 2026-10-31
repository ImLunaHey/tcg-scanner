#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
swift build -c release --product ScannerMac --disable-index-store -Xswiftc -warnings-as-errors
app="$PWD/.build/Scanner.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp .build/release/ScannerMac "$app/Contents/MacOS/ScannerMac"
cp Apps/ScannerMac/Info.plist "$app/Contents/Info.plist"
codesign --force --sign - --identifier local.scanner.mac "$app"
codesign --verify --strict "$app"
printf '%s\n' "Built $app"
