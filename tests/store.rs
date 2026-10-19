//! The packed record store, filled from synthetic archives.
#![cfg_attr(
    feature = "store",
    expect(
        clippy::unwrap_used,
        reason = "test helpers fail by panicking, like the tests"
    )
)]
#![cfg(feature = "store")]
mod common;

use common::file_names;
use eve_sde::ids::{CelestialId, GroupId, StationId, SystemId, TypeId};
use eve_sde::model::{CharacterTitle, NpcStation, SolarSystem, Star, Type};
use eve_sde::store::{self, ImportOutcome, LAYOUT_VERSION, Store};
use eve_sde::testing::ArchiveBuilder;
use eve_sde::{Archive, Error, Sde};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::thread;

const TRITANIUM: &str = r#"{"_key":34,"groupID":18,"name":{"en":"Tritanium","de":"Tritanium"},"portionSize":1,"published":true}"#;
const RIFTER: &str = r#"{"_key":587,"groupID":25,"name":{"en":"Rifter","ja":"リフター"},"portionSize":1,"published":true}"#;
const SYSTEMS: &str = r#"{"_key":30000142,"constellationID":20000020,"name":{"en":"Jita"},"position":{"x":0,"y":0,"z":0},"radius":1.0,"regionID":10000002,"securityStatus":0.9,"stargateIDs":[],"planetIDs":[]}"#;
const TITLES: &str = r#"{"_key":"a1b2","name":{"en":"Captain","fr":"Capitaine"}}
{"_key":"c3d4","name":{"en":"Scout"}}"#;
const STATION: &str = r#"{"_key":60000001,"operationID":1,"orbitID":40000001,"ownerID":1000035,"position":{"x":0.0,"y":0.0,"z":0.0},"reprocessingEfficiency":0.5,"reprocessingHangarFlag":4,"reprocessingStationsTake":0.05,"solarSystemID":30000142,"typeID":1531,"useOperationName":false}"#;

fn fixture(build: u32) -> ArchiveBuilder {
    ArchiveBuilder::new(build)
        .release_date("2026-10-02T11:08:57Z")
        .rows::<Type>(&format!("{TRITANIUM}\n{RIFTER}"))
        .rows::<SolarSystem>(SYSTEMS)
        .rows::<CharacterTitle>(TITLES)
}

fn archive(builder: ArchiveBuilder) -> Archive<Cursor<Vec<u8>>> {
    Archive::from_reader(Cursor::new(builder.into_bytes())).unwrap()
}

fn import(builder: ArchiveBuilder, dir: &Path) -> ImportOutcome {
    store::import(&mut archive(builder), dir).unwrap()
}

/// Import `builder` into a fresh folder and open the store.
fn imported(builder: ArchiveBuilder) -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(import(builder, dir.path()), ImportOutcome::Imported);
    let store = Store::open(dir.path()).unwrap();
    (dir, store)
}

fn store_file(build: u32) -> String {
    format!("sde-{build}-v{LAYOUT_VERSION}.sde")
}

#[test]
fn records_read_back_through_their_models() {
    let (_dir, store) = imported(fixture(1));
    assert_eq!(store.build().build_number, 1);
    assert_eq!(
        store.build().release_date.as_deref(),
        Some("2026-10-02T11:08:57Z")
    );

    let rifter = store.get::<Type>(TypeId(587)).unwrap().unwrap();
    assert_eq!(rifter.name, "Rifter");
    assert_eq!(rifter.group_id, GroupId(25));
    let tritanium = store.get::<Type>(TypeId(34)).unwrap().unwrap();
    assert_eq!(tritanium.name, "Tritanium");
    let jita = store
        .get::<SolarSystem>(SystemId(30_000_142))
        .unwrap()
        .unwrap();
    assert_eq!(jita.name, "Jita");
}

