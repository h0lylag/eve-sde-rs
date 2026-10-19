//! Runs against a real SDE ZIP. Skipped unless asked for:
//! `EVE_SDE_ZIP=/path/to/sde.zip cargo test --release --all-features -- --ignored --nocapture`
#![expect(
    clippy::unwrap_used,
    reason = "test helpers fail by panicking, like the tests"
)]

use eve_sde::ids::{RegionId, StationId, SystemId, TypeId};
use eve_sde::model::{
    Category, Group, Moon, NpcCorporation, NpcStation, Planet, Region, SolarSystem, Stargate,
    StationOperation, Type, TypeDogma,
};
use eve_sde::{
    Archive, Error, Record, SecurityBand, Slot, SpaceKind, TableInfo, TableVisitor, names,
    visit_tables,
};
use std::time::Instant;

fn archive() -> Archive {
    let path = std::env::var("EVE_SDE_ZIP").expect("set EVE_SDE_ZIP to an SDE ZIP");
    Archive::open(path).unwrap()
}

fn peak_memory_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

#[test]
#[ignore = "needs a real SDE ZIP in EVE_SDE_ZIP"]
fn every_table_parses_one_record_at_a_time() {
    struct Count<'a> {
        archive: &'a mut Archive,
        rows: usize,
    }
    impl TableVisitor for Count<'_> {
        type Error = Error;
        fn visit<T: Record>(&mut self, _: &TableInfo) -> Result<(), Error> {
            for record in self.archive.records::<T>()? {
                record?;
                self.rows += 1;
            }
            Ok(())
        }
    }

    let start = Instant::now();
    let mut archive = archive();
    assert!(
        archive.unmodeled_files().is_empty(),
        "{:?}",
        archive.unmodeled_files()
    );
    let build = archive.build().build_number;
    let mut count = Count {
        archive: &mut archive,
        rows: 0,
    };
    visit_tables(&mut count).unwrap();
    println!(
        "read {} records of build {build} in {:?}; peak memory {:?} KiB",
        count.rows,
        start.elapsed(),
        peak_memory_kib()
    );
}

#[test]
#[ignore = "needs a real SDE ZIP in EVE_SDE_ZIP"]
fn answers_known_questions() {
    let mut archive = archive();
    let find = |types: &std::collections::HashMap<TypeId, Type>, name: &str| {
        let mut found: Vec<&Type> = types.values().filter(|t| t.name == name).collect();
        found.sort_by_key(|t| (!t.published, t.id));
        found.first().map(|t| t.id)
    };

    let types = archive.table::<Type>().unwrap();
    let groups = archive.table::<Group>().unwrap();
    let categories = archive.table::<Category>().unwrap();
    assert_eq!(find(&types, "Tritanium"), Some(TypeId(34)));
    let rifter = &types[&TypeId(587)];
    assert_eq!(rifter.name, "Rifter");
    let frigate = &groups[&rifter.group_id];
    assert_eq!(frigate.name, "Frigate");
    assert_eq!(categories[&frigate.category_id].name, "Ship");

    let dogma = archive.table::<TypeDogma>().unwrap();
    let hi_slots = dogma[&rifter.id]
        .dogma_attributes
        .iter()
        .find(|a| a.attribute_id.0 == 14)
        .map(|a| a.value);
    assert_eq!(hi_slots, Some(3.0));
    let damage_control = find(&types, "Damage Control II").unwrap();
    assert_eq!(dogma[&damage_control].slot(), Some(Slot::Low));

    let systems = archive.table::<SolarSystem>().unwrap();
    let regions = archive.table::<Region>().unwrap();
    let jita = &systems[&SystemId(30_000_142)];
    assert_eq!(jita.name, "Jita");
    assert_eq!(jita.region_id, RegionId(10_000_002));
    assert_eq!(regions[&jita.region_id].name, "The Forge");
    assert_eq!(jita.space(), SpaceKind::KnownSpace);
    assert_eq!(jita.security_band(), SecurityBand::High);
    assert_eq!(jita.security_display(), "0.9");
    let wormholes: Vec<_> = systems
        .values()
        .filter(|s| s.space() == SpaceKind::Wormhole)
        .collect();
    assert!(wormholes.len() > 2000);
    assert!(wormholes.iter().all(|s| s.stargate_ids.is_empty()));

    let neighbors: Vec<&str> = archive
        .records::<Stargate>()
        .unwrap()
        .map(Result::unwrap)
        .filter(|gate| gate.solar_system_id == jita.id)
        .map(|gate| systems[&gate.destination.solar_system_id].name.as_str())
        .collect();
    assert!(neighbors.contains(&"Perimeter"), "{neighbors:?}");

    // Name a station from the tables it is built from, streaming the big
    // celestial tables.
    let stations = archive.table::<NpcStation>().unwrap();
    let station = &stations[&StationId(60_003_760)];
    let moon = archive
        .records::<Moon>()
        .unwrap()
        .map(Result::unwrap)
        .find(|moon| moon.id == station.orbit_id)
        .unwrap();
    let planet = archive
        .records::<Planet>()
        .unwrap()
        .map(Result::unwrap)
        .find(|planet| planet.id == moon.orbit_id)
        .unwrap();
    let planet_name = planet.unique_name.unwrap_or_else(|| {
        names::planet(
            &systems[&planet.solar_system_id].name,
            planet.celestial_index,
        )
    });
    let moon_name = moon
        .unique_name
        .unwrap_or_else(|| names::moon(&planet_name, moon.orbit_index));
    let corps = archive.table::<NpcCorporation>().unwrap();
    let operations = archive.table::<StationOperation>().unwrap();
    let operation = station
        .use_operation_name
        .then(|| operations[&station.operation_id].operation_name.as_str());
    assert_eq!(
        names::station(
            &moon_name,
            Some(corps[&station.owner_id].name.as_str()),
            operation
        ),
        "Jita IV - Moon 4 - Caldari Navy Assembly Plant"
    );
}

