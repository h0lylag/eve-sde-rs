//! A local SQLite copy of the whole SDE. Needs the `sqlite` feature, or
//! `sqlite-bundled` to compile SQLite in.
//!
//! [`import`] writes every file of an archive into a new store file in a
//! folder. With the `download` feature, [`update`] first downloads the latest
//! build into the same folder, and [`update_with_progress`] reports each step
//! of it, such as for a progress bar. [`Sde::open_sqlite`](crate::Sde::open_sqlite)
//! then opens the newest store with every lookup of [`Sde`](crate::Sde), and
//! [`Store::open`] opens it for queries of your own.
//!
//! ```no_run
//! use eve_sde::{Archive, Sde};
//! use eve_sde::sqlite;
//!
//! let mut archive = Archive::open("sde.zip")?;
//! sqlite::import(&mut archive, "sde")?;
//! let sde = Sde::open_sqlite("sde")?;
//! let jita = sde.system_id("Jita")?; // Some(SystemId(30_000_142))
//! # Ok::<(), eve_sde::Error>(())
//! ```
//!
//! # Layout
//!
//! Every file of the archive is a table, including files this crate has no
//! model for, unless the name it would take is taken already (see
//! [Format changes](#format-changes)). A table with a model has the name that
//! [`tables`](crate::tables) gives it, such as `types` or `solar_systems`.
//! Any other table is named after its file in snake case, so
//! `newThings.jsonl` becomes `new_things`. Each table has four columns:
//!
//! - `key`: CCP's `_key`, the primary key. It is an integer, or text in the
//!   few files keyed by UUIDs.
//! - `name`: the record's English name, or `NULL`. NPC stations have the
//!   names the game shows, which the SDE leaves out, such as
//!   `Jita IV - Moon 4 - Caldari Navy Assembly Plant`.
//! - `name_key`: the name without surrounding whitespace, in lower case, for
//!   searches that ignore case. It is indexed in the tables that have names.
//! - `data`: the whole record as JSON, in CCP's shape but with the English
//!   text only, such as `{"_key":587,"name":{"en":"Rifter"},…}`.
//!
//! `types` and `groups` are also indexed by
//! `json_extract(data, '$.groupID')` and `json_extract(data, '$.categoryID')`.
//!
//! SQLite's JSON functions reach every field:
//!
//! ```sql
//! SELECT key, name FROM types WHERE data ->> '$.groupID' = 25;
//! ```
//!
//! Two more tables describe the store:
//!
//! - `sde_state`: one row with the `build`, its `release_date`, the
//!   `layout` ([`LAYOUT_VERSION`]), the `eve_sde` version that wrote it,
//!   `imported_at` in seconds since 1970, `station_names_error`, which is
//!   `NULL` unless the NPC stations have no names, and `refused_files`,
//!   which is `NULL` unless the import left a file out. Both are described
//!   under [Format changes](#format-changes).
//! - `sde_tables`: one row per table with its `name`, its `file` in the
//!   archive, whether this crate has a model for it (`modeled`), and its
//!   number of `rows`.
//!
//! # Files
//!
//! Each build is a file of its own, `sde-<build>-v<layout>.sqlite`, and is
//! never written again once it is complete. An import writes a temporary
//! `.sde-….part-…` file beside it and renames it when it is done. A reader
//! therefore never sees half an import, and nothing replaces a file that
//! another process has open, which Windows does not allow. After an import,
//! the files of older builds are deleted where possible; one that is still
//! open stays until a later import.
//!
//! # Format changes
//!
//! The import copies records without the models, so when CCP changes the
//! format before this crate's models catch up, [`Store::get`] fails for the
//! changed table only, and the import still succeeds.
//!
//! A file this crate has no model for becomes a table named after it. If
//! that name belongs to a modeled table, to `sde_state` or `sde_tables`, or
//! to another file already, the file is left out of the store and
//! [`Store::refused_files`] names it, because a table is found by name and
//! reading the file back would mean reading it as that other table. The rest
//! of the archive imports as usual, and the names are in
//! `sde_state.refused_files` as a JSON array.
//!
//! Station names are the one thing the import reads through the models: they
//! come from the stations, celestials, corporations and station operations.
//! If one of those records does not fit its model, the stations get no names
//! and `sde_state.station_names_error` says why. The station name searches of
//! [`Sde`](crate::Sde) then fail with [`Error::Invalid`]. A store is never
//! written again, so the names come back with the next build, or the next
//! layout, that a version of this crate with fitting models imports.
//!
//! # Threads
//!
//! A [`Store`] holds one SQLite connection. It can move to another thread
//! but cannot be shared, so open one per thread; opening takes well under a
//! millisecond.

