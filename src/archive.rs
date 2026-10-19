//! Reading CCP's JSON Lines ZIP: one file per table, one record per line.

use crate::error::{Error, PathContext, Result};
use crate::tables::{Record, is_table_file};
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek};
use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::path::Path;
use zip::ZipArchive;
use zip::read::ZipFile;
use zip::result::ZipError;

/// The file in every SDE archive that says which build it is.
pub(crate) const BUILD_FILE: &str = "_sde.jsonl";

/// Maximum size of `_sde.jsonl` or the latest-build response: 1 MiB.
pub const MAX_METADATA_BYTES: u64 = 1024 * 1024;
/// Maximum size of a table record, including its line ending: 1 MiB.
pub const MAX_RECORD_BYTES: u64 = 1024 * 1024;
/// Maximum total size of decompressed archive entries: 2 GiB.
pub const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub(crate) fn limit_error(resource: impl Into<String>, limit: u64) -> Error {
    Error::LimitExceeded {
        resource: resource.into(),
        limit,
    }
}

/// Read build metadata of at most [`MAX_METADATA_BYTES`]. The limit counts
/// the bytes actually read, so a ZIP or HTTP header that understates the size
/// does not get around it. `read_error` turns a failed read into the caller's
/// error.
pub(crate) fn read_metadata(
    reader: impl Read,
    read_error: impl FnOnce(io::Error) -> Error,
) -> Result<String> {
    let mut text = String::new();
    reader
        .take(MAX_METADATA_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(read_error)?;
    // Widening: `usize` is at most 64 bits on every target.
    if text.len() as u64 > MAX_METADATA_BYTES {
        return Err(limit_error("build metadata", MAX_METADATA_BYTES));
    }
    Ok(text)
}

/// Which SDE build an archive holds, or which build CCP says is the latest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct BuildInfo {
    /// CCP's number for the build, e.g. `3569502`. Later builds have higher
    /// numbers.
    pub build_number: u32,
    /// When CCP released the build, in ISO 8601, e.g. `2026-10-02T11:08:57Z`.
    pub release_date: Option<String>,
}

impl BuildInfo {
    /// Parse the first record of an archive's `_sde.jsonl` or of CCP's
    /// `latest.jsonl`: `{"_key":"sde","buildNumber":…,"releaseDate":…}`.
    pub(crate) fn parse(text: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct BuildRecord {
            #[serde(rename = "_key")]
            key: String,
            #[serde(flatten)]
            info: BuildInfo,
        }

        let line = text
            .lines()
            .find(|line| !line.trim().is_empty())
            .ok_or_else(|| Error::Invalid("SDE build record is empty".into()))?;
        let record: BuildRecord = serde_json::from_str(line)
            .map_err(|e| Error::Invalid(format!("bad SDE build record: {e}")))?;
        if record.key != "sde" || record.info.build_number == 0 {
            return Err(Error::Invalid(format!("bad SDE build record: {line}")));
        }
        Ok(record.info)
    }
}

/// An open SDE archive: CCP's JSON Lines ZIP.
///
/// Opening reads only the build record and the list of files. Each table is
/// read when asked for: one record at a time with [`Archive::records`], or
/// all at once with [`Archive::table`].
///
/// ```no_run
/// use eve_sde::Archive;
/// use eve_sde::model::Type;
///
/// let mut archive = Archive::open("sde.zip")?;
/// println!("build {}", archive.build().build_number);
/// for ty in archive.records::<Type>()? {
///     let ty = ty?;
///     if ty.name == "Rifter" {
///         println!("{}", ty.id);
///     }
/// }
/// # Ok::<(), eve_sde::Error>(())
/// ```
///
/// Every byte read counts toward [`MAX_ARCHIVE_BYTES`] for the life of the
/// archive, so an archive that lies about its sizes cannot run on forever.
/// Reading every table once stays far below it.
pub struct Archive<R = BufReader<File>> {
    zip: ZipArchive<R>,
    build: BuildInfo,
    unmodeled: Vec<String>,
    max_bytes: u64,
    /// Decompressed bytes that table reads may still use.
    remaining: u64,
}

