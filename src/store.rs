//! A local copy of the whole SDE in one file, with an index of its own.
//! Needs the `store` feature.
//!
//! [`import`] writes every file of an archive into a new store file in a
//! folder, and [`Store::open`] opens the newest one. Reading a record takes
//! one binary search and one read, so a store opens in about a millisecond
//! and costs a few KiB until something is looked up.
//!
//! A whole SDE imports in about 2.5 s into a file of about 525 MiB, so a
//! folder needs room for two while an import runs. [`Store::open`] takes
//! under 100 µs, a record about 2 µs once its table's index is loaded, and
//! that index costs one read of 24 bytes a row the first time a table is
//! asked for: about 1 MiB for `types`, 8 MiB for `moons`, nothing at all for
//! a table never touched. A lookup that walks several records, such as a
//! station's name, is tens of microseconds.
//!
//! [`Sde::open_store`](crate::Sde::open_store) gives the store every lookup
//! of [`Sde`](crate::Sde) — by name, along the item hierarchy, across the
//! map — with the same answers as the SDE held in memory. There is no lock:
//! a store reads positionally, so threads share one through an `Arc`.
//!
//! ```no_run
//! use eve_sde::{Archive, Sde, store};
//! use eve_sde::ids::TypeId;
//! use eve_sde::model::Type;
//!
//! let mut archive = Archive::open("sde.zip")?;
//! store::import(&mut archive, "sde")?;
//!
//! let store = store::Store::open("sde")?;
//! let rifter = store.get::<Type>(TypeId(587))?;
//!
//! let sde = Sde::open_store("sde")?; // or every lookup of `Sde`
//! let rifter = sde.type_id("Rifter")?;
//! # Ok::<(), eve_sde::Error>(())
//! ```
//!
//! # Layout
//!
//! One file per build, `sde-<build>-v<layout>.sde`. It holds, in this order,
//! a block for each file of the archive, then the header, then a footer of
//! 20 bytes. Nothing in it is ever written again once the import is done.
//!
//! The **footer** is read first: the header's offset as a little-endian
//! `u64`, its length as a little-endian `u32`, the CRC-32 of its bytes as a
//! little-endian `u32`, then the four bytes `ESDE`. The header ends where
//! the footer begins, so the offset and the length together say how long the
//! file should be, and a store that has lost or gained bytes is refused
//! rather than read. The checksum covers what a length cannot: the header
//! says where every record in the file is, so one changed byte in it could
//! otherwise send a lookup into another table's bytes, or make a whole table
//! read as empty, without changing the file's length at all.
//!
//! The **header** is one JSON object naming the build, the release date, the
//! layout, the `eve_sde` version that wrote it, `imported_at` in seconds
//! since 1970, and every table. A table has the name that
//! [`tables`](crate::tables) gives it, such as `types` or `solar_systems`, or
//! for a file this crate has no model for, its file name in snake case, so
//! `newThings.jsonl` becomes `new_things`. Each one names its file, whether
//! this crate has a model for it (`modeled`), its number of `rows`, whether
//! its keys are `integer` or `text`, and the spans of its records and of its
//! key index:
//!
//! ```json
//! {"name":"types","file":"types.jsonl","modeled":true,"rows":51682,
//!  "key":"integer","records":{"at":0,"len":148007829},
//!  "keys":{"at":148007829,"len":1033640}}
//! ```
//!
//! A table's **records** are the lines of its file, in CCP's shape and with
//! every language CCP ships, exactly as they came out of the archive, each
//! ending in a newline. A span of records is therefore a JSON Lines file of
//! its own: `tail -c +<at+1> … | head -c <len>` feeds it to `jq`.
//!
//! A table's **key index** is sorted by key, for a binary search. Every row
//! is 24 bytes and ends with the record's offset as a `u64`, its length as a
//! `u32` and the CRC-32 of its bytes as a `u32`. What comes first is the key:
//!
//! - `integer` keys: the key as an `i64`.
//! - `text` keys: the key's offset and length within the table's `key_blob`
//!   span, as two `u32`s.
//!
//! Every number is little-endian.
//!
//! The checksum is what tells a damaged record from a record this crate's
//! models have fallen behind: [`Store::get`] looks at it only once a record
//! has failed to parse, so a lookup that works pays nothing for it, and
//! [`Store::verify`] checks every record in the store against it.
//!
//! A table the name lookups search also has a **name index**, and the child
//! table of a relation a **children index**. Both are read the same way: a
//! binary search for the start of a run, then the run itself.
//!
//! - `names`: 12 bytes a row — the name's offset and length within the
//!   table's `name_blob` span as two `u32`s, then the record's key as a
//!   `u32`. Sorted by name, then by the rank that puts published (or
//!   undeleted) records first, then by key, which is the order
//!   [`Sde`](crate::Sde)'s name lookups answer in. The names are
//!   `name_key`s: trimmed, in lower case. NPC stations have the names the
//!   game shows, which the SDE leaves out.
//! - `children`: 8 bytes a row — the parent's key and the child's, as two
//!   `u32`s, sorted by both.
//!
//! A table has an index only if the version that wrote the store looked
//! things up that way; `rows` of zero is an index with nothing in it, which
//! is not the same thing. Both are `u32`-keyed, so a table whose `_key` is
//! text has neither.
//!
//! # Files
//!
//! Each build is a file of its own and is never written again once it is
//! complete. An import writes a temporary `.sde-….sde.part-…` file beside it
//! and renames it when it is done, so a reader never sees half an import, and
//! nothing replaces a file that another process has open, which Windows does
//! not allow. After an import, the files of older builds are deleted where
//! possible; one that is still open stays until a later import.
//!
//! A store that cannot be opened is skipped: [`Store::open`] takes the
//! newest one that opens and lists the newer ones it passed over in
//! [`Store::skipped`], so reading a stale build is never silent. [`import`]
//! neither counts a damaged store as a build the folder holds nor lets it
//! keep its name, so the repair is an ordinary import. Nothing else repairs a
//! store, because nothing else writes one. [`reimport`] replaces the store of
//! a build that CCP has published again under the same number.
//!
//! # Format changes
//!
//! The import copies records without the models, and reads only each line's
//! `_key`, so when CCP changes the format before this crate's models catch
//! up, [`Store::get`] fails for the changed record only and the import still
//! succeeds. [`Store::get_raw`] returns the line whatever its shape, and
//! [`Store::lines`] reads a whole table, including one this crate has no
//! model for. A record that fails to parse because the file was damaged
//! rather than because CCP moved on is reported as damage, not as
//! [`Error::Decode`], because the index holds its checksum.
//!
//! Tables are matched by name, not by position, so a store written by an
//! older version of this crate still answers for a table that has since
//! gained a model, as long as the model took the name the import had given
//! the file: the rule that names an unmodeled file is the one the model
//! generator uses. Where `schema/settings.toml` shortens a name — and it
//! does, `mapMoons.jsonl` becoming the table `moons` rather than
//! `map_moons` — the older store holds that file under the longer name, and
//! the lookup for the new one fails loudly and asks for an import. Three
//! changes are answered that way rather than with `None`:
//!
//! - the store has no table of that name at all, because the version that
//!   wrote it did not know the file, or knew it under another name. Import
//!   again with this version.
//! - the store has no name or children index for a table this version looks
//!   up that way, because the version that wrote it did not. Import again.
//! - the table's keys are text where the model expects numbers, or the other
//!   way round, because CCP changed `_key`. [`Store::lines`] still reads it.
//!
//! The import reads each line's `_key`, and for a table with a name or
//! children index the few fields those need, without the models: a field
//! whose shape has changed is left out of the index rather than failing the
//! import. Station names are the one thing read through the models, because
//! they are built from the stations, celestials, corporations and station
//! operations. If one of those does not fit, the stations get no names and
//! the header says why, and the station name searches of
//! [`Sde`](crate::Sde) then fail with [`Error::Invalid`]; the next build, or
//! the next layout, brings them back. A failure to read the part file being
//! written, on the other hand, fails the import.
//!
//! A file this crate has no model for is stored under the name the rule
//! above gives it. If a modeled table holds that name already, the file is
//! left out of the store and [`Store::refused_files`] names it, because a
//! table is found by name and reading the file back would mean reading it as
//! that other table. The rest of the archive imports as usual.
//!
//! # Threads
//!
//! [`Store`] reads at an offset without seeking, so it is `Send` and `Sync`
//! and threads can share one through an `Arc`. Several processes can read one
//! store at the same time.

use crate::archive::{Archive, BuildInfo, MAX_METADATA_BYTES, MAX_RECORD_BYTES, limit_error};
use crate::error::{Error, PathContext, Result, error_chain, io_context};
use crate::model::NpcStation;
use crate::parts;
use crate::sde::{Named, Preferred, Relation, Source, name_key, naming};
use crate::tables::sealed::{Sealed, SealedKey};
use crate::tables::{KeyKind, Record, StoreKey, table_name, tables};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs::{self, DirEntry, File};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek, Write};
#[cfg(feature = "download")]
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(not(any(unix, windows)))]
compile_error!(
    "the `store` feature needs positional file reads, which this crate has \
     only for Unix and Windows; build without `store`, or open an issue for \
     this target"
);

/// The version of the store's layout.
///
/// It changes when the header or a section changes, and is part of each
/// store's file name, so a new version of this crate does not open a store an
/// older one wrote. Import again instead.
pub const LAYOUT_VERSION: u32 = 3;

