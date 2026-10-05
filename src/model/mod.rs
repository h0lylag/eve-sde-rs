//! One struct per SDE file, and the structs nested in them. Field names are
//! snake_case versions of CCP's, with a few renamed to read better. Text
//! fields hold the English text only.
//!
//! Every file's struct has an `id` field, which holds CCP's `_key`. It is a
//! `u32` except in the few files keyed by strings. Fields in the data that
//! this crate does not know are ignored.
//!
//! The structs are `#[non_exhaustive]`: a later version can add fields as CCP
//! adds them without breaking your code. To make a small value type such as
//! [`Position`] yourself, use its `new` function.

use serde::Deserialize;

macro_rules! record {
    ($t:ident, $file:literal) => {
        impl crate::archive::Record for $t {
            const FILE: &'static str = $file;
            type Id = u32;
            fn id(&self) -> u32 {
                self.id
            }
        }
    };
    ($t:ident, $file:literal, String) => {
        impl crate::archive::Record for $t {
            const FILE: &'static str = $file;
            type Id = String;
            fn id(&self) -> String {
                self.id.clone()
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

/// A point in meters.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Position {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Position { x, y, z }
    }
}

/// A point on the 2D map.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct Position2D {
    pub x: f64,
    pub y: f64,
}

impl Position2D {
    pub fn new(x: f64, y: f64) -> Self {
        Position2D { x, y }
    }
}

/// A color, each channel from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Rgb {
    pub fn new(r: f64, g: f64, b: f64) -> Self {
        Rgb { r, g, b }
    }
}

/// A color with alpha, each channel from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Rgba { r, g, b, a }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct TypeQuantity {
    #[serde(rename = "typeID")]
    pub type_id: u32,
    pub quantity: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct SkillLevel {
    #[serde(rename = "typeID")]
    pub type_id: u32,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct AttributeRef {
    #[serde(rename = "dogmaAttributeID")]
    pub attribute_id: u32,
}
