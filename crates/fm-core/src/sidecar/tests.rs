use fm_api::library::LicenseType;

use super::*;

fn meta(favorite: bool, rating: Option<u8>, notes: Option<&str>, tags: &[&str]) -> Meta {
    Meta {
        favorite,
        rating,
        notes: notes.map(Into::into),
        tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        collections: Vec::new(),
        pins: Vec::new(),
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
        pins: vec![
            pin(" Wide ", Some(&["B"]), None, &[("wdth", 110.0)]),
            pin("WIDE", Some(&["B"]), None, &[("wdth", 120.0)]),
            pin(" ", Some(&["B"]), None, &[]),
            pin("No holder", None, None, &[]),
            pin("Two holders", Some(&["B"]), Some("S"), &[]),
            pin("Empty step", Some(&["B", " "]), None, &[]),
            pin("Blank set", None, Some(" "), &[]),
            pin("Not a number", None, Some("S"), &[("wght", f32::NAN)]),
        ],
    };
    m.normalize();
    assert_eq!(m.rating, None);
    assert_eq!(m.notes, None);
    assert_eq!(m.tags, ["X"]);
    assert_eq!(m.collections, [path(&["B"])]);
    // Trimmed, one per holder and name in any case, the first kept.
    assert_eq!(
        m.pins,
        [pin("Wide", Some(&["B"]), None, &[("wdth", 110.0)])]
    );
}

fn pin(name: &str, collection: Option<&[&str]>, set: Option<&str>, at: &[(&str, f32)]) -> Pin {
    Pin {
        name: name.into(),
        collection: collection.map(path),
        set: set.map(Into::into),
        coordinates: at.iter().map(|(t, v)| ((*t).to_owned(), *v)).collect(),
    }
}

#[test]
fn pins_merge_as_a_set_and_take_the_side_that_changed() {
    let wide = pin("Wide", Some(&["Brand"]), None, &[("wdth", 110.0)]);
    let wider = pin("Wide", Some(&["Brand"]), None, &[("wdth", 125.0)]);
    let black = pin("Black", None, Some("Client"), &[("wght", 900.0)]);
    let light = pin("Light", None, Some("client"), &[("wght", 300.0)]);
    let with = |pins: &[&Pin]| Meta {
        pins: pins.iter().map(|p| (*p).clone()).collect(),
        ..Meta::default()
    };
    let base = with(&[&wide, &black]);
    // The other machine added Light and removed Black; this one left both.
    let merged = merge(&base, &base, &with(&[&wide, &light]));
    assert_eq!(merged.meta.pins, [light.clone(), wide.clone()]);
    assert!(merged.conflicts.is_empty());
    // The other machine changed Wide: its coordinates are taken.
    let merged = merge(&base, &base, &with(&[&wider, &black]));
    assert_eq!(merged.meta.pins, [black.clone(), wider.clone()]);
    // Both changed it: this machine's stay.
    let narrow = pin("Wide", Some(&["Brand"]), None, &[("wdth", 80.0)]);
    let merged = merge(&base, &with(&[&narrow, &black]), &with(&[&wider, &black]));
    assert_eq!(merged.meta.pins, [black, narrow]);
    // No base (a new machine): both sides' pins are kept.
    let merged = merge(&Meta::default(), &with(&[&wide]), &with(&[&light]));
    assert_eq!(merged.meta.pins, [light, wide]);
}