use crate::archive::{Archive, BuildInfo};
use crate::error::{Error, PathContext, Result, error_chain, io_context};
use crate::ids::StationId;
use crate::model::NpcStation;
use crate::parts;
use crate::sde::{Named, Preferred, Relation, Source, name_key, naming};
use crate::tables::sealed::{Sealed, SealedKey};
use crate::tables::{Key, KeyKind, Record, table_name, tables};
use rusqlite::types::{ToSqlOutput, ValueRef};
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, ToSql, Transaction, params};
use serde_json::{Map, Value};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, DirEntry, OpenOptions};
use std::io::{self, Read, Seek};
#[cfg(feature = "download")]
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

/// The `rusqlite` this crate uses. A program can link only one version of
/// SQLite, so use this one, or the same version, for queries of your own.
pub use rusqlite;

/// The version of the store's layout.
///
/// It changes when a table or column changes, and is part of each store's
/// file name, so a new version of this crate does not open a store an older
/// one wrote. Import again instead.
pub const LAYOUT_VERSION: u32 = 2;

/// The file in which [`update`] keeps the ZIP, as
/// [`eve_sde::ZIP_FILE`](crate::ZIP_FILE).
#[cfg(feature = "download")]
#[cfg_attr(docsrs, doc(cfg(feature = "download")))]
pub use crate::update::ZIP_FILE;

/// The languages CCP writes text in, as in `LANGUAGES` in
/// `xtask/src/schema.rs`. An object that has `en` and otherwise only these
/// keys, or `_key`, is text.
const LANGUAGES: [&str; 9] = ["de", "en", "es", "fr", "it", "ja", "ko", "ru", "zh"];

/// The store's own tables. No SDE table may take their names.
const STATE_TABLE: &str = "sde_state";
const TABLES_TABLE: &str = "sde_tables";

/// What an import or an update did, as
/// [`eve_sde::ImportOutcome`](crate::ImportOutcome).
pub use crate::update::ImportOutcome;

/// Import `archive` into a new store file in `dir`, creating `dir` if it is
/// missing.
///
/// Nothing is imported if `dir` already holds this build or a newer one.
/// After an import, the store files of older builds and older layouts are
/// deleted where possible, as are temporary files that imports which died
/// left behind.
///
/// Two imports of the same build may run at once. Both do the work; the
/// first to finish wins, and the other returns [`ImportOutcome::UpToDate`].
///
/// # Errors
///
/// Fails if `dir` cannot be created or read, a file of the archive cannot be
/// read, a line is not JSON or has no `_key`, two records of a file share a
/// key, or SQLite fails. On failure the temporary file is deleted and `dir`
/// stays as it was. A file this crate has no model for whose name is taken
/// already is left out instead, as [`Store::refused_files`] describes.
pub fn import<R: Read + Seek>(
    archive: &mut Archive<R>,
    dir: impl AsRef<Path>,
) -> Result<ImportOutcome> {
    import_into(archive, dir.as_ref())
}

/// A step of [`update_with_progress`], as [`eve_sde::Step`](crate::Step).
#[cfg(feature = "download")]
#[cfg_attr(docsrs, doc(cfg(feature = "download")))]
pub use crate::update::Step;

/// Bring the store in `dir` up to the latest build: download it, then
/// [`import`] it.
///
/// Nothing is downloaded if `dir` already holds a store of the latest build
/// or a newer one, or if [`ZIP_FILE`] already holds the latest build. The ZIP
/// is kept, so a store of a new [`LAYOUT_VERSION`] can be imported from it
/// without a download. A [`ZIP_FILE`] that is not a readable SDE ZIP is
/// downloaded again.
///
/// # Errors
///
/// Fails if `dir` cannot be created or read, for the reasons listed under
/// [`Client::latest`](crate::download::Client::latest) and
/// [`Client::download`](crate::download::Client::download), and for those
/// listed under [`import`]. On failure the store stays as it was.
#[cfg(feature = "download")]
#[cfg_attr(docsrs, doc(cfg(feature = "download")))]
pub fn update(client: &crate::download::Client, dir: impl AsRef<Path>) -> Result<ImportOutcome> {
    update_with_progress(client, dir, |_| ControlFlow::Continue(()))
}

/// Like [`update`], reporting each [`Step`] to `progress`, such as for a
/// progress bar.
///
/// `progress` sees [`Step::Checking`] first, [`Step::Downloading`] after each
/// chunk of a download, and [`Step::Importing`] before the import. If it
/// returns `ControlFlow::Break(())`, the update stops with
/// [`Error::Cancelled`] and the store and [`ZIP_FILE`] stay as they were. The
/// import cannot be cancelled once it has begun.
///
/// ```no_run
/// use eve_sde::download::Client;
/// use eve_sde::sqlite::{self, Step};
/// use std::ops::ControlFlow;
///
/// let client = Client::new("my-app/1.0");
/// sqlite::update_with_progress(&client, "sde", |step| {
///     if let Step::Downloading { received, total: Some(total), .. } = step {
///         println!("{}%", received * 100 / total);
///     }
///     ControlFlow::Continue(())
/// })?;
/// # Ok::<(), eve_sde::Error>(())
/// ```
///
/// # Errors
///
/// Fails for the reasons listed under [`update`], and with
/// [`Error::Cancelled`] if `progress` asks to stop.
#[cfg(feature = "download")]
#[cfg_attr(docsrs, doc(cfg(feature = "download")))]
pub fn update_with_progress(
    client: &crate::download::Client,
    dir: impl AsRef<Path>,
    mut progress: impl FnMut(Step) -> ControlFlow<()>,
) -> Result<ImportOutcome> {
    crate::update::update_dir(client, dir.as_ref(), &mut progress, &Sqlite)
}

