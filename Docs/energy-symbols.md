# Energy type from the corner symbol

`EnergySymbolDetector` identifies basic Energy types locally from the small
top-right printed symbol. It does not use background colour, the central artwork,
the batch number, or a hand-assigned card identity.

The detector requires a readable Energy heading in the upright title area and
rejects cards with readable rules text in the body. It compares normalized dark
ink shapes with reference silhouettes, checks aspect ratio and separation from
the next candidate, and requires agreement at two or more ink thresholds. Blank,
clipped, ambiguous and unsupported symbols remain unresolved. The reference bank
contains Grass, Fire, Water, Lightning, Psychic, Fighting, Darkness, Metal and
Fairy shapes; reference coverage is not a claim that every era/printing works.

The optional API field `evidence.energySymbol` contains the type, method,
shape-overlap similarity and normalized top-left symbol bounds in the upright
image. Similarity is not a calibrated confidence percentage. An unnumbered card
can have this field while its catalogue state remains `noMatch`: no exact
printing has been found. The web and native applications instead display the
type, such as **Psychic Energy**, with **Exact printing unknown**. This evidence
does not assign a catalogue ID, price, foil finish, condition or Store identity.

Unnumbered Energy classification completes without a catalogue request. Numbered
cards still use the existing printed-number/catalogue checks. The shared service
persists the symbol evidence for CLI, HTTP, MCP and native consumers.

## Validation

B-045 C-009 and C-019 both classify as Psychic from their saved upright corner
symbols. C-009 also passed the full recognizer in all four rotations. Automated
checks also identify Fire correctly on six previously matched saved scans from
B-008, B-009, B-010, B-011, B-012 and B-026. Covering only C-009's corner symbol
removes its type evidence even though the central symbol and purple artwork
remain visible. Automated
tests check title/body exclusions, blank/noisy/circular shapes, persistence and
UI treatment without fabricated catalogue identity. Public small card-reference
images include both recognized and rejected symbols; low-resolution images can
lose enough detail to remain unresolved. No physical rescanning is needed.

Saved-image tests are opt-in through `SCANNER_ENERGY_SYMBOL_IMAGES` (JSON array
of image paths), `SCANNER_ENERGY_RECOGNITION_IMAGE` (the saved Psychic front) and
`SCANNER_ENERGY_KNOWN_IMAGES` (JSON array of `{name,path}` known Energy fixtures).
Private scans are not committed and are never uploaded for analysis.

## Reference provenance and regeneration

The 24×24 silhouette masks are derived from the TCG Energy SVG glyphs in
[xero/pokemon](https://github.com/xero/pokemon/tree/26073226f1f569ce013f4fad0d0bb45a2d3fe118/assets/glyphs),
whose [source notes](https://github.com/xero/pokemon/blob/26073226f1f569ce013f4fad0d0bb45a2d3fe118/README.md)
credit Poképédia. Pokémon names and symbols belong to their respective owners.
The masks are recognition references, not full card images or finish references.

Save `tcg-{grass,fire,water,lightning,psychic,fighting,darkness,metal,fairy}.svg`
from that pinned revision into a development directory, then run:

```sh
swift Scripts/generate-energy-symbol-templates.swift /path/to/glyphs > /tmp/EnergySymbolTemplates.swift
```

Review the generated file before replacing the compiled templates. Rendering is
development tooling only; production uses Swift/Core Image in process and does
not download glyphs or run helper programs. Public card-image checks used the
[TCGdex Sun & Moon set](https://api.tcgdex.net/v2/en/sets/sm1); these are test inputs,
not shipped assets or proofs of finish.
