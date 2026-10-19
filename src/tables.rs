//! Every table of the SDE, and the trait that ties a model to its file.

use serde::de::DeserializeOwned;
#[cfg(feature = "store")]
use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;

pub(crate) mod sealed {
    /// Only this crate implements [`Record`](super::Record), so adding a
    /// table is not a breaking change. The `tables!` macro implements it, so
    /// a model that is missing from the table list does not compile.
    pub trait Sealed {
        /// The table's name, as in [`TableInfo::name`](super::TableInfo::name).
        const TABLE: &'static str;
    }

    /// Only this crate implements [`Key`](super::Key).
    pub trait SealedKey {
        /// The key as an SQL value, for a lookup by key.
        #[cfg(feature = "sqlite")]
        fn sql_key(&self) -> rusqlite::types::ToSqlOutput<'_>;
        /// The key as the packed store compares it. Its
        /// [`kind`](super::StoreKey::kind) is always the
        /// [`Key::KIND`](super::Key::KIND) of the implementing type, so a
        /// key always has a representation in a store and the lookup never
        /// has to answer "no such key shape".
        #[cfg(feature = "store")]
        fn store_key(&self) -> super::StoreKey<'_>;
    }
}

/// A key as the packed store compares it: one row of a key index reads back
/// as one of these, and so does the key a lookup is given, so a binary
/// search compares one type against itself. Text is bytes because that is
/// the order the index is sorted in.
///
/// It is `pub` only because it appears in the signature of a sealed trait
/// that public traits are bound by, as `rusqlite`'s types do. Nothing
/// outside this crate can name it or implement the trait.
#[cfg(feature = "store")]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StoreKey<'a> {
    /// CCP's `_key` as a number. Keys outside `u32` only reach a store from
    /// a file this crate has no model for.
    Integer(i64),
    /// CCP's `_key` as text, such as a UUID, in the bytes the store holds.
    Text(&'a [u8]),
}

#[cfg(feature = "store")]
impl StoreKey<'_> {
    /// Whether this key is a number or text.
    pub(crate) fn kind(&self) -> KeyKind {
        match self {
            Self::Integer(_) => KeyKind::Integer,
            Self::Text(_) => KeyKind::Text,
        }
    }
}

/// Names a key the way an error message about one reads.
#[cfg(feature = "store")]
impl fmt::Display for StoreKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(key) => write!(f, "{key}"),
            Self::Text(key) => write!(f, "{:?}", String::from_utf8_lossy(key)),
        }
    }
}

/// One SDE file and the model its lines parse into.
///
/// Every model in [`model`](crate::model) implements it. Only this crate can
/// implement it.
pub trait Record: DeserializeOwned + Clone + Send + Sync + 'static + sealed::Sealed {
    /// File name inside the ZIP, e.g. `types.jsonl`.
    const FILE: &'static str;
    /// The type of CCP's `_key`, which the model holds as `id`.
    type Key: Key;
    /// This record's key.
    fn key(&self) -> Self::Key;
}

/// The type of a table's key.
pub trait Key: Clone + Eq + Ord + Hash + Debug + Send + Sync + 'static + sealed::SealedKey {
    /// Whether keys are numbers or text.
    const KIND: KeyKind;
}

/// Whether a table's keys are numbers or text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyKind {
    /// A `u32` or one of the [`ids`](crate::ids) types.
    Integer,
    /// A string, such as a UUID.
    Text,
}

impl sealed::SealedKey for u32 {
    #[cfg(feature = "sqlite")]
    fn sql_key(&self) -> rusqlite::types::ToSqlOutput<'_> {
        rusqlite::types::ToSqlOutput::Owned(rusqlite::types::Value::Integer(i64::from(*self)))
    }

    #[cfg(feature = "store")]
    fn store_key(&self) -> StoreKey<'_> {
        StoreKey::Integer(i64::from(*self))
    }
}
impl Key for u32 {
    const KIND: KeyKind = KeyKind::Integer;
}

