//! Linux: draws a page in a WebKitGTK view and reads its pixels (E58.4,
//! ADR 0029). Test support; see [`crate::webview_probe`].
//!
//! GTK lives on the thread that runs the default main context, so a snapshot
//! is queued there as an idle source, as the PDF printer does (ADR 0013): an
//! offscreen window with a WebKitGTK view, the zoom set with
//! `webkit_web_view_set_zoom_level` (what Tauri's `set_zoom` calls), a poll
//! until the page stopped loading, its fonts are loaded and it drew three
//! frames, then `webkit_web_view_get_snapshot` of the visible region as a
//! cairo image (ARGB32, BGRA in memory on the little-endian runners).

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{cairo, gio, glib};
use javascriptcore::ValueExt as _;
use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebView, WebViewExt as _};

use crate::pdf::file_url;
use crate::webview_probe::{ProbeError, SNAPSHOT_TIMEOUT, Snapshot};

/// How often the page is polled.
const POLL: Duration = Duration::from_millis(20);
/// Counts three animation frames after the fonts loaded.
const FRAMES: &str = "window.__fmFrames=0;(function t(){window.__fmFrames++;\
if(window.__fmFrames<3)requestAnimationFrame(t)})();0";

type Answer = mpsc::Sender<Result<Snapshot, ProbeError>>;

pub(crate) fn snapshot_html(
    html: &Path,
    view: (u32, u32),
    page_zoom: f64,
) -> Result<Snapshot, ProbeError> {
    if !gtk::is_initialized() {
        return Err(ProbeError::new("GTK is not running"));
    }
    if glib::MainContext::default().is_owner() {
        return Err(ProbeError::new("a snapshot was started on the UI thread"));
    }
    let (tx, rx) = mpsc::channel();
    let html = html.to_path_buf();
    glib::idle_add_once(move || start(&html, view, page_zoom, tx));
    rx.recv_timeout(SNAPSHOT_TIMEOUT)
        .map_err(|_| ProbeError::new("the snapshot did not finish in time"))?
}

/// One snapshot in progress; the window is destroyed when the last step drops it.
struct Job {
    window: gtk::OffscreenWindow,
    webview: WebView,
    answer: Answer,
    deadline: Instant,
    inner_width: Cell<f64>,
    done: Cell<bool>,
}

impl Job {
    fn finish(&self, result: Result<Snapshot, ProbeError>) {
        if !self.done.replace(true) {
            let _ = self.answer.send(result);
        }
    }

    fn late(&self) -> bool {
        if Instant::now() > self.deadline {
            self.finish(Err(ProbeError::new("the page did not draw in time")));
            return true;
        }
        self.done.get()
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.window.close();
    }
}

fn start(html: &Path, view: (u32, u32), page_zoom: f64, answer: Answer) {
    let (w, h) = (
        i32::try_from(view.0).unwrap_or(i32::MAX),
        i32::try_from(view.1).unwrap_or(i32::MAX),
    );
    let window = gtk::OffscreenWindow::new();
    window.set_default_size(w, h);
    let webview = WebView::new();
    webview.set_size_request(w, h);
    webview.set_zoom_level(page_zoom);
    window.add(&webview);
    window.show_all();
    webview.load_uri(&file_url(html));
    let job = Rc::new(Job {
        window,
        webview,
        answer,
        deadline: Instant::now() + SNAPSHOT_TIMEOUT,
        inner_width: Cell::new(0.0),
        done: Cell::new(false),
    });
    later(job, wait_for_page);
}

fn later(job: Rc<Job>, step: fn(Rc<Job>)) {
    glib::timeout_add_local_once(POLL, move || step(job));
}

/// Runs `js` and hands its result, as text, to `then`.
fn eval(job: &Rc<Job>, js: &str, then: fn(Rc<Job>, String)) {
    let next = Rc::clone(job);
    #[allow(deprecated)] // evaluate_javascript needs WebKitGTK 2.40; wry targets 2.38.
    job.webview
        .run_javascript(js, None::<&gio::Cancellable>, move |res| {
            let text = res
                .ok()
                .and_then(|r| r.js_value())
                .map(|v| v.to_str().to_string())
                .unwrap_or_default();
            then(next, text);
        });
}

fn wait_for_page(job: Rc<Job>) {
    if job.late() {
        return;
    }
    if job.webview.is_loading() {
        later(job, wait_for_page);
        return;
    }
    eval(&job, "document.fonts.status", |job, status| {
        if status == "loaded" {
            eval(&job, FRAMES, |job, _| later(job, wait_for_frames));
        } else {
            later(job, wait_for_page);
        }
    });
}

fn wait_for_frames(job: Rc<Job>) {
    if job.late() {
        return;
    }
    eval(&job, "window.__fmFrames", |job, frames| {
        if frames == "3" {
            eval(&job, "window.innerWidth", |job, width| {
                job.inner_width.set(width.parse().unwrap_or(0.0));
                snapshot(job);
            });
        } else {
            later(job, wait_for_frames);
        }
    });
}

fn snapshot(job: Rc<Job>) {
    let next = Rc::clone(&job);
    job.webview.snapshot(
        SnapshotRegion::Visible,
        SnapshotOptions::NONE,
        None::<&gio::Cancellable>,
        move |res| {
            let result = res
                .map_err(|e| ProbeError::new(format!("WebKitGTK snapshot: {e}")))
                .and_then(pixels)
                .map(|mut snap| {
                    snap.inner_width = next.inner_width.get();
                    snap
                });
            next.finish(result);
        },
    );
}

fn pixels(surface: cairo::Surface) -> Result<Snapshot, ProbeError> {
    let image = cairo::ImageSurface::try_from(surface)
        .map_err(|_| ProbeError::new("the snapshot is not an image surface"))?;
    image.flush();
    let (w, h) = (
        u32::try_from(image.width()).map_err(|_| ProbeError::new("bad width"))?,
        u32::try_from(image.height()).map_err(|_| ProbeError::new("bad height"))?,
    );
    let stride = usize::try_from(image.stride()).map_err(|_| ProbeError::new("bad stride"))?;
    let mut snap = None;
    image
        .with_data(|bytes| snap = Some(Snapshot::from_rgba(w, h, stride, bytes, true)))
        .map_err(|e| ProbeError::new(format!("the snapshot's pixels: {e}")))?;
    snap.ok_or_else(|| ProbeError::new("the snapshot has no pixels"))
}
