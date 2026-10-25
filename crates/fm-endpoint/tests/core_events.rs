//! A real scan of `fixtures/fonts/` and an activation change arrive over
//! `GET /v1/events` from `CoreApi` with a mock activator (E25.2 criterion 4).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::EventStream;
use fm_api::activation::{ActivateRequest, ActivationMode, ConflictPolicy, Target};
use fm_api::common::Page;
use fm_api::library::{AddSourceRequest, ImportMode, ListFamiliesRequest};
use fm_api::{
    ActivationService as _, Api, Event, JobId, LibraryService as _, SettingsService as _,
};
use fm_core::{CoreApi, Index, SettingsStore};
use fm_endpoint::Endpoint;
use fm_platform::{AppPaths, FontActivator, MockActivator};

/// A scratch folder, removed at the end.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn core_api(scratch: &Scratch) -> CoreApi {
    let base = &scratch.0;
    let paths = AppPaths {
        data: base.join("data"),
        config: base.join("config"),
        cache: base.join("cache"),
        logs: base.join("logs"),
        library: base.join("Fonts"),
    };
    fm_core::startup::prepare(&paths).expect("app folders");
    let index = Index::open(&paths.data.join(fm_core::index::INDEX_FILE))
        .expect("index")
        .index;
    CoreApi::new(
        index,
        Arc::new(MockActivator::new()) as Arc<dyn FontActivator>,
        SettingsStore::new(&paths),
        None,
    )
    .with_endpoint_in(paths.data.clone())
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

#[test]
fn a_fixture_scan_and_an_activation_arrive_over_the_stream() {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("fm-endpoint-events-it-{}", std::process::id())));
    let api = Arc::new(core_api(&scratch));
    let endpoint = Endpoint::start(Arc::clone(&api) as Arc<dyn Api>).expect("start");
    let token = api.endpoint_info().expect("info").token;
    let mut stream = EventStream::open(endpoint.addr(), &token);
    assert_eq!(
        stream.next().expect("open").comment.as_deref(),
        Some("stream open")
    );

    // The fixtures, indexed where they are (read only), scanned over HTTP.
    let source = api
        .add_source(AddSourceRequest {
            path: fixtures().display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    let host = format!("127.0.0.1:{}", endpoint.addr().port());
    let auth = format!("Bearer {token}");
    let reply = common::send(
        endpoint.addr(),
        "POST",
        "/v1/scan",
        &[("Host", &host), ("Authorization", &auth)],
        &format!(r#"{{"source":{}}}"#, source.id),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    let job: JobId = serde_json::from_value(reply.body).expect("job id");

    let mut last_id = 0;
    let mut next = |stream: &mut EventStream| {
        let message = stream.next_event();
        let id = message.id.expect("an id");
        assert!(id > last_id, "ids rise: {id} after {last_id}");
        last_id = id;
        let event = message.decode();
        let tag = serde_json::to_value(&event).expect("json")["type"].clone();
        assert_eq!(
            message.event.as_deref(),
            tag.as_str(),
            "event: is the type tag"
        );
        event
    };

    // This scan's events, in order: started, progress, finished.
    let mut seen = Vec::new();
    let added = loop {
        match next(&mut stream) {
            Event::ScanStarted { job: j, .. } if j == job => seen.push("started"),
            Event::ScanProgress { job: j, .. } if j == job => {
                assert_eq!(seen.first(), Some(&"started"), "progress before start");
                seen.push("progress");
            }
            Event::ScanFinished { job: j, added, .. } if j == job => {
                assert!(seen.contains(&"progress"), "no progress: {seen:?}");
                break added;
            }
            Event::JobFailed { job: j, error } if j == job => panic!("scan failed: {error:?}"),
            _ => {}
        }
    };
    assert!(added > 0, "the scan added no face");

    // An activation change.
    let family = api
        .list_families(ListFamiliesRequest {
            page: Page {
                offset: 0,
                limit: 1,
            },
            ..ListFamiliesRequest::default()
        })
        .expect("families")
        .items
        .into_iter()
        .next()
        .expect("a family");
    let face = api.get_family(family.id).expect("family").faces[0].id;
    api.activate(ActivateRequest {
        target: Target::Faces { faces: vec![face] },
        mode: ActivationMode::Session,
        on_conflict: ConflictPolicy::default(),
    })
    .expect("activate");
    loop {
        if let Event::ActivationChanged { activated, .. } = next(&mut stream) {
            assert_eq!(activated, vec![face]);
            break;
        }
    }

    drop(stream);
    endpoint.stop();
}
