//! Reading tables from synthetic archives, and the errors on bad ones.
#![expect(
    clippy::unwrap_used,
    reason = "test helpers fail by panicking, like the tests"
)]
mod common;

use eve_sde::ids::{CorporationId, GroupId, StationId, SystemId, TypeId};
use eve_sde::model::{
    AsteroidBelt, Moon, NpcCorporation, NpcStation, Planet, SolarSystem, Star, StationOperation,
    Type, TypeDogma,
};
use eve_sde::testing::ArchiveBuilder;
use eve_sde::{
    Archive, Error, Record, SecurityBand, Slot, SpaceKind, TableInfo, TableVisitor, names,
    visit_tables,
};
use std::collections::HashMap;
use std::io::Cursor;

const TYPES: &str = r#"{"_key":34,"groupID":18,"name":{"en":"Tritanium","de":"Tritanium"},"portionSize":1,"published":true,"marketGroupID":3}
{"_key":587,"groupID":25,"name":{"en":"Rifter"},"portionSize":1,"published":true,"marketGroupID":2}
{"_key":2046,"groupID":60,"name":{"en":"Damage Control II"},"portionSize":1,"published":true}"#;
const TYPE_DOGMA: &str = r#"{"_key":587,"dogmaAttributes":[{"attributeID":14,"value":3.0}],"dogmaEffects":[{"effectID":12,"isDefault":false}]}
{"_key":2046,"dogmaEffects":[{"effectID":11,"isDefault":false}]}"#;

const POS: &str = r#"{"x":0,"y":0,"z":0}"#;

fn systems() -> String {
    let row = |id: u32, name: &str, sec: f64, gates: &str| {
        format!(
            r#"{{"_key":{id},"constellationID":20000001,"name":{{"en":"{name}"}},"position":{POS},"radius":1.0,"regionID":10000001,"securityStatus":{sec},"stargateIDs":{gates},"planetIDs":[40000002]}}"#
        )
    };
    [
        row(30_000_001, "Alpha", 0.9, "[50000001]"),
        row(30_000_002, "Beta", 0.03, "[50000002]"),
        row(31_000_001, "J100001", -1.0, "[]"),
    ]
    .join("\n")
}

const STARS: &str = r#"{"_key":40000001,"solarSystemID":30000001,"typeID":6,"radius":1.0,"statistics":{"age":1,"life":1,"luminosity":1,"spectralClass":"G","temperature":1}}"#;

