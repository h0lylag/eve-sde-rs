//! `schema/settings.toml`: what the generator cannot infer from the data.

use anyhow::Context;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Everything a person decides about the generated models.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// The ID type of a JSON field name wherever it appears, such as
    /// `typeID = "TypeId"`, unless the field sets its own `type`.
    #[serde(default)]
    pub ids: BTreeMap<String, String>,
    /// The docs of each module in `src/model/`.
    #[serde(default)]
    pub modules: BTreeMap<String, ModuleSettings>,
    /// Every struct, by its Rust name.
    #[serde(default)]
    pub structs: BTreeMap<String, StructSettings>,
}

/// A module of `src/model/`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleSettings {
    /// The module's doc comment.
    pub doc: Option<String>,
}

/// One struct.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructSettings {
    /// The module it is generated in, such as `types`.
    pub module: Option<String>,
    /// For a table's record: its file in the archive, such as `types.jsonl`.
    pub file: Option<String>,
    /// For a table's record: its name in `tables()`, when it is not the file
    /// name in snake case.
    pub table: Option<String>,
    /// For a table's record: the type of its key, such as `TypeId` or
    /// `String`. The default is `u32`.
    pub key: Option<String>,
    /// The struct's doc comment.
    pub doc: Option<String>,
    /// Traits to derive besides `Debug`, `Clone`, `PartialEq` and
    /// `Deserialize`.
    #[serde(default)]
    pub derive: Vec<String>,
    /// Settings of its fields, by JSON name.
    #[serde(default)]
    pub fields: BTreeMap<String, FieldSettings>,
}

/// One field, by its JSON name.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldSettings {
    /// The Rust name, when it is not the JSON name in snake case.
    pub name: Option<String>,
    /// The Rust type of the value: a scalar type, an ID type or a struct
    /// name. For a list it is the type of an item, and for a map the type of
    /// a value.
    #[serde(rename = "type")]
    pub ty: Option<String>,
    /// For a map: the type of its keys.
    pub key: Option<String>,
    /// An `Option` even though every record has the field.
    #[serde(default)]
    pub optional: bool,
    /// A missing value reads as the type's default, not as `None`.
    #[serde(default)]
    pub default: bool,
    /// Keep the value as raw JSON.
    pub raw: Option<Raw>,
    /// The JSON type of a field that no record has, such as `float`, to keep
    /// it as an optional field. For a field CCP stopped sending.
    pub missing: Option<crate::schema::Shape>,
    /// The field's doc comment.
    pub doc: Option<String>,
}

/// How much of a field stays raw JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Raw {
    /// The whole value is a `serde_json::Value`.
    Value,
    /// The value is a list of `serde_json::Value`.
    Items,
}

/// Read the settings file.
pub fn read(path: &Path) -> anyhow::Result<Settings> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
}
