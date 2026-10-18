use super::*;

fn meta(favorite: bool, rating: Option<u8>, notes: Option<&str>, tags: &[&str]) -> Meta {
    Meta {
        favorite,
        rating,
        notes: notes.map(Into::into),
        tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        collections: Vec::new(),
    }
}

fn path(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

#[test]
fn file_name_is_sixteen_hex_digits_of_the_grouped_family() {
    let name = file_name("Source Serif");
    assert_eq!(name.len(), 16 + ".json".len());
    assert!(name[..16].chars().all(|c| c.is_ascii_hexdigit()));
    // The library groups "source  serif" and "Source Serif" as one family.
    assert_eq!(file_name("source  serif"), name);
    assert_ne!(file_name("Source Sans"), name);
}

#[test]
fn the_side_that_changed_wins() {
    let base = meta(false, Some(3), Some("old"), &[]);
    let local = meta(true, Some(3), Some("old"), &[]);
    let disk = meta(false, Some(5), Some("new"), &[]);
    let merged = merge(&base, &local, &disk);
    assert_eq!(merged.meta, meta(true, Some(5), Some("new"), &[]));
    assert!(merged.conflicts.is_empty());
}

#[test]
fn a_field_both_sides_changed_keeps_the_local_value_and_reports_the_other() {
    let base = meta(false, Some(3), Some("old"), &[]);
    let local = meta(false, Some(4), Some("mine"), &[]);
    let disk = meta(false, Some(2), Some("theirs"), &[]);
    let merged = merge(&base, &local, &disk);
    assert_eq!(merged.meta.rating, Some(4));
    assert_eq!(merged.meta.notes.as_deref(), Some("mine"));
    assert_eq!(
        merged.conflicts,
        [
            Conflict {
                field: Field::Rating,
                local: "4".into(),
                disk: "2".into(),
            },
            Conflict {
                field: Field::Notes,
                local: "\"mine\"".into(),
                disk: "\"theirs\"".into(),
            },
        ]
    );
}

#[test]
fn the_same_change_on_both_sides_is_no_conflict() {
    let base = meta(false, None, None, &[]);
    let both = meta(true, Some(4), Some("same"), &[]);
    let merged = merge(&base, &both, &both);
    assert_eq!(merged.meta, both);
    assert!(merged.conflicts.is_empty());
}

#[test]
fn without_a_base_set_values_are_kept_and_different_ones_conflict() {
    let empty = Meta::default();
    let local = meta(true, None, Some("mine"), &[]);
    let disk = meta(false, Some(5), Some("theirs"), &[]);
    let merged = merge(&empty, &local, &disk);
    assert!(merged.meta.favorite);
    assert_eq!(merged.meta.rating, Some(5));
    assert_eq!(merged.meta.notes.as_deref(), Some("mine"));
    assert_eq!(merged.conflicts.len(), 1);
    assert_eq!(merged.conflicts[0].field, Field::Notes);
}

#[test]
fn tags_merge_as_sets_ignoring_case() {
    let base = meta(false, None, None, &["Display", "old"]);
    // Local removed "old" and added "serif"; disk added "body" and wrote
    // "display" in another case.
    let local = meta(false, None, None, &["Display", "serif"]);
    let disk = meta(false, None, None, &["display", "old", "body"]);
    let merged = merge(&base, &local, &disk);
    assert_eq!(merged.meta.tags, ["body", "Display", "serif"]);
    assert!(merged.conflicts.is_empty());
}

#[test]
fn a_removal_on_one_side_wins_over_no_change_on_the_other() {
    let base = Meta {
        collections: vec![path(&["Work"]), path(&["Work", "Client"])],
        ..Meta::default()
    };
    let local = base.clone();
    let mut disk = base.clone();
    disk.collections = vec![path(&["Work"]), path(&["Personal"])];
    let merged = merge(&base, &local, &disk);
    assert_eq!(
        merged.meta.collections,
        [path(&["Personal"]), path(&["Work"])]
    );
}

#[test]
fn without_a_base_sets_are_united() {
    let local = meta(false, None, None, &["a"]);
    let disk = meta(false, None, None, &["b"]);
    let merged = merge(&Meta::default(), &local, &disk);
    assert_eq!(merged.meta.tags, ["a", "b"]);
}

#[test]
fn normalize_drops_what_the_index_would_not_store() {
    let mut m = Meta {
        favorite: false,
        rating: Some(9),
        notes: Some(String::new()),
        tags: vec![" x ".into(), "X".into(), " ".into()],
        collections: vec![vec![], path(&["A", " "]), path(&["B"]), path(&["B"])],
    };
    m.normalize();
    assert_eq!(m.rating, None);
    assert_eq!(m.notes, None);
    assert_eq!(m.tags, ["X"]);
    assert_eq!(m.collections, [path(&["B"])]);
}

#[test]
fn parse_reads_what_to_json_writes() {
    let mut sidecar = Sidecar::new("Inter");
    sidecar.faces.push(FaceEntry {
        file: "sub/Inter-Regular.ttf".into(),
        index: 0,
        postscript_name: Some("Inter-Regular".into()),
        meta: meta(true, Some(4), Some("ünïcode notes"), &["display"]),
        history: vec!["0123456789abcdef".into()],
    });
    sidecar.normalize();
    assert_eq!(parse(&sidecar.to_json()), Parsed::Sidecar(sidecar));
}

#[test]
fn malformed_and_newer_sidecars_are_not_parsed_as_sidecars() {
    assert!(matches!(parse("{ not json"), Parsed::Unreadable(_)));
    assert!(matches!(parse("[]"), Parsed::Unreadable(_)));
    assert!(matches!(
        parse(r#"{"version": 1, "faces": [{"index": 0}]}"#),
        Parsed::Unreadable(_)
    ));
    assert_eq!(
        parse(r#"{"version": 2, "faces": "anything"}"#),
        Parsed::Newer(2)
    );
}

#[test]
fn unknown_fields_of_the_same_version_are_read() {
    let text = r#"{"version": 1, "family": "A", "extra": true,
        "faces": [{"file": "A.ttf", "rating": 3, "color": "red"}]}"#;
    let Parsed::Sidecar(sidecar) = parse(text) else {
        panic!("not parsed");
    };
    assert_eq!(sidecar.faces[0].meta.rating, Some(3));
}

fn entry(m: Meta, prior: Option<&FaceEntry>) -> FaceEntry {
    FaceEntry {
        file: "A.ttf".into(),
        index: 0,
        postscript_name: None,
        history: next_history(&m, prior),
        meta: m,
    }
}

#[test]
fn a_change_made_on_top_of_this_machines_version_has_that_version_as_ancestor() {
    let ours = entry(meta(false, Some(4), None, &["a"]), None);
    let theirs = entry(meta(false, Some(2), None, &[]), Some(&ours));
    let later = entry(meta(false, Some(1), None, &[]), Some(&theirs));
    assert_eq!(ancestor(Some(&ours), Some(&theirs)), ours.meta);
    // Versions this machine missed in between do not matter.
    assert_eq!(ancestor(Some(&ours), Some(&later)), ours.meta);
    let merged = merge(
        &ancestor(Some(&ours), Some(&later)),
        &ours.meta,
        &later.meta,
    );
    assert_eq!(merged.meta.rating, Some(1));
    assert!(merged.meta.tags.is_empty(), "the removal is taken");
    assert!(merged.conflicts.is_empty());
}

#[test]
fn two_changes_of_the_same_version_conflict() {
    // Both machines had 3 stars. This one wrote 4; the other wrote 2, and the
    // sync client kept the other machine's file.
    let older = entry(meta(false, Some(3), None, &["x"]), None);
    let ours = entry(meta(false, Some(4), None, &[]), Some(&older));
    let theirs = entry(meta(false, Some(2), None, &["x"]), Some(&older));
    let base = ancestor(Some(&ours), Some(&theirs));
    assert_eq!(base, Meta::default());
    let merged = merge(&base, &ours.meta, &theirs.meta);
    assert_eq!(merged.meta.rating, Some(4));
    assert_eq!(merged.conflicts.len(), 1);
    assert_eq!(merged.meta.tags, ["x"], "nothing is dropped on a guess");
}

#[test]
fn without_a_base_nothing_is_common() {
    let theirs = entry(meta(true, None, None, &[]), None);
    assert_eq!(ancestor(None, Some(&theirs)), Meta::default());
}

#[test]
fn an_unchanged_or_missing_entry_has_the_base_as_ancestor() {
    let base = entry(meta(true, None, None, &[]), None);
    let same = FaceEntry {
        history: Vec::new(),
        ..base.clone()
    };
    assert_eq!(ancestor(Some(&base), Some(&same)), base.meta);
    assert_eq!(ancestor(Some(&base), None), base.meta);
}

#[test]
fn history_grows_only_with_a_change_and_stays_bounded() {
    let mut e = entry(meta(false, Some(1), None, &[]), None);
    assert_eq!(e.history.len(), 1);
    assert_eq!(next_history(&e.meta, Some(&e)), e.history);
    for rating in 2..=40u8 {
        let m = meta(false, Some(rating % 5 + 1), None, &[]);
        e = entry(m, Some(&e));
    }
    assert_eq!(e.history.len(), HISTORY);
    let a = entry(meta(false, Some(2), None, &[]), None);
    let b = entry(meta(false, Some(3), None, &[]), None);
    assert_ne!(a.history, b.history);
}