#[cfg(feature = "store")]
#[test]
#[ignore = "needs a real SDE ZIP in EVE_SDE_ZIP"]
fn the_whole_sde_imports_into_a_store() {
    use eve_sde::Sde;
    use eve_sde::ids::{CelestialId, GroupId};
    use eve_sde::store::{self, ImportOutcome, Store};

    let dir = tempfile::tempdir().unwrap();
    let start = Instant::now();
    let mut archive = archive();
    let build = archive.build().build_number;
    assert_eq!(
        store::import(&mut archive, dir.path()).unwrap(),
        ImportOutcome::Imported
    );
    let imported = start.elapsed();
    let file = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "sde"))
        .unwrap();
    let bytes = std::fs::metadata(&file).unwrap().len();
    let imported_peak = peak_memory_kib();

    let start = Instant::now();
    let store = Store::open(dir.path()).unwrap();
    let opened = start.elapsed();
    assert_eq!(store.build().build_number, build);
    assert!(
        store.unmodeled_files().is_empty(),
        "{:?}",
        store.unmodeled_files()
    );

    // The first lookup of a table reads its key index; later ones do not.
    let start = Instant::now();
    let rifter = store.get::<Type>(TypeId(587)).unwrap().unwrap();
    let first_lookup = start.elapsed();
    assert_eq!(rifter.name, "Rifter");

    let start = Instant::now();
    for _ in 0..1000 {
        let rifter = store.get::<Type>(TypeId(587)).unwrap().unwrap();
        assert_eq!(rifter.name, "Rifter");
    }
    let lookup = start.elapsed() / 1000;

    let jita = store
        .get::<SolarSystem>(SystemId(30_000_142))
        .unwrap()
        .unwrap();
    assert_eq!(jita.name, "Jita");
    // A moon, from the table that is 42% of the SDE and costs nothing until
    // it is asked for.
    let start = Instant::now();
    let moon = store.get::<Moon>(CelestialId(40_000_004)).unwrap();
    let moon_lookup = start.elapsed();
    assert!(moon.is_some());

    // The name and children indexes, and the station names the import
    // builds from the records it has just written.
    let sde = Sde::from(store);
    let start = Instant::now();
    assert_eq!(sde.type_id("Rifter").unwrap(), Some(TypeId(587)));
    let named = start.elapsed();
    assert_eq!(sde.system_id("Jita").unwrap(), Some(SystemId(30_000_142)));
    assert!(
        sde.types_in_group(GroupId(25))
            .unwrap()
            .contains(&TypeId(587))
    );
    let station = "Jita IV - Moon 4 - Caldari Navy Assembly Plant";
    assert_eq!(
        sde.station_id(station).unwrap(),
        Some(StationId(60_003_760))
    );

    // What a lookup costs when it is not one read but a walk: a station's
    // name reads the station, its celestial, that celestial's planet, the
    // system, the owner and the operation.
    let start = Instant::now();
    for _ in 0..100 {
        assert_eq!(
            sde.station_name(StationId(60_003_760)).unwrap().as_deref(),
            Some(station)
        );
    }
    let station_name = start.elapsed() / 100;
    let start = Instant::now();
    for _ in 0..100 {
        assert!(sde.name(40_000_004).unwrap().is_some());
    }
    let celestial_name = start.elapsed() / 100;

    println!(
        "imported build {build} in {imported:?} into {bytes} bytes; open {opened:?}; \
         first type lookup {first_lookup:?}, then {lookup:?}; first moon lookup \
         {moon_lookup:?}; first name lookup {named:?}; station_name {station_name:?}; \
         celestial name {celestial_name:?}; peak memory {imported_peak:?} KiB importing, \
         {:?} KiB now",
        peak_memory_kib()
    );
}