/// The last four bytes of a store file.
const MAGIC: [u8; 4] = *b"ESDE";
/// The footer: the header's offset, its length, its checksum, and [`MAGIC`].
const FOOTER_BYTES: u64 = 8 + 4 + 4 + 4;
/// Bytes per row of a key index, for either kind of key.
const ENTRY_BYTES: usize = 24;
/// Bytes per row of a name index.
const NAME_ENTRY_BYTES: usize = 12;
/// Bytes per row of a children index.
const CHILD_ENTRY_BYTES: usize = 8;
/// The footer is read into a fixed buffer of this size.
const _: () = assert!(FOOTER_BYTES == 20);

/// What an import or an update did, as [`eve_sde::ImportOutcome`](crate::ImportOutcome).
pub use crate::update::ImportOutcome;
/// A step of an [`update`], as [`eve_sde::Step`](crate::Step).
#[cfg(feature = "download")]
#[cfg_attr(docsrs, doc(cfg(feature = "download")))]
pub use crate::update::Step;
/// The file [`update`] keeps the ZIP in, as
/// [`eve_sde::ZIP_FILE`](crate::ZIP_FILE).
#[cfg(feature = "download")]
#[cfg_attr(docsrs, doc(cfg(feature = "download")))]
pub use crate::update::ZIP_FILE;

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
/// use eve_sde::store::{self, Step};
/// use std::ops::ControlFlow;
///
/// let client = Client::new("my-app/1.0");
/// store::update_with_progress(&client, "sde", |step| {
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
    crate::update::update_dir(client, dir.as_ref(), &mut progress, &Packed)
}

/// The packed store, as [`update`] drives it.
#[cfg(feature = "download")]
struct Packed;

#[cfg(feature = "download")]
impl crate::update::Local for Packed {
    fn already_holds(&self, dir: &Path, build: u32) -> Result<Option<ImportOutcome>> {
        already_holds(dir, build)
    }

    fn import(&self, archive: &mut Archive, dir: &Path) -> Result<ImportOutcome> {
        import_into(archive, dir)
    }
}

/// Import `archive` into a new store file in `dir`, creating `dir` if it is
/// missing.
///
/// Nothing is imported if `dir` already holds this build or a newer one in a
/// store that opens; a store that does not open is treated as absent and
/// replaced. Use [`reimport`] to replace a store that does open, which only
/// a build CCP published twice calls for. After an import, the store files of
/// older builds and older layouts are deleted where possible, as are
/// temporary files that imports which died left behind.
///
/// Two imports of the same build may run at once. Both do the work; the
/// first to finish wins, and the other returns [`ImportOutcome::UpToDate`].
///
/// # Errors
///
/// Fails if `dir` cannot be created or read, a file of the archive cannot be
/// read, a line is not JSON or has no `_key`, or two records of a file share
/// a key. On failure the temporary file is deleted and `dir` stays as it
/// was. A file this crate has no model for whose name a table holds already
/// is left out instead, as [`Store::refused_files`] describes.
pub fn import<R: Read + Seek>(
    archive: &mut Archive<R>,
    dir: impl AsRef<Path>,
) -> Result<ImportOutcome> {
    import_into(archive, dir.as_ref())
}

/// Import `archive` into `dir` even if it already holds this build, replacing
/// that store.
///
/// CCP publishes a build under one number, so [`import`] trusts the number
/// and does nothing when the folder already holds it. This is for the rare
/// case of a build republished under the same number, and for a store a
/// caller has reason to distrust.
///
/// A store of a **newer** build is left alone and still wins, because
/// [`Store::open`] takes the newest.
///
/// # Errors
///
/// Fails for the reasons listed under [`import`], and if the store it
/// replaces cannot be deleted, which on Windows includes one that another
/// process has open.
pub fn reimport<R: Read + Seek>(archive: &mut Archive<R>, dir: impl AsRef<Path>) -> Result<()> {
    let dir = dir.as_ref();
    let build = archive.build().build_number;
    fs::create_dir_all(dir).path_context("create", dir)?;
    let name = file_name(build);
    let part = write_part(archive, dir, &name)?;
    let dest = dir.join(&name);
    part.persist(&dest)
        .map_err(|e| cannot_move(e.error, &dest))?;
    finish(dir, build);
    Ok(())
}

fn import_into<R: Read + Seek>(archive: &mut Archive<R>, dir: &Path) -> Result<ImportOutcome> {
    let build = archive.build().build_number;
    fs::create_dir_all(dir).path_context("create", dir)?;
    if let Some(outcome) = already_holds(dir, build)? {
        return Ok(outcome);
    }

    let name = file_name(build);
    let part = write_part(archive, dir, &name)?;
    let dest = dir.join(&name);
    let part = match part.persist_noclobber(&dest) {
        Ok(()) => return Ok(finish(dir, build)),
        // Something took the name while this import ran.
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => e.path,
        Err(e) => return Err(cannot_move(e.error, &dest)),
    };

    // `already_holds` found no store of this build, so what is in the way is
    // either another import of it that finished first, or a damaged file.
    if Store::open_file(&dest).is_ok_and(|store| store.build.build_number == build) {
        return Ok(ImportOutcome::UpToDate);
    }
    fs::remove_file(&dest).path_context("replace the damaged store", &dest)?;
    match part.persist_noclobber(&dest) {
        Ok(()) => Ok(finish(dir, build)),
        // A third import got there in the moment the damaged file was gone.
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => Ok(ImportOutcome::UpToDate),
        Err(e) => Err(cannot_move(e.error, &dest)),
    }
}

/// Make a finished import durable, and clean up what it makes obsolete.
fn finish(dir: &Path, build: u32) -> ImportOutcome {
    sync_dir(dir);
    remove_obsolete(dir, build);
    ImportOutcome::Imported
}

fn cannot_move(source: io::Error, dest: &Path) -> Error {
    let context = format!("cannot move the import to `{}`", dest.display());
    Error::Io(io_context(source, context))
}

/// Write `archive` into a temporary file in `dir`, named after the store it
/// is to become.
fn write_part<R: Read + Seek>(
    archive: &mut Archive<R>,
    dir: &Path,
    name: &str,
) -> Result<tempfile::TempPath> {
    let part = tempfile::Builder::new()
        .prefix(&format!(".{name}.part-"))
        .tempfile_in(dir)
        .map_err(|e| {
            let context = format!("cannot create a temporary file in `{}`", dir.display());
            Error::Io(io_context(e, context))
        })?;
    write(part.as_file(), part.path(), archive).map_err(|e| {
        // Report the path the error happened on, not the temporary name.
        if let Error::Io(source) = e {
            Error::Io(io_context(source, format!("cannot write `{name}`")))
        } else {
            e
        }
    })?;
    Ok(part.into_temp_path())
}

