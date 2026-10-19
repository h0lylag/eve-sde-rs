//! [`Sde`]: the whole SDE behind one handle, with lookups by name, joins
//! between tables, and the names the SDE leaves out.

mod lookups;
mod memory;
pub(crate) mod naming;

use crate::archive::{Archive, BuildInfo};
use crate::error::Result;
#[cfg(any(feature = "sqlite", feature = "store"))]
use crate::model::{
    Attribute, Category, Constellation, Effect, Faction, Group, MarketGroup, NpcCorporation,
    NpcStation, Region, SolarSystem, Type,
};
use crate::tables::Record;
#[cfg(any(feature = "sqlite", feature = "store"))]
use crate::tables::sealed::Sealed;
use std::borrow::Borrow;
use std::fmt;
use std::io::{Read, Seek};
use std::path::Path;
#[cfg(feature = "sqlite")]
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The SDE, held in memory or in a store on disk, with the same lookups
/// either way.
///
/// ```no_run
/// use eve_sde::Sde;
/// use eve_sde::ids::{SystemId, TypeId};
///
/// let sde = Sde::load("sde.zip")?; // every table in memory
/// # #[cfg(feature = "store")]
/// let sde = Sde::open_store("sde")?; // or the newest store in a folder
/// # #[cfg(feature = "sqlite")]
/// let sde = Sde::open_sqlite("sde")?; // or the newest SQLite one
///
/// sde.type_id("Tritanium")?; // Some(TypeId(34))
/// sde.system_id("Jita")?; // Some(SystemId(30_000_142))
/// sde.neighbors(SystemId(30_000_142))?; // the systems one gate away
/// sde.name(60_003_760)?; // Some("Jita IV - Moon 4 - Caldari Navy Assembly Plant")
/// sde.attribute_named(TypeId(587), "hiSlots")?; // Some(3.0)
/// # Ok::<(), eve_sde::Error>(())
/// ```
///
/// [`Sde::load`] reads every table into memory, which takes about 1.6 s and
/// 350 MiB for a whole SDE. [`Sde::open_store`] and [`Sde::open_sqlite`]
/// instead open a store that an import filled, in well under a millisecond
/// and a few MiB, and read records from it as the lookups ask for them.
///
/// Lookups return owned values, because a store cannot lend out its rows.
/// An `Sde` is `Send` and `Sync`, so threads can share one through an
/// `Arc`. A packed store needs no lock for that; a SQLite one is behind a
/// mutex, so its lookups take turns.
///
/// # Errors
///
/// Every lookup returns a `Result`. In memory it never fails. A store on
/// disk fails with [`Error::Io`](crate::Error::Io) or
/// [`Error::Sqlite`](crate::Error::Sqlite) if it cannot be read, and with
/// [`Error::Decode`](crate::Error::Decode) if a record does not fit its
/// model, which happens when CCP changes the format before this crate
/// catches up. It fails with [`Error::Invalid`](crate::Error::Invalid) for a
/// question it cannot answer rather than answering nothing: a table or an
/// index the store does not hold, keys of a shape the models do not expect,
/// or station names an import could not build. See
/// [Format changes](crate::store#format-changes).
pub struct Sde {
    build: BuildInfo,
    backend: Backend,
}

enum Backend {
    Memory(Box<memory::Memory>),
    #[cfg(feature = "sqlite")]
    Sqlite(Box<Mutex<crate::sqlite::Store>>),
    /// No lock: a packed store reads positionally, so it is `Sync`.
    #[cfg(feature = "store")]
    Store(Box<crate::store::Store>),
}

