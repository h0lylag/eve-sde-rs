//! NPC corporations, characters, stations and factions.

use super::*;
use crate::de;
use std::collections::BTreeMap;

/// `npcStations.jsonl`. The file has no names, but
/// [`Sde::station_name`](crate::Sde::station_name) derives them.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct NpcStation {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "ownerID")]
    pub owner_id: u32,
    #[serde(rename = "typeID")]
    pub type_id: u32,
    #[serde(rename = "operationID")]
    pub operation_id: u32,
    /// The planet, moon or star the station orbits.
    #[serde(rename = "orbitID")]
    pub orbit_id: u32,
    /// The number of the planet the station orbits, directly or through a
    /// moon.
    pub celestial_index: Option<u32>,
    /// The moon's number, if the station orbits a moon.
    pub orbit_index: Option<u32>,
    pub position: Position,
    pub use_operation_name: bool,
    pub reprocessing_efficiency: f64,
    pub reprocessing_hangar_flag: i32,
    pub reprocessing_stations_take: f64,
}
record!(NpcStation, "npcStations.jsonl");

/// `npcCorporations.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct NpcCorporation {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    pub ticker_name: String,
    pub deleted: bool,
    pub extent: String,
    pub size: String,
    pub size_factor: Option<f64>,
    pub has_player_personnel_manager: bool,
    pub send_char_termination_message: bool,
    pub unique_name: bool,
    pub initial_price: i64,
    pub member_limit: i64,
    pub minimum_join_standing: i64,
    pub min_security: f64,
    pub shares: i64,
    pub tax_rate: f64,
    #[serde(rename = "ceoID")]
    pub ceo_id: Option<u32>,
    #[serde(rename = "stationID")]
    pub station_id: Option<u32>,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: Option<u32>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    #[serde(rename = "raceID")]
    pub race_id: Option<u32>,
    #[serde(rename = "friendID")]
    pub friend_id: Option<u32>,
    #[serde(rename = "enemyID")]
    pub enemy_id: Option<u32>,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    #[serde(rename = "mainActivityID")]
    pub main_activity_id: Option<u32>,
    #[serde(rename = "secondaryActivityID")]
    pub secondary_activity_id: Option<u32>,
    #[serde(default, rename = "allowedMemberRaces")]
    pub allowed_member_races: Vec<u32>,
    #[serde(default, rename = "lpOfferTables")]
    pub lp_offer_tables: Vec<u32>,
    /// Type ID to CCP's supply and demand value for it, which can be
    /// negative.
    #[serde(default, deserialize_with = "de::kv_map")]
    pub corporation_trades: BTreeMap<u32, f64>,
    /// Investor corporation ID to its percentage of the shares.
    #[serde(default, deserialize_with = "de::kv_map")]
    pub investors: BTreeMap<u32, i64>,
    #[serde(default, deserialize_with = "de::kv_map")]
    pub exchange_rates: BTreeMap<u32, f64>,
    #[serde(default)]
    pub divisions: Vec<CorporationDivision>,
}
record!(NpcCorporation, "npcCorporations.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CorporationDivision {
    #[serde(rename = "_key")]
    pub division_id: u32,
    pub division_number: u32,
    #[serde(rename = "leaderID")]
    pub leader_id: u32,
    pub size: i64,
}

/// `npcCorporationDivisions.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct NpcCorporationDivision {
    #[serde(rename = "_key")]
    pub id: u32,
    pub internal_name: String,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    pub display_name: Option<String>,
    #[serde(deserialize_with = "de::en")]
    pub leader_type_name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
}
record!(NpcCorporationDivision, "npcCorporationDivisions.jsonl");

/// `npcCharacters.jsonl`: agents, corporation CEOs and other NPCs.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct NpcCharacter {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(rename = "corporationID")]
    pub corporation_id: u32,
    #[serde(rename = "bloodlineID")]
    pub bloodline_id: u32,
    #[serde(rename = "raceID")]
    pub race_id: u32,
    /// `true` for male, `false` for female.
    pub gender: bool,
    pub ceo: bool,
    pub unique_name: bool,
    #[serde(rename = "locationID")]
    pub location_id: Option<u32>,
    pub start_date: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "ancestryID")]
    pub ancestry_id: Option<u32>,
    #[serde(rename = "careerID")]
    pub career_id: Option<u32>,
    #[serde(rename = "schoolID")]
    pub school_id: Option<u32>,
    #[serde(rename = "specialityID")]
    pub speciality_id: Option<u32>,
    #[serde(default)]
    pub skills: Vec<NpcSkill>,
    pub agent: Option<Agent>,
}
record!(NpcCharacter, "npcCharacters.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct NpcSkill {
    #[serde(rename = "typeID")]
    pub type_id: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Agent {
    #[serde(rename = "agentTypeID")]
    pub agent_type_id: u32,
    #[serde(rename = "divisionID")]
    pub division_id: u32,
    pub is_locator: bool,
    pub level: u8,
}

/// `agentTypes.jsonl`. `name` is CCP's internal name, e.g. `BasicAgent`.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct AgentType {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
}
record!(AgentType, "agentTypes.jsonl");

/// `agentsInSpace.jsonl`: agents (the `id`) found in a dungeon in space
/// instead of a station.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AgentInSpace {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "typeID")]
    pub type_id: u32,
    #[serde(rename = "dungeonID")]
    pub dungeon_id: u32,
    #[serde(rename = "spawnPointID")]
    pub spawn_point_id: u32,
}
record!(AgentInSpace, "agentsInSpace.jsonl");

/// `factions.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Faction {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub short_description: Option<String>,
    pub unique_name: bool,
    pub size_factor: f64,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
    #[serde(rename = "corporationID")]
    pub corporation_id: Option<u32>,
    #[serde(rename = "militiaCorporationID")]
    pub militia_corporation_id: Option<u32>,
    #[serde(rename = "memberRaces")]
    pub member_races: Vec<u32>,
    #[serde(rename = "iconID")]
    pub icon_id: u32,
    pub flat_logo: Option<String>,
    pub flat_logo_with_name: Option<String>,
}
record!(Faction, "factions.jsonl");

/// `stationOperations.jsonl`: the "Assembly Plant" part of a station's name.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct StationOperation {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub operation_name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    #[serde(rename = "activityID")]
    pub activity_id: u32,
    pub border: f64,
    pub corridor: f64,
    pub fringe: f64,
    pub hub: f64,
    pub manufacturing_factor: f64,
    pub ratio: f64,
    pub research_factor: f64,
    /// The [`StationService`] IDs on offer.
    pub services: Vec<u32>,
    /// Race ID to station type ID.
    #[serde(default, deserialize_with = "de::kv_map")]
    pub station_types: BTreeMap<u32, u32>,
}
record!(StationOperation, "stationOperations.jsonl");

/// `stationServices.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct StationService {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub service_name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
}
record!(StationService, "stationServices.jsonl");

/// `stationStandingsRestrictions.jsonl`: the standing needed for each station
/// service of a faction (the `id`).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct StationStandingsRestriction {
    #[serde(rename = "_key")]
    pub id: u32,
    /// Service ID to minimum standing.
    #[serde(deserialize_with = "de::kv_map")]
    pub services: BTreeMap<u32, f64>,
}
record!(
    StationStandingsRestriction,
    "stationStandingsRestrictions.jsonl"
);