#[test]
fn text_keys_are_looked_up_as_text() {
    let (_dir, store) = imported(fixture(1));
    let captain = store
        .get::<CharacterTitle>("a1b2".to_owned())
        .unwrap()
        .unwrap();
    assert_eq!(captain.name, "Captain");
    let scout = store
        .get::<CharacterTitle>("c3d4".to_owned())
        .unwrap()
        .unwrap();
    assert_eq!(scout.name, "Scout");
    assert!(
        store
            .get::<CharacterTitle>("nope".to_owned())
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_key_the_store_does_not_hold_is_none() {
    let (_dir, store) = imported(fixture(1));
    assert!(store.get::<Type>(TypeId(1)).unwrap().is_none());
    assert!(store.get::<SolarSystem>(SystemId(1)).unwrap().is_none());
    // An empty table answers, rather than failing.
    assert!(
        store
            .get::<Star>(CelestialId(40_000_001))
            .unwrap()
            .is_none()
    );
}

#[test]
fn records_keep_the_bytes_ccp_shipped() {
    let (_dir, store) = imported(fixture(1));
    // Every language, in CCP's own shape and byte for byte.
    assert_eq!(
        store.get_raw::<Type>(TypeId(587)).unwrap().as_deref(),
        Some(RIFTER)
    );
    assert_eq!(
        store.get_raw::<Type>(TypeId(34)).unwrap().as_deref(),
        Some(TRITANIUM)
    );
    assert_eq!(store.get_raw::<Type>(TypeId(1)).unwrap(), None);
}

#[test]
fn a_record_that_does_not_fit_its_model_still_reads_raw() {
    // CCP changed `groupID` to text, which `Type` cannot hold.
    let broken =
        r#"{"_key":587,"groupID":"25","name":{"en":"Rifter"},"portionSize":1,"published":true}"#;
    let (_dir, store) = imported(fixture(1).rows::<Type>(broken));
    assert!(matches!(
        store.get::<Type>(TypeId(587)),
        Err(Error::Decode { table, key, .. }) if table == "types" && key.contains("587")
    ));
    assert_eq!(
        store.get_raw::<Type>(TypeId(587)).unwrap().as_deref(),
        Some(broken)
    );
    // Another table is unaffected.
    assert!(
        store
            .get::<SolarSystem>(SystemId(30_000_142))
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_file_the_archive_lacks_becomes_an_empty_table() {
    let (_dir, store) = imported(fixture(1).without("mapStars.jsonl"));
    assert!(
        store
            .get::<Star>(CelestialId(40_000_001))
            .unwrap()
            .is_none()
    );
    assert!(store.get::<Type>(TypeId(587)).unwrap().is_some());
}

#[test]
fn files_with_no_model_are_listed() {
    let (_dir, store) = imported(fixture(1).file("newThings.jsonl", r#"{"_key":1,"a":2}"#));
    assert_eq!(store.unmodeled_files(), vec!["newThings.jsonl".to_owned()]);
    let (_dir, store) = imported(fixture(1));
    assert_eq!(store.unmodeled_files(), Vec::<String>::new());
}

/// A file with no model cannot be stored under a table's name, because
/// tables are found by name and `get` for that table would read this file's
/// records. CCP shipping one must not fail the whole import.
#[test]
fn a_file_with_no_model_that_would_take_a_tables_name_is_left_out() {
    let (_dir, store) = imported(fixture(1).file("Types.jsonl", r#"{"_key":1,"a":2}"#));
    assert_eq!(store.refused_files(), vec!["Types.jsonl".to_owned()]);
    // It is left out rather than stored under another name, and the table
    // whose name it wanted is untouched.
    assert_eq!(store.unmodeled_files(), Vec::<String>::new());
    assert_eq!(store.table("types").unwrap().rows, 2);
    assert_eq!(
        store.get::<Type>(TypeId(587)).unwrap().unwrap().name,
        "Rifter"
    );
    // The rest of the archive is there, and a store with nothing refused
    // says so.
    assert!(
        store
            .get::<CharacterTitle>("a1b2".to_owned())
            .unwrap()
            .is_some()
    );
    let (_dir, store) = imported(fixture(1));
    assert_eq!(store.refused_files(), Vec::<String>::new());
}

#[test]
fn two_records_with_one_key_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let builder = fixture(1).rows::<Type>(&format!("{RIFTER}\n{RIFTER}"));
    assert!(matches!(
        store::import(&mut archive(builder), dir.path()),
        Err(Error::DuplicateKey { file, key }) if file == "types.jsonl" && key == "587"
    ));
}

#[test]
fn a_line_without_a_usable_key_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let builder = fixture(1).rows::<Type>(r#"{"groupID":25,"name":{"en":"Rifter"}}"#);
    assert!(matches!(
        store::import(&mut archive(builder), dir.path()),
        Err(Error::Invalid(message)) if message.contains("has no integer or text `_key`")
    ));
}

#[test]
fn a_line_that_is_not_json_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let builder = fixture(1).rows::<Type>("{not json");
    assert!(matches!(
        store::import(&mut archive(builder), dir.path()),
        Err(Error::Parse { file, line, .. }) if file == "types.jsonl" && line == 1
    ));
}

#[test]
fn one_file_per_build_and_older_ones_are_deleted() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(import(fixture(10), dir.path()), ImportOutcome::Imported);
    assert_eq!(file_names(dir.path()), vec![store_file(10)]);

    // The same build again does nothing.
    assert_eq!(import(fixture(10), dir.path()), ImportOutcome::UpToDate);
    assert_eq!(file_names(dir.path()), vec![store_file(10)]);

    // A newer build replaces it.
    assert_eq!(import(fixture(11), dir.path()), ImportOutcome::Imported);
    assert_eq!(file_names(dir.path()), vec![store_file(11)]);
    assert_eq!(Store::open(dir.path()).unwrap().build().build_number, 11);

    // An older build is refused and changes nothing.
    assert_eq!(import(fixture(9), dir.path()), ImportOutcome::OlderRefused);
    assert_eq!(file_names(dir.path()), vec![store_file(11)]);
}

#[test]
fn an_empty_folder_has_no_store() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Store::open(dir.path()),
        Err(Error::NoStore { dir: missing }) if missing == dir.path()
    ));
    assert!(matches!(
        Store::open(dir.path().join("nothing-here")),
        Err(Error::NoStore { .. })
    ));
}

#[test]
fn a_store_of_another_layout_is_not_opened() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(10), dir.path());

    // A file name of another layout is not even a candidate.
    let other = dir
        .path()
        .join(format!("sde-11-v{}.sde", LAYOUT_VERSION + 1));
    fs::rename(dir.path().join(store_file(10)), &other).unwrap();
    assert!(matches!(
        Store::open(dir.path()),
        Err(Error::NoStore { .. })
    ));

    // The header is the authority, so a store written by another layout is
    // refused however it is named.
    let renamed = dir.path().join(store_file(10));
    fs::copy(&other, &renamed).unwrap();
    let other_layout = LAYOUT_VERSION + 1;
    rewrite_header(&renamed, |header| {
        header["layout"] = other_layout.into();
    });
    assert!(matches!(
        Store::open_file(&renamed),
        Err(Error::Invalid(message)) if message.contains(&format!("has layout {other_layout}"))
    ));
    assert!(matches!(
        Store::open(dir.path()),
        Err(Error::Invalid(message)) if message.contains(&format!("has layout {other_layout}"))
    ));
}

