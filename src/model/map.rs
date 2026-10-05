//! The map from regions down to stargates, plus landmarks, sovereignty
//! resources and planetary industry schematics.

use super::*;
use crate::de;

/// `mapRegions.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Region {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    #[serde(rename = "constellationIDs")]
    pub constellation_ids: Vec<u32>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    #[serde(rename = "nebulaID")]
    pub nebula_id: u32,
    pub position: Position,
    #[serde(rename = "wormholeClassID")]
    pub wormhole_class_id: Option<u32>,
}
record!(Region, "mapRegions.jsonl");

/// `mapConstellations.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Constellation {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(rename = "regionID")]
    pub region_id: u32,
    #[serde(rename = "solarSystemIDs")]
    pub solar_system_ids: Vec<u32>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    pub position: Position,
    #[serde(rename = "wormholeClassID")]
    pub wormhole_class_id: Option<u32>,
}
record!(Constellation, "mapConstellations.jsonl");

/// `mapSolarSystems.jsonl`. Gate links are in [`Stargate`], and
/// [`Sde::neighbors`](crate::Sde::neighbors) joins them.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SolarSystem {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(rename = "constellationID")]
    pub constellation_id: u32,
    #[serde(rename = "regionID")]
    pub region_id: u32,
    pub security_status: f64,
    pub security_class: Option<String>,
    pub position: Position,
    #[serde(rename = "position2D")]
    pub position_2d: Option<Position2D>,
    pub radius: f64,
    pub luminosity: Option<f64>,
    #[serde(rename = "starID")]
    pub star_id: Option<u32>,
    #[serde(default, rename = "planetIDs")]
    pub planet_ids: Vec<u32>,
    #[serde(default, rename = "stargateIDs")]
    pub stargate_ids: Vec<u32>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    #[serde(rename = "wormholeClassID")]
    pub wormhole_class_id: Option<u32>,
    pub visual_effect: Option<String>,
    #[serde(default)]
    pub border: bool,
    #[serde(default)]
    pub corridor: bool,
    #[serde(default)]
    pub fringe: bool,
    #[serde(default)]
    pub hub: bool,
    #[serde(default)]
    pub international: bool,
    #[serde(default)]
    pub regional: bool,
    #[serde(default, rename = "disallowedAnchorCategories")]
    pub disallowed_anchor_categories: Vec<u32>,
    #[serde(default, rename = "disallowedAnchorGroups")]
    pub disallowed_anchor_groups: Vec<u32>,
}
record!(SolarSystem, "mapSolarSystems.jsonl");

/// `mapStargates.jsonl`. Each gate pair has one record per end.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Stargate {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "typeID")]
    pub type_id: u32,
    pub position: Position,
    pub destination: GateDestination,
}
record!(Stargate, "mapStargates.jsonl");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct GateDestination {
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "stargateID")]
    pub stargate_id: u32,
}

/// `mapStars.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Star {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "typeID")]
    pub type_id: u32,
    pub radius: f64,
    pub statistics: StarStatistics,
}
record!(Star, "mapStars.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct StarStatistics {
    pub age: f64,
    pub life: f64,
    pub luminosity: f64,
    pub spectral_class: String,
    pub temperature: f64,
}

/// Physical numbers shared by planets, moons and asteroid belts.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CelestialStatistics {
    pub density: f64,
    pub eccentricity: f64,
    pub escape_velocity: f64,
    pub locked: bool,
    pub mass_dust: f64,
    pub mass_gas: Option<f64>,
    pub orbit_period: Option<f64>,
    pub orbit_radius: Option<f64>,
    pub pressure: Option<f64>,
    pub rotation_rate: f64,
    pub spectral_class: String,
    pub surface_gravity: Option<f64>,
    pub temperature: f64,
}

/// Rendering settings of a planet or moon.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CelestialAttributes {
    pub height_map1: i64,
    pub height_map2: i64,
    pub shader_preset: i64,
    /// Planets only.
    pub population: Option<bool>,
}

