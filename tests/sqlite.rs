//! The SQLite store, filled from synthetic archives.
#![cfg_attr(
    feature = "sqlite",
    expect(
        clippy::unwrap_used,
        reason = "test helpers fail by panicking, like the tests"
    )
)]
#![cfg(feature = "sqlite")]
mod common;

use common::file_names;
use eve_sde::ids::{GroupId, SystemId, TypeId};
use eve_sde::model::{CharacterTitle, NpcStation, SolarSystem, Star, Type};
use eve_sde::sqlite::{self, ImportOutcome, LAYOUT_VERSION, Store, rusqlite};
use eve_sde::testing::ArchiveBuilder;
use eve_sde::{Archive, Error, Sde};
use std::fs;
use std::io::Cursor;
use std::path::Path;

const TYPES: &str = r#"{"_key":34,"groupID":18,"name":{"en":"Tritanium","de":"Tritanium"},"portionSize":1,"published":true}
{"_key":587,"groupID":25,"name":{"en":"Rifter","ja":"リフター"},"portionSize":1,"published":true}"#;
const SYSTEMS: &str = r#"{"_key":30000142,"constellationID":20000020,"name":{"en":"Jita"},"position":{"x":0,"y":0,"z":0},"radius":1.0,"regionID":10000002,"securityStatus":0.9,"stargateIDs":[],"planetIDs":[]}"#;
const TITLES: &str = r#"{"_key":"a1b2","name":{"en":"Captain","fr":"Capitaine"}}"#;
const STATION: &str = r#"{"_key":60000001,"operationID":1,"orbitID":40000001,"ownerID":1000035,"position":{"x":0.0,"y":0.0,"z":0.0},"reprocessingEfficiency":0.5,"reprocessingHangarFlag":4,"reprocessingStationsTake":0.05,"solarSystemID":30000142,"typeID":1531,"useOperationName":false}"#;

fn fixture(build: u32) -> ArchiveBuilder {
    ArchiveBuilder::new(build)
        .release_date("2026-10-02T11:08:57Z")
        .rows::<Type>(TYPES)
        .rows::<SolarSystem>(SYSTEMS)
        .rows::<CharacterTitle>(TITLES)
}

fn archive(builder: ArchiveBuilder) -> Archive<Cursor<Vec<u8>>> {
    Archive::from_reader(Cursor::new(builder.into_bytes())).unwrap()
}

fn in_memory(builder: ArchiveBuilder) -> Store {
    Store::in_memory(&mut archive(builder)).unwrap()
}

fn import(builder: ArchiveBuilder, dir: &Path) -> ImportOutcome {
    sqlite::import(&mut archive(builder), dir).unwrap()
}

fn store_file(build: u32) -> String {
    format!("sde-{build}-v{LAYOUT_VERSION}.sqlite")
}

#[test]
fn records_read_back_through_their_models() {
    let store = in_memory(fixture(1));
    assert_eq!(store.build().build_number, 1);
    assert_eq!(
        store.build().release_date.as_deref(),
        Some("2026-10-02T11:08:57Z")
    );
    let rifter = store.get::<Type>(TypeId(587)).unwrap().unwrap();
    assert_eq!(rifter.name, "Rifter");
    assert_eq!(rifter.group_id, GroupId(25));
    let jita = store
        .get::<SolarSystem>(SystemId(30_000_142))
        .unwrap()
        .unwrap();
    assert_eq!(jita.name, "Jita");
    let title = store
        .get::<CharacterTitle>("a1b2".to_owned())
        .unwrap()
        .unwrap();
    assert_eq!(title.name, "Captain");
    assert!(store.get::<Type>(&TypeId(1)).unwrap().is_none());
}

#[test]
fn data_keeps_only_english_text() {
    let store = in_memory(fixture(1));
    let data: String = store
        .connection()
        .query_row("SELECT data FROM types WHERE key = 587", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        data,
        r#"{"_key":587,"groupID":25,"name":{"en":"Rifter"},"portionSize":1,"published":true}"#
    );
}