fn planets() -> String {
    let row = |id: u32, index: u32, unique: &str| {
        format!(
            r#"{{"_key":{id},"solarSystemID":30000001,"typeID":11,"celestialIndex":{index},"orbitID":40000001,"position":{POS},"radius":1.0,"statistics":{{"density":1,"eccentricity":0,"escapeVelocity":1,"locked":false,"massDust":1,"rotationRate":1,"spectralClass":"G","temperature":1}},"attributes":{{"heightMap1":1,"heightMap2":1,"shaderPreset":1,"population":false}}{unique}}}"#
        )
    };
    [
        row(40_000_002, 4, ""),
        row(40_000_005, 5, r#","uniqueName":{"en":"Alpha V (Prime)"}"#),
    ]
    .join("\n")
}

fn moons() -> String {
    let row = |id: u32, planet: u32, index: u32, orbit: u32| {
        format!(
            r#"{{"_key":{id},"solarSystemID":30000001,"typeID":14,"celestialIndex":{index},"orbitIndex":{orbit},"orbitID":{planet},"position":{POS},"radius":1.0,"attributes":{{"heightMap1":1,"heightMap2":1,"shaderPreset":1}}}}"#
        )
    };
    [
        row(40_000_003, 40_000_002, 4, 2),
        row(40_000_006, 40_000_005, 5, 1),
    ]
    .join("\n")
}

fn belts() -> String {
    let row = |id: u32, planet: u32, index: u32| {
        format!(
            r#"{{"_key":{id},"solarSystemID":30000001,"typeID":15,"celestialIndex":{index},"orbitIndex":1,"orbitID":{planet},"position":{POS}}}"#
        )
    };
    [
        row(40_000_004, 40_000_002, 4),
        row(40_000_007, 40_000_005, 5),
    ]
    .join("\n")
}

fn stations() -> String {
    let row = |id: u32, orbit: u32, op_name: bool| {
        format!(
            r#"{{"_key":{id},"operationID":1,"orbitID":{orbit},"ownerID":1000001,"position":{POS},"reprocessingEfficiency":0.5,"reprocessingHangarFlag":4,"reprocessingStationsTake":0.05,"solarSystemID":30000001,"typeID":1531,"useOperationName":{op_name}}}"#
        )
    };
    [
        row(60_000_001, 40_000_003, true),
        row(60_000_002, 40_000_002, false),
        row(60_000_003, 40_000_001, false),
    ]
    .join("\n")
}

const CORPS: &str = r#"{"_key":1000001,"deleted":false,"extent":"national","hasPlayerPersonnelManager":false,"initialPrice":1,"memberLimit":-1,"minSecurity":0.0,"minimumJoinStanding":0,"name":{"en":"Caldari Navy"},"sendCharTerminationMessage":true,"shares":1,"size":"large","taxRate":0.1,"tickerName":"CN","uniqueName":true}"#;
const OPERATIONS: &str = r#"{"_key":1,"activityID":1,"border":0.0,"corridor":0.0,"fringe":0.0,"hub":0.0,"manufacturingFactor":1.0,"operationName":{"en":"Assembly Plant"},"ratio":1.0,"researchFactor":1.0,"services":[]}"#;

fn builder() -> ArchiveBuilder {
    ArchiveBuilder::new(3_569_502)
        .release_date("2026-10-02T11:08:57Z")
        .rows::<Type>(TYPES)
        .rows::<TypeDogma>(TYPE_DOGMA)
        .rows::<SolarSystem>(systems())
        .rows::<Star>(STARS)
        .rows::<Planet>(planets())
        .rows::<Moon>(moons())
        .rows::<AsteroidBelt>(belts())
        .rows::<NpcStation>(stations())
        .rows::<NpcCorporation>(CORPS)
        .rows::<StationOperation>(OPERATIONS)
}

fn open(bytes: Vec<u8>) -> Result<Archive<Cursor<Vec<u8>>>, Error> {
    Archive::from_reader(Cursor::new(bytes))
}

fn fixture() -> Archive<Cursor<Vec<u8>>> {
    open(builder().into_bytes()).unwrap()
}

#[test]
fn reads_the_build_and_a_table_keyed_by_id() {
    let mut archive = fixture();
    assert_eq!(archive.build().build_number, 3_569_502);
    assert_eq!(
        archive.build().release_date.as_deref(),
        Some("2026-10-02T11:08:57Z")
    );
    assert_eq!(archive.unmodeled_files(), Vec::<String>::new());

    let types = archive.table::<Type>().unwrap();
    assert_eq!(types.len(), 3);
    assert_eq!(types[&TypeId(34)].name, "Tritanium");
    assert_eq!(types[&TypeId(587)].group_id, GroupId(25));
}

#[test]
fn records_are_parsed_one_line_at_a_time() {
    let types = format!("{}\nnot json\n", TYPES.lines().next().unwrap());
    let mut archive = open(builder().rows::<Type>(types).into_bytes()).unwrap();

    // The first record arrives although a later line is broken.
    let mut records = archive.records::<Type>().unwrap();
    assert_eq!(records.next().unwrap().unwrap().id, TypeId(34));
    assert!(matches!(
        records.next(),
        Some(Err(Error::Parse { line: 2, .. }))
    ));
    // The iterator stops after an error.
    assert!(records.next().is_none());
}

#[test]
fn a_table_can_be_read_again() {
    let mut archive = fixture();
    let first = archive.records::<Type>().unwrap().count();
    let second = archive.records::<Type>().unwrap().count();
    assert_eq!((first, second), (3, 3));
}

#[test]
fn the_visitor_can_read_every_table() {
    struct Count<'a> {
        archive: &'a mut Archive<Cursor<Vec<u8>>>,
        rows: HashMap<&'static str, usize>,
    }
    impl TableVisitor for Count<'_> {
        type Error = Error;
        fn visit<T: Record>(&mut self, table: &TableInfo) -> Result<(), Error> {
            let mut rows = 0;
            for record in self.archive.records::<T>()? {
                record?;
                rows += 1;
            }
            self.rows.insert(table.name, rows);
            Ok(())
        }
    }

    let mut archive = fixture();
    let mut count = Count {
        archive: &mut archive,
        rows: HashMap::new(),
    };
    visit_tables(&mut count).unwrap();
    assert_eq!(count.rows.len(), eve_sde::tables().len());
    assert_eq!(count.rows["types"], 3);
    assert_eq!(count.rows["npc_stations"], 3);
    assert_eq!(count.rows["groups"], 0);
}

#[test]
fn type_dogma_gives_the_slot() {
    let dogma = fixture().table::<TypeDogma>().unwrap();
    assert_eq!(dogma[&TypeId(587)].slot(), Some(Slot::High));
    assert_eq!(dogma[&TypeId(2046)].slot(), Some(Slot::Low));
    assert_eq!(dogma[&TypeId(587)].hardpoint(), None);
}

