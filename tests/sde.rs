//! The lookups of `Sde`, run against each backend on one synthetic SDE.
#![expect(
    clippy::unwrap_used,
    reason = "test helpers fail by panicking, like the tests"
)]

use eve_sde::ids::{
    AttributeId, CategoryId, CelestialId, ConstellationId, CorporationId, EffectId, FactionId,
    GroupId, MarketGroupId, RegionId, StargateId, StationId, SystemId, TypeId,
};
use eve_sde::model::{
    AsteroidBelt, Attribute, Category, Constellation, Effect, Faction, Group, MarketGroup,
    MetaGroup, Moon, NpcCharacter, NpcCorporation, NpcStation, Planet, Region, SolarSystem, Star,
    Stargate, StationOperation, Type, TypeDogma,
};
use eve_sde::testing::ArchiveBuilder;
use eve_sde::{Archive, Hardpoint, Sde, Slot};
use std::io::Cursor;

const POS: &str = r#"{"x":0.0,"y":0.0,"z":0.0}"#;

/// A small SDE of build 7: some items and a corner of the map.
fn fixture() -> ArchiveBuilder {
    map(items(ArchiveBuilder::new(7)))
}

/// Types, their groups, categories and market groups, and their dogma.
fn items(builder: ArchiveBuilder) -> ArchiveBuilder {
    let group = |id: u32, category: u32, name: &str| {
        format!(
            r#"{{"_key":{id},"anchorable":false,"anchored":false,"categoryID":{category},"fittableNonSingleton":false,"name":{{"en":"{name}"}},"published":true,"useBasePrice":false}}"#
        )
    };
    let attribute = |id: u32, name: &str, default: f64| {
        format!(
            r#"{{"_key":{id},"dataType":4,"defaultValue":{default:?},"displayWhenZero":false,"highIsGood":true,"name":"{name}","published":true,"stackable":true}}"#
        )
    };
    let effect = |id: u32, name: &str| {
        format!(
            r#"{{"_key":{id},"disallowAutoRepeat":false,"effectCategoryID":0,"electronicChance":false,"isAssistance":false,"isOffensive":false,"isWarpSafe":false,"name":"{name}","propulsionChance":false,"published":false,"rangeChance":false}}"#
        )
    };
    builder
        .rows::<Type>(
            [
                r#"{"_key":34,"groupID":18,"name":{"en":"Tritanium","de":"Tritanium"},"portionSize":1,"published":true}"#,
                r#"{"_key":100,"groupID":25,"name":{"en":"Rifter"},"portionSize":1,"published":false}"#,
                r#"{"_key":587,"groupID":25,"name":{"en":"Rifter"},"portionSize":1,"published":true,"marketGroupID":61,"metaGroupID":1}"#,
                r#"{"_key":2046,"groupID":60,"name":{"en":"Damage Control II"},"portionSize":1,"published":true,"metaGroupID":2}"#,
                r#"{"_key":2873,"groupID":55,"name":{"en":"125mm Gatling AutoCannon I"},"portionSize":1,"published":true}"#,
            ]
            .join("\n"),
        )
        .rows::<Group>(
            [
                group(18, 4, "Mineral"),
                group(25, 6, "Frigate"),
                group(55, 7, "Projectile Weapon"),
                group(60, 7, "Damage Control"),
            ]
            .join("\n"),
        )
        .rows::<Category>(
            [
                r#"{"_key":4,"name":{"en":"Material"},"published":true}"#,
                r#"{"_key":6,"name":{"en":"Ship"},"published":true}"#,
                r#"{"_key":7,"name":{"en":"Module"},"published":true}"#,
            ]
            .join("\n"),
        )
        .rows::<MetaGroup>(
            [
                r#"{"_key":1,"name":{"en":"Tech I"}}"#,
                r#"{"_key":2,"name":{"en":"Tech II"}}"#,
            ]
            .join("\n"),
        )
        .rows::<MarketGroup>(
            [
                r#"{"_key":4,"hasTypes":false,"name":{"en":"Ships"}}"#,
                r#"{"_key":61,"hasTypes":true,"name":{"en":"Frigates"},"parentGroupID":4}"#,
            ]
            .join("\n"),
        )
        .rows::<Attribute>([attribute(14, "hiSlots", 0.0), attribute(50, "cpu", 5.0)].join("\n"))
        .rows::<Effect>(
            [
                effect(11, "loPower"),
                effect(12, "hiPower"),
                effect(42, "turretFitted"),
            ]
            .join("\n"),
        )
        .rows::<TypeDogma>(
            [
                r#"{"_key":587,"dogmaAttributes":[{"attributeID":14,"value":3.0}]}"#,
                r#"{"_key":2046,"dogmaEffects":[{"effectID":11,"isDefault":false}]}"#,
                r#"{"_key":2873,"dogmaEffects":[{"effectID":12,"isDefault":false},{"effectID":42,"isDefault":true}]}"#,
            ]
            .join("\n"),
        )
}

