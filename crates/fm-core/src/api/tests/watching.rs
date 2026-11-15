//! The folder watcher in the library (E51.1): fonts copied into or deleted
//! from a source folder reach `list_families` and the event stream by
//! themselves. These run on every OS of the CI matrix.

use std::sync::mpsc::Receiver;

use super::*;
use crate::scanner::WatchSettings;

/// The time criterion 4 allows from a copy or a delete to `list_families`.
const WITHIN: Duration = Duration::from_secs(2);

/// A font that none of the library's folders holds.
const NEW_FONT: &str = "google-static/asap/Asap_Condensed-ExtraLightItalic.ttf";

/// A library whose folder is added, scanned, then watched.
fn watched(label: &str, settings: WatchSettings) -> (Lib, Source) {
    let (mut lib, source, _) = Lib::scanned(label);
    lib.api = lib.api.with_watch_settings(settings);
    (lib, source)
}

/// Stops the watcher before the folders go: Windows does not remove a
/// watched folder.
fn close(lib: Lib) {
    let Lib { dir, api, .. } = lib;
    drop(api);
    drop(dir);
}

fn family_total(api: &CoreApi) -> u32 {
    api.list_families(ListFamiliesRequest {
        page: Page {
            offset: 0,
            limit: 1,
        },
        ..ListFamiliesRequest::default()
    })
    .expect("families")
    .total
}

