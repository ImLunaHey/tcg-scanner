# Public card catalogues

`TcgDex` fetches public English/Korean Pokémon catalogue, card and set metadata
with in-process HTTPS. It uploads no images, OCR readings or inventory. Requests
use a fixed HTTPS origin, refuse redirects, have deadlines and a two-request
limit, and bound body bytes before deserialization. IDs and response identity,
collection sizes and field lengths are validated.

Catalogue results expire after 24 hours; up to 128 card details expire after 15
minutes. Concurrent identical reads share their load, and failed loads can be
retried. An injected `JsonTransport` supports offline catalogues and tests.

```sh
cargo run -p scanner-catalogue --example card -- sv09-177
cargo test -p scanner-catalogue
```

This is an internal workspace library (`publish = false`), with a versioned path
dependency on shared `scanner-types`. Publication and licensing are not inferred.
Other games and typed market-price conversion are still being migrated.

`library::LibraryCatalogue` adds bounded set checklists, background release dates
and opaque reference-thumbnail keys. Dates and checklists share 64 cached TCGdex
set responses for one day. Three date workers queue at most 2,000 sources; failed
lookups retry after five minutes. Thumbnail keys register only allowlisted public
catalogue URLs; callers cannot supply arbitrary URLs. Two workers accept at most
8 MiB / 16 megapixels per static PNG/JPEG, returning a checked 512-pixel JPEG.
The 128-entry thumbnail cache and release-date cache are currently memory-only;
restarting may require refetching optional metadata and reloading a checklist.

`yugioh::Yugioh` and `star_wars::StarWars` provide typed English catalogues with
fixed origins, deadlines, size/field limits and coalesced caches. Inventory keeps
YGOPRODeck passcode plus printed set code as the printing ID, deduplicates rarity
rows, and includes English/Asian-English/legacy numbered printings only. SWU
printings retain set plus collector number. Korean requests never fall back to
English. Reference URLs are registered without downloading artwork during listing.
Provider contracts: [YGOPRODeck v7](https://ygoprodeck.com/api-guide/) and
[SWU-DB API](https://www.swu-db.com/api).
