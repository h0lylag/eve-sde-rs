//! Convert between type IDs and names.
//!
//! `cargo run --release --example type_name -- sde.zip 34 587 "Damage Control II"`

use eve_sde::Archive;
use eve_sde::ids::TypeId;
use eve_sde::model::Type;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: type_name <sde.zip> <type ID or name>...");
        return ExitCode::from(2);
    };
    match run(&path, args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &str, queries: impl Iterator<Item = String>) -> Result<(), eve_sde::Error> {
    let types = Archive::open(path)?.table::<Type>()?;
    for query in queries {
        if let Ok(id) = query.parse::<u32>() {
            match types.get(&TypeId(id)) {
                Some(ty) => println!("{id} -> {}", ty.name),
                None => println!("{id} -> not found"),
            }
            continue;
        }
        let key = query.trim().to_lowercase();
        let mut found: Vec<&Type> = types
            .values()
            .filter(|ty| ty.name.to_lowercase() == key)
            .collect();
        // Several types can share a name. List published ones first, then by
        // ID.
        found.sort_by_key(|ty| (!ty.published, ty.id));
        if found.is_empty() {
            println!("{query} -> not found");
        }
        for ty in found {
            let published = if ty.published { "" } else { " (unpublished)" };
            println!("{query} -> {}{published}", ty.id);
        }
    }
    Ok(())
}
