//! macOS: draws a page in a WKWebView and reads its pixels (E58.4, ADR 0029).
//! Test support; see [`crate::webview_probe`].
//!
//! AppKit and WebKit live on the main thread, so a snapshot is a small state
//! machine run there by the main dispatch queue, as the PDF printer does
//! (ADR 0013): a borderless window with a WKWebView, the page loaded, the
//! zoom set with `pageZoom` (what Tauri's `set_zoom` sets), a poll until the
//! page stopped loading, its fonts are loaded and it drew three frames, then
//! `takeSnapshotWithConfiguration:` drawn into a bitmap of the view's size in
//! points, so one pixel is one CSS pixel at 100 per cent. WebKit does not draw
//! for a window that is not on screen, so the window is ordered in, nearly
//! transparent and deaf to the mouse.

use std::rc::Rc;
use std::sync::mpsc;
use std::time::Instant;
use std::{cell::Cell, path::Path};

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchTime, MainThreadBound};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace,
    NSGraphicsContext, NSImage, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSError, NSNumber, NSPoint, NSRect, NSSize, NSString, NSURL};
use objc2_web_kit::{WKSnapshotConfiguration, WKWebView, WKWebViewConfiguration};

use crate::webview_probe::{ProbeError, SNAPSHOT_TIMEOUT, Snapshot};

/// How often the page is polled, in nanoseconds.
const POLL_NS: i64 = 20_000_000;
/// Counts three animation frames after the fonts loaded.
const FRAMES: &str = "window.__fmFrames=0;(function t(){window.__fmFrames++;\
if(window.__fmFrames<3)requestAnimationFrame(t)})();0";

type Answer = mpsc::Sender<Result<Snapshot, ProbeError>>;

pub(crate) fn snapshot_html(
    html: &Path,
    view: (u32, u32),
    page_zoom: f64,
) -> Result<Snapshot, ProbeError> {
    if MainThreadMarker::new().is_some() {
        return Err(ProbeError::new("a snapshot was started on the UI thread"));
    }
    let (tx, rx) = mpsc::channel();
    let html = html.to_path_buf();
    DispatchQueue::main().exec_async(move || match MainThreadMarker::new() {
        Some(mtm) => start(mtm, &html, view, page_zoom, tx),
        None => {
            let _ = tx.send(Err(ProbeError::new(
                "the main queue ran off the main thread",
            )));
        }
    });
    rx.recv_timeout(SNAPSHOT_TIMEOUT)
        .map_err(|_| ProbeError::new("the snapshot did not finish in time"))?
}

/// One snapshot in progress; the window is closed when the last step drops it.
struct Job {
    window: Retained<NSWindow>,
    webview: Retained<WKWebView>,
    view: (u32, u32),
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

type Step = fn(Rc<Job>, MainThreadMarker);

fn start(mtm: MainThreadMarker, html: &Path, view: (u32, u32), page_zoom: f64, answer: Answer) {
    let size = NSSize::new(f64::from(view.0), f64::from(view.1));
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
    // SAFETY: a fresh borderless window; it is not released on close, so the
    // `Retained` stays the only owner.
    let window = unsafe {
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        );
        window.setReleasedWhenClosed(false);
        window
    };
    // SAFETY: a default configuration and a web view made on the main thread.
    let webview = unsafe {
        let config = WKWebViewConfiguration::new(mtm);
        WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &config)
    };
    window.setContentView(Some(&webview));
    window.setIgnoresMouseEvents(true);
    window.setAlphaValue(0.01);
    window.orderFrontRegardless();
    // SAFETY: a property set on the main thread (macOS 11+, as wry).
    unsafe { webview.setPageZoom(page_zoom) };
    let url = NSURL::fileURLWithPath(&NSString::from_str(&html.to_string_lossy()));
    let dir = html.parent().unwrap_or(html);
    let dir = NSURL::fileURLWithPath(&NSString::from_str(&dir.to_string_lossy()));
    // SAFETY: both are file URLs; the page may read its own folder only.
    let _ = unsafe { webview.loadFileURL_allowingReadAccessToURL(&url, &dir) };
    let job = Rc::new(Job {
        window,
        webview,
        view,
        answer,
        deadline: Instant::now() + SNAPSHOT_TIMEOUT,
        inner_width: Cell::new(0.0),
        done: Cell::new(false),
    });
    later(job, mtm, wait_for_page);
}

/// Runs `step` on the main queue a moment from now.
fn later(job: Rc<Job>, mtm: MainThreadMarker, step: Step) {
    let bound = MainThreadBound::new(job, mtm);
    let _ = DispatchQueue::main().after(DispatchTime::NOW.time(POLL_NS), move || {
        if let Some(mtm) = MainThreadMarker::new() {
            step(bound.into_inner(mtm), mtm);
        }
    });
}