/// The footer is the only fixed part of the file, and the module docs call it
/// part of the layout.
#[test]
fn a_store_ends_in_its_footer() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(1), dir.path());
    let bytes = fs::read(dir.path().join(store_file(1))).unwrap();
    assert_eq!(&bytes[bytes.len() - 4..], b"ESDE");
    // The header's offset and length point inside the file, its checksum is
    // of its bytes, and the header is the JSON the docs describe.
    let footer = bytes.len() - FOOTER_BYTES;
    let at = u64::from_le_bytes(bytes[footer..footer + 8].try_into().unwrap());
    let len = u32::from_le_bytes(bytes[footer + 8..footer + 12].try_into().unwrap());
    let crc = u32::from_le_bytes(bytes[footer + 12..footer + 16].try_into().unwrap());
    let at = usize::try_from(at).unwrap();
    let len = usize::try_from(len).unwrap();
    assert_eq!(at + len, footer);
    assert_eq!(crc, crc32(&bytes[at..at + len]));
    let header = std::str::from_utf8(&bytes[at..at + len]).unwrap();
    assert!(header.starts_with('{'), "{header}");
    assert!(header.contains(r#""name":"types""#), "{header}");
    assert!(header.contains(r#""key":"integer""#), "{header}");
    assert!(header.contains(r#""key":"text""#), "{header}");

    // A table's records are one contiguous JSON Lines run, in the order they
    // came out of the archive. `types` is not the first table in the file, so
    // look for the run rather than assuming where it starts.
    let types = format!("{TRITANIUM}\n{RIFTER}\n").into_bytes();
    assert!(
        bytes
            .windows(types.len())
            .any(|window| window == types.as_slice()),
        "the records of `types` are not a contiguous run"
    );
}

#[test]
fn a_file_that_is_not_a_store_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(store_file(1));

    fs::write(&path, b"").unwrap();
    assert!(matches!(Store::open(dir.path()), Err(Error::Invalid(_))));

    fs::write(&path, b"nowhere near a store, but long enough").unwrap();
    assert!(matches!(
        Store::open(dir.path()),
        Err(Error::Invalid(message)) if message.contains("does not end in `ESDE`")
    ));

    // A good store with its last byte of header lost.
    let good = tempfile::tempdir().unwrap();
    import(fixture(1), good.path());
    let mut bytes = fs::read(good.path().join(store_file(1))).unwrap();
    let cut = bytes.len() - FOOTER_BYTES - 1;
    bytes.remove(cut);
    fs::write(&path, &bytes).unwrap();
    assert!(matches!(Store::open(dir.path()), Err(Error::Invalid(_))));
}

#[test]
fn many_threads_read_one_store_at_once() {
    let (_dir, store) = imported(fixture(1));
    let store = Arc::new(store);
    let readers: Vec<_> = (0..8)
        .map(|_| {
            let store = Arc::clone(&store);
            thread::spawn(move || {
                for _ in 0..50 {
                    assert_eq!(
                        store.get::<Type>(TypeId(587)).unwrap().unwrap().name,
                        "Rifter"
                    );
                    assert_eq!(
                        store
                            .get::<CharacterTitle>("c3d4".to_owned())
                            .unwrap()
                            .unwrap()
                            .name,
                        "Scout"
                    );
                }
            })
        })
        .collect();
    for reader in readers {
        reader.join().unwrap();
    }
}

/// A store needs no lock, so it can be shared rather than opened per thread.
#[test]
fn a_store_is_send_and_sync() {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Store>();
}

/// Bytes in the footer, as the module docs describe it.
const FOOTER_BYTES: usize = 20;

/// Bytes per row of a key index, as the module docs describe it.
const ENTRY_BYTES: usize = 24;

/// The checksum the store uses, computed the way a reader of the layout
/// docs would.
fn crc32(bytes: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

/// Helpers that take a store apart, for the damage a reader has to survive.
/// The header is the JSON the footer points at, and the footer is rewritten
/// so that the file stays the length its header describes and keeps the
/// checksum its header needs.
fn header_of(path: &Path) -> (serde_json::Value, usize) {
    let bytes = fs::read(path).unwrap();
    let end = bytes.len() - FOOTER_BYTES;
    let at = u64::from_le_bytes(bytes[end..end + 8].try_into().unwrap());
    let len = u32::from_le_bytes(bytes[end + 8..end + 12].try_into().unwrap());
    let at = usize::try_from(at).unwrap();
    let len = usize::try_from(len).unwrap();
    (serde_json::from_slice(&bytes[at..at + len]).unwrap(), at)
}

fn rewrite_header(path: &Path, change: impl FnOnce(&mut serde_json::Value)) {
    let bytes = fs::read(path).unwrap();
    let (mut header, at) = header_of(path);
    change(&mut header);
    let json = serde_json::to_vec(&header).unwrap();
    let mut out = bytes[..at].to_vec();
    out.extend_from_slice(&json);
    out.extend_from_slice(&u64::try_from(at).unwrap().to_le_bytes());
    out.extend_from_slice(&u32::try_from(json.len()).unwrap().to_le_bytes());
    out.extend_from_slice(&crc32(&json).to_le_bytes());
    out.extend_from_slice(b"ESDE");
    fs::write(path, out).unwrap();
}

/// Where one table's key index is in the file.
fn key_index_of(path: &Path, table: &str) -> (usize, usize) {
    let (header, _) = header_of(path);
    let tables = header["tables"].as_array().unwrap().clone();
    let found = tables
        .iter()
        .find(|entry| entry["name"] == table)
        .unwrap()
        .clone();
    let at = usize::try_from(found["keys"]["at"].as_u64().unwrap()).unwrap();
    let len = usize::try_from(found["keys"]["len"].as_u64().unwrap()).unwrap();
    (at, len)
}

/// CCP changing a table's `_key` from a number to text is a change the
/// import survives, so the read has to say so rather than answer `None`.
#[test]
fn keys_of_the_other_shape_fail_rather_than_answer_none() {
    let text_keyed =
        r#"{"_key":"587","groupID":25,"name":{"en":"Rifter"},"portionSize":1,"published":true}"#;
    let (_dir, store) = imported(fixture(1).rows::<Type>(text_keyed));

    assert!(matches!(
        store.get::<Type>(TypeId(587)),
        Err(Error::Invalid(message))
            if message.contains("table `types`")
                && message.contains("has text keys")
                && message.contains("expects integer")
    ));
    assert!(matches!(
        store.get_raw::<Type>(TypeId(587)),
        Err(Error::Invalid(_))
    ));
    // The records are still there, whatever shape their keys are.
    let lines: Vec<String> = store
        .lines("types")
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(lines, vec![text_keyed.to_owned()]);
    // A table keyed by text is reached by text, and one keyed by numbers is
    // not reached by text either.
    assert!(matches!(
        store.get::<CharacterTitle>("a1b2".to_owned()),
        Ok(Some(_))
    ));
    assert!(matches!(
        store.get::<SolarSystem>(SystemId(30_000_142)),
        Ok(Some(_))
    ));
}

/// A store written before this crate had a model for a file holds no table
/// of that name. Answering `None` would say the SDE is empty.
#[test]
fn a_table_the_store_does_not_hold_fails_rather_than_answering_none() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(1), dir.path());
    let path = dir.path().join(store_file(1));
    rewrite_header(&path, |header| {
        let tables = header["tables"].as_array_mut().unwrap();
        tables.retain(|table| table["name"] != "types");
        header["eve_sde"] = serde_json::Value::String("0.1.0".to_owned());
    });

    let store = Store::open(dir.path()).unwrap();
    assert!(store.table("types").is_none());
    assert!(matches!(
        store.get::<Type>(TypeId(587)),
        Err(Error::Invalid(message))
            if message.contains("no table `types`") && message.contains("eve_sde 0.1.0")
    ));
    assert!(matches!(
        store.lines("types"),
        Err(Error::Invalid(message)) if message.contains("no table `types`")
    ));
    // Every other table still answers.
    assert!(
        store
            .get::<SolarSystem>(SystemId(30_000_142))
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_store_that_lost_or_gained_bytes_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(1), dir.path());
    let path = dir.path().join(store_file(1));
    let good = fs::read(&path).unwrap();

    let mut longer = good.clone();
    longer.insert(0, b'\n');
    fs::write(&path, &longer).unwrap();
    assert!(matches!(
        Store::open(dir.path()),
        Err(Error::Invalid(message)) if message.contains("bytes, not the")
    ));

    // Bytes lost from the middle, with the footer left alone.
    let mut shorter = good.clone();
    shorter.remove(0);
    fs::write(&path, &shorter).unwrap();
    assert!(matches!(
        Store::open(dir.path()),
        Err(Error::Invalid(message)) if message.contains("bytes, not the")
    ));

    fs::write(&path, &good).unwrap();
    assert!(Store::open(dir.path()).is_ok());
}

/// The header says where every record in the file is, so a change to it
/// that keeps the file's length passes every other check. Its checksum is
/// what catches that.
#[test]
fn a_header_that_does_not_match_its_checksum_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(1), dir.path());
    let path = dir.path().join(store_file(1));

    // Say that `character_titles` holds one row, not the two it holds, in
    // the bytes it took to say two, and leave the footer alone.
    let mut bytes = fs::read(&path).unwrap();
    let (_, at) = header_of(&path);
    let header = std::str::from_utf8(&bytes[at..bytes.len() - FOOTER_BYTES]).unwrap();
    let titles = header.find(r#""name":"character_titles""#).unwrap();
    let rows = titles + header[titles..].find(r#""rows":"#).unwrap() + r#""rows":"#.len();
    assert_eq!(header.as_bytes()[rows], b'2');
    bytes[at + rows] = b'1';
    fs::write(&path, &bytes).unwrap();
    assert!(matches!(
        Store::open(dir.path()),
        Err(Error::Invalid(message)) if message.contains("header does not match its checksum")
    ));

    // The same claim, in a store whose checksum was written with it, passes
    // every other check: the file is the length its footer describes, the
    // key index is 24 bytes a row, and the table has no other index to be
    // the wrong size. It opens, and the second title is gone without a
    // word. That is what the checksum is for.
    rewrite_header(&path, |header| {
        for table in header["tables"].as_array_mut().unwrap() {
            if table["name"] == "character_titles" {
                table["rows"] = 1.into();
                table["keys"]["len"] = ENTRY_BYTES.into();
            }
        }
    });
    let store = Store::open(dir.path()).unwrap();
    let title = |key: &str| store.get::<CharacterTitle>(key.to_owned()).unwrap();
    assert!(title("a1b2").is_some());
    assert!(title("c3d4").is_none());
}

#[test]
fn a_key_index_that_is_out_of_order_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(1), dir.path());
    let path = dir.path().join(store_file(1));
    let (at, len) = key_index_of(&path, "types");
    assert_eq!(len, 2 * ENTRY_BYTES, "two rows of one entry each");

    // Swap the two entries, which leaves every byte and offset in place but
    // puts the keys out of order, so a binary search would miss one.
    let mut bytes = fs::read(&path).unwrap();
    let (one, two) = (at, at + ENTRY_BYTES);
    let first: Vec<u8> = bytes[one..two].to_vec();
    let second: Vec<u8> = bytes[two..two + ENTRY_BYTES].to_vec();
    bytes[one..two].copy_from_slice(&second);
    bytes[two..two + ENTRY_BYTES].copy_from_slice(&first);
    fs::write(&path, &bytes).unwrap();

    let store = Store::open(dir.path()).unwrap();
    assert!(matches!(
        store.get::<Type>(TypeId(34)),
        Err(Error::Invalid(message))
            if message.contains("key index of table `types`") && message.contains("out of order")
    ));
    // Another table is unaffected.
    assert!(
        store
            .get::<SolarSystem>(SystemId(30_000_142))
            .unwrap()
            .is_some()
    );
}

/// A damaged store must not take a folder out of service, and must not block
/// the import that would replace it.
#[test]
fn a_damaged_store_is_skipped_and_then_replaced() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(2), dir.path());
    // An older store beside the newest, as one an import could not delete
    // would be. An import of an older build is refused, so copy it in.
    let older = tempfile::tempdir().unwrap();
    import(fixture(1), older.path());
    fs::copy(
        older.path().join(store_file(1)),
        dir.path().join(store_file(1)),
    )
    .unwrap();
    let newest = dir.path().join(store_file(2));
    fs::write(&newest, b"not a store any more, but long enough").unwrap();

    // The newest does not open, so the one before it answers.
    let store = Store::open(dir.path()).unwrap();
    assert_eq!(store.build().build_number, 1);
    drop(store);

    // And it is not counted as a build the folder holds.
    assert_eq!(import(fixture(2), dir.path()), ImportOutcome::Imported);
    let store = Store::open(dir.path()).unwrap();
    assert_eq!(store.build().build_number, 2);
    assert!(store.get::<Type>(TypeId(587)).unwrap().is_some());
    assert_eq!(file_names(dir.path()), vec![store_file(2)]);
}

