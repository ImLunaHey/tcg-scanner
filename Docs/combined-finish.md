# Combined automatic finish estimates

`modern-set-code-and-border-v5` runs inside ScannerService using native Swift,
ImageIO and CoreGraphics. Both new-scan identification and explicit finish refresh
use the same implementation. It does not invoke the offline experiment scripts,
upload images, use physical test labels, or write reviewed finish/condition fields.

## Decision rules

The existing modern English set-code/catalogue scope is retained: one printing
with a readable collector number, a supported modern set, a number within the
ordinary set count, and a listed reverse-holo variant. Energy, older/promotional
layouts and unresolved identities continue to abstain when outside that scope.

- A clear light set-code box keeps its reverse-holo estimate independently of
  border texture. Missing texture does not contradict reverse holo.
- A dark box indicates a regular printing. For a prepared, upright Pokémon card,
  stable brightness texture on both borders can resolve normal-or-holo to holo
  when the catalogue lists holo.
- Positive texture conflicting with a catalogue that lists no regular holo
  returns uncertain with `conflictingFinishEvidence`; neither source silently wins.
- Absent, unstable, unsupported or unavailable border evidence never establishes
  normal. Existing independent marker/catalogue inferences remain; an ambiguous
  normal-or-holo result stays ambiguous without stable positive texture.
- Missing or conflicting set-code evidence cannot be overridden by texture.

Only the exact identification source edit's verified `text` preparation enables
the border check. Originals, unverified preparation and detected backs cannot
enable it. The image must also fit the bounded, portrait card geometry. Aspect
ratio alone does not prove the image contains a card. These gates target ordinary
modern layouts, not a guarantee for every full-art or unusual printing.

## Measurement and API evidence

`BorderFoilAnalyzer` ports the frozen `borderMinimumBand` measurement exactly:
1260-pixel analysis, x=1.5–4% / 96–98.5%, y=12–86%; narrower strips at
2–3.5% / 96.5–98%; and a 630-pixel variant. Row brightness residuals use a
height-scaled smoothing window. The lower of the two border energies must exceed
`0.008213026997467985` in all three variants to count as present. Disagreement is
unstable. No threshold was tuned against B-026 labels.

Inputs are bounded to one frame, 32 MiB, 40 megapixels and 20,000 pixels per axis;
analysis decodes bounded thumbnails and honours cancellation. Integer quarter-turn
orientation avoids additional resampling. `finishEstimate.borderEvidence` stores
the method, state, threshold and three energies when the border check runs.
The existing suggested-finish/state fields drive all clients and pricing. Store
records are updated only through an explicit send/resync.

This remains an estimate, not measured reflectivity or a calibrated probability.
The calibration used a small collection of modern cards; printed patterns can
mimic foil. An absent signal cannot rule out either ordinary or reverse foil.
The weakest earlier positive was close to the threshold. Other layouts, feeding
conditions and scanner settings require further validation.

## Validation

- Native measurements matched all 116 saved fronts (348 variant measurements)
  to the offline experiment within `1e-12`, including B-026.
- Synthetic checks cover phase-independent bands, one-sided features, blank
  borders, rotations, cancellation, bounds and variant disagreement.
- Combination tests cover reverse-holo independence, marker conflicts, absent
  texture, catalogue conflicts and Codable evidence round trips.
- Saved-image checks cover B-014, B-015, B-016, B-024, B-025 and B-026, with no
  false holo/reverse estimates introduced on the known-normal controls.
- B-026 offline integration produced nine holo, seven normal and two reverse-holo
  estimates, all matching the user's physical labels. Fire Energy remained
  unsupported: 18/19 resolved, not 19/19 automatically classified.
- The full Swift suite passed 158 tests with saved-image integration enabled;
  all 28 web tests passed. Service tests retain reviewed finish/condition values.

The combined rule was designed after inspecting B-026, so these results are a
regression/recheck, not an independent accuracy benchmark. The earlier blind
predictions remain unchanged in [the trial record](foil-mixed-trial.md).

```sh
SCANNER_BORDER_FIXTURES=Diagnostics/foil-experiment/all-texture-features.json swift test --filter borderTextureMatchesFrozenExperimentalMeasurements
SCANNER_COMBINED_FINISH_RECORDS=Diagnostics/combined-finish/b026-before.json swift test --filter combinedFinishSavedBatchesPreserveUncertaintyAndMatchPhysicalChecks
```

These optional tests only read saved image fixtures. Normal tests never require
the scanner or start hardware.

## Signed deployment and live B-026 recheck — 2026-11-01

The release build passed. The idle service, CLI/MCP clients, native app and HTTP
gateway were installed with the existing certificate-backed identities; web assets
were updated. The old service exited before SQLite backup and binary replacement.
No scans, identification operations or Store sends were active during replacement.
The installed service and native app passed strict code-signature verification.

All nineteen B-026 fronts were refreshed through `v1.images.estimateFinish`.
The saved result is nine holo, seven normal, two reverse holo and one unsupported
Fire Energy. The four formerly ambiguous holos (Goodra, Spiritomb, Moltres and
Mismagius) now resolve to holo. All eighteen issued estimates match the physical
labels; coverage is 18/19 (94.7%), with no claim of general 100% accuracy.

Verified that every non-estimate job/identification field remained identical,
including source edits, reviewed finish/condition and timestamps. All 38 original
files still matched their saved SHA-256 values. No images were regenerated and
no Store transfer was initiated. The SMB manifest matches the live identification
records. Authenticated HTTPS returned all nineteen v5 estimates, and the clean
batch URL returned HTTP 200. Evidence is retained in ignored
`Diagnostics/combined-finish/`.

The subsequent [saved-library refresh](finish-library-validation.md) applied this
version to every eligible saved card identity and records remaining uncertainty,
unsupported Energy/ex layouts and original-file integrity checks.
