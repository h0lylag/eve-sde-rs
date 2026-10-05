//! Icons, graphics, SKINs, the SKINR designer and the ship tree.

use super::*;
use crate::de;
use std::collections::BTreeMap;

/// `icons.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Icon {
    #[serde(rename = "_key")]
    pub id: u32,
    pub icon_file: String,
}
record!(Icon, "icons.jsonl");

/// `graphics.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Graphic {
    #[serde(rename = "_key")]
    pub id: u32,
    pub graphic_file: Option<String>,
    pub icon_folder: Option<String>,
    pub sof_faction_name: Option<String>,
    pub sof_hull_name: Option<String>,
    pub sof_race_name: Option<String>,
    #[serde(rename = "sofMaterialSetID")]
    pub sof_material_set_id: Option<u32>,
    #[serde(default)]
    pub sof_layout: Vec<String>,
}
record!(Graphic, "graphics.jsonl");

/// `graphicMaterialSets.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct GraphicMaterialSet {
    #[serde(rename = "_key")]
    pub id: u32,
    pub description: String,
    pub color_hull: Option<Rgba>,
    pub color_primary: Option<Rgba>,
    pub color_secondary: Option<Rgba>,
    pub color_window: Option<Rgba>,
    pub sof_faction_name: Option<String>,
    pub sof_race_hint: Option<String>,
    pub sof_pattern_name: Option<String>,
    pub res_path_insert: Option<String>,
    pub material1: Option<String>,
    pub material2: Option<String>,
    pub material3: Option<String>,
    pub material4: Option<String>,
    pub custommaterial1: Option<String>,
    pub custommaterial2: Option<String>,
}
record!(GraphicMaterialSet, "graphicMaterialSets.jsonl");

/// `skins.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Skin {
    #[serde(rename = "_key")]
    pub id: u32,
    pub internal_name: String,
    #[serde(rename = "skinMaterialID")]
    pub skin_material_id: u32,
    /// Ship types this SKIN fits.
    pub types: Vec<u32>,
    #[serde(rename = "allowCCPDevs")]
    pub allow_ccp_devs: bool,
    pub visible_serenity: bool,
    pub visible_tranquility: bool,
    #[serde(default)]
    pub is_structure_skin: bool,
}
record!(Skin, "skins.jsonl");

/// `skinLicenses.jsonl`: the item you buy to get a SKIN.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SkinLicense {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "licenseTypeID")]
    pub license_type_id: u32,
    #[serde(rename = "skinID")]
    pub skin_id: u32,
    /// In days, or -1 for permanent.
    pub duration: i64,
    #[serde(default)]
    pub is_single_use: bool,
}
record!(SkinLicense, "skinLicenses.jsonl");

/// `skinMaterials.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SkinMaterial {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub display_name: Option<String>,
    #[serde(rename = "materialSetID")]
    pub material_set_id: u32,
}
record!(SkinMaterial, "skinMaterials.jsonl");

/// `skinrComponentCategories.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SkinrComponentCategory {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
}
record!(SkinrComponentCategory, "skinrComponentCategories.jsonl");

/// `skinrComponentPointValues.jsonl`: the points of a component category (the
/// `id`).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SkinrComponentPointValues {
    #[serde(rename = "_key")]
    pub id: u32,
    /// Rarity ID to points.
    #[serde(rename = "_value", deserialize_with = "de::kv_map")]
    pub values: BTreeMap<u32, i64>,
}
record!(SkinrComponentPointValues, "skinrComponentPointValues.jsonl");

/// `skinrComponentRarities.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SkinrComponentRarity {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    pub rank: i64,
}
record!(SkinrComponentRarity, "skinrComponentRarities.jsonl");