/// The SQLite store, as [`update`] drives it.
#[cfg(feature = "download")]
struct Sqlite;

#[cfg(feature = "download")]
impl crate::update::Local for Sqlite {
    fn already_holds(&self, dir: &Path, build: u32) -> Result<Option<ImportOutcome>> {
        already_holds(dir, build)
    }

    fn import(&self, archive: &mut Archive, dir: &Path) -> Result<ImportOutcome> {
        import_into(archive, dir)
    }
}

/// What an import of `build` into `dir` returns without importing: `Some` if
/// `dir` already holds a store of that build or a newer one.
fn already_holds(dir: &Path, build: u32) -> Result<Option<ImportOutcome>> {
    Ok(match newest_store(dir)? {
        Some((have, _)) if have == build => Some(ImportOutcome::UpToDate),
        Some((have, _)) if have > build => Some(ImportOutcome::OlderRefused),
        _ => None,
    })
}

fn import_into<R: Read + Seek>(archive: &mut Archive<R>, dir: &Path) -> Result<ImportOutcome> {
    let build = archive.build().build_number;
    fs::create_dir_all(dir).path_context("create", dir)?;
    if let Some(outcome) = already_holds(dir, build)? {
        return Ok(outcome);
    }

    let name = file_name(build);
    let part = tempfile::Builder::new()
        .prefix(&format!(".{name}.part-"))
        .tempfile_in(dir)
        .map_err(|e| {
            let context = format!("cannot create a temporary file in `{}`", dir.display());
            Error::Io(io_context(e, context))
        })?
        .into_temp_path();
    let mut connection = Connection::open(&part).map_err(Error::sqlite)?;
    write(&mut connection, archive)?;
    connection.close().map_err(|(_, e)| Error::sqlite(e))?;
    // SQLite wrote without syncing, so make the file durable before it
    // becomes visible. Windows needs write access to flush a file.
    OpenOptions::new()
        .write(true)
        .open(&part)
        .and_then(|file| file.sync_all())
        .path_context("write", &part)?;

    let dest = dir.join(&name);
    match part.persist_noclobber(&dest) {
        Ok(()) => {}
        // Another import of this build finished first.
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
            return Ok(ImportOutcome::UpToDate);
        }
        Err(e) => {
            let context = format!("cannot move the import to `{}`", dest.display());
            return Err(Error::Io(io_context(e.error, context)));
        }
    }
    remove_obsolete(dir, build);
    Ok(ImportOutcome::Imported)
}

/// A store: one build of the SDE in SQLite.
///
/// Open the newest store in a folder with [`Store::open`], or build one in
/// memory with [`Store::in_memory`]. Read a record with [`Store::get`], or
/// run your own queries on [`Store::connection`]; the
/// [module docs](crate::sqlite#layout) describe the tables.
pub struct Store {
    connection: Connection,
    build: BuildInfo,
    /// Why the import gave the NPC stations no names, if it did not.
    station_names_error: Option<String>,
    /// The kind of key each table is stored with, read on first use.
    key_kinds: RefCell<HashMap<&'static str, KeyKind>>,
    /// The files the import left out, because their names were taken.
    refused_files: Vec<String>,
    /// The files in the store this crate has no model for, sorted.
    unmodeled_files: Vec<String>,
}

