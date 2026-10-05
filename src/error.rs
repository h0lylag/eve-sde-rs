use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::path::Path;

/// Everything that can go wrong while reading, loading or downloading the SDE.
///
/// Errors from the ZIP and HTTP libraries are boxed, so upgrading those
/// libraries does not change this type. Downcast the box to reach them.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Reading or writing a file failed, or a download broke off. The message
    /// names the file or URL.
    Io(io::Error),
    /// The data is not a ZIP archive, or the archive's directory is damaged.
    /// Damage inside one file is [`Error::Read`].
    Zip(Box<dyn StdError + Send + Sync>),
    /// The archive has no file with this name. Did CCP change the format?
    MissingFile(String),
    /// A file in the archive could not be read. `source` says why, e.g.
    /// `Invalid checksum` when the file is damaged.
    Read { file: String, source: io::Error },
    /// A line failed to parse. `line` counts from 1.
    Parse {
        file: String,
        line: usize,
        source: serde_json::Error,
    },
    /// Two records in one file share a key.
    DuplicateKey { file: String, key: String },
    /// The archive or a response is not what we expected.
    Invalid(String),
    /// Metadata, a record, the decompressed archive or a download exceeded a
    /// byte limit. `resource` says which, e.g. `download`.
    LimitExceeded { resource: String, limit: u64 },
    /// The request failed before a response arrived: DNS, TLS, a timeout, …
    #[cfg(feature = "download")]
    Http(Box<dyn StdError + Send + Sync>),
    /// The server answered with a status other than the one expected.
    #[cfg(feature = "download")]
    HttpStatus { url: String, status: u16 },
    /// A downloaded archive has a different build than the one requested.
    #[cfg(feature = "download")]
    BuildMismatch { expected: u32, got: u32 },
    /// The progress callback asked to stop.
    #[cfg(feature = "download")]
    Cancelled,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn zip(e: zip::result::ZipError) -> Self {
        Error::Zip(Box::new(e))
    }

    pub(crate) fn read(file: &str, source: io::Error) -> Self {
        Error::Read {
            file: file.to_owned(),
            source,
        }
    }

    #[cfg(feature = "download")]
    pub(crate) fn http(e: ureq::Error) -> Self {
        Error::Http(Box::new(e))
    }
}

// A plain wrapper shows the wrapped error's message and passes on its source.
// A variant that adds context shows only the context and returns the cause
// from `source`, so error reporters never print a message twice.
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => e.fmt(f),
            Error::Zip(e) => e.fmt(f),
            Error::MissingFile(name) => write!(f, "archive has no file `{name}`"),
            Error::Read { file, .. } => write!(f, "cannot read `{file}`"),
            Error::Parse { file, line, .. } => write!(f, "cannot parse `{file}` line {line}"),
            Error::DuplicateKey { file, key } => {
                write!(f, "`{file}` has more than one record with key {key}")
            }
            Error::Invalid(msg) => f.write_str(msg),
            Error::LimitExceeded { resource, limit } => {
                write!(f, "{resource} exceeds the limit of {limit} bytes")
            }
            #[cfg(feature = "download")]
            Error::Http(e) => e.fmt(f),
            #[cfg(feature = "download")]
            Error::HttpStatus { url, status } => write!(f, "{url} returned HTTP {status}"),
            #[cfg(feature = "download")]
            Error::BuildMismatch { expected, got } => {
                write!(
                    f,
                    "expected SDE build {expected} but the archive is build {got}"
                )
            }
            #[cfg(feature = "download")]
            Error::Cancelled => f.write_str("download cancelled"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(e) => e.source(),
            Error::Zip(e) => e.source(),
            Error::Read { source, .. } => Some(source),
            Error::Parse { source, .. } => Some(source),
            #[cfg(feature = "download")]
            Error::Http(e) => e.source(),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// Says what the crate was doing when an I/O error happened, e.g. "cannot
/// open `sde.zip`". The error keeps its kind, and its source is the original.
pub(crate) fn io_context(source: io::Error, context: String) -> io::Error {
    io::Error::new(source.kind(), IoContext { context, source })
}

/// Names the file in a failed I/O result's error, using [`io_context`].
pub(crate) trait PathContext<T> {
    /// `action` completes "cannot …", e.g. `open` or `write`.
    fn path_context(self, action: &str, path: &Path) -> io::Result<T>;
}

impl<T> PathContext<T> for io::Result<T> {
    fn path_context(self, action: &str, path: &Path) -> io::Result<T> {
        self.map_err(|e| io_context(e, format!("cannot {action} `{}`", path.display())))
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