impl sealed::SealedKey for String {
    #[cfg(feature = "sqlite")]
    fn sql_key(&self) -> rusqlite::types::ToSqlOutput<'_> {
        rusqlite::types::ToSqlOutput::Borrowed(rusqlite::types::ValueRef::Text(self.as_bytes()))
    }

    #[cfg(feature = "store")]
    fn store_key(&self) -> StoreKey<'_> {
        StoreKey::Text(self.as_bytes())
    }
}
impl Key for String {
    const KIND: KeyKind = KeyKind::Text;
}

/// One table of the SDE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct TableInfo {
    /// The table's name, such as `types` or `solar_systems`.
    pub name: &'static str,
    /// The file inside the ZIP, such as `types.jsonl`.
    pub file: &'static str,
    /// Whether the keys are numbers or text.
    pub key: KeyKind,
}

/// Visits every table in turn, with the table's model as a type parameter.
///
/// Pass it to [`visit_tables`] to do the same thing with every table, such as
/// counting its rows with [`Archive::records`](crate::Archive::records).
pub trait TableVisitor {
    /// The error that stops the visit.
    type Error;

    /// Called once for each table, in the order of [`tables`].
    ///
    /// # Errors
    ///
    /// An error stops the visit, and [`visit_tables`] returns it.
    fn visit<T: Record>(&mut self, table: &TableInfo) -> Result<(), Self::Error>;
}

macro_rules! table_info {
    ($name:ident, $model:ident) => {
        TableInfo {
            name: <$model as $crate::tables::sealed::Sealed>::TABLE,
            file: <$model as Record>::FILE,
            key: <<$model as Record>::Key as Key>::KIND,
        }
    };
}

macro_rules! tables {
    ($($name:ident: $model:ident),* $(,)?) => {
        $(impl $crate::tables::sealed::Sealed for $model {
            const TABLE: &'static str = stringify!($name);
        })*

        /// Every table this crate reads, in a fixed order.
        pub fn tables() -> &'static [TableInfo] {
            const TABLES: &[TableInfo] = &[$(table_info!($name, $model)),*];
            TABLES
        }

        /// Calls `visitor` once for each table, in the order of [`tables`].
        ///
        /// # Errors
        ///
        /// Returns the first error that `visitor` returns. The tables after
        /// it are not visited.
        pub fn visit_tables<V: TableVisitor>(visitor: &mut V) -> Result<(), V::Error> {
            $(visitor.visit::<$model>(&table_info!($name, $model))?;)*
            Ok(())
        }
    };
}

// The list of every table, generated from `schema/` by `cargo xtask models`.
mod list;

pub use list::{tables, visit_tables};

/// The table of [`tables`] that reads `file`, if any.
pub(crate) fn table_for_file(file: &str) -> Option<&'static TableInfo> {
    tables().iter().find(|table| table.file == file)
}

/// Whether `file` is one of the table files of [`tables`].
pub(crate) fn is_table_file(file: &str) -> bool {
    table_for_file(file).is_some()
}

