//! The whole SDE in memory, with the indexes the lookups need.

use super::{Named, Relation, Source, name_key, naming};
use crate::archive::Archive;
use crate::error::{Error, Result};
use crate::model::{
    Attribute, Category, Constellation, Effect, Faction, Group, MarketGroup, NpcCorporation,
    NpcStation, Region, SolarSystem, Type,
};
use crate::tables::{Record, TableInfo, TableVisitor, visit_tables};
use std::any::{Any, TypeId as AnyTypeId};
use std::collections::HashMap;
use std::hash::Hash;
use std::io::{Read, Seek};

/// Lower-case name → IDs, preferred records first, then lower IDs.
type NameIndex = HashMap<String, Vec<u32>>;

pub(super) struct Memory {
    /// Each table as a `HashMap<T::Key, T>`, by the model's type.
    tables: HashMap<AnyTypeId, Box<dyn Any + Send + Sync>>,
    names: HashMap<Named, NameIndex>,
    children: HashMap<Relation, HashMap<u32, Vec<u32>>>,
    unmodeled: Vec<String>,
}

impl Memory {
    pub(super) fn load<R: Read + Seek>(archive: &mut Archive<R>) -> Result<Self> {
        struct Load<'a, R> {
            archive: &'a mut Archive<R>,
            tables: HashMap<AnyTypeId, Box<dyn Any + Send + Sync>>,
        }

        impl<R: Read + Seek> TableVisitor for Load<'_, R> {
            type Error = Error;

            fn visit<T: Record>(&mut self, _: &TableInfo) -> Result<()> {
                let table: HashMap<T::Key, T> = if self.archive.has_file(T::FILE) {
                    self.archive.table::<T>()?
                } else {
                    HashMap::new()
                };
                self.tables.insert(AnyTypeId::of::<T>(), Box::new(table));
                Ok(())
            }
        }

        let mut load = Load {
            archive,
            tables: HashMap::new(),
        };
        visit_tables(&mut load)?;
        let unmodeled = load
            .archive
            .unmodeled_files()
            .iter()
            .filter(|file| file.strip_suffix(".jsonl").is_some())
            .cloned()
            .collect();
        let mut memory = Self {
            tables: load.tables,
            names: HashMap::new(),
            children: HashMap::new(),
            unmodeled,
        };
        memory.names = memory.name_indexes()?;
        memory.children = HashMap::from([
            (
                Relation::TypesInGroup,
                multimap(memory.rows::<Type>().map(|t| (t.group_id.0, t.id.0))),
            ),
            (
                Relation::GroupsInCategory,
                multimap(memory.rows::<Group>().map(|g| (g.category_id.0, g.id.0))),
            ),
        ]);
        Ok(memory)
    }

    fn name_indexes(&self) -> Result<HashMap<Named, NameIndex>> {
        let stations = self
            .rows::<NpcStation>()
            .map(|station| Ok((station.id.0, naming::station_name(self, station)?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(HashMap::from([
            (
                Named::Type,
                name_index(
                    self.rows::<Type>()
                        .map(|t| (t.id.0, t.name.as_str(), t.published)),
                ),
            ),
            (
                Named::Group,
                name_index(
                    self.rows::<Group>()
                        .map(|g| (g.id.0, g.name.as_str(), g.published)),
                ),
            ),
            (
                Named::Category,
                name_index(
                    self.rows::<Category>()
                        .map(|c| (c.id.0, c.name.as_str(), c.published)),
                ),
            ),
            (
                Named::MarketGroup,
                name_index(
                    self.rows::<MarketGroup>()
                        .map(|g| (g.id.0, g.name.as_str(), true)),
                ),
            ),
            (
                Named::System,
                name_index(
                    self.rows::<SolarSystem>()
                        .map(|s| (s.id.0, s.name.as_str(), true)),
                ),
            ),
            (
                Named::Constellation,
                name_index(
                    self.rows::<Constellation>()
                        .map(|c| (c.id.0, c.name.as_str(), true)),
                ),
            ),
            (
                Named::Region,
                name_index(
                    self.rows::<Region>()
                        .map(|r| (r.id.0, r.name.as_str(), true)),
                ),
            ),
            (
                Named::Station,
                name_index(stations.iter().map(|(id, name)| (*id, name.as_str(), true))),
            ),
            (
                Named::Faction,
                name_index(
                    self.rows::<Faction>()
                        .map(|f| (f.id.0, f.name.as_str(), true)),
                ),
            ),
            (
                Named::Corporation,
                name_index(
                    self.rows::<NpcCorporation>()
                        .map(|c| (c.id.0, c.name.as_str(), !c.deleted)),
                ),
            ),
            (
                Named::Attribute,
                name_index(
                    self.rows::<Attribute>()
                        .map(|a| (a.id.0, a.name.as_str(), true)),
                ),
            ),
            (
                Named::Effect,
                name_index(
                    self.rows::<Effect>()
                        .map(|e| (e.id.0, e.name.as_str(), true)),
                ),
            ),
        ]))
    }

    fn table<T: Record>(&self) -> &HashMap<T::Key, T> {
        self.tables
            .get(&AnyTypeId::of::<T>())
            .and_then(|table| table.downcast_ref())
            .expect("`load` fills a table for every model")
    }

    fn rows<T: Record>(&self) -> impl Iterator<Item = &T> {
        self.table::<T>().values()
    }

    pub(super) fn get<T: Record>(&self, key: &T::Key) -> Option<T> {
        self.table::<T>().get(key).cloned()
    }

    pub(super) fn ids_named(&self, named: Named, key: &str) -> Vec<u32> {
        self.names
            .get(&named)
            .and_then(|index| index.get(key))
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn children(&self, relation: Relation, parent: u32) -> Vec<u32> {
        self.children
            .get(&relation)
            .and_then(|children| children.get(&parent))
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn unmodeled_files(&self) -> Vec<String> {
        self.unmodeled.clone()
    }
}

impl Source for Memory {
    fn get_record<T: Record>(&self, key: &T::Key) -> Result<Option<T>> {
        Ok(self.get(key))
    }
}

/// Group values by key. Each list is sorted, without repeats.
fn multimap<K: Hash + Eq, V: Ord>(pairs: impl Iterator<Item = (K, V)>) -> HashMap<K, Vec<V>> {
    let mut map: HashMap<K, Vec<V>> = HashMap::new();
    for (key, value) in pairs {
        map.entry(key).or_default().push(value);
    }
    for values in map.values_mut() {
        values.sort_unstable();
        values.dedup();
    }
    map
}

/// Index `(id, name, preferred)` rows by [`name_key`]: preferred IDs first,
/// then lower IDs.
fn name_index<'a>(rows: impl Iterator<Item = (u32, &'a str, bool)>) -> NameIndex {
    multimap(rows.map(|(id, name, preferred)| (name_key(name), (!preferred, id))))
        .into_iter()
        .map(|(name, ids)| (name, ids.into_iter().map(|(_, id)| id).collect()))
        .collect()
}