impl Archive {
    /// Open an SDE ZIP on disk.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be opened, or for any reason listed under
    /// [`Archive::from_reader`].
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        fn open(path: &Path) -> Result<Archive> {
            let file = File::open(path).path_context("open", path)?;
            Archive::from_reader(BufReader::new(file))
        }
        open(path.as_ref())
    }
}

impl<R: Read + Seek> Archive<R> {
    /// Open an SDE ZIP from any seekable source, such as a
    /// [`Cursor`](std::io::Cursor) over bytes in memory.
    ///
    /// # Errors
    ///
    /// Fails if the data is not a ZIP, its build record is missing or
    /// malformed, or a size limit is exceeded.
    pub fn from_reader(reader: R) -> Result<Self> {
        Self::with_limit(reader, MAX_ARCHIVE_BYTES)
    }

    fn with_limit(reader: R, max_bytes: u64) -> Result<Self> {
        let mut zip = ZipArchive::new(reader).map_err(Error::zip)?;
        let mut remaining = max_bytes;
        for i in 0..zip.len() {
            let file = zip.by_index_raw(i).map_err(Error::zip)?;
            remaining = remaining
                .checked_sub(file.size())
                .ok_or_else(|| limit_error("decompressed archive", max_bytes))?;
            let is_build_file = file.name().map_err(Error::zip)? == BUILD_FILE;
            if is_build_file && file.size() > MAX_METADATA_BYTES {
                return Err(limit_error("build metadata", MAX_METADATA_BYTES));
            }
        }
        let text = read_metadata(open(&mut zip, BUILD_FILE)?, |e| Error::read(BUILD_FILE, e))?;
        // The declared sizes can lie, so from here on count the bytes read.
        // Widening: `usize` is at most 64 bits on every target.
        let remaining = max_bytes
            .checked_sub(text.len() as u64)
            .ok_or_else(|| limit_error("decompressed archive", max_bytes))?;
        let build = BuildInfo::parse(&text)?;
        let mut unmodeled = Vec::new();
        for name in zip.file_names() {
            let name = name.map_err(Error::zip)?;
            if name != BUILD_FILE && !is_table_file(&name) {
                unmodeled.push(name.into_owned());
            }
        }
        unmodeled.sort();
        Ok(Self {
            zip,
            build,
            unmodeled,
            max_bytes,
            remaining,
        })
    }

    /// The build this archive holds.
    pub fn build(&self) -> &BuildInfo {
        &self.build
    }

    /// Sorted names of the files in the archive that this crate does not
    /// read. Not empty means CCP added a table this version does not know.
    pub fn unmodeled_files(&self) -> &[String] {
        &self.unmodeled
    }