#[test]
fn sql_reaches_names_and_every_field() {
    let store = in_memory(fixture(1));
    let connection = store.connection();
    let key: i64 = connection
        .query_row(
            "SELECT key FROM types WHERE name_key = 'rifter'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(key, 587);
    let plan = |sql: &str| -> String {
        connection
            .query_row(&format!("EXPLAIN QUERY PLAN {sql}"), [], |row| row.get(3))
            .unwrap()
    };
    let by_name = plan("SELECT key FROM types WHERE name_key = 'rifter'");
    assert!(by_name.contains("types_by_name"), "{by_name}");
    let by_group = plan("SELECT key FROM types WHERE json_extract(data, '$.groupID') = 18");
    assert!(by_group.contains("types_by_groupID"), "{by_group}");
    let key: i64 = connection
        .query_row(
            "SELECT key FROM types WHERE data ->> '$.groupID' = 18",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(key, 34);
}

#[test]
fn stations_get_the_names_the_game_shows() {
    let store = in_memory(fixture(1).rows::<NpcStation>(STATION));
    let (name, key): (String, String) = store
        .connection()
        .query_row(
            "SELECT name, name_key FROM npc_stations WHERE key = 60000001",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    // Neither the star nor the owner is in the fixture.
    assert_eq!(name, "Jita");
    assert_eq!(key, "jita");
    let error: Option<String> = store
        .connection()
        .query_row("SELECT station_names_error FROM sde_state", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(error, None);
}

#[test]
fn stations_stay_unnamed_when_a_record_they_need_does_not_fit() {
    // As if CCP had changed the stars, which naming a station reads: this
    // star lacks fields its model requires.
    let stars = r#"{"_key":40000001,"typeID":6}"#;
    let store = in_memory(fixture(1).rows::<NpcStation>(STATION).rows::<Star>(stars));
    let name: Option<String> = store
        .connection()
        .query_row(
            "SELECT name FROM npc_stations WHERE key = 60000001",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(name, None);
    let error: String = store
        .connection()
        .query_row("SELECT station_names_error FROM sde_state", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(error.starts_with("cannot parse record"), "{error}");
    assert!(error.contains("`stars`: missing field"), "{error}");
    // The rest of the store reads as usual.
    let rifter = store.get::<Type>(TypeId(587)).unwrap().unwrap();
    assert_eq!(rifter.name, "Rifter");
}

#[test]
fn files_without_a_model_become_tables() {
    let store = in_memory(fixture(1).file(
        "newThings.jsonl",
        r#"{"_key":7,"name":{"en":"Seven","de":"Sieben"},"size":3}"#,
    ));
    let connection = store.connection();
    let (name, data): (String, String) = connection
        .query_row(
            "SELECT name, data FROM new_things WHERE key = 7",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(name, "Seven");
    assert_eq!(data, r#"{"_key":7,"name":{"en":"Seven"},"size":3}"#);
    let (file, modeled, rows): (String, bool, i64) = connection
        .query_row(
            "SELECT file, modeled, rows FROM sde_tables WHERE name = 'new_things'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (file.as_str(), modeled, rows),
        ("newThings.jsonl", false, 1)
    );
}

#[test]
fn the_store_describes_itself() {
    let store = in_memory(fixture(3));
    let connection = store.connection();
    let (build, layout, version): (u32, u32, String) = connection
        .query_row("SELECT build, layout, eve_sde FROM sde_state", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap();
    assert_eq!(
        (build, layout, version.as_str()),
        (3, LAYOUT_VERSION, env!("CARGO_PKG_VERSION"))
    );
    let tables: i64 = connection
        .query_row("SELECT count(*) FROM sde_tables WHERE modeled", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(tables, i64::try_from(eve_sde::tables().len()).unwrap());
    let rows: i64 = connection
        .query_row(
            "SELECT rows FROM sde_tables WHERE name = 'types'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rows, 2);
}

#[test]
fn a_record_its_model_cannot_read_still_imports() {
    // As if CCP had dropped `groupID`, which the model requires.
    let types = r#"{"_key":587,"name":{"en":"Rifter"},"portionSize":1,"published":true}"#;
    let store = in_memory(ArchiveBuilder::new(1).rows::<Type>(types));
    let name: String = store
        .connection()
        .query_row("SELECT name FROM types WHERE key = 587", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(name, "Rifter");
    assert!(matches!(
        store.get::<Type>(TypeId(587)),
        Err(Error::Decode { table, key, .. }) if table == "types" && key == "TypeId(587)"
    ));
}

#[test]
fn a_missing_file_leaves_its_table_empty() {
    let store = in_memory(ArchiveBuilder::new(1).without("types.jsonl"));
    assert!(store.get::<Type>(TypeId(587)).unwrap().is_none());
}

#[test]
fn a_repeated_key_is_an_error() {
    let row = r#"{"_key":1,"groupID":1,"name":{"en":"x"},"portionSize":1,"published":true}"#;
    let builder = ArchiveBuilder::new(1).rows::<Type>(format!("{row}\n{row}"));
    assert!(matches!(
        Store::in_memory(&mut archive(builder)),
        Err(Error::DuplicateKey { file, key }) if file == "types.jsonl" && key == "1"
    ));
}

#[test]
fn a_line_without_a_key_is_an_error() {
    let builder = ArchiveBuilder::new(1).rows::<Type>(r#"{"name":{"en":"x"}}"#);
    assert!(matches!(
        Store::in_memory(&mut archive(builder)),
        Err(Error::Invalid(message)) if message.contains("`types.jsonl` line 1")
    ));
}

#[test]
fn a_file_without_a_model_cannot_take_a_used_name() {
    // `stargates` is the table of `mapStargates.jsonl`, and `sde_state` is
    // one of the store's own two tables. Neither name can be taken, because
    // `get` for them would read this file's records; and CCP shipping such a
    // file must not fail the whole import.
    let store = in_memory(
        fixture(1)
            .file("stargates.jsonl", r#"{"_key":1}"#)
            .file("sdeState.jsonl", r#"{"_key":1}"#)
            .file("newThings.jsonl", r#"{"_key":1}"#),
    );
    assert_eq!(
        store.refused_files(),
        vec!["sdeState.jsonl".to_owned(), "stargates.jsonl".to_owned()]
    );
    // The table whose name was wanted is untouched.
    assert_eq!(
        store.get::<Type>(TypeId(587)).unwrap().unwrap().name,
        "Rifter"
    );
    // The docs call the column a JSON array, so plain SQL reaches it too.
    let sql: Vec<String> = store
        .connection()
        .prepare("SELECT value FROM sde_state, json_each(refused_files) ORDER BY value")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(sql, store.refused_files());
    // A file whose name is free is still a table of its own, and a store
    // with nothing refused says so.
    assert_eq!(
        Sde::from(store).unmodeled_files(),
        vec!["newThings.jsonl".to_owned()]
    );
    assert_eq!(in_memory(fixture(1)).refused_files(), Vec::<String>::new());
}

#[test]
fn each_build_gets_a_file_and_open_takes_the_newest() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(import(fixture(5), dir.path()), ImportOutcome::Imported);
    assert_eq!(file_names(dir.path()), [store_file(5)]);
    assert_eq!(import(fixture(5), dir.path()), ImportOutcome::UpToDate);
    assert_eq!(import(fixture(4), dir.path()), ImportOutcome::OlderRefused);

    let old = Store::open(dir.path()).unwrap();
    assert_eq!(import(fixture(6), dir.path()), ImportOutcome::Imported);
    assert!(file_names(dir.path()).contains(&store_file(6)));
    // Windows cannot delete a file that is open, so only other systems
    // delete build 5 here.
    #[cfg(not(windows))]
    assert_eq!(file_names(dir.path()), [store_file(6)]);
    // A reader of the old build keeps working.
    let rifter = old.get::<Type>(TypeId(587)).unwrap().unwrap();
    assert_eq!(rifter.name, "Rifter");
    assert_eq!(Store::open(dir.path()).unwrap().build().build_number, 6);
}

#[test]
fn other_layouts_are_ignored_and_older_ones_removed() {
    let dir = tempfile::tempdir().unwrap();
    let older = format!("sde-9-v{}.sqlite", LAYOUT_VERSION - 1);
    let newer = format!("sde-9-v{}.sqlite", LAYOUT_VERSION + 1);
    fs::write(dir.path().join(&older), "older layout").unwrap();
    fs::write(dir.path().join(&newer), "newer layout").unwrap();
    assert!(matches!(
        Store::open(dir.path()),
        Err(Error::NoStore { .. })
    ));
    assert_eq!(import(fixture(3), dir.path()), ImportOutcome::Imported);
    assert_eq!(file_names(dir.path()), [store_file(3), newer]);
}

#[test]
fn a_failed_import_leaves_the_folder_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    import(fixture(1), dir.path());
    let broken = ArchiveBuilder::new(2).rows::<Type>(r#"{"name":{"en":"x"}}"#);
    assert!(sqlite::import(&mut archive(broken), dir.path()).is_err());
    assert_eq!(file_names(dir.path()), [store_file(1)]);
    assert_eq!(Store::open(dir.path()).unwrap().build().build_number, 1);
}

#[test]
fn open_without_a_store_says_so() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Store::open(dir.path()),
        Err(Error::NoStore { dir: d }) if d == dir.path()
    ));
    assert!(matches!(
        Store::open(dir.path().join("missing")),
        Err(Error::NoStore { .. })
    ));
}

#[test]
fn import_creates_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("a/b");
    assert_eq!(import(fixture(1), &nested), ImportOutcome::Imported);
    assert_eq!(Store::open(&nested).unwrap().build().build_number, 1);
}

#[test]
fn a_store_can_move_to_another_thread() {
    fn send<T: Send>() {}
    send::<Store>();
}

/// `copy_table` types the `key` column from the data, so a table whose
/// `_key` CCP changed must fail the lookup rather than find no row.
#[test]
fn keys_of_the_other_shape_fail_rather_than_answer_none() {
    // `types` keyed by text, as if CCP had changed `_key`.
    let text_keyed =
        r#"{"_key":"587","groupID":25,"name":{"en":"Rifter"},"portionSize":1,"published":true}"#;
    let store = in_memory(fixture(1).rows::<Type>(text_keyed));
    assert!(matches!(
        store.get::<Type>(TypeId(587)),
        Err(Error::Invalid(message))
            if message.contains("table `types`")
                && message.contains("has text keys")
                && message.contains("expects integer")
    ));

    // `character_titles` keyed by numbers, the direction that used to answer
    // `None`: the parameter is text and the column is an integer.
    let number_keyed = r#"{"_key":12,"name":{"en":"Captain"}}"#;
    let store = in_memory(fixture(1).rows::<CharacterTitle>(number_keyed));
    assert!(matches!(
        store.get::<CharacterTitle>("a1b2".to_owned()),
        Err(Error::Invalid(message))
            if message.contains("table `character_titles`")
                && message.contains("has integer keys")
                && message.contains("expects text")
    ));

    // Tables whose keys are as their models say still answer.
    let store = in_memory(fixture(1));
    assert!(store.get::<Type>(TypeId(587)).unwrap().is_some());
    assert!(store.get::<Type>(TypeId(1)).unwrap().is_none());
    assert!(
        store
            .get::<CharacterTitle>("a1b2".to_owned())
            .unwrap()
            .is_some()
    );
    // An empty table is typed from its model, so it answers `None`.
    assert!(
        store
            .get::<Star>(eve_sde::ids::CelestialId(40_000_001))
            .unwrap()
            .is_none()
    );
}
