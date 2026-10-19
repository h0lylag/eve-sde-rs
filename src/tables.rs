//! Every table of the SDE, and the trait that ties a model to its file.

use serde::de::DeserializeOwned;
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
}
impl Key for u32 {
    const KIND: KeyKind = KeyKind::Integer;
}

impl sealed::SealedKey for String {
    #[cfg(feature = "sqlite")]
    fn sql_key(&self) -> rusqlite::types::ToSqlOutput<'_> {
        rusqlite::types::ToSqlOutput::Borrowed(rusqlite::types::ValueRef::Text(self.as_bytes()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_table_has_a_unique_name_and_file() {
        let names: HashSet<_> = tables().iter().map(|t| t.name).collect();
        let files: HashSet<_> = tables().iter().map(|t| t.file).collect();
        assert_eq!(names.len(), tables().len());
        assert_eq!(files.len(), tables().len());
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
