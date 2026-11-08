#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
scanner_cache_modules="$(mktemp -d /tmp/scanner-image-cache-tests.XXXXXX)"
trap 'rm -rf "$scanner_cache_modules"' EXIT
Web/node_modules/.bin/tsc --target ES2022 --module ES2022 --moduleResolution bundler --lib ES2022,DOM --skipLibCheck --outDir "$scanner_cache_modules" Web/src/PersistentImageCache.ts Web/src/ImageRequests.ts Web/src/api.ts
SCANNER_CACHE_TEST_MODULES="$scanner_cache_modules" Web/node_modules/.bin/playwright test --config Web/playwright-cache.config.ts
