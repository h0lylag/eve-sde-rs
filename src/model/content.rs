//! Missions, dungeons, campaigns and other game content.
//!
//! Deeply nested records that few programs read (military campaign
//! annotations, freelance job schemas) keep those parts as raw JSON.

use super::*;
use crate::de;
use serde_json::Value;
use std::collections::BTreeMap;

/// `missions.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Mission {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    pub has_standing_rewards: bool,
    pub expiration_time: Option<i64>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    #[serde(rename = "corporationID")]
    pub corporation_id: Option<u32>,
    #[serde(rename = "agentTypeID")]
    pub agent_type_id: Option<u32>,
    #[serde(rename = "initialAgentGiftTypeID")]
    pub initial_agent_gift_type_id: Option<u32>,
    pub initial_agent_gift_quantity: Option<i64>,
    /// Message key to English text.
    #[serde(default, deserialize_with = "de::keyed_en")]
    pub messages: BTreeMap<String, String>,
    /// Faction ID to standing change.
    #[serde(default, deserialize_with = "de::kv_map")]
    pub extra_standings: BTreeMap<u32, f64>,
    pub kill_mission: Option<KillMission>,
    pub courier_mission: Option<CourierMission>,
    pub mission_rewards: Option<MissionRewards>,
}
record!(Mission, "missions.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct KillMission {
    #[serde(rename = "dungeonID")]
    pub dungeon_id: Option<u32>,
    #[serde(rename = "objectiveTypeID")]
    pub objective_type_id: Option<u32>,
    pub objective_quantity: Option<i64>,
    pub drop_item_in_mission_container: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CourierMission {
    #[serde(rename = "objectiveTypeID")]
    pub objective_type_id: u32,
    pub objective_quantity: i64,
    pub objective_singleton: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MissionRewards {
    pub reward: Option<MissionReward>,
    pub bonus_reward: Option<MissionReward>,
    pub bonus_time_interval: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MissionReward {
    #[serde(rename = "rewardTypeID")]
    pub reward_type_id: Option<u32>,
    pub reward_quantity: Option<i64>,
}

/// `epicArcs.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct EpicArc {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    pub arc_restart_interval: i64,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    #[serde(rename = "iconID")]
    pub icon_id: u32,
    pub missions: Vec<EpicArcMission>,
}
record!(EpicArc, "epicArcs.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct EpicArcMission {
    #[serde(rename = "_key")]
    pub mission_id: u32,
    #[serde(rename = "agentID")]
    pub agent_id: u32,
    #[serde(default, rename = "nextMissions")]
    pub next_missions: Vec<u32>,
    #[serde(rename = "failMissionID")]
    pub fail_mission_id: Option<u32>,
}

/// `dungeons.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Dungeon {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(rename = "archetypeID")]
    pub archetype_id: u32,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub gameplay_description: Option<String>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    #[serde(default, rename = "allowedShipsList")]
    pub allowed_ships_list: Vec<u32>,
}
record!(Dungeon, "dungeons.jsonl");

/// `archetypes.jsonl`: dungeon archetypes.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct Archetype {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub title: Option<String>,
}
record!(Archetype, "archetypes.jsonl");

/// `mercenaryTacticalOperations.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MercenaryTacticalOperation {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    #[serde(rename = "dungeonID")]
    pub dungeon_id: u32,
    pub anarchy_impact: i64,
    pub development_impact: i64,
    pub infomorph_bonus: i64,
}
record!(
    MercenaryTacticalOperation,
    "mercenaryTacticalOperations.jsonl"
);

/// `militaryCampaigns.jsonl`. The key is a UUID string.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MilitaryCampaign {
    #[serde(rename = "_key")]
    pub id: String,
    #[serde(deserialize_with = "de::en")]
    pub title: String,
    #[serde(deserialize_with = "de::en")]
    pub subtitle: String,
    pub target_progress: i64,
    pub issuer: Issuer,
    /// Raw JSON: dozens of image paths and localized texts.
    pub annotations: Value,
}
record!(MilitaryCampaign, "militaryCampaigns.jsonl", String);

/// `militaryCampaignObjectives.jsonl`. The key is a UUID string.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MilitaryCampaignObjective {
    #[serde(rename = "_key")]
    pub id: String,
    #[serde(rename = "campaignID")]
    pub campaign_id: String,
    #[serde(deserialize_with = "de::en")]
    pub title: String,
    #[serde(deserialize_with = "de::en")]
    pub subtitle: String,
    pub career_path: String,
    pub content_tags: Vec<String>,
    pub target_progress: i64,
    pub max_progress_per_participant: i64,
    #[serde(rename = "presentingCharacterID")]
    pub presenting_character_id: u32,
    pub issuer: Issuer,
    /// Raw JSON: ISK, loyalty point and standing rewards.
    pub rewards: Value,
    /// Raw JSON.
    pub contribution_method_configuration: Value,
    /// Raw JSON.
    pub annotations: Option<Value>,
}
record!(
    MilitaryCampaignObjective,
    "militaryCampaignObjectives.jsonl",
    String
);

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct Issuer {
    #[serde(rename = "corporationID")]
    pub corporation_id: Option<u32>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
}

/// `freelanceJobSchemas.jsonl`: one record holding every job schema.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct FreelanceJobSchemas {
    #[serde(rename = "_key")]
    pub id: u32,
    /// Raw JSON: large, deeply nested and mostly UI text.
    #[serde(rename = "_value")]
    pub schemas: Vec<Value>,
}
record!(FreelanceJobSchemas, "freelanceJobSchemas.jsonl");

/// `notificationTypes.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct NotificationType {
    #[serde(rename = "_key")]
    pub id: u32,
    pub internal_name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub display_name: Option<String>,
}
record!(NotificationType, "notificationTypes.jsonl");

/// `accountingEntryTypes.jsonl`: wallet journal entry kinds.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AccountingEntryType {
    #[serde(rename = "_key")]
    pub id: u32,
    pub internal_name: String,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub journal_message: Option<String>,
}
record!(AccountingEntryType, "accountingEntryTypes.jsonl");
