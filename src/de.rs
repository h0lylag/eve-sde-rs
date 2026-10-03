//! Serde helpers for the shapes CCP uses in the SDE.

use serde::de::Error;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt::Debug;

#[derive(Deserialize)]
struct Text {
    en: String,
}

/// `{"en": "…", "de": "…", …}` → the English string.
pub fn en<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Text::deserialize(d).map(|t| t.en)
}

/// Like [`en`], for optional fields. Use with `#[serde(default)]`.
pub fn en_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<Text>::deserialize(d).map(|t| t.map(|t| t.en))
}

/// `[{"_key": k, "_value": v}, …]` → map. A repeated key is an error.
pub fn kv_map<'de, D, K, V>(d: D) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord + Debug,
    V: Deserialize<'de>,
{
    #[derive(Deserialize)]
    struct KeyValue<K, V> {
        #[serde(rename = "_key")]
        key: K,
        #[serde(rename = "_value")]
        value: V,
    }
    let entries = Vec::<KeyValue<K, V>>::deserialize(d)?;
    unique_map(entries.into_iter().map(|e| (e.key, e.value)))
}

/// `[{"_key": "name", "en": "…", …}, …]` → map from key to English text. A
/// repeated key is an error.
pub fn keyed_en<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, String>, D::Error> {
    #[derive(Deserialize)]
    struct KeyedText {
        #[serde(rename = "_key")]
        key: String,
        en: String,
    }
    let entries = Vec::<KeyedText>::deserialize(d)?;
    unique_map(entries.into_iter().map(|e| (e.key, e.en)))
}

fn unique_map<K: Ord + Debug, V, E: Error>(
    pairs: impl Iterator<Item = (K, V)>,
) -> Result<BTreeMap<K, V>, E> {
    let mut map = BTreeMap::new();
    for (key, value) in pairs {
        match map.entry(key) {
            Entry::Vacant(slot) => {
                slot.insert(value);
            }
            Entry::Occupied(slot) => {
                return Err(E::custom(format_args!("repeated key {:?}", slot.key())));
            }
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Row {
        #[serde(deserialize_with = "en")]
        name: String,
        #[serde(default, deserialize_with = "en_opt")]
        description: Option<String>,
        #[serde(default, deserialize_with = "kv_map")]
        skills: BTreeMap<u32, u8>,
    }

    #[test]
    fn keeps_only_english() {
        let r: Row = serde_json::from_str(
            r#"{"name":{"en":"Rifter","de":"Rifter","ja":"リフター"},"description":{"en":"d"},
                "skills":[{"_key":3,"_value":1},{"_key":2,"_value":5}]}"#,
        )
        .unwrap();
        assert_eq!(r.name, "Rifter");
        assert_eq!(r.description.as_deref(), Some("d"));
        assert_eq!(r.skills.into_iter().collect::<Vec<_>>(), [(2, 5), (3, 1)]);
    }

    #[test]
    fn optional_pieces_may_be_missing() {
        let r: Row = serde_json::from_str(r#"{"name":{"en":"x"}}"#).unwrap();
        assert!(r.description.is_none() && r.skills.is_empty());
    }

    #[test]
    fn name_needs_english() {
        assert!(serde_json::from_str::<Row>(r#"{"name":{"de":"x"}}"#).is_err());
    }

    #[test]
    fn repeated_kv_key_is_an_error() {
        let bad = r#"{"name":{"en":"x"},"skills":[{"_key":1,"_value":1},{"_key":1,"_value":2}]}"#;
        assert!(serde_json::from_str::<Row>(bad).is_err());
    }

    #[test]
    fn keyed_english_keeps_text_and_rejects_repeated_keys() {
        #[derive(Debug, Deserialize)]
        struct Row {
            #[serde(deserialize_with = "keyed_en")]
            text: BTreeMap<String, String>,
        }

        let row: Row = serde_json::from_str(
            r#"{"text":[{"_key":"name","en":"Rifter","de":"Rifter"},{"_key":"description","en":"A frigate"}]}"#,
        )
        .unwrap();
        assert_eq!(row.text["name"], "Rifter");
        assert_eq!(row.text["description"], "A frigate");

        let err = serde_json::from_str::<Row>(
            r#"{"text":[{"_key":"name","en":"Rifter"},{"_key":"name","en":"Other"}]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("repeated key \"name\""));
    }
}
