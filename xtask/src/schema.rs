//! Infers the shape of every table from an SDE ZIP, and reads and writes it
//! in `schema/`.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek};
use std::path::Path;
use zip::ZipArchive;

/// The file in every SDE archive that says which build it is.
const BUILD_FILE: &str = "_sde.jsonl";

/// The file in `schema/` that records the build the schema was read from.
const BUILD_RECORD: &str = "_build.json";

/// The languages CCP translates text into.
pub const LANGUAGES: [&str; 9] = ["de", "en", "es", "fr", "it", "ja", "ko", "ru", "zh"];

/// The shape of a JSON value, as far as every record of one build shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// A number always written without a decimal point.
    Integer,
    /// A number written with a decimal point at least once.
    Float,
    /// `true` or `false`.
    Bool,
    /// A plain string.
    String,
    /// Text with translations, such as `{"en": "Rifter", "de": "Rifter"}`.
    Text,
    /// Only empty lists were seen, so the shape of their items is not known.
    Unknown,
    /// A list of items of one shape.
    Array(Box<Self>),
    /// An object, with its fields by name.
    Object(BTreeMap<String, Field>),
}

/// One field of an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    /// Whether every object of this shape has the field.
    pub required: bool,
    /// The shape of the field's value.
    #[serde(rename = "type")]
    pub shape: Shape,
}

/// One table: its file in the archive and the shape of its records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    /// The file inside the archive, such as `types.jsonl`.
    pub file: String,
    /// The shape of every record, an object.
    pub record: Shape,
}

/// The build a schema was read from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    /// CCP's build number.
    pub build_number: u64,
    /// When CCP released the build.
    pub release_date: Option<String>,
}

/// The schema of every table, by file stem such as `types`.
pub type Tables = BTreeMap<String, Table>;

/// What the records of one table showed at one place in their structure.
#[derive(Debug, Default)]
struct Seen {
    /// The kinds of value seen here, other than objects.
    kinds: BTreeSet<Kind>,
    /// How many objects were seen here.
    objects: u64,
    /// Each field of those objects: how many objects had it, and its values.
    fields: BTreeMap<String, (u64, Self)>,
    /// The items of every list seen here, if any list was.
    items: Option<Box<Self>>,
}

/// A kind of JSON value, other than an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Null,
    Bool,
    Integer,
    Float,
    String,
    Text,
    Array,
}

impl Seen {
    fn add(&mut self, value: &Value) {
        match value {
            Value::Null => self.see(Kind::Null),
            Value::Bool(_) => self.see(Kind::Bool),
            Value::Number(n) if n.is_f64() => self.see(Kind::Float),
            Value::Number(_) => self.see(Kind::Integer),
            Value::String(_) => self.see(Kind::String),
            Value::Array(items) => {
                self.see(Kind::Array);
                let seen = self.items.get_or_insert_with(Box::default);
                for item in items {
                    seen.add(item);
                }
            }
            Value::Object(map) if is_text(map) => self.see(Kind::Text),
            Value::Object(map) => {
                self.objects += 1;
                for (key, value) in map {
                    let (count, seen) = self.fields.entry(key.clone()).or_default();
                    *count += 1;
                    seen.add(value);
                }
            }
        }
    }

    fn see(&mut self, kind: Kind) {
        self.kinds.insert(kind);
    }

    fn has(&self, kind: Kind) -> bool {
        self.kinds.contains(&kind)
    }

    /// Whether any value was seen here.
    fn any(&self) -> bool {
        !self.kinds.is_empty() || self.objects > 0
    }

    fn shape(&self, path: &str) -> anyhow::Result<Shape> {
        if self.has(Kind::Null) {
            bail!("`{path}` is null in some records");
        }
        // Integers and floats are one kind: numbers.
        let both_numbers = usize::from(self.has(Kind::Integer) && self.has(Kind::Float));
        let kinds = self.kinds.len() - both_numbers + usize::from(self.objects > 0);
        if kinds > 1 {
            bail!("`{path}` holds values of different kinds");
        }
        Ok(if self.has(Kind::Float) {
            Shape::Float
        } else if self.has(Kind::Integer) {
            Shape::Integer
        } else if self.has(Kind::Bool) {
            Shape::Bool
        } else if self.has(Kind::String) {
            Shape::String
        } else if self.has(Kind::Text) {
            Shape::Text
        } else if self.objects > 0 {
            let mut fields = BTreeMap::new();
            for (name, (count, seen)) in &self.fields {
                let shape = seen.shape(&format!("{path}.{name}"))?;
                let required = *count == self.objects;
                fields.insert(name.clone(), Field { required, shape });
            }
            Shape::Object(fields)
        } else {
            match &self.items {
                Some(items) if items.any() => {
                    Shape::Array(Box::new(items.shape(&format!("{path}[]"))?))
                }
                _ if self.has(Kind::Array) => Shape::Array(Box::new(Shape::Unknown)),
                _ => Shape::Unknown,
            }
        })
    }
}

/// Whether an object holds translations of one text, keyed by language.
fn is_text(map: &Map<String, Value>) -> bool {
    map.contains_key("en") && map.keys().all(|key| LANGUAGES.contains(&key.as_str()))
}

