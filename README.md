# eve-sde

Load EVE Online's Static Data Export (SDE), look stuff up in it, and keep
your local copy fresh.

```toml
[dependencies]
eve-sde = "0.1"
```

Needs Rust 1.89 or newer.

## Look things up

```rust
let sde = eve_sde::Sde::load("sde.zip")?;

sde.type_id("Tritanium");             // Some(34)
sde.system_id("Jita");                // Some(30000142)
sde.neighbors(30000142);              // systems one gate away
sde.name(60003760);                   // name of any station, planet, gate, ...
sde.attribute_named(587, "hiSlots");  // Some(3.0)
```

Each table has a read-only accessor (`sde.types()`, `sde.solar_systems()`,
etc). Tables and their lookup indexes are locked in once loading is done, so
you can't accidentally mutate one half of the SDE out of sync with the rest.

Try it out on a real file:

```sh
cargo run --release --example lookup -- sde.zip Jita
cargo run --release --example type_name -- sde.zip 587 Rifter
```

## Get the SDE

Grab it by hand from <https://developers.eveonline.com/static-data/>, or
turn on the `download` feature and let the crate handle it for you:

```toml
[dependencies]
eve-sde = { version = "0.1", features = ["download"] }
```

```rust
use eve_sde::download::Client;

let client = Client::new("my-app/1.0 (me@example.com)");
client.update("sde.zip")?; // only downloads if there's a newer build
```

`update` won't ever replace a newer build with an older one, even if two
processes race to write the same file at the same time.

```sh
cargo run --release --features download --example update -- sde.zip
```

## ID ranges

`eve_sde::ids::kind(id)` tells you what kind of thing an ID is (type,
system, station, player-owned, whatever) just from its numeric range.

## Disclaimer

EVE Online is the registered trademarks of Fenris Creations. This crate is not affiliated with or endorsed by Fenris
Creations. The Static Data Export is their data and is subject to the
[EVE Online Developer License Agreement](https://developers.eveonline.com/license-agreement);
this crate contains none of it.