/// What an import of `build` into `dir` returns without importing: `Some` if
/// `dir` already holds a store of that build or a newer one that opens. A
/// store that does not open is no answer to a lookup, so it does not count.
fn already_holds(dir: &Path, build: u32) -> Result<Option<ImportOutcome>> {
    let have = match Store::open(dir) {
        Ok(store) => store.build.build_number,
        // No store at all, or none that opens: the import goes ahead and
        // takes the damaged one's place.
        Err(Error::NoStore { .. } | Error::Invalid(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(match have.cmp(&build) {
        Ordering::Equal => Some(ImportOutcome::UpToDate),
        Ordering::Greater => Some(ImportOutcome::OlderRefused),
        Ordering::Less => None,
    })
}

/// Make a rename durable, so a crash cannot lose a complete store. Best
/// effort: the data is already synced, and the worst a lost rename costs is
/// an import. Windows has no handle for a directory to sync.
#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(handle) = File::open(dir) {
        let _ = handle.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

/// A store: one build of the SDE in one file.
///
/// Open the newest store in a folder with [`Store::open`], then read a
/// record with [`Store::get`] or its line with [`Store::get_raw`]. The
/// [module docs](crate::store#layout) describe the file.
pub struct Store {
    file: File,
    path: PathBuf,
    size: u64,
    header: Header,
    build: BuildInfo,
    /// The newer stores in the folder that would not open, newest first.
    skipped: Vec<Skipped>,
    /// The files in the store this crate has no model for, sorted.
    unmodeled_files: Vec<String>,
    /// Where each table is in `header.tables`, by name.
    by_name: HashMap<String, usize>,
    /// Each table's key index, read on first use.
    keys: Box<[OnceLock<Keys>]>,
    /// Each table's name index, read on first use.
    names: Box<[OnceLock<Names>]>,
    /// Each table's children index, read on first use.
    children: Box<[OnceLock<Children>]>,
}

impl Store {
    /// Open the newest store in `dir` that has this version's
    /// [`LAYOUT_VERSION`] and opens, read-only.
    ///
    /// A store that is damaged, or whose header does not hold the build its
    /// name claims, is skipped in favour of the next-newest, so one bad file
    /// does not take a folder out of service. If none opens, the error is the
    /// newest one's.
    ///
    /// # Errors
    ///
    /// Fails with [`Error::NoStore`] if `dir` is missing or holds no store of
    /// this layout, with [`Error::Invalid`] if none of them is a readable
    /// store of this layout, and otherwise if `dir` or a file cannot be read.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        fn open(dir: &Path) -> Result<Store> {
            let files = store_files(dir)?;
            if files.is_empty() {
                return Err(Error::NoStore {
                    dir: dir.to_owned(),
                });
            }
            let mut skipped: Vec<Skipped> = Vec::new();
            for (build, path) in files {
                let reason = match Store::open_file(&path) {
                    Ok(mut store) if store.header.build == build => {
                        store.skipped = skipped;
                        return Ok(store);
                    }
                    Ok(store) => Error::Invalid(format!(
                        "`{}` holds build {}, not the {build} its name claims",
                        path.display(),
                        store.header.build
                    )),
                    Err(e) => e,
                };
                skipped.push(Skipped {
                    build,
                    path,
                    reason,
                });
            }
            // None of them opened, so the newest one's reason is the answer.
            Err(skipped.into_iter().next().map_or_else(
                || Error::NoStore {
                    dir: dir.to_owned(),
                },
                |first| first.reason,
            ))
        }
        open(dir.as_ref())
    }

    /// Open one store file by its path, whatever its name.
    ///
    /// # Errors
    ///
    /// Fails for the reasons listed under [`Store::open`], except that a
    /// missing file is [`Error::Io`], and that the build in the file name is
    /// not checked.
    pub fn open_file(path: impl AsRef<Path>) -> Result<Self> {
        fn open(path: &Path) -> Result<Store> {
            let file = File::open(path).path_context("open", path)?;
            let head = read_head(&file, path)?;
            let Head { header, at, size } = head;
            if header.layout != LAYOUT_VERSION {
                return Err(Error::Invalid(format!(
                    "`{}` has layout {}, not {LAYOUT_VERSION}",
                    path.display(),
                    header.layout
                )));
            }
            let by_name: HashMap<String, usize> = header
                .tables
                .iter()
                .enumerate()
                .map(|(i, table)| (table.name.clone(), i))
                .collect();
            check_tables(&header, at, by_name.len(), path)?;
            Ok(Store::new(file, path.to_owned(), size, header, by_name))
        }
        open(path.as_ref())
    }

    /// A store over a part file an import has written the records and key
    /// indexes of, for the lookups that naming the stations makes. It opens
    /// the file again rather than sharing the import's handle, because on
    /// Windows a positional read moves a handle's own position.
    fn provisional(path: &Path, build: &BuildInfo, tables: &[TableHeader]) -> Result<Self> {
        let file = File::open(path).path_context("open", path)?;
        let header = Header {
            layout: LAYOUT_VERSION,
            eve_sde: env!("CARGO_PKG_VERSION").to_owned(),
            build: build.build_number,
            release_date: build.release_date.clone(),
            imported_at: 0,
            station_names_error: None,
            refused_files: Vec::new(),
            tables: tables.to_vec(),
        };
        let by_name = header
            .tables
            .iter()
            .enumerate()
            .map(|(i, table)| (table.name.clone(), i))
            .collect();
        Ok(Self::new(file, path.to_owned(), 0, header, by_name))
    }

    fn new(
        file: File,
        path: PathBuf,
        size: u64,
        header: Header,
        by_name: HashMap<String, usize>,
    ) -> Self {
        let build = BuildInfo {
            build_number: header.build,
            release_date: header.release_date.clone(),
        };
        let tables = header.tables.len();
        let mut unmodeled_files: Vec<String> = header
            .tables
            .iter()
            .filter(|table| !table.modeled)
            .map(|table| table.file.clone())
            .collect();
        unmodeled_files.sort();
        Self {
            file,
            path,
            size,
            header,
            build,
            unmodeled_files,
            skipped: Vec::new(),
            by_name,
            keys: cells(tables),
            names: cells(tables),
            children: cells(tables),
        }
    }

    /// The build this store holds.
    #[must_use]
    pub fn build(&self) -> &BuildInfo {
        &self.build
    }

    /// The file this store reads.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The newer stores in the folder that [`Store::open`] passed over
    /// because they would not open, newest first.
    ///
    /// Not empty means this store is not the newest build the folder holds,
    /// and the newer file is damaged. Reading on is safe — that is why they
    /// are skipped rather than fatal — but it is worth telling someone, and
    /// an [`import`] of the newer build will replace the bad file. Always
    /// empty for a store opened with [`Store::open_file`].
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.skipped
    }

    /// The size of the file, in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// When the import that wrote this store ran.
    #[must_use]
    pub fn imported_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.header.imported_at)
    }

    /// The version of this crate that wrote this store, such as `0.3.0`.
    #[must_use]
    pub fn written_by(&self) -> &str {
        &self.header.eve_sde
    }

    /// Every table in the store, in the order it was written: the tables of
    /// [`tables`](crate::tables) first, then the files this crate had no
    /// model for when the store was written.
    pub fn tables(&self) -> impl Iterator<Item = StoredTable<'_>> {
        self.header.tables.iter().map(StoredTable::new)
    }

    /// One table by the name [`tables`](crate::tables) gives it, such as
    /// `types`, or `None` if this store has no such table.
    #[must_use]
    pub fn table(&self, name: &str) -> Option<StoredTable<'_>> {
        let index = *self.by_name.get(name)?;
        self.header.tables.get(index).map(StoredTable::new)
    }

    /// The files of the archive this store does not hold, because the name
    /// each would take is a table's already, sorted.
    ///
    /// Not empty means CCP has shipped a file that this version of this
    /// crate cannot store, because reading it back would mean reading it as
    /// another table. The rest of the archive was imported as usual, and a
    /// version that has a model for the file, or another name for the table
    /// it collides with, stores it. [`Store::lines`] cannot reach it: it is
    /// not in the file.
    #[must_use]
    pub fn refused_files(&self) -> &[String] {
        &self.header.refused_files
    }

    /// Sorted names of the files in the store that this crate has no model
    /// for. Not empty means CCP added a table this version does not know.
    ///
    /// [`Store::lines`] reads one, and [`Store::tables`] describes it.
    #[must_use]
    pub fn unmodeled_files(&self) -> &[String] {
        &self.unmodeled_files
    }

    /// The record of `T` with this key, or `None` if there is none.
    ///
    /// ```no_run
    /// # use eve_sde::store::Store;
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
    /// happens when CCP changes the format before this crate catches up,
    /// with [`Error::Invalid`] if the store has no such table or holds its
    /// keys in the other shape (see
    /// [Format changes](crate::store#format-changes)), and with
    /// [`Error::Io`] if the store cannot be read.
    pub fn get<T: Record>(&self, key: impl Borrow<T::Key>) -> Result<Option<T>> {
        let table = <T as Sealed>::TABLE;
        let key = key.borrow();
        let Some((line, crc)) = self.raw(table, key.store_key())? else {
            return Ok(None);
        };
        match serde_json::from_slice(&line) {
            Ok(record) => Ok(Some(record)),
            Err(source) => Err(self.not_a_record(table, &format!("{key:?}"), &line, crc, source)),
        }
    }

    /// The line of `T`'s record with this key, as CCP wrote it, without the
    /// newline. `None` if there is no such record.
    ///
    /// Unlike [`Store::get`] this does not need the record to fit its model,
    /// so it reaches a table whose records have changed shape. It still needs
    /// the table's keys to be of the shape `T` expects; [`Store::lines`]
    /// reads a table whose keys have changed.
    ///
    /// # Errors
    ///
    /// Fails for the reasons listed under [`Store::get`], except
    /// [`Error::Decode`], and with [`Error::Invalid`] if the line is not
    /// UTF-8.
    pub fn get_raw<T: Record>(&self, key: impl Borrow<T::Key>) -> Result<Option<String>> {
        let table = <T as Sealed>::TABLE;
        let key = key.borrow();
        let Some((line, crc)) = self.raw(table, key.store_key())? else {
            return Ok(None);
        };
        // The import read every line as UTF-8, so one that is not is damage.
        String::from_utf8(line).map(Some).map_err(|e| {
            self.record_damaged(table, &format!("{key:?}"), e.as_bytes(), crc)
                .unwrap_or_else(|| {
                    Error::Invalid(format!("a record of table `{table}` is not UTF-8"))
                })
        })
    }

    /// Check every record against the checksum its index holds, and fail on
    /// the first that does not match.
    ///
    /// Reads the whole store, so about a second for a whole SDE, and loads
    /// every key index. [`Store::get`] checks a record's checksum only when
    /// it has already failed to parse, so this is the way to ask whether a
    /// store is sound before trusting it.
    ///
    /// It says nothing about whether records fit their models: a store CCP's
    /// format has moved on from is sound, and [`Store::get`] reports that
    /// with [`Error::Decode`].
    ///
    /// # Errors
    ///
    /// Fails with [`Error::Invalid`] naming the first record whose bytes do
    /// not match their checksum, and with [`Error::Io`] if the store cannot
    /// be read.
    pub fn verify(&self) -> Result<()> {
        let mut line = Vec::new();
        for (index, table) in self.header.tables.iter().enumerate() {
            let keys = self.table_keys(index)?;
            for row in 0..keys.rows() {
                let record = keys.record(row);
                line.clear();
                line.resize(record.len as usize, 0);
                self.read_exact_at(&mut line, record.at)?;
                if let Some(damaged) =
                    self.record_damaged(&table.name, &keys.key(row).to_string(), &line, record.crc)
                {
                    return Err(damaged);
                }
            }
        }
        Ok(())
    }

    /// Every record of one table, in the order CCP shipped them, as the lines
    /// of its file without their newlines.
    ///
    /// This reads the table's records straight through and touches no index,
    /// so it costs one buffer however big the table is, and it reaches a
    /// table whatever has changed about it — including a file this crate has
    /// no model for, which [`Store::unmodeled_files`] lists.
    ///
    /// ```no_run
    /// # use eve_sde::store::Store;
    /// # let store = Store::open("sde")?;
    /// for line in store.lines("types")? {
    ///     let line = line?;
    ///     // One JSON object per line, as CCP wrote it.
    /// }
    /// # Ok::<(), eve_sde::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`Error::Invalid`] if the store has no table of that name.
    /// Each line fails with [`Error::Io`] if the store cannot be read, and
    /// with [`Error::Invalid`] if it is not UTF-8.
    pub fn lines(&self, table: &str) -> Result<Lines<'_>> {
        let index = *self
            .by_name
            .get(table)
            .ok_or_else(|| self.missing_table(table))?;
        let table = &self.header.tables[index];
        Ok(Lines {
            table: &table.name,
            path: &self.path,
            reader: BufReader::new(SpanReader {
                file: &self.file,
                at: table.records.at,
                left: table.records.len,
            }),
            done: false,
        })
    }

    /// The IDs in `named` whose `name_key` is `key`, preferred records
    /// first, then lower IDs: the order the index was written in.
    pub(crate) fn ids_named(&self, named: Named, key: &str) -> Result<Vec<u32>> {
        if named == Named::Station
            && let Some(reason) = &self.header.station_names_error
        {
            return Err(Error::Invalid(format!(
                "the store has no station names: {reason}"
            )));
        }
        let table = named.table();
        let table_index = self.table_index(table)?;
        let Some(span) = self.header.tables[table_index].names else {
            return Err(self.missing_index("name index", table));
        };
        let blob = self.header.tables[table_index].name_blob;
        let found = self.cached(&self.names, table_index, || {
            self.read_names(table_index, span, blob)
        })?;
        Ok(found.find(key.as_bytes()))
    }

    /// The IDs of the records under `parent`, ascending.
    pub(crate) fn children(&self, relation: Relation, parent: u32) -> Result<Vec<u32>> {
        let (table, _) = relation.child();
        let index = self.table_index(table)?;
        let Some(span) = self.header.tables[index].children else {
            return Err(self.missing_index("children index", table));
        };
        let children = self.cached(&self.children, index, || self.read_children(index, span))?;
        Ok(children.find(parent))
    }

    /// Where `table` is in the header.
    fn table_index(&self, table: &str) -> Result<usize> {
        self.by_name
            .get(table)
            .copied()
            .ok_or_else(|| self.missing_table(table))
    }

    /// The bytes of the record of `table` with this key, without the
    /// newline, and the checksum its index holds for them.
    fn raw(&self, table: &str, key: StoreKey<'_>) -> Result<Option<(Vec<u8>, u32)>> {
        let index = *self
            .by_name
            .get(table)
            .ok_or_else(|| self.missing_table(table))?;
        let stored = self.header.tables[index].key;
        if stored != KeyFormat::from(key.kind()) {
            return Err(Error::Invalid(format!(
                "table `{table}` in `{}` has {stored} keys, but this version of eve_sde \
                 expects {} ones: read it with `Store::lines`, or update eve_sde",
                self.path.display(),
                KeyFormat::from(key.kind()),
            )));
        }
        let Some(record) = self.table_keys(index)?.find(key) else {
            return Ok(None);
        };
        let mut line = vec![0; record.len as usize];
        self.read_exact_at(&mut line, record.at)?;
        Ok(Some((line, record.crc)))
    }

    /// The key index of the table at `index` in the header, read on first
    /// use.
    fn table_keys(&self, index: usize) -> Result<&Keys> {
        self.cached(&self.keys, index, || self.read_keys(index))
    }

    /// One of a table's indexes, read on first use by `read`.
    ///
    /// `OnceLock::get_or_try_init` is unstable, so a failed read is not
    /// cached. A race reads twice and keeps the first result.
    fn cached<'a, T>(
        &self,
        cells: &'a [OnceLock<T>],
        index: usize,
        read: impl FnOnce() -> Result<T>,
    ) -> Result<&'a T> {
        let cell = cells
            .get(index)
            .ok_or_else(|| self.damaged("the header names more tables than the store holds"))?;
        if let Some(loaded) = cell.get() {
            return Ok(loaded);
        }
        let loaded = read()?;
        Ok(cell.get_or_init(|| loaded))
    }

    /// Read one table's name index and the blob its rows point into.
    fn read_names(&self, index: usize, span: Span, blob: Option<Span>) -> Result<Names> {
        let names = Names {
            entries: self.read_span(span)?,
            blob: self.read_span(blob.unwrap_or(Span { at: 0, len: 0 }))?,
        };
        if !names.is_whole() {
            return Err(self.damaged(&format!(
                "the name index of table `{}` is out of order or out of bounds",
                self.header.tables[index].name
            )));
        }
        Ok(names)
    }

    /// Read one table's children index.
    fn read_children(&self, index: usize, span: Span) -> Result<Children> {
        let children = Children {
            entries: self.read_span(span)?,
        };
        if !children.is_whole() {
            return Err(self.damaged(&format!(
                "the children index of table `{}` is out of order",
                self.header.tables[index].name
            )));
        }
        Ok(children)
    }

    /// Read one run of the file into memory.
    fn read_span(&self, span: Span) -> Result<Box<[u8]>> {
        let len = usize::try_from(span.len)
            .map_err(|_| self.damaged("an index is too big for this platform"))?;
        let mut bytes = vec![0; len];
        self.read_exact_at(&mut bytes, span.at)?;
        Ok(bytes.into_boxed_slice())
    }

    /// Read one table's key index as the bytes it is stored as. Searching
    /// those bytes in place costs no decode step, exactly the
    /// [`ENTRY_BYTES`] a row the file holds, and one allocation for the
    /// table.
    fn read_keys(&self, index: usize) -> Result<Keys> {
        let table = &self.header.tables[index];
        let blob = match table.key {
            KeyFormat::Integer => Span { at: 0, len: 0 },
            KeyFormat::Text => table.key_blob.unwrap_or(Span { at: 0, len: 0 }),
        };
        let keys = Keys {
            kind: table.key,
            entries: self.read_span(table.keys)?,
            blob: self.read_span(blob)?,
        };
        // A binary search over an index that is not sorted, or whose keys
        // point outside the blob, answers `None` for records the store
        // holds. Checking costs one pass over an index already in memory.
        if !keys.is_whole() {
            return Err(self.damaged(&format!(
                "the key index of table `{}` is out of order or out of bounds",
                table.name
            )));
        }
        Ok(keys)
    }

    /// Read exactly `buf.len()` bytes from `at`, without seeking, so that
    /// `&self` is enough and threads do not have to take turns.
    fn read_exact_at(&self, buf: &mut [u8], at: u64) -> Result<()> {
        read_exact_at(&self.file, buf, at).path_context("read", &self.path)
    }

    fn damaged(&self, what: &str) -> Error {
        Error::Invalid(format!("`{}` is damaged: {what}", self.path.display()))
    }

    /// Why a record did not parse: its bytes are damaged, or CCP changed the
    /// format. Only ever asked once something has gone wrong, so the happy
    /// path pays nothing for the checksum.
    fn not_a_record(
        &self,
        table: &str,
        key: &str,
        line: &[u8],
        crc: u32,
        source: serde_json::Error,
    ) -> Error {
        self.record_damaged(table, key, line, crc)
            .unwrap_or_else(|| Error::Decode {
                table: table.to_owned(),
                key: key.to_owned(),
                source: Box::new(source),
            })
    }

    /// `Some` if these bytes do not match the checksum the index holds for
    /// them, which means the file has been damaged since it was written.
    fn record_damaged(&self, table: &str, key: &str, line: &[u8], crc: u32) -> Option<Error> {
        (crc32(line) != crc).then(|| {
            self.damaged(&format!(
                "the record of table `{table}` with key {key} does not match its checksum"
            ))
        })
    }

    /// A table this version knows that the store does not hold. The version
    /// that wrote it had no model for the file, so it was not written.
    fn missing_table(&self, table: &str) -> Error {
        Error::Invalid(format!(
            "`{}` has no table `{table}`: it was written by eve_sde {}, \
             which had no model for it. Import again with this version.",
            self.path.display(),
            self.header.eve_sde
        ))
    }

    /// An index this version looks things up in that the store does not
    /// hold, because the version that wrote it did not build that one.
    fn missing_index(&self, what: &str, table: &str) -> Error {
        Error::Invalid(format!(
            "`{}` has no {what} for table `{table}`: it was written by eve_sde {}, \
             which did not build one. Import again with this version.",
            self.path.display(),
            self.header.eve_sde
        ))
    }
}