/// Jita and its neighbours, the celestials around Jita IV, and two stations.
fn map(builder: ArchiveBuilder) -> ArchiveBuilder {
    let system = |id: u32, name: &str, security: f64, gates: &str| {
        format!(
            r#"{{"_key":{id},"constellationID":20000020,"name":{{"en":"{name}"}},"position":{POS},"radius":1.0,"regionID":10000002,"securityStatus":{security:?},"stargateIDs":{gates}}}"#
        )
    };
    let gate = |id: u32, system: u32, to_system: u32, to_gate: u32| {
        format!(
            r#"{{"_key":{id},"destination":{{"solarSystemID":{to_system},"stargateID":{to_gate}}},"position":{POS},"solarSystemID":{system},"typeID":16}}"#
        )
    };
    let planet = |id: u32, index: u32, unique: &str| {
        format!(
            r#"{{"_key":{id},"solarSystemID":30000142,"typeID":11,"celestialIndex":{index},"orbitID":40000001,"position":{POS},"radius":1.0,"statistics":{{"density":1.0,"eccentricity":0.0,"escapeVelocity":1.0,"locked":false,"massDust":1.0,"rotationRate":1.0,"spectralClass":"G","temperature":1.0}},"attributes":{{"heightMap1":1,"heightMap2":1,"shaderPreset":1,"population":false}}{unique}}}"#
        )
    };
    let station = |id: u32, orbit: u32, operation_name: bool| {
        format!(
            r#"{{"_key":{id},"operationID":1,"orbitID":{orbit},"ownerID":1000035,"position":{POS},"reprocessingEfficiency":0.5,"reprocessingHangarFlag":4,"reprocessingStationsTake":0.05,"solarSystemID":30000142,"typeID":1531,"useOperationName":{operation_name}}}"#
        )
    };
    let corporation = |id: u32, deleted: bool| {
        format!(
            r#"{{"_key":{id},"deleted":{deleted},"extent":"national","hasPlayerPersonnelManager":false,"initialPrice":1,"memberLimit":-1,"minSecurity":0.0,"minimumJoinStanding":0,"name":{{"en":"Caldari Navy"}},"sendCharTerminationMessage":true,"shares":1,"size":"large","taxRate":0.1,"tickerName":"CN","uniqueName":true}}"#
        )
    };

    builder
        .rows::<Region>(format!(
            r#"{{"_key":10000002,"name":{{"en":"The Forge"}},"nebulaID":1,"position":{POS}}}"#
        ))
        .rows::<Constellation>(format!(
            r#"{{"_key":20000020,"name":{{"en":"Kimotoro"}},"position":{POS},"regionID":10000002}}"#
        ))
        .rows::<SolarSystem>(
            [
                system(30_000_142, "Jita", 0.9, "[50000001,50000002]"),
                system(30_000_144, "Perimeter", 0.9, "[50000003]"),
                system(30_000_139, "Urlen", 0.9, "[50000004]"),
                system(31_000_001, "J100001", -1.0, "[]"),
            ]
            .join("\n"),
        )
        .rows::<Stargate>(
            [
                gate(50_000_001, 30_000_142, 30_000_144, 50_000_003),
                gate(50_000_002, 30_000_142, 30_000_139, 50_000_004),
                gate(50_000_003, 30_000_144, 30_000_142, 50_000_001),
                gate(50_000_004, 30_000_139, 30_000_142, 50_000_002),
            ]
            .join("\n"),
        )
        .rows::<Star>(r#"{"_key":40000001,"solarSystemID":30000142,"typeID":6,"radius":1.0,"statistics":{"age":1.0,"life":1.0,"luminosity":1.0,"spectralClass":"G","temperature":1.0}}"#)
        .rows::<Planet>(
            [
                planet(40_000_002, 4, ""),
                planet(40_000_005, 5, r#","uniqueName":{"en":"Jita V (Prime)"}"#),
            ]
            .join("\n"),
        )
        .rows::<Moon>(format!(
            r#"{{"_key":40000003,"solarSystemID":30000142,"typeID":14,"celestialIndex":4,"orbitIndex":4,"orbitID":40000002,"position":{POS},"radius":1.0,"attributes":{{"heightMap1":1,"heightMap2":1,"shaderPreset":1}}}}"#
        ))
        .rows::<AsteroidBelt>(format!(
            r#"{{"_key":40000004,"solarSystemID":30000142,"typeID":15,"celestialIndex":4,"orbitIndex":1,"orbitID":40000002,"position":{POS}}}"#
        ))
        .rows::<NpcStation>(
            [
                station(60_003_760, 40_000_003, true),
                station(60_000_001, 40_000_001, false),
            ]
            .join("\n"),
        )
        .rows::<NpcCorporation>(
            [corporation(1_000_001, true), corporation(1_000_035, false)].join("\n"),
        )
        .rows::<StationOperation>(r#"{"_key":1,"activityID":1,"border":0.0,"corridor":0.0,"fringe":0.0,"hub":0.0,"manufacturingFactor":1.0,"operationName":{"en":"Assembly Plant"},"ratio":1.0,"researchFactor":1.0,"services":[]}"#)
        .rows::<Faction>(r#"{"_key":500001,"description":{"en":"x"},"iconID":1,"name":{"en":"Caldari State"},"sizeFactor":1.0,"solarSystemID":30000142,"uniqueName":true}"#)
        .rows::<NpcCharacter>(r#"{"_key":3000001,"bloodlineID":1,"ceo":false,"corporationID":1000035,"gender":true,"name":{"en":"Some Agent"},"raceID":1,"uniqueName":false}"#)
}

fn archive(builder: ArchiveBuilder) -> Archive<Cursor<Vec<u8>>> {
    Archive::from_reader(Cursor::new(builder.into_bytes())).unwrap()
}

/// The fixture in every backend, with the backend's name.
fn backends(builder: &ArchiveBuilder) -> Vec<(&'static str, Sde)> {
    vec![
        (
            "memory",
            Sde::from_archive(&mut archive(builder.clone())).unwrap(),
        ),
        #[cfg(feature = "sqlite")]
        (
            "sqlite",
            Sde::from(eve_sde::sqlite::Store::in_memory(&mut archive(builder.clone())).unwrap()),
        ),
    ]
}

#[test]
fn names_are_found_ignoring_case_and_surrounding_space() {
    for (backend, sde) in backends(&fixture()) {
        assert_eq!(
            sde.type_id("  tritanium ").unwrap(),
            Some(TypeId(34)),
            "{backend}"
        );
        assert_eq!(
            sde.group_id("FRIGATE").unwrap(),
            Some(GroupId(25)),
            "{backend}"
        );
        assert_eq!(
            sde.category_id("ship").unwrap(),
            Some(CategoryId(6)),
            "{backend}"
        );
        assert_eq!(
            sde.market_group_id("frigates").unwrap(),
            Some(MarketGroupId(61)),
            "{backend}"
        );
        assert_eq!(
            sde.system_id("jita").unwrap(),
            Some(SystemId(30_000_142)),
            "{backend}"
        );
        assert_eq!(
            sde.constellation_id("kimotoro").unwrap(),
            Some(ConstellationId(20_000_020)),
            "{backend}"
        );
        assert_eq!(
            sde.region_id("the forge").unwrap(),
            Some(RegionId(10_000_002)),
            "{backend}"
        );
        assert_eq!(
            sde.faction_id("caldari state").unwrap(),
            Some(FactionId(500_001)),
            "{backend}"
        );
        assert_eq!(
            sde.attribute_id("hiSlots").unwrap(),
            Some(AttributeId(14)),
            "{backend}"
        );
        assert_eq!(
            sde.effect_id("hipower").unwrap(),
            Some(EffectId(12)),
            "{backend}"
        );
        assert_eq!(
            sde.station_id("Jita IV - Moon 4 - Caldari Navy Assembly Plant")
                .unwrap(),
            Some(StationId(60_003_760)),
            "{backend}"
        );
        assert_eq!(sde.type_id("nothing").unwrap(), None, "{backend}");
        assert_eq!(sde.system_ids("nothing").unwrap(), [], "{backend}");
    }
}

#[test]
fn preferred_records_come_first_among_equal_names() {
    for (backend, sde) in backends(&fixture()) {
        // Published types first, although the unpublished one has a lower ID.
        assert_eq!(
            sde.type_ids("rifter").unwrap(),
            [TypeId(587), TypeId(100)],
            "{backend}"
        );
        assert_eq!(
            sde.type_id("rifter").unwrap(),
            Some(TypeId(587)),
            "{backend}"
        );
        // Corporations that are not deleted first.
        assert_eq!(
            sde.corporation_ids("caldari navy").unwrap(),
            [CorporationId(1_000_035), CorporationId(1_000_001)],
            "{backend}"
        );
    }
}

#[test]
fn types_join_their_group_category_meta_group_and_market_path() {
    for (backend, sde) in backends(&fixture()) {
        let rifter = TypeId(587);
        assert_eq!(
            sde.type_name(rifter).unwrap().unwrap(),
            "Rifter",
            "{backend}"
        );
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
            sde.meta_group_of(rifter).unwrap().unwrap().name,
            "Tech I",
            "{backend}"
        );
        assert!(
            sde.meta_group_of(TypeId(34)).unwrap().is_none(),
            "{backend}"
        );
        let path: Vec<String> = sde
            .market_path(rifter)
            .unwrap()
            .into_iter()
            .map(|group| group.name)
            .collect();
        assert_eq!(path, ["Ships", "Frigates"], "{backend}");
        assert_eq!(sde.market_path(TypeId(34)).unwrap(), [], "{backend}");
        assert!(sde.type_name(TypeId(1)).unwrap().is_none(), "{backend}");
        assert!(sde.group_of(TypeId(1)).unwrap().is_none(), "{backend}");
    }
}

#[test]
fn groups_list_their_types_and_categories_their_groups() {
    for (backend, sde) in backends(&fixture()) {
        assert_eq!(
            sde.types_in_group(GroupId(25)).unwrap(),
            [TypeId(100), TypeId(587)],
            "{backend}"
        );
        assert_eq!(
            sde.groups_in_category(CategoryId(7)).unwrap(),
            [GroupId(55), GroupId(60)],
            "{backend}"
        );
        assert_eq!(sde.types_in_group(GroupId(1)).unwrap(), [], "{backend}");
    }
}

#[test]
fn dogma_attributes_fall_back_to_their_default() {
    for (backend, sde) in backends(&fixture()) {
        let rifter = TypeId(587);
        assert_eq!(
            sde.attribute(rifter, AttributeId(14)).unwrap(),
            Some(3.0),
            "{backend}"
        );
        assert_eq!(
            sde.attribute(rifter, AttributeId(50)).unwrap(),
            Some(5.0),
            "{backend}"
        );
        assert_eq!(
            sde.attribute(TypeId(1), AttributeId(14)).unwrap(),
            None,
            "{backend}"
        );
        assert_eq!(
            sde.attribute(rifter, AttributeId(9)).unwrap(),
            None,
            "{backend}"
        );
        assert_eq!(
            sde.attribute_named(rifter, "hiSlots").unwrap(),
            Some(3.0),
            "{backend}"
        );
        assert_eq!(
            sde.attribute_named(rifter, "nothing").unwrap(),
            None,
            "{backend}"
        );
        let attributes: Vec<(String, f64)> = sde
            .attributes(rifter)
            .unwrap()
            .into_iter()
            .map(|(attribute, value)| (attribute.name, value))
            .collect();
        assert_eq!(attributes, [("hiSlots".to_owned(), 3.0)], "{backend}");
    }
}

#[test]
fn modules_have_effects_slots_and_hardpoints() {
    for (backend, sde) in backends(&fixture()) {
        let autocannon = TypeId(2873);
        let effects: Vec<(String, bool)> = sde
            .effects(autocannon)
            .unwrap()
            .into_iter()
            .map(|(effect, is_default)| (effect.name, is_default))
            .collect();
        assert_eq!(
            effects,
            [
                ("hiPower".to_owned(), false),
                ("turretFitted".to_owned(), true)
            ],
            "{backend}"
        );
        assert_eq!(sde.slot(autocannon).unwrap(), Some(Slot::High), "{backend}");
        assert_eq!(
            sde.hardpoint(autocannon).unwrap(),
            Some(Hardpoint::Turret),
            "{backend}"
        );
        assert_eq!(
            sde.slot(TypeId(2046)).unwrap(),
            Some(Slot::Low),
            "{backend}"
        );
        assert_eq!(sde.hardpoint(TypeId(2046)).unwrap(), None, "{backend}");
        assert_eq!(sde.slot(TypeId(587)).unwrap(), None, "{backend}");
    }
}

#[test]
fn systems_list_their_gates_and_neighbours() {
    for (backend, sde) in backends(&fixture()) {
        let jita = SystemId(30_000_142);
        assert_eq!(
            sde.neighbors(jita).unwrap(),
            [SystemId(30_000_139), SystemId(30_000_144)],
            "{backend}"
        );
        let gates: Vec<StargateId> = sde.gates(jita).unwrap().iter().map(|g| g.id).collect();
        assert_eq!(
            gates,
            [StargateId(50_000_001), StargateId(50_000_002)],
            "{backend}"
        );
        assert_eq!(
            sde.neighbors(SystemId(31_000_001)).unwrap(),
            [],
            "{backend}"
        );
        assert_eq!(sde.neighbors(SystemId(1)).unwrap(), [], "{backend}");
    }
}

#[test]
fn everything_with_an_id_has_its_in_game_name() {
    for (backend, sde) in backends(&fixture()) {
        let name = |id: u32| sde.name(id).unwrap();
        assert_eq!(name(10_000_002).as_deref(), Some("The Forge"), "{backend}");
        assert_eq!(name(20_000_020).as_deref(), Some("Kimotoro"), "{backend}");
        assert_eq!(name(30_000_142).as_deref(), Some("Jita"), "{backend}");
        assert_eq!(
            name(40_000_001).as_deref(),
            Some("Jita - Star"),
            "{backend}"
        );
        assert_eq!(name(40_000_002).as_deref(), Some("Jita IV"), "{backend}");
        assert_eq!(
            name(40_000_005).as_deref(),
            Some("Jita V (Prime)"),
            "{backend}"
        );
        assert_eq!(
            name(40_000_003).as_deref(),
            Some("Jita IV - Moon 4"),
            "{backend}"
        );
        assert_eq!(
            name(40_000_004).as_deref(),
            Some("Jita IV - Asteroid Belt 1"),
            "{backend}"
        );
        assert_eq!(
            name(50_000_001).as_deref(),
            Some("Stargate (Perimeter)"),
            "{backend}"
        );
        assert_eq!(
            name(60_003_760).as_deref(),
            Some("Jita IV - Moon 4 - Caldari Navy Assembly Plant"),
            "{backend}"
        );
        // A station that orbits the star starts with the system's name.
        assert_eq!(
            name(60_000_001).as_deref(),
            Some("Jita - Caldari Navy"),
            "{backend}"
        );
        assert_eq!(name(500_001).as_deref(), Some("Caldari State"), "{backend}");
        assert_eq!(
            name(1_000_035).as_deref(),
            Some("Caldari Navy"),
            "{backend}"
        );
        assert_eq!(name(3_000_001).as_deref(), Some("Some Agent"), "{backend}");
        assert_eq!(name(98_000_001), None, "{backend}");
        assert_eq!(name(587), None, "{backend}");
        assert_eq!(
            sde.celestial_name(CelestialId(40_000_003))
                .unwrap()
                .as_deref(),
            Some("Jita IV - Moon 4"),
            "{backend}"
        );
    }
}

#[test]
fn records_are_read_by_key() {
    for (backend, sde) in backends(&fixture()) {
        assert_eq!(sde.build().build_number, 7, "{backend}");
        let rifter = sde.get::<Type>(TypeId(587)).unwrap().unwrap();
        assert_eq!(rifter.name, "Rifter", "{backend}");
        assert!(sde.get::<Type>(&TypeId(1)).unwrap().is_none(), "{backend}");
    }
}

#[test]
fn tables_without_a_model_are_listed() {
    let builder = fixture().file("newThings.jsonl", r#"{"_key":1}"#);
    for (backend, sde) in backends(&builder) {
        assert_eq!(
            sde.unmodeled_files().unwrap(),
            ["newThings.jsonl"],
            "{backend}"
        );
    }
}

#[test]
fn threads_can_share_an_sde() {
    fn shared<T: Send + Sync>() {}
    shared::<Sde>();
}

#[cfg(feature = "sqlite")]
#[test]
fn a_store_in_a_folder_answers_the_same_lookups() {
    let dir = tempfile::tempdir().unwrap();
    eve_sde::sqlite::import(&mut archive(fixture()), dir.path()).unwrap();
    let sde = Sde::open_sqlite(dir.path()).unwrap();
    assert_eq!(sde.build().build_number, 7);
    assert_eq!(
        sde.station_id("jita iv - moon 4 - caldari navy assembly plant")
            .unwrap(),
        Some(StationId(60_003_760))
    );
    assert_eq!(
        sde.attribute_named(TypeId(587), "hiSlots").unwrap(),
        Some(3.0)
    );
}

#[cfg(feature = "sqlite")]
#[test]
fn a_store_without_station_names_fails_station_searches_only() {
    // As if CCP had changed the stars, which naming a station reads: this
    // star lacks fields its model requires.
    let builder = fixture().rows::<Star>(r#"{"_key":40000001,"typeID":6}"#);
    let dir = tempfile::tempdir().unwrap();
    let outcome = eve_sde::sqlite::import(&mut archive(builder), dir.path()).unwrap();
    assert_eq!(outcome, eve_sde::sqlite::ImportOutcome::Imported);
    let sde = Sde::open_sqlite(dir.path()).unwrap();
    let search = sde.station_id("Jita IV - Moon 4 - Caldari Navy Assembly Plant");
    assert!(
        matches!(&search, Err(eve_sde::Error::Invalid(message))
            if message.starts_with("the store has no station names: ")),
        "{search:?}"
    );
    // Naming the station reads the star, so it fails like reading the star.
    assert!(matches!(
        sde.name(60_000_001),
        Err(eve_sde::Error::Decode { table, .. }) if table == "stars"
    ));
    // Lookups that do not need the stars still answer.
    assert_eq!(sde.system_id("Jita").unwrap(), Some(SystemId(30_000_142)));
    assert_eq!(sde.type_id("Rifter").unwrap(), Some(TypeId(587)));
}