    /// Read the table of `T` one record at a time. Only the current line is
    /// held in memory.
    ///
    /// The iterator stops after the first error.
    ///
    /// # Errors
    ///
    /// Fails if the archive has no file for `T`. Each item fails if its line
    /// cannot be read or parsed, or exceeds a size limit.
    pub fn records<T: Record>(&mut self) -> Result<Records<'_, R, T>> {
        Ok(Records {
            lines: self.lines(T::FILE)?,
            done: false,
            record: PhantomData,
        })
    }

    /// Read any file of the archive one line at a time, with the same limits
    /// as [`Archive::records`].
    pub(crate) fn lines(&mut self, file: &str) -> Result<Lines<'_, R>> {
        let Self {
            zip,
            remaining,
            max_bytes,
            ..
        } = self;
        Ok(Lines {
            reader: BufReader::new(open(zip, file)?),
            file: file.to_owned(),
            remaining,
            max_bytes: *max_bytes,
            line: String::new(),
            line_number: 0,
        })
    }

    /// Whether the archive has a file named `file`.
    pub(crate) fn has_file(&self, file: &str) -> bool {
        self.zip.index_for_name(file).is_some()
    }

    /// Read the whole table of `T` into a map by key.
    ///
    /// # Errors
    ///
    /// Fails for the reasons listed under [`Archive::records`], and if two
    /// records share a key.
    pub fn table<T: Record>(&mut self) -> Result<HashMap<T::Key, T>> {
        let mut table = HashMap::new();
        for row in self.records::<T>()? {
            let row = row?;
            match table.entry(row.key()) {
                Entry::Vacant(slot) => {
                    slot.insert(row);
                }
                Entry::Occupied(slot) => {
                    return Err(Error::DuplicateKey {
                        file: T::FILE.to_owned(),
                        key: format!("{:?}", slot.key()),
                    });
                }
            }
        }
        Ok(table)
    }

    /// Decompress every file, which checks each one against its CRC-32.
    #[cfg(feature = "download")]
    pub(crate) fn verify(&mut self) -> Result<()> {
        let mut remaining = self.max_bytes;
        for i in 0..self.zip.len() {
            let mut file = self.zip.by_index(i).map_err(Error::zip)?;
            let name = file.name().map_err(Error::zip)?.into_owned();
            let copied = io::copy(&mut (&mut file).take(remaining + 1), &mut io::sink());
            let bytes = copied.map_err(|e| Error::read(&name, e))?;
            remaining = remaining
                .checked_sub(bytes)
                .ok_or_else(|| limit_error("decompressed archive", self.max_bytes))?;
        }
        Ok(())
    }
}

/// Shows the build and unknown files only.
impl<R> fmt::Debug for Archive<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Archive")
            .field("build", &self.build)
            .field("unmodeled", &self.unmodeled)
            .finish_non_exhaustive()
    }
}

/// The records of one table, parsed one line at a time. Made by
/// [`Archive::records`].
pub struct Records<'a, R: Read + Seek, T> {
    lines: Lines<'a, R>,
    done: bool,
    record: PhantomData<fn() -> T>,
}

impl<R: Read + Seek, T: Record> Iterator for Records<'_, R, T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let next = self.lines.next_line().map(|line| {
            let (number, text) = line?;
            serde_json::from_str(text).map_err(|source| Error::parse(T::FILE, number, source))
        });
        self.done = !matches!(next, Some(Ok(_)));
        next
    }
}

impl<R: Read + Seek, T: Record> FusedIterator for Records<'_, R, T> {}

impl<R: Read + Seek, T: Record> fmt::Debug for Records<'_, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Records")
            .field("file", &T::FILE)
            .field("line", &self.lines.line_number)
            .finish_non_exhaustive()
    }
}

/// The lines of one file in the archive. Each line counts toward the
/// archive's limit and must fit in [`MAX_RECORD_BYTES`].
pub(crate) struct Lines<'a, R: Read + Seek> {
    reader: BufReader<ZipFile<'a, R>>,
    file: String,
    remaining: &'a mut u64,
    max_bytes: u64,
    line: String,
    line_number: usize,
}

impl<R: Read + Seek> Lines<'_, R> {
    /// The next line that is not blank, with its number counting from 1.
    /// `None` at the end of the file.
    pub(crate) fn next_line(&mut self) -> Option<Result<(usize, &str)>> {
        loop {
            self.line.clear();
            let bytes = match (&mut self.reader)
                .take(MAX_RECORD_BYTES.min(*self.remaining) + 1)
                .read_line(&mut self.line)
            {
                Ok(0) => return None,
                // Widening: `usize` is at most 64 bits on every target.
                Ok(bytes) => bytes as u64,
                Err(e) => return Some(Err(Error::read(&self.file, e))),
            };
            self.line_number += 1;
            if bytes > *self.remaining {
                return Some(Err(limit_error("decompressed archive", self.max_bytes)));
            }
            if bytes > MAX_RECORD_BYTES {
                return Some(Err(limit_error(
                    format!("`{}` line {}", self.file, self.line_number),
                    MAX_RECORD_BYTES,
                )));
            }
            *self.remaining -= bytes;
            if !self.line.trim().is_empty() {
                return Some(Ok((self.line_number, &self.line)));
            }
        }
    }
}

