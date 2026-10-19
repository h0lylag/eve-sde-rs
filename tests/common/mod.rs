//! Helpers for the integration tests: low-level ZIP helpers for archives that
//! `ArchiveBuilder` cannot make, such as one whose headers understate its
//! sizes, and a listing of a folder.
#![allow(dead_code, reason = "each test crate uses only some of these helpers")]

use std::io::{Cursor, Write};
use std::path::Path;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// The `_sde.jsonl` record of this build.
pub fn build_line(build: u32) -> String {
    format!(
        "{{\"_key\":\"sde\",\"buildNumber\":{build},\"releaseDate\":\"2026-10-02T11:08:57Z\"}}\n"
    )
}

/// A ZIP holding exactly these files.
pub fn zip_bytes(files: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in files {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// A ZIP holding only `_sde.jsonl` for this build.
pub fn build_only_zip(build: u32) -> Vec<u8> {
    zip_bytes(&[("_sde.jsonl", &build_line(build))])
}

/// Set each entry's uncompressed size to 0 in its local header (offset 22)
/// and its central directory entry (offset 24). The data and checksums stay.
pub fn understate_sizes(bytes: &mut [u8]) {
    let headers: Vec<_> = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(i, signature)| match signature {
            b"PK\x03\x04" => Some(i + 22),
            b"PK\x01\x02" => Some(i + 24),
            _ => None,
        })
        .collect();
    for offset in headers {
        bytes[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes());
    }
}

/// The sorted names of the files in `dir`.
pub fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
