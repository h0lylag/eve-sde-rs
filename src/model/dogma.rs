//! Dogma: the attribute and effect system behind every item's numbers.

use super::*;
use crate::de;
use std::collections::BTreeMap;

/// `dogmaAttributes.jsonl`. `name` is the internal name, e.g. `hiSlots`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Attribute {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub display_name: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub tooltip_title: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub tooltip_description: Option<String>,
    #[serde(rename = "attributeCategoryID")]
    pub attribute_category_id: Option<u32>,
    pub data_type: i32,
    /// Used when a type does not list the attribute.
    pub default_value: f64,
    pub display_when_zero: bool,
    pub high_is_good: bool,
    pub published: bool,
    pub stackable: bool,
    #[serde(rename = "unitID")]
    pub unit_id: Option<u32>,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    #[serde(rename = "chargeRechargeTimeID")]
    pub charge_recharge_time_id: Option<u32>,
    #[serde(rename = "maxAttributeID")]
    pub max_attribute_id: Option<u32>,
    #[serde(rename = "minAttributeID")]
    pub min_attribute_id: Option<u32>,
}
record!(Attribute, "dogmaAttributes.jsonl");

/// `dogmaAttributeCategories.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct AttributeCategory {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
}
record!(AttributeCategory, "dogmaAttributeCategories.jsonl");

/// `dogmaEffects.jsonl`. `name` is the internal name, e.g. `hiPower`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Effect {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub display_name: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    pub guid: Option<String>,
    #[serde(rename = "effectCategoryID")]
    pub effect_category_id: u32,
    pub disallow_auto_repeat: bool,
    pub electronic_chance: bool,
    pub propulsion_chance: bool,
    pub range_chance: bool,
    pub is_assistance: bool,
    pub is_offensive: bool,
    pub is_warp_safe: bool,
    pub published: bool,
    pub distribution: Option<u32>,
    #[serde(rename = "dischargeAttributeID")]
    pub discharge_attribute_id: Option<u32>,
    #[serde(rename = "durationAttributeID")]
    pub duration_attribute_id: Option<u32>,
    #[serde(rename = "falloffAttributeID")]
    pub falloff_attribute_id: Option<u32>,
    #[serde(rename = "rangeAttributeID")]
    pub range_attribute_id: Option<u32>,
    #[serde(rename = "trackingSpeedAttributeID")]
    pub tracking_speed_attribute_id: Option<u32>,
    #[serde(rename = "resistanceAttributeID")]
    pub resistance_attribute_id: Option<u32>,
    #[serde(rename = "npcUsageChanceAttributeID")]
    pub npc_usage_chance_attribute_id: Option<u32>,
    #[serde(rename = "npcActivationChanceAttributeID")]
    pub npc_activation_chance_attribute_id: Option<u32>,
    #[serde(rename = "fittingUsageChanceAttributeID")]
    pub fitting_usage_chance_attribute_id: Option<u32>,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    #[serde(default)]
    pub modifier_info: Vec<ModifierInfo>,
}
record!(Effect, "dogmaEffects.jsonl");

/// One thing an effect changes.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ModifierInfo {
    pub domain: String,
    pub func: String,
    #[serde(rename = "modifiedAttributeID")]
    pub modified_attribute_id: Option<u32>,
    #[serde(rename = "modifyingAttributeID")]
    pub modifying_attribute_id: Option<u32>,
    pub operation: Option<i32>,
    #[serde(rename = "groupID")]
    pub group_id: Option<u32>,
    #[serde(rename = "skillTypeID")]
    pub skill_type_id: Option<u32>,
    #[serde(rename = "effectID")]
    pub effect_id: Option<u32>,
}

/// `dogmaUnits.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DogmaUnit {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub display_name: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
}
record!(DogmaUnit, "dogmaUnits.jsonl");