impl Sde {
    /// Read every table of an SDE ZIP into memory, and index it.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be opened, or for any reason listed under
    /// [`Sde::from_archive`].
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        fn load(path: &Path) -> Result<Sde> {
            Sde::from_archive(&mut Archive::open(path)?)
        }
        load(path.as_ref())
    }

    /// Read every table of an open archive into memory, and index it.
    ///
    /// # Errors
    ///
    /// Fails if a file cannot be read, a record does not parse, two records
    /// of a table share a key, or a size limit is exceeded. A table whose
    /// file is missing is empty.
    pub fn from_archive<R: Read + Seek>(archive: &mut Archive<R>) -> Result<Self> {
        let memory = memory::Memory::load(archive)?;
        Ok(Self {
            build: archive.build().clone(),
            backend: Backend::Memory(Box::new(memory)),
        })
    }

    /// Open the newest SQLite store in `dir`, as
    /// [`Store::open`](crate::sqlite::Store::open) does.
    ///
    /// # Errors
    ///
    /// Fails for the reasons listed under
    /// [`Store::open`](crate::sqlite::Store::open).
    #[cfg(feature = "sqlite")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
    pub fn open_sqlite(dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::from(crate::sqlite::Store::open(dir)?))
    }

    /// Open the newest packed store in `dir`, as
    /// [`Store::open`](crate::store::Store::open) does.
    ///
    /// Build one from a [`store::Store`](crate::store::Store) with
    /// [`Sde::from`] instead to keep what only the store itself offers, such
    /// as [`verify`](crate::store::Store::verify). The newer stores it
    /// passed over are [`Sde::skipped`].
    ///
    /// # Errors
    ///
    /// Fails for the reasons listed under
    /// [`Store::open`](crate::store::Store::open).
    #[cfg(feature = "store")]
    #[cfg_attr(docsrs, doc(cfg(feature = "store")))]
    pub fn open_store(dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::from(crate::store::Store::open(dir)?))
    }

    /// The build this SDE holds.
    #[must_use]
    pub fn build(&self) -> &BuildInfo {
        &self.build
    }

    /// The newer stores that [`Sde::open_store`] passed over because they
    /// would not open, newest first, as
    /// [`Store::skipped`](crate::store::Store::skipped) reports them.
    ///
    /// Not empty means this SDE is not the newest build its folder holds,
    /// and the newer file is damaged. Reading on is safe, but it is worth
    /// telling someone, and an import of the newer build replaces the bad
    /// file. Always empty for an SDE held in memory or in SQLite, and for a
    /// packed store opened by its path.
    #[cfg(feature = "store")]
    #[cfg_attr(docsrs, doc(cfg(feature = "store")))]
    #[must_use]
    pub fn skipped(&self) -> &[crate::store::Skipped] {
        match &self.backend {
            Backend::Store(store) => store.skipped(),
            Backend::Memory(_) => &[],
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(_) => &[],
        }
    }

    /// The record of `T` with this key, or `None` if there is none.
    ///
    /// ```no_run
    /// use eve_sde::Sde;
    /// use eve_sde::ids::TypeId;
    /// use eve_sde::model::Type;
    ///
    /// let sde = Sde::load("sde.zip")?;
    /// let rifter: Option<Type> = sde.get::<Type>(TypeId(587))?;
    /// # Ok::<(), eve_sde::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn get<T: Record>(&self, key: impl Borrow<T::Key>) -> Result<Option<T>> {
        self.get_record(key.borrow())
    }

    /// The tables in this SDE that this crate has no model for, by file
    /// name. Not empty means CCP added a table this version does not know.
    ///
    /// Every backend works this out when it is opened, so asking costs a
    /// copy of the list and nothing else.
    #[must_use]
    pub fn unmodeled_files(&self) -> Vec<String> {
        match &self.backend {
            Backend::Memory(memory) => memory.unmodeled_files().to_vec(),
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(store) => lock(store).unmodeled_files().to_vec(),
            #[cfg(feature = "store")]
            Backend::Store(store) => store.unmodeled_files().to_vec(),
        }
    }

    /// The files of the archive this SDE does not hold, because the name
    /// each would take is a table's already, sorted.
    ///
    /// Not empty means CCP has shipped a file that this version of this
    /// crate cannot store, because reading it back would mean reading it as
    /// another table. The rest of the archive was imported as usual, and a
    /// version that has a model for the file, or another name for the table
    /// it collides with, holds it. Always empty for an SDE held in memory,
    /// which names no tables and so refuses no file.
    #[cfg(any(feature = "sqlite", feature = "store"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "sqlite", feature = "store"))))]
    #[must_use]
    pub fn refused_files(&self) -> Vec<String> {
        match &self.backend {
            Backend::Memory(_) => Vec::new(),
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(store) => lock(store).refused_files().to_vec(),
            #[cfg(feature = "store")]
            Backend::Store(store) => store.refused_files().to_vec(),
        }
    }

    /// The IDs of the records of `named` with this name, in the order the
    /// name lookups document.
    #[cfg_attr(
        not(any(feature = "sqlite", feature = "store")),
        expect(
            clippy::unnecessary_wraps,
            reason = "only a store on disk can fail, and that needs a feature"
        )
    )]
    fn ids_named(&self, named: Named, name: &str) -> Result<Vec<u32>> {
        let key = name_key(name);
        match &self.backend {
            Backend::Memory(memory) => Ok(memory.ids_named(named, &key)),
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(store) => lock(store).ids_named(named, &key),
            #[cfg(feature = "store")]
            Backend::Store(store) => store.ids_named(named, &key),
        }
    }

    /// The IDs of the records under `parent`, ascending.
    #[cfg_attr(
        not(any(feature = "sqlite", feature = "store")),
        expect(
            clippy::unnecessary_wraps,
            reason = "only a store on disk can fail, and that needs a feature"
        )
    )]
    fn children(&self, relation: Relation, parent: u32) -> Result<Vec<u32>> {
        match &self.backend {
            Backend::Memory(memory) => Ok(memory.children(relation, parent)),
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(store) => lock(store).children(relation, parent),
            #[cfg(feature = "store")]
            Backend::Store(store) => store.children(relation, parent),
        }
    }
}