impl Store {
    /// Open the newest store in `dir` that has this version's
    /// [`LAYOUT_VERSION`], read-only.
    ///
    /// # Errors
    ///
    /// Fails with [`Error::NoStore`] if `dir` is missing or holds no such
    /// store, and otherwise if `dir` cannot be read or SQLite fails.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        fn open(dir: &Path) -> Result<Store> {
            let Some((_, path)) = newest_store(dir)? else {
                return Err(Error::NoStore {
                    dir: dir.to_owned(),
                });
            };
            let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
            let connection = Connection::open_with_flags(path, flags).map_err(Error::sqlite)?;
            Store::from_connection(connection)
        }
        open(dir.as_ref())
    }

    /// Import `archive` into a store that lives in memory only, such as for
    /// tests.
    ///
    /// # Errors
    ///
    /// Fails for the reasons listed under [`import`] that concern the
    /// archive, or if SQLite fails.
    pub fn in_memory<R: Read + Seek>(archive: &mut Archive<R>) -> Result<Self> {
        let mut connection = Connection::open_in_memory().map_err(Error::sqlite)?;
        write(&mut connection, archive)?;
        Self::from_connection(connection)
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        let (build_number, release_date, layout, station_names_error) = connection
            .query_row(
                &format!(
                    "SELECT build, release_date, layout, station_names_error FROM {STATE_TABLE}"
                ),
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, u32>(2)?, row.get(3)?)),
            )
            .map_err(Error::sqlite)?;
        if layout != LAYOUT_VERSION {
            return Err(Error::Invalid(format!(
                "the store has layout {layout}, not {LAYOUT_VERSION}"
            )));
        }
        // A second query, after the layout check, so that a store of another
        // layout reports its layout rather than failing on a column that
        // layout does not have.
        let refused: Option<String> = connection
            .query_row(
                &format!("SELECT refused_files FROM {STATE_TABLE}"),
                [],
                |row| row.get(0),
            )
            .map_err(Error::sqlite)?;
        let refused_files = match refused {
            Some(json) => serde_json::from_str(&json).map_err(|e| {
                Error::Invalid(format!(
                    "the store's refused file names are not readable: {e}"
                ))
            })?,
            None => Vec::new(),
        };
        let unmodeled_files = connection
            .prepare(&format!(
                "SELECT file FROM {TABLES_TABLE} WHERE NOT modeled ORDER BY file"
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()
            })
            .map_err(Error::sqlite)?;
        let build = BuildInfo {
            build_number,
            release_date,
        };
        Ok(Self {
            connection,
            build,
            station_names_error,
            key_kinds: RefCell::new(HashMap::new()),
            refused_files,
            unmodeled_files,
        })
    }

    /// The build this store holds.
    pub fn build(&self) -> &BuildInfo {
        &self.build
    }

    /// The record of `T` with this key, or `None` if there is none.
    ///
    /// ```no_run
    /// # use eve_sde::sqlite::Store;
    /// use eve_sde::ids::SystemId;
    /// use eve_sde::model::SolarSystem;
    ///
    /// # let store = Store::open("sde")?;
    /// let jita = store.get::<SolarSystem>(SystemId(30_000_142))?;
    /// # Ok::<(), eve_sde::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`Error::Decode`] if the record does not fit `T`, which
    /// happens when CCP changes the format before this crate catches up, and
    /// with [`Error::Sqlite`] if the query fails.
    pub fn get<T: Record>(&self, key: impl Borrow<T::Key>) -> Result<Option<T>> {
        self.check_key_kind::<T>()?;
        get_record(&self.connection, key.borrow())
    }

    /// Fail if `T`'s table is keyed by text where `T` expects numbers, or
    /// the other way round.
    ///
    /// [`copy_table`] types the `key` column from the first record and not
    /// from the model, so a table whose `_key` CCP has changed is stored as
    /// the data has it. Binding the model's kind against that column finds
    /// no row, which is the one thing a lookup must not report as "no such
    /// record". The column type is read once per table per store.
    fn check_key_kind<T: Record>(&self) -> Result<()> {
        let table = <T as Sealed>::TABLE;
        let wanted = <T::Key as Key>::KIND;
        if let Some(stored) = self.key_kinds.borrow().get(table) {
            return mismatch(table, *stored, wanted);
        }
        // The declared type of the `key` column, which `create_table` wrote.
        let declared: Option<String> = self
            .connection
            .prepare_cached("SELECT type FROM pragma_table_info(?1) WHERE name = 'key'")
            .and_then(|mut statement| statement.query_row([table], |row| row.get(0)).optional())
            .map_err(Error::sqlite)?;
        let Some(declared) = declared else {
            return Err(Error::Invalid(format!(
                "the store has no table `{table}`: it was written by a version of \
                 eve_sde that had no model for it. Import again with this version."
            )));
        };
        let stored = if declared.eq_ignore_ascii_case("text") {
            KeyKind::Text
        } else {
            KeyKind::Integer
        };
        self.key_kinds.borrow_mut().insert(table, stored);
        mismatch(table, stored, wanted)
    }

    /// The files of the archive this store does not hold, because the name
    /// each would take is a table's already, sorted.
    ///
    /// Not empty means CCP has shipped a file that this version of this
    /// crate cannot store, because reading it back would mean reading it as
    /// another table, as one of the store's own two tables, or as another
    /// file. The rest of the archive was imported as usual, and a version
    /// that has a model for the file, or another name for the table it
    /// collides with, stores it. The names are in `sde_state.refused_files`
    /// as a JSON array, for a query of your own.
    #[must_use]
    pub fn refused_files(&self) -> &[String] {
        &self.refused_files
    }

    /// The SQLite connection, for queries of your own. It is read-only in a
    /// store opened with [`Store::open`].
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// The IDs in `named` whose `name_key` is `key`, preferred records
    /// first, then lower IDs. Fails for stations if the import could not name
    /// them, rather than finding none.
    pub(crate) fn ids_named(&self, named: Named, key: &str) -> Result<Vec<u32>> {
        if named == Named::Station
            && let Some(reason) = &self.station_names_error
        {
            return Err(Error::Invalid(format!(
                "the store has no station names: {reason}"
            )));
        }
        let table = named.table();
        let order = match named.preferred() {
            Preferred::Published => "coalesce(json_extract(data, '$.published'), 0) DESC, key",
            Preferred::NotDeleted => "coalesce(json_extract(data, '$.deleted'), 0), key",
            Preferred::None => "key",
        };
        let sql = format!(r#"SELECT key FROM "{table}" WHERE name_key = ?1 ORDER BY {order}"#);
        self.ids(&sql, key)
    }

    /// The IDs of the records under `parent`, ascending.
    pub(crate) fn children(&self, relation: Relation, parent: u32) -> Result<Vec<u32>> {
        let (table, field) = relation.child();
        let sql = format!(
            r#"SELECT key FROM "{table}" WHERE json_extract(data, '$.{field}') = ?1 ORDER BY key"#
        );
        self.ids(&sql, parent)
    }

    /// The integer keys that `sql` selects with one parameter.
    fn ids(&self, sql: &str, param: impl ToSql) -> Result<Vec<u32>> {
        let mut statement = self.connection.prepare_cached(sql).map_err(Error::sqlite)?;
        let rows = statement
            .query_map([param], |row| row.get(0))
            .map_err(Error::sqlite)?;
        rows.collect::<rusqlite::Result<_>>().map_err(Error::sqlite)
    }

    /// The files of the tables that have no model, sorted.
    /// Sorted names of the files in the store that this crate has no model
    /// for. Not empty means CCP added a table this version does not know.
    ///
    /// Each is a table of its own, named after it, and `sde_tables` describes
    /// it, so plain SQL reads it as well.
    #[must_use]
    pub fn unmodeled_files(&self) -> &[String] {
        &self.unmodeled_files
    }
}

