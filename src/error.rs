use std::error::Error as StdError;
use std::fmt;
use std::io;

/// Everything that can go wrong while reading, loading or downloading the SDE.
///
/// Errors from the ZIP and HTTP libraries are boxed, so upgrading those
/// libraries does not change this type. Downcast the box to reach them.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Reading or writing a file failed.
    Io(io::Error),
    /// The data is not a ZIP archive, or the archive is damaged.
    Zip(Box<dyn StdError + Send + Sync>),
    /// The archive has no file with this name. Did CCP change the format?
    MissingFile(String),
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
    /// Metadata, a record, or the decompressed archive exceeded a byte limit.
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
    /// A download went over [`MAX_DOWNLOAD_BYTES`](crate::download::MAX_DOWNLOAD_BYTES).
    #[cfg(feature = "download")]
    TooLarge { limit: u64 },
    /// The progress callback asked to stop.
    #[cfg(feature = "download")]
    Cancelled,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn zip(e: zip::result::ZipError) -> Self {
        Error::Zip(Box::new(e))
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
            Error::TooLarge { limit } => write!(f, "download is larger than {limit} bytes"),
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