fn open<'a, R: Read + Seek>(zip: &'a mut ZipArchive<R>, name: &str) -> Result<ZipFile<'a, R>> {
    zip.by_name(name).map_err(|e| match e {
        ZipError::FileNotFound => Error::MissingFile(name.to_owned()),
        other => Error::zip(other),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Type, TypeDogma};
    use std::io::{Cursor, Write};
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn fixture(understate_tables: bool) -> (Vec<u8>, u64) {
        let entries = [
            (BUILD_FILE, r#"{"_key":"sde","buildNumber":1}"#),
            (
                "types.jsonl",
                r#"{"_key":1,"groupID":1,"name":{"en":"x"},"portionSize":1,"published":true}"#,
            ),
            ("typeDogma.jsonl", r#"{"_key":1}"#),
        ];
        let total = entries.iter().map(|(_, text)| text.len() as u64).sum();
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in entries {
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(text.as_bytes()).unwrap();
        }
        let mut bytes = writer.finish().unwrap().into_inner();
        if understate_tables {
            // Zero the uncompressed size of every entry but the first, the
            // build record, in its local header (offset 22) and its central
            // directory entry (offset 24).
            let local = size_offsets(&bytes, *b"PK\x03\x04", 22);
            let central = size_offsets(&bytes, *b"PK\x01\x02", 24);
            for offset in local.into_iter().skip(1).chain(central.into_iter().skip(1)) {
                bytes[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes());
            }
        }
        (bytes, total)
    }

    /// Where each header with `signature` starts, plus `offset`.
    fn size_offsets(bytes: &[u8], signature: [u8; 4], offset: usize) -> Vec<usize> {
        bytes
            .windows(4)
            .enumerate()
            .filter(|(_, window)| *window == signature)
            .map(|(i, _)| i + offset)
            .collect()
    }

    #[test]
    fn rejects_declared_total_before_reading_tables() {
        let (bytes, total) = fixture(false);
        assert!(matches!(
            Archive::with_limit(Cursor::new(&bytes), total - 1),
            Err(Error::LimitExceeded { resource, .. }) if resource == "decompressed archive"
        ));
        assert!(Archive::with_limit(Cursor::new(&bytes), total).is_ok());
    }

    #[test]
    fn the_table_budget_counts_every_read() {
        let (bytes, total) = fixture(false);
        let mut archive = Archive::with_limit(Cursor::new(&bytes), total).unwrap();
        archive.table::<Type>().unwrap();
        assert_eq!(archive.table::<TypeDogma>().unwrap().len(), 1);
        // Every byte has been read once, so a second read goes over.
        assert!(matches!(
            archive.table::<Type>(),
            Err(Error::LimitExceeded { resource, .. }) if resource == "decompressed archive"
        ));
    }

    #[test]
    fn a_table_bigger_than_declared_is_rejected() {
        let (bytes, _) = fixture(true);
        let mut archive = Archive::from_reader(Cursor::new(&bytes)).unwrap();
        assert!(matches!(
            archive.table::<Type>(),
            Err(Error::Read { file, source })
                if file == "types.jsonl" && source.kind() == io::ErrorKind::InvalidData
        ));
    }

    #[cfg(feature = "download")]
    #[test]
    fn verification_rejects_an_entry_bigger_than_declared() {
        let (bytes, _) = fixture(true);
        let mut archive = Archive::from_reader(Cursor::new(&bytes)).unwrap();
        assert!(matches!(
            archive.verify(),
            Err(Error::Read { file, .. }) if file == "types.jsonl"
        ));
    }
}
