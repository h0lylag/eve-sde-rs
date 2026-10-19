//! One struct per SDE file, and the structs nested in them. Field names are
//! `snake_case` versions of CCP's, with a few renamed to read better. Text
//! fields hold the English text only.
//!
//! Every file's struct has an `id` field, which holds CCP's `_key`: one of
//! the [`ids`](crate::ids) types, a plain `u32`, or a string in the few files
//! keyed by UUIDs. Fields in the data that this crate does not know are
//! ignored.
//!
//! The structs are `#[non_exhaustive]`: a later version can add fields as CCP
//! adds them without breaking your code.

macro_rules! record {
    ($t:ident, $file:literal) => {
        impl crate::tables::Record for $t {
            const FILE: &'static str = $file;
            type Key = u32;
            fn key(&self) -> u32 {
                self.id
            }
        }
    };
    ($t:ident, $file:literal, String) => {
        impl crate::tables::Record for $t {
            const FILE: &'static str = $file;
            type Key = String;
            fn key(&self) -> String {
                self.id.clone()
            }
        }
    };
    ($t:ident, $file:literal, $key:ident) => {
        impl crate::tables::Record for $t {
            const FILE: &'static str = $file;
            type Key = crate::ids::$key;
            fn key(&self) -> crate::ids::$key {
                self.id
            }
        }
    };
}

mod character;
mod content;
mod cosmetic;
mod dogma;
mod industry;
mod map;
mod misc;
mod npc;
mod types;

pub use character::*;
pub use content::*;
pub use cosmetic::*;
pub use dogma::*;
pub use industry::*;
pub use map::*;
pub use misc::*;
pub use npc::*;
pub use types::*;