#[test]
fn only_a_newer_build_is_refused_when_one_is_there() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(7), dir.path());
    assert_eq!(import(fixture(7), dir.path()), ImportOutcome::UpToDate);
    assert_eq!(import(fixture(6), dir.path()), ImportOutcome::OlderRefused);
    assert_eq!(import(fixture(8), dir.path()), ImportOutcome::Imported);
}

/// A build CCP publishes again under the same number is the one case that
/// needs a store replaced.
#[test]
fn reimport_replaces_the_store_of_a_build() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(3), dir.path());
    let first = fs::read(dir.path().join(store_file(3))).unwrap();

    let changed =
        r#"{"_key":587,"groupID":25,"name":{"en":"Rifter II"},"portionSize":1,"published":true}"#;
    let builder = fixture(3).rows::<Type>(&format!("{TRITANIUM}\n{changed}"));
    assert_eq!(import(builder, dir.path()), ImportOutcome::UpToDate);

    let builder = fixture(3).rows::<Type>(&format!("{TRITANIUM}\n{changed}"));
    store::reimport(&mut archive(builder), dir.path()).unwrap();
    assert_eq!(file_names(dir.path()), vec![store_file(3)]);
    assert_ne!(fs::read(dir.path().join(store_file(3))).unwrap(), first);

    let store = Store::open(dir.path()).unwrap();
    assert_eq!(
        store.get::<Type>(TypeId(587)).unwrap().unwrap().name,
        "Rifter II"
    );
    assert_eq!(store.table("types").unwrap().rows, 2);
}

