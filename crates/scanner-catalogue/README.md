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