/// Use a store, such as one from
/// [`Store::in_memory`](crate::sqlite::Store::in_memory), with the lookups of
/// [`Sde`].
#[cfg(feature = "sqlite")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
impl From<crate::sqlite::Store> for Sde {
    fn from(store: crate::sqlite::Store) -> Self {
        Self {
            build: store.build().clone(),
            backend: Backend::Sqlite(Box::new(Mutex::new(store))),
        }
    }
}

/// Use a packed store, such as one opened by its path with
/// [`Store::open_file`](crate::store::Store::open_file), with the lookups of
/// [`Sde`].
#[cfg(feature = "store")]
#[cfg_attr(docsrs, doc(cfg(feature = "store")))]
impl From<crate::store::Store> for Sde {
    fn from(store: crate::store::Store) -> Self {
        Self {
            build: store.build().clone(),
            backend: Backend::Store(Box::new(store)),
        }
    }
}

/// Shows the build and where the tables are.
impl fmt::Debug for Sde {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let backend = match &self.backend {
            Backend::Memory(_) => "memory",
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(_) => "sqlite",
            #[cfg(feature = "store")]
            Backend::Store(_) => "store",
        };
        f.debug_struct("Sde")
            .field("build", &self.build)
            .field("backend", &backend)
            .finish_non_exhaustive()
    }
}

/// The store, even if a thread panicked while it held the lock: lookups only
/// read, so a panic cannot leave the store half changed.
#[cfg(feature = "sqlite")]
fn lock(store: &Mutex<crate::sqlite::Store>) -> MutexGuard<'_, crate::sqlite::Store> {
    store.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Where the lookups get records. The naming rules work on any source, so
/// the SQLite import can name stations before an `Sde` exists.
pub(crate) trait Source {
    /// The record of `T` with this key.
    fn get_record<T: Record>(&self, key: &T::Key) -> Result<Option<T>>;
}

impl Source for Sde {
    fn get_record<T: Record>(&self, key: &T::Key) -> Result<Option<T>> {
        match &self.backend {
            Backend::Memory(memory) => Ok(memory.get(key)),
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(store) => lock(store).get::<T>(key),
            #[cfg(feature = "store")]
            Backend::Store(store) => store.get::<T>(key),
        }
    }
}

/// How names are compared: without surrounding whitespace, in lower case.
pub(crate) fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Which records come first among those that share a name. Lower IDs come
/// next in every case.
#[cfg(any(feature = "sqlite", feature = "store"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Preferred {
    /// Published records.
    Published,
    /// Records not marked deleted.
    NotDeleted,
    /// No record is preferred.
    None,
}

/// Define `Named` from one list of tables, so that a table cannot join the
/// name lookups without `Named::ALL` gaining it too. A table left out of
/// `ALL` gets no name index when a packed store is imported, and every
/// search of it then fails at read.
macro_rules! named_tables {
    ($($variant:ident => $model:ty, $preferred:ident;)*) => {
        /// A table that the name lookups search.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum Named {
            $($variant,)*
        }

        #[cfg(any(feature = "sqlite", feature = "store"))]
        impl Named {
            /// Every table the name lookups search. Only the packed store
            /// needs them as a list: the SQLite one asks SQLite for each in
            /// turn.
            #[cfg(feature = "store")]
            pub(crate) const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub(crate) fn table(self) -> &'static str {
                match self {
                    $(Self::$variant => <$model as Sealed>::TABLE,)*
                }
            }

            pub(crate) fn preferred(self) -> Preferred {
                match self {
                    $(Self::$variant => Preferred::$preferred,)*
                }
            }
        }
    };
}

named_tables! {
    Type => Type, Published;
    Group => Group, Published;
    Category => Category, Published;
    MarketGroup => MarketGroup, None;
    System => SolarSystem, None;
    Constellation => Constellation, None;
    Region => Region, None;
    Station => NpcStation, None;
    Faction => Faction, None;
    Corporation => NpcCorporation, NotDeleted;
    Attribute => Attribute, None;
    Effect => Effect, None;
}

/// Define `Relation` from one list, for the reason `named_tables` exists: a
/// relation missing from `Relation::ALL` would have no children index in a
/// packed store, and its lookup would fail at read.
macro_rules! relations {
    ($($(#[$meta:meta])* $variant:ident => $model:ty, $field:literal;)*) => {
        /// Records listed under a parent record.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum Relation {
            $($(#[$meta])* $variant,)*
        }

        #[cfg(any(feature = "sqlite", feature = "store"))]
        impl Relation {
            /// Every relation the lookups follow: the packed store builds
            /// an index for each, and the SQLite store an SQL index.
            pub(crate) const ALL: &'static [Self] = &[$(Self::$variant,)*];

            /// The child table, and the field in it that holds the parent's
            /// ID.
            pub(crate) fn child(self) -> (&'static str, &'static str) {
                match self {
                    $(Self::$variant => (<$model as Sealed>::TABLE, $field),)*
                }
            }
        }
    };
}

relations! {
    /// The types of a group.
    TypesInGroup => Type, "groupID";
    /// The groups of a category.
    GroupsInCategory => Group, "categoryID";
}
