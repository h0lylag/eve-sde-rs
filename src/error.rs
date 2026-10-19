use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::path::Path;
#[cfg(any(feature = "sqlite", feature = "store"))]
use std::path::PathBuf;

/// Everything that can go wrong while reading, downloading or storing the
/// SDE.
///
/// Errors from the ZIP, JSON, HTTP and SQLite libraries are boxed, so
/// upgrading those libraries does not change this type. Downcast the box to
/// reach them.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file operation failed, or a download broke off. The message names
    /// the file or URL.
    Io(io::Error),
    /// The data is not a ZIP archive, or the archive's structure is damaged.
    /// Damage inside one file is [`Error::Read`].
    Zip(Box<dyn StdError + Send + Sync>),
    /// The archive has no file with this name. Did CCP change the format?
    MissingFile(String),
    /// A file in the archive could not be read.
    Read {
        /// The file inside the archive.
        file: String,
        /// Why, e.g. `Invalid checksum` when the file is damaged.
        source: io::Error,
    },
    /// A line of a file in the archive failed to parse.
    Parse {
        /// The file inside the archive.
        file: String,
        /// The line, counting from 1.
        line: usize,
        /// Why. It downcasts to `serde_json::Error`.
        source: Box<dyn StdError + Send + Sync>,
    },
    /// Two records in one file of the archive share a key.
    DuplicateKey {
        /// The file inside the archive.
        file: String,
        /// The repeated key.
        key: String,
    },
    /// The archive, a SQLite store or a server response is not in the
    /// expected format.
    Invalid(String),
    /// Build metadata, a record, the decompressed archive or a download is
    /// bigger than its limit.
    LimitExceeded {
        /// What was too big, e.g. `download`.
        resource: String,
        /// The limit in bytes.
        limit: u64,
    },
    /// The request failed before a response arrived, e.g. because of DNS, TLS
    /// or a timeout.
    #[cfg(feature = "download")]
    #[cfg_attr(docsrs, doc(cfg(feature = "download")))]
    Http(Box<dyn StdError + Send + Sync>),
    /// The server answered with a status other than the one expected.
    #[cfg(feature = "download")]
    #[cfg_attr(docsrs, doc(cfg(feature = "download")))]
    HttpStatus {
        /// The URL that was requested.
        url: String,
        /// The HTTP status code.
        status: u16,
    },
    /// A downloaded archive holds another build than the one requested.
    #[cfg(feature = "download")]
    #[cfg_attr(docsrs, doc(cfg(feature = "download")))]
    BuildMismatch {
        /// The build that was requested.
        expected: u32,
        /// The build the archive holds.
        got: u32,
    },
    /// The progress callback asked to stop.
    #[cfg(feature = "download")]
    #[cfg_attr(docsrs, doc(cfg(feature = "download")))]
    Cancelled,
    /// The SQLite store could not be read or written. It downcasts to
    /// `rusqlite::Error`.
    #[cfg(feature = "sqlite")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
    Sqlite(Box<dyn StdError + Send + Sync>),
    /// The folder holds no store of this version's layout. Fill it with an
    /// import.
    #[cfg(any(feature = "sqlite", feature = "store"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "sqlite", feature = "store"))))]
    NoStore {
        /// The folder.
        dir: PathBuf,
    },
    /// A record in a store does not fit its model. Did CCP change the
    /// format?
    #[cfg(any(feature = "sqlite", feature = "store"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "sqlite", feature = "store"))))]
    Decode {
        /// The table.
        table: String,
        /// The record's key.
        key: String,
        /// Why. It downcasts to `serde_json::Error`.
        source: Box<dyn StdError + Send + Sync>,
    },
}

