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
Typed market-price conversion is still being migrated.

`library::LibraryCatalogue` adds bounded set checklists, background release dates
and opaque reference-thumbnail keys. Dates and checklists share 64 cached TCGdex
set responses for one day. Three date workers queue at most 2,000 sources; failed
lookups retry after five minutes. Thumbnail keys register only allowlisted public
catalogue URLs; callers cannot supply arbitrary URLs. Two workers accept at most
8 MiB / 16 megapixels per static PNG/JPEG, returning a checked 512-pixel JPEG.
The 128-entry thumbnail cache and release-date scheduling cache live in memory.
`LibraryCatalogue::cached` also persists validated source metadata and encoded
previews through a host-supplied `persistent::Documents` cache, so a restart can
reuse fresh responses. Reload a checklist to register its reference keys.

`yugioh::Yugioh` and `star_wars::StarWars` provide typed English catalogues with
fixed origins, deadlines, size/field limits and coalesced caches. Inventory keeps
YGOPRODeck passcode plus printed set code as the printing ID, deduplicates rarity
rows, and includes English/Asian-English/legacy numbered printings only. SWU
printings retain set plus collector number. Korean requests never fall back to
English. Reference URLs are registered without downloading artwork during listing.
Provider contracts: [YGOPRODeck v7](https://ygoprodeck.com/api-guide/) and
[SWU-DB API](https://www.swu-db.com/api).

`Documents::open` takes an explicit absolute SQLite path. Private ownership and
no-link checks cover the cache and journal files. Stored bytes carry SHA-256;
loaders check freshness, size and provider identity/schema again. Live payloads
are limited to 256 MiB / 512 entries / 64 MiB each; SQLite pages are capped at
384 MiB. Cache work uses four bounded blocking workers. Invalid/missing/expired
cache data causes a refetch; a failed cache write never discards a valid lookup.
Expired records are not silently served as fresh. Cancelled in-memory cache
initializers release capacity when the next lookup arrives.

```sh
cargo run -p scanner-catalogue --example checklist -- swu-db SOR /absolute/private/public.sqlite
```