/// Fail if a table's keys are not of the kind the model expects.
fn mismatch(table: &str, stored: KeyKind, wanted: KeyKind) -> Result<()> {
    if stored == wanted {
        return Ok(());
    }
    let name = |kind| match kind {
        KeyKind::Integer => "integer",
        KeyKind::Text => "text",
    };
    Err(Error::Invalid(format!(
        "table `{table}` has {} keys, but this version of eve_sde expects {} ones: \
         read it with plain SQL, or update eve_sde",
        name(stored),
        name(wanted),
    )))
}

/// The record of `T` with this key, from any connection to a store.
fn get_record<T: Record>(connection: &Connection, key: &T::Key) -> Result<Option<T>> {
    let table = <T as Sealed>::TABLE;
    let data: Option<String> = connection
        .prepare_cached(&format!(r#"SELECT data FROM "{table}" WHERE key = ?1"#))
        .and_then(|mut statement| {
            statement
                .query_row([key.sql_key()], |row| row.get(0))
                .optional()
        })
        .map_err(Error::sqlite)?;
    let Some(data) = data else {
        return Ok(None);
    };
    serde_json::from_str(&data)
        .map(Some)
        .map_err(|source| Error::Decode {
            table: table.to_owned(),
            key: format!("{key:?}"),
            source: Box::new(source),
        })
}

/// Records from a connection, for the naming rules during an import.
struct ConnectionSource<'c>(&'c Connection);

impl Source for ConnectionSource<'_> {
    fn get_record<T: Record>(&self, key: &T::Key) -> Result<Option<T>> {
        get_record(self.0, key)
    }
}

/// Shows the build only.
impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("build", &self.build)
            .finish_non_exhaustive()
    }
}

