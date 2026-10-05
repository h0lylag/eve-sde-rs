//! Load EVE Online's Static Data Export (SDE) and look things up in it.
//!
//! ```no_run
//! # #[cfg(feature = "load")] {
//! use eve_sde::Sde;
//!
//! let sde = Sde::load("sde.zip")?;
//! let id = sde.type_id("Tritanium").unwrap();
//! assert_eq!(sde.type_name(id), Some("Tritanium"));
//!
//! let jita = sde.system_id("Jita").unwrap();
//! for next in sde.neighbors(jita) {
//!     println!("Jita → {}", sde.name(*next).unwrap());
//! }
//! # }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The `load` feature (on by default) reads CCP's JSON Lines ZIP, every table,
//! English text only. The `download` feature checks for new builds and
//! downloads them. With neither feature, only [`ids`] is available.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod ids;

#[cfg(any(feature = "load", feature = "download"))]
mod archive;
#[cfg(any(feature = "load", feature = "download"))]
mod error;

#[cfg(any(feature = "load", feature = "download"))]
pub use archive::{BuildInfo, MAX_ARCHIVE_BYTES, MAX_METADATA_BYTES, MAX_RECORD_BYTES};
#[cfg(any(feature = "load", feature = "download"))]
pub use error::{Error, Result};

#[cfg(feature = "load")]
mod de;
#[cfg(feature = "load")]
mod lookup;
#[cfg(feature = "load")]
pub mod model;
#[cfg(feature = "load")]
mod names;
#[cfg(feature = "load")]
mod sde;
#[cfg(feature = "load")]
mod space;

#[cfg(feature = "load")]
pub use lookup::{Hardpoint, Slot};
#[cfg(feature = "load")]
pub use sde::{Sde, modeled_files};
#[cfg(feature = "load")]
pub use space::{SecurityBand, SpaceKind};

#[cfg(feature = "download")]
pub mod download;
