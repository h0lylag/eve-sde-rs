#![cfg(feature = "load")]
mod common;

use common::sde_zip;
use eve_sde::{Error, Sde, SecurityBand, Slot, SpaceKind};

const TYPES: &str = r#"{"_key":34,"groupID":18,"name":{"en":"Tritanium","de":"Tritanium"},"portionSize":1,"published":true,"marketGroupID":3}
{"_key":587,"groupID":25,"name":{"en":"Rifter"},"portionSize":1,"published":true,"marketGroupID":2}
{"_key":2046,"groupID":60,"name":{"en":"Damage Control II"},"portionSize":1,"published":true}
{"_key":900,"groupID":18,"name":{"en":"Spiked Quafe"},"portionSize":1,"published":false}
{"_key":901,"groupID":18,"name":{"en":"Spiked Quafe"},"portionSize":1,"published":true}
{"_key":902,"groupID":18,"name":{"en":"Spiked Quafe"},"portionSize":1,"published":true}"#;
const GROUPS: &str = r#"{"_key":18,"anchorable":false,"anchored":false,"categoryID":4,"fittableNonSingleton":false,"name":{"en":"Mineral"},"published":true,"useBasePrice":false}
{"_key":25,"anchorable":false,"anchored":false,"categoryID":6,"fittableNonSingleton":false,"name":{"en":"Frigate"},"published":true,"useBasePrice":false}
{"_key":60,"anchorable":false,"anchored":false,"categoryID":7,"fittableNonSingleton":false,"name":{"en":"Damage Control"},"published":true,"useBasePrice":false}"#;
const CATEGORIES: &str = r#"{"_key":4,"name":{"en":"Material"},"published":true}
{"_key":6,"name":{"en":"Ship"},"published":true}
{"_key":7,"name":{"en":"Module"},"published":true}"#;
const MARKET_GROUPS: &str = r#"{"_key":1,"hasTypes":false,"name":{"en":"Ships"}}
{"_key":2,"hasTypes":true,"name":{"en":"Frigates"},"parentGroupID":1}
{"_key":3,"hasTypes":true,"name":{"en":"Minerals"}}"#;
const ATTRIBUTES: &str = r#"{"_key":14,"name":"hiSlots","dataType":0,"defaultValue":0.0,"displayWhenZero":false,"highIsGood":true,"published":true,"stackable":true}"#;
const EFFECTS: &str = r#"{"_key":11,"name":"loPower","disallowAutoRepeat":false,"effectCategoryID":0,"electronicChance":false,"isAssistance":false,"isOffensive":false,"isWarpSafe":false,"propulsionChance":false,"published":false,"rangeChance":false}
{"_key":12,"name":"hiPower","disallowAutoRepeat":false,"effectCategoryID":0,"electronicChance":false,"isAssistance":false,"isOffensive":false,"isWarpSafe":false,"propulsionChance":false,"published":false,"rangeChance":false}"#;
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
        row(30000001, "Alpha", 0.9, "[50000001]"),
        row(30000002, "Beta", 0.03, "[50000002]"),
        row(31000001, "J100001", -1.0, "[]"),
    ]
    .join("\n")
}

fn stargates() -> String {
    let row = |id: u32, from: u32, to: u32, back: u32| {
        format!(
            r#"{{"_key":{id},"destination":{{"solarSystemID":{to},"stargateID":{back}}},"position":{POS},"solarSystemID":{from},"typeID":29624}}"#
        )
    };
    [
        row(50000001, 30000001, 30000002, 50000002),
        row(50000002, 30000002, 30000001, 50000001),
    ]
    .join("\n")
}