/// Waits up to 5 s until `done`, and returns how long it took; `None` when
/// it never was, so a slow runner reports a number instead of hanging.
fn until(done: impl Fn() -> bool) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if done() {
            return Some(start.elapsed());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// The `LibraryChanged` events of `source` that arrive within `quiet` of
/// each other, until none does.
fn changes_of(rx: &Receiver<Event>, source: SourceId, quiet: Duration) -> Vec<Event> {
    let mut out = Vec::new();
    while let Ok(event) = rx.recv_timeout(quiet) {
        if matches!(&event, Event::LibraryChanged { source: Some(s), .. } if *s == source) {
            out.push(event);
        }
    }
    out
}

fn added(event: &Event) -> &[FaceId] {
    match event {
        Event::LibraryChanged { added, .. } => added,
        _ => &[],
    }
}

fn removed(event: &Event) -> &[FaceId] {
    match event {
        Event::LibraryChanged { removed, .. } => removed,
        _ => &[],
    }
}

/// The faces `events` add, in all. A copy the OS reports in two parts
/// (created, then written) may land as two refreshes; the face is added once.
fn gained(events: &[Event]) -> usize {
    events.iter().map(|e| added(e).len()).sum()
}

/// The faces `events` remove, in all.
fn lost(events: &[Event]) -> usize {
    events.iter().map(|e| removed(e).len()).sum()
}

#[test]
fn a_copied_font_is_listed_within_2_s_and_gone_after_its_delete() {
    let (lib, source) = watched("api-watch-copy", WatchSettings::default());
    let before = family_total(&lib.api);
    let rx = lib.api.subscribe();

    let path = lib.root.join("new").join("Asap Condensed ü.ttf");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
    std::fs::copy(fixtures().join(NEW_FONT), &path).expect("copy");
    let took = until(|| family_total(&lib.api) == before + 1).expect("listed within 5 s");
    assert!(took < WITHIN, "the copy took {took:?} to be listed");
    let events = changes_of(&rx, source.id, Duration::from_millis(1500));
    assert_eq!(gained(&events), 1, "{events:?}");

    std::fs::remove_file(&path).expect("delete");
    let took = until(|| family_total(&lib.api) == before).expect("gone within 5 s");
    assert!(took < WITHIN, "the delete took {took:?} to leave the list");
    let events = changes_of(&rx, source.id, Duration::from_millis(1500));
    assert_eq!(lost(&events), 1, "{events:?}");

    // The watcher never touched a font file of the library (ADR 0004).
    for folder in FOLDERS {
        let name = folder.rsplit('/').next().unwrap_or(folder);
        for entry in std::fs::read_dir(fixtures().join(folder)).expect("fixture folder") {
            let from = entry.expect("entry").path();
            if crate::scanner::is_font_file(&from) {
                let copy = lib.root.join(name).join(from.file_name().expect("name"));
                assert_eq!(
                    std::fs::read(&copy).expect("still there"),
                    std::fs::read(&from).expect("fixture"),
                );
            }
        }
    }
    close(lib);
}

#[test]
fn a_burst_of_200_files_lands_as_one_refresh() {
    // A long debounce, so a slow runner's copies still make one burst.
    let settings = WatchSettings {
        quiet: Duration::from_millis(1500),
        max_wait: Duration::from_secs(30),
        ..WatchSettings::default()
    };
    let (lib, source) = watched("api-watch-burst", settings);
    let rx = lib.api.subscribe();
    let data = std::fs::read(fixtures().join(NEW_FONT)).expect("fixture");
    let folder = lib.root.join("burst");
    std::fs::create_dir_all(&folder).expect("folder");
    for i in 0..200 {
        std::fs::write(folder.join(format!("copy-{i:03}.ttf")), &data).expect("write");
    }

    let events = changes_of(&rx, source.id, Duration::from_secs(5));
    assert_eq!(events.len(), 1, "one refresh, not {}", events.len());
    assert_eq!(added(&events[0]).len(), 200);
    close(lib);
}

#[test]
fn sources_added_paused_resumed_and_removed_change_what_is_watched() {
    let (lib, source) = watched("api-watch-sources", WatchSettings::default());
    let rx = lib.api.subscribe();

    // Added while the watcher runs: watched without a restart.
    let other = lib.dir.path().join("Other");
    std::fs::create_dir_all(&other).expect("folder");
    let second = lib
        .api
        .add_source(AddSourceRequest {
            path: other.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    let path = other.join("a.ttf");
    std::fs::copy(fixtures().join(NEW_FONT), &path).expect("copy");
    let events = changes_of(&rx, second.id, Duration::from_millis(1500));
    assert_eq!(gained(&events), 1, "{events:?}");

    // Paused: not watched.
    lib.api
        .set_source_paused(SetSourcePausedRequest {
            source: second.id,
            paused: true,
        })
        .expect("pause");
    let _ = changes_of(&rx, second.id, Duration::from_millis(500));
    let later = other.join("b.ttf");
    std::fs::copy(fixtures().join(NEW_FONT), &later).expect("copy");
    let events = changes_of(&rx, second.id, Duration::from_millis(1500));
    assert!(events.is_empty(), "a paused source was watched: {events:?}");

    // Resumed: rescanned at once, so the file copied while paused lands, and
    // watched again.
    lib.api
        .set_source_paused(SetSourcePausedRequest {
            source: second.id,
            paused: false,
        })
        .expect("resume");
    let events = changes_of(&rx, second.id, Duration::from_millis(1500));
    // The resume's own event brings back the first face; the rescan adds b.
    assert_eq!(gained(&events), 2, "{events:?}");
    std::fs::remove_file(&later).expect("delete");
    let events = changes_of(&rx, second.id, Duration::from_millis(1500));
    assert_eq!(lost(&events), 1, "{events:?}");

    // Removed: no longer watched.
    lib.api.remove_source(second.id).expect("remove");
    let _ = changes_of(&rx, second.id, Duration::from_millis(500));
    std::fs::copy(fixtures().join(NEW_FONT), other.join("c.ttf")).expect("copy");
    let events = changes_of(&rx, second.id, Duration::from_millis(1500));
    assert!(
        events.is_empty(),
        "a removed source was watched: {events:?}"
    );

    // The first source was watched all along.
    let first = lib.root.join("d.ttf");
    std::fs::copy(fixtures().join(NEW_FONT), &first).expect("copy");
    let events = changes_of(&rx, source.id, Duration::from_millis(1500));
    assert_eq!(gained(&events), 1, "{events:?}");
    close(lib);
}

#[test]
fn a_changed_sidecar_is_merged_and_sent_as_read_again() {
    let (lib, source) = watched("api-watch-sidecar", WatchSettings::default());
    let family = lib.family("Bebas Neue");
    let face = lib.api.get_family(family.id).expect("family").faces[0].id;
    // Tag the face: its family's sidecar is written into the source.
    lib.api
        .tag_faces(TagFacesRequest {
            faces: vec![face],
            tags: vec!["poster".into()],
            families: Vec::new(),
        })
        .expect("tag");
    let rx = lib.api.subscribe();
    // The app's own sidecar write is merged quietly (no new metadata), so
    // it ends without a loop.
    let _ = changes_of(&rx, source.id, Duration::from_millis(1500));

    // Another machine's edit arrives through a sync client.
    let dir = lib.root.join(crate::sidecar::SIDECAR_DIR);
    let sidecar = std::fs::read_dir(&dir)
        .expect("sidecar folder")
        .map(|e| e.expect("entry").path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .expect("a sidecar");
    let text = std::fs::read_to_string(&sidecar).expect("read sidecar");
    assert!(text.contains("poster"), "{text}");
    std::fs::write(&sidecar, text.replace("poster", "headline")).expect("edit sidecar");

    let events = changes_of(&rx, source.id, Duration::from_millis(1500));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::LibraryChanged { updated, .. } if updated.contains(&face))),
        "{events:?}"
    );
    let tags = lib.api.get_face(face).expect("face").tags;
    assert!(tags.contains(&"headline".to_owned()), "{tags:?}");
    close(lib);
}

#[test]
fn without_the_watcher_nothing_is_watched() {
    let (lib, source, _) = Lib::scanned("api-watch-off");
    let rx = lib.api.subscribe();
    std::fs::copy(fixtures().join(NEW_FONT), lib.root.join("a.ttf")).expect("copy");
    let events = changes_of(&rx, source.id, Duration::from_millis(1500));
    assert!(events.is_empty(), "{events:?}");
    close(lib);
}
