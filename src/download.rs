//! Check for new SDE builds and download them. Needs the `download` feature.
//!
//! These calls block the calling thread. In async code run them with
//! `spawn_blocking` or similar.
//!
//! A download goes to a temporary file next to the destination. Only when the
//! ZIP is intact and has the build you asked for does it replace the
//! destination, so a failed or cancelled download never damages an existing
//! SDE file.
//!
//! Writers coordinate through a persistent `.<filename>.lock` file beside
//! the destination. Do not delete that file while any writer can use it.
//!
//! If a process dies during a download, its `.<filename>.part-…` file stays
//! behind. The next download to the same destination deletes it once it has
//! not changed for an hour.

use crate::archive::{Archive, BuildInfo, limit_error, read_metadata};
use crate::error::{Error, PathContext, Result, io_context};
use crate::parts;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::NamedTempFile;
use ureq::http::header::{ETAG, IF_NONE_MATCH};

/// Where CCP publishes SDE builds.
pub const BASE_URL: &str = "https://developers.eveonline.com/static-data/tranquility";

/// Maximum size of a download: 512 MiB. The SDE ZIP is about 100 MB.
pub const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the server may take to start answering.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
/// How long checking for the latest build may take in all.
const CHECK_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a whole download may take.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Result of asking CCP for the latest build.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Latest {
    /// The `ETag` you passed still matches, so nothing changed.
    Unchanged,
    /// The latest build, and the `ETag` of this answer if the server sent one.
    /// Pass `etag` to the next [`Client::latest`] call so that CCP can answer
    /// [`Latest::Unchanged`] without sending the build again.
    Available {
        /// The latest build.
        build: BuildInfo,
        /// The `ETag` of this answer, if the server sent one.
        etag: Option<String>,
    },
}

/// Result of [`Client::update`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Update {
    /// The file already holds this build, which is the latest or newer.
    UpToDate(u32),
    /// The file was missing or older, and now holds a newer build.
    Updated {
        /// The build the file held before, or `None` if it was missing.
        from: Option<u32>,
        /// The build the file holds now.
        to: u32,
    },
}

/// The build in a local SDE ZIP, or `None` if there is no such file.
///
/// # Errors
///
/// Fails if the file exists but cannot be opened or is not an SDE ZIP.
pub fn local_build(path: impl AsRef<Path>) -> Result<Option<BuildInfo>> {
    fn local_build(path: &Path) -> Result<Option<BuildInfo>> {
        let file = match File::open(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            other => other.path_context("open", path)?,
        };
        let archive = Archive::from_reader(BufReader::new(file))?;
        Ok(Some(archive.build().clone()))
    }
    local_build(path.as_ref())
}

/// Talks to CCP's SDE server. Clones share one connection pool.
#[derive(Debug, Clone)]
pub struct Client {
    agent: ureq::Agent,
    base: String,
}

