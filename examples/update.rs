//! Download the latest SDE if the file is missing or old.
//!
//! `cargo run --release --features download --example update -- sde.zip`

use eve_sde::download::{Client, Update};
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: update <sde.zip>");
        return ExitCode::from(2);
    };
    let client = Client::new("eve-sde-example/0.2");
    match client.update(&path) {
        Ok(Update::UpToDate(build)) => println!("{path} is up to date (build {build})"),
        Ok(Update::Updated {
            from: Some(from),
            to,
        }) => {
            println!("updated {path}: build {from} -> {to}");
        }
        Ok(Update::Updated { from: None, to }) => println!("downloaded build {to} to {path}"),
        Ok(other) => println!("{path}: {other:?}"),
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