/// Runs `js` and hands its result (an `NSString` or `NSNumber`, as text) to `then`.
fn eval(
    job: &Rc<Job>,
    mtm: MainThreadMarker,
    js: &str,
    then: fn(Rc<Job>, MainThreadMarker, String),
) {
    let next = MainThreadBound::new(Rc::clone(job), mtm);
    let block = RcBlock::new(move |result: *mut AnyObject, _error: *mut NSError| {
        // SAFETY: WebKit passes a valid object or null.
        let text = unsafe { result.as_ref() }
            .and_then(|o| {
                o.downcast_ref::<NSString>()
                    .map(ToString::to_string)
                    .or_else(|| {
                        o.downcast_ref::<NSNumber>()
                            .map(|n| n.doubleValue().to_string())
                    })
            })
            .unwrap_or_default();
        then(Rc::clone(next.get(mtm)), mtm, text);
    });
    // SAFETY: the script is a constant of this file; the block lives as
    // long as WebKit keeps it.
    unsafe {
        job.webview
            .evaluateJavaScript_completionHandler(&NSString::from_str(js), Some(&block));
    }
}

fn wait_for_page(job: Rc<Job>, mtm: MainThreadMarker) {
    if job.late() {
        return;
    }
    // SAFETY: a property read on the main thread.
    if unsafe { job.webview.isLoading() } {
        later(job, mtm, wait_for_page);
        return;
    }
    eval(&job, mtm, "document.fonts.status", |job, mtm, status| {
        if status == "loaded" {
            eval(&job, mtm, FRAMES, |job, mtm, _| {
                later(job, mtm, wait_for_frames)
            });
        } else {
            later(job, mtm, wait_for_page);
        }
    });
}

fn wait_for_frames(job: Rc<Job>, mtm: MainThreadMarker) {
    if job.late() {
        return;
    }
    eval(&job, mtm, "window.__fmFrames", |job, mtm, frames| {
        if frames == "3" {
            eval(&job, mtm, "window.innerWidth", |job, mtm, width| {
                job.inner_width.set(width.parse().unwrap_or(0.0));
                snapshot(job, mtm);
            });
        } else {
            later(job, mtm, wait_for_frames);
        }
    });
}

fn snapshot(job: Rc<Job>, mtm: MainThreadMarker) {
    // SAFETY: a fresh configuration on the main thread.
    let config = unsafe { WKSnapshotConfiguration::new(mtm) };
    let width = NSNumber::new_f64(f64::from(job.view.0));
    // SAFETY: setters on the fresh configuration.
    unsafe {
        config.setSnapshotWidth(Some(&width));
        config.setAfterScreenUpdates(true);
    }
    let next = MainThreadBound::new(Rc::clone(&job), mtm);
    let block = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
        let job = Rc::clone(next.get(mtm));
        // SAFETY: WebKit passes a valid image or null, and an error or null.
        let result = match unsafe { image.as_ref() } {
            Some(image) => pixels(image, job.view),
            None => Err(ProbeError::new(format!(
                "WKWebView snapshot: {}",
                // SAFETY: as above.
                unsafe { error.as_ref() }
                    .map_or_else(String::new, |e| e.localizedDescription().to_string())
            ))),
        };
        job.finish(result.map(|mut snap| {
            snap.inner_width = job.inner_width.get();
            snap
        }));
    });
    // SAFETY: the configuration and the block outlive the call.
    unsafe {
        job.webview
            .takeSnapshotWithConfiguration_completionHandler(Some(&config), &block);
    }
}

/// Draws `image` into an RGBA bitmap of `view` pixels and reads it.
fn pixels(image: &NSImage, view: (u32, u32)) -> Result<Snapshot, ProbeError> {
    let (w, h) = (view.0 as isize, view.1 as isize);
    // SAFETY: AppKit allocates the planes (null); 8 bits, 4 samples, meshed.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            w,
            h,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }
    .ok_or_else(|| ProbeError::new("no bitmap"))?;
    let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)
        .ok_or_else(|| ProbeError::new("no bitmap context"))?;
    let rect = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(f64::from(view.0), f64::from(view.1)),
    );
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&ctx));
    image.drawInRect_fromRect_operation_fraction(
        rect,
        NSRect::ZERO,
        NSCompositingOperation::Copy,
        1.0,
    );
    ctx.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    let stride = usize::try_from(rep.bytesPerRow()).map_err(|_| ProbeError::new("bad stride"))?;
    let data = rep.bitmapData();
    if data.is_null() {
        return Err(ProbeError::new("the bitmap has no data"));
    }
    // SAFETY: the rep owns `stride * height` bytes at `data` while it lives,
    // and it lives to the end of this function.
    let bytes = unsafe { std::slice::from_raw_parts(data, stride * view.1 as usize) };
    Ok(Snapshot::from_rgba(view.0, view.1, stride, bytes, false))
}