#[test]
fn the_store_says_what_it_holds() {
    let dir = tempfile::tempdir().unwrap();
    let before = std::time::SystemTime::now();
    import(
        fixture(42).file("newThings.jsonl", r#"{"_key":1,"a":2}"#),
        dir.path(),
    );
    let path = dir.path().join(store_file(42));
    let store = Store::open(dir.path()).unwrap();

    assert_eq!(store.path(), path);
    assert_eq!(store.size(), fs::metadata(&path).unwrap().len());
    assert_eq!(store.written_by(), env!("CARGO_PKG_VERSION"));
    assert_eq!(store.build().build_number, 42);
    assert_eq!(
        store.build().release_date.as_deref(),
        Some("2026-10-02T11:08:57Z")
    );
    // Seconds since 1970, so allow for the second it was rounded down to.
    let imported = store.imported_at();
    assert!(
        imported + std::time::Duration::from_secs(1) >= before
            && imported <= std::time::SystemTime::now(),
        "{imported:?}"
    );

    let types = store.table("types").unwrap();
    assert_eq!(
        (types.name, types.file, types.rows, types.modeled),
        ("types", "types.jsonl", 2, true)
    );
    assert_eq!(store.table("solar_systems").unwrap().rows, 1);
    // A file with no model is a table of its own, named after the file.
    let new = store.table("new_things").unwrap();
    assert_eq!(
        (new.file, new.rows, new.modeled),
        ("newThings.jsonl", 1, false)
    );
    assert_eq!(store.unmodeled_files(), vec!["newThings.jsonl".to_owned()]);

    // Every table of the crate is in the store, in order, and the unmodeled
    // files come after them.
    let names: Vec<&str> = store.tables().map(|table| table.name).collect();
    let expected: Vec<&str> = eve_sde::tables().iter().map(|table| table.name).collect();
    assert_eq!(names[..expected.len()], expected[..]);
    assert_eq!(&names[expected.len()..], ["new_things"]);
    assert_eq!(store.tables().count(), expected.len() + 1);
}

#[test]
fn lines_read_a_whole_table_without_its_index() {
    let (_dir, store) = imported(fixture(1).file("newThings.jsonl", "{\"_key\":1}\n{\"_key\":2}"));

    let types: Vec<String> = store
        .lines("types")
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(types, vec![TRITANIUM.to_owned(), RIFTER.to_owned()]);

    // A file with no model, which nothing typed can reach.
    let new: Vec<String> = store
        .lines("new_things")
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        new,
        vec!["{\"_key\":1}".to_owned(), "{\"_key\":2}".to_owned()]
    );

    // An empty table, and one whose file the archive lacked.
    let (_dir, store) = imported(fixture(1).without("mapStars.jsonl"));
    assert_eq!(store.lines("stars").unwrap().count(), 0);
    assert!(matches!(
        store.lines("nothing_like_it"),
        Err(Error::Invalid(message)) if message.contains("no table `nothing_like_it`")
    ));
}