/// `skinrComponents.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SkinrComponent {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    pub category: u32,
    pub rarity: u32,
    pub finish: String,
    pub published: bool,
    pub icon_file: String,
    pub resource_file: String,
    pub projection_type_u: String,
    pub projection_type_v: String,
    #[serde(rename = "associatedTypeIds")]
    pub associated_types: Vec<AssociatedType>,
    pub sequence_binder: SequenceBinder,
}
record!(SkinrComponent, "skinrComponents.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AssociatedType {
    #[serde(rename = "typeID")]
    pub type_id: u32,
    pub license_uses_granted: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SequenceBinder {
    #[serde(rename = "itemTypeID")]
    pub item_type_id: u32,
    pub count: i64,
}

/// `skinrSlotCategories.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SkinrSlotCategory {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
}
record!(SkinrSlotCategory, "skinrSlotCategories.jsonl");

/// `skinrSlotConfigurations.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SkinrSlotConfiguration {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    pub priority: i64,
    #[serde(default)]
    pub allow_all_ships: bool,
    #[serde(default)]
    pub config: Vec<u32>,
    #[serde(default)]
    pub ships: Vec<u32>,
}
record!(SkinrSlotConfiguration, "skinrSlotConfigurations.jsonl");

/// `skinrSlotNames.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SkinrSlotName {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
}
record!(SkinrSlotName, "skinrSlotNames.jsonl");

/// `skinrSlots.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SkinrSlot {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    pub category: u32,
    pub allowed_design_component_categories: Vec<u32>,
}
record!(SkinrSlot, "skinrSlots.jsonl");

/// `skinrSlotsToMaterials.jsonl`: the material in each SKINR slot for a
/// faction (the `id`).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SkinrSlotsToMaterials {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "_value")]
    pub bindings: Vec<SlotMaterial>,
}
record!(SkinrSlotsToMaterials, "skinrSlotsToMaterials.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct SlotMaterial {
    #[serde(rename = "slotID")]
    pub slot_id: u32,
    #[serde(rename = "materialID")]
    pub material_id: u32,
}

/// `skinrTierThresholds.jsonl`: the tier thresholds of a ship tree group (the
/// `id`).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SkinrTierThreshold {
    #[serde(rename = "_key")]
    pub id: u32,
    /// Tier, from 1, to its threshold.
    #[serde(rename = "_value", deserialize_with = "de::kv_map")]
    pub thresholds: BTreeMap<u32, i64>,
}
record!(SkinrTierThreshold, "skinrTierThresholds.jsonl");

/// `shipTreeElements.jsonl`: ship tree tags, such as Small, Combat or Shields.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ShipTreeElement {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    pub icon: String,
}
record!(ShipTreeElement, "shipTreeElements.jsonl");

/// `shipTreeFactions.jsonl`: the ship tree entry of a faction (the `id`).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ShipTreeFaction {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub description: String,
    pub icon: String,
    /// Order, from 1, to [`ShipTreeElement`] ID.
    #[serde(deserialize_with = "de::kv_map")]
    pub elements: BTreeMap<u32, u32>,
}
record!(ShipTreeFaction, "shipTreeFactions.jsonl");

/// `shipTreeGroups.jsonl`: ship classes in the ship tree, such as Corvette.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ShipTreeGroup {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    pub icon: String,
    pub icon_large: String,
    pub icon_small: String,
    #[serde(rename = "iconSmallNPC")]
    pub icon_small_npc: String,
    /// Order, from 1, to [`ShipTreeElement`] ID.
    #[serde(default, deserialize_with = "de::kv_map")]
    pub elements: BTreeMap<u32, u32>,
    #[serde(default)]
    pub pre_req_skills: Vec<FactionPrereqs>,
}
record!(ShipTreeGroup, "shipTreeGroups.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct FactionPrereqs {
    #[serde(rename = "_key")]
    pub faction_id: u32,
    pub skills: Vec<PrereqSkill>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct PrereqSkill {
    #[serde(rename = "_key")]
    pub type_id: u32,
    pub level: u8,
    pub display: bool,
}