/// `Result` with this crate's [`Error`] as the default error type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn zip(e: zip::result::ZipError) -> Self {
        Self::Zip(Box::new(e))
    }

    pub(crate) fn read(file: &str, source: io::Error) -> Self {
        Self::Read {
            file: file.to_owned(),
            source,
        }
    }

    pub(crate) fn parse(file: &str, line: usize, source: serde_json::Error) -> Self {
        Self::Parse {
            file: file.to_owned(),
            line,
            source: Box::new(source),
        }
    }

    #[cfg(feature = "download")]
    pub(crate) fn http(e: ureq::Error) -> Self {
        Self::Http(Box::new(e))
    }

    #[cfg(feature = "sqlite")]
    pub(crate) fn sqlite(e: rusqlite::Error) -> Self {
        Self::Sqlite(Box::new(e))
    }
}

// A plain wrapper shows the wrapped error's message and passes on its source.
// A variant that adds context shows only the context and returns the cause
// from `source`, so error reporters never print a message twice.
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Zip(e) => e.fmt(f),
            Self::MissingFile(name) => write!(f, "archive has no file `{name}`"),
            Self::Read { file, .. } => write!(f, "cannot read `{file}`"),
            Self::Parse { file, line, .. } => write!(f, "cannot parse `{file}` line {line}"),
            Self::DuplicateKey { file, key } => {
                write!(f, "`{file}` has more than one record with key {key}")
            }
            Self::Invalid(msg) => f.write_str(msg),
            Self::LimitExceeded { resource, limit } => {
                write!(f, "{resource} exceeds the limit of {limit} bytes")
            }
            #[cfg(feature = "download")]
            Self::Http(e) => e.fmt(f),
            #[cfg(feature = "download")]
            Self::HttpStatus { url, status } => write!(f, "{url} returned HTTP {status}"),
            #[cfg(feature = "download")]
            Self::BuildMismatch { expected, got } => {
                write!(
                    f,
                    "expected SDE build {expected} but the archive is build {got}"
                )
            }
            #[cfg(feature = "download")]
            Self::Cancelled => f.write_str("download cancelled"),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(e) => e.fmt(f),
            #[cfg(any(feature = "sqlite", feature = "store"))]
            Self::NoStore { dir } => write!(f, "no SDE store in `{}`", dir.display()),
            #[cfg(any(feature = "sqlite", feature = "store"))]
            Self::Decode { table, key, .. } => {
                write!(f, "cannot parse record {key} of table `{table}`")
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(e) => e.source(),
            Self::Zip(e) => e.source(),
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source.as_ref()),
            #[cfg(feature = "download")]
            Self::Http(e) => e.source(),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(e) => e.source(),
            #[cfg(any(feature = "sqlite", feature = "store"))]
            Self::Decode { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// A message naming an error and every cause under it, for a store that has
/// to record why it could not name the stations.
#[cfg(any(feature = "sqlite", feature = "store"))]
pub(crate) fn error_chain(error: &Error) -> String {
    let mut text = error.to_string();
    let mut source = StdError::source(error);
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// Wrap an I/O error in a message that says what failed, e.g. "cannot open
/// `sde.zip`". The result keeps the error's kind, and its source is the
/// original error.
pub(crate) fn io_context(source: io::Error, context: String) -> io::Error {
    io::Error::new(source.kind(), IoContext { context, source })
}

/// Names the file in a failed I/O result's error, using [`io_context`], and
/// turns it into [`Error::Io`].
pub(crate) trait PathContext<T> {
    /// `action` completes "cannot …", e.g. `open` or `write`.
    fn path_context(self, action: &str, path: &Path) -> Result<T>;
}

impl<T> PathContext<T> for io::Result<T> {
    fn path_context(self, action: &str, path: &Path) -> Result<T> {
        self.map_err(|e| {
            Error::Io(io_context(
                e,
                format!("cannot {action} `{}`", path.display()),
            ))
        })
    }
}

#[derive(Debug)]
struct IoContext {
    context: String,
    source: io::Error,
}

impl fmt::Display for IoContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.context)
    }
}

impl StdError for IoContext {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.source)
    }
}