#[test]
fn a_sidecar_with_pins_is_version_2_and_one_without_stays_version_1() {
    let mut sidecar = Sidecar::new("Asap");
    sidecar.faces.push(FaceEntry {
        file: "Asap.ttf".into(),
        index: 0,
        postscript_name: None,
        meta: meta(true, None, None, &[]),
        history: Vec::new(),
    });
    sidecar.normalize();
    assert_eq!(sidecar.version, PLAIN_VERSION);
    sidecar.faces[0].meta.pins = vec![pin("Wide", Some(&["Brand"]), None, &[("wdth", 110.0)])];
    sidecar.normalize();
    assert_eq!(sidecar.version, PINS_VERSION);
    // It reads back the same, coordinates exactly.
    assert_eq!(parse(&sidecar.to_json()), Parsed::Sidecar(sidecar.clone()));
    assert!(sidecar.to_json().contains(r#""pins""#));
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
        parse(r#"{"version": 5, "faces": "anything"}"#),
        Parsed::Newer(5)
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
    assert_eq!(ancestor(Some(&ours), Some(&theirs), true), ours.meta);
    // Versions this machine missed in between do not matter.
    assert_eq!(ancestor(Some(&ours), Some(&later), true), ours.meta);
    let merged = merge(
        &ancestor(Some(&ours), Some(&later), true),
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
    let base = ancestor(Some(&ours), Some(&theirs), true);
    assert_eq!(base, Meta::default());
    let merged = merge(&base, &ours.meta, &theirs.meta);
    assert_eq!(merged.meta.rating, Some(4));
    assert_eq!(merged.conflicts.len(), 1);
    assert_eq!(merged.meta.tags, ["x"], "nothing is dropped on a guess");
}

#[test]
fn without_a_base_nothing_is_common() {
    let theirs = entry(meta(true, None, None, &[]), None);
    assert_eq!(ancestor(None, Some(&theirs), true), Meta::default());
}

#[test]
fn an_unchanged_or_missing_entry_has_the_base_as_ancestor() {
    let base = entry(meta(true, None, None, &[]), None);
    let same = FaceEntry {
        history: Vec::new(),
        ..base.clone()
    };
    assert_eq!(ancestor(Some(&base), Some(&same), false), base.meta);
    assert_eq!(ancestor(Some(&base), None, true), base.meta);
}

#[test]
fn a_missing_entry_is_a_removal_only_in_a_file_that_descends_from_ours() {
    let base = entry(meta(false, None, None, &["a"]), None);
    // The other machine removed it from our version of the file.
    assert_eq!(ancestor(Some(&base), None, true), base.meta);
    // The other machine wrote its file without ever having the entry.
    assert_eq!(ancestor(Some(&base), None, false), Meta::default());
    let merged = merge(
        &ancestor(Some(&base), None, false),
        &base.meta,
        &Meta::default(),
    );
    assert_eq!(merged.meta, base.meta, "nothing is removed on a guess");
}

#[test]
fn a_file_descends_through_its_history() {
    let mut ours = Sidecar::new("A");
    ours.faces.push(entry(meta(true, None, None, &[]), None));
    ours.history = next_file_history(&ours, None);
    let mut theirs = ours.clone();
    theirs.faces.clear();
    theirs.history = next_file_history(&theirs, Some(&ours));
    assert!(descends(&ours, &theirs));
    assert!(descends(&ours, &ours));
    let mut unrelated = Sidecar::new("A");
    unrelated.history = next_file_history(&unrelated, None);
    assert!(!descends(&ours, &unrelated));
    // An unchanged file keeps its history.
    assert_eq!(next_file_history(&ours, Some(&ours)), ours.history);
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

fn family(notes: Option<&str>, prior: Option<&FamilyEntry>) -> FamilyEntry {
    FamilyEntry {
        notes: notes.map(Into::into),
        history: next_family_history(notes, prior),
        license: None,
    }
}

#[test]
fn family_notes_merge_like_face_notes() {
    // The side that changed wins.
    let kept = merge_family(Some("old"), Some("old"), Some("theirs"));
    assert_eq!(kept.notes.as_deref(), Some("theirs"));
    assert_eq!(kept.conflict, None);
    let kept = merge_family(Some("old"), Some("mine"), Some("old"));
    assert_eq!(kept.notes.as_deref(), Some("mine"));
    // A side that cleared them wins over one that left them alone.
    assert_eq!(merge_family(Some("old"), Some("old"), None).notes, None);
    assert_eq!(merge_family(Some("old"), Some(""), Some("old")).notes, None);
    // Two different changes keep this machine's notes and report the other.
    let both = merge_family(Some("old"), Some("mine"), Some("theirs"));
    assert_eq!(both.notes.as_deref(), Some("mine"));
    let conflict = both.conflict.expect("conflict");
    assert_eq!(conflict.field, Field::FamilyNotes);
    assert_eq!(conflict.disk, r#""theirs""#);
    // Without a base, two different values conflict too.
    assert!(
        merge_family(None, Some("mine"), Some("theirs"))
            .conflict
            .is_some()
    );
    assert_eq!(
        Field::from_name(Field::FamilyNotes.as_str()),
        Some(Field::FamilyNotes)
    );
}

#[test]
fn the_family_ancestor_follows_the_entry_history() {
    let ours = family(Some("mine"), None);
    // The other machine changed our version: ours is the ancestor.
    let theirs = family(Some("theirs"), Some(&ours));
    assert_eq!(
        family_ancestor(Some(&ours), Some(&theirs), false).as_deref(),
        Some("mine")
    );
    // An unrelated change has nothing in common with ours.
    let unrelated = family(Some("theirs"), None);
    assert_eq!(family_ancestor(Some(&ours), Some(&unrelated), true), None);
    // Gone from a file that descends from ours: removed there.
    assert_eq!(
        family_ancestor(Some(&ours), None, true).as_deref(),
        Some("mine")
    );
    assert_eq!(family_ancestor(Some(&ours), None, false), None);
    assert_eq!(family_ancestor(None, Some(&theirs), true), None);
    // The history grows only with a change.
    assert_eq!(next_family_history(Some("mine"), Some(&ours)), ours.history);
    assert_eq!(theirs.history.len(), 2);
    // A family entry that only holds a license (E15.2) is no notes entry:
    // notes cleared there were removed when the file descends from ours.
    let license_only = FamilyEntry {
        notes: None,
        history: Vec::new(),
        license: Some(LicenseEntry::default()),
    };
    assert_eq!(
        family_ancestor(Some(&ours), Some(&license_only), true).as_deref(),
        Some("mine")
    );
    assert_eq!(
        family_ancestor(Some(&ours), Some(&license_only), false),
        None
    );
}

#[test]
fn a_sidecar_with_family_notes_is_version_3_and_reads_back() {
    let mut sidecar = Sidecar::new("Asap");
    sidecar.family_meta = Some(family(Some(""), None));
    sidecar.normalize();
    // Empty notes are none, and no family entry is written for them.
    assert_eq!(sidecar.family_meta, None);
    assert_eq!(sidecar.version, PLAIN_VERSION);
    assert!(!sidecar.to_json().contains("familyMeta"));
    sidecar.family_meta = Some(family(Some("Licensed for the rebrand"), None));
    sidecar.normalize();
    assert_eq!(sidecar.version, FAMILY_NOTES_VERSION);
    assert_eq!(sidecar.family_notes(), Some("Licensed for the rebrand"));
    assert!(sidecar.to_json().contains(r#""familyMeta""#));
    assert_eq!(parse(&sidecar.to_json()), Parsed::Sidecar(sidecar.clone()));
}

#[test]
fn a_change_of_the_family_notes_is_a_new_file_version() {
    let mut ours = Sidecar::new("A");
    ours.faces.push(entry(meta(true, None, None, &[]), None));
    ours.history = next_file_history(&ours, None);
    let before = ours.history.clone();
    let mut theirs = ours.clone();
    theirs.family_meta = Some(family(Some("theirs"), None));
    // Not the same entries: a new version, which descends from ours.
    theirs.history = next_file_history(&theirs, Some(&ours));
    assert_ne!(theirs.history, before);
    assert!(descends(&ours, &theirs));
    // A version id is unique to its version (E28.1): the same content
    // written apart is two versions.
    let mut plain = Sidecar::new("A");
    plain.faces = ours.faces.clone();
    assert_ne!(next_file_history(&plain, None), before);
}

fn license(kind: LicenseType, seats: Option<u32>) -> License {
    License {
        license_type: Some(kind),
        seats,
        purchase_date: Some("2026-03-01".into()),
        receipt: Some("receipts/invoice.pdf".into()),
    }
}

fn licensed(license: Option<License>, prior: Option<&LicenseEntry>) -> FamilyEntry {
    FamilyEntry {
        notes: None,
        history: Vec::new(),
        license: license.map(|license| LicenseEntry {
            history: next_license_history(&license, prior),
            license,
        }),
    }
}

#[test]
fn a_sidecar_with_a_declared_license_is_version_4_and_reads_back() {
    let mut sidecar = Sidecar::new("Asap");
    sidecar.family_meta = Some(licensed(Some(License::default()), None));
    sidecar.normalize();
    // An empty license is none, and no family entry is written for it.
    assert_eq!(sidecar.family_meta, None);
    assert_eq!(sidecar.version, PLAIN_VERSION);
    sidecar.family_meta = Some(licensed(
        Some(license(LicenseType::PersonalOnly, Some(0))),
        None,
    ));
    sidecar.normalize();
    assert_eq!(sidecar.version, SIDECAR_VERSION);
    assert_eq!(
        sidecar.family_license(),
        Some(&license(LicenseType::PersonalOnly, Some(0)))
    );
    let json = sidecar.to_json();
    assert!(json.contains(r#""licenseType": "personalOnly""#), "{json}");
    assert!(json.contains(r#""seats": 0"#), "{json}");
    assert!(json.contains(r#""purchaseDate": "2026-03-01""#), "{json}");
    assert_eq!(parse(&json), Parsed::Sidecar(sidecar.clone()));
}

#[test]
fn a_license_merges_as_one_value() {
    let a = license(LicenseType::Desktop, Some(2));
    let b = license(LicenseType::Web, Some(2));
    let c = license(LicenseType::Desktop, Some(9));
    // The side that changed wins whole.
    let kept = merge_license(Some(&a), Some(&a), Some(&b));
    assert_eq!((kept.license, kept.conflict), (Some(b.clone()), None));
    let kept = merge_license(Some(&a), Some(&c), Some(&a));
    assert_eq!((kept.license, kept.conflict), (Some(c.clone()), None));
    // A removal on one side removes it; an empty license is none.
    assert_eq!(merge_license(Some(&a), Some(&a), None).license, None);
    assert_eq!(
        merge_license(Some(&a), Some(&License::default()), Some(&a)).license,
        None
    );
    // Two different changes, even of different fields, are one conflict:
    // this machine's license stays, never a mix of the two.
    let both = merge_license(Some(&a), Some(&c), Some(&b));
    assert_eq!(both.license, Some(c.clone()));
    let conflict = both.conflict.expect("conflict");
    assert_eq!(conflict.field, Field::FamilyLicense);
    assert_eq!(
        Field::from_name(conflict.field.as_str()),
        Some(Field::FamilyLicense)
    );
    assert!(conflict.disk.contains("\"web\""), "{}", conflict.disk);
    // No common ancestor: two different licenses conflict.
    assert!(merge_license(None, Some(&a), Some(&b)).conflict.is_some());
    assert!(merge_license(None, Some(&a), Some(&a)).conflict.is_none());
}

#[test]
fn the_license_ancestor_follows_its_own_history() {
    let ours = licensed(Some(license(LicenseType::Desktop, None)), None);
    let theirs = licensed(Some(license(LicenseType::Web, None)), ours.license.as_ref());
    let unrelated = licensed(Some(license(LicenseType::Web, None)), None);
    assert_eq!(
        license_ancestor(Some(&ours), Some(&theirs), false),
        Some(license(LicenseType::Desktop, None))
    );
    assert_eq!(license_ancestor(Some(&ours), Some(&unrelated), true), None);
    // A license missing on disk was removed there only when the file
    // descends from ours.
    let without = licensed(None, None);
    assert_eq!(
        license_ancestor(Some(&ours), Some(&without), true),
        Some(license(LicenseType::Desktop, None))
    );
    assert_eq!(license_ancestor(Some(&ours), Some(&without), false), None);
    assert_eq!(
        license_ancestor(Some(&ours), None, true),
        ours.license.map(|l| l.license)
    );
    assert_eq!(license_ancestor(None, Some(&theirs), true), None);
    // The history grows only with a change.
    let entry = theirs.license.expect("license");
    assert_eq!(
        next_license_history(&entry.license, Some(&entry)),
        entry.history
    );
    assert_eq!(entry.history.len(), 2);
}

#[test]
fn a_change_of_the_license_is_a_new_file_version() {
    let mut ours = Sidecar::new("A");
    ours.family_meta = Some(family(Some("notes"), None));
    ours.history = next_file_history(&ours, None);
    let mut theirs = ours.clone();
    if let Some(family) = &mut theirs.family_meta {
        family.license = Some(LicenseEntry {
            license: license(LicenseType::App, None),
            history: Vec::new(),
        });
    }
    theirs.history = next_file_history(&theirs, Some(&ours));
    assert_ne!(theirs.history, ours.history);
    assert!(descends(&ours, &theirs));
}
