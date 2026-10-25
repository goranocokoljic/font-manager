//! `GET /v1/events` over `MockApi` (E25.2 criteria 1–3).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::io::{ErrorKind, Read as _};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::{Served, send};
use fm_api::library::ScanRequest;
use fm_api::{Event, JobId, LibraryService as _};
use fm_endpoint::events::{KEEP_ALIVE, STREAM_BUFFER, StreamOptions};

#[test]
fn a_scan_streams_started_progress_and_finished_in_order() {
    let served = Served::new();
    let mut stream = served.events();
    let head = stream.headers.to_ascii_lowercase();
    assert!(head.contains("content-type: text/event-stream"), "{head}");
    assert!(head.contains("cache-control: no-store"), "{head}");
    assert_eq!(
        stream.next().expect("first message").comment.as_deref(),
        Some("stream open")
    );

    let reply = served.call("scan", "{}");
    assert_eq!(reply.status, 200, "{}", reply.body);
    let job: JobId = serde_json::from_value(reply.body).expect("job id");

    let messages: Vec<_> = (0..3).map(|_| stream.next_event()).collect();
    let tags: Vec<_> = messages.iter().map(|m| m.event.as_deref()).collect();
    assert_eq!(
        tags,
        [
            Some("scanStarted"),
            Some("scanProgress"),
            Some("scanFinished")
        ]
    );
    let ids: Vec<_> = messages.iter().map(|m| m.id).collect();
    assert_eq!(ids, [Some(1), Some(2), Some(3)]);
    let events: Vec<Event> = messages.iter().map(common::Message::decode).collect();
    assert!(matches!(events[0], Event::ScanStarted { job: j, .. } if j == job));
    assert!(matches!(events[1], Event::ScanProgress { job: j, .. } if j == job));
    assert!(matches!(events[2], Event::ScanFinished { job: j, .. } if j == job));
}

#[test]
fn a_quiet_stream_gets_a_keep_alive_comment() {
    assert_eq!(KEEP_ALIVE, Duration::from_secs(15));
    assert_eq!(StreamOptions::default().keep_alive, KEEP_ALIVE);
    let keep_alive = Duration::from_millis(300);
    let served = Served::with(StreamOptions {
        keep_alive,
        ..StreamOptions::default()
    });
    let mut stream = served.events();
    let opened = Instant::now();
    assert_eq!(
        stream.next().expect("open").comment.as_deref(),
        Some("stream open")
    );
    for _ in 0..2 {
        let message = stream.next().expect("keep-alive");
        assert_eq!(
            message.comment.as_deref(),
            Some("keep-alive"),
            "{message:?}"
        );
    }
    let took = opened.elapsed();
    assert!(
        took >= keep_alive * 2 - Duration::from_millis(50),
        "two keep-alives came after {took:?}"
    );
}

#[test]
fn the_stream_needs_the_same_checks_as_every_route() {
    let served = Served::new();
    let addr = served.endpoint.addr();
    let host = served.host();
    let auth = format!("Bearer {}", served.token());
    let wrong = format!("Bearer {}", "0".repeat(64));
    let cases: [(&[(&str, &str)], u16); 4] = [
        (&[("Host", &host)], 401),
        (&[("Host", &host), ("Authorization", &wrong)], 401),
        (&[("Host", "evil.example"), ("Authorization", &auth)], 403),
        (
            &[
                ("Host", &host),
                ("Authorization", &auth),
                ("Origin", "https://evil.example"),
            ],
            403,
        ),
    ];
    for (headers, status) in cases {
        let reply = send(addr, "GET", "/v1/events", headers, "");
        assert_eq!(reply.status, status, "{headers:?}: {}", reply.body);
        assert_eq!(reply.body["code"], "permissionDenied");
    }
    let reply = send(
        addr,
        "POST",
        "/v1/events",
        &[("Host", &host), ("Authorization", &auth)],
        "",
    );
    assert_eq!(reply.status, 405, "{}", reply.body);
    assert!(reply.headers.to_ascii_lowercase().contains("allow: get"));
    assert_eq!(
        served.endpoint.event_streams(),
        0,
        "a refused request opened a stream"
    );
}

#[test]
fn closing_the_connection_drops_the_subscription_and_its_thread() {
    let served = Served::new();
    let mut stream = served.events();
    stream.next().expect("open");
    served.wait_for_streams(1);
    let second = served.events();
    served.wait_for_streams(2);

    drop(stream);
    // The server learns of the close when it next writes; events make it
    // write.
    common::wait_for_streams(&served.endpoint, 1, || {
        served.api.scan(ScanRequest::default()).expect("scan");
    });
    drop(second);
    common::wait_for_streams(&served.endpoint, 0, || {
        served.api.scan(ScanRequest::default()).expect("scan");
    });
}

#[test]
fn a_stalled_client_is_cut_off_while_another_keeps_receiving() {
    // A small buffer and no keep-alive, so only events fill the stream.
    let buffer = 256;
    assert!(buffer < STREAM_BUFFER);
    let served = Served::with(StreamOptions {
        keep_alive: Duration::from_secs(3600),
        buffer,
    });
    // The stalled client reads the response head and nothing more.
    let stalled = served.events();
    let mut reading = served.events();
    served.wait_for_streams(2);

    let received = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&received);
    let reader = std::thread::spawn(move || {
        while let Some(message) = reading.next() {
            if message.comment.is_none() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }
    });

    // Scans in batches smaller than the buffer; each batch waits for the
    // reading client, so only the stalled one falls behind.
    let scans_per_batch = 50;
    let mut sent = 0;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut batches = 0;
    while served.endpoint.event_streams() == 2 {
        assert!(
            Instant::now() < deadline,
            "the stalled stream was never closed ({sent} events sent)"
        );
        for _ in 0..scans_per_batch {
            // The API never waits for the stalled client.
            served.api.scan(ScanRequest::default()).expect("scan");
        }
        sent += scans_per_batch * 3;
        wait_for(&received, sent);
        batches += 1;
        if batches == 1 {
            let reply = served.call("list_sources", "");
            assert_eq!(reply.status, 200, "a call waited on the stalled stream");
        }
    }
    assert!(sent > buffer, "cut off after only {sent} events");
    assert_eq!(served.endpoint.event_streams(), 1);

    // The other client still receives.
    served.api.scan(ScanRequest::default()).expect("scan");
    sent += 3;
    wait_for(&received, sent);
    assert_eq!(served.endpoint.event_streams(), 1);

    // The stalled client's connection was closed by the server: reading it
    // ends (after what was already sent) instead of waiting.
    let mut rest = Vec::new();
    match stalled.into_inner().read_to_end(&mut rest) {
        Ok(_) => {}
        Err(e) => assert!(
            !matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
            "the stalled connection is still open: {e}"
        ),
    }

    drop(served);
    reader.join().expect("reader");
}

/// Waits up to 30 s for the reading client to have `n` events.
fn wait_for(received: &AtomicUsize, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while received.load(Ordering::SeqCst) < n {
        assert!(
            Instant::now() < deadline,
            "the reading client got {} of {n} events",
            received.load(Ordering::SeqCst)
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
