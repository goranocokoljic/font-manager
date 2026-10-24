//! Linux: prints a page to PDF with a WebKitGTK print operation (ADR 0013).
//!
//! GTK lives on one thread, the one that runs the default main context (the
//! app's UI thread). A print is queued there as an idle source: it makes an
//! offscreen window with a WebKitGTK view, loads the page, polls until it
//! stopped loading and `document.fonts.status` is `loaded`, then prints it
//! without a dialog to GTK's "Print to File" printer as PDF. The caller waits
//! on a channel on its own thread; a call from the GTK thread itself, or
//! before GTK is initialized, is refused.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};
use javascriptcore::ValueExt as _;
use webkit2gtk::{PrintOperation, PrintOperationExt as _, WebView, WebViewExt as _};

use crate::pdf::{
    MARGIN_MM, PRINT_TIMEOUT, PdfPrinter, PrintError, Reply, file_url, wait_for_ui_thread,
};

/// The page's viewport while it is printed, in pixels.
const VIEW: (i32, i32) = (1280, 800);
/// How often the load and the fonts are polled.
const POLL: Duration = Duration::from_millis(20);

/// Prints through WebKitGTK on the GTK thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxPdfPrinter;

impl LinuxPdfPrinter {
    /// The printer; its web views are made per print.
    pub fn new() -> Self {
        Self
    }
}

impl PdfPrinter for LinuxPdfPrinter {
    fn print_html(&self, html: &Path, out: &Path) -> Result<(), PrintError> {
        if !gtk::is_initialized() {
            return Err(PrintError::new("GTK is not running"));
        }
        if glib::MainContext::default().is_owner() {
            return Err(PrintError::new("a print was started on the UI thread"));
        }
        let html = html.to_path_buf();
        wait_for_ui_thread(out, PRINT_TIMEOUT, |reply| {
            glib::idle_add_once(move || start(&html, reply));
        })
    }
}

/// One print in progress; the window is destroyed when the last step drops it.
struct Job {
    window: gtk::OffscreenWindow,
    webview: WebView,
    reply: Reply,
    deadline: Instant,
    done: Cell<bool>,
    /// The running operation, kept alive until it finished.
    op: RefCell<Option<PrintOperation>>,
}

impl Job {
    /// Answers the caller once, and lets go of the operation after its
    /// signal handler returned.
    fn finish(&self, result: Result<(), PrintError>) {
        if !self.done.replace(true) {
            self.reply.send(result);
        }
        if let Some(op) = self.op.borrow_mut().take() {
            glib::idle_add_local_once(move || drop(op));
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.window.close();
    }
}

fn start(html: &Path, reply: Reply) {
    let window = gtk::OffscreenWindow::new();
    window.set_default_size(VIEW.0, VIEW.1);
    let webview = WebView::new();
    webview.set_size_request(VIEW.0, VIEW.1);
    window.add(&webview);
    window.show_all();
    webview.load_uri(&file_url(html));
    let job = Rc::new(Job {
        window,
        webview,
        reply,
        deadline: Instant::now() + PRINT_TIMEOUT,
        done: Cell::new(false),
        op: RefCell::new(None),
    });
    later(job, wait_for_page);
}

/// Runs `step` on the GTK thread a moment from now.
fn later(job: Rc<Job>, step: fn(Rc<Job>)) {
    glib::timeout_add_local_once(POLL, move || step(job));
}

/// Waits until the page stopped loading and its fonts are loaded.
fn wait_for_page(job: Rc<Job>) {
    if job.reply.gone() || Instant::now() > job.deadline {
        job.finish(Err(PrintError::new("the page did not load in time")));
        return;
    }
    if job.webview.is_loading() {
        later(job, wait_for_page);
        return;
    }
    let next = Rc::clone(&job);
    #[allow(deprecated)] // evaluate_javascript needs WebKitGTK 2.40; wry targets 2.38.
    job.webview.run_javascript(
        "document.fonts.status",
        None::<&gio::Cancellable>,
        move |res| {
            let loaded = res
                .ok()
                .and_then(|r| r.js_value())
                .is_some_and(|v| v.to_str() == "loaded");
            if loaded {
                print(next);
            } else {
                later(next, wait_for_page);
            }
        },
    );
}

fn print(job: Rc<Job>) {
    if job.reply.gone() {
        job.finish(Err(PrintError::new("the caller stopped waiting")));
        return;
    }
    let paper = gtk::PaperSize::new(Some(gtk::PAPER_NAME_A4.as_str()));
    let settings = gtk::PrintSettings::new();
    // The file printer's name is translated with GTK's own catalogue.
    settings.set_printer(&glib::dgettext(Some("gtk30"), "Print to File"));
    settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT.as_str(), Some("pdf"));
    settings.set(
        gtk::PRINT_SETTINGS_OUTPUT_URI.as_str(),
        Some(&file_url(job.reply.out())),
    );
    settings.set_paper_size(&paper);
    settings.set_orientation(gtk::PageOrientation::Portrait);
    let setup = gtk::PageSetup::new();
    setup.set_paper_size(&paper);
    setup.set_orientation(gtk::PageOrientation::Portrait);
    setup.set_top_margin(MARGIN_MM, gtk::Unit::Mm);
    setup.set_bottom_margin(MARGIN_MM, gtk::Unit::Mm);
    setup.set_left_margin(MARGIN_MM, gtk::Unit::Mm);
    setup.set_right_margin(MARGIN_MM, gtk::Unit::Mm);

    let op = PrintOperation::new(&job.webview);
    op.set_print_settings(&settings);
    op.set_page_setup(&setup);
    let failed = Rc::clone(&job);
    op.connect_failed(move |_, e| {
        failed.finish(Err(PrintError::new(format!("WebKitGTK print: {e}"))));
    });
    let finished = Rc::clone(&job);
    op.connect_finished(move |_| finished.finish(Ok(())));
    *job.op.borrow_mut() = Some(op.clone());
    op.print();
}

/// Runs `work` on a new thread while this thread runs GTK's main context;
/// see [`crate::pdf::run_ui_loop`]. Without a display GTK does not start,
/// and `work` runs alone (a print then fails at once).
pub(crate) fn run_ui_loop<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    if gtk::init().is_err() {
        return work();
    }
    let ctx = glib::MainContext::default();
    // Wakes the loop so it notices the worker is done.
    let tick = glib::timeout_add_local(Duration::from_millis(50), || glib::ControlFlow::Continue);
    let worker = std::thread::spawn(work);
    while !worker.is_finished() {
        ctx.iteration(true);
    }
    tick.remove();
    match worker.join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
