# Card and batch price guides

Authenticated `GET /api/v1/batches/{job UUID}/valuation` forwards to the Swift
service's `v1.batches.valuation` JSON-RPC method (`{"id":"job UUID"}`). The web UI
shows GBP values at the top right of each card panel and a batch value above the
cards. It refreshes when identity/finish changes and polls while quotes are loading.
No scan, image processing or manual identification is triggered by pricing.

The guide uses [Cardmarket data through TCGdex](https://tcgdex.dev/markets-prices)
and the [ECB daily EUR reference rates](https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml).
The service makes bounded HTTPS GETs to those two fixed hosts. No images, OCR,
condition checks or batch information are uploaded. TCGdex receives catalogue IDs.
No pricing keys or new packages are required.

## Valuation rules

- Use the variant-specific Cardmarket product in `variants_detailed`, with the
  `trend` series for regular normal/holo printings and `trend-holo` for reverse
  printings. Holo rares are regular products; the suffix must not be selected just
  because the physical card is holo. Keep the product ID, EUR amount and source
  update timestamp in every returned quote. Do not substitute low/average prices.
- Standard-size, unstamped printings only. Special subtypes are excluded. Cards
  with first-edition alternatives need edition identification and remain unpriced.
  Missing, zero, negative, excessive, wrong-currency or ambiguous product quotes
  remain unavailable. Never fall back to a regular quote for a missing reverse.
- If detailed variants are absent, root quotes can price one regular finish plus
  its reverse; two competing regular finishes cannot share an assumed root price.
- One unique suggested identity or a confirmed selected identity is required.
  Rejected/ambiguous identities and incomplete or conflicting duplex pairs are
  unpriced. Duplex grouping uses consecutive sequence slots, regardless of which
  side has the identity. Repeated copies count separately; backs never add value.
- A reviewed finish takes precedence over the matching automatic finish estimate.
  Unresolved normal/holo estimates give a range across those two finishes. Other
  unresolved finishes use the available normal/holo/reverse alternatives. Multiple
  foil patterns also give a range. Every possible variant must have a usable quote;
  a partially priced range is unavailable, rather than an artificially narrow range.
- Convert EUR with decimal arithmetic using ECB GBP-per-EUR, round each physical
  card to integer pence, then sum those displayed values. FX and price dates appear
  under the batch's Cardmarket source disclosure. Values are market guides, not
  condition-adjusted proceeds, and exclude fees and postage.

## Contract and caching

Response schema version 1 includes `cards` (number, cardID, state, lowPence,
highPence, stale, reason, quotes), totalCount, pricedCount, lowPence, highPence,
exchangeRate, source, metric, currency, refreshing, stale and warning. Card states
are `priced`, `range`, `pending`, `unavailable`. Absent values are not zero. Batch
figures sum only priced cards: fewer than totalCount is explicitly a **subtotal**.
All amounts named `Pence` are integer GBP minor units; quote `euros` and
exchangeRate.poundsPerEuro are decimal JSON numbers.

Reads return immediately with cached values and enqueue missing/expired public
metadata. At most two fetches run concurrently, with a 200-key queue and coalesced
requests. UI polls at 1.5 seconds while refreshing, then once per minute. API auth,
rate limits and no-store headers remain in force.

SQLite migration v5 stores up to 5,000 catalogue/rate cache entries independently
of scan records. Six-hour refresh interval, 60-second failure cooldown. Offline
cached data remains available but labelled cached, with a seven-day maximum cache
age. Source quotes older than three days are also labelled cached; ECB source
rates older than fourteen days are rejected. Weekends use the latest published
rate. A pricing outage never interrupts hardware jobs or changes scan metadata.

Tests cover monetary rounding, duplicate physical cards, flipped fronts, partial
coverage, missing variant prices, ambiguous mappings, invalid currencies/FX,
offline fallback, retry coalescing, database reopening, RPC validation and HTTP
authentication. Public catalogue inaccuracies remain possible; this is a guide.
