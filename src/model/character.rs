//! Character creation, skills, certificates and corporation roles.

use super::*;
use crate::de;
use std::collections::BTreeMap;

/// `races.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Race {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    #[serde(rename = "shipTypeID")]
    pub ship_type_id: Option<u32>,
    /// Starting skill type to level.
    #[serde(default, deserialize_with = "de::kv_map")]
    pub skills: BTreeMap<u32, u32>,
}
record!(Race, "races.jsonl");

/// `bloodlines.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Bloodline {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    #[serde(rename = "raceID")]
    pub race_id: u32,
    #[serde(rename = "corporationID")]
    pub corporation_id: u32,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    pub charisma: i32,
    pub intelligence: i32,
    pub memory: i32,
    pub perception: i32,
    pub willpower: i32,
}
record!(Bloodline, "bloodlines.jsonl");

/// `ancestries.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Ancestry {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    pub short_description: Option<String>,
    #[serde(rename = "bloodlineID")]
    pub bloodline_id: u32,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    pub charisma: i32,
    pub intelligence: i32,
    pub memory: i32,
    pub perception: i32,
    pub willpower: i32,
}
record!(Ancestry, "ancestries.jsonl");

/// `schools.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct School {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub character_description: Option<String>,
    #[serde(rename = "careerID")]
    pub career_id: u32,
    #[serde(rename = "corporationID")]
    pub corporation_id: u32,
    #[serde(rename = "raceID")]
    pub race_id: u32,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    pub is_starter_space_school: Option<bool>,
    #[serde(default, rename = "careerAgents")]
    pub career_agents: Vec<u32>,
    #[serde(default, rename = "startingStations")]
    pub starting_stations: Vec<u32>,
}
record!(School, "schools.jsonl");

/// `schoolMap.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SchoolMap {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "schoolID")]
    pub school_id: u32,
    #[serde(rename = "solarSystemID")]
    pub solar_system_id: u32,
}
record!(SchoolMap, "schoolMap.jsonl");

/// `characterAttributes.jsonl`: Intelligence, Memory and so on.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CharacterAttribute {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    pub description: String,
    pub short_description: String,
    pub notes: String,
    #[serde(rename = "iconID")]
    pub icon_id: u32,
}
record!(CharacterAttribute, "characterAttributes.jsonl");

/// `characterTitles.jsonl`. The key is a UUID string.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct CharacterTitle {
    #[serde(rename = "_key")]
    pub id: String,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
}
record!(CharacterTitle, "characterTitles.jsonl", String);

/// `cloneGrades.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct CloneGrade {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    pub skills: Vec<SkillLevel>,
}
record!(CloneGrade, "cloneGrades.jsonl");

/// `certificates.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Certificate {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    #[serde(rename = "groupID")]
    pub group_id: u32,
    #[serde(default, rename = "recommendedFor")]
    pub recommended_for: Vec<u32>,
    pub skill_types: Vec<CertificateSkill>,
}
record!(Certificate, "certificates.jsonl");

/// Skill levels needed for each certificate grade.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct CertificateSkill {
    #[serde(rename = "_key")]
    pub type_id: u32,
    pub basic: u8,
    pub standard: u8,
    pub improved: u8,
    pub advanced: u8,
    pub elite: u8,
}

/// `skillPlans.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SkillPlan {
    #[serde(rename = "_key")]
    pub id: u32,
    pub internal_name: String,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    #[serde(rename = "careerPathID")]
    pub career_path_id: Option<u32>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    pub npc_corporation_division: Option<u32>,
    pub milestones: Vec<Milestone>,
    pub skill_requirements: Vec<SkillLevel>,
}
record!(SkillPlan, "skillPlans.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct Milestone {
    #[serde(rename = "typeID")]
    pub type_id: u32,
    pub level: Option<u8>,
}

/// `expertSystems.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ExpertSystem {
    #[serde(rename = "_key")]
    pub id: u32,
    pub internal_name: String,
    pub duration_days: i64,
    pub hidden: bool,
    pub retired: bool,
    pub skills_granted: Vec<SkillLevel>,
    #[serde(default, rename = "associatedShipTypes")]
    pub associated_ship_types: Vec<u32>,
}
record!(ExpertSystem, "expertSystems.jsonl");

/// `corporationActivities.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct CorporationActivity {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
}
record!(CorporationActivity, "corporationActivities.jsonl");

/// `corporationRoles.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CorporationRole {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    pub short_name: String,
    #[serde(default, rename = "roleGroupIDs")]
    pub role_group_ids: Vec<u32>,
}
record!(CorporationRole, "corporationRoles.jsonl");

/// `corporationRoleGroups.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CorporationRoleGroup {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    pub applies_to: String,
    pub applies_to_grantable: String,
    pub is_divisional: bool,
    pub is_locational: bool,
}
record!(CorporationRoleGroup, "corporationRoleGroups.jsonl");
