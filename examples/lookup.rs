//! Look up a name, or describe an ID.
//!
//! `cargo run --release --example lookup -- sde.zip Jita`
//! `cargo run --release --example lookup -- sde.zip 587`

use eve_sde::Sde;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(path), Some(query)) = (args.next(), args.next()) else {
        eprintln!("usage: lookup <sde.zip> <name or ID>");
        std::process::exit(2);
    };
    let sde = Sde::load(path)?;
    println!("SDE build {}", sde.build().build_number);

    match query.parse::<u32>() {
        Ok(id) => describe(&sde, id),
        Err(_) => {
            let found = [
                ("type", sde.type_ids(&query)),
                ("system", sde.system_ids(&query)),
                ("constellation", sde.constellation_ids(&query)),
                ("region", sde.region_ids(&query)),
                ("station", sde.station_ids(&query)),
                ("faction", sde.faction_ids(&query)),
                ("corporation", sde.corporation_ids(&query)),
            ];
            for (what, ids) in found {
                for id in ids {
                    println!("{what} {id}");
                }
            }
        }
    }
    Ok(())
}

fn describe(sde: &Sde, id: u32) {
    if let Some(t) = sde.types().get(&id) {
        println!("type {id}: {}", t.name);
        if let Some(g) = sde.group_of(id) {
            println!("  group {}: {}", g.id, g.name);
        }
        if let Some(c) = sde.category_of(id) {
            println!("  category {}: {}", c.id, c.name);
        }
        if let Some(slot) = sde.slot(id) {
            println!("  slot: {slot:?}");
        }
        for (attribute, value) in sde.attributes(id) {
            println!("  {} = {value}", attribute.name);
        }
    }
    if let Some(name) = sde.name(id) {
        println!("{id}: {name}");
    }
    if let Some(system) = sde.solar_systems().get(&id) {
        let region = sde.name(system.region_id).unwrap_or_default();
        println!(
            "  {region}, security {} ({:?}, {:?})",
            system.security_display(),
            system.security_band(),
            system.space()
        );
        for next in sde.neighbors(id) {
            println!("  gate to {}", sde.name(*next).unwrap_or_default());
        }
    }
}