impl Client {
    /// A client that sends `user_agent` with every request.
    #[must_use]
    pub fn new(user_agent: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .user_agent(user_agent)
            .http_status_as_error(false)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_TIMEOUT))
            .timeout_global(Some(DOWNLOAD_TIMEOUT))
            .build()
            .into();
        Self {
            agent,
            base: BASE_URL.to_owned(),
        }
    }

    /// Use another server with the same layout instead of CCP's, such as a
    /// mirror or a test server.
    #[must_use]
    pub fn with_base_url(mut self, base: impl Into<String>) -> Self {
        base.into().trim_end_matches('/').clone_into(&mut self.base);
        self
    }

    /// Ask for the latest build. With the `etag` of an earlier answer, the
    /// result is [`Latest::Unchanged`] if nothing changed. CCP caches this
    /// answer for five minutes, so polling more often is pointless.
    ///
    /// # Errors
    ///
    /// Fails if the request fails, the server answers with another status, or
    /// the answer is not a build record or is too big.
    pub fn latest(&self, etag: Option<&str>) -> Result<Latest> {
        let url = format!("{}/latest.jsonl", self.base);
        let mut request = self
            .agent
            .get(&url)
            .config()
            .timeout_global(Some(CHECK_TIMEOUT))
            .build();
        if let Some(etag) = etag {
            request = request.header(IF_NONE_MATCH, etag);
        }
        let mut response = request.call().map_err(Error::http)?;
        match response.status().as_u16() {
            // "Not modified" only makes sense as an answer to an ETag.
            304 if etag.is_some() => Ok(Latest::Unchanged),
            200 => {
                let etag = response
                    .headers()
                    .get(ETAG)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                let body = read_metadata(response.body_mut().as_reader(), |e| {
                    Error::Io(io_context(e, format!("cannot read `{url}`")))
                })?;
                Ok(Latest::Available {
                    build: BuildInfo::parse(&body)?,
                    etag,
                })
            }
            status => Err(Error::HttpStatus { url, status }),
        }
    }

    /// Download build `build` to `dest`. The new file replaces `dest` only
    /// after it is complete, undamaged and the right build. The folder of
    /// `dest` must exist. Unlike [`Client::update`], this replaces `dest` even
    /// if it holds a newer build.
    ///
    /// `progress(bytes_so_far, total)` runs after each chunk. `total` is `None`
    /// if the server does not send the size. Return `ControlFlow::Break(())`
    /// to cancel with [`Error::Cancelled`].
    ///
    /// # Errors
    ///
    /// Fails if `dest` has no file name, the request fails, the download is
    /// too big or damaged, it holds another build, `progress` cancels it, or
    /// `dest` cannot be replaced. On failure `dest` stays as it was.
    pub fn download(
        &self,
        build: u32,
        dest: impl AsRef<Path>,
        mut progress: impl FnMut(u64, Option<u64>) -> ControlFlow<()>,
    ) -> Result<BuildInfo> {
        let dest = dest.as_ref();
        let lock_file = lock_path(dest)?;
        let (temp, info) = self.fetch(build, dest, &mut progress)?;
        let _guard = lock(&lock_file)?;
        persist(temp, dest)?;
        Ok(info)
    }

    /// Download and check a build without holding the lock, so a slow
    /// download does not block other writers.
    fn fetch(
        &self,
        build: u32,
        dest: &Path,
        progress: &mut dyn FnMut(u64, Option<u64>) -> ControlFlow<()>,
    ) -> Result<(NamedTempFile, BuildInfo)> {
        remove_stale_parts(dest);
        let url = format!("{}/eve-online-static-data-{build}-jsonl.zip", self.base);
        let response = self.agent.get(&url).call().map_err(Error::http)?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(Error::HttpStatus { url, status });
        }
        let total = response.body().content_length();
        if total.is_some_and(|total| total > MAX_DOWNLOAD_BYTES) {
            return Err(limit_error("download", MAX_DOWNLOAD_BYTES));
        }

        let mut temp = temp_file_next_to(dest)?;
        let mut body = response.into_body().into_reader();
        let mut buf = vec![0; 64 * 1024];
        let mut written = 0;
        loop {
            let n = match body.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(Error::Io(io_context(e, format!("cannot read `{url}`")))),
            };
            // Widening: `usize` is at most 64 bits on every target.
            written += n as u64;
            if written > MAX_DOWNLOAD_BYTES {
                return Err(limit_error("download", MAX_DOWNLOAD_BYTES));
            }
            let path = temp.path().to_owned();
            temp.as_file_mut()
                .write_all(&buf[..n])
                .path_context("write", &path)?;
            if progress(written, total).is_break() {
                return Err(Error::Cancelled);
            }
        }
        temp.as_file()
            .sync_all()
            .path_context("write", temp.path())?;

        let info = verify(temp.path())?;
        if info.build_number != build {
            return Err(Error::BuildMismatch {
                expected: build,
                got: info.build_number,
            });
        }
        Ok((temp, info))
    }

    /// Make the file at `path` the latest build, downloading it only if `path`
    /// is missing or older. Concurrent updates never replace a newer build
    /// with an older one.
    ///
    /// # Errors
    ///
    /// Fails for the reasons listed under [`Client::latest`] and
    /// [`Client::download`], and if `path` exists but is not a readable SDE
    /// ZIP, because `update` never replaces a file it cannot identify. On
    /// failure `path` stays as it was.
    pub fn update(&self, path: impl AsRef<Path>) -> Result<Update> {
        self.update_with_progress(path, |_, _| ControlFlow::Continue(()))
    }

    /// Like [`Client::update`], reporting the download to `progress` as
    /// [`Client::download`] does.
    ///
    /// # Errors
    ///
    /// Fails for the reasons listed under [`Client::update`], and with
    /// [`Error::Cancelled`] if `progress` cancels the download.
    pub fn update_with_progress(
        &self,
        path: impl AsRef<Path>,
        mut progress: impl FnMut(u64, Option<u64>) -> ControlFlow<()>,
    ) -> Result<Update> {
        self.update_path(path.as_ref(), None, &mut progress)
    }

    /// The latest build's number.
    pub(crate) fn latest_build(&self) -> Result<u32> {
        match self.latest(None)? {
            Latest::Available { build, .. } => Ok(build.build_number),
            Latest::Unchanged => {
                unreachable!("`latest` answers `Unchanged` only when given an ETag")
            }
        }
    }

    /// Make `path` hold `build`, or the latest build if `build` is `None`,
    /// unless it holds that build or a newer one.
    pub(crate) fn update_path(
        &self,
        path: &Path,
        build: Option<u32>,
        progress: &mut dyn FnMut(u64, Option<u64>) -> ControlFlow<()>,
    ) -> Result<Update> {
        let lock_file = lock_path(path)?;
        let local = local_build(path)?.map(|b| b.build_number);
        let build = match build {
            Some(build) => build,
            None => self.latest_build()?,
        };
        if let Some(have) = local
            && have >= build
        {
            return Ok(Update::UpToDate(have));
        }
        let (temp, _) = self.fetch(build, path, progress)?;
        let _guard = lock(&lock_file)?;
        // Another writer may have updated the file in the meantime.
        let current = local_build(path)?.map(|b| b.build_number);
        if let Some(have) = current
            && have >= build
        {
            return Ok(Update::UpToDate(have));
        }
        persist(temp, path)?;
        Ok(Update::Updated {
            from: current,
            to: build,
        })
    }
}

