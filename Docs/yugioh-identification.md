# Yu-Gi-Oh! identification

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

English Yu-Gi-Oh! fronts now use local Apple Vision OCR and the public
[YGOPRODeck v7 catalogue](https://ygoprodeck.com/api-guide/). The game is selected
from the paired back. There is no game selector and no image upload. Both sides
are prepared before routing; only the recognized front enters this matcher.
The existing Pokémon recognizer and its finish/condition checks are separate.

The matcher reads the title, eight-digit passcode and printed set code. Exact
normalized name or passcode evidence establishes a candidate; contradictory
recognized names/passcodes produce no match. A matching set code narrows that
candidate to a printing. Missing or unlisted set codes retain a base identity
with “Printing not identified” instead of choosing an arbitrary set. Results
remain suggestions for review. The English implementation does not substitute
English metadata for a Korean-configured job.

Candidate metadata includes `provider:ygoprodeck`, `catalogueID` (base card ID),
`localID` (matched printed set code), set name and `rarities`. IDs are namespaced
as `ygoprodeck:<card-id>:<set-code>` (or `:unresolved`). If a code has several
rarities, all are shown. Catalogue rarity is not a measured finish. No Pokémon
finish, border-condition, pricing or Store pipeline runs for these cards.

## Local caches and network boundaries

The service requests the full catalogue from the fixed HTTPS endpoint
`https://db.ygoprodeck.com/api/v7/cardinfo.php`, then matches text locally. OCR
text is not submitted as a search query. Data is cached in
`~/Library/Caches/Scanner/ygoprodeck/catalogue.json` for two days. Concurrent
catalogue requests share one download; a valid stale cache remains usable if a
refresh fails, with a warning and a five-minute retry interval.

One base artwork for each matched card is downloaded from the fixed
`images.ygoprodeck.com/images/cards/<numeric-id>.jpg` path and cached locally.
This follows the provider's instruction to cache data/images rather than
continually hotlink them. It is a reference artwork, not a verified photograph
of every printing or foil treatment. References are not used to infer rarity
and are not hotlinked in the dashboard. Failed image downloads do not discard
text matches.

Catalogue responses are limited to 64 MiB, reference images to 8 MiB and
16 megapixels, and the image cache to 256 MiB with oldest-access eviction.
Requests have timeouts, reject redirects, accept no arbitrary host or path,
and use no cookies. Cache writes are atomic and reject symbolic-link files.
All runtime work is native Swift; no shell or interpreter is launched.

YGOPRODeck's top-level Cardmarket price is a minimum across versions. This
integration deliberately does not treat it as the value of the matched printing.
Physical finish, condition and edition details such as first-edition status
remain unassessed. Store staging for Yu-Gi-Oh! is still unsupported.

## Saved-image validation

B-044 contains four copies of GMX Suppression Squad. Local OCR independently
read `39706423` and `BLZD-EN091` on all four prepared fronts. Each matched
Blazing Dominion with Common as its listed catalogue rarity. This validates
one printing on four physical cards, not an accuracy percentage across the game.

The optional test uses existing saved images and a locally downloaded catalogue;
it neither feeds hardware nor downloads reference images:

```sh
SCANNER_YUGIOH_BATCH=/Volumes/scans/B-044 \
SCANNER_YUGIOH_CATALOGUE=/path/to/cardinfo.json \
swift test --filter savedYugiohFrontsMatchPrintingsUsingLocalOCR
```

Regular tests cover contradictory evidence, unreadable fields, unresolved
printings, multiple rarities, cache reuse/restart/offline behaviour, reference
validation, service routing and client presentation.

## Installed validation — 2026-11-04

Signed service, web and native updates were installed after 220 Swift tests,
40 web tests and production builds passed. The installed CLI initiated
identification for B-044 captures 2, 4, 6 and 8 without another physical scan.
All four completed with the name, passcode and BLZD-EN091 printing described
above. Each result records `apple-vision-ygoprodeck-text-v1` and the matching
prepared-source checksum. No finish estimate or condition check was generated.

The service downloaded and cached the real catalogue (14,655 entries in the
observed response) and the base artwork. All eight original and eight prepared
PNG hashes remained unchanged. Manifest and authenticated HTTPS metadata agreed
on all four identities. Refresh an already-open dashboard page to load the new
client bundle; no rescan is required.
