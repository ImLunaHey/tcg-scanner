# B-010 repeat scan — 2026-10-31

> Historical implementation/validation notes. Swift/React source and deployment
> commands below are retired. Use [the Rust application](../README.md) and
> [portable installation](portable-install.md) for the current project.

The user loaded the same 11 cards and authorized one repeat scan. Job
`C8764C18-F221-4707-B7CD-6AE384F4D1FD` used the existing fi-8170 configuration:
300 DPI, RGB, duplex PNG, A4, card preparation enabled, blank-page skipping off,
black-background capture, and multifeed unchanged. No feeder settings were altered.

Capture completed with 22 original PNGs and 22 cropped/oriented PNG copies at
`/Volumes/scans/B-010`, no reported job errors, and the scanner session closed.
The user confirmed all 11 cards fed cleanly, with no jams, overlapping feeds or
new damage. This observation applies to this batch, not a general media-safety
claim.

## Repeatability result

The installed version at capture time did not automatically start OCR. Explicit
identification was then run on the eleven front captures using saved request IDs.
Ten produced one suggested catalogue match, each already present in B-009. None
was automatically confirmed. The reload changed some card positions:

| B-010 card | Suggested identity | Catalogue ID | B-009 card |
| --- | --- | --- | --- |
| C-001 | Fire Energy | sve-010 | C-001 |
| C-002 | No match; image reads Larvesta | — | C-002 |
| C-003 | Virizion | sv09-015 | C-004 |
| C-004 | Pangoro | sv09-099 | C-003 |
| C-005 | Iris's Fighting Spirit | sv09-149 | C-005 |
| C-006 | Lokix | sv09-100 | C-007 |
| C-007 | Cubone | sv09-076 | C-008 |
| C-008 | Salamence ex | sv09-177 | C-006 |
| C-009 | Lechonk | sv09-139 | C-009 |
| C-010 | Impidimp | sv09-071 | C-010 |
| C-011 | Dunsparce | sv09-120 | C-011 |

Larvesta's OCR correctly read the name but returned `028/1590` for the printed
collector number and set total. The catalogue's official total is 159, so the
matcher rejected that evidence. No extra digit was silently discarded, and no
identity was copied from the prior batch. This repeat therefore did **not**
reproduce all eleven matches. The remaining OCR error needs separate validation.

## Automatic workflow change

After this run, card preparation was extended to identify each saved image during
post-capture processing. The UI distinguishes waiting, identifying, suggestions,
no match and failure. General-document scans with card preparation off do not
start OCR. Both sides are analyzed without assuming which capture is the front.
Matching remains local Apple Vision plus TCGdex catalogue GETs; images and OCR
text are not uploaded. Finish and condition remain manual.

Mock-backend tests cover automatic success and source provenance, fallback to an
original when cropping fails, processing progress, cancellation, rejecting a
competing manual operation, multiple failed identifications without losing a
batch, and the disabled option. The focused Swift suite passed 22 tests (two
opt-in image fixtures skipped); the web build and eight unit tests passed.
The automatic trigger was tested with mocks; B-010's physical scan preceded the
change. No additional physical scan was initiated to validate deployment.

Private request/results, OCR evidence, full job records, comparison JSON and user
feeding confirmation are retained under ignored
`Diagnostics/identification-repeat/`. Original scans remain on the SMB volume.

## Deployment checks

The signed service and HTTP/web update were installed while no job was active.
The existing native GUI was left running; its label change is in the built source
and will appear after its next app update. All clients already use the updated
service's automatic processing behavior.

The SMB mount disappeared during deployment. Reopening `smb://luna@lake/scans`
restored it; launchd then restarted the service without a local-storage fallback.
All 22 original and 22 prepared B-010 files passed byte-count and SHA-256 checks,
and the restarted service successfully generated a JPEG preview from the share.

A read-only Chromium check through the existing Tailscale HTTPS address verified
11 paired cards, ten displayed suggestions, Larvesta's no-match state, and the
updated scan option. A browser-only replay verified waiting → identifying →
suggestion transitions while Processing. That replay made no server writes or
hardware requests; job history was unchanged. Screenshots and assertions are in
the private diagnostics directory above.

## Follow-up: focused footer OCR and lightbox transfer fix

A later saved-image retry fixed Larvesta without feeding any cards. Vision rereads
only the region containing the printed collector number when the initial
name/number candidates fail catalogue validation. It reads `028/159•`, preserving
the rarity dot as punctuation. Region-relative boxes are converted back to
whole-card coordinates. Candidate IDs must agree with the initial ranking and
the independently reread set total; no trailing digit is removed to fit a set.

All 11 B-010 fronts passed a read-only recognition regression against the expected
printings. The new signed service then re-identified only C-002/front capture 4.
B-010 now has the same eleven unique suggested catalogue IDs as B-009, with the
changed order documented above. These remain suggestions requiring review.
The previous no-match result and new request/result are retained in diagnostics.

The lightbox delay included quadratic scanning in the Swift Unix-socket line
reader: each 4 KiB receive searched the whole accumulated image reply again. The
reader now searches only the newly received bytes, retaining framing limits and
trailing-data checks. A bounded browser cache also retains full PNG blobs, and
the lightbox displays the existing preview while waiting for the full original
resolution. PNG bytes are not recompressed or replaced by JPEGs for inspection.

Four uncached HTTPS requests for two B-010 cropped PNGs (1.46–1.55 MB) on this Mac
measured 3.248, 8.662, 4.153 and 2.625 seconds before, versus 0.185, 0.149, 0.124
and 0.121 seconds after the transport update. These are small local measurements,
not a guarantee of another device's network speed. A Chromium browser test held
the full-image response to verify the cached preview appeared first, checked the
full PNG's SHA-256 in the browser, verified native-pixel 100% zoom, and reopened
it without another image request. The preview appeared in 51 ms and reopening
took 62 ms in that check. Mobile layout was checked at 390 × 844.

Socket/HTTP/client tests passed, including large frames crossing 4 KiB boundaries,
limits, authentication and image integrity; recognition tests included the actual
B-010 Larvesta and all eleven fronts. Diagnostic timing samples and browser checks
are retained under `Diagnostics/identification-repeat/`.
The updated paired-card browser workflow passed on desktop Chromium, mobile
Chromium and tablet WebKit (three tests), including automatic identification,
full-resolution loading, zero-request reopening, themes and reload caching.
A live HTTPS browser check confirmed B-010 now displays eleven suggestions,
including Larvesta, and created no scan jobs.