#[cfg(feature = "sqlite")]
#[test]
#[ignore = "needs a real SDE ZIP in EVE_SDE_ZIP"]
fn the_whole_sde_imports_into_sqlite() {
    use eve_sde::sqlite::{self, ImportOutcome, Store};

    let dir = tempfile::tempdir().unwrap();
    let start = Instant::now();
    let mut archive = archive();
    let build = archive.build().build_number;
    assert_eq!(
        sqlite::import(&mut archive, dir.path()).unwrap(),
        ImportOutcome::Imported
    );
    let imported = start.elapsed();
    let file = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "sqlite"))
        .unwrap();
    let bytes = std::fs::metadata(&file).unwrap().len();

    let start = Instant::now();
    let store = Store::open(dir.path()).unwrap();
    let opened = start.elapsed();
    let rows: i64 = store
        .connection()
        .query_row("SELECT sum(rows) FROM sde_tables", [], |row| row.get(0))
        .unwrap();
    let start = Instant::now();
    for _ in 0..1000 {
        let rifter = store.get::<Type>(TypeId(587)).unwrap().unwrap();
        assert_eq!(rifter.name, "Rifter");
    }
    let lookup = start.elapsed() / 1000;
    let jita = store
        .get::<SolarSystem>(SystemId(30_000_142))
        .unwrap()
        .unwrap();
    assert_eq!(jita.name, "Jita");
    let frigates: i64 = store
        .connection()
        .query_row(
            "SELECT count(*) FROM types WHERE data ->> '$.groupID' = 25",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(frigates > 50, "{frigates}");
    println!(
        "imported {rows} records of build {build} in {imported:?} into {bytes} bytes; \
         open {opened:?}; one lookup {lookup:?}; peak memory {:?} KiB",
        peak_memory_kib()
    );
}

#[test]
#[ignore = "needs a real SDE ZIP in EVE_SDE_ZIP"]
fn both_backends_answer_the_lookups_alike() {
    use eve_sde::Sde;
    use eve_sde::ids::GroupId;

    let start = Instant::now();
    #[cfg_attr(
        not(any(feature = "sqlite", feature = "store")),
        expect(unused_mut, reason = "only a store on disk is added below")
    )]
    let mut sdes = vec![("memory", Sde::from_archive(&mut archive()).unwrap())];
    println!("loaded into memory in {:?}", start.elapsed());
    #[cfg(feature = "store")]
    let store_dir = tempfile::tempdir().unwrap();
    #[cfg(feature = "store")]
    {
        eve_sde::store::import(&mut archive(), store_dir.path()).unwrap();
        sdes.push(("store", Sde::open_store(store_dir.path()).unwrap()));
        println!(
            "imported and opened a packed store in {:?}",
            start.elapsed()
        );
    }
    #[cfg(feature = "sqlite")]
    let dir = tempfile::tempdir().unwrap();
    #[cfg(feature = "sqlite")]
    {
        let start = Instant::now();
        eve_sde::sqlite::import(&mut archive(), dir.path()).unwrap();
        println!("imported into SQLite in {:?}", start.elapsed());
        let start = Instant::now();
        sdes.push(("sqlite", Sde::open_sqlite(dir.path()).unwrap()));
        println!("opened the store in {:?}", start.elapsed());
    }

    let jita = SystemId(30_000_142);
    let station = "Jita IV - Moon 4 - Caldari Navy Assembly Plant";
    for (backend, sde) in &sdes {
        let start = Instant::now();
        assert_eq!(
            sde.type_id("Tritanium").unwrap(),
            Some(TypeId(34)),
            "{backend}"
        );
        assert_eq!(sde.system_id("jita").unwrap(), Some(jita), "{backend}");
        let perimeter = sde.system_id("Perimeter").unwrap().unwrap();
        assert!(
            sde.neighbors(jita).unwrap().contains(&perimeter),
            "{backend}"
        );
        assert_eq!(
            sde.name(60_003_760).unwrap().as_deref(),
            Some(station),
            "{backend}"
        );
        assert_eq!(
            sde.station_id(station).unwrap(),
            Some(StationId(60_003_760)),
            "{backend}"
        );
        let rifter = TypeId(587);
        assert_eq!(
            sde.group_of(rifter).unwrap().unwrap().name,
            "Frigate",
            "{backend}"
        );
        assert_eq!(
            sde.category_of(rifter).unwrap().unwrap().name,
            "Ship",
            "{backend}"
        );
        assert_eq!(
            sde.attribute_named(rifter, "hiSlots").unwrap(),
            Some(3.0),
            "{backend}"
        );
        assert!(!sde.market_path(rifter).unwrap().is_empty(), "{backend}");
        let damage_control = sde.type_id("Damage Control II").unwrap().unwrap();
        assert_eq!(
            sde.slot(damage_control).unwrap(),
            Some(Slot::Low),
            "{backend}"
        );
        assert!(
            sde.types_in_group(GroupId(25)).unwrap().contains(&rifter),
            "{backend}"
        );
        println!("{backend}: the lookups took {:?}", start.elapsed());
    }

    // The backends agree on every station name, and on the groups and types
    // of the Ship category.
    // Every backend against the one in memory, over every station and every
    // group of ships. A pattern that named two backends would skip the
    // third, so take the first and compare the rest to it.
    let [(_, memory), others @ ..] = sdes.as_slice() else {
        return;
    };
    for (backend, other) in others {
        every_station_and_group_alike(backend, memory, other);
    }
    println!("{} backends agree", sdes.len());
}