/// Infer the build and every table's schema from an SDE ZIP.
pub fn infer<R: Read + Seek>(reader: R) -> anyhow::Result<(Build, Tables)> {
    let mut zip = ZipArchive::new(reader).context("cannot open the archive")?;
    let mut names = zip
        .file_names()
        .map(|name| name.map(Cow::into_owned))
        .collect::<Result<Vec<_>, _>>()
        .context("cannot read the file names")?;
    names.sort();
    let mut build = None;
    let mut tables = Tables::new();
    for name in names {
        let Some(stem) = name.strip_suffix(".jsonl") else {
            continue;
        };
        let file = zip
            .by_name(&name)
            .with_context(|| format!("cannot open `{name}`"))?;
        let mut seen = Seen::default();
        for (number, line) in BufReader::new(file).lines().enumerate() {
            let line = line.with_context(|| format!("cannot read `{name}`"))?;
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(&line)
                .with_context(|| format!("cannot parse `{name}` line {}", number + 1))?;
            if name == BUILD_FILE {
                build = Some(serde_json::from_value(value).context("bad build record")?);
                break;
            }
            seen.add(&value);
        }
        if name != BUILD_FILE {
            let record = seen.shape(stem)?;
            tables.insert(stem.to_owned(), Table { file: name, record });
        }
    }
    let build = build.with_context(|| format!("the archive has no `{BUILD_FILE}`"))?;
    Ok((build, tables))
}

/// Infer the schema of the SDE ZIP at `zip` and write it to `dir`: one file
/// per table, and the build. Removes the files of tables that are gone.
pub fn write(zip: &Path, dir: &Path) -> anyhow::Result<()> {
    let file = File::open(zip).with_context(|| format!("cannot open {}", zip.display()))?;
    let (build, tables) = infer(BufReader::new(file))?;
    fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let path = entry?.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let is_table = path.extension().is_some_and(|e| e == "json") && !stem.starts_with('_');
        if is_table && !tables.contains_key(stem) {
            fs::remove_file(&path).with_context(|| format!("cannot remove {}", path.display()))?;
        }
    }
    write_json(&dir.join(BUILD_RECORD), &build)?;
    for (stem, table) in &tables {
        write_json(&dir.join(format!("{stem}.json")), table)?;
    }
    println!(
        "wrote the schema of {} tables from build {}",
        tables.len(),
        build.build_number
    );
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
}

/// Read every table's schema from `dir`.
pub fn read(dir: &Path) -> anyhow::Result<Tables> {
    let mut tables = Tables::new();
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let path = entry?.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().is_none_or(|e| e != "json") || stem.starts_with('_') {
            continue;
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        let table = serde_json::from_str(&text)
            .with_context(|| format!("cannot parse {}", path.display()))?;
        tables.insert(stem.to_owned(), table);
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    fn archive(files: &[(&str, &str)]) -> Cursor<Vec<u8>> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in files {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(text.as_bytes()).unwrap();
        }
        Cursor::new(writer.finish().unwrap().into_inner())
    }

    fn field(shape: Shape, required: bool) -> Field {
        Field { required, shape }
    }

    #[test]
    fn infers_types_optional_fields_text_and_lists() {
        let rows = [
            r#"{"_key":1,"name":{"en":"A","de":"A"},"mass":1.0,"ids":[1,2],"tags":[]}"#,
            r#"{"_key":2,"name":{"en":"B"},"mass":2.5,"ids":[],"tags":[],"flag":true}"#,
        ]
        .join("\n");
        let (build, tables) = infer(archive(&[
            ("_sde.jsonl", r#"{"_key":"sde","buildNumber":7}"#),
            ("things.jsonl", &rows),
        ]))
        .unwrap();
        assert_eq!(build.build_number, 7);
        let Shape::Object(fields) = &tables["things"].record else {
            panic!("{:?}", tables["things"]);
        };
        assert_eq!(fields["_key"], field(Shape::Integer, true));
        assert_eq!(fields["name"], field(Shape::Text, true));
        assert_eq!(fields["mass"], field(Shape::Float, true));
        assert_eq!(fields["flag"], field(Shape::Bool, false));
        assert_eq!(
            fields["ids"],
            field(Shape::Array(Box::new(Shape::Integer)), true)
        );
        assert_eq!(
            fields["tags"],
            field(Shape::Array(Box::new(Shape::Unknown)), true)
        );
    }

    #[test]
    fn a_number_written_with_a_decimal_point_once_is_a_float() {
        let (_, tables) = infer(archive(&[
            ("_sde.jsonl", r#"{"_key":"sde","buildNumber":7}"#),
            ("things.jsonl", "{\"v\":1}\n{\"v\":2.0}"),
        ]))
        .unwrap();
        let Shape::Object(fields) = &tables["things"].record else {
            panic!();
        };
        assert_eq!(fields["v"].shape, Shape::Float);
    }

    #[test]
    fn mixed_kinds_are_an_error_that_names_the_path() {
        let err = infer(archive(&[
            ("_sde.jsonl", r#"{"_key":"sde","buildNumber":7}"#),
            ("things.jsonl", "{\"v\":[1]}\n{\"v\":\"x\"}"),
        ]))
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "`things.v` holds values of different kinds"
        );
    }
}
