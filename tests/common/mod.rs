#![allow(dead_code)]

use std::io::{Cursor, Write};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

pub fn build_line(build: u32) -> String {
    format!(
        "{{\"_key\":\"sde\",\"buildNumber\":{build},\"releaseDate\":\"2026-10-02T11:08:57Z\"}}\n"
    )
}

/// A ZIP holding exactly these files.
pub fn zip_bytes(files: &[(&str, &str)]) -> Vec<u8> {
    zip_with(files, SimpleFileOptions::default())
}

/// Like [`zip_bytes`] but uncompressed, so file contents appear verbatim.
pub fn stored_zip_bytes(files: &[(&str, &str)]) -> Vec<u8> {
    zip_with(
        files,
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
}

fn zip_with(files: &[(&str, &str)], options: SimpleFileOptions) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in files {
        writer.start_file(*name, options).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// A ZIP holding only `_sde.jsonl` for this build.
pub fn build_only_zip(build: u32) -> Vec<u8> {
    zip_bytes(&[("_sde.jsonl", &build_line(build))])
}

/// Understate entry sizes without changing compressed data or checksums.
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

/// A full-looking SDE ZIP: the given files, plus an empty file for every
/// table the crate expects. Pass `skip` to leave one table out.
#[cfg(feature = "load")]
pub fn sde_zip(files: &[(&str, &str)], skip: Option<&str>) -> Vec<u8> {
    let metadata = build_line(3569502);
    let mut entries: Vec<(&str, &str)> = vec![("_sde.jsonl", &metadata)];
    for name in eve_sde::modeled_files() {
        if Some(*name) == skip {
            continue;
        }
        let body = files
            .iter()
            .find(|(n, _)| n == name)
            .map_or("", |(_, b)| *b);
        entries.push((name, body));
    }
    for (name, body) in files {
        if !eve_sde::modeled_files().contains(name) {
            entries.push((name, body));
        }
    }
    zip_bytes(&entries)
}
