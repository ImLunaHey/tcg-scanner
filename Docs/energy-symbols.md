# Energy type from the corner symbol

## Portable Rust implementation

`scanner_recognition::symbols::energy` now owns the same bounded ink-shape
algorithm in Rust. Its compiled reference masks are byte-for-byte transcriptions
of the existing glyph and printed-symbol references described below. The service
combines whole-card body text with regional title OCR before accepting an Energy
heading, so regional cropping cannot hide Special Energy rules. Unnumbered
Energy results return before any catalogue access; numbered cards still require
the ordinary printing checks.

On 10 November 2026, the Rust pixel detector reproduced all nine saved Energy
fixtures (two Psychic, one Metal, six Fire). Covering the small corner removed
the evidence in all nine cases. This tests saved-image shape parity, not accuracy
across every Energy printing. Run the opt-in Rust test with
`SCANNER_ENERGY_FIXTURES` pointing to a private JSON array of
`{name,path,lines,turns,id}` records; no private scans are committed or uploaded.

The fresh OCR test accepts `SCANNER_ENERGY_RECOGNITION_IMAGE`,
`SCANNER_TESSERACT_LIBRARY` and `SCANNER_TESSERACT_DATA`, rotates the saved Psychic
front four ways and substitutes a catalogue transport that fails on any request.
All four rotations passed with Tesseract 5.5.1 on 10 November 2026. A bounded,
downsampled title retry resolves the stylized heading when whole-card OCR is weak;
the same 0.7 heading threshold remains. Printed identifiers require readable
footer text, excluding low-confidence digit artifacts in the central artwork.

## Original detector and evidence contract

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

Version `energy-corner-shape-v2` checks two stroke-grouping scales, retaining
only original ink in the scored mask. This handles Metal's
disconnected triangle and outer pieces. Separable dilation avoids scanning a
large neighbourhood for every pixel. Low ink thresholds distinguish those marks
from the shaded badge rim without relaxing the shape-score requirement.

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

B-046 C-009 passes as Metal. Cached TCGdex XY references `xy1-132` through
`xy1-140` pass for all nine basic types. This is a reference coverage check, not
a blind accuracy percentage: the shaded Darkness template uses `xy1-138` itself.
Some other low-resolution Sun & Moon references remain unresolved; we do not
claim all Energy printings have been physically validated. Special Energy names
and effects still require catalogue identification and are not inferred from a
basic type symbol.

Saved-image tests are opt-in through `SCANNER_ENERGY_SYMBOL_IMAGES` (JSON array
of image paths), `SCANNER_ENERGY_RECOGNITION_IMAGE` (the saved Psychic front) and
`SCANNER_ENERGY_KNOWN_IMAGES` (JSON array of `{name,path}` known Energy fixtures).
Private scans are not committed and are never uploaded for analysis.
Set `SCANNER_CHECK_TCGDEX_ENERGIES=1` to run the opt-in reference check. It uses
the existing bounded TCGdex image cache in `~/Library/Caches/Scanner/card-references`,
so downloaded public references are reused. `SCANNER_ENERGY_RECOGNITION_TYPE`
selects the expected type for the four-orientation saved-image test.

## Reference provenance and regeneration

The 24×24 silhouette masks are derived from the TCG Energy SVG glyphs in
[xero/pokemon](https://github.com/xero/pokemon/tree/26073226f1f569ce013f4fad0d0bb45a2d3fe118/assets/glyphs),
whose [source notes](https://github.com/xero/pokemon/blob/26073226f1f569ce013f4fad0d0bb45a2d3fe118/README.md)
credit Poképédia. Pokémon names and symbols belong to their respective owners.
The masks are recognition references, not full card images or finish references.

`EnergyPrintedSymbolTemplates` additionally includes the shaded Darkness corner
from [TCGdex xy1-138](https://api.tcgdex.net/v2/en/cards/xy1-138). Its source is
`https://assets.tcgdex.net/en/xy/xy1/138/high.png` (600×825 pixels). The recorded
crop `(521,41,34,30)` uses top-left coordinates; neutral pixels below 65 are
normalized to 24×24. This supplements the flat glyph with a printed shading
variant; candidate scores are aggregated by Energy type before checking the
margin to the next type.

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
