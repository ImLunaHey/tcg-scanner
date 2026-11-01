# Energy and Pokémon ex finish scope

Research checked 2026-11-01. All image analysis remains local. Catalogue requests
download public metadata; no user images are uploaded. Physical review values are
not detector inputs and are never replaced by these estimates.

## Modern Pokémon ex

All five saved unsupported Salamence ex captures identify Journey Together
177/159 (`sv09-177`). They were excluded from the ordinary marker rule because
the number exceeds the printed set total and the catalogue lists no reverse-holo
variant. Neither condition prevents a separate catalogue inference.

[TCGdex's exact printing](https://api.tcgdex.net/v2/en/cards/sv09-177) lists Ultra
Rare, holo true, normal/reverse false, and a standard holo detailed variant.
[Pokémon's Scarlet & Violet design guide](https://www.pokemon.com/uk/news/pokemon-tcg-scarlet-and-violet-revamps-pokemon-tcg-card-aesthetic)
describes modern double-rare ex foil borders and full-art ultra-rare ex foil.
Together these support Holo as an estimate for this printing, not a measurement
of the scanned surface or proof of authenticity.

`modern-ex-catalogue-v1` requires a unique candidate with matching collector-number
evidence and ID, a numeric `sv` expansion, a positive numeric local ID and positive
official set total, literal ` ex` suffix, and exactly one catalogue finish: holo.
The category field is unnecessary for this identity rule because the recognizer
already exclusively uses the English Pokémon catalogue. Ordinary marker/border
rules and their thresholds are unchanged.

This is not an unrestricted “ex means holo” rule. Pokémon's
[POP Series 4 reference](https://assets.pokemon.com/assets/cms/pdf/tcg/carddex/POP_Series_4_Promo_Cards.pdf)
includes non-foil Deoxys ex. Older/promotional expansions are excluded.
[TCGdex notes that variant data is being improved](https://tcgdex.dev/faq#how-do-normal--reverse--holo-variants-work);
wrong identification or incomplete metadata can invalidate a catalogue estimate.

## Basic Energy

Fire Energy `sve-010` is recognized successfully, but its finish is unsupported.
The specific explanation now states that Basic Energy foil variants need
validation, rather than treating this as an unidentified card or missing OCR.

Pokémon's design guide describes the inverted expansion-code box for parallel
foil cards and the addition of expansion codes to Basic Energy. It does not
guarantee that every special Basic Energy foil treatment follows the same rule.

The [SVE 002](https://api.tcgdex.net/v2/en/cards/sve-002) and
[SVE 010](https://api.tcgdex.net/v2/en/cards/sve-010) catalogue entries each list
normal and reverse true, holo false. Detailed variants include special cosmos,
Poké Ball, cracked-ice and/or stamped foil treatments. A false coarse holo flag
therefore cannot prove that a particular Energy card is non-foil. Pokémon's
[Terapagos collection](https://www.pokemon.com/uk/pokemon-tcg/product-gallery/terapagos-ex-ultra-premium-collection)
also explicitly includes crystalline foil Basic Energy.

The six saved Fire Energy captures provide repeated observations of a printing;
they are not six independent foil/normal controls. B-026/C-010 is physically
confirmed normal. The user does not currently have a known foil SVE 010 control.
No Energy inference is enabled from this one-sided evidence.

A future validation needs physically checked normal and foil examples of the same
collector number, with foil treatments recorded separately. Freeze predictions
before revealing labels. Test more than the usual parallel foil before asserting
a universal dark-box-to-normal rule. Do not substitute another Energy generation
or the absence of ordinary Pokémon border texture as proof.

## Automated checks

Tests cover ordinary and above-total modern ex numbers, exact ID/number matching,
ambiguous identities, mixed/missing variants, older/promotional sets and Energy
abstention. Catalogue-only estimates leave image-measurement fields absent and
round-trip through the shared API. Optional saved-image integration checks every
saved Salamence printing and existing known-normal/foil controls. It never starts
the scanner or changes physical review fields.