#[test]
fn systems_know_their_space_and_security() {
    let systems = fixture().table::<SolarSystem>().unwrap();
    let alpha = &systems[&SystemId(30_000_001)];
    assert_eq!(alpha.space(), SpaceKind::KnownSpace);
    assert_eq!(alpha.security_band(), SecurityBand::High);
    assert_eq!(alpha.security_display(), "0.9");
    let beta = &systems[&SystemId(30_000_002)];
    assert_eq!(beta.security_band(), SecurityBand::Low);
    assert_eq!(beta.security_display(), "0.1");
    let wormhole = &systems[&SystemId(31_000_001)];
    assert_eq!(wormhole.space(), SpaceKind::Wormhole);
    assert_eq!(wormhole.security_band(), SecurityBand::Null);
    assert_eq!(wormhole.security_display(), "-1.0");
}

/// Names every planet, moon, belt and station of the fixture with the rules
/// in `names`, using the SDE's own name where it has one.
fn derived_names(archive: &mut Archive<Cursor<Vec<u8>>>) -> HashMap<u32, String> {
    let systems = archive.table::<SolarSystem>().unwrap();
    let system = |id: SystemId| systems[&id].name.clone();
    let mut names = HashMap::new();
    for planet in archive.records::<Planet>().unwrap() {
        let planet = planet.unwrap();
        let name = planet.unique_name.unwrap_or_else(|| {
            names::planet(&system(planet.solar_system_id), planet.celestial_index)
        });
        names.insert(planet.id.0, name);
    }
    for moon in archive.records::<Moon>().unwrap() {
        let moon = moon.unwrap();
        let name = moon
            .unique_name
            .unwrap_or_else(|| names::moon(&names[&moon.orbit_id.0], moon.orbit_index));
        names.insert(moon.id.0, name);
    }
    for belt in archive.records::<AsteroidBelt>().unwrap() {
        let belt = belt.unwrap();
        let name = belt
            .unique_name
            .unwrap_or_else(|| names::asteroid_belt(&names[&belt.orbit_id.0], belt.orbit_index));
        names.insert(belt.id.0, name);
    }
    let corps = archive.table::<NpcCorporation>().unwrap();
    let operations = archive.table::<StationOperation>().unwrap();
    for station in archive.records::<NpcStation>().unwrap() {
        let station = station.unwrap();
        // A station that orbits its star is named after the system.
        let place = names
            .get(&station.orbit_id.0)
            .cloned()
            .unwrap_or_else(|| system(station.solar_system_id));
        let owner = corps.get(&station.owner_id).map(|c| c.name.as_str());
        let operation = station
            .use_operation_name
            .then(|| operations[&station.operation_id].operation_name.as_str());
        names.insert(station.id.0, names::station(&place, owner, operation));
    }
    names
}

#[test]
fn celestials_and_stations_are_named_with_the_naming_rules() {
    let names = derived_names(&mut fixture());
    assert_eq!(names[&40_000_002], "Alpha IV");
    assert_eq!(names[&40_000_003], "Alpha IV - Moon 2");
    assert_eq!(names[&40_000_004], "Alpha IV - Asteroid Belt 1");
    // The SDE names this planet but not its moon or belt.
    assert_eq!(names[&40_000_005], "Alpha V (Prime)");
    assert_eq!(names[&40_000_006], "Alpha V (Prime) - Moon 1");
    assert_eq!(names[&40_000_007], "Alpha V (Prime) - Asteroid Belt 1");
    assert_eq!(
        names[&60_000_001],
        "Alpha IV - Moon 2 - Caldari Navy Assembly Plant"
    );
    assert_eq!(names[&60_000_002], "Alpha IV - Caldari Navy");
    assert_eq!(names[&60_000_003], "Alpha - Caldari Navy");
}

#[test]
fn stations_name_their_owner_by_corporation_id() {
    let stations = fixture().table::<NpcStation>().unwrap();
    assert_eq!(
        stations[&StationId(60_000_001)].owner_id,
        CorporationId(1_000_001)
    );
}

