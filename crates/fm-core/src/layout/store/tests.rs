use std::fs;

use fm_api::ErrorCode;
use fm_api::layout::{BlockAlign, Layout, LayoutPreset};

use super::*;
use crate::test_support::TempDir;

/// A store over a layouts folder that does not exist yet.
fn store_in(dir: &TempDir) -> (LayoutStore, PathBuf) {
    let folder = dir.path().join("config").join("layouts");
    (LayoutStore::new(folder.clone()), folder)
}

fn ids(store: &LayoutStore) -> Vec<String> {
    store
        .list()
        .expect("list")
        .into_iter()
        .map(|e| e.id)
        .collect()
}

fn edited(mut layout: Layout, size: f32) -> Layout {
    layout.blocks[0].size_pt = size;
    layout
}

const PRESET_IDS: [&str; 6] = [
    "book",
    "columns",
    "magazine",
    "quote",
    "characters",
    "waterfall",
];

#[test]
fn the_six_presets_are_written_on_first_use_as_json_files() {
    let dir = TempDir::new("layouts-seed");
    let (store, folder) = store_in(&dir);
    assert!(!folder.exists(), "nothing is written before first use");
    assert_eq!(ids(&store), PRESET_IDS);
    for p in PRESETS {
        let text = fs::read_to_string(folder.join(format!("{}.json", preset_id(p)))).expect("file");
        let on_disk: Layout = serde_json::from_str(&text).expect("plain JSON layout");
        assert_eq!(on_disk, preset(p));
    }
}

#[test]
fn a_preset_file_is_never_overwritten_and_a_deleted_one_stays_deleted() {
    let dir = TempDir::new("layouts-no-overwrite");
    let (store, folder) = store_in(&dir);
    store.list().expect("seed");
    let book = folder.join("book.json");
    let mine = edited(preset(LayoutPreset::Book), 40.0);
    fs::write(&book, serde_json::to_string(&mine).expect("json")).expect("hand edit");
    fs::remove_file(folder.join("quote.json")).expect("delete");

    // The next app start (a new store) seeds again: nothing is replaced or brought back.
    let again = LayoutStore::new(folder.clone());
    let listed = ids(&again);
    assert!(!listed.contains(&"quote".to_owned()), "{listed:?}");
    assert_eq!(again.get("book").expect("book").layout, mine);
    assert_eq!(
        serde_json::from_str::<Layout>(&fs::read_to_string(&book).expect("read")).expect("json"),
        mine
    );
}

#[test]
fn a_hand_edit_is_picked_up_the_next_time_layouts_are_listed() {
    let dir = TempDir::new("layouts-hand-edit");
    let (store, folder) = store_in(&dir);
    store.list().expect("seed");
    let mut l = preset(LayoutPreset::Columns);
    l.name = "Newspaper".into();
    l.blocks[1].align = BlockAlign::End;
    fs::write(
        folder.join("columns.json"),
        serde_json::to_string(&l).expect("json"),
    )
    .expect("edit");
    let entry = store
        .list()
        .expect("list")
        .into_iter()
        .find(|e| e.id == "columns")
        .expect("columns");
    assert_eq!(entry.name, "Newspaper");
    assert_eq!(entry.broken, None);
    assert_eq!(store.get("columns").expect("get").layout, l);

    // A file a user drops in is listed too, after the presets.
    let mut own = preset(LayoutPreset::Quote);
    own.preset = None;
    own.name = "Aardvark".into();
    fs::write(
        folder.join("aardvark.json"),
        serde_json::to_string(&own).expect("json"),
    )
    .expect("drop in");
    let listed = ids(&store);
    assert_eq!(listed.last().map(String::as_str), Some("aardvark"));
}

#[test]
fn a_hand_edited_value_out_of_range_is_clamped_when_read() {
    let dir = TempDir::new("layouts-clamp-read");
    let (store, folder) = store_in(&dir);
    store.list().expect("seed");
    fs::write(
        folder.join("book.json"),
        serde_json::to_string(&edited(preset(LayoutPreset::Book), 9000.0)).expect("json"),
    )
    .expect("edit");
    assert_eq!(
        store.get("book").expect("get").layout.blocks[0].size_pt,
        600.0
    );
}

