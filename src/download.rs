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

use crate::archive::{Archive, BuildInfo, limit_error, read_metadata};
use crate::error::{Error, PathContext, Result, io_context};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use ureq::http::header::{ETAG, IF_NONE_MATCH};

/// Where CCP publishes SDE builds.
pub const BASE_URL: &str = "https://developers.eveonline.com/static-data/tranquility";

/// Refuse downloads bigger than this. The SDE is about 100 MiB.
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
    /// The ETag you passed still matches; nothing changed.
    Unchanged,
    /// The latest build. Pass `etag` to the next [`Client::latest`] call so
    /// CCP can answer "unchanged" without sending anything.
    Available {
        build: BuildInfo,
        etag: Option<String>,
    },
}

/// Result of [`Client::update`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Update {
    /// The local file is already the latest build (or newer).
    UpToDate(u32),
    /// The file was missing or old and now holds build `to`.
    Updated { from: Option<u32>, to: u32 },
}

/// The build in a local SDE ZIP, or `None` if there is no such file. A file
/// that exists but is not an SDE ZIP is an error.
pub fn local_build(path: impl AsRef<Path>) -> Result<Option<BuildInfo>> {
    let path = path.as_ref();
    match File::open(path).path_context("open", path) {
        Ok(file) => Ok(Some(Archive::new(BufReader::new(file))?.build().clone())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Talks to CCP's SDE server. Clones share one connection pool.
#[derive(Debug, Clone)]
pub struct Client {
    agent: ureq::Agent,
    base: String,
}

impl Client {
    /// `user_agent` identifies your app to CCP, e.g. `my-bot/1.2 (me@example.com)`.
    pub fn new(user_agent: &str) -> Client {
        let agent = ureq::Agent::config_builder()
            .user_agent(user_agent)
            .http_status_as_error(false)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_TIMEOUT))
            .timeout_global(Some(DOWNLOAD_TIMEOUT))
            .build()
            .into();
        Client {
            agent,
            base: BASE_URL.to_owned(),
        }
    }

    /// Use another server instead of CCP's. For tests.
    pub fn with_base_url(mut self, base: impl Into<String>) -> Client {
        self.base = base.into().trim_end_matches('/').to_owned();
        self
    }

    /// Ask for the latest build. CCP caches this answer for five minutes, so
    /// polling more often is pointless.
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
                    io_context(e, format!("cannot read `{url}`")).into()
                })?;
                Ok(Latest::Available {
                    build: BuildInfo::parse(&body)?,
                    etag,
                })
            }
            status => Err(Error::HttpStatus { url, status }),
        }
    }

    /// Download one build to `dest`, replacing it only once the new file checks
    /// out. The folder of `dest` must exist.
    /// This explicit request can replace a newer build. Use [`Client::update`]
    /// to preserve newer builds installed by another writer.
    ///
    /// `progress(bytes_so_far, total)` runs after each chunk; return
    /// `ControlFlow::Break(())` to cancel with [`Error::Cancelled`].
    pub fn download(
        &self,
        build: u32,
        dest: impl AsRef<Path>,
        progress: impl FnMut(u64, Option<u64>) -> ControlFlow<()>,
    ) -> Result<BuildInfo> {
        let dest = dest.as_ref();
        let (temp, info) = self.fetch(build, dest, progress)?;
        let _lock = lock_destination(dest)?;
        temp.persist(dest)?;
        Ok(info)
    }

    /// Download and validate before taking the destination lock.
    fn fetch(
        &self,
        build: u32,
        dest: &Path,
        mut progress: impl FnMut(u64, Option<u64>) -> ControlFlow<()>,
    ) -> Result<(TempFile, BuildInfo)> {
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

        let (temp, mut file) = TempFile::next_to(dest)?;
        let mut body = response.into_body().into_reader();
        let mut buf = vec![0; 64 * 1024];
        let mut written = 0;
        loop {
            let n = match body.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_context(e, format!("cannot read `{url}`")).into()),
            };
            written += n as u64;
            if written > MAX_DOWNLOAD_BYTES {
                return Err(limit_error("download", MAX_DOWNLOAD_BYTES));
            }
            file.write_all(&buf[..n])
                .path_context("write", &temp.path)?;
            if progress(written, total).is_break() {
                return Err(Error::Cancelled);
            }
        }
        file.sync_all().path_context("write", &temp.path)?;
        drop(file);

        let info = verify(&temp.path)?;
        if info.build_number != build {
            return Err(Error::BuildMismatch {
                expected: build,
                got: info.build_number,
            });
        }
        Ok((temp, info))
    }

    /// Make the file at `path` the latest build. A missing file counts as
    /// "no build yet".
    /// Concurrent updates never replace a newer build with an older one.
    pub fn update(&self, path: impl AsRef<Path>) -> Result<Update> {
        let path = path.as_ref();
        let local = local_build(path)?.map(|b| b.build_number);
        let Latest::Available { build, .. } = self.latest(None)? else {
            unreachable!("`latest` answers `Unchanged` only when given an ETag");
        };
        match local {
            Some(have) if have >= build.build_number => Ok(Update::UpToDate(have)),
            _ => {
                let (temp, _) =
                    self.fetch(build.build_number, path, |_, _| ControlFlow::Continue(()))?;
                let _lock = lock_destination(path)?;
                let current = local_build(path)?.map(|b| b.build_number);
                if let Some(have) = current
                    && have >= build.build_number
                {
                    return Ok(Update::UpToDate(have));
                }
                temp.persist(path)?;
                Ok(Update::Updated {
                    from: current,
                    to: build.build_number,
                })
            }
        }
    }
}

/// Keep this file in place so every process locks the same file object.
/// Closing the handle releases the lock, including during unwinding.
fn lock_destination(dest: &Path) -> io::Result<File> {
    let filename = dest.file_name().ok_or_else(|| {
        let message = format!("destination `{}` needs a file name", dest.display());
        io::Error::new(io::ErrorKind::InvalidInput, message)
    })?;
    let mut name = OsString::from(".");
    name.push(filename);
    name.push(".lock");
    let path = dest.with_file_name(name);
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .path_context("open", &path)?;
    file.lock().path_context("lock", &path)?;
    Ok(file)
}

/// Check the ZIP opens, has a build number, and every entry decompresses
/// with a matching checksum.
fn verify(path: &Path) -> Result<BuildInfo> {
    let file = File::open(path).path_context("open", path)?;
    let mut archive = Archive::new(BufReader::new(file))?;
    archive.verify()?;
    Ok(archive.build().clone())
}

/// A file next to the destination that deletes itself unless persisted.
struct TempFile {
    path: PathBuf,
    keep: bool,
}

impl TempFile {
    /// Create a file without replacing an existing file or following a symlink.
    fn next_to(dest: &Path) -> io::Result<(TempFile, File)> {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let name = dest
            .file_name()
            .map_or_else(|| "sde".into(), |n| n.to_string_lossy());
        loop {
            let id = format!(
                "{}-{}",
                std::process::id(),
                COUNT.fetch_add(1, Ordering::Relaxed)
            );
            let path = dest.with_file_name(format!(".{name}.part-{id}"));
            match File::create_new(&path).path_context("create", &path) {
                Ok(file) => return Ok((TempFile { path, keep: false }, file)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }

    /// Move the file to `dest`, replacing what is there.
    fn persist(mut self, dest: &Path) -> io::Result<()> {
        fs::rename(&self.path, dest).map_err(|e| {
            io_context(
                e,
                format!("cannot move the download to `{}`", dest.display()),
            )
        })?;
        self.keep = true;
        Ok(())
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}