/// A record whose bytes were damaged after the import must not look like a
/// record this crate's models have fallen behind.
#[test]
fn a_damaged_record_is_told_from_a_model_that_has_fallen_behind() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(1), dir.path());
    let path = dir.path().join(store_file(1));

    // A good store verifies, and the Rifter reads back.
    let store = Store::open(dir.path()).unwrap();
    store.verify().unwrap();
    assert_eq!(
        store.get::<Type>(TypeId(587)).unwrap().unwrap().name,
        "Rifter"
    );
    drop(store);

    // Flip one byte of the Rifter's record, keeping the file's length.
    let mut bytes = fs::read(&path).unwrap();
    let at = bytes
        .windows(RIFTER.len())
        .position(|window| window == RIFTER.as_bytes())
        .unwrap();
    let digit = at + RIFTER.find("\"groupID\":25").unwrap() + r#""groupID":2"#.len();
    assert_eq!(bytes[digit], b'5');
    bytes[digit] = b'4';
    fs::write(&path, &bytes).unwrap();

    let store = Store::open(dir.path()).unwrap();
    // The record still parses — it is valid JSON for a different group — so
    // only the checksum knows, and `verify` is what asks.
    assert_eq!(
        store.get::<Type>(TypeId(587)).unwrap().unwrap().group_id,
        GroupId(24)
    );
    assert!(matches!(
        store.verify(),
        Err(Error::Invalid(message))
            if message.contains("is damaged")
                && message.contains("table `types` with key 587")
                && message.contains("checksum")
    ));

    // Damage that stops it parsing is reported as damage, not as a model
    // that does not fit.
    let mut bytes = fs::read(&path).unwrap();
    bytes[digit] = b'"';
    fs::write(&path, &bytes).unwrap();
    let store = Store::open(dir.path()).unwrap();
    assert!(matches!(
        store.get::<Type>(TypeId(587)),
        Err(Error::Invalid(message)) if message.contains("checksum")
    ));
    assert!(matches!(store.get_raw::<Type>(TypeId(587)), Ok(Some(_))));
    // And a record CCP changed, whose bytes are sound, is still `Decode`.
    let broken =
        r#"{"_key":587,"groupID":"25","name":{"en":"Rifter"},"portionSize":1,"published":true}"#;
    let (_dir, store) = imported(fixture(1).rows::<Type>(broken));
    store.verify().unwrap();
    assert!(matches!(
        store.get::<Type>(TypeId(587)),
        Err(Error::Decode { table, .. }) if table == "types"
    ));
}

