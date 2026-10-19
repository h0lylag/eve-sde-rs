//! [`Sde`]: the whole SDE behind one handle, with lookups by name, joins
//! between tables, and the names the SDE leaves out.

mod lookups;
mod memory;
pub(crate) mod naming;

use crate::archive::{Archive, BuildInfo};
use crate::error::Result;
#[cfg(feature = "sqlite")]
use crate::model::{
    Attribute, Category, Constellation, Effect, Faction, Group, MarketGroup, NpcCorporation,
    NpcStation, Region, SolarSystem, Type,
};
use crate::tables::Record;
#[cfg(feature = "sqlite")]
use crate::tables::sealed::Sealed;
use std::borrow::Borrow;
use std::fmt;
use std::io::{Read, Seek};
use std::path::Path;
#[cfg(feature = "sqlite")]
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The SDE, held in memory or in a SQLite store, with the same lookups
/// either way.
///
/// ```no_run
/// use eve_sde::Sde;
/// use eve_sde::ids::{SystemId, TypeId};
///
/// let sde = Sde::load("sde.zip")?; // every table in memory
/// # #[cfg(feature = "sqlite")]
/// let sde = Sde::open_sqlite("sde")?; // or the newest SQLite store in a folder
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
/// 350 MiB for a whole SDE. [`Sde::open_sqlite`], with the `sqlite` feature,
/// opens a store that [`sqlite::import`](crate::sqlite::import) or
/// [`sqlite::update`](crate::sqlite::update) filled, in under a millisecond
/// and a few MiB.
///
/// Lookups return owned values, because a store cannot lend out its rows.
/// An `Sde` is `Send` and `Sync`, so threads can share one through an
/// `Arc`.
///
/// # Errors
///
/// Every lookup returns a `Result`. In memory it never fails. A SQLite store
/// fails with [`Error::Sqlite`](crate::Error::Sqlite) if it cannot be read,
/// and with [`Error::Decode`](crate::Error::Decode) if a record does not fit
/// its model, which happens when CCP changes the format before this crate
/// catches up. If the import could not name the NPC stations for that
/// reason, the station name searches fail with
/// [`Error::Invalid`](crate::Error::Invalid); see
/// [Format changes](crate::sqlite#format-changes).
pub struct Sde {
    build: BuildInfo,
    backend: Backend,
}

enum Backend {
    Memory(Box<memory::Memory>),
    #[cfg(feature = "sqlite")]
    Sqlite(Mutex<crate::sqlite::Store>),
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

    /// The build this SDE holds.
    #[must_use]
    pub fn build(&self) -> &BuildInfo {
        &self.build
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
    /// # Errors
    ///
    /// See [`Sde`](Sde#errors).
    pub fn unmodeled_files(&self) -> Result<Vec<String>> {
        match &self.backend {
            Backend::Memory(memory) => Ok(memory.unmodeled_files()),
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(store) => lock(store).unmodeled_files(),
        }
    }

    /// The IDs of the records of `named` with this name, in the order the
    /// name lookups document.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(
            clippy::unnecessary_wraps,
            reason = "only the SQLite store can fail, and it needs the feature"
        )
    )]
    fn ids_named(&self, named: Named, name: &str) -> Result<Vec<u32>> {
        let key = name_key(name);
        match &self.backend {
            Backend::Memory(memory) => Ok(memory.ids_named(named, &key)),
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(store) => lock(store).ids_named(named, &key),
        }
    }

    /// The IDs of the records under `parent`, ascending.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(
            clippy::unnecessary_wraps,
            reason = "only the SQLite store can fail, and it needs the feature"
        )
    )]
    fn children(&self, relation: Relation, parent: u32) -> Result<Vec<u32>> {
        match &self.backend {
            Backend::Memory(memory) => Ok(memory.children(relation, parent)),
            #[cfg(feature = "sqlite")]
            Backend::Sqlite(store) => lock(store).children(relation, parent),
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
            backend: Backend::Sqlite(Mutex::new(store)),
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
        }
    }
}

/// How names are compared: without surrounding whitespace, in lower case.
pub(crate) fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// A table that the name lookups search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Named {
    Type,
    Group,
    Category,
    MarketGroup,
    System,
    Constellation,
    Region,
    Station,
    Faction,
    Corporation,
    Attribute,
    Effect,
}

/// Which records come first among those that share a name. Lower IDs come
/// next in every case.
#[cfg(feature = "sqlite")]
pub(crate) enum Preferred {
    /// Published records.
    Published,
    /// Records not marked deleted.
    NotDeleted,
    /// No record is preferred.
    None,
}

#[cfg(feature = "sqlite")]
impl Named {
    pub(crate) fn table(self) -> &'static str {
        match self {
            Self::Type => <Type as Sealed>::TABLE,
            Self::Group => <Group as Sealed>::TABLE,
            Self::Category => <Category as Sealed>::TABLE,
            Self::MarketGroup => <MarketGroup as Sealed>::TABLE,
            Self::System => <SolarSystem as Sealed>::TABLE,
            Self::Constellation => <Constellation as Sealed>::TABLE,
            Self::Region => <Region as Sealed>::TABLE,
            Self::Station => <NpcStation as Sealed>::TABLE,
            Self::Faction => <Faction as Sealed>::TABLE,
            Self::Corporation => <NpcCorporation as Sealed>::TABLE,
            Self::Attribute => <Attribute as Sealed>::TABLE,
            Self::Effect => <Effect as Sealed>::TABLE,
        }
    }

    pub(crate) fn preferred(self) -> Preferred {
        match self {
            Self::Type | Self::Group | Self::Category => Preferred::Published,
            Self::Corporation => Preferred::NotDeleted,
            Self::MarketGroup
            | Self::System
            | Self::Constellation
            | Self::Region
            | Self::Station
            | Self::Faction
            | Self::Attribute
            | Self::Effect => Preferred::None,
        }
    }
}

/// Records listed under a parent record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Relation {
    /// The types of a group.
    TypesInGroup,
    /// The groups of a category.
    GroupsInCategory,
}

impl Relation {
    /// The child table, and the JSON path of the parent's ID in it.
    #[cfg(feature = "sqlite")]
    pub(crate) fn column(self) -> (&'static str, &'static str) {
        match self {
            Self::TypesInGroup => (<Type as Sealed>::TABLE, "$.groupID"),
            Self::GroupsInCategory => (<Group as Sealed>::TABLE, "$.categoryID"),
        }
    }
}