#[test]
fn a_missing_table_is_an_error_only_when_read() {
    let mut archive = open(builder().without(Moon::FILE).into_bytes()).unwrap();
    assert!(archive.table::<Type>().is_ok());
    match archive.records::<Moon>() {
        Err(Error::MissingFile(name)) => assert_eq!(name, "mapMoons.jsonl"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_missing_archive_error_names_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sde.zip");
    let err = Archive::open(&path).unwrap_err();
    let Error::Io(io) = &err else {
        panic!("{err:?}");
    };
    assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(err.to_string(), format!("cannot open `{}`", path.display()));
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn a_damaged_file_error_names_the_file() {
    for (file, text) in [("_sde.jsonl", "buildNumber"), ("types.jsonl", "Tritanium")] {
        // Uncompressed, so the text appears as is. Changing the case of one
        // letter keeps the data valid but breaks its checksum.
        let mut bytes = ArchiveBuilder::new(1)
            .rows::<Type>(TYPES)
            .stored()
            .into_bytes();
        let at = bytes
            .windows(text.len())
            .position(|w| w == text.as_bytes())
            .unwrap();
        bytes[at] ^= 0x20;
        let result = open(bytes).and_then(|mut archive| archive.table::<Type>().map(drop));
        match result {
            Err(Error::Read { file: name, source }) => {
                assert_eq!(name, file);
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
            }
            other => panic!("{file}: {other:?}"),
        }
    }
}

#[test]
fn a_parse_error_names_the_file_and_line() {
    let types = format!("{}\n\nnot json\n", TYPES.lines().next().unwrap());
    let mut archive = open(builder().rows::<Type>(types).into_bytes()).unwrap();
    let err = archive.table::<Type>().unwrap_err();
    assert_eq!(err.to_string(), "cannot parse `types.jsonl` line 3");
    let Error::Parse { source, .. } = &err else {
        panic!("{err:?}");
    };
    assert!(source.downcast_ref::<serde_json::Error>().is_some());
}

#[test]
fn a_missing_required_field_is_an_error() {
    let mut archive = open(
        builder()
            .rows::<Type>(r#"{"_key":1,"groupID":1}"#)
            .into_bytes(),
    )
    .unwrap();
    assert!(matches!(archive.table::<Type>(), Err(Error::Parse { .. })));
}

#[test]
fn a_repeated_key_is_an_error_for_a_table_but_not_for_records() {
    let row = TYPES.lines().next().unwrap();
    let mut archive = open(builder().rows::<Type>(format!("{row}\n{row}")).into_bytes()).unwrap();
    assert_eq!(archive.records::<Type>().unwrap().count(), 2);
    assert!(matches!(
        archive.table::<Type>(),
        Err(Error::DuplicateKey { .. })
    ));
}

#[test]
fn unknown_fields_are_ignored_and_new_files_listed() {
    let types = r#"{"_key":1,"groupID":1,"name":{"en":"x","xx":"y"},"portionSize":1,"published":true,"brandNew":42}"#;
    let mut archive = open(
        builder()
            .rows::<Type>(types)
            .file("newTable.jsonl", "{}")
            .into_bytes(),
    )
    .unwrap();
    assert_eq!(archive.table::<Type>().unwrap().len(), 1);
    assert_eq!(archive.unmodeled_files(), ["newTable.jsonl"]);
}

#[test]
fn bad_build_metadata_is_rejected() {
    for body in [
        "",
        "not json",
        r#"{"_key":"nope","buildNumber":1}"#,
        r#"{"_key":"sde","buildNumber":0}"#,
    ] {
        let bytes = builder().file("_sde.jsonl", body).into_bytes();
        assert!(open(bytes).is_err(), "{body:?}");
    }
    assert!(matches!(
        open(builder().without("_sde.jsonl").into_bytes()),
        Err(Error::MissingFile(name)) if name == "_sde.jsonl"
    ));
    assert!(matches!(open(b"not a zip".to_vec()), Err(Error::Zip(_))));
}

#[test]
fn oversized_metadata_is_rejected_even_with_false_zip_sizes() {
    let limit = usize::try_from(eve_sde::MAX_METADATA_BYTES).unwrap();
    let text = format!("{}{}", common::build_line(1), " ".repeat(limit));
    let mut bytes = common::zip_bytes(&[("_sde.jsonl", &text)]);
    assert!(matches!(
        open(bytes.clone()),
        Err(Error::LimitExceeded { resource, limit })
            if resource == "build metadata" && limit == eve_sde::MAX_METADATA_BYTES
    ));
    // With a size of 0 in the headers, reading stops at the declared size.
    common::understate_sizes(&mut bytes);
    assert!(matches!(
        open(bytes),
        Err(Error::Read { file, source })
            if file == "_sde.jsonl" && source.kind() == std::io::ErrorKind::InvalidData
    ));
}

#[test]
fn the_record_limit_keeps_line_numbers_and_accepts_the_boundary() {
    let row = TYPES.lines().next().unwrap();
    let limit = usize::try_from(eve_sde::MAX_RECORD_BYTES).unwrap();
    let exact = format!("{row}{}\n", " ".repeat(limit - row.len() - 1));
    let mut archive = open(builder().rows::<Type>(exact).into_bytes()).unwrap();
    assert_eq!(
        archive.table::<Type>().unwrap()[&TypeId(34)].name,
        "Tritanium"
    );

    // A blank line must also stay within the limit.
    let oversized = format!("{row}\n{}\n", " ".repeat(limit));
    let mut archive = open(builder().rows::<Type>(oversized).into_bytes()).unwrap();
    assert!(matches!(
        archive.table::<Type>(),
        Err(Error::LimitExceeded { resource, limit })
            if resource == "`types.jsonl` line 2" && limit == eve_sde::MAX_RECORD_BYTES
    ));
}