/// Reading an older build because the newest is damaged must not be silent.
#[test]
fn a_skipped_store_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(2), dir.path());
    let older = tempfile::tempdir().unwrap();
    import(fixture(1), older.path());
    fs::copy(
        older.path().join(store_file(1)),
        dir.path().join(store_file(1)),
    )
    .unwrap();
    fs::write(
        dir.path().join(store_file(2)),
        b"not a store, but long enough",
    )
    .unwrap();

    let store = Store::open(dir.path()).unwrap();
    assert_eq!(store.build().build_number, 1);
    let skipped = store.skipped();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].build, 2);
    assert_eq!(skipped[0].path, dir.path().join(store_file(2)));
    assert!(
        matches!(&skipped[0].reason, Error::Invalid(message) if message.contains("ESDE")),
        "{:?}",
        skipped[0].reason
    );
    // An `Sde` says so too, so the warning is not lost by opening one.
    let sde = Sde::open_store(dir.path()).unwrap();
    assert_eq!(sde.build().build_number, 1);
    assert_eq!(sde.skipped().len(), 1);
    assert_eq!(sde.skipped()[0].build, 2);

    // A store opened by its path knows nothing of the folder.
    let only = Store::open_file(dir.path().join(store_file(1))).unwrap();
    assert!(only.skipped().is_empty());
    assert!(Sde::from(only).skipped().is_empty());
}

