# Automatic game detection

Card preparation now detects the game separately for each complete duplex pair.
There is no game selector or batch-wide Pokémon assumption. Both captures are
prepared first, so a back-first card receives the same routing as a front-first
card. Different games can coexist in a batch.

The service exposes `cardGames` on a scan record and saves it with prepared
images in the manifest. Each entry contains its physical card `number`, `game`
(`pokemon`, `yugioh`, or `unknown`), and, when detected, `frontSequence`,
`backSequence`, `backSHA256`, `method`, and a human-readable `reason`.
The API derives this evidence from preparation metadata matched to the current
original checksum. It does not trust a client-supplied game or cached label.

- Pokémon uses the international blue back layout and Pokémon logo evidence.
- Yu-Gi-Oh! uses the standard English TCG back: dark centre, warm vortex surround,
  KONAMI near the upper left, and Yu-Gi-Oh!/Trading Card Game branding at the
  bottom. One OCR character error in “Trading Card Game” is tolerated only with
  all the other spatial and colour checks. A brown image or KONAMI alone is not
  sufficient.
- Missing sides, two recognized backs, unknown backs, stale preparation evidence,
  and simplex scans remain unknown. Missing captures never shift later pairs.

Both games get crop, deskew, upright orientation and front/back presentation.
Only detected Pokémon pairs enter Pokémon catalogue matching, finish assessment,
condition checks, pricing and Store staging. Yu-Gi-Oh! names, sets, rarity,
condition, pricing and Store export are not implemented. Unknown games retain
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