fn planets() -> String {
    let row = |id: u32, index: u32, unique: &str| {
        format!(
            r#"{{"_key":{id},"solarSystemID":30000001,"typeID":11,"celestialIndex":{index},"orbitID":40000001,"position":{POS},"radius":1.0,"statistics":{{"density":1,"eccentricity":0,"escapeVelocity":1,"locked":false,"massDust":1,"rotationRate":1,"spectralClass":"G","temperature":1}},"attributes":{{"heightMap1":1,"heightMap2":1,"shaderPreset":1,"population":false}}{unique}}}"#
        )
    };
    [
        row(40000002, 4, ""),
        row(40000005, 5, r#","uniqueName":{"en":"Alpha V (Prime)"}"#),
    ]
    .join("\n")
}

fn moons() -> String {
    let row = |id: u32, planet: u32, index: u32, orbit: u32| {
        format!(
            r#"{{"_key":{id},"solarSystemID":30000001,"typeID":14,"celestialIndex":{index},"orbitIndex":{orbit},"orbitID":{planet},"position":{POS},"radius":1.0,"attributes":{{"heightMap1":1,"heightMap2":1,"shaderPreset":1}}}}"#
        )
    };
    [row(40000003, 40000002, 4, 2), row(40000006, 40000005, 5, 1)].join("\n")
}

fn belts() -> String {
    let row = |id: u32, planet: u32, index: u32| {
        format!(
            r#"{{"_key":{id},"solarSystemID":30000001,"typeID":15,"celestialIndex":{index},"orbitIndex":1,"orbitID":{planet},"position":{POS}}}"#
        )
    };
    [row(40000004, 40000002, 4), row(40000007, 40000005, 5)].join("\n")
}

fn stations() -> String {
    let row = |id: u32, orbit: u32, op_name: bool| {
        format!(
            r#"{{"_key":{id},"operationID":1,"orbitID":{orbit},"ownerID":1000001,"position":{POS},"reprocessingEfficiency":0.5,"reprocessingHangarFlag":4,"reprocessingStationsTake":0.05,"solarSystemID":30000001,"typeID":1531,"useOperationName":{op_name}}}"#
        )
    };
    [
        row(60000001, 40000003, true),
        row(60000002, 40000002, false),
    ]
    .join("\n")
}

const CORPS: &str = r#"{"_key":1000001,"deleted":false,"extent":"national","hasPlayerPersonnelManager":false,"initialPrice":1,"memberLimit":-1,"minSecurity":0.0,"minimumJoinStanding":0,"name":{"en":"Caldari Navy"},"sendCharTerminationMessage":true,"shares":1,"size":"large","taxRate":0.1,"tickerName":"CN","uniqueName":true}"#;
const OPERATIONS: &str = r#"{"_key":1,"activityID":1,"border":0.0,"corridor":0.0,"fringe":0.0,"hub":0.0,"manufacturingFactor":1.0,"operationName":{"en":"Assembly Plant"},"ratio":1.0,"researchFactor":1.0,"services":[]}"#;

fn fixture() -> Sde {
    let systems = systems();
    let stargates = stargates();
    let planets = planets();
    let moons = moons();
    let belts = belts();
    let stations = stations();
    let files = [
        ("types.jsonl", TYPES),
        ("groups.jsonl", GROUPS),
        ("categories.jsonl", CATEGORIES),
        ("marketGroups.jsonl", MARKET_GROUPS),
        ("dogmaAttributes.jsonl", ATTRIBUTES),
        ("dogmaEffects.jsonl", EFFECTS),
        ("typeDogma.jsonl", TYPE_DOGMA),
        ("mapSolarSystems.jsonl", &systems),
        ("mapStargates.jsonl", &stargates),
        ("mapPlanets.jsonl", &planets),
        ("mapMoons.jsonl", &moons),
        ("mapAsteroidBelts.jsonl", &belts),
        ("npcStations.jsonl", &stations),
        ("npcCorporations.jsonl", CORPS),
        ("stationOperations.jsonl", OPERATIONS),
    ];
    Sde::from_bytes(&sde_zip(&files, None)).unwrap()
}

#[test]
fn reads_build_and_tables() {
    let sde = fixture();
    assert_eq!(sde.build().build_number, 3569502);
    assert_eq!(
        sde.build().release_date.as_deref(),
        Some("2026-10-02T11:08:57Z")
    );
    assert_eq!(sde.types().len(), 6);
    assert_eq!(sde.types()[&34].name, "Tritanium");
    assert!(sde.unmodeled_files().is_empty());
}

#[test]
fn looks_up_names_both_ways() {
    let sde = fixture();
    assert_eq!(sde.type_id("  tRiTaNiUm "), Some(34));
    assert_eq!(sde.type_name(587), Some("Rifter"));
    assert_eq!(sde.type_id("Nothing"), None);
    assert_eq!(sde.system_id("beta"), Some(30000002));
    assert_eq!(sde.attribute_id("hiSlots"), Some(14));
    assert_eq!(sde.name(30000001).as_deref(), Some("Alpha"));
    assert_eq!(sde.name(98000001), None);
}

#[test]
fn duplicate_type_names_prefer_published_then_lowest_id() {
    let sde = fixture();
    assert_eq!(sde.type_ids("Spiked Quafe"), [901, 902, 900]);
    assert_eq!(sde.type_id("Spiked Quafe"), Some(901));
}

#[test]
fn joins_items_to_groups_and_market() {
    let sde = fixture();
    assert_eq!(sde.group_of(587).unwrap().name, "Frigate");
    assert_eq!(sde.category_of(587).unwrap().name, "Ship");
    let path: Vec<_> = sde
        .market_path(587)
        .iter()
        .map(|g| g.name.as_str())
        .collect();
    assert_eq!(path, ["Ships", "Frigates"]);
    assert!(sde.market_path(2046).is_empty());
    assert_eq!(sde.types_in_group(18), [34, 900, 901, 902]);
    assert_eq!(sde.groups_in_category(6), [25]);
}

#[test]
fn reads_dogma() {
    let sde = fixture();
    assert_eq!(sde.attribute(587, 14), Some(3.0));
    assert_eq!(sde.attribute_named(587, "hiSlots"), Some(3.0));
    // A type without the attribute gets its default. An unknown type gets
    // nothing.
    assert_eq!(sde.attribute(34, 14), Some(0.0));
    assert_eq!(sde.attribute(99999, 14), None);
    assert_eq!(sde.attributes(587).count(), 1);
    assert_eq!(sde.slot(587), Some(Slot::High));
    assert_eq!(sde.slot(2046), Some(Slot::Low));
    assert_eq!(sde.slot(34), None);
    assert_eq!(sde.hardpoint(587), None);
}

#[test]
fn joins_systems_and_gates() {
    let sde = fixture();
    assert_eq!(sde.neighbors(30000001), [30000002]);
    assert_eq!(sde.neighbors(30000002), [30000001]);
    assert!(sde.neighbors(31000001).is_empty());
    assert!(sde.neighbors(1).is_empty());
    let gate = sde.gates(30000001).next().unwrap();
    assert_eq!(gate.id, 50000001);
    assert_eq!(sde.name(gate.id).as_deref(), Some("Stargate (Beta)"));

    let alpha = &sde.solar_systems()[&30000001];
    assert_eq!(alpha.space(), SpaceKind::KnownSpace);
    assert_eq!(alpha.security_band(), SecurityBand::High);
    assert_eq!(alpha.security_display(), "0.9");
    let beta = &sde.solar_systems()[&30000002];
    assert_eq!(beta.security_band(), SecurityBand::Low);
    assert_eq!(beta.security_display(), "0.1");
    let wormhole = &sde.solar_systems()[&31000001];
    assert_eq!(wormhole.space(), SpaceKind::Wormhole);
    assert_eq!(wormhole.security_band(), SecurityBand::Null);
    assert_eq!(wormhole.security_display(), "-1.0");
}

#[test]
fn builds_celestial_and_station_names() {
    let sde = fixture();
    assert_eq!(sde.name(40000002).as_deref(), Some("Alpha IV"));
    assert_eq!(sde.name(40000003).as_deref(), Some("Alpha IV - Moon 2"));
    assert_eq!(
        sde.station_name(60000001),
        Some("Alpha IV - Moon 2 - Caldari Navy Assembly Plant")
    );
    assert_eq!(sde.station_name(60000002), Some("Alpha IV - Caldari Navy"));
    assert_eq!(sde.station_id("alpha iv - caldari navy"), Some(60000002));
    assert_eq!(
        sde.name(60000002).as_deref(),
        Some("Alpha IV - Caldari Navy")
    );
}

#[test]
fn moons_and_belts_are_named_after_their_planet() {
    let sde = fixture();
    assert_eq!(
        sde.name(40000004).as_deref(),
        Some("Alpha IV - Asteroid Belt 1")
    );
    // The SDE names this planet but not its moon or belt.
    assert_eq!(sde.name(40000005).as_deref(), Some("Alpha V (Prime)"));
    assert_eq!(
        sde.name(40000006).as_deref(),
        Some("Alpha V (Prime) - Moon 1")
    );
    assert_eq!(
        sde.name(40000007).as_deref(),
        Some("Alpha V (Prime) - Asteroid Belt 1")
    );
}

#[test]
fn missing_table_names_the_file() {
    let bytes = sde_zip(&[], Some("mapMoons.jsonl"));
    match Sde::from_bytes(&bytes) {
        Err(Error::MissingFile(name)) => assert_eq!(name, "mapMoons.jsonl"),
        other => panic!("{:?}", other.err()),
    }
}

#[test]
fn missing_sde_file_error_names_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sde.zip");
    let err = Sde::load(&path).unwrap_err();
    let Error::Io(io) = &err else {
        panic!("{err:?}");
    };
    assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(err.to_string(), format!("cannot open `{}`", path.display()));
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn damaged_file_error_names_the_file() {
    let metadata = common::build_line(1);
    let mut files = vec![("_sde.jsonl", metadata.as_str())];
    for &name in eve_sde::modeled_files() {
        let body = if name == "types.jsonl" { TYPES } else { "" };
        files.push((name, body));
    }
    for (file, text) in [("_sde.jsonl", "buildNumber"), ("types.jsonl", "Tritanium")] {
        // Uncompressed, so the text appears as is. Changing the case of one
        // letter keeps the data valid but breaks its checksum.
        let mut bytes = common::stored_zip_bytes(&files);
        let at = bytes
            .windows(text.len())
            .position(|w| w == text.as_bytes())
            .unwrap();
        bytes[at] ^= 0x20;
        match Sde::from_bytes(&bytes) {
            Err(Error::Read { file: name, source }) => {
                assert_eq!(name, file);
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
            }
            other => panic!("{file}: {:?}", other.err()),
        }
    }
}

#[test]
fn parse_error_names_file_and_line() {
    let types = format!("{}\n\nnot json\n", TYPES.lines().next().unwrap());
    let bytes = sde_zip(&[("types.jsonl", &types)], None);
    match Sde::from_bytes(&bytes) {
        Err(Error::Parse { file, line, .. }) => {
            assert_eq!(file, "types.jsonl");
            assert_eq!(line, 3);
        }
        other => panic!("{:?}", other.err()),
    }
}

#[test]
fn missing_required_field_is_an_error() {
    let bytes = sde_zip(&[("types.jsonl", r#"{"_key":1,"groupID":1}"#)], None);
    assert!(matches!(Sde::from_bytes(&bytes), Err(Error::Parse { .. })));
}

#[test]
fn repeated_key_is_an_error() {
    let row = TYPES.lines().next().unwrap();
    let types = format!("{row}\n{row}");
    let bytes = sde_zip(&[("types.jsonl", &types)], None);
    assert!(matches!(
        Sde::from_bytes(&bytes),
        Err(Error::DuplicateKey { .. })
    ));
}

#[test]
fn unknown_fields_are_ignored_and_new_files_listed() {
    let types = r#"{"_key":1,"groupID":1,"name":{"en":"x","xx":"y"},"portionSize":1,"published":true,"brandNew":42}"#;
    let bytes = sde_zip(&[("types.jsonl", types), ("newTable.jsonl", "{}")], None);
    let sde = Sde::from_bytes(&bytes).unwrap();
    assert_eq!(sde.types().len(), 1);
    assert_eq!(sde.unmodeled_files(), ["newTable.jsonl"]);
}

#[test]
fn rejects_bad_build_metadata() {
    for body in [
        "",
        "not json",
        r#"{"_key":"nope","buildNumber":1}"#,
        r#"{"_key":"sde","buildNumber":0}"#,
    ] {
        let bytes = common::zip_bytes(&[("_sde.jsonl", body)]);
        assert!(Sde::from_bytes(&bytes).is_err(), "{body:?}");
    }
    assert!(matches!(Sde::from_bytes(b"not a zip"), Err(Error::Zip(_))));
}

#[test]
fn oversized_metadata_is_rejected_even_with_false_zip_sizes() {
    let text = format!(
        "{}{}",
        common::build_line(1),
        " ".repeat(eve_sde::MAX_METADATA_BYTES as usize)
    );
    let mut bytes = common::zip_bytes(&[("_sde.jsonl", &text)]);
    for understate in [false, true] {
        if understate {
            common::understate_sizes(&mut bytes);
        }
        assert!(matches!(
            Sde::from_bytes(&bytes),
            Err(Error::LimitExceeded { resource, limit })
                if resource == "build metadata" && limit == eve_sde::MAX_METADATA_BYTES
        ));
    }
}

#[test]
fn record_limit_preserves_line_numbers_and_accepts_the_boundary() {
    let row = TYPES.lines().next().unwrap();
    let limit = eve_sde::MAX_RECORD_BYTES as usize;
    let exact = format!("{row}{}\n", " ".repeat(limit - row.len() - 1));
    let bytes = sde_zip(&[("types.jsonl", &exact)], None);
    assert_eq!(
        Sde::from_bytes(&bytes).unwrap().type_name(34),
        Some("Tritanium")
    );

    // A blank line must also stay within the limit.
    let oversized = format!("{row}\n{}\n", " ".repeat(limit));
    let bytes = sde_zip(&[("types.jsonl", &oversized)], None);
    assert!(matches!(
        Sde::from_bytes(&bytes),
        Err(Error::LimitExceeded { resource, limit })
            if resource == "`types.jsonl` line 2" && limit == eve_sde::MAX_RECORD_BYTES
    ));
}
