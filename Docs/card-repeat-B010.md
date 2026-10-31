# B-010 repeat scan — 2026-10-31

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