#[test]
fn a_malformed_file_is_listed_as_broken_and_the_others_still_load() {
    let dir = TempDir::new("layouts-broken");
    let (store, folder) = store_in(&dir);
    store.list().expect("seed");
    fs::write(folder.join("book.json"), "{ \"name\": \"Book\", ").expect("truncated");
    fs::write(folder.join("odd.json"), r#"{"name": "Odd", "blocks": 3}"#).expect("not a layout");
    fs::write(folder.join("bytes.json"), [0xff, 0xfe, 0x00]).expect("not text");

    let entries = store.list().expect("list never fails on one file");
    assert_eq!(entries.len(), 8);
    let by_id = |id: &str| entries.iter().find(|e| e.id == id).expect(id).clone();
    let book = by_id("book");
    assert!(
        book.broken
            .as_deref()
            .is_some_and(|r| r.starts_with("Not valid JSON")),
        "{book:?}"
    );
    assert_eq!(book.preset, Some(LayoutPreset::Book));
    assert!(
        by_id("odd")
            .broken
            .as_deref()
            .is_some_and(|r| r.starts_with("Not a layout"))
    );
    assert!(
        by_id("bytes")
            .broken
            .as_deref()
            .is_some_and(|r| r.starts_with("Not UTF-8"))
    );
    for id in &PRESET_IDS[1..] {
        assert_eq!(by_id(id).broken, None, "{id}");
    }
    let err = store.get("book").expect_err("broken");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    assert!(
        err.detail
            .as_deref()
            .is_some_and(|d| d.contains("Not valid JSON"))
    );
    // Listing and reading never rewrite the broken file.
    assert_eq!(
        fs::read_to_string(folder.join("book.json")).expect("read"),
        "{ \"name\": \"Book\", "
    );
}

#[test]
fn every_save_keeps_the_previous_content_as_a_version_newest_first() {
    let dir = TempDir::new("layouts-versions");
    let (store, _) = store_in(&dir);
    let original = store.get("book").expect("book").layout;
    assert!(store.history("book").expect("history").is_empty());

    store
        .save("book", edited(original.clone(), 20.0))
        .expect("save 1");
    store
        .save("book", edited(original.clone(), 30.0))
        .expect("save 2");
    let history = store.history("book").expect("history");
    assert_eq!(history.iter().map(|v| v.number).collect::<Vec<_>>(), [2, 1]);
    assert!(
        history
            .iter()
            .all(|v| v.name.as_deref() == Some("Book") && v.broken.is_none())
    );
    assert!(history[0].saved_at.ends_with('Z') && history[0].saved_at.len() == 20);

    // A save that changes nothing keeps no copy of itself.
    store
        .save("book", edited(original, 30.0))
        .expect("same again");
    assert_eq!(store.history("book").expect("history").len(), 2);
}

#[test]
fn versions_are_capped_and_the_oldest_go_first() {
    let dir = TempDir::new("layouts-cap");
    let (store, _) = store_in(&dir);
    let base = store.get("quote").expect("quote").layout;
    let saves = MAX_VERSIONS + 5;
    for i in 0..saves {
        store
            .save("quote", edited(base.clone(), 20.0 + i as f32))
            .expect("save");
    }
    let numbers: Vec<u32> = store
        .history("quote")
        .expect("history")
        .into_iter()
        .map(|v| v.number)
        .collect();
    assert_eq!(numbers.len(), MAX_VERSIONS);
    let newest = u32::try_from(saves).expect("small");
    assert_eq!(numbers[0], newest);
    assert_eq!(
        *numbers.last().expect("oldest"),
        newest + 1 - MAX_VERSIONS as u32
    );
}

#[test]
fn restoring_a_version_makes_it_current_and_keeps_what_it_replaced() {
    let dir = TempDir::new("layouts-restore");
    let (store, _) = store_in(&dir);
    let base = store.get("magazine").expect("magazine").layout;
    store
        .save("magazine", edited(base.clone(), 20.0))
        .expect("save 1"); // keeps v1 = base
    store
        .save("magazine", edited(base.clone(), 30.0))
        .expect("save 2"); // keeps v2 = 20pt

    let restored = store.restore("magazine", 1).expect("restore");
    assert_eq!(restored.layout, base);
    assert_eq!(store.get("magazine").expect("get").layout, base);
    let history = store.history("magazine").expect("history");
    assert_eq!(
        history.iter().map(|v| v.number).collect::<Vec<_>>(),
        [3, 2, 1]
    );

    // v3 is the 30 pt content the restore replaced; v1 stays restorable.
    let back = store
        .restore("magazine", 3)
        .expect("restore the replaced one");
    assert_eq!(back.layout.blocks[0].size_pt, 30.0);
    assert_eq!(store.restore("magazine", 1).expect("again").layout, base);
    assert_eq!(
        store.restore("magazine", 99).expect_err("gone").code,
        ErrorCode::NotFound
    );
}

#[test]
fn saving_over_a_broken_file_keeps_the_broken_text_as_a_version() {
    let dir = TempDir::new("layouts-save-broken");
    let (store, folder) = store_in(&dir);
    store.list().expect("seed");
    fs::write(folder.join("book.json"), "oops").expect("break");
    store
        .save("book", preset(LayoutPreset::Book))
        .expect("save over");
    let history = store.history("book").expect("history");
    assert_eq!(history.len(), 1);
    assert!(history[0].broken.is_some());
    assert_eq!(history[0].name, None);
    let err = store
        .restore("book", history[0].number)
        .expect_err("broken");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
}

#[test]
fn duplicate_makes_an_independent_copy_with_a_new_name() {
    let dir = TempDir::new("layouts-duplicate");
    let (store, _) = store_in(&dir);
    let book = store.get("book").expect("book").layout;

    let copy = store.duplicate("book", None).expect("copy");
    assert_eq!(copy.id, "book-copy");
    assert_eq!(copy.layout.name, "Book copy");
    assert_eq!(copy.layout.blocks, book.blocks);
    assert_eq!(
        copy.layout.preset,
        Some(LayoutPreset::Book),
        "remembers where it started"
    );
    let second = store.duplicate("book", None).expect("second");
    assert_eq!(
        (second.id.as_str(), second.layout.name.as_str()),
        ("book-copy-2", "Book copy 2")
    );

    // Editing the copy leaves the original and its history alone.
    store
        .save("book-copy", edited(copy.layout, 50.0))
        .expect("edit copy");
    assert_eq!(store.get("book").expect("book").layout, book);
    assert!(store.history("book").expect("history").is_empty());
    assert_eq!(store.history("book-copy").expect("history").len(), 1);

    let named = store
        .duplicate("quote", Some("  Zitat für Ränder "))
        .expect("named");
    assert_eq!(named.layout.name, "Zitat für Ränder");
    assert_eq!(named.id, "zitat-für-ränder");
    assert!(ids(&store).contains(&named.id));
    assert_eq!(
        store
            .duplicate("quote", Some("book COPY"))
            .expect_err("taken")
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(
        store.duplicate("nope", None).expect_err("missing").code,
        ErrorCode::NotFound
    );
}

#[test]
fn revert_reads_the_saved_file_and_a_preset_resets_to_its_shipped_original() {
    let dir = TempDir::new("layouts-revert");
    let (store, folder) = store_in(&dir);
    let saved = store
        .save("book", edited(preset(LayoutPreset::Book), 40.0))
        .expect("save");
    // The editor's unsaved changes are not in the file: revert gives the saved one.
    assert_eq!(
        store.revert("book", false).expect("revert").layout,
        saved.layout
    );

    let reset = store.revert("book", true).expect("reset");
    assert_eq!(reset.layout, preset(LayoutPreset::Book));
    assert_eq!(
        store.get("book").expect("get").layout,
        preset(LayoutPreset::Book)
    );
    // The reset is a save: the 40 pt layout is kept and can come back.
    let newest = store.history("book").expect("history")[0].number;
    assert_eq!(
        store.restore("book", newest).expect("undo reset").layout,
        saved.layout
    );

    // A copy resets to its preset's blocks and keeps its own name.
    let copy = store.duplicate("waterfall", None).expect("copy");
    store
        .save(&copy.id, edited(copy.layout, 99.0))
        .expect("edit");
    let reset = store.revert(&copy.id, true).expect("reset copy");
    assert_eq!(reset.layout.name, "Waterfall copy");
    assert_eq!(reset.layout.blocks, preset(LayoutPreset::Waterfall).blocks);

    // A broken or deleted preset file resets too.
    fs::write(folder.join("quote.json"), "{").expect("break");
    assert_eq!(
        store.revert("quote", true).expect("broken").layout,
        preset(LayoutPreset::Quote)
    );
    fs::remove_file(folder.join("columns.json")).expect("delete");
    assert_eq!(
        store.revert("columns", true).expect("deleted").layout,
        preset(LayoutPreset::Columns)
    );

    // A layout of the user's own has no original.
    let mut own = preset(LayoutPreset::Book);
    own.preset = None;
    own.name = "Own".into();
    store.save("own", own).expect("own");
    assert_eq!(
        store.revert("own", true).expect_err("no preset").code,
        ErrorCode::InvalidRequest
    );
}

#[test]
fn ids_that_could_leave_the_folder_or_break_on_an_os_are_refused() {
    let dir = TempDir::new("layouts-ids");
    let (store, _) = store_in(&dir);
    for id in [
        "", "../evil", "a/b", r"a\b", ".history", "con", "COM1", "x.", "x ", "a:b",
    ] {
        assert!(!is_valid_id(id), "{id:?}");
        let err = store.save(id, preset(LayoutPreset::Book)).expect_err(id);
        assert_eq!(err.code, ErrorCode::InvalidRequest, "{id:?}");
        assert_eq!(store.get(id).expect_err(id).code, ErrorCode::InvalidRequest);
    }
    assert!(!dir.path().join("evil.json").exists());
    assert_eq!(
        store.get("missing").expect_err("missing").code,
        ErrorCode::NotFound
    );
    assert_eq!(
        store.history("missing").expect_err("missing").code,
        ErrorCode::NotFound
    );
    let mut nameless = preset(LayoutPreset::Book);
    nameless.name = "  ".into();
    assert_eq!(
        store.save("book", nameless).expect_err("no name").code,
        ErrorCode::InvalidRequest
    );
}

#[test]
fn slugs_are_file_safe_ids() {
    assert_eq!(slug("Book copy 2"), "book-copy-2");
    assert_eq!(slug("  --Hello,  World!  "), "hello-world");
    assert_eq!(slug("字体 布局"), "字体-布局");
    assert_eq!(slug("!!!"), "layout");
    assert_eq!(slug("CON"), "con-layout");
    assert!(is_valid_id(&slug(&"x".repeat(500))));
}

#[test]
fn utc_timestamps_are_formatted_from_unix_milliseconds() {
    assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(iso_utc(951_782_400_000), "2000-02-29T00:00:00Z");
    assert_eq!(iso_utc(1_792_576_245_999), "2026-10-21T09:50:45Z");
}