/// Lets the naming rules and [`Sde`](crate::Sde) read records from a store.
impl Source for Store {
    fn get_record<T: Record>(&self, key: &T::Key) -> Result<Option<T>> {
        self.get::<T>(key)
    }
}

/// Shows the build and the file only.
impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("build", &self.build)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// One table's key index, as the file holds it: [`ENTRY_BYTES`] a row,
/// ascending by key, searched in place.
struct Keys {
    kind: KeyFormat,
    entries: Box<[u8]>,
    /// The key bytes that text entries point into. Empty for integer keys.
    blob: Box<[u8]>,
}

impl Keys {
    /// How many rows the index holds.
    fn rows(&self) -> usize {
        self.entries.len() / ENTRY_BYTES
    }

    fn entry(&self, row: usize) -> &[u8] {
        entry(&self.entries, row, ENTRY_BYTES)
    }

    /// The key of one row, in the kind the table is indexed by. Text keys
    /// are compared as bytes, which is the order they were sorted in.
    fn key(&self, row: usize) -> StoreKey<'_> {
        let entry = self.entry(row);
        match self.kind {
            KeyFormat::Integer => StoreKey::Integer(le_i64(&entry[0..8])),
            // `is_whole` has checked that every key is inside the blob.
            KeyFormat::Text => StoreKey::Text(
                text_span(entry)
                    .and_then(|(at, end)| self.blob.get(at..end))
                    .unwrap_or_default(),
            ),
        }
    }

    fn record(&self, row: usize) -> RecordAt {
        let entry = self.entry(row);
        RecordAt {
            at: le_u64(&entry[8..16]),
            len: le_u32(&entry[16..20]),
            crc: le_u32(&entry[20..24]),
        }
    }

    /// Where the record with this key is, if the table holds it. The caller
    /// has already matched `key`'s kind against the table's.
    fn find(&self, key: StoreKey<'_>) -> Option<RecordAt> {
        let (mut low, mut high) = (0, self.rows());
        while low < high {
            let middle = low + (high - low) / 2;
            match self.key(middle).cmp(&key) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Some(self.record(middle)),
            }
        }
        None
    }

    /// Whether the index is one a binary search can answer with: whole
    /// entries, every text key inside the blob, and keys strictly ascending.
    fn is_whole(&self) -> bool {
        if !self.entries.len().is_multiple_of(ENTRY_BYTES) {
            return false;
        }
        if self.kind == KeyFormat::Text {
            let inside = |row: usize| {
                text_span(self.entry(row)).is_some_and(|(_, end)| end <= self.blob.len())
            };
            if !(0..self.rows()).all(inside) {
                return false;
            }
        }
        // Duplicates are refused at import, so equal keys are damage too.
        (1..self.rows()).all(|row| self.key(row - 1) < self.key(row))
    }
}

