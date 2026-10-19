//! `cargo xtask`: generates eve-sde's models from the SDE.
//!
//! - `cargo xtask schema <sde.zip>` infers the schema of every table in an
//!   SDE ZIP and writes it to `schema/`.
//! - `cargo xtask models` writes `src/model/` and `src/tables/list.rs` from
//!   `schema/` and `schema/settings.toml`. With `--check` it only reports the
//!   files that are out of date, and fails if there are any.

mod generate;
mod schema;
mod settings;

use generate::Mode;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask schema <sde.zip>\n       cargo xtask models [--check]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let root = root();
    let result = match args.as_slice() {
        ["schema", zip] => {
            schema::write(Path::new(zip), &root.join("schema")).map(|()| ExitCode::SUCCESS)
        }
        ["models"] => generate::run(&root, Mode::Write),
        ["models", "--check"] => generate::run(&root, Mode::Check),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// The repository root: the folder that holds `xtask/`.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}
