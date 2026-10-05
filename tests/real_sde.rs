//! Runs against a real SDE ZIP. Skipped unless asked for:
//! `EVE_SDE_ZIP=/path/to/sde.zip cargo test --release -- --ignored --nocapture`
#![cfg(feature = "load")]

use eve_sde::Sde;
use std::time::Instant;

fn load() -> Sde {
    let path = std::env::var("EVE_SDE_ZIP").expect("set EVE_SDE_ZIP to an SDE ZIP");
    let start = Instant::now();
    let sde = Sde::load(path).unwrap();
    println!(
        "loaded build {} in {:?}",
        sde.build().build_number,
        start.elapsed()
    );
    sde
}

fn peak_memory_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

#[test]
#[ignore]
fn loads_everything() {
    let sde = load();
    assert!(
        sde.unmodeled_files().is_empty(),
        "{:?}",
        sde.unmodeled_files()
    );
    println!("peak memory: {:?} KiB", peak_memory_kib());
}

#[test]
#[ignore]
fn answers_known_questions() {
    use eve_sde::{SecurityBand, Slot, SpaceKind};
    let sde = load();

    assert_eq!(sde.type_id("tritanium"), Some(34));
    assert_eq!(sde.type_name(34), Some("Tritanium"));
    let jita = sde.system_id("Jita").unwrap();
    assert_eq!(jita, 30000142);
    assert_eq!(sde.name(jita).as_deref(), Some("Jita"));
    assert_eq!(sde.region_id("The Forge"), Some(10000002));
    assert_eq!(sde.faction_id("Caldari State"), Some(500001));

    // Several types share this name. All are listed, a published one first.
    let quafe = sde.type_ids("Spiked Quafe");
    assert!(quafe.len() > 1);
    assert!(sde.types()[&quafe[0]].published);

    assert!(
        sde.neighbors(jita)
            .contains(&sde.system_id("Perimeter").unwrap())
    );
    assert_eq!(sde.gates(jita).count(), sde.neighbors(jita).len());
    let gate = sde.gates(jita).next().unwrap();
    assert!(sde.name(gate.id).unwrap().starts_with("Stargate ("));
    assert_eq!(
        sde.station_name(60003760),
        Some("Jita IV - Moon 4 - Caldari Navy Assembly Plant")
    );
    assert_eq!(
        sde.station_id("jita iv - moon 4 - caldari navy assembly plant"),
        Some(60003760)
    );

    assert_eq!(sde.solar_systems()[&jita].space(), SpaceKind::KnownSpace);
    assert_eq!(
        sde.solar_systems()[&jita].security_band(),
        SecurityBand::High
    );
    assert_eq!(sde.solar_systems()[&jita].security_display(), "0.9");
    let wormholes = sde
        .solar_systems()
        .values()
        .filter(|s| s.space() == SpaceKind::Wormhole);
    assert!(wormholes.clone().count() > 2000);
    assert!(
        wormholes
            .into_iter()
            .all(|s| sde.neighbors(s.id).is_empty())
    );

    let rifter = sde.type_id("Rifter").unwrap();
    assert_eq!(rifter, 587);
    assert_eq!(sde.group_of(rifter).unwrap().name, "Frigate");
    assert_eq!(sde.category_of(rifter).unwrap().name, "Ship");
    assert_eq!(sde.attribute_named(rifter, "hiSlots"), Some(3.0));
    assert!(sde.market_path(rifter).len() >= 2);
    let dc = sde.type_id("Damage Control II").unwrap();
    assert_eq!(sde.slot(dc), Some(Slot::Low));
    assert!(
        sde.types_in_group(sde.types()[&rifter].group_id)
            .contains(&rifter)
    );
}
