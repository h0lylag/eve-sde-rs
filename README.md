# eve-sde

Look things up in EVE Online's Static Data Export (SDE), read it one table
at a time, and keep your local copy fresh.

```toml
[dependencies]
eve-sde = "0.3"
```

Needs Rust 1.89 or newer.

## Look things up

```rust
use eve_sde::Sde;
use eve_sde::ids::{SystemId, TypeId};

let sde = Sde::load("sde.zip")?;

sde.type_id("Tritanium")?;                    // Some(TypeId(34))
sde.system_id("Jita")?;                       // Some(SystemId(30000142))
sde.neighbors(SystemId(30000142))?;           // systems one gate away
sde.name(60003760)?;                          // name of any station, planet, gate, ...
sde.attribute_named(TypeId(587), "hiSlots")?; // Some(3.0)
sde.category_of(TypeId(587))?;                // the Ship category
```

Name searches ignore case, and there's one for types, groups, categories,
market groups, systems, constellations, regions, stations, factions,
corporations, dogma attributes and effects. The rest walk the tables for
you: a type's group, category, meta group and market path, the types in a
group, attribute values with their defaults, effects, fitting slots and
hardpoints, gates and neighbours, and the in-game names of planets, moons,
belts, stargates and stations, which the SDE leaves out. `sde.get::<T>(key)`
reads any record by its key.

`Sde::load` keeps the whole SDE in memory: about 2 s to load and 350 MiB.
To start in under a millisecond with a few MiB instead, keep a local copy
([below](#keep-a-local-copy)) and open it with `Sde::open_store`. Either way
you get the same methods and the same answers.

## Read tables

CCP publishes the SDE as a ZIP of JSON Lines files, one per table. Open it
with `Archive`, then read only the tables you need:

```rust
use eve_sde::Archive;
use eve_sde::ids::TypeId;
use eve_sde::model::{Group, Type};

let mut archive = Archive::open("sde.zip")?;
println!("build {}", archive.build().build_number);

// A whole table, keyed by ID.
let groups = archive.table::<Group>()?;

// Or one record at a time, holding only the current line in memory.
for ty in archive.records::<Type>()? {
    let ty = ty?;
    if ty.id == TypeId(587) {
        println!("{} is a {}", ty.name, groups[&ty.group_id].name); // Rifter is a Frigate
    }
}
```

Reading every table this way takes about 1.2 s and 60 MiB at peak. Each
table's model is in `eve_sde::model`, and `eve_sde::tables()` lists them all.
`visit_tables` runs the same code over every table.

IDs that tables share have their own types, such as `TypeId`, `GroupId` and
`SystemId`, so a system ID cannot be passed where a type ID is expected.
`eve_sde::ids::kind(id)` tells what kind of thing a raw ID is from its range.

The SDE leaves most planet, moon, belt and station names out.
`eve_sde::names` builds them the way the game does, e.g.
`Jita IV - Moon 4 - Caldari Navy Assembly Plant`.

Try it on a real file:

```sh
cargo run --release --example lookup -- sde.zip Jita
cargo run --release --example type_name -- sde.zip 587 Rifter
```

## Get the SDE

Grab it by hand from <https://developers.eveonline.com/static-data/>, or
turn on the `download` feature and let the crate handle it for you:

```toml
[dependencies]
eve-sde = { version = "0.3", features = ["download"] }
```

```rust
use eve_sde::download::Client;

let client = Client::new("my-app/1.0");
client.update("sde.zip")?; // only downloads if there's a newer build
```

`update` won't ever replace a newer build with an older one, even if two
processes race to write the same file at the same time.

```sh
cargo run --release --features download --example update -- sde.zip
```

## Keep a local copy

An app that starts often should not read the ZIP every time. The `store`
feature keeps the whole SDE in one local file with an index of its own, so
looking things up needs no ZIP and no loading:

```toml
[dependencies]
eve-sde = { version = "0.3", features = ["download", "store"] }
```

```rust
use eve_sde::Sde;
use eve_sde::download::Client;
use eve_sde::store;

store::update(&Client::new("my-app/1.0"), "sde")?; // download and import into the `sde` folder
let sde = Sde::open_store("sde")?;                 // the same lookups as above
let jita = sde.system_id("Jita")?;
```

`store::update_with_progress` does the same and reports each step, such as
for a progress bar, and can be cancelled until the import begins.

Importing a whole SDE takes about 2.5 s and makes a file of about 525 MB.
Each build gets its own file, so an update never touches a store that another
process has open, and several threads or processes can read one store at
once. Opening takes well under a millisecond and a few KiB; a table's index
is read the first time something asks for that table.

The store is written once and never again, and every record's checksum is in
its index, so `store::Store::verify()` can say whether a store is still
sound. `Store::lines("types")` reads a table's records as the lines CCP
shipped, including a table this crate has no model for yet.

## Or keep it in SQLite

The `sqlite` feature does the same in a SQLite file, which plain SQL can also
query. Use `sqlite-bundled` to compile SQLite in, which Windows needs.

```toml
[dependencies]
eve-sde = { version = "0.3", features = ["download", "sqlite-bundled"] }
```

```rust
use eve_sde::Sde;
use eve_sde::download::Client;
use eve_sde::sqlite;

sqlite::update(&Client::new("my-app/1.0"), "sde")?;
let sde = Sde::open_sqlite("sde")?; // the same lookups, the same answers
```

Each file of the SDE becomes a table with a `key`, an English `name`, a
lower-case `name_key` and the record as JSON in `data`, so plain SQL reaches
every field for anything the methods don't cover:

```sql
SELECT key, name FROM types WHERE data ->> '$.groupID' = 25;
```

`sqlite::Store::open("sde")?.connection()` runs queries like that. Importing
takes about 4 s and makes a file of about 390 MB, and a store can move
between threads but not be shared.

Both kinds of store live in one folder and share one `sde.zip`, so having
both costs one download. Pick `store` unless you want SQL.

## Tests

The `test-support` feature adds `eve_sde::testing::ArchiveBuilder`, which
builds small synthetic SDE archives, so your tests need no real SDE data.

## Disclaimer

EVE Online is the registered trademark of Fenris Creations. This crate is not
affiliated with or endorsed by Fenris Creations. The Static Data Export is
their data and is subject to the
[EVE Online Developer License Agreement](https://developers.eveonline.com/license-agreement);
this crate contains none of it.
