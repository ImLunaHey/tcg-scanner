# Local pumpkin-stamp and printing analysis

The automatic scan pipeline and saved finish refresh use the same native Swift
`CardFinishAnalyzer`. Originals, OCR evidence, selected IDs and reviewed
finish/condition values are retained. No scanner action is needed to refresh a
saved card. All image analysis runs on the Mac; only public catalogue metadata
and reference artwork are downloaded.

## Visual evidence and finish inference

`CardStampDetector` searches an upright prepared front's illustration-edge area
for the orange Pikachu-pumpkin shape. Connected orange regions propose locations;
the normalized silhouette, ear separation and dark eye/mouth details must agree
at 1260- and 630-pixel analysis sizes. A shape score is not a probability. Both
scores must reach 0.88 with overlapping regions; near matches from 0.75 are
uncertain. Bad geometry, small images and unverified preparation do not establish
absence. Other stamp designs are outside this detector's scope.

The 32×32 reference silhouette was measured from B-027/C-001. It contains only
the mark, with no card identity, year or finish labels. Inputs are limited to one
frame, 32 MiB, 40 megapixels and 20,000 pixels per axis, with bounded thumbnail
analysis and cancellation checks. The other five stamped scans test transfer of
that mark; this is a small development set, not a universal accuracy benchmark.

A detected mark is resolved separately against the numbered base card and known
stamped variants. Missing or conflicting metadata never falls back to the base
card's ordinary finish. A common card can have a holo stamped reprint: for example,
the 2023 catalogue lists Pikachu PAL 062/193 as holo. Existing rarity alone is not
sufficient.

When all matching stamped entries agree, their finish is an explicitly
catalogue-based estimate. This supports older yellow-border stamped cards without
applying modern set-code polarity rules to them. If entries disagree, stable
positive border texture may select holo only when the candidate finishes are
exactly normal and holo, within the already supported modern border layout.
It cannot resolve a reverse-holo alternative. Missing texture never selects normal. Unstamped older layouts and
unknown stamp families retain their existing limitations.

The Mimikyu PAL 097/193 catalogue disagreement is preserved: TCGdex lists a stamped
holo, while TCGplayer's 2023 guide lists both Normal and Holofoil. The local image
cue can resolve the finish while retaining all catalogue candidates and sources.

## Bounded catalogue evidence

