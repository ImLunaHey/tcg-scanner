# Card-test readiness — 2026-10-31

Card feeding has **not** been tested with this scanner. During web development the
user reported removing a roller after watching a video and intends to scan **bare
cards**. The video and exact removed assembly have not been identified. No removal
of a plastic part was confirmed. Treat the feeder as modified and pause physical
tests until its configuration is established. Browser scan tests use mocks only.

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

1. Confirm the feeder configuration and the precise expendable bare card to test.
   Record its dimensions, condition and thickness where measurable. Photograph
   both sides before feeding. Avoid valuable, damaged, curled or unusually thick
   cards during validation.
2. Add and validate card dimensions using the ICA APIs exposed by this driver.
   Current production capture is A4 only; a profile named Pokémon does not change
   that limitation. Preserve lossless PNG without automatic processing.
3. Ask the user to approve the specific physical test and confirm the single card
   is loaded. Do not reuse earlier paper-loading confirmation as card approval.
4. Scan once, then inspect both card surfaces and edges for marks, dents, scratches,
   curling and feeding problems. Record image dimensions, orientation and order.
5. Only after that passes, agree a small expendable numbered batch to verify
   separation, order and front/back correspondence. Uncertain or incomplete pairs
   must be flagged rather than silently assigned card identities.
6. Enable `C-001-front.png`/`C-001-back.png` only after the pairing rule is verified.
   Keep original bytes and permanent B/C labels. High-volume use follows sustained
   feed, error-recovery and damage checks.

The web dashboard is not a prerequisite for safe physical card validation; the
remaining blockers are feeder configuration, card-sized capture and controlled
hardware tests. Inventory recognition, pricing and selling integrations remain
outside this milestone.
