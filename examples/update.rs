//! Download the latest SDE if the file is missing or old.
//!
//! `cargo run --release --features download --example update -- sde.zip`

use eve_sde::download::{Client, Update};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: update <sde.zip>");
        std::process::exit(2);
    };
    let client = Client::new("eve-sde-example/0.1");
    match client.update(&path)? {
        Update::UpToDate(build) => println!("{path} is up to date (build {build})"),
        Update::Updated { from, to } => match from {
            Some(from) => println!("updated {path}: build {from} -> {to}"),
            None => println!("downloaded build {to} to {path}"),
        },
        other => println!("{path}: {other:?}"),
    }
    Ok(())
}
