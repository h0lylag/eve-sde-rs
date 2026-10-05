//! Items: types, groups, categories, market groups and what hangs off them.

use super::*;
use crate::de;
use std::collections::BTreeMap;

/// `types.jsonl`: every item, ship, skill, blueprint and so on.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Type {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "groupID")]
    pub group_id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    pub published: bool,
    /// How many units one reprocessing batch takes.
    pub portion_size: u32,
    pub mass: Option<f64>,
    /// Assembled volume in m³. Packaged volume is `packaged_volume`.
    pub volume: Option<f64>,
    /// Only meaningful when `is_repackable` is set.
    pub packaged_volume: Option<f64>,
    pub capacity: Option<f64>,
    pub radius: Option<f64>,
    pub base_price: Option<f64>,
    #[serde(rename = "marketGroupID")]
    pub market_group_id: Option<u32>,
    /// Meta group, such as Tech I, Tech II, Faction or Officer. Many items,
    /// including some Tech I modules, leave it unset.
    #[serde(rename = "metaGroupID")]
    pub meta_group_id: Option<u32>,
    pub meta_level: Option<u32>,
    pub tech_level: Option<u32>,
    /// The basic version of this module, ship or drone.
    #[serde(rename = "variationParentTypeID")]
    pub variation_parent_type_id: Option<u32>,
    #[serde(rename = "factionID")]
    pub faction_id: Option<u32>,
    #[serde(rename = "raceID")]
    pub race_id: Option<u32>,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    #[serde(rename = "graphicID")]
    pub graphic_id: Option<u32>,
    #[serde(rename = "soundID")]
    pub sound_id: Option<u32>,
    #[serde(rename = "shipTreeGroupID")]
    pub ship_tree_group_id: Option<u32>,
    #[serde(default)]
    pub is_repackable: bool,
    /// True for mutated items, whose attribute values differ per item. See
    /// [`DynamicItemAttributes`].
    #[serde(default)]
    pub is_dynamic_type: bool,
}
record!(Type, "types.jsonl");

/// `groups.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Group {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(rename = "categoryID")]
    pub category_id: u32,
    pub published: bool,
    pub anchorable: bool,
    pub anchored: bool,
    pub fittable_non_singleton: bool,
    pub use_base_price: bool,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
}
record!(Group, "groups.jsonl");

/// `categories.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Category {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    pub published: bool,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
}
record!(Category, "categories.jsonl");

/// `marketGroups.jsonl`: the market's tree of folders.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MarketGroup {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    /// True for the folders that hold items directly.
    pub has_types: bool,
    #[serde(rename = "parentGroupID")]
    pub parent_group_id: Option<u32>,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
}
record!(MarketGroup, "marketGroups.jsonl");

/// `metaGroups.jsonl`: Tech I, Tech II, Faction, Officer and so on.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MetaGroup {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(deserialize_with = "de::en")]
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub description: Option<String>,
    pub color: Option<Rgb>,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    pub icon_suffix: Option<String>,
}
record!(MetaGroup, "metaGroups.jsonl");

/// `typeDogma.jsonl`: the attribute values and effects of a type (the `id`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TypeDogma {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(default)]
    pub dogma_attributes: Vec<TypeAttribute>,
    #[serde(default)]
    pub dogma_effects: Vec<TypeEffect>,
}
record!(TypeDogma, "typeDogma.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct TypeAttribute {
    #[serde(rename = "attributeID")]
    pub attribute_id: u32,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct TypeEffect {
    #[serde(rename = "effectID")]
    pub effect_id: u32,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

/// `typeMaterials.jsonl`: what reprocessing a type (the `id`) gives.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TypeMaterials {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(default)]
    pub materials: Vec<Material>,
    #[serde(default)]
    pub randomized_materials: Vec<RandomMaterial>,
}
record!(TypeMaterials, "typeMaterials.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Material {
    #[serde(rename = "materialTypeID")]
    pub material_type_id: u32,
    pub quantity: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RandomMaterial {
    #[serde(rename = "materialTypeID")]
    pub material_type_id: u32,
    pub quantity_min: i64,
    pub quantity_max: i64,
}

/// `typeBonus.jsonl`: the bonus lines that the game shows for a type (the
/// `id`), mostly ships, subsystems and structures.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TypeBonuses {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "iconID")]
    pub icon_id: Option<u32>,
    #[serde(default)]
    pub role_bonuses: Vec<Bonus>,
    /// Skill type ID to the bonuses that skill gives.
    #[serde(default, deserialize_with = "de::kv_map")]
    pub types: BTreeMap<u32, Vec<Bonus>>,
    #[serde(default)]
    pub misc_bonuses: Vec<Bonus>,
}
record!(TypeBonuses, "typeBonus.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Bonus {
    pub bonus: Option<f64>,
    #[serde(deserialize_with = "de::en")]
    pub bonus_text: String,
    pub importance: i32,
    #[serde(rename = "unitID")]
    pub unit_id: Option<u32>,
    pub is_positive: Option<bool>,
}

/// `typeElements.jsonl`: the ship tree tags of a ship type (the `id`), such
/// as Small, Support and Shields.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct TypeElement {
    #[serde(rename = "_key")]
    pub id: u32,
    /// Order, from 1, to [`ShipTreeElement`] ID.
    #[serde(deserialize_with = "de::kv_map")]
    pub elements: BTreeMap<u32, u32>,
}
record!(TypeElement, "typeElements.jsonl");

/// `typeLists.jsonl`: named sets of types, groups and categories.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TypeList {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub display_name: Option<String>,
    #[serde(default, deserialize_with = "de::en_opt")]
    pub display_description: Option<String>,
    #[serde(default, rename = "includedTypeIDs")]
    pub included_type_ids: Vec<u32>,
    #[serde(default, rename = "excludedTypeIDs")]
    pub excluded_type_ids: Vec<u32>,
    #[serde(default, rename = "includedGroupIDs")]
    pub included_group_ids: Vec<u32>,
    #[serde(default, rename = "excludedGroupIDs")]
    pub excluded_group_ids: Vec<u32>,
    #[serde(default, rename = "includedCategoryIDs")]
    pub included_category_ids: Vec<u32>,
    #[serde(default, rename = "excludedCategoryIDs")]
    pub excluded_category_ids: Vec<u32>,
}
record!(TypeList, "typeLists.jsonl");

/// `compressibleTypes.jsonl`: what a type (the `id`), such as an ore, ice or
/// gas, compresses into.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct CompressibleType {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "compressedTypeID")]
    pub compressed_type_id: u32,
}
record!(CompressibleType, "compressibleTypes.jsonl");

/// `dynamicItemAttributes.jsonl`: how a mutaplasmid (the `id`) changes an
/// item.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DynamicItemAttributes {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "attributeIDs")]
    pub attributes: Vec<DynamicAttribute>,
    pub input_output_mapping: Vec<DynamicMapping>,
}
record!(DynamicItemAttributes, "dynamicItemAttributes.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DynamicAttribute {
    #[serde(rename = "_key")]
    pub attribute_id: u32,
    pub min: f64,
    pub max: f64,
    pub high_is_good: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DynamicMapping {
    pub applicable_types: Vec<u32>,
    pub resulting_type: u32,
}

/// `masteries.jsonl`: the certificates needed for each mastery level of a
/// ship type (the `id`).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct Mastery {
    #[serde(rename = "_key")]
    pub id: u32,
    /// Mastery level (0–4) to certificate IDs.
    #[serde(rename = "_value", deserialize_with = "de::kv_map")]
    pub levels: BTreeMap<u32, Vec<u32>>,
}
record!(Mastery, "masteries.jsonl");
