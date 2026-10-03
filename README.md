# eve-sde

Load EVE Online's Static Data Export (SDE) and look things up in it.

```toml
eve-sde = { path = "../eve-sde-rs" }
```

```rust
let sde = eve_sde::Sde::load("sde.zip")?;

sde.type_id("Tritanium");                  // Some(34)
sde.system_id("Jita");                     // Some(30000142)
sde.neighbors(30000142);                   // systems one gate away
sde.name(60003760);                        // name of any station, planet, gate, ...
sde.attribute_named(587, "hiSlots");       // Some(3.0)
```

Try `cargo run --release --example type_name -- sde.zip 587 Rifter`.

Each SDE table has a read-only accessor (`sde.types()`, `sde.solar_systems()`, ...).
Table data and lookup indexes stay fixed after loading.
Replace field access such as `sde.types[&34]` with `sde.types()[&34]`.
Get the ZIP from <https://developers.eveonline.com/static-data/>, or enable the
`download` feature and call `eve_sde::download::Client::update`.

Rust 1.89 or later is required. Updates preserve newer builds installed by
concurrent updates. Writers use a persistent `.<filename>.lock` file beside the
archive. Do not delete that file while any writer can use it. An explicit
`Client::download` request can replace a newer build with the requested build.

Metadata and individual table records have a 1 MiB limit. The total decompressed
archive has a 2 GiB limit. Oversized input returns `Error::LimitExceeded`.

```sh
cargo test --all-features
EVE_SDE_ZIP=sde.zip cargo test --release -- --ignored   # against a real SDE
```

EVE Online is a trademark of Fenris Creations.