`TrickOrTradePrintings.swift` contains 90 public catalogue products: thirty each
from the [2022](https://www.tcgplayer.com/categories/trading-and-collectible-card-games/pokemon/price-guides/trick-or-trade-booster-bundle),
[2023](https://www.tcgplayer.com/categories/trading-and-collectible-card-games/pokemon/price-guides/trick-or-trade-booster-bundle-2023)
and [2024](https://www.tcgplayer.com/categories/trading-and-collectible-card-games/pokemon/price-guides/trick-or-trade-booster-bundle-2024)
TCGplayer guides, captured 2026-11-01. Complete public frontend snapshots and
pagination were inspected; prices are not bundled. Base IDs were matched uniquely
using name, numeric collector number and printed set total against TCGdex.
Production checks those same fields before using an entry.

This is primary market-catalogue evidence, not manufacturer certification.
Catalogue mistakes remain possible. Listed years are candidates within bounded
coverage, not proof of a unique physical release year. Later promotional families
are not silently treated as Trick or Trade. Detailed provenance, raw responses,
checksums and the generated registry are retained in ignored
`Diagnostics/stamp-research/`.

TCGdex variant IDs encode attributes and can repeat across cards. References must
retain source, base card ID and variant ID together. The API stores visual mark
evidence, source regions, catalogue candidates and comparison provenance under
`finishEstimate.edition`, separately from a reviewed finish.

## TCGdex reference images and local caching

TCGdex documents one optional [card-level image](https://tcgdex.dev/reference/card).
Its [asset choices](https://tcgdex.dev/assets) select resolution and PNG/WebP/JPEG
format, not finish. The documented high image is 600×825 **pixels**, not 600 DPI.
The current [variant schema](https://github.com/tcgdex/cards-database/blob/master/interfaces.d.ts)
has no image field, and the [compiler](https://github.com/tcgdex/cards-database/blob/master/server/compiler/utils/cardUtil.ts)
generates one image URL per base card/language.

Live inspection found one image for every B-027 base card, no finish-specific
variant images, and no image for Fire Energy SVE 010. Mimikyu's downloaded image
is unstamped despite its API record containing a stamped variant. Distinct base
card IDs can have different artwork; that does not supply images for every finish
sharing one ID.

`TCGdexReferenceCache` downloads only the documented `/high.png` asset. It keeps
SHA-256-validated copies in `~/Library/Caches/Scanner/card-references`, coalesces
requests, refreshes after seven days and bounds disk use to 256 MiB. Downloads
require the fixed HTTPS asset host, no redirects, one valid PNG up to 8 MiB/40
megapixels, safe derived filenames and atomic cache writes. Missing references
and failed requests are distinct outcomes; neither implies absence of a finish.
Cancelling one caller does not cancel another client's shared download; the
bounded fetch can complete into the cache while cancelled callers receive cancellation.

For detected stamps, the same local mark detector checks the cached artwork.
The API records its URL, hash and mark state. This compares the stamp cue; it
does not claim full artwork registration or use the reference as a labelled
holo/reverse-holo photograph. Scanner illumination, rendering and alignment
differences prevent treating a raw pixel difference as reflectivity.

## Pricing, display and Store

Web and native UI show the edition alongside the base set and number. Uncertain
marks and unlisted editions remain explicit. No unique release year is forced.
Prepared fronts receive local stamp checks even when OCR/catalogue identification
fails or finds no match. Retrying the same image preserves existing mark evidence
until a new check completes; a failed retry cannot erase the export restriction.

Valuation selects only explicitly matching stamped TCGdex variants. Missing
stamped quotes remain unavailable, including when a finish has been reviewed.
Ordinary quotes are excluded. Edition keys invalidate old displayed prices, and
the transformed quote-cache namespace changes to avoid reusing earlier records
that omitted stamped variants. Bundled TCGplayer catalogue records do not supply
Cardmarket prices or silently fall back to another currency/source.

Store's live scan-import schema currently has no edition/variant fields and
rejects additional properties. A detected or uncertain stamp therefore blocks
`send to Store` with the card number until that contract can preserve the edition.
The guard also checks failed-transfer retries and asynchronous write boundaries.
It does not hide the edition in names, set names or condition notes. No external
Store contract or data was changed.

The Store change needed is an explicit, validated edition object on scan staging
and resync, retained separately from the base-card identity. It must preserve the
stamp family/state, catalogue source, base card ID, variant ID, candidate years
and suggested finish without treating a candidate year as confirmed. The staging
review must expose unresolved editions, and approval must not silently choose the
ordinary catalogue printing. Existing batch/card numbers remain unchanged. The
separate catalogue-create endpoint's variant field does not solve scan staging.

## Checks

Automated checks cover detector geometry/colour/rotation, reference-cache security
and integrity, exact variant scope, contradictory catalogue entries, pricing
isolation, UI invalidation and Store retry boundaries. Optional saved-image tests
exercise B-027 and existing controls without hardware calls or image uploads.

The full Swift suite passed 192 tests with compiler warnings treated as errors;
the web suite passed 35 tests and its TypeScript/production build. The final
recognition-warning wording change passed four targeted tests. Production Swift,
native app and HTTP builds succeeded. Upstream SwiftPM reports unhandled
`CMakeLists.txt` files in its tracing dependency; there were no project compiler
warnings.

The opt-in offline comparison processed 223 identified fronts: six B-027 pumpkin
stamps and 217 prior controls. All six marks were detected. Of the prior controls,
216 were absent and one had unsupported geometry; none was called present or
uncertain, and all existing finish decisions were retained. Shape calibration used
B-027/C-001, so these numbers describe a development recheck, not blind accuracy.

For B-027 the updated finish estimates match the six physical labels: Greavard,
Dusclops, Gastly and Drifblim normal; Mimikyu and Spectrier holo. Mimikyu uses the
existing independently calibrated positive border cue because the stamped
catalogue entries disagree. Spectrier uses the stamped catalogue finish, rather
than applying modern-border rules to a yellow-border card.

## Installed saved-library recheck

The signed service, CLI, MCP executable, native app and HTTP/web update were
installed on 2026-11-01. The service refreshed all 223 identified fronts across
the saved library without feeding hardware. All 217 prior control finish
decisions were unchanged. Every refreshed identification retained its source,
OCR, manual reviews and condition data; batch fields outside finish estimates
were unchanged. On-disk manifests matched service records.

B-027's twelve originals and twelve prepared PNGs were SHA-256 checked before and
after refresh. All bytes were unchanged. Its sealed pre-development predictions
retain SHA-256 `2433687d08a22f0d9412736669e0870881e4261ca63d3ad2b516c75ac64d6cda`.
No physical scan or remote Store write was performed during this work.

All six B-027 public image references were downloaded and validated. Their local
stamp checks were absent, in contrast to the six positive scan marks. A subsequent
refresh after restarting ScannerService reused the cached PNG and receipt with
unchanged fetch time, inode and hash. PNG modification time is deliberately
touched for least-recently-used eviction, not a new download.

The current Cardmarket valuation leaves these stamped printings unavailable
because their exact supported variant prices are missing; ordinary prices are
not substituted. Runtime snapshots, source receipts, refresh results, integrity
checks and deployment evidence remain under ignored
`Diagnostics/stamped-editions-20261101/`. Cache files, scans, tokens and database
contents are not committed.

## Implementation commits

- `6377904` — `feat(recognition): detect pumpkin stamps and cache reference artwork`
- `ab5170e` — `fix(recognition): isolate cancellation of shared reference downloads`
- `935e5a7` — `feat(recognition): resolve stamped editions in automatic card checks`
- `2cdc455` — `fix(integration): preserve stamped editions in prices and exports`
- `674a05b` — `feat(ui): show stamped editions and invalidate mismatched prices`
- `901b0a3` — `fix(recognition): clarify base catalogue evidence warnings`
- `64d09d3` — `fix(ui): keep rejected card editions unresolved`

## 30th anniversary Pikachu mark

`AnniversaryStampDetector` checks the separated 3/0 cheek glyphs at the expected
right-hand stamp location, using red ink for the yellow mark and blue ink for
the rainbow Energy mark. Glyph masks exclude artwork and card identity. A zero
must retain an enclosed counter (allowing one-pixel scan breaks), preventing the
older 25 logo's rounded 5 from being accepted as 0. Evidence must agree at 1260
and 1050 pixels on the long edge. Shape scores are not probabilities.

The two frozen glyph templates come from saved B-001 C-003 and C-002. Validation
uses the other cards in B-001, saved ordinary/Trick or Trade controls and public
TCGdex Celebrations 25th-logo images. Reproduce with an opt-in JSON list of
`{label,path,present}` records in `SCANNER_ANNIVERSARY_FIXTURES` and the Swift test
`savedAnniversaryStampsAgreeAtTwoResolutions`. These checks never feed hardware.

A visible mark is not enough to substitute a printing. A single independently
numbered catalogue identity and a `variants_detailed` entry stamped
`30th-anniversary` are required for the edition label. This includes `mee` Energy
cards; their base set stays **Mega Evolution Energy**, with **30th Anniversary**
shown separately. Unknown identities and inconsistent/missing variants remain
unresolved and cannot fall back to an ordinary-printing price.

The current TCGdex records for some `30th` Pokémon cards say `normal`, despite
foil descriptions in the product material. The detector does not turn this into
a confident non-holo estimate. Explicit consistent holo/reverse metadata can
support an estimate, not an authenticity or reflectivity measurement.

Sources checked during implementation:
- [Official 30th Celebration product showcase](https://www.pokemon.com/uk/news/pokemon-tcg-30th-celebration-product-showcase)
- [TCGdex Mewtwo 063](https://api.tcgdex.net/v2/en/cards/30th-063)
- [TCGdex MEE Grass Energy 009](https://api.tcgdex.net/v2/en/cards/mee-009)

Validation on the saved fixtures: all 19 B-001 playable fronts were detected,
with no 30th detections among 148 other images (including backs, inserts, archived
ordinary/Trick or Trade scans and three 25th-anniversary references). Two of the
19 provided the glyph templates; this is limited development validation, not a
population accuracy claim.