/// Write every file of `archive` and the store's own tables.
fn write<R: Read + Seek>(connection: &mut Connection, archive: &mut Archive<R>) -> Result<()> {
    // A new store becomes visible only when it is complete, and a failed
    // import deletes it, so building it needs no journal and no syncs.
    connection
        .execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")
        .map_err(Error::sqlite)?;
    let transaction = connection.transaction().map_err(Error::sqlite)?;
    let mut written = Vec::new();
    for table in tables() {
        let rows = if archive.has_file(table.file) {
            copy_table(&transaction, archive, table.name, table.file, table.key)?
        } else {
            // CCP removed the file. The table stays, empty, so queries
            // against it still run.
            create_table(&transaction, table.name, table.key)?;
            0
        };
        written.push(Written {
            name: table.name.to_owned(),
            file: table.file.to_owned(),
            modeled: true,
            rows,
        });
    }
    let mut names: HashSet<String> = written.iter().map(|table| table.name.clone()).collect();
    let mut refused_files = Vec::new();
    for file in archive.unmodeled_files().to_vec() {
        let Some(stem) = file.strip_suffix(".jsonl") else {
            continue;
        };
        let name = table_name(stem);
        let reserved = name == STATE_TABLE || name == TABLES_TABLE || name.starts_with("sqlite_");
        // A store holds its tables by name, so a file cannot take the name of
        // a table this crate models, of the store's own two tables, or of
        // another file: `get` for that model would read this file's records.
        // CCP shipping such a file must not fail the whole import, so the
        // file is left out and `sde_state` says which.
        if reserved || !names.insert(name.clone()) {
            refused_files.push(file);
            continue;
        }
        let rows = copy_table(&transaction, archive, &name, &file, KeyKind::Integer)?;
        written.push(Written {
            name,
            file,
            modeled: false,
            rows,
        });
    }
    refused_files.sort();
    let station_names_error = name_stations(&transaction)?;
    index_relations(&transaction)?;
    write_state(
        &transaction,
        archive.build(),
        &written,
        station_names_error.as_deref(),
        &refused_files,
    )?;
    transaction.commit().map_err(Error::sqlite)
}

/// Give every NPC station the name the game shows, which the SDE leaves out.
///
/// The names come from the stations, celestials, corporations and station
/// operations, read through their models. If a record they need does not fit
/// its model, the stations keep no names and the import
/// goes on, so a format change this crate has not caught up with does not
/// stop an update. The result is then why, for `sde_state`.
fn name_stations(transaction: &Transaction<'_>) -> Result<Option<String>> {
    let names = match station_names(transaction) {
        Ok(names) => names,
        Err(error @ Error::Decode { .. }) => return Ok(Some(error_chain(&error))),
        Err(error) => return Err(error),
    };
    if names.is_empty() {
        return Ok(None);
    }
    let table = <NpcStation as Sealed>::TABLE;
    let mut update = transaction
        .prepare(&format!(
            r#"UPDATE "{table}" SET name = ?1, name_key = ?2 WHERE key = ?3"#
        ))
        .map_err(Error::sqlite)?;
    for (id, name) in &names {
        update
            .execute(params![name, name_key(name), id.0])
            .map_err(Error::sqlite)?;
    }
    create_name_index(transaction, table)?;
    Ok(None)
}