/// `mapPlanets.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Planet {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "typeID")]
    pub type_id: u32,
    /// The planet's number in its system, from 1, as in Jita IV.
    pub celestial_index: u32,
    /// The star this planet orbits.
    #[serde(rename = "orbitID")]
    pub orbit_id: u32,
    pub position: Position,
    pub radius: f64,
    /// Set only for a few planets with special names.
    #[serde(default, deserialize_with = "de::en_opt")]
    pub unique_name: Option<String>,
    pub statistics: CelestialStatistics,
    pub attributes: CelestialAttributes,
    #[serde(default, rename = "moonIDs")]
    pub moon_ids: Vec<u32>,
    #[serde(default, rename = "asteroidBeltIDs")]
    pub asteroid_belt_ids: Vec<u32>,
    #[serde(default, rename = "npcStationIDs")]
    pub npc_station_ids: Vec<u32>,
}
record!(Planet, "mapPlanets.jsonl");

/// `mapMoons.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Moon {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "typeID")]
    pub type_id: u32,
    /// The number of the planet this moon orbits.
    pub celestial_index: u32,
    /// The moon's number around that planet, from 1.
    pub orbit_index: u32,
    /// The planet this moon orbits.
    #[serde(rename = "orbitID")]
    pub orbit_id: u32,
    pub position: Position,
    pub radius: f64,
    /// Set only for a few moons with special names.
    #[serde(default, deserialize_with = "de::en_opt")]
    pub unique_name: Option<String>,
    pub statistics: Option<CelestialStatistics>,
    pub attributes: CelestialAttributes,
    #[serde(default, rename = "npcStationIDs")]
    pub npc_station_ids: Vec<u32>,
}
record!(Moon, "mapMoons.jsonl");

/// `mapAsteroidBelts.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AsteroidBelt {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "typeID")]
    pub type_id: u32,
    /// The number of the planet this belt orbits.
    pub celestial_index: u32,
    /// The belt's number around that planet, from 1.
    pub orbit_index: u32,
    /// The planet this belt orbits.
    #[serde(rename = "orbitID")]
    pub orbit_id: u32,
    pub position: Position,
    pub radius: Option<f64>,
    /// Set only for a few belts with special names.
    #[serde(default, deserialize_with = "de::en_opt")]
    pub unique_name: Option<String>,
    pub statistics: Option<CelestialStatistics>,
}
record!(AsteroidBelt, "mapAsteroidBelts.jsonl");

/// `mapSecondarySuns.jsonl`: the second sun, such as a black hole or a
/// Wolf-Rayet star, that gives a wormhole system its effect.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SecondarySun {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "typeID")]
    pub type_id: u32,
    #[serde(rename = "effectBeaconTypeID")]
    pub effect_beacon_type_id: u32,
    pub position: Position,
}
record!(SecondarySun, "mapSecondarySuns.jsonl");

/// `landmarks.jsonl`: notable places on the map, such as the EVE Gate.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Landmark {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    pub position: Position,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    #[serde(rename = "locationID")]
    pub location_id: Option<u32>,
}
record!(Landmark, "landmarks.jsonl");

/// `planetResources.jsonl`: the power, workforce or reagent that a star or
/// planet (the `id`) gives sovereignty upgrades.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct PlanetResource {
    #[serde(rename = "_key")]
    pub id: u32,
    pub power: Option<i64>,
    pub workforce: Option<i64>,
    pub reagent: Option<PlanetReagent>,
}
record!(PlanetResource, "planetResources.jsonl");

// No `rename_all`: this part of the file already uses snake_case names.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct PlanetReagent {
    pub type_id: u32,
    pub amount_per_cycle: i64,
    pub cycle_period: i64,
    pub secured_capacity: i64,
    pub unsecured_capacity: i64,
}

/// `planetSchematics.jsonl`: planetary industry recipes.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PlanetSchematic {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    /// In seconds.
    pub cycle_time: i64,
    /// Structure types that can run this schematic.
    pub pins: Vec<u32>,
    pub types: Vec<SchematicType>,
}
record!(PlanetSchematic, "planetSchematics.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SchematicType {
    #[serde(rename = "_key")]
    pub type_id: u32,
    pub is_input: bool,
    pub quantity: i64,
}
