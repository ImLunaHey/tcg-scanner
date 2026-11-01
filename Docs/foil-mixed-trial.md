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

Physical card-by-card comparison is recorded below. The candidate and threshold
were not adjusted using this batch. No further scan was needed.

The primary check found nine stable foil signals and ten cards with no signal,
with no disagreements across variants. That matches the reported count of
ordinary holos; the count alone did not prove they were the correct nine cards.
The later per-card physical check confirmed that they were. The existing
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
from absence of a border signal.

The user subsequently confirmed the complete breakdown from the physical cards:

- C-001–C-009: holo (9).
- C-010–C-011 and C-013–C-018: normal (8).
- C-012 and C-019: reverse holo (2).

## Scoring against the frozen predictions

Prediction and measurement hashes were verified unchanged before scoring.
`physical-labels.json` and `physical-scoring.json` in the diagnostic directory
record the labels and per-card results separately from the original predictions.
No thresholds or saved scanner finish values were changed.

| Physical finish | Cards | Stable border signal | Existing definite correct finish | Existing unresolved finish |
| --- | ---: | ---: | ---: | ---: |
| Holo | 9 | 9 | 5 | 4 |
| Normal | 8 | 0 | 7 | 1 |
| Reverse holo | 2 | 0 | 2 | 0 |

The border check detected 9/9 ordinary holos (100% on this batch) with zero
false signals on eight normals. Both reverse holos lacked the signal. If treated
as a detector of **any foil**, its recall is 9/11 (81.8%): nine true positives,
two false negatives, zero false positives and eight true negatives. These binary
counts treat absence as a negative solely for scoring; the application must not
translate that absence into a confirmed normal label. No variant was unstable.

The existing set-code/catalogue estimator issued fourteen definite finish
estimates, all correct: five holo, seven normal and two reverse holo. Coverage
was 14/19 (73.7%), with five unresolved cards. Its reverse-holo result was 2/2
correct. C-010 remains outside its supported layout after the catalogue retry.
Correctness among fourteen issued estimates is not overall 100% classification.

Retrospectively, border evidence could resolve the four ordinary holos left
ambiguous by the existing estimator, giving eighteen supported finish estimates;
the energy card remains unresolved. **No combined decision rule was frozen or
tested**, so 18/19 must not be advertised as validated pipeline accuracy. A future
integration must preserve unsupported/uncertain outcomes, use the reverse-holo
signal independently, and be checked on further fresh cards. This one small batch
supports complementary signals, not a general accuracy guarantee.

## Subsequent implementation and service recheck

The user then requested the combined detector. Its native implementation and
decision rules are documented in [combined finish estimates](combined-finish.md).
The deployed service rechecked the existing B-026 prepared fronts and stored
nine holo, seven normal and two reverse-holo estimates. Fire Energy remains
unsupported. All eighteen issued estimates match the physical labels. The original
blind results above are retained; this later implementation used the trial findings
and is a regression/recheck, not an independent blind accuracy result.