/// The table name for a file this crate has no model for: its name without
/// `.jsonl`, in snake case by the rule that names the modeled tables
/// (`snake_case` in `xtask/src/generate.rs`), with any other character
/// replaced by `_`.
///
/// Keep the two rules the same. A store holds its tables by name, so a file
/// that had no model when the store was written is still found once it has
/// one — unless `schema/settings.toml` gives the model a shorter name, as it
/// does for `mapMoons.jsonl` and the table `moons`. Then the older store
/// holds the file under the name this rule gave it, the lookup for the new
/// name fails and says to import again, and
/// `an_unmodeled_files_name_is_never_another_tables` is what keeps that from
/// being a wrong answer instead.
#[cfg(any(feature = "sqlite", feature = "store"))]
pub(crate) fn table_name(stem: &str) -> String {
    let stem = stem.replace("IDs", "Ids").replace("ID", "Id");
    let chars: Vec<char> = stem.chars().collect();
    let mut name = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_ascii_uppercase() {
            name.push(if c.is_ascii_alphanumeric() { c } else { '_' });
            continue;
        }
        let prev = i.checked_sub(1).and_then(|j| chars.get(j));
        let next = chars.get(i + 1);
        let after_lowercase = prev.is_some_and(char::is_ascii_lowercase);
        let ends_acronym = prev.is_some_and(char::is_ascii_uppercase)
            && next.is_some_and(char::is_ascii_lowercase);
        if after_lowercase || ends_acronym {
            name.push('_');
        }
        name.push(c.to_ascii_lowercase());
    }
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[cfg(any(feature = "sqlite", feature = "store"))]
    #[test]
    fn unmodeled_files_are_named_in_snake_case() {
        assert_eq!(table_name("mapSolarSystems"), "map_solar_systems");
        assert_eq!(table_name("includedTypeIDs"), "included_type_ids");
        assert_eq!(table_name("allowCCPDevs"), "allow_ccp_devs");
        assert_eq!(table_name("odd-name.v2"), "odd_name_v2");
        assert_eq!(table_name("3dThings"), "_3d_things");
    }

    /// A file that has no model is stored under the name this rule gives
    /// it, and tables are matched by name, so that name must never be some
    /// other table's. If it were, a later version with a model for one of
    /// them would read the other's records as its own.
    #[cfg(any(feature = "sqlite", feature = "store"))]
    #[test]
    fn an_unmodeled_files_name_is_never_another_tables() {
        let names: HashSet<_> = tables().iter().map(|t| t.name).collect();
        for table in tables() {
            let stem = table.file.strip_suffix(".jsonl").unwrap_or(table.file);
            let name = table_name(stem);
            assert!(
                name == table.name || !names.contains(name.as_str()),
                "`{}` with no model would be table `{name}`, which is taken",
                table.file,
            );
        }
    }

    #[test]
    fn every_table_has_a_unique_name_and_file() {
        let names: HashSet<_> = tables().iter().map(|t| t.name).collect();
        let files: HashSet<_> = tables().iter().map(|t| t.file).collect();
        assert_eq!(names.len(), tables().len());
        assert_eq!(files.len(), tables().len());
    }

    /// Every lookup in a store relies on this: a key's representation is
    /// always of the kind its table is indexed by.
    #[cfg(feature = "store")]
    #[test]
    fn a_keys_representation_is_of_the_kind_it_declares() {
        use crate::ids::TypeId;
        use sealed::SealedKey;

        fn check<K: Key>(key: &K) {
            assert_eq!(key.store_key().kind(), K::KIND);
        }
        check(&587_u32);
        check(&"a1b2".to_owned());
        check(&TypeId(587));
        assert_eq!(TypeId(587).store_key(), StoreKey::Integer(587));
        assert_eq!("a1b2".to_owned().store_key(), StoreKey::Text(b"a1b2"));
    }

    #[test]
    fn keys_say_whether_they_are_numbers_or_text() {
        let key = |name| tables().iter().find(|t| t.name == name).unwrap().key;
        assert_eq!(key("types"), KeyKind::Integer);
        assert_eq!(key("character_titles"), KeyKind::Text);
    }

    #[test]
    fn the_visitor_sees_every_table_in_order() {
        struct Files(Vec<&'static str>);
        impl TableVisitor for Files {
            type Error = ();
            fn visit<T: Record>(&mut self, table: &TableInfo) -> Result<(), ()> {
                assert_eq!(T::FILE, table.file);
                self.0.push(table.file);
                Ok(())
            }
        }
        let mut files = Files(Vec::new());
        visit_tables(&mut files).unwrap();
        let expected: Vec<_> = tables().iter().map(|t| t.file).collect();
        assert_eq!(files.0, expected);
    }

    #[test]
    fn an_error_stops_the_visit() {
        struct StopAtGroups(usize);
        impl TableVisitor for StopAtGroups {
            type Error = &'static str;
            fn visit<T: Record>(&mut self, table: &TableInfo) -> Result<(), &'static str> {
                self.0 += 1;
                if table.name == "groups" {
                    return Err("stop");
                }
                Ok(())
            }
        }
        let mut visitor = StopAtGroups(0);
        assert_eq!(visit_tables(&mut visitor), Err("stop"));
        let groups = tables().iter().position(|t| t.name == "groups").unwrap();
        assert_eq!(visitor.0, groups + 1);
    }
}
