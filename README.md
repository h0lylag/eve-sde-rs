# eve-sde

A Rust library for EVE Online's Static Data Export (SDE). It reads the SDE
ZIP, looks things up in it, downloads new builds, and keeps a local copy on
disk.

```toml
[dependencies]
eve-sde = "0.3"
```

Requires Rust 1.89 or newer.

## Features

| Feature          | Adds                                                        |
| ---------------- | ----------------------------------------------------------- |
| `archive`        | `Archive`, `Sde` in memory and the models. On by default.   |
| `download`       | `download::Client`, which downloads new builds from CCP.    |
| `store`          | A local copy in one packed file. Unix and Windows only.     |
| `sqlite`         | A local copy in SQLite, linked against the system's SQLite. |
| `sqlite-bundled` | `sqlite` with SQLite compiled in. Needed on Windows.        |
| `test-support`   | `testing::ArchiveBuilder`, which builds synthetic archives. |

With no features, only `ids`, `names` and the fitting rules are built.

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

`Sde` has:

- Name searches, case-insensitive, for types, groups, categories, market
  groups, systems, constellations, regions, stations, factions,
  corporations, dogma attributes and effects.
- A type's group, category, meta group and market path, and the types in a
  group.
- Attribute values with their defaults, effects, fitting slots and
  hardpoints.
- Stargates and neighbouring systems.
- Names of planets, moons, belts, stargates and stations, which the SDE
  does not include.
- `sde.get::<T>(key)`, which reads any record by its key.

`Sde::load` holds the whole SDE in memory: about 2 s to load and 350 MiB.
`Sde::open_store` and `Sde::open_sqlite` open a [local copy](#local-copy)
instead, which takes under a millisecond. All three give the same answers.

## Read tables

The SDE is a ZIP of JSON Lines files, one per table. `Archive` reads one
table at a time:

```rust
use eve_sde::Archive;
use eve_sde::ids::TypeId;
use eve_sde::model::{Group, Type};

let mut archive = Archive::open("sde.zip")?;
println!("build {}", archive.build().build_number);

// A whole table, keyed by ID.
let groups = archive.table::<Group>()?;

// One record at a time.
for ty in archive.records::<Type>()? {
    let ty = ty?;
    if ty.id == TypeId(587) {
        println!("{} is a {}", ty.name, groups[&ty.group_id].name); // Rifter is a Frigate
    }
}
```

Reading every record of every table one at a time takes about 1.3 s and
under 10 MiB of memory.

- `eve_sde::model` has one struct per table. `eve_sde::tables()` lists the
  tables, and `visit_tables` runs code over each.
- IDs have their own types, such as `TypeId`, `GroupId` and `SystemId`.
  `eve_sde::ids::kind(id)` tells what kind of thing a raw ID is from its
  range.
- `eve_sde::names` builds the names the SDE leaves out, such as
  `Jita IV - Moon 4 - Caldari Navy Assembly Plant`.

```sh
cargo run --release --example lookup -- sde.zip Jita
cargo run --release --example type_name -- sde.zip 587 Rifter
```

## Download

The SDE is at <https://developers.eveonline.com/static-data/>. With the
`download` feature:

```rust
use eve_sde::download::Client;

let client = Client::new("my-app/1.0");
client.update("sde.zip")?; // downloads only if CCP has a newer build
```

`update` never replaces a newer build with an older one, including when two
processes update the same file at once.

```sh
cargo run --release --features download --example update -- sde.zip
```

## Local copy

`store` and `sqlite` import the SDE into a folder, so a program does not
read the ZIP each time it starts. Both stores can share one folder and one
`sde.zip`.

### `store`

```rust
use eve_sde::Sde;
use eve_sde::download::Client;
use eve_sde::store;

store::update(&Client::new("my-app/1.0"), "sde")?; // download and import into ./sde
let sde = Sde::open_store("sde")?;
```

- Import takes about 2 s and writes a file of about 600 MB.
- Each build gets its own file, so an update does not touch a store that is
  open. Several threads and processes can read one store.
- `store::Store::verify()` checks every record against its checksum.
- `Store::lines("types")` returns a table's records as CCP's original
  lines, including tables this crate has no model for.

### `sqlite`

```rust
use eve_sde::Sde;
use eve_sde::download::Client;
use eve_sde::sqlite;

sqlite::update(&Client::new("my-app/1.0"), "sde")?;
let sde = Sde::open_sqlite("sde")?;
```

Each SDE file becomes a table with a `key`, an English `name`, a lower-case
`name_key`, and the record as JSON in `data`:

```sql
SELECT key, name FROM types WHERE data ->> '$.groupID' = 25;
```

- `sqlite::Store::open("sde")?.connection()` gives the `rusqlite`
  connection.
- Import takes about 5 s and writes a file of about 425 MB.
- A SQLite store can move between threads but cannot be shared.

`store::update_with_progress` and `sqlite::update_with_progress` report each
step and can be cancelled until the import starts.

## Testing

The `test-support` feature adds `eve_sde::testing::ArchiveBuilder`, which
builds small synthetic SDE archives for tests.

## Disclaimer

EVE Online is the registered trademark of Fenris Creations. This crate is not
affiliated with or endorsed by Fenris Creations. The Static Data Export is
their data and is subject to the
[EVE Online Developer License Agreement](https://developers.eveonline.com/license-agreement);
this crate contains none of it.
