//! Everything else: fighters, sovereignty, control towers, contraband.

use super::*;
use crate::de;

/// `fighterAbilities.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FighterAbility {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub display_name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub tooltip_text: Option<String>,
    pub target_mode: String,
    pub disallow_in_high_sec: bool,
    pub disallow_in_low_sec: bool,
    #[serde(rename = "iconID")]
    pub icon_id: u32,
    #[serde(rename = "turretGraphicID")]
    pub turret_graphic_id: Option<u32>,
}
record!(FighterAbility, "fighterAbilities.jsonl");

/// `fighterAbilitiesByType.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FighterAbilitiesByType {
    #[serde(rename = "_key")]
    pub id: u32,
    pub ability_slot0: AbilitySlot,
    pub ability_slot1: AbilitySlot,
    pub ability_slot2: Option<AbilitySlot>,
}
record!(FighterAbilitiesByType, "fighterAbilitiesByType.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilitySlot {
    #[serde(rename = "abilityID")]
    pub ability_id: u32,
    pub cooldown_seconds: Option<i64>,
    pub charges: Option<AbilityCharges>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityCharges {
    pub charge_count: i64,
    pub rearm_time_seconds: i64,
}

/// `sovereigntyUpgrades.jsonl`. Field names are snake_case in the source.
#[derive(Debug, Clone, Deserialize)]
pub struct SovereigntyUpgrade {
    #[serde(rename = "_key")]
    pub id: u32,
    pub mutually_exclusive_group: String,
    pub fuel: Option<SovereigntyFuel>,
    pub power_allocation: Option<i64>,
    pub workforce_allocation: Option<i64>,
    pub power_production: Option<i64>,
    pub workforce_production: Option<i64>,
}
record!(SovereigntyUpgrade, "sovereigntyUpgrades.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SovereigntyFuel {
    pub type_id: u32,
    pub hourly_upkeep: i64,
    pub startup_cost: i64,
}

/// `metenoxMoonDrill.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetenoxMoonDrill {
    #[serde(rename = "_key")]
    pub id: u32,
    pub mining_cycle_time: i64,
    pub mining_efficiency: f64,
    pub reagents_consumed_per_cycle: i64,
}
record!(MetenoxMoonDrill, "metenoxMoonDrill.jsonl");

/// `controlTowerResources.jsonl`: fuel use of each control tower.
#[derive(Debug, Clone, Deserialize)]
pub struct ControlTowerResources {
    #[serde(rename = "_key")]
    pub id: u32,
    pub resources: Vec<TowerResource>,
}
record!(ControlTowerResources, "controlTowerResources.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TowerResource {
    #[serde(rename = "resourceTypeID")]
    pub resource_type_id: u32,
    pub purpose: u32,
    pub quantity: i64,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    pub min_security_level: Option<f64>,
}

/// `contrabandTypes.jsonl`
#[derive(Debug, Clone, Deserialize)]
pub struct ContrabandType {
    #[serde(rename = "_key")]
    pub id: u32,
    pub factions: Vec<ContrabandFaction>,
}
record!(ContrabandType, "contrabandTypes.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContrabandFaction {
    #[serde(rename = "_key")]
    pub faction_id: u32,
    pub attack_min_sec: f64,
    pub confiscate_min_sec: f64,
    pub fine_by_value: f64,
    pub standing_loss: f64,
}

/// `translationLanguages.jsonl`: `en`, `de`, … and their names.
#[derive(Debug, Clone, Deserialize)]
pub struct TranslationLanguage {
    #[serde(rename = "_key")]
    pub id: String,
    pub name: String,
}
record!(TranslationLanguage, "translationLanguages.jsonl", String);