/// Shows the shape of an index, never the megabytes in it.
impl fmt::Debug for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keys")
            .field("kind", &self.kind)
            .field("rows", &self.rows())
            .finish_non_exhaustive()
    }
}

/// Where the key of a text entry starts and ends in its table's key blob.
fn text_span(entry: &[u8]) -> Option<(usize, usize)> {
    let at = usize::try_from(le_u32(&entry[0..4])).ok()?;
    let len = usize::try_from(le_u32(&entry[4..8])).ok()?;
    Some((at, at.checked_add(len)?))
}

/// One table's name index: 12 bytes a row, sorted by name, then by the rank
/// that puts preferred records first, then by key.
struct Names {
    entries: Box<[u8]>,
    /// The name bytes the rows point into.
    blob: Box<[u8]>,
}

impl Names {
    fn rows(&self) -> usize {
        self.entries.len() / NAME_ENTRY_BYTES
    }

    fn entry(&self, row: usize) -> &[u8] {
        entry(&self.entries, row, NAME_ENTRY_BYTES)
    }

    /// The `name_key` of one row. `is_whole` has checked that it is inside
    /// the blob.
    fn name(&self, row: usize) -> &[u8] {
        text_span(self.entry(row))
            .and_then(|(at, end)| self.blob.get(at..end))
            .unwrap_or_default()
    }

    fn id(&self, row: usize) -> u32 {
        le_u32(&self.entry(row)[8..12])
    }

    /// The IDs whose name is `name`, in the order they are stored.
    fn find(&self, name: &[u8]) -> Vec<u32> {
        let first = lower_bound(self.rows(), |row| self.name(row) >= name);
        (first..self.rows())
            .take_while(|row| self.name(*row) == name)
            .map(|row| self.id(row))
            .collect()
    }

    /// Whether a search can answer with this index: whole rows, every name
    /// inside the blob, and names in order. Equal names are a run, so the
    /// order is not strict.
    fn is_whole(&self) -> bool {
        self.entries.len().is_multiple_of(NAME_ENTRY_BYTES)
            && (0..self.rows()).all(|row| {
                text_span(self.entry(row)).is_some_and(|(_, end)| end <= self.blob.len())
            })
            && (1..self.rows()).all(|row| self.name(row - 1) <= self.name(row))
    }
}

/// One table's children index: a `u32` parent and a `u32` child a row,
/// sorted by both.
struct Children {
    entries: Box<[u8]>,
}

impl Children {
    fn rows(&self) -> usize {
        self.entries.len() / CHILD_ENTRY_BYTES
    }

    fn entry(&self, row: usize) -> &[u8] {
        entry(&self.entries, row, CHILD_ENTRY_BYTES)
    }

    fn parent(&self, row: usize) -> u32 {
        le_u32(&self.entry(row)[0..4])
    }

    fn child(&self, row: usize) -> u32 {
        le_u32(&self.entry(row)[4..8])
    }

    /// The children of `parent`, ascending.
    fn find(&self, parent: u32) -> Vec<u32> {
        let first = lower_bound(self.rows(), |row| self.parent(row) >= parent);
        (first..self.rows())
            .take_while(|row| self.parent(*row) == parent)
            .map(|row| self.child(row))
            .collect()
    }

    fn is_whole(&self) -> bool {
        self.entries.len().is_multiple_of(CHILD_ENTRY_BYTES)
            && (1..self.rows()).all(|row| {
                (self.parent(row - 1), self.child(row - 1)) <= (self.parent(row), self.child(row))
            })
    }
}

/// Shows the shape of an index, never the megabytes in it.
impl fmt::Debug for Names {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Names")
            .field("rows", &self.rows())
            .finish_non_exhaustive()
    }
}

/// Shows the shape of an index, never the megabytes in it.
impl fmt::Debug for Children {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Children")
            .field("rows", &self.rows())
            .finish_non_exhaustive()
    }
}

/// One `OnceLock` per table, for an index that is read on first use.
fn cells<T>(tables: usize) -> Box<[OnceLock<T>]> {
    (0..tables).map(|_| OnceLock::new()).collect()
}

/// One row of an index.
///
/// # Panics
///
/// Panics if `row` is not below the index's `rows()`, which is where every
/// caller takes its rows from. `rows()` is the length over the width, so
/// every row below it is whole, and answering with zeroes instead would turn
/// a bug here into a wrong answer there.
fn entry(entries: &[u8], row: usize, width: usize) -> &[u8] {
    &entries[row * width..][..width]
}

/// The first of `rows` for which `at_least` holds, given that it holds for
/// every row after it: a binary search for the start of a run.
fn lower_bound(rows: usize, at_least: impl Fn(usize) -> bool) -> usize {
    let (mut low, mut high) = (0, rows);
    while low < high {
        let middle = low + (high - low) / 2;
        if at_least(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// Where one record is in the store file, and the CRC-32 of its bytes.
#[derive(Debug, Clone, Copy)]
struct RecordAt {
    at: u64,
    len: u32,
    crc: u32,
}

/// The CRC-32 of a record's bytes, as the index holds it.
fn crc32(bytes: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

/// A store that [`Store::open`] passed over, from [`Store::skipped`].
#[derive(Debug)]
#[non_exhaustive]
pub struct Skipped {
    /// The build its file name claims.
    pub build: u32,
    /// The file.
    pub path: PathBuf,
    /// Why it would not open.
    pub reason: Error,
}

/// One table of a store, as its header describes it.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct StoredTable<'a> {
    /// The name [`tables`](crate::tables) gives the table, such as `types`,
    /// or for a file this crate had no model for when the store was written,
    /// its file name in snake case.
    pub name: &'a str,
    /// The file inside the archive, such as `types.jsonl`.
    pub file: &'a str,
    /// Whether the version that wrote the store had a model for the file.
    pub modeled: bool,
    /// How many records the table holds.
    pub rows: u64,
    /// Whether its keys are numbers or text.
    pub key: KeyKind,
}

impl<'a> StoredTable<'a> {
    fn new(table: &'a TableHeader) -> Self {
        Self {
            name: &table.name,
            file: &table.file,
            modeled: table.modeled,
            rows: table.rows,
            key: match table.key {
                KeyFormat::Integer => KeyKind::Integer,
                KeyFormat::Text => KeyKind::Text,
            },
        }
    }
}

/// One run of bytes of a store file, read positionally.
#[derive(Debug)]
struct SpanReader<'a> {
    file: &'a File,
    at: u64,
    left: u64,
}

impl Read for SpanReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Widening: `usize` is at most 64 bits on every target.
        let want = (buf.len() as u64).min(self.left);
        let want = usize::try_from(want).unwrap_or(buf.len());
        if want == 0 {
            return Ok(0);
        }
        read_exact_at(self.file, &mut buf[..want], self.at)?;
        // Widening: `usize` is at most 64 bits on every target.
        self.at += want as u64;
        self.left -= want as u64;
        Ok(want)
    }
}

/// The records of one table, from [`Store::lines`].
#[derive(Debug)]
pub struct Lines<'a> {
    table: &'a str,
    path: &'a Path,
    reader: BufReader<SpanReader<'a>>,
    /// A read that failed ends the iterator, rather than failing on and on.
    done: bool,
}

impl Iterator for Lines<'_> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => {
                self.done = true;
                None
            }
            Ok(_) => {
                let end = line.trim_end_matches(['\n', '\r']).len();
                line.truncate(end);
                Some(Ok(line))
            }
            Err(e) => {
                self.done = true;
                Some(Err(if e.kind() == io::ErrorKind::InvalidData {
                    Error::Invalid(format!(
                        "a record of table `{}` in `{}` is not UTF-8",
                        self.table,
                        self.path.display()
                    ))
                } else {
                    Error::Io(io_context(
                        e,
                        format!("cannot read `{}`", self.path.display()),
                    ))
                }))
            }
        }
    }
}

/// A run of bytes in the store file.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Span {
    at: u64,
    len: u64,
}

/// Whether a table's keys are stored as numbers or as text: [`KeyKind`] as
/// the header writes it. The two are separate so that the file format does
/// not change when that public enum does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum KeyFormat {
    Integer,
    Text,
}

impl From<KeyKind> for KeyFormat {
    fn from(kind: KeyKind) -> Self {
        match kind {
            KeyKind::Integer => Self::Integer,
            KeyKind::Text => Self::Text,
        }
    }
}

impl fmt::Display for KeyFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Integer => "integer",
            Self::Text => "text",
        })
    }
}

/// The store's header: one JSON object, named by the footer.
#[derive(Debug, Serialize, Deserialize)]
struct Header {
    layout: u32,
    eve_sde: String,
    build: u32,
    release_date: Option<String>,
    imported_at: u64,
    /// Why the import gave the NPC stations no names, if it did not. Always
    /// `None` until the names are built here.
    station_names_error: Option<String>,
    /// The files of the archive the import left out, because the name each
    /// would take is a modeled table's already.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    refused_files: Vec<String>,
    tables: Vec<TableHeader>,
}