/// `dbuffCollections.jsonl`: warfare links and other area buffs.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DynamicBuff {
    #[serde(rename = "_key")]
    pub id: u32,
    pub aggregate_mode: String,
    pub developer_description: String,
    pub operation_name: String,
    #[serde(rename = "showOutputValueInUI")]
    pub show_output_value_in_ui: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub item_modifiers: Vec<AttributeRef>,
    #[serde(default)]
    pub location_modifiers: Vec<AttributeRef>,
    #[serde(default)]
    pub location_group_modifiers: Vec<GroupModifier>,
    #[serde(default)]
    pub location_required_skill_modifiers: Vec<SkillModifier>,
}
record!(DynamicBuff, "dbuffCollections.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct GroupModifier {
    #[serde(rename = "dogmaAttributeID")]
    pub attribute_id: u32,
    #[serde(rename = "groupID")]
    pub group_id: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct SkillModifier {
    #[serde(rename = "dogmaAttributeID")]
    pub attribute_id: u32,
    #[serde(rename = "skillID")]
    pub skill_id: u32,
}

/// `appliedProximityEffects.jsonl`: buffs that a type (the `id`), such as a
/// Stasis Field Effect Subpylon, applies to ships near it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AppliedProximityEffect {
    #[serde(rename = "_key")]
    pub id: u32,
    /// [`DynamicBuff`] ID to value.
    #[serde(deserialize_with = "de::kv_map")]
    pub dbuffs: BTreeMap<u32, f64>,
    pub delay_seconds: i64,
    pub radius: i64,
}
record!(AppliedProximityEffect, "appliedProximityEffects.jsonl");

/// `linkWithShip.jsonl`: what happens when a ship links with a type (the
/// `id`), such as a Skyhook Reagent Silo.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct LinkWithShip {
    #[serde(rename = "_key")]
    pub id: u32,
    /// [`DynamicBuff`] ID to value.
    #[serde(deserialize_with = "de::kv_map")]
    pub dbuffs: BTreeMap<u32, f64>,
    pub apply_pvp_flag: bool,
    pub can_relink: bool,
    pub character_energy_cost: Option<f64>,
    pub dbuff_post_link_duration: i64,
    pub generate_cyno_inhibitor: bool,
    pub keep_dbuff_duration_on_link_break: bool,
    pub link_duration: i64,
    #[serde(rename = "linkEffectGraphicIDOverride")]
    pub link_effect_graphic_id_override: i64,
    #[serde(rename = "linkableShipTypeListID")]
    pub linkable_ship_type_list_id: u32,
    pub max_link_range: i64,
    pub omega_only: bool,
    pub solarsystem_interference_cost: Option<f64>,
}
record!(LinkWithShip, "linkWithShip.jsonl");

/// `proximityTrap.jsonl`: a trap type (the `id`), such as the AEGIS Proximity
/// Mine.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ProximityTrap {
    #[serde(rename = "_key")]
    pub id: u32,
    /// [`DynamicBuff`] ID to value.
    #[serde(default, deserialize_with = "de::kv_map")]
    pub dbuffs: BTreeMap<u32, f64>,
    pub dbuff_duration: i64,
    pub show_perimeter_lights: bool,
    pub trigger_delay: i64,
    #[serde(rename = "triggerFilterTypeListID")]
    pub trigger_filter_type_list_id: u32,
    pub trigger_range: i64,
    pub force_decloak_duration: Option<i64>,
    pub reset_delay: Option<i64>,
}
record!(ProximityTrap, "proximityTrap.jsonl");

/// `systemWideEffects.jsonl`: an effect type (the `id`) that covers a whole
/// system, such as a wormhole effect.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SystemWideEffect {
    #[serde(rename = "_key")]
    pub id: u32,
    /// [`DynamicBuff`] ID to value.
    #[serde(default, deserialize_with = "de::kv_map")]
    pub dbuffs: BTreeMap<u32, f64>,
    #[serde(rename = "eligibleTypeListID")]
    pub eligible_type_list_id: Option<u32>,
    #[serde(rename = "environmentTypeID")]
    pub environment_type_id: Option<u32>,
}
record!(SystemWideEffect, "systemWideEffects.jsonl");

/// `systemDbuffEmitters.jsonl`: buffs that a type (the `id`) applies to a
/// whole system.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SystemDbuffEmitter {
    #[serde(rename = "_key")]
    pub id: u32,
    /// [`DynamicBuff`] ID to value.
    #[serde(deserialize_with = "de::kv_map")]
    pub dbuffs: BTreeMap<u32, f64>,
    pub duration: i64,
    pub exclude_protected: bool,
    pub interval: i64,
}
record!(SystemDbuffEmitter, "systemDbuffEmitters.jsonl");
