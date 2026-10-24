//! macOS: prints a page to PDF with a WKWebView print operation (ADR 0013).
//!
//! AppKit and WebKit live on the main thread, so a print is a small state
//! machine run there by the main dispatch queue: make an offscreen window
//! with a WKWebView, load the page, poll until it stopped loading and
//! `document.fonts.status` is `loaded`, then run the web view's print
//! operation with a save-to-file job (no panel) modally for that window.
//! The caller waits on a channel on its own thread, so the UI thread is
//! never blocked by a wait; a call from the main thread itself is refused.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Instant;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchTime, MainThreadBound};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEventMask,
    NSPaperOrientation, NSPrintInfo, NSPrintJobSavingURL, NSPrintOperation, NSPrintSaveJob,
    NSPrintingPaginationMode, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSDate, NSDefaultRunLoopMode, NSError, NSObject, NSPoint, NSRect, NSSize, NSString, NSURL,
    ns_string,
};
use objc2_web_kit::{WKWebView, WKWebViewConfiguration};

use crate::pdf::{A4_POINTS, MARGIN_MM, PRINT_TIMEOUT, PdfPrinter, PrintError};

/// The page's viewport while it is printed, in points.
const VIEW: (f64, f64) = (1280.0, 800.0);
/// How often the load and the fonts are polled, in nanoseconds.
const POLL_NS: i64 = 20_000_000;

/// Prints through WKWebView on the main thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct MacosPdfPrinter;

impl MacosPdfPrinter {
    /// The printer; its web views are made per print.
    pub fn new() -> Self {
        Self
    }
}

impl PdfPrinter for MacosPdfPrinter {
    fn print_html(&self, html: &Path, out: &Path) -> Result<(), PrintError> {
        if MainThreadMarker::new().is_some() {
            return Err(PrintError::new("a print was started on the UI thread"));
        }
        let (tx, rx) = mpsc::channel();
        let (html, out) = (html.to_path_buf(), out.to_path_buf());
        DispatchQueue::main().exec_async(move || match MainThreadMarker::new() {
            Some(mtm) => start(mtm, &html, out, tx),
            None => {
                let _ = tx.send(Err(PrintError::new(
                    "the main queue ran off the main thread",
                )));
            }
        });
        rx.recv_timeout(PRINT_TIMEOUT)
            .map_err(|_| PrintError::new("the print did not finish in time"))?
    }
}

/// One print in progress; the window is closed when the last step drops it.
struct Job {
    window: Retained<NSWindow>,
    webview: Retained<WKWebView>,
    out: PathBuf,
    tx: mpsc::Sender<Result<(), PrintError>>,
    deadline: Instant,
    done: Cell<bool>,
}