/// One table in the header. `rows` is a `u64` rather than a `usize` because
/// the header is a file, read on any target by any later version.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableHeader {
    name: String,
    file: String,
    modeled: bool,
    rows: u64,
    key: KeyFormat,
    records: Span,
    keys: Span,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key_blob: Option<Span>,
    /// The name index, for a table the name lookups search. `None` means
    /// this store has none: either the table is not one of them, or the
    /// version that wrote it did not index it. An indexed table with no
    /// names has a span of zero length, so the two are never confused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    names: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name_blob: Option<Span>,
    /// The parent-to-child index, for the child table of a relation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    children: Option<Span>,
}

/// What the name and children indexes need from a table.
#[derive(Debug, Clone, Copy)]
struct Wanted {
    /// Which records come first among those that share a name, if the name
    /// lookups search this table.
    named: Option<Preferred>,
    /// The field holding the parent's ID, if this is a relation's child
    /// table.
    parent: Option<&'static str>,
}

impl Wanted {
    /// Nothing: the table is neither searched by name nor a child table.
    const NONE: Self = Self {
        named: None,
        parent: None,
    };

    /// What the indexes need from the table of this name.
    fn of(table: &str) -> Self {
        Self {
            named: Named::ALL
                .iter()
                .copied()
                .find(|named| named.table() == table)
                .map(Named::preferred),
            parent: Relation::ALL
                .iter()
                .copied()
                .map(Relation::child)
                .find(|(child, _)| *child == table)
                .map(|(_, field)| field),
        }
    }
}

/// What one table gave the name and children indexes, before they are
/// sorted and written.
#[derive(Debug, Default)]
struct Extracted {
    /// One row per record that has a name.
    names: Option<Vec<NameRow>>,
    /// One row per record that names a parent.
    children: Option<Vec<(u32, u32)>>,
}

/// One row of a name index, before it is written.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NameRow {
    /// The name as the lookups compare it: trimmed, in lower case.
    key: String,
    /// 0 for a record that comes first among those that share a name.
    rank: u8,
    /// The record's key.
    id: u32,
}

/// The fields the name and children indexes need, read without the models so
/// that a table whose records have changed shape still imports.
#[derive(Debug, Deserialize)]
struct Fields {
    #[serde(rename = "_key")]
    key: ParsedKeyDe,
    #[serde(default, deserialize_with = "lenient")]
    name: Option<EnglishName>,
    #[serde(default, deserialize_with = "lenient")]
    published: Option<bool>,
    #[serde(default, deserialize_with = "lenient")]
    deleted: Option<bool>,
    #[serde(default, rename = "groupID", deserialize_with = "lenient")]
    group_id: Option<u32>,
    #[serde(default, rename = "categoryID", deserialize_with = "lenient")]
    category_id: Option<u32>,
}

impl Fields {
    /// The record's English name, by the rule the SQLite store uses: a plain
    /// string, or the `en` of an object of text.
    fn name(&self) -> Option<&str> {
        match self.name.as_ref()? {
            EnglishName::Text(name) | EnglishName::Localized { en: name } => Some(name),
        }
    }

    /// The parent's ID in the field a [`Relation`] names. Every field of
    /// [`Relation::child`] has to be one of these, which
    /// `every_relations_field_is_read` checks.
    fn parent(&self, field: &str) -> Option<u32> {
        match field {
            "groupID" => self.group_id,
            "categoryID" => self.category_id,
            _ => None,
        }
    }
}

/// `"Rifter"` or `{"en": "Rifter", …}`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum EnglishName {
    Text(String),
    Localized { en: String },
}

/// Read a field if it has the shape this crate expects, and ignore it
/// otherwise. A field only the indexes use must not fail an import.
fn lenient<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(T::deserialize(value).ok())
}

/// Write every file of `archive` into `file` as a whole store. `path` is
/// where `file` can be opened again, which naming the stations needs.
fn write<R: Read + Seek>(file: &File, path: &Path, archive: &mut Archive<R>) -> Result<()> {
    let mut out = Out {
        writer: BufWriter::new(file),
        at: 0,
    };
    let mut tables_written = Vec::new();
    let mut extracted = Vec::new();
    for table in tables() {
        let wanted = Wanted::of(table.name);
        let (header, extra) = if archive.has_file(table.file) {
            write_table(&mut out, archive, table.name, table.file, true, wanted)?
        } else {
            // CCP removed the file. The table stays, empty, so lookups
            // against it still answer `None` rather than failing.
            (
                empty_table(&out, table.name, table.file, table.key.into(), true),
                Extracted::empty(wanted),
            )
        };
        tables_written.push(header);
        extracted.push(extra);
    }
    let mut refused_files = Vec::new();
    for file in archive.unmodeled_files().to_vec() {
        let Some(stem) = file.strip_suffix(".jsonl") else {
            continue;
        };
        let name = table_name(stem);
        // A store holds its tables by name, so a file cannot take the name
        // of a table this crate models: `get` for that model would read this
        // file's records as its own. CCP shipping such a file must not fail
        // the whole import, so the file is left out and the header says so.
        if tables_written.iter().any(|table| table.name == name) {
            refused_files.push(file);
            continue;
        }
        let (header, extra) = write_table(&mut out, archive, &name, &file, false, Wanted::NONE)?;
        tables_written.push(header);
        extracted.push(extra);
    }
    refused_files.sort();

    // The children indexes need nothing but what the records held.
    for (header, extra) in tables_written.iter_mut().zip(&mut extracted) {
        if let Some(children) = &mut extra.children {
            header.children = Some(write_children(&mut out, children)?);
        }
    }

    // The stations' names are not in the SDE, so they are built from the
    // records that are now in the file, through the models, as the SQLite
    // import builds them from its own tables.
    out.writer.flush().map_err(Error::Io)?;
    let (stations, station_names_error) = station_names(path, archive.build(), &tables_written)?;
    if let Some(index) = tables_written
        .iter()
        .position(|table| table.name == Named::Station.table())
        && let Some(names) = extracted[index].names.as_mut()
    {
        *names = stations;
    }

    for (header, extra) in tables_written.iter_mut().zip(&mut extracted) {
        if let Some(names) = &mut extra.names {
            let (index, blob) = write_names(&mut out, names, &header.file)?;
            header.names = Some(index);
            header.name_blob = Some(blob);
        }
    }

    let header = Header {
        layout: LAYOUT_VERSION,
        eve_sde: env!("CARGO_PKG_VERSION").to_owned(),
        build: archive.build().build_number,
        release_date: archive.build().release_date.clone(),
        imported_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
        station_names_error,
        refused_files,
        tables: tables_written,
    };
    let json = serde_json::to_vec(&header)
        .map_err(|e| Error::Invalid(format!("cannot write the store header: {e}")))?;
    let header_at = out.at;
    let header_len =
        u32::try_from(json.len()).map_err(|_| limit_error("store header", MAX_METADATA_BYTES))?;
    out.write(&json)?;
    out.write(&header_at.to_le_bytes())?;
    out.write(&header_len.to_le_bytes())?;
    out.write(&crc32(&json).to_le_bytes())?;
    out.write(&MAGIC)?;
    out.writer.flush().map_err(Error::Io)?;
    // The store becomes visible under its final name next, so make it
    // durable first.
    file.sync_all().map_err(Error::Io)
}

/// Copy `file` into the store as table `name`: its records, then its key
/// index.
fn write_table<R: Read + Seek>(
    out: &mut Out<'_>,
    archive: &mut Archive<R>,
    name: &str,
    file: &str,
    modeled: bool,
    wanted: Wanted,
) -> Result<(TableHeader, Extracted)> {
    let records_at = out.at;
    let mut keys = Vec::new();
    let mut extracted = Extracted::empty(wanted);
    // The first record decides whether the keys are numbers or text, as the
    // model's `KeyKind` says they should be. An empty file follows the model.
    let mut kind = None;
    let mut lines = archive.lines(file)?;
    while let Some(line) = lines.next_line() {
        let (number, text) = line?;
        let text = text.trim_end_matches(['\n', '\r']);
        let (key, fields) = parse_line(text, file, number, wanted)?;
        if *kind.get_or_insert(key.kind()) != key.kind() {
            return Err(Error::Invalid(format!(
                "`{file}` line {number} has a text `_key` where the file has numbers, \
                 or the other way round"
            )));
        }
        if let (Some(fields), ParsedKey::Integer(id)) = (&fields, &key)
            && let Ok(id) = u32::try_from(*id)
        {
            extracted.take(fields, id, wanted);
        }
        let len = u32::try_from(text.len())
            .map_err(|_| limit_error(format!("`{file}` line {number}"), MAX_RECORD_BYTES))?;
        keys.push((
            key,
            RecordAt {
                at: out.at,
                len,
                crc: crc32(text.as_bytes()),
            },
        ));
        out.write(text.as_bytes())?;
        out.write(b"\n")?;
    }
    let records = Span {
        at: records_at,
        len: out.at - records_at,
    };

    // Sorting by key puts any duplicates next to each other.
    keys.sort_by(|(a, _), (b, _)| a.cmp(b));
    if let Some(pair) = keys.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(Error::DuplicateKey {
            file: file.to_owned(),
            key: pair[0].0.to_string(),
        });
    }

    let kind = kind.unwrap_or_else(|| {
        KeyFormat::from(crate::tables::table_for_file(file).map_or(KeyKind::Integer, |t| t.key))
    });
    // Widening: `usize` is at most 64 bits on every target.
    let rows = keys.len() as u64;
    let (key_index, key_blob) = write_keys(out, &keys, kind, file)?;

    // A table whose keys are not numbers cannot be indexed by name or by
    // parent, because both hold `u32` keys. Saying so is better than an
    // index that is silently empty.
    if kind == KeyFormat::Text {
        extracted = Extracted::default();
    }

    Ok((
        TableHeader {
            name: name.to_owned(),
            file: file.to_owned(),
            modeled,
            rows,
            key: kind,
            records,
            keys: key_index,
            key_blob,
            names: None,
            name_blob: None,
            children: None,
        },
        extracted,
    ))
}

