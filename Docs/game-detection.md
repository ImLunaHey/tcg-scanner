# Automatic game detection

Card preparation now detects the game separately for each complete duplex pair.
There is no game selector or batch-wide Pokémon assumption. Both captures are
prepared first, so a back-first card receives the same routing as a front-first
card. Different games can coexist in a batch.

The service exposes `cardGames` on a scan record and saves it with prepared
images in the manifest. Each entry contains its physical card `number`, `game`
(`pokemon`, `yugioh`, `starwarsUnlimited`, or `unknown`), and, when detected, `frontSequence`,
`backSequence`, `backSHA256`, `method`, and a human-readable `reason`.
The API derives this evidence from preparation metadata matched to the current
original checksum. It does not trust a client-supplied game or cached label.

- Pokémon uses the international blue back layout and Pokémon logo evidence.
- Yu-Gi-Oh! uses the standard English TCG back: dark centre, warm vortex surround,
  KONAMI near the upper left, and Yu-Gi-Oh!/Trading Card Game branding at the
  bottom. One OCR character error in “Trading Card Game” is tolerated only with
  all the other spatial and colour checks. A brown image or KONAMI alone is not
  sufficient.
- SWU uses its dark logo back or printed SWU footer evidence for two-sided cards.
  It retains numbered sides when neither face is a standard back.
- Missing sides, two recognized backs, unsupported backs without SWU footer evidence, stale preparation evidence,
  and simplex scans remain unknown. Missing captures never shift later pairs.

Pokémon and Yu-Gi-Oh! get crop, deskew, upright orientation and front/back presentation.
SWU dark-border crops preserve faint edge pixels for fine deskew and retain a small interpolation margin.
Detected Pokémon pairs enter Pokémon catalogue matching, finish assessment,
condition checks, pricing and Store staging. Detected English Yu-Gi-Oh! fronts
use [local OCR and YGOPRODeck](yugioh-identification.md) for name, set-code and
catalogue-rarity suggestions. Yu-Gi-Oh! finish/condition checks, pricing and
Store export are not implemented. Unknown games retain
originals and prepared images for inspection; they are not silently treated as
Pokémon. Game detection is not an authenticity check or a proof that sequential
captures came from the same physical card.

## Saved-image verification

B-044 (`ED29895E-ABAD-49FF-95AC-AB71CD271349`) contains four Yu-Gi-Oh! cards,
eight original PNG captures, at 600 DPI with duplex and explicit overscan.
The driver recorded `cropping:4`, `overscan:true`, and black backing. The user
confirmed all four fed cleanly without overlapping feeds or new marks.
All four backs and all four fronts passed orientation checks. Each back was
also tested at all four quarter turns. This is evidence for these four cards,
not measured accuracy across the full game or regional back designs.

These optional tests read existing local scans and never operate the scanner:

```sh
SCANNER_GAME_FIXTURE=/Volumes/scans/B-044 swift test \
  --filter 'savedGameBacksDetectAcrossRotations|savedPairsAutomaticallyPrepareAndRouteByGame'
SCANNER_GAME_FIXTURE=/Volumes/scans/B-041 SCANNER_GAME_EXPECTED=pokemon \
  SCANNER_GAME_COUNT=8 swift test \
  --filter 'savedGameBacksDetectAcrossRotations|savedPairsAutomaticallyPrepareAndRouteByGame'
```

The fixture expects back/front acquisition order. The second test drives the
normal automatic pipeline through a backend that supplies saved bytes into a
temporary batch; it checks game metadata, cropping and game-specific routing.
Ordinary automated tests cover mixed game metadata, both feed orders, ambiguous
pairs, gaps, stale checksums, and rejection of non-Pokémon analysis/export.

The first four Pokémon pairs from B-041 also passed the automatic pipeline and
all back rotations. This exposed an existing upright-logo OCR miss on capture 3:
a whole-card pass read “PareMoy”, while isolating the top logo read “PokeMoy”.
The detector now retries that region after the independent layout check, using
the same bounded logo matcher and a minimum OCR confidence. It does not loosen
the colour/layout gates or default an unreadable back to Pokémon.

## Installed validation — 2026-11-04

The signed service, HTTP/web dashboard and native app were updated. B-044 was
prepared through the installed service using `images prepare-card`, without
feeding hardware again. All eight copies verified a two-quarter-turn rotation;
all four backs were labeled Yu-Gi-Oh!, fronts were captures 2/4/6/8, and no
preparation warnings or Pokémon identification records were produced.
SHA-256 and byte counts matched all eight unchanged originals and all eight
prepared PNGs. The on-disk manifest agreed with the service and authenticated
HTTPS batch response. The direct batch URL returned the deployed application.
The first prepared front/back pair was also visually inspected upright.

Validation: 216 Swift tests, 38 web tests, production web build and Swift release
build passed. Saved-image tests passed on four Yu-Gi-Oh! and four Pokémon pairs,
including all quarter turns of each back and the automatic preparation pipeline.
No Yu-Gi-Oh! catalogue, rarity, condition, pricing or Store support is claimed.

Star Wars: Unlimited detection and its two-sided card handling are documented in
[star-wars-unlimited.md](star-wars-unlimited.md).
