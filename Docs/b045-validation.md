# B-045 capture and automatic processing check

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

Validated on 6 November 2026 against saved fi-8170 captures. Batch ID:
`DFD89430-E806-4ABC-A7EA-4EAAEAFC086E`.

## Capture

- 23 physical cards, 46 original PNGs, 600 DPI duplex with overscan and black background.
- The user confirmed that all cards fed through successfully.
- Completed without recorded errors. All 46 original file SHA-256 hashes verified;
  subsequent preparation left the original records and images unchanged.
- Initial job ran from 22:28:55 to 22:38:50 UTC, before the parallel processing update.

## Results

- 18 playable cards have suggested catalogue matches; they remain review-required.
- C-009 and C-019 are Energy cards without an exact catalogue match. Automatic
  heading/footer text evidence now verifies their upright orientation and rotates
  both by 180 degrees. This is a software correction, not a manual rotation.
- C-016, C-020 and C-021 now classify as Pokémon online/code cards. Both sides
  contain qualifying public redemption wording. Six original captures passed
  the complete crop/orientation/classification test; deployed service reprocessing
  persisted all three classifications. Redemption strings were not logged or sent
  to a catalogue service.
- Eleven front-side rarity symbols were accepted: nine circles and two diamonds.
- Back-border checks flagged five candidate marks across C-001, C-011 and C-012.
  These are review candidates, not confirmed physical damage or condition grades.
- Finish estimates remain unsupported for the 18 matched older fronts and
  uncertain for the two Energy cards. This change does not improve foil detection.

## Processing changes and validation

Automatic preparation now uses two concurrent card-pair workers, retains capture
ordering, skips catalogue/OCR/finish work on recognized Pokémon backs, avoids
repeating finish analysis, and coalesces shared catalogue downloads. The UI
distinguishes receiving images from processing and shows known capture totals,
unique prepared images and checked cards, including classified code cards.

The driver may continue transferring images after physical feeding stops. No new
physical batch was run after this update, so an end-to-end speed improvement has
not yet been measured.

Validation: 241 Swift tests passed with `--no-parallel`; 45 web tests passed.
Separate opt-in saved-image tests exercised the raw six-sided code-card fixture,
the low-text Energy images and automatic pair routing. Signed service, web and
native application updates were installed. Originals and redemption codes remain
local; private fixtures are not committed.