/// A table whose file the archive does not have: no records, no keys.
fn empty_table(
    out: &Out<'_>,
    name: &str,
    file: &str,
    key: KeyFormat,
    modeled: bool,
) -> TableHeader {
    let empty = Span { at: out.at, len: 0 };
    TableHeader {
        name: name.to_owned(),
        file: file.to_owned(),
        modeled,
        rows: 0,
        key,
        records: empty,
        keys: empty,
        key_blob: None,
        names: None,
        name_blob: None,
        children: None,
    }
}

impl Extracted {
    /// Empty rows for the indexes this table has, and none for those it has
    /// not. An indexed table with no rows is not the same as a table with no
    /// index, so the distinction starts here.
    fn empty(wanted: Wanted) -> Self {
        Self {
            names: wanted.named.map(|_| Vec::new()),
            children: wanted.parent.map(|_| Vec::new()),
        }
    }

    /// Take what one record gives the indexes.
    fn take(&mut self, fields: &Fields, id: u32, wanted: Wanted) {
        if let (Some(rows), Some(preferred), Some(name)) =
            (self.names.as_mut(), wanted.named, fields.name())
        {
            let rank = match preferred {
                Preferred::Published => u8::from(!fields.published.unwrap_or(false)),
                Preferred::NotDeleted => u8::from(fields.deleted.unwrap_or(false)),
                Preferred::None => 0,
            };
            rows.push(NameRow {
                key: name_key(name),
                rank,
                id,
            });
        }
        if let (Some(rows), Some(field)) = (self.children.as_mut(), wanted.parent)
            && let Some(parent) = fields.parent(field)
        {
            rows.push((parent, id));
        }
    }
}

/// Write a table's key index, and for text keys the blob its rows point
/// into: [`ENTRY_BYTES`] a row, in the order `keys` is already sorted in.
/// Returns the spans of the index and of the blob.
fn write_keys(
    out: &mut Out<'_>,
    keys: &[(ParsedKey, RecordAt)],
    kind: KeyFormat,
    file: &str,
) -> Result<(Span, Option<Span>)> {
    let at = out.at;
    let mut blob = String::new();
    let too_big = || limit_error(format!("`{file}` keys"), u64::from(u32::MAX));
    for (key, record) in keys {
        match key {
            ParsedKey::Integer(key) => out.write(&key.to_le_bytes())?,
            ParsedKey::Text(key) => {
                // Widening: `usize` is at most 64 bits on every target.
                let offset = u32::try_from(blob.len()).map_err(|_| too_big())?;
                let len = u32::try_from(key.len()).map_err(|_| too_big())?;
                blob.push_str(key);
                out.write(&offset.to_le_bytes())?;
                out.write(&len.to_le_bytes())?;
            }
        }
        out.write(&record.at.to_le_bytes())?;
        out.write(&record.len.to_le_bytes())?;
        out.write(&record.crc.to_le_bytes())?;
    }
    let index = Span {
        at,
        len: out.at - at,
    };
    let blob = match kind {
        KeyFormat::Integer => None,
        KeyFormat::Text => {
            let at = out.at;
            out.write(blob.as_bytes())?;
            Some(Span {
                at,
                len: out.at - at,
            })
        }
    };
    Ok((index, blob))
}

/// Write a table's children index: `(parent, child)` a row, sorted by both,
/// so the children of a parent are one run in ascending order.
fn write_children(out: &mut Out<'_>, rows: &mut Vec<(u32, u32)>) -> Result<Span> {
    rows.sort_unstable();
    rows.dedup();
    let at = out.at;
    for (parent, child) in rows.iter() {
        out.write(&parent.to_le_bytes())?;
        out.write(&child.to_le_bytes())?;
    }
    Ok(Span {
        at,
        len: out.at - at,
    })
}

/// Write a table's name index, then the blob its rows point into: 12 bytes a
/// row — the name's offset and length in the blob as two `u32`s, then the
/// record's key as a `u32` — sorted by name, then by rank, then by key, so
/// that the records sharing a name are one run in the order the lookups
/// want. Returns the spans of the index and of the blob.
fn write_names(out: &mut Out<'_>, rows: &mut [NameRow], file: &str) -> Result<(Span, Span)> {
    rows.sort_unstable();
    let at = out.at;
    let mut blob = String::new();
    let too_big = || limit_error(format!("`{file}` names"), u64::from(u32::MAX));
    for row in rows.iter() {
        // Widening: `usize` is at most 64 bits on every target.
        let offset = u32::try_from(blob.len()).map_err(|_| too_big())?;
        let len = u32::try_from(row.key.len()).map_err(|_| too_big())?;
        blob.push_str(&row.key);
        out.write(&offset.to_le_bytes())?;
        out.write(&len.to_le_bytes())?;
        out.write(&row.id.to_le_bytes())?;
    }
    let index = Span {
        at,
        len: out.at - at,
    };
    let blob_at = out.at;
    out.write(blob.as_bytes())?;
    Ok((
        index,
        Span {
            at: blob_at,
            len: out.at - blob_at,
        },
    ))
}

/// The names the game shows for NPC stations, which the SDE leaves out, and
/// why there are none if they could not be built.
///
/// The records and key indexes are already in the part file, so this reads
/// them back through the models, as [`naming`] does for any other source. A
/// record that does not fit its model leaves the stations unnamed and says
/// why, rather than failing the import.
fn station_names(
    path: &Path,
    build: &BuildInfo,
    tables: &[TableHeader],
) -> Result<(Vec<NameRow>, Option<String>)> {
    let store = Store::provisional(path, build, tables)?;
    let mut rows = Vec::new();
    // A store with no stations table at all would be this crate's own doing,
    // not CCP's, so that is an error and not a naming failure.
    for line in store.lines(Named::Station.table())? {
        // The part file is this import's own work, so a line it cannot read
        // is the import failing, not the models falling behind.
        let line = line?;
        let station = match serde_json::from_str::<NpcStation>(&line) {
            Ok(station) => station,
            Err(e) => {
                return Ok((
                    Vec::new(),
                    Some(format!("a station does not fit its model: {e}")),
                ));
            }
        };
        match naming::station_name(&store, &station) {
            Ok(name) => rows.push(NameRow {
                key: name_key(&name),
                rank: 0,
                id: station.id.0,
            }),
            // Reading the part file, or the disk, failed. An import must not
            // pass that off as an SDE that only happens to have no station
            // names.
            Err(e @ Error::Io(_)) => return Err(e),
            // Anything else is this build's own data: a record that does not
            // fit its model, or a table whose `_key` has changed shape.
            Err(e) => return Ok((Vec::new(), Some(error_chain(&e)))),
        }
    }
    Ok((rows, None))
}

/// The store file being written, and how many bytes are in it.
struct Out<'a> {
    writer: BufWriter<&'a File>,
    at: u64,
}

impl Out<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes).map_err(Error::Io)?;
        // Widening: `usize` is at most 64 bits on every target.
        self.at += bytes.len() as u64;
        Ok(())
    }
}

/// A record's `_key`, as the import reads it.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ParsedKey {
    Integer(i64),
    Text(String),
}

impl ParsedKey {
    fn kind(&self) -> KeyFormat {
        match self {
            Self::Integer(_) => KeyFormat::Integer,
            Self::Text(_) => KeyFormat::Text,
        }
    }
}

impl fmt::Display for ParsedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(key) => key.fmt(f),
            Self::Text(key) => write!(f, "{key:?}"),
        }
    }
}

/// CCP's `_key`, as JSON holds it.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ParsedKeyDe {
    Integer(i64),
    Text(String),
}

impl From<&ParsedKeyDe> for ParsedKey {
    fn from(key: &ParsedKeyDe) -> Self {
        match key {
            ParsedKeyDe::Integer(key) => Self::Integer(*key),
            ParsedKeyDe::Text(key) => Self::Text(key.clone()),
        }
    }
}

/// Read one line's `_key` without building the whole record, and the fields
/// the name and children indexes need if this table feeds either. One pass
/// over the line either way.
fn parse_line(
    text: &str,
    file: &str,
    number: usize,
    wanted: Wanted,
) -> Result<(ParsedKey, Option<Fields>)> {
    #[derive(Deserialize)]
    struct KeyOnly {
        #[serde(rename = "_key")]
        key: ParsedKeyDe,
    }

    // The line is JSON, so what it lacks is a usable `_key`.
    let failed = |e: serde_json::Error| {
        if serde_json::from_str::<serde_json::Value>(text).is_ok() {
            Error::Invalid(format!(
                "`{file}` line {number} has no integer or text `_key`"
            ))
        } else {
            Error::parse(file, number, e)
        }
    };
    if wanted.named.is_none() && wanted.parent.is_none() {
        return match serde_json::from_str::<KeyOnly>(text) {
            Ok(line) => Ok((ParsedKey::from(&line.key), None)),
            Err(e) => Err(failed(e)),
        };
    }
    match serde_json::from_str::<Fields>(text) {
        Ok(fields) => Ok((ParsedKey::from(&fields.key), Some(fields))),
        Err(e) => Err(failed(e)),
    }
}