/// `.<filename>.lock` beside `dest`. Called first, so a destination without
/// a file name fails before anything is downloaded.
fn lock_path(dest: &Path) -> Result<PathBuf> {
    let Some(filename) = dest.file_name() else {
        let message = format!("destination `{}` needs a file name", dest.display());
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            message,
        )));
    };
    let mut name = OsString::from(".");
    name.push(filename);
    name.push(".lock");
    Ok(dest.with_file_name(name))
}

/// Lock `path`, waiting while another writer holds it. The file is never
/// deleted, so every process locks the same file. Dropping the handle
/// releases the lock, even during a panic.
fn lock(path: &Path) -> Result<File> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .path_context("open", path)?;
    file.lock().path_context("lock", path)?;
    Ok(file)
}

/// Check that the ZIP opens, has a build number, and that every entry
/// decompresses with a matching checksum.
fn verify(path: &Path) -> Result<BuildInfo> {
    let file = File::open(path).path_context("open", path)?;
    let mut archive = Archive::from_reader(BufReader::new(file))?;
    archive.verify()?;
    Ok(archive.build().clone())
}

/// A new file `.<filename>.part-<random>` beside `dest`. It deletes itself
/// unless persisted, and is never a symlink or an existing file.
fn temp_file_next_to(dest: &Path) -> Result<NamedTempFile> {
    let dir = folder_of(dest);
    tempfile::Builder::new()
        .prefix(&part_prefix(dest))
        .tempfile_in(dir)
        .map_err(|e| {
            let context = format!("cannot create a temporary file in `{}`", dir.display());
            Error::Io(io_context(e, context))
        })
}

/// The folder of `dest`, which is `.` for a bare file name.
fn folder_of(dest: &Path) -> &Path {
    match dest.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

/// `.<filename>.part-`, the start of the name of a download's temporary file.
fn part_prefix(dest: &Path) -> String {
    let name = dest
        .file_name()
        .map_or_else(|| "sde".into(), |n| n.to_string_lossy());
    format!(".{name}.part-")
}

/// Delete the temporary files that downloads which died left beside `dest`.
/// Best effort: a file that cannot be deleted, such as one that another
/// process has open on Windows, stays for the next download.
fn remove_stale_parts(dest: &Path) {
    let prefix = part_prefix(dest);
    let Ok(entries) = fs::read_dir(folder_of(dest)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let is_part = name.to_str().is_some_and(|name| name.starts_with(&prefix));
        if is_part && parts::is_stale(&entry) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Move the finished download to `dest`, replacing what is there.
fn persist(temp: NamedTempFile, dest: &Path) -> Result<()> {
    temp.persist(dest).map(drop).map_err(|e| {
        let context = format!("cannot move the download to `{}`", dest.display());
        Error::Io(io_context(e.error, context))
    })
}