/// The name of every NPC station, derived through the models. Writes
/// nothing, so a failure leaves every station unnamed.
fn station_names(transaction: &Transaction<'_>) -> Result<Vec<(StationId, String)>> {
    let table = <NpcStation as Sealed>::TABLE;
    let stations: Vec<NpcStation> = {
        let mut select = transaction
            .prepare(&format!(r#"SELECT key, data FROM "{table}""#))
            .map_err(Error::sqlite)?;
        let rows = select
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(Error::sqlite)?;
        let mut stations = Vec::new();
        for row in rows {
            let (key, data) = row.map_err(Error::sqlite)?;
            let station = serde_json::from_str(&data).map_err(|source| Error::Decode {
                table: table.to_owned(),
                key: key.to_string(),
                source: Box::new(source),
            })?;
            stations.push(station);
        }
        stations
    };
    let source = ConnectionSource(transaction);
    stations
        .iter()
        .map(|station| Ok((station.id, naming::station_name(&source, station)?)))
        .collect()
}

/// Index the parent IDs that [`Relation`] looks records up by.
fn index_relations(transaction: &Transaction<'_>) -> Result<()> {
    for relation in Relation::ALL {
        let (table, field) = relation.child();
        let index = format!("{table}_by_{field}");
        transaction
            .execute_batch(&format!(
                r#"CREATE INDEX "{index}" ON "{table}" (json_extract(data, '$.{field}'))"#
            ))
            .map_err(Error::sqlite)?;
    }
    Ok(())
}

fn create_name_index(transaction: &Transaction<'_>, table: &str) -> Result<()> {
    transaction
        .execute_batch(&format!(
            r#"CREATE INDEX IF NOT EXISTS "{table}_by_name" ON "{table}" (name_key)"#
        ))
        .map_err(Error::sqlite)
}

/// A table [`write`] wrote, for `sde_tables`.
struct Written {
    name: String,
    file: String,
    modeled: bool,
    rows: i64,
}

/// Copy `file` into a new table `table`, and return its number of rows.
/// The first record decides whether the keys are integers or text; an empty
/// file uses `empty_key`.
fn copy_table<R: Read + Seek>(
    transaction: &Transaction<'_>,
    archive: &mut Archive<R>,
    table: &str,
    file: &str,
    empty_key: KeyKind,
) -> Result<i64> {
    let mut lines = archive.lines(file)?;
    let Some(first) = lines.next_line() else {
        create_table(transaction, table, empty_key)?;
        return Ok(0);
    };
    let mut row = Row::parse(first, file)?;
    create_table(transaction, table, row.key.kind())?;
    let mut insert = transaction
        .prepare(&format!(
            r#"INSERT INTO "{table}" (key, name, name_key, data) VALUES (?1, ?2, ?3, ?4)"#
        ))
        .map_err(Error::sqlite)?;
    let mut rows = 0;
    let mut named = false;
    loop {
        named |= row.name.is_some();
        insert
            .execute(params![
                row.key,
                row.name,
                row.name.as_deref().map(name_key),
                row.data
            ])
            .map_err(|e| match e.sqlite_error_code() {
                Some(ErrorCode::ConstraintViolation) => Error::DuplicateKey {
                    file: file.to_owned(),
                    key: row.key.to_string(),
                },
                _ => Error::sqlite(e),
            })?;
        rows += 1;
        match lines.next_line() {
            Some(line) => row = Row::parse(line, file)?,
            None => break,
        }
    }
    if named {
        create_name_index(transaction, table)?;
    }
    Ok(rows)
}

fn create_table(transaction: &Transaction<'_>, table: &str, key: KeyKind) -> Result<()> {
    let key = match key {
        KeyKind::Integer => "INTEGER PRIMARY KEY",
        KeyKind::Text => "TEXT PRIMARY KEY NOT NULL",
    };
    transaction
        .execute_batch(&format!(
            r#"CREATE TABLE "{table}" (key {key}, name TEXT, name_key TEXT, data TEXT NOT NULL)"#
        ))
        .map_err(Error::sqlite)
}

fn write_state(
    transaction: &Transaction<'_>,
    build: &BuildInfo,
    tables: &[Written],
    station_names_error: Option<&str>,
    refused_files: &[String],
) -> Result<()> {
    // A JSON array, as every other record in the store is JSON, so
    // `json_each` reaches it. `NULL` rather than `[]` for the usual case.
    let refused = if refused_files.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(refused_files)
                .map_err(|e| Error::Invalid(format!("cannot write the refused file names: {e}")))?,
        )
    };
    transaction
        .execute_batch(&format!(
            "CREATE TABLE {STATE_TABLE} (build INTEGER NOT NULL, release_date TEXT, \
             layout INTEGER NOT NULL, eve_sde TEXT NOT NULL, imported_at INTEGER NOT NULL, \
             station_names_error TEXT, refused_files TEXT);
             CREATE TABLE {TABLES_TABLE} (name TEXT PRIMARY KEY NOT NULL, file TEXT NOT NULL, \
             modeled INTEGER NOT NULL, rows INTEGER NOT NULL);"
        ))
        .map_err(Error::sqlite)?;
    transaction
        .execute(
            &format!(
                "INSERT INTO {STATE_TABLE} \
                 VALUES (?1, ?2, ?3, ?4, CAST(strftime('%s', 'now') AS INTEGER), ?5, ?6)"
            ),
            params![
                build.build_number,
                build.release_date,
                LAYOUT_VERSION,
                env!("CARGO_PKG_VERSION"),
                station_names_error,
                refused,
            ],
        )
        .map_err(Error::sqlite)?;
    let mut insert = transaction
        .prepare(&format!(
            "INSERT INTO {TABLES_TABLE} VALUES (?1, ?2, ?3, ?4)"
        ))
        .map_err(Error::sqlite)?;
    for table in tables {
        insert
            .execute(params![table.name, table.file, table.modeled, table.rows])
            .map_err(Error::sqlite)?;
    }
    Ok(())
}

/// One record, ready to insert.
struct Row {
    key: RowKey,
    name: Option<String>,
    data: String,
}

impl Row {
    fn parse(line: Result<(usize, &str)>, file: &str) -> Result<Self> {
        let (number, text) = line?;
        let mut record: Value =
            serde_json::from_str(text).map_err(|source| Error::parse(file, number, source))?;
        let key = match record.get("_key") {
            Some(Value::Number(key)) => key.as_i64().map(RowKey::Integer),
            Some(Value::String(key)) => Some(RowKey::Text(key.clone())),
            _ => None,
        };
        let Some(key) = key else {
            return Err(Error::Invalid(format!(
                "`{file}` line {number} has no integer or text `_key`"
            )));
        };
        keep_english(&mut record);
        Ok(Self {
            key,
            name: english_name(&record),
            data: record.to_string(),
        })
    }
}

/// A record's `_key`.
enum RowKey {
    Integer(i64),
    Text(String),
}

impl RowKey {
    fn kind(&self) -> KeyKind {
        match self {
            Self::Integer(_) => KeyKind::Integer,
            Self::Text(_) => KeyKind::Text,
        }
    }
}

impl ToSql for RowKey {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            Self::Integer(key) => ToSqlOutput::Owned(rusqlite::types::Value::Integer(*key)),
            Self::Text(key) => ToSqlOutput::Borrowed(ValueRef::Text(key.as_bytes())),
        })
    }
}

impl fmt::Display for RowKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(key) => key.fmt(f),
            Self::Text(key) => write!(f, "{key:?}"),
        }
    }
}