impl Job {
    /// Answers the caller once.
    fn finish(&self, result: Result<(), PrintError>) {
        if !self.done.replace(true) {
            let _ = self.tx.send(result);
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.window.close();
    }
}

type Step = fn(Rc<Job>, MainThreadMarker);

fn start(
    mtm: MainThreadMarker,
    html: &Path,
    out: PathBuf,
    tx: mpsc::Sender<Result<(), PrintError>>,
) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(VIEW.0, VIEW.1));
    // SAFETY: a fresh borderless window that is never shown; it is not
    // released on close, so the `Retained` stays the only owner.
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
    let url = NSURL::fileURLWithPath(&NSString::from_str(&html.to_string_lossy()));
    let dir = html.parent().unwrap_or(html);
    let dir = NSURL::fileURLWithPath(&NSString::from_str(&dir.to_string_lossy()));
    // SAFETY: both are file URLs; the page may read its own folder only.
    let _ = unsafe { webview.loadFileURL_allowingReadAccessToURL(&url, &dir) };
    let job = Rc::new(Job {
        window,
        webview,
        out,
        tx,
        deadline: Instant::now() + PRINT_TIMEOUT,
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

/// Waits until the page stopped loading and its fonts are loaded.
fn wait_for_page(job: Rc<Job>, mtm: MainThreadMarker) {
    if Instant::now() > job.deadline {
        job.finish(Err(PrintError::new("the page did not load in time")));
        return;
    }
    // SAFETY: a property read on the main thread.
    if unsafe { job.webview.isLoading() } {
        later(job, mtm, wait_for_page);
        return;
    }
    let next = Rc::clone(&job);
    let block = RcBlock::new(move |result: *mut AnyObject, _error: *mut NSError| {
        // SAFETY: WebKit passes a valid object or null.
        let loaded = unsafe { result.as_ref() }
            .and_then(|o| o.downcast_ref::<NSString>())
            .is_some_and(|s| s.to_string() == "loaded");
        if loaded {
            print(Rc::clone(&next), mtm);
        } else {
            later(Rc::clone(&next), mtm, wait_for_page);
        }
    });
    // SAFETY: a constant script; the block lives as long as WebKit keeps it.
    unsafe {
        job.webview.evaluateJavaScript_completionHandler(
            ns_string!("document.fonts.status"),
            Some(&block),
        );
    }
}

// The print delegates waiting for their callback; AppKit does not retain them.
thread_local! {
    static PENDING: RefCell<Vec<Retained<PrintDelegate>>> = const { RefCell::new(Vec::new()) };
}

/// What a delegate calls when its print operation ran.
type Done = RefCell<Option<Box<dyn FnOnce(bool)>>>;

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the class has no `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FmPdfPrintDelegate"]
    #[ivars = Done]
    struct PrintDelegate;

    impl PrintDelegate {
        #[unsafe(method(printOperationDidRun:success:contextInfo:))]
        fn did_run(&self, _op: &NSPrintOperation, success: Bool, _context: *mut c_void) {
            let done = self.ivars().borrow_mut().take();
            if let Some(done) = done {
                done(success.as_bool());
            }
            // Released later, once this method has returned.
            DispatchQueue::main().exec_async(|| {
                PENDING.with(|p| p.borrow_mut().retain(|d| d.ivars().borrow().is_some()));
            });
        }
    }
);

impl PrintDelegate {
    fn new(mtm: MainThreadMarker, done: Box<dyn FnOnce(bool)>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(RefCell::new(Some(done)));
        // SAFETY: NSObject's `init` on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

fn print(job: Rc<Job>, mtm: MainThreadMarker) {
    let info = NSPrintInfo::new();
    info.setPaperSize(NSSize::new(A4_POINTS.0, A4_POINTS.1));
    info.setOrientation(NSPaperOrientation::Portrait);
    let margin = MARGIN_MM / 25.4 * 72.0;
    info.setTopMargin(margin);
    info.setBottomMargin(margin);
    info.setLeftMargin(margin);
    info.setRightMargin(margin);
    info.setHorizontalPagination(NSPrintingPaginationMode::Fit);
    info.setVerticalPagination(NSPrintingPaginationMode::Automatic);
    info.setHorizontallyCentered(false);
    info.setVerticallyCentered(false);
    let out = NSURL::fileURLWithPath(&NSString::from_str(&job.out.to_string_lossy()));
    // SAFETY: the AppKit constants are valid for the life of the process;
    // the job-saving key takes an NSURL.
    unsafe {
        info.setJobDisposition(NSPrintSaveJob);
        info.dictionary()
            .setObject_forKey(&out, ProtocolObject::from_ref(NSPrintJobSavingURL));
    }
    // SAFETY: the web view is live and on the main thread.
    let op = unsafe { job.webview.printOperationWithPrintInfo(&info) };
    op.setShowsPrintPanel(false);
    op.setShowsProgressPanel(false);
    // WebKit's print view starts with an empty frame and prints blank pages.
    if let Some(view) = op.view() {
        view.setFrame(job.webview.frame());
    }
    let finished = Rc::clone(&job);
    let delegate = PrintDelegate::new(
        mtm,
        Box::new(move |ok| {
            finished.finish(if ok {
                Ok(())
            } else {
                Err(PrintError::new(format!(
                    "the print operation failed for {}",
                    finished.out.display()
                )))
            });
        }),
    );
    PENDING.with(|p| p.borrow_mut().push(delegate.clone()));
    let target: &AnyObject = &delegate;
    // SAFETY: the window is live, the delegate is kept in `PENDING` until
    // its callback ran, and the selector matches `did_run`'s signature.
    unsafe {
        op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
            &job.window,
            Some(target),
            Some(sel!(printOperationDidRun:success:contextInfo:)),
            std::ptr::null_mut(),
        );
    }
}

/// Runs `work` on a new thread while the main thread runs AppKit's event
/// loop; see [`crate::pdf::run_ui_loop`].
pub(crate) fn run_ui_loop<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let Some(mtm) = MainThreadMarker::new() else {
        return work();
    };
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
    let worker = std::thread::spawn(work);
    while !worker.is_finished() {
        let until = NSDate::dateWithTimeIntervalSinceNow(0.05);
        // SAFETY: the run-loop mode constant is valid for the life of the process.
        let event = unsafe {
            app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&until),
                NSDefaultRunLoopMode,
                true,
            )
        };
        if let Some(event) = event {
            app.sendEvent(&event);
        }
    }
    match worker.join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
