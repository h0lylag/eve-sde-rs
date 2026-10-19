//! Synthetic SDE archives for tests, so no real SDE data is needed.
//!
//! ```
//! use eve_sde::Archive;
//! use eve_sde::model::Type;
//! use eve_sde::testing::ArchiveBuilder;
//! use std::io::Cursor;
//!
//! let bytes = ArchiveBuilder::new(1)
//!     .rows::<Type>(r#"{"_key":587,"groupID":25,"name":{"en":"Rifter"},"portionSize":1,"published":true}"#)
//!     .into_bytes();
//! let mut archive = Archive::from_reader(Cursor::new(bytes))?;
//! assert_eq!(archive.table::<Type>()?.len(), 1);
//! # Ok::<(), eve_sde::Error>(())
//! ```

use crate::archive::BUILD_FILE;
use crate::tables::{Record, tables};
use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Builds an SDE ZIP in memory.
///
/// It starts with a build record and an empty file for every table this crate
/// reads. Fill the tables a test needs with [`ArchiveBuilder::rows`].
#[derive(Debug, Clone)]
#[must_use = "call `into_bytes` to get the archive"]
pub struct ArchiveBuilder {
    build_number: u32,
    release_date: Option<String>,
    build_record: BuildRecord,
    files: BTreeMap<String, String>,
    compression: CompressionMethod,
}

/// What goes in `_sde.jsonl`.
#[derive(Debug, Clone)]
enum BuildRecord {
    /// A valid record made from the build number and release date.
    Generated,
    /// Text a test chose, valid or not.
    Custom(String),
    /// No `_sde.jsonl` at all.
    Missing,
}

impl ArchiveBuilder {
    /// An archive of build `build_number` with every table empty.
    pub fn new(build_number: u32) -> Self {
        Self {
            build_number,
            release_date: None,
            build_record: BuildRecord::Generated,
            files: tables()
                .iter()
                .map(|table| (table.file.to_owned(), String::new()))
                .collect(),
            compression: CompressionMethod::Deflated,
        }
    }

    /// Gives the build record a release date, e.g. `2026-10-02T11:08:57Z`.
    pub fn release_date(mut self, date: impl Into<String>) -> Self {
        self.release_date = Some(date.into());
        self
    }

    /// Sets the contents of `T`'s table: JSON Lines, one record per line.
    pub fn rows<T: Record>(self, lines: impl Into<String>) -> Self {
        self.file(T::FILE, lines)
    }

    /// Adds a file, or replaces one. Use it for a file this crate does not
    /// read, or to replace the build record `_sde.jsonl` with other text.
    pub fn file(mut self, name: impl Into<String>, contents: impl Into<String>) -> Self {
        let name = name.into();
        if name == BUILD_FILE {
            self.build_record = BuildRecord::Custom(contents.into());
        } else {
            self.files.insert(name, contents.into());
        }
        self
    }

    /// Leaves a file out of the archive, including the build record
    /// `_sde.jsonl`.
    pub fn without(mut self, name: &str) -> Self {
        if name == BUILD_FILE {
            self.build_record = BuildRecord::Missing;
        } else {
            self.files.remove(name);
        }
        self
    }

    /// Stores the files uncompressed, so their bytes appear as is in the
    /// archive.
    pub fn stored(mut self) -> Self {
        self.compression = CompressionMethod::Stored;
        self
    }

    /// The archive as ZIP bytes.
    ///
    /// # Panics
    ///
    /// Panics if the ZIP cannot be written, which in memory only happens on a
    /// bug in the ZIP library.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        let build_record = match self.build_record {
            BuildRecord::Generated => Some(match &self.release_date {
                Some(date) => format!(
                    r#"{{"_key":"sde","buildNumber":{},"releaseDate":"{date}"}}"#,
                    self.build_number
                ),
                None => format!(r#"{{"_key":"sde","buildNumber":{}}}"#, self.build_number),
            }),
            BuildRecord::Custom(text) => Some(text),
            BuildRecord::Missing => None,
        };
        let options = SimpleFileOptions::default().compression_method(self.compression);
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let files = build_record
            .iter()
            .map(|text| (BUILD_FILE, text))
            .chain(self.files.iter().map(|(name, text)| (name.as_str(), text)));
        for (name, contents) in files {
            writer
                .start_file(name, options)
                .expect("writing a ZIP entry to memory should not fail");
            writer
                .write_all(contents.as_bytes())
                .expect("writing to memory should not fail");
        }
        writer
            .finish()
            .expect("finishing a ZIP in memory should not fail")
            .into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Archive;
    use crate::model::Type;

    #[test]
    fn archives_hold_the_build_and_an_empty_file_for_every_table() {
        let bytes = ArchiveBuilder::new(7)
            .release_date("2026-10-02T11:08:57Z")
            .into_bytes();
        let mut archive = Archive::from_reader(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.build().build_number, 7);
        assert_eq!(
            archive.build().release_date.as_deref(),
            Some("2026-10-02T11:08:57Z")
        );
        assert_eq!(archive.unmodeled_files(), Vec::<String>::new());
        assert!(archive.table::<Type>().unwrap().is_empty());
    }
}
