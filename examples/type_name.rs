//! Convert between type IDs and names.
//!
//! `cargo run --release --example type_name -- sde.zip 34 587 "Damage Control II"`

use eve_sde::Sde;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: type_name <sde.zip> <type ID or name>...");
        std::process::exit(2);
    };
    let sde = Sde::load(path)?;

    for query in args {
        if let Ok(id) = query.parse::<u32>() {
            match sde.type_name(id) {
                Some(name) => println!("{id} -> {name}"),
                None => println!("{id} -> not found"),
            }
        } else {
            let ids = sde.type_ids(&query);
            if ids.is_empty() {
                println!("{query} -> not found");
            }
            // `type_ids` lists the published item first.
            for id in ids {
                let published = if sde.types()[id].published {
                    ""
                } else {
                    " (unpublished)"
                };
                println!("{query} -> {id}{published}");
            }
        }
    }
    Ok(())
}
