//! Find types and solar systems by name, reading one record at a time.
//!
//! `cargo run --release --example lookup -- sde.zip Jita`

use eve_sde::Archive;
use eve_sde::model::{Region, SolarSystem, Type};
use std::error::Error;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(path), Some(query)) = (args.next(), args.next()) else {
        eprintln!("usage: lookup <sde.zip> <name>");
        return ExitCode::from(2);
    };
    match run(&path, &query) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            let mut source = error.source();
            while let Some(cause) = source {
                eprintln!("  caused by: {cause}");
                source = cause.source();
            }
            ExitCode::FAILURE
        }
    }
}

fn run(path: &str, query: &str) -> Result<(), eve_sde::Error> {
    let query = query.trim().to_lowercase();
    let mut archive = Archive::open(path)?;
    println!("SDE build {}", archive.build().build_number);

    for ty in archive.records::<Type>()? {
        let ty = ty?;
        if ty.name.to_lowercase() == query {
            let published = if ty.published { "" } else { " (unpublished)" };
            println!("type {}: {}{published}", ty.id, ty.name);
        }
    }

    let regions = archive.table::<Region>()?;
    for system in archive.records::<SolarSystem>()? {
        let system = system?;
        if system.name.to_lowercase() == query {
            let region = regions
                .get(&system.region_id)
                .map_or("unknown region", |r| r.name.as_str());
            println!(
                "system {}: {}, {region}, security {} ({:?}, {:?})",
                system.id,
                system.name,
                system.security_display(),
                system.security_band(),
                system.space()
            );
        }
    }
    Ok(())
}