/// Drop every language but English from the text in `value`, keeping CCP's
/// shape: `{"de": "…", "en": "Rifter"}` becomes `{"en": "Rifter"}`.
fn keep_english(value: &mut Value) {
    match value {
        Value::Object(map) if is_text(map) => map.retain(|key, _| key == "en" || key == "_key"),
        Value::Object(map) => map.values_mut().for_each(keep_english),
        Value::Array(items) => items.iter_mut().for_each(keep_english),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// Whether `map` is text in several languages, possibly with a `_key`, as
/// in CCP's lists of text by key.
fn is_text(map: &Map<String, Value>) -> bool {
    map.get("en").is_some_and(Value::is_string)
        && map
            .keys()
            .all(|key| key == "_key" || LANGUAGES.contains(&key.as_str()))
}

/// The English `name` of a record, after [`keep_english`].
fn english_name(record: &Value) -> Option<String> {
    match record.get("name")? {
        Value::String(name) => Some(name.clone()),
        Value::Object(text) => text.get("en")?.as_str().map(str::to_owned),
        _ => None,
    }
}

/// The file name of a store of `build` with this [`LAYOUT_VERSION`].
fn file_name(build: u32) -> String {
    format!("sde-{build}-v{LAYOUT_VERSION}.sqlite")
}

/// The build and layout in a store's file name.
fn parse_file_name(name: &str) -> Option<(u32, u32)> {
    let (build, layout) = name
        .strip_prefix("sde-")?
        .strip_suffix(".sqlite")?
        .split_once("-v")?;
    Some((build.parse().ok()?, layout.parse().ok()?))
}

/// The newest build in `dir` with this [`LAYOUT_VERSION`], and its file.
fn newest_store(dir: &Path) -> Result<Option<(u32, PathBuf)>> {
    let entries = match fs::read_dir(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        other => other.path_context("read", dir)?,
    };
    let mut newest: Option<(u32, PathBuf)> = None;
    for entry in entries {
        let entry = entry.path_context("read", dir)?;
        let name = entry.file_name();
        if let Some((build, LAYOUT_VERSION)) = name.to_str().and_then(parse_file_name)
            && newest.as_ref().is_none_or(|(have, _)| build > *have)
        {
            newest = Some((build, entry.path()));
        }
    }
    Ok(newest)
}

/// Delete what an import of `build` leaves obsolete in `dir`: the stores of
/// older builds of this layout, the stores of older layouts, and temporary
/// files of imports that died. A file that cannot be deleted, such as one
/// another process has open on Windows, stays until a later import.
fn remove_obsolete(dir: &Path, build: u32) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let obsolete = match parse_file_name(name) {
            Some((have, layout)) => {
                layout < LAYOUT_VERSION || (layout == LAYOUT_VERSION && have < build)
            }
            None => is_stale_part(name, &entry),
        };
        if obsolete {
            // Best effort: the next import tries again.
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Whether `entry` is a temporary file of an import that died.
fn is_stale_part(name: &str, entry: &DirEntry) -> bool {
    name.starts_with(".sde-") && name.contains(".sqlite.part-") && parts::is_stale(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_english_text_is_kept() {
        let mut record = json!({
            "_key": 587,
            "name": {"de": "Rifter", "en": "Rifter", "ja": "リフター"},
            "traits": [{"_key": 1, "en": "bonus", "fr": "bonus"}],
            "position": {"x": 1.0, "y": 2.0},
            "label": {"en": 5, "de": 6},
        });
        keep_english(&mut record);
        assert_eq!(
            record,
            json!({
                "_key": 587,
                "name": {"en": "Rifter"},
                "traits": [{"_key": 1, "en": "bonus"}],
                "position": {"x": 1.0, "y": 2.0},
                "label": {"en": 5, "de": 6},
            })
        );
    }

    #[test]
    fn names_are_english_text_or_plain_strings() {
        assert_eq!(
            english_name(&json!({"name": {"en": "Jita"}})).as_deref(),
            Some("Jita")
        );
        assert_eq!(
            english_name(&json!({"name": "Jita"})).as_deref(),
            Some("Jita")
        );
        assert_eq!(english_name(&json!({"name": 1})), None);
        assert_eq!(english_name(&json!({"_key": 1})), None);
    }

    #[test]
    fn store_file_names_hold_the_build_and_layout() {
        assert_eq!(
            parse_file_name(&file_name(3_579_973)),
            Some((3_579_973, LAYOUT_VERSION))
        );
        assert_eq!(parse_file_name("sde-12-v3.sqlite"), Some((12, 3)));
        assert_eq!(parse_file_name("sde-12.sqlite"), None);
        assert_eq!(parse_file_name(".sde-12-v1.sqlite.part-abc"), None);
        assert_eq!(parse_file_name("sde.zip"), None);
    }
}
