# Star Wars: Unlimited

English scans use automatic game detection; no game selector is required.
The standard black back requires its dark/white logo layout and a confidently
read, correctly positioned UNLIMITED word. Printed SWU copyright and English
set identifiers provide evidence for cards with content on both faces. Those
pairs retain numbered side labels rather than inventing a standard back.

Local Vision OCR reads the set and collector number, optional printed set total,
name and subtitle. Public metadata comes from [SWU-DB's documented API](https://www.swu-db.com/api).
The Swift client permits bounded HTTPS GET responses without redirects, cookies,
or image uploads. Metadata is cached in memory for 24 hours. It does not require
an API key. Scanned images and full OCR transcripts stay on this Mac.

Matching requires the English set code, collector number and printed name.
When a total is present it must agree with the catalogue. One substituted name
character is accepted only with an independently exact subtitle as well as the
set/number evidence. Several orientations agreeing on the same printing are
one suggestion; conflicting printings stay unresolved. Some alternate-art
printings omit the set total, so a bare collector number is valid next to the
set/language code. Token-only faces without a catalogue identifier may remain
unmatched. A match is a suggestion requiring review, not an assessed foil finish.

The dark-border crop fallback retains actual pixels from disconnected foreground
components, including the faint outer border. Minimum-area and straight-edge
analysis corrects small tilts before cropping; component bounding-box corners
are not used because they erase the tilt. The crop retains one analysis pixel
around the observed edges for interpolation safety. It is accepted only with
SWU text/logo evidence and a card-shaped enclosing area. Original PNGs are
preserved. Cropping or recognition failures do not discard captures.

Pokémon finish, condition, price and Store export logic does not run on SWU
cards. Supporting SWU pricing or Store staging needs separate provider/contracts
work; the current UI explicitly says finish and condition are not assessed.

Saved-image regressions are opt-in and never operate the scanner:

```sh
SCANNER_SWU_BATCH=/Volumes/scans/B-050 swift test --filter savedStarWarsPreparation
SCANNER_SWU_RECOGNIZE=1 swift test --filter savedStarWarsCatalogueMatches
```

The first command creates prepared test copies under `/tmp/swu-prepared-*.png`;
the second reads them and the public catalogue. No private images are committed.
Offline tests cover parsing, conflicting evidence, game pairing and UI labels.