/// The station names are the one thing the import reads through the models,
/// so a record that does not fit has to leave them out and say why.
#[test]
fn stations_stay_unnamed_when_a_record_they_need_does_not_fit() {
    // As if CCP had changed the stars, which naming a station reads: this
    // star lacks fields its model requires.
    let stars = r#"{"_key":40000001,"typeID":6}"#;
    let (_dir, store) = imported(fixture(1).rows::<NpcStation>(STATION).rows::<Star>(stars));

    // The records are all there, and so is every other lookup.
    assert!(
        store
            .get::<NpcStation>(StationId(60_000_001))
            .unwrap()
            .is_some()
    );
    assert_eq!(store.table("npc_stations").unwrap().rows, 1);

    // But the station name searches say what went wrong.
    let sde = Sde::from(store);
    assert!(matches!(
        sde.station_id("Jita"),
        Err(Error::Invalid(message))
            if message.contains("no station names") && message.contains("`stars`: missing field")
    ));
    // And the names that do not need a station still answer.
    assert_eq!(sde.type_id("Rifter").unwrap(), Some(TypeId(587)));
}

/// A store written before this crate searched a table by name has no index
/// for it. Answering nothing would say the SDE holds no such name.
#[test]
fn an_index_the_store_does_not_hold_fails_rather_than_answering_nothing() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(1), dir.path());
    let path = dir.path().join(store_file(1));
    rewrite_header(&path, |header| {
        for table in header["tables"].as_array_mut().unwrap() {
            if table["name"] == "types" {
                let table = table.as_object_mut().unwrap();
                table.remove("names");
                table.remove("name_blob");
                table.remove("children");
            }
        }
        header["eve_sde"] = serde_json::Value::String("0.2.9".to_owned());
    });

    let sde = Sde::open_store(dir.path()).unwrap();
    assert!(matches!(
        sde.type_id("Rifter"),
        Err(Error::Invalid(message))
            if message.contains("no name index for table `types`")
                && message.contains("eve_sde 0.2.9")
    ));
    assert!(matches!(
        sde.types_in_group(GroupId(25)),
        Err(Error::Invalid(message))
            if message.contains("no children index for table `types`")
    ));
    // Everything else still answers, including records of that table.
    assert_eq!(sde.system_id("Jita").unwrap(), Some(SystemId(30_000_142)));
    assert_eq!(
        sde.get::<Type>(TypeId(587)).unwrap().unwrap().name,
        "Rifter"
    );
}
