//! Industry: blueprints, assembly lines and their modifiers.

use super::*;

/// `blueprints.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Blueprint {
    #[serde(rename = "_key")]
    pub id: u32,
    #[serde(rename = "blueprintTypeID")]
    pub blueprint_type_id: u32,
    pub max_production_limit: i64,
    pub activities: BlueprintActivities,
}
record!(Blueprint, "blueprints.jsonl");

/// What a blueprint can do. Missing means it can't.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct BlueprintActivities {
    pub manufacturing: Option<Activity>,
    pub reaction: Option<Activity>,
    pub copying: Option<Activity>,
    pub research_material: Option<Activity>,
    pub research_time: Option<Activity>,
    pub invention: Option<Activity>,
}

/// Time in seconds, inputs and outputs of one blueprint activity.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct Activity {
    #[serde(default)]
    pub time: i64,
    #[serde(default)]
    pub materials: Vec<TypeQuantity>,
    #[serde(default)]
    pub products: Vec<Product>,
    #[serde(default)]
    pub skills: Vec<SkillLevel>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct Product {
    #[serde(rename = "typeID")]
    pub type_id: u32,
    pub quantity: i64,
    /// Invention only.
    pub probability: Option<f64>,
}

/// `industryActivities.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct IndustryActivity {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    pub description: String,
}
record!(IndustryActivity, "industryActivities.jsonl");

/// `industryAssemblyLines.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct IndustryAssemblyLine {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "activityID")]
    pub activity_id: u32,
    pub base_material_multiplier: f64,
    pub base_time_multiplier: f64,
    pub base_cost_multiplier: Option<f64>,
    #[serde(default)]
    pub details_per_group: Vec<GroupMultipliers>,
    #[serde(default)]
    pub details_per_category: Vec<CategoryMultipliers>,
    #[serde(default)]
    pub details_per_type_list: Vec<TypeListMultipliers>,
}
record!(IndustryAssemblyLine, "industryAssemblyLines.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct GroupMultipliers {
    #[serde(rename = "groupID")]
    pub group_id: u32,
    pub material_multiplier: f64,
    pub time_multiplier: f64,
    pub cost_multiplier: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CategoryMultipliers {
    #[serde(rename = "categoryID")]
    pub category_id: u32,
    pub material_multiplier: f64,
    pub time_multiplier: f64,
    pub cost_multiplier: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TypeListMultipliers {
    #[serde(rename = "typeListID")]
    pub type_list_id: u32,
    pub material_multiplier: f64,
    pub time_multiplier: f64,
    pub cost_multiplier: Option<f64>,
}

/// `industryInstallationTypes.jsonl`: which assembly lines a structure type has.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct IndustryInstallationType {
    #[serde(rename = "_key")]
    pub id: u32,
    pub assembly_lines: Vec<AssemblyLineRef>,
}
record!(IndustryInstallationType, "industryInstallationTypes.jsonl");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct AssemblyLineRef {
    #[serde(rename = "assemblyLineID")]
    pub assembly_line_id: u32,
}

/// `industryModifierSources.jsonl`: which attributes of a type change industry
/// cost, time or material use. Section names are camelCase in this file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct IndustryModifierSource {
    #[serde(rename = "_key")]
    pub id: u32,
    pub manufacturing: Option<ModifierSection>,
    pub reaction: Option<ModifierSection>,
    pub copying: Option<ModifierSection>,
    pub research_material: Option<ModifierSection>,
    pub research_time: Option<ModifierSection>,
    pub invention: Option<ModifierSection>,
}
record!(IndustryModifierSource, "industryModifierSources.jsonl");

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ModifierSection {
    #[serde(default)]
    pub cost: Vec<ModifierAttribute>,
    #[serde(default)]
    pub time: Vec<ModifierAttribute>,
    #[serde(default)]
    pub material: Vec<ModifierAttribute>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct ModifierAttribute {
    #[serde(rename = "dogmaAttributeID")]
    pub attribute_id: u32,
    #[serde(rename = "filterID")]
    pub filter_id: Option<u32>,
}

/// `industryTargetFilters.jsonl`
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct IndustryTargetFilter {
    #[serde(rename = "_key")]
    pub id: u32,
    pub name: String,
    #[serde(default, rename = "categoryIDs")]
    pub category_ids: Vec<u32>,
    #[serde(default, rename = "groupIDs")]
    pub group_ids: Vec<u32>,
}
record!(IndustryTargetFilter, "industryTargetFilters.jsonl");
