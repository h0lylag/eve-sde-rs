//! What an import or an update of a local store does, for both kinds of
//! store: the packed one in [`store`](crate::store) and the SQLite one in
//! [`sqlite`](crate::sqlite).
//!
//! Each kind has its own layout, its own file names and its own version, but
//! a folder of builds behaves the same either way, and so does bringing one
//! up to date: check with CCP, download what the folder lacks, import it.
//! The types a caller sees are therefore the same for both, and
//! [`update_dir`] drives both.

#[cfg(feature = "download")]
use crate::archive::Archive;
#[cfg(feature = "download")]
use crate::error::{Error, PathContext, Result};
#[cfg(feature = "download")]
use std::fs;
#[cfg(feature = "download")]
use std::ops::ControlFlow;
#[cfg(feature = "download")]
use std::path::Path;

/// What an import or an update of a store did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImportOutcome {
    /// The build was imported.
    Imported,
    /// The folder already holds this build.
    UpToDate,
    /// The folder holds a newer build, so this one was not imported.
    OlderRefused,
}

/// The file an update keeps the SDE ZIP in, inside the store's folder.
///
/// The ZIP is kept, so a store of a new layout can be imported from it
/// without downloading again. Both kinds of store use the same file, so a
/// folder that holds both downloads once.
#[cfg(feature = "download")]
#[cfg_attr(docsrs, doc(cfg(feature = "download")))]
pub const ZIP_FILE: &str = "sde.zip";

/// A step of an update, for a progress bar.
#[cfg(feature = "download")]
#[cfg_attr(docsrs, doc(cfg(feature = "download")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Step {
    /// Asking CCP for the latest build.
    Checking,
    /// Downloading a build into [`ZIP_FILE`].
    Downloading {
        /// The build being downloaded.
        build: u32,
        /// The bytes received so far.
        received: u64,
        /// The size of the download, if the server sent it.
        total: Option<u64>,
    },
    /// Importing a build into a new store. It cannot be cancelled.
    Importing {
        /// The build being imported.
        build: u32,
    },
}

/// One kind of local store, as an update drives it. Each kind answers for
/// its own folder and writes its own files; everything else is shared.
#[cfg(feature = "download")]
pub(crate) trait Local {
    /// What an import of `build` into `dir` would return without importing.
    fn already_holds(&self, dir: &Path, build: u32) -> Result<Option<ImportOutcome>>;

    /// Import `archive` into `dir`.
    fn import(&self, archive: &mut Archive, dir: &Path) -> Result<ImportOutcome>;
}

/// Bring the store in `dir` up to the latest build, reporting each step.
///
/// Nothing is downloaded if `dir` already holds a store of the latest build
/// or a newer one, or if [`ZIP_FILE`] already holds the latest build. A
/// [`ZIP_FILE`] that is not a readable SDE ZIP is downloaded again.
#[cfg(feature = "download")]
pub(crate) fn update_dir(
    client: &crate::download::Client,
    dir: &Path,
    progress: &mut dyn FnMut(Step) -> ControlFlow<()>,
    local: &dyn Local,
) -> Result<ImportOutcome> {
    fs::create_dir_all(dir).path_context("create", dir)?;
    if progress(Step::Checking).is_break() {
        return Err(Error::Cancelled);
    }
    let build = client.latest_build()?;
    if let Some(outcome) = local.already_holds(dir, build)? {
        return Ok(outcome);
    }
    let zip = dir.join(ZIP_FILE);
    let mut downloading = |received: u64, total: Option<u64>| {
        progress(Step::Downloading {
            build,
            received,
            total,
        })
    };
    if crate::download::local_build(&zip).is_ok() {
        client.update_path(&zip, Some(build), &mut downloading)?;
    } else {
        // The ZIP is this module's own file, so one that cannot be read is
        // replaced rather than kept.
        client.download(build, &zip, &mut downloading)?;
    }
    let mut archive = Archive::open(&zip)?;
    let build = archive.build().build_number;
    if progress(Step::Importing { build }).is_break() {
        return Err(Error::Cancelled);
    }
    local.import(&mut archive, dir)
}
