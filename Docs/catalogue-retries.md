# Cards awaiting catalogue data

`awaitingCatalogue` means local OCR read one English set code, one collector
fraction and a title, but identification returned no catalogue candidate. The
`evidence.uncataloguedCard` fields are transcriptions, not verified catalogue IDs.
They must not be used to invent prices, finishes or Store catalogue matches.
Conflicting or incomplete readings remain `noMatch`. DLR is labelled Delta Reign
from the owner's confirmation; other codes remain verbatim until labelled.

The service persists `catalogueRetryAfter` on each pending identification, 24
hours after the attempt. Its independent 30-second maintenance timer starts at
most one due identification when scanning, editing and recognition are idle.
The recognizer refreshes its 24-hour catalogue cache on the next lookup. Pending
cards therefore retry daily while the service is running, including after a
restart or scanner disconnection. A successful match clears the deadline and
normal identity/price handling applies. Failed background attempts retain a
future deadline; confirmed and rejected records are never retried automatically.
Original and prepared images are reused without feeding hardware.
