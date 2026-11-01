# Saved-library finish refresh — 2026-11-01

The user requested the combined finish detector across all saved scans. Inventory
covered 26 job/batch records and 451 original captures. Sixteen batches contained
identified card fronts eligible for `v1.images.estimateFinish`; paper-only test
batches and the failed zero-image batch had no eligible card identities.
The partial/cancelled B-008 batch was included for its saved images.

All 216 existing eligible identities were refreshed successfully through the
running service, serially, using `modern-set-code-and-border-v5`. No hardware
scan was started. One old unmatched B-008/C-001 front clearly contained an SVE
Energy card, so its unreviewed recognition was retried through `v1.images.identify`.
It now identifies Fire Energy `sve-010` and correctly reports unsupported finish.
There are 217 currently identified card fronts across these batches; this counts
repeat scans, not unique physical cards.

## Results

| Finish result | Saved fronts |
| --- | ---: |
| Non-holo | 149 |
| Holo | 39 |
| Reverse holo | 11 |
| Uncertain | 7 |
| Unsupported | 11 |

Resolved estimates increased from 167 to 199, including 32 newly resolved
identities. Forty-one existing records changed decision state or suggested
finish; all eligible records received current-version evidence. This measures
coverage, not accuracy: physical labels are unavailable for much of the library.

| Batch | Non-holo | Holo | Reverse holo | Uncertain | Unsupported |
| --- | ---: | ---: | ---: | ---: | ---: |
| B-007 | 1 | 0 | 0 | 0 | 0 |
| B-008 | 5 | 0 | 1 | 1 | 2 |
| B-009 | 7 | 0 | 2 | 0 | 2 |
| B-010 | 6 | 0 | 2 | 1 | 2 |
| B-011 | 7 | 0 | 2 | 0 | 2 |
| B-012 | 7 | 0 | 2 | 0 | 2 |
| B-013 | 4 | 16 | 0 | 0 | 0 |
| B-014 | 15 | 0 | 0 | 0 | 0 |
| B-015 | 14 | 0 | 0 | 0 | 0 |
| B-016 | 19 | 0 | 0 | 3 | 0 |
| B-017 | 19 | 0 | 0 | 0 | 0 |
| B-022 | 19 | 0 | 0 | 0 | 0 |
| B-023 | 3 | 0 | 0 | 0 | 0 |
| B-024 | 0 | 14 | 0 | 2 | 0 |
| B-025 | 16 | 0 | 0 | 0 | 0 |
| B-026 | 7 | 9 | 2 | 0 | 1 |

## Remaining uncertainty and scope

| Card | Candidate(s) | Reason |
| --- | --- | --- |
| B-008/C-003 | Larvesta | markerNotFound |
| B-010/C-005 | Iris's Fighting Spirit | markerContrast |
| B-016/C-010 | Erika's Tangela | multipleRegularFinishes |
| B-016/C-011 | Totodile | multipleRegularFinishes |
| B-016/C-012 | Charmander | multipleRegularFinishes |
| B-024/C-006 | Kilowattrel | ambiguousIdentity |
| B-024/C-011 | Zarude | ambiguousIdentity |

The eleven unsupported records are six Fire Energy and five Salamence ex scans.
No known physical label was substituted for an automatic inference.

Fire Energy is not an OCR failure. B-026/C-010 has a single `sve-010` Energy
candidate, verified upright preparation and a clear `SVEEN` marker. Its catalogue
entry lists both normal and reverse holo. The printed-marker detector deliberately
accepts numbered `sv…`/`me…` sets, excluding the separate `sve` Energy layout;
the border branch separately requires category `Pokemon`. Recognising the card
therefore does not establish its finish. Extending support needs physically
checked normal/reverse SVE Energy examples to validate the marker rule, not a
blanket assumption that Energy is normal or removal of the scope guards.

## Preservation and verification

- All 451 original files matched their recorded byte lengths and SHA-256 hashes,
  with no missing files or paths outside their batch directories.
- Every non-estimate job/identity field remained unchanged during the 216 finish
  refreshes, including source edits and manually reviewed fields.
- The explicit old Energy identification retry is the sole recognition-record
  replacement; its reviewed finish/condition/selection/timestamps were preserved.
- All sixteen updated SMB manifests match the live service identification records.
- Authenticated HTTPS verified B-008's nine v5 estimates and recovered Energy name.
- The service finished idle, with no queued jobs. No Store send/resync occurred.

No detector or client code changed during this refresh. Per-record snapshots,
updates, unresolved reasons and integrity results are retained in ignored
`Diagnostics/finish-library-20261101T203242Z/`. See [combined detector](combined-finish.md)
for implementation tests and decision rules.

## Modern ex scope follow-up

The subsequent [catalogue scope update](finish-catalogue-scope.md) resolves all
five saved Salamence ex captures with an explicitly catalogue-based Holo estimate.
Six Energy captures remain unsupported with a more precise explanation. The
updated library has 204/217 finish coverage (94.0%): 149 normal, 44 holo,
11 reverse-holo, seven uncertain and six unsupported. The original refresh above
is retained as the historical v5 result.
