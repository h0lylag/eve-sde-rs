//! Reading CCP's JSON Lines ZIP: one file per table, one record per line.

use crate::error::{Error, Result};
use serde::Deserialize;
#[cfg(feature = "load")]
use serde::de::DeserializeOwned;
#[cfg(feature = "load")]
use std::collections::HashMap;
#[cfg(feature = "load")]
use std::collections::hash_map::Entry;
#[cfg(feature = "load")]
use std::fmt::Debug;
#[cfg(feature = "load")]
use std::hash::Hash;
use std::io::{self, Read, Seek};
#[cfg(feature = "load")]
use std::io::{BufRead, BufReader};
use zip::ZipArchive;
use zip::read::ZipFile;
use zip::result::ZipError;

/// The file in every SDE archive that says which build it is.
const BUILD_FILE: &str = "_sde.jsonl";

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

/// One SDE file and the Rust type its lines parse into.
#[cfg(feature = "load")]
pub trait Record: DeserializeOwned {
    /// File name inside the ZIP, e.g. `types.jsonl`.
    const FILE: &'static str;
    type Id: Eq + Hash + Debug;
    fn id(&self) -> Self::Id;
}

pub struct Archive<R: Read + Seek> {
    zip: ZipArchive<R>,
    build: BuildInfo,
    max_bytes: u64,
    /// Decompressed bytes that table reads may still use.
    #[cfg(feature = "load")]
    remaining: u64,
}

impl<R: Read + Seek> Archive<R> {
    /// Open an archive, check its declared sizes and read its build.
    pub fn new(reader: R) -> Result<Self> {
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
            if file.name() == BUILD_FILE && file.size() > MAX_METADATA_BYTES {
                return Err(limit_error("build metadata", MAX_METADATA_BYTES));
            }
        }
        let text = read_metadata(open(&mut zip, BUILD_FILE)?, |e| Error::read(BUILD_FILE, e))?;
        // The declared sizes can lie, so from here on count the bytes read.
        let remaining = max_bytes
            .checked_sub(text.len() as u64)
            .ok_or_else(|| limit_error("decompressed archive", max_bytes))?;
        #[cfg(not(feature = "load"))]
        let _ = remaining;
        let build = BuildInfo::parse(&text)?;
        Ok(Archive {
            zip,
            build,
            max_bytes,
            #[cfg(feature = "load")]
            remaining,
        })
    }

    pub fn build(&self) -> &BuildInfo {
        &self.build
    }

    /// Names of the table files: everything except `_sde.jsonl`.
    #[cfg(feature = "load")]
    pub fn table_files(&self) -> impl Iterator<Item = &str> {
        self.zip.file_names().filter(|name| *name != BUILD_FILE)
    }

    /// Parse every line of `T::FILE` into a map by key. A repeated key is an
    /// error.
    #[cfg(feature = "load")]
    pub fn table<T: Record>(&mut self) -> Result<HashMap<T::Id, T>> {
        let mut table = HashMap::new();
        let mut reader = BufReader::new(open(&mut self.zip, T::FILE)?);
        let mut line = String::new();
        let mut line_number = 0;
        loop {
            line.clear();
            let bytes = (&mut reader)
                .take(MAX_RECORD_BYTES.min(self.remaining) + 1)
                .read_line(&mut line)
                .map_err(|e| Error::read(T::FILE, e))? as u64;
            if bytes == 0 {
                break;
            }
            line_number += 1;
            if bytes > self.remaining {
                return Err(limit_error("decompressed archive", self.max_bytes));
            }
            if bytes > MAX_RECORD_BYTES {
                return Err(limit_error(
                    format!("`{}` line {line_number}", T::FILE),
                    MAX_RECORD_BYTES,
                ));
            }
            self.remaining -= bytes;
            if line.trim().is_empty() {
                continue;
            }
            let row: T = serde_json::from_str(&line).map_err(|source| Error::Parse {
                file: T::FILE.to_owned(),
                line: line_number,
                source,
            })?;
            match table.entry(row.id()) {
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
    pub fn verify(&mut self) -> Result<()> {
        let mut remaining = self.max_bytes;
        for i in 0..self.zip.len() {
            let mut file = self.zip.by_index(i).map_err(Error::zip)?;
            let copied = io::copy(&mut (&mut file).take(remaining + 1), &mut io::sink());
            let bytes = copied.map_err(|e| Error::read(file.name(), e))?;
            remaining = remaining
                .checked_sub(bytes)
                .ok_or_else(|| limit_error("decompressed archive", self.max_bytes))?;
        }
        Ok(())
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
    use std::io::{Cursor, Write};
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn fixture(understate_sizes: bool) -> (Vec<u8>, u64) {
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
        if understate_sizes {
            // Zero the uncompressed size in each local header (offset 22) and
            // central directory entry (offset 24).
            let offsets: Vec<_> = bytes
                .windows(4)
                .enumerate()
                .filter_map(|(i, signature)| match signature {
                    b"PK\x03\x04" => Some(i + 22),
                    b"PK\x01\x02" => Some(i + 24),
                    _ => None,
                })
                .collect();
            for offset in offsets {
                bytes[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes());
            }
        }
        (bytes, total)
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

    #[cfg(feature = "load")]
    #[test]
    fn table_budget_counts_actual_bytes_across_files() {
        let (bytes, total) = fixture(true);
        for limit in [total - 1, total] {
            let mut archive = Archive::with_limit(Cursor::new(&bytes), limit).unwrap();
            archive.table::<crate::model::Type>().unwrap();
            let result = archive.table::<crate::model::TypeDogma>();
            if limit < total {
                assert!(
                    matches!(result, Err(Error::LimitExceeded { resource, .. }) if resource == "decompressed archive")
                );
            } else {
                assert_eq!(result.unwrap().len(), 1);
            }
        }
    }

    #[cfg(feature = "download")]
    #[test]
    fn verification_budget_counts_actual_bytes_across_files() {
        let (bytes, total) = fixture(true);
        for limit in [total - 1, total] {
            let mut archive = Archive::with_limit(Cursor::new(&bytes), limit).unwrap();
            let result = archive.verify();
            if limit < total {
                assert!(
                    matches!(result, Err(Error::LimitExceeded { resource, .. }) if resource == "decompressed archive")
                );
            } else {
                result.unwrap();
            }
        }
    }
}
