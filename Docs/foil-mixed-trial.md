# B-026 mixed-finish blind trial

Batch ID: `952666E0-7253-4BC4-B43C-52D89D9E9AFA`.
The user loaded 19 cards, reporting nine holo, nine non-holo and one reverse
holo. Individual physical labels were not supplied before prediction. After
predictions were recorded, the user corrected the reverse-holo count: Pangoro
and Larvesta are both reverse holo. The frozen initial totals were not changed.

## Frozen protocol

Recorded before job submission at `2026-11-01T20:17:00.942037+00:00`:

- 600 DPI, duplex PNG, overscan and automatic card preparation.
- Primary feature: `borderMinimumBand`, threshold `0.008213026997467985`.
- Require agreement across baseline, narrower border strips and 630-pixel variants.
- All above: foil signal; all below: no foil evidence; disagreement: unstable.
- No foil evidence does not establish non-holo. No subtype is assigned from the border signal.
- Record the existing set-code/catalogue estimate separately; do not invent a combined rule after seeing the result.
- Never adjust predictions to match the reported 9/9/1 totals.

The image measurement source SHA-256 is `96876f4b44de068ef4d68484ec02a09715ae81596a2ed8ea5146dd92063f9c33`;
the fitted evaluation SHA-256 is `4e88d842dbab74cbe0c7fb5f1da127609d070779eb6fc84345588a031f1d2e4d`. Both are checked before
measurement. The offline predictor accepts only unlabelled records and cannot
compute accuracy without a subsequent physical-label comparison.

## Capture

The driver reported 600 DPI, RGB 8-bit, black backing, cropping value 4, duplex,
PNG and blank-page skipping disabled. Multifeed was unchanged. All 38 originals
were captured in `/Volumes/scans/B-026`. All original hashes and selected
prepared-front hashes were verified. The user confirmed all 19 fed cleanly with
no new damage. All 38 prepared images passed software upright checks and every
card had a verified front/back pair. Automatic processing completed without job
errors; C-010's front identification separately failed on a catalogue response.

Diagnostic evidence is retained under ignored
`Diagnostics/foil-experiment/mixed-trial/`. Production finish labels, profile
defaults and Store staging are not changed by the experiment.

## Recorded predictions before physical per-card labels

Recorded at `2026-11-01T20:20:58.842760+00:00`. Prediction JSON SHA-256:
`1ae556f5b8577cc9a05bae21b7bdd3fe0f429b86d500e4836444ec73507c1628`.

The table preserves both independent results. No accuracy or subtype detection
rate is inferred from the aggregate 9/9/1 totals. Candidate names remain OCR/catalogue
suggestions, not reviewed identities.

| Card | Candidate name(s) | Border signal | Existing set-code/catalogue estimate |
| --- | --- | --- | --- |
| C-001 | Goodra | Foil signal | multipleRegularFinishes |
| C-002 | Spiritomb | Foil signal | multipleRegularFinishes |
| C-003 | Moltres | Foil signal | multipleRegularFinishes |
| C-004 | Mismagius | Foil signal | multipleRegularFinishes |
| C-005 | Team Rocket's Exeggutor | Foil signal | holo |
| C-006 | Pecharunt | Foil signal | holo |
| C-007 | Roaring Moon | Foil signal | holo |
| C-008 | Entei | Foil signal | holo |
| C-009 | Iron Crown | Foil signal | holo |
| C-010 | Unidentified | No foil signal | Unavailable |
| C-011 | Virizion | No foil signal | normal |
| C-012 | Pangoro | No foil signal | reverseHolo |
| C-013 | Iris's Fighting Spirit | No foil signal | normal |
| C-014 | Lokix | No foil signal | normal |
| C-015 | Cubone | No foil signal | normal |
| C-016 | Lechonk | No foil signal | normal |
| C-017 | Impidimp | No foil signal | normal |
| C-018 | Dunsparce | No foil signal | normal |
| C-019 | Larvesta | No foil signal | reverseHolo |

Physical card-by-card comparison is pending. The candidate and threshold have
not been adjusted using this batch. No new scan is needed for physical checking.

The primary check found nine stable foil signals and ten cards with no signal,
with no disagreements across variants. That matches the reported count of
ordinary holos, but does not prove they are the correct nine cards. The existing
set-code check suggested two reverse holos, conflicting with the original user
total. Subsequent physical confirmation resolved that discrepancy in favour of
the predictions: the original total was mistaken. No card was relabelled to
make the initial totals fit.

## Catalogue retry after prediction recording

C-010's failed catalogue lookup was retried without rescanning or changing the
prepared pixels. It recovered a single candidate, Fire Energy (`sve-010`), at
2026-11-01T20:21:34Z. The existing set-code estimator reports this energy layout
as unsupported. The original blind table and prediction hash above are retained
unchanged; its border result remains no foil evidence. All nineteen fronts now
have recognition candidates, still requiring identity review.

## Physical confirmation received after prediction recording

The user confirmed C-012 Pangoro and C-019 Larvesta are both reverse holo. Both
existing set-code estimates were therefore correct on these two examples.
Neither produced the experimental border signal in any variant. This is direct
evidence that the border check alone misses at least these reverse holos; it
must not be described as a universal foil detector or used to establish non-holo
from absence of a border signal. Ordinary-holo and remaining normal per-card
labels are still awaiting confirmation; overall accuracy is not yet scored.
