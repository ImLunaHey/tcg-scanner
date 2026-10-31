# Card-test readiness — 2026-10-31

One explicitly authorized expendable bare card was scanned successfully in B-007.
The user reported clean feeding and no visible new damage. B-008 subsequently
retained 18 images before cancellation: the user later clarified that 11 cards
were loaded, all 11 reached the output tray, and reported no visible new damage.
Two cards are unaccounted for in the saved batch. See
[physical test results](card-preparation.md). This is not complete batch-count,
order or bulk-feeding validation.

The user reports removing the “main roller”; the video and precise assembly remain
unidentified. No removal of a plastic part was confirmed. The feeder remains
modified. The user subsequently authorized a controlled single-card test despite
that uncertainty, followed by exactly three expendable cards for a batch test.
Browser scan tests use mocks only. Do not infer permission for further hardware
tests or changed transport-safety settings from these results.

## Evidence

Ricoh specifies paper weights of 20–465 g/m², with a narrower range for A8. Its
plastic-card guidance concerns ISO7810 ID-1 PVC/PVCA cards up to 1.4 mm thick;
that does not certify trading cards. Coated media is listed among documents that
may feed poorly. The guide calls for testing unfamiliar paper types and correctly
installed brake/pick rollers. See the [Ricoh operator's guide](https://www.pfu-us.ricoh.com/-/media/project/scanners/files/products/manuals/r_operators_guide_fi_8000_series.pdf),
printed pages 116–120 and 237–240.

Third-party vendor advice varies by setup:

- [Kronozio](https://kronozio.freshdesk.com/support/solutions/articles/73000571240-scan-cards-using-the-fujitsu-ricoh-fi-8170-raw-sleeves-and-toploaders-)
  describes the normal setup for bare cards and brake-roller removal as a workaround
  for crumpling sleeves. Its PaperStream/TWAIN profiles and multifeed changes are
  Windows-specific and must not be assumed available or appropriate on macOS.
- [Card Dealer Pro](https://carddealerpro.zendesk.com/hc/en-us/articles/9624604145563-Modifying-the-Ricoh-8170-Before-Scanning-Sports-Cards-and-Trading-Cards)
  discusses roller removal for sleeved/top-loaded cards and tape around feeder
  arms in response to reported indentations. This is third-party modification
  advice, not a Ricoh requirement for bare trading cards.

Recommendation given to the user: if the removed part was the brake roller,
restore the normal assembly before a bare-card test. Identify the actual part from
the video first; do not infer it from the word “roller” or improvise disassembly.
No modifications, multifeed disabling or safety-setting changes are authorized
by this document.

## Next focused milestone

1. Reconcile B-008's 11 loaded cards against nine captured fronts and nine apparent
   backs. The test had been authorized as three cards; cancellation was requested
   when the image count exceeded six. Do not silently mark it completed or retry
   feeding. All 11 reached the output tray; pending driver image delivery at
   cancellation is possible but unverified.
2. Confirm the exact feeder modification before broader validation. A no-damage
   report for this test is not a general guarantee for all card stock.
3. Current capture remains full A4. Software whole-card preparation works on
   saved scans, but one edge is flush with the scan boundary. Investigate supported
   capture geometry without silently changing transport or multifeed settings.
4. Establish physical order and front/back correspondence with an agreed numbered
   test. The observed back/front alternation alone does not establish which
   indistinguishable back belongs to which front after an interrupted feed.
5. Enable `C-001-front.png`/`C-001-back.png` only after the pairing rule is verified.
   Keep original bytes and permanent B/C labels. High-volume use follows sustained
   feed, error-recovery and damage checks.

Card preparation and English OCR run independently on saved images and require no
additional feeder action. Finish and condition remain manual assessments.