/// Two backends over every NPC station's name and every group of ships.
#[cfg(any(feature = "sqlite", feature = "store"))]
fn every_station_and_group_alike(backend: &str, memory: &eve_sde::Sde, other: &eve_sde::Sde) {
    use eve_sde::ids::CategoryId;

    for station in archive().records::<NpcStation>().unwrap() {
        let id = station.unwrap().id;
        assert_eq!(
            memory.station_name(id).unwrap(),
            other.station_name(id).unwrap(),
            "{backend} station {id}"
        );
    }
    let ships = CategoryId(6);
    let groups = memory.groups_in_category(ships).unwrap();
    assert_eq!(
        groups,
        other.groups_in_category(ships).unwrap(),
        "{backend}"
    );
    for group in groups {
        assert_eq!(
            memory.types_in_group(group).unwrap(),
            other.types_in_group(group).unwrap(),
            "{backend} group {group}"
        );
    }
}

/// The two stores against each other, over every station name and every type
/// and group name. Neither import reads records through the models, so this
/// runs against an SDE older than `schema/`, where the tests that load the
/// SDE into memory cannot.
#[cfg(all(feature = "store", feature = "sqlite"))]
#[test]
#[ignore = "needs a real SDE ZIP in EVE_SDE_ZIP"]
fn the_two_stores_agree_on_a_real_sde() {
    use eve_sde::Sde;
    use eve_sde::ids::{CategoryId, StationId};
    use eve_sde::model::Group;

    let packed_dir = tempfile::tempdir().unwrap();
    let sqlite_dir = tempfile::tempdir().unwrap();
    eve_sde::store::import(&mut archive(), packed_dir.path()).unwrap();
    eve_sde::sqlite::import(&mut archive(), sqlite_dir.path()).unwrap();
    let packed = Sde::open_store(packed_dir.path()).unwrap();
    let sqlite = Sde::open_sqlite(sqlite_dir.path()).unwrap();

    // Every station's name, built by each import in its own way.
    let start = Instant::now();
    let mut stations = 0;
    for station in archive().records::<NpcStation>().unwrap() {
        let id = station.unwrap().id;
        assert_eq!(
            packed.station_name(id).unwrap(),
            sqlite.station_name(id).unwrap(),
            "station {id}"
        );
        stations += 1;
    }
    println!("{stations} station names agree in {:?}", start.elapsed());

    // Every type and group looked up by its own name, in both directions.
    let start = Instant::now();
    let mut names = 0;
    for ty in archive().records::<Type>().unwrap() {
        let ty = ty.unwrap();
        assert_eq!(
            packed.type_ids(&ty.name).unwrap(),
            sqlite.type_ids(&ty.name).unwrap(),
            "type {} named {:?}",
            ty.id,
            ty.name
        );
        names += 1;
    }
    for group in archive().records::<Group>().unwrap() {
        let group = group.unwrap();
        assert_eq!(
            packed.group_ids(&group.name).unwrap(),
            sqlite.group_ids(&group.name).unwrap(),
            "group {}",
            group.id
        );
        assert_eq!(
            packed.types_in_group(group.id).unwrap(),
            sqlite.types_in_group(group.id).unwrap(),
            "group {}",
            group.id
        );
        names += 1;
    }
    println!(
        "{names} name and children lookups agree in {:?}",
        start.elapsed()
    );

    // And a few by hand.
    assert_eq!(packed.type_id("Rifter").unwrap(), Some(TypeId(587)));
    assert_eq!(
        packed
            .station_id("Jita IV - Moon 4 - Caldari Navy Assembly Plant")
            .unwrap(),
        Some(StationId(60_003_760))
    );
    assert_eq!(
        packed.groups_in_category(CategoryId(6)).unwrap(),
        sqlite.groups_in_category(CategoryId(6)).unwrap()
    );
}