/// A store's header, where it begins, and how long the whole file is.
struct Head {
    header: Header,
    /// Where the header begins, so where the records and indexes end.
    at: u64,
    size: u64,
}

/// Read and parse the header of a store file, through its footer.
fn read_head(file: &File, path: &Path) -> Result<Head> {
    let damaged =
        |what: String| Error::Invalid(format!("`{}` is not a store: {what}", path.display()));
    let size = file.metadata().path_context("read", path)?.len();
    if size < FOOTER_BYTES {
        return Err(damaged(format!("it is only {size} bytes")));
    }
    let mut footer = [0; 20];
    read_exact_at(file, &mut footer, size - FOOTER_BYTES).path_context("read", path)?;
    if footer[16..20] != MAGIC {
        return Err(damaged("it does not end in `ESDE`".to_owned()));
    }
    let at = le_u64(&footer[0..8]);
    let len = u64::from(le_u32(&footer[8..12]));
    let crc = le_u32(&footer[12..16]);
    if len > MAX_METADATA_BYTES {
        return Err(limit_error("store header", MAX_METADATA_BYTES));
    }
    // The header ends where the footer begins, so these three numbers say
    // how long a whole store of this header is. A file that has lost or
    // gained bytes fails here, rather than answering with another file's.
    if at.checked_add(len) != Some(size - FOOTER_BYTES) {
        return Err(damaged(format!(
            "it is {size} bytes, not the {} its header and footer describe",
            at.saturating_add(len).saturating_add(FOOTER_BYTES)
        )));
    }
    let len = usize::try_from(len).map_err(|_| limit_error("store header", MAX_METADATA_BYTES))?;
    let mut json = vec![0; len];
    read_exact_at(file, &mut json, at).path_context("read", path)?;
    // The header is the only part of the file a reader has to trust before
    // it can check anything else, and a change that keeps its length would
    // pass every other check.
    if crc32(&json) != crc {
        return Err(damaged("its header does not match its checksum".to_owned()));
    }
    let header = serde_json::from_slice(&json)
        .map_err(|e| damaged(format!("its header is not readable: {e}")))?;
    Ok(Head { header, at, size })
}

/// Check that every table the header names lies inside the file and has an
/// index of the right size, so a lookup cannot read another table's bytes.
/// `named` is how many distinct names the tables have.
fn check_tables(header: &Header, header_at: u64, named: usize, path: &Path) -> Result<()> {
    let damaged = |what: String| Error::Invalid(format!("`{}` is damaged: {what}", path.display()));
    if named != header.tables.len() {
        return Err(damaged("two of its tables have one name".to_owned()));
    }
    for table in &header.tables {
        let name = &table.name;
        let inside = |span: Span| {
            span.at
                .checked_add(span.len)
                .is_some_and(|end| end <= header_at)
        };
        if !inside(table.records)
            || !inside(table.keys)
            || !table.key_blob.is_none_or(inside)
            || !table.names.is_none_or(inside)
            || !table.name_blob.is_none_or(inside)
            || !table.children.is_none_or(inside)
        {
            return Err(damaged(format!("table `{name}` runs past its records")));
        }
        let width = |bytes: usize| u64::try_from(bytes).unwrap_or(u64::MAX);
        if table.keys.len != table.rows.saturating_mul(width(ENTRY_BYTES)) {
            return Err(damaged(format!(
                "the key index of table `{name}` is {} bytes for {} rows",
                table.keys.len, table.rows
            )));
        }
        // A record has at most one name and one parent, so neither index can
        // be longer than the table.
        let rows_of = |span: Span, bytes: usize| {
            span.len.is_multiple_of(width(bytes)) && span.len / width(bytes) <= table.rows
        };
        if !table
            .names
            .is_none_or(|span| rows_of(span, NAME_ENTRY_BYTES))
            || !table
                .children
                .is_none_or(|span| rows_of(span, CHILD_ENTRY_BYTES))
        {
            return Err(damaged(format!(
                "the name or children index of table `{name}` is of the wrong size"
            )));
        }
    }
    Ok(())
}

/// Read exactly `buf.len()` bytes from `at`, taking the file by reference so
/// that no seek position is shared.
#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], at: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, at)
}

/// Windows has no `read_exact_at`, so loop over `seek_read`. Each call takes
/// the offset to read from and does not depend on the file's position, so
/// threads reading at once do not interfere, although the position moves.
#[cfg(windows)]
fn read_exact_at(file: &File, buf: &mut [u8], at: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;

    let mut rest = buf;
    let mut at = at;
    while !rest.is_empty() {
        match file.seek_read(rest, at) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the store ends early",
                ));
            }
            Ok(read) => {
                rest = &mut rest[read..];
                // Widening: `usize` is at most 64 bits on every target.
                at += read as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn le_u32(bytes: &[u8]) -> u32 {
    let mut four = [0; 4];
    four.copy_from_slice(bytes);
    u32::from_le_bytes(four)
}

fn le_u64(bytes: &[u8]) -> u64 {
    let mut eight = [0; 8];
    eight.copy_from_slice(bytes);
    u64::from_le_bytes(eight)
}

fn le_i64(bytes: &[u8]) -> i64 {
    let mut eight = [0; 8];
    eight.copy_from_slice(bytes);
    i64::from_le_bytes(eight)
}

/// The file name of a store of `build` with this [`LAYOUT_VERSION`].
fn file_name(build: u32) -> String {
    format!("sde-{build}-v{LAYOUT_VERSION}.sde")
}

/// The build and layout in a store's file name.
fn parse_file_name(name: &str) -> Option<(u32, u32)> {
    let (build, layout) = name
        .strip_prefix("sde-")?
        .strip_suffix(".sde")?
        .split_once("-v")?;
    Some((build.parse().ok()?, layout.parse().ok()?))
}

/// The stores in `dir` with this [`LAYOUT_VERSION`], newest build first. A
/// missing folder holds none.
fn store_files(dir: &Path) -> Result<Vec<(u32, PathBuf)>> {
    let entries = match fs::read_dir(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        other => other.path_context("read", dir)?,
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.path_context("read", dir)?;
        let name = entry.file_name();
        if let Some((build, LAYOUT_VERSION)) = name.to_str().and_then(parse_file_name) {
            found.push((build, entry.path()));
        }
    }
    found.sort_unstable_by(|(a, _), (b, _)| b.cmp(a));
    Ok(found)
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
    name.starts_with(".sde-") && name.contains(".sde.part-") && parts::is_stale(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_file_names_hold_the_build_and_layout() {
        assert_eq!(
            parse_file_name(&file_name(3_579_973)),
            Some((3_579_973, LAYOUT_VERSION))
        );
        assert_eq!(parse_file_name("sde-12-v3.sde"), Some((12, 3)));
        assert_eq!(parse_file_name("sde-12.sde"), None);
        assert_eq!(parse_file_name(".sde-12-v1.sde.part-abc"), None);
        assert_eq!(parse_file_name("sde.zip"), None);
    }

    /// `Fields::parent` has to know every field a relation names, or that
    /// relation's index would be silently empty.
    #[test]
    fn every_relations_field_is_read() {
        for relation in Relation::ALL {
            let (_, field) = relation.child();
            let line = format!(r#"{{"_key":1,"{field}":7}}"#);
            let fields: Fields = serde_json::from_str(&line).unwrap();
            assert_eq!(fields.parent(field), Some(7), "{field}");
        }
    }

    /// The fields the indexes use must not fail an import when CCP changes
    /// their shape, and a name is a string or an object of text, as the
    /// SQLite store reads it too.
    #[test]
    fn the_fields_the_indexes_use_are_read_leniently() {
        let fields = |line: &str| serde_json::from_str::<Fields>(line).unwrap();

        let plain = fields(r#"{"_key":1,"name":"Rifter","published":true}"#);
        assert_eq!(plain.name(), Some("Rifter"));
        assert_eq!(plain.published, Some(true));

        let localized = fields(r#"{"_key":1,"name":{"en":"Rifter","de":"Rifter"}}"#);
        assert_eq!(localized.name(), Some("Rifter"));

        // Shapes this crate does not expect are ignored, not errors.
        let drifted = fields(
            r#"{"_key":1,"name":{"de":"Rifter"},"published":"yes","deleted":7,"groupID":"25"}"#,
        );
        assert_eq!(drifted.name(), None);
        assert_eq!(drifted.published, None);
        assert_eq!(drifted.deleted, None);
        assert_eq!(drifted.parent("groupID"), None);

        // A missing `_key` is still an error, because the store is keyed by
        // it.
        assert!(serde_json::from_str::<Fields>(r#"{"name":"Rifter"}"#).is_err());
    }

    /// One line's key, read the way a table with no name or children index
    /// reads it.
    fn key_of(text: &str, file: &str, number: usize) -> Result<ParsedKey> {
        parse_line(text, file, number, Wanted::NONE).map(|(key, _)| key)
    }

    #[test]
    fn keys_are_read_as_numbers_or_text() {
        assert_eq!(
            key_of(r#"{"_key":587,"name":"x"}"#, "types.jsonl", 1).unwrap(),
            ParsedKey::Integer(587)
        );
        assert_eq!(
            key_of(r#"{"_key":"a-b-c"}"#, "titles.jsonl", 1).unwrap(),
            ParsedKey::Text("a-b-c".to_owned())
        );
        assert!(matches!(
            key_of(r#"{"name":"x"}"#, "types.jsonl", 7),
            Err(Error::Invalid(message)) if message.contains("line 7 has no integer or text `_key`")
        ));
        assert!(matches!(
            key_of(r#"{"_key":true}"#, "types.jsonl", 1),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            key_of("not json", "types.jsonl", 3),
            Err(Error::Parse { line: 3, .. })
        ));
    }
}
