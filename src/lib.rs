//! Read, look up and update EVE Online's Static Data Export (SDE).
//!
//! CCP publishes the SDE as a ZIP of JSON Lines files, one per table. This
//! crate reads it in two ways:
//!
//! - [`Sde`] holds the whole SDE, in memory or in a SQLite store, and looks
//!   things up: by name, along the item hierarchy, into dogma, across the
//!   map, and the names the SDE leaves out.
//! - [`Archive`] reads one table at a time, one record at a time with
//!   [`Archive::records`] or into a map with [`Archive::table`], for code
//!   that copies the SDE somewhere else.
//!
//! Each table's model is in [`model`]; [`tables`] lists them all.
//!
//! ```no_run
//! # #[cfg(feature = "archive")] {
//! use eve_sde::Sde;
//! use eve_sde::ids::TypeId;
//!
//! let sde = Sde::load("sde.zip")?;
//! let tritanium = sde.type_id("Tritanium")?; // Some(TypeId(34))
//! let frigate = sde.group_of(TypeId(587))?; // the Rifter's group
//! # }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ```no_run
//! # #[cfg(feature = "archive")] {
//! use eve_sde::Archive;
//! use eve_sde::ids::TypeId;
//! use eve_sde::model::{Group, Type};
//!
//! let mut archive = Archive::open("sde.zip")?;
//! let groups = archive.table::<Group>()?;
//! for ty in archive.records::<Type>()? {
//!     let ty = ty?;
//!     if ty.id == TypeId(587) {
//!         println!("{} is a {}", ty.name, groups[&ty.group_id].name);
//!     }
//! }
//! # }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Features
//!
//! - `archive` (default): [`Sde`] in memory, [`Archive`], the models and the
//!   table list. Text is English only.
//! - `download`: [`download`], which checks for new builds and downloads them.
//! - `sqlite`: [`sqlite`], a local SQLite copy of the whole SDE that [`Sde`]
//!   can look things up in, linked against the system's SQLite.
//!   `sqlite-bundled` compiles SQLite in.
//! - `test-support`: [`testing`], which builds synthetic archives for tests.
//!
//! With no features, only [`ids`], [`names`] and the [`Slot`] and
//! [`Hardpoint`] rules are available.

#![cfg_attr(docsrs, feature(doc_cfg))]

mod fitting;
pub mod ids;
pub mod names;

pub use fitting::{Hardpoint, Slot};

#[cfg(feature = "archive")]
mod archive;
#[cfg(feature = "archive")]
mod de;
#[cfg(feature = "archive")]
mod error;
#[cfg(feature = "archive")]
#[cfg_attr(docsrs, doc(cfg(feature = "archive")))]
#[expect(
    missing_docs,
    reason = "model fields mirror CCP's SDE fields one to one, and the struct docs \
              name each file; only fields whose meaning is not obvious have docs"
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the models mirror CCP's records, which hold many independent flags"
)]
pub mod model;
#[cfg(feature = "archive")]
mod sde;
#[cfg(feature = "archive")]
mod space;
#[cfg(feature = "archive")]
mod tables;

#[cfg(feature = "archive")]
#[cfg_attr(docsrs, doc(cfg(feature = "archive")))]
pub use archive::{
    Archive, BuildInfo, MAX_ARCHIVE_BYTES, MAX_METADATA_BYTES, MAX_RECORD_BYTES, Records,
};
#[cfg(feature = "archive")]
#[cfg_attr(docsrs, doc(cfg(feature = "archive")))]
pub use error::{Error, Result};
#[cfg(feature = "archive")]
#[cfg_attr(docsrs, doc(cfg(feature = "archive")))]
pub use sde::Sde;
#[cfg(feature = "archive")]
#[cfg_attr(docsrs, doc(cfg(feature = "archive")))]
pub use space::{SecurityBand, SpaceKind};
#[cfg(feature = "archive")]
#[cfg_attr(docsrs, doc(cfg(feature = "archive")))]
pub use tables::{Key, KeyKind, Record, TableInfo, TableVisitor, tables, visit_tables};

#[cfg(feature = "download")]
#[cfg_attr(docsrs, doc(cfg(feature = "download")))]
pub mod download;

#[cfg(feature = "sqlite")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
pub mod sqlite;

#[cfg(feature = "test-support")]
#[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
pub mod testing;
