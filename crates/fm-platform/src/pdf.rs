//! Prints an HTML page to a paginated A4 PDF through the webview the app
//! already ships (E21.2, ADR 0013): WebView2 `PrintToPdf` on Windows, a
//! WKWebView print operation on macOS, a WebKitGTK print operation on Linux.
//! Each OS has its own file under `src/{windows,macos,linux}/pdf.rs`; no
//! external browser is ever started.
//!
//! The printer owns an offscreen webview. On macOS and Linux it lives on the
//! process's UI thread (AppKit and GTK are single-threaded): a print is sent
//! there and the caller waits, so [`PdfPrinter::print_html`] must be called
//! from a worker thread, never the UI thread. On Windows it runs on a thread
//! of its own. A host without a UI loop of its own (a test) runs one with
//! [`run_ui_loop`].

use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use crate::AppPaths;

/// A4 in inches, the unit WebView2 takes: 210 × 297 mm.
pub const A4_INCHES: (f64, f64) = (210.0 / 25.4, 297.0 / 25.4);
/// A4 in PostScript points, the unit AppKit takes.
pub const A4_POINTS: (f64, f64) = (A4_INCHES.0 * 72.0, A4_INCHES.1 * 72.0);
/// The page margin on every side, in millimetres.
pub const MARGIN_MM: f64 = 12.0;
/// How long a print may take before the printer gives up.
pub const PRINT_TIMEOUT: Duration = Duration::from_secs(120);

/// Why a page was not printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintError(pub String);

impl PrintError {
    /// An error with this developer detail.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }
}

impl fmt::Display for PrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PrintError {}

/// Prints a local HTML file to a PDF file.
pub trait PdfPrinter: Send + Sync + fmt::Debug {
    /// Loads `html` (a local file; everything it shows is in it), waits for
    /// its fonts, and prints it to `out` as a paginated A4 PDF with
    /// backgrounds and no header or footer. Blocks until the file is written
    /// or the print failed; a failed print may leave a partial `out`, which
    /// the caller removes. Never call it from the UI thread.
    fn print_html(&self, html: &Path, out: &Path) -> Result<(), PrintError>;
}

/// The OS printer, its webview data (Windows) under the cache folder of `paths`.
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
pub fn system(paths: &AppPaths) -> Result<Arc<dyn PdfPrinter>, PrintError> {
    #[cfg(windows)]
    let printer = crate::windows::WindowsPdfPrinter::new(paths.cache.join(WEBVIEW_DIR));
    #[cfg(target_os = "macos")]
    let printer = {
        let _ = paths;
        crate::macos::MacosPdfPrinter::new()
    };
    #[cfg(target_os = "linux")]
    let printer = {
        let _ = paths;
        crate::linux::LinuxPdfPrinter::new()
    };
    Ok(Arc::new(printer))
}

/// No printer on an OS the app does not support.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn system(_paths: &AppPaths) -> Result<Arc<dyn PdfPrinter>, PrintError> {
    Err(PrintError::new("no PDF printer on this OS"))
}

/// The folder under the cache folder that holds the printer's webview data.
pub const WEBVIEW_DIR: &str = "pdf-webview";

/// Runs `work` on a new thread while this thread runs the OS UI loop, and
/// returns what `work` returned once it is done. Call it from the process's
/// main thread in a host that has no UI loop of its own; the app does not
/// need it (Tauri runs the loop). On Windows the printer has its own thread,
/// so `work` simply runs.
pub fn run_ui_loop<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    #[cfg(target_os = "macos")]
    return crate::macos::run_ui_loop(work);
    #[cfg(target_os = "linux")]
    return crate::linux::run_ui_loop(work);
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    work()
}

/// The UI thread's side of one print (macOS and Linux): answers the caller
/// waiting in [`wait_for_ui_thread`] and, once that caller has given up,
/// removes what the print wrote, so a late print leaves no file behind.
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
#[derive(Debug)]
pub(crate) struct Reply {
    tx: mpsc::Sender<Result<(), PrintError>>,
    gone: Arc<AtomicBool>,
    out: PathBuf,
}

#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
impl Reply {
    /// The file the print writes.
    pub(crate) fn out(&self) -> &Path {
        &self.out
    }

    /// Whether the caller stopped waiting; a print not started yet must not start.
    pub(crate) fn gone(&self) -> bool {
        self.gone.load(Ordering::SeqCst)
    }

    /// Answers the caller. When nobody waits any more, the file the print
    /// may have written is removed: the caller's cleanup has already run.
    pub(crate) fn send(&self, result: Result<(), PrintError>) {
        if self.tx.send(result).is_err() || self.gone() {
            let _ = std::fs::remove_file(&self.out);
        }
    }
}

/// Starts a print on the UI thread through `start`, which gets the
/// [`Reply`] for `out`, and waits up to `timeout` for its answer. On a
/// timeout the reply is marked gone, so the job does not start its print,
/// and a print still running removes its file when it ends.
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
pub(crate) fn wait_for_ui_thread(
    out: &Path,
    timeout: Duration,
    start: impl FnOnce(Reply),
) -> Result<(), PrintError> {
    let (tx, rx) = mpsc::channel();
    let gone = Arc::new(AtomicBool::new(false));
    start(Reply {
        tx,
        gone: Arc::clone(&gone),
        out: out.to_path_buf(),
    });
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => {
            gone.store(true, Ordering::SeqCst);
            Err(PrintError::new("the print did not finish in time"))
        }
    }
}

/// The `file://` URL of an absolute path, every byte outside the unreserved
/// set, `/` and `:` percent-encoded; on Windows `C:\a b` is `file:///C:/a%20b`.
pub fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut url = String::from("file://");
    if !text.starts_with('/') {
        url.push('/');
    }
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                url.push(char::from(b));
            }
            _ => {
                let _ = write!(url, "%{b:02X}");
            }
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_encode_spaces_and_non_ascii() {
        assert_eq!(
            file_url(Path::new("/tmp/Spec ü/a.html")),
            "file:///tmp/Spec%20%C3%BC/a.html"
        );
        assert_eq!(
            file_url(Path::new(r"C:\Users\A B\x#1.html")),
            "file:///C:/Users/A%20B/x%231.html"
        );
    }

    /// A scratch folder, removed at the end.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(label: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("fm-pdf-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        Scratch(dir)
    }

    #[test]
    fn a_print_that_ends_after_the_caller_gave_up_removes_its_file() {
        let dir = scratch("late");
        let out = dir.0.join(".Family.pdf.partial");
        let (go_tx, go_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel();
        let res = wait_for_ui_thread(&out, Duration::from_millis(50), |reply| {
            std::thread::spawn(move || {
                // The "UI thread": it started before the timeout and ends after it.
                go_rx.recv().expect("go");
                std::fs::write(reply.out(), b"%PDF-1.7 late").expect("write");
                reply.send(Ok(()));
                done_tx.send(()).expect("done");
            });
        });
        assert_eq!(
            res,
            Err(PrintError::new("the print did not finish in time"))
        );
        go_tx.send(()).expect("go");
        done_rx.recv().expect("done");
        assert!(!out.exists(), "no file is left behind after a timeout");
    }

    #[test]
    fn a_job_sees_the_caller_gone_before_it_starts_printing() {
        let dir = scratch("gone");
        let out = dir.0.join(".Family.pdf.partial");
        let (tx, rx) = mpsc::channel();
        let res = wait_for_ui_thread(&out, Duration::from_millis(20), |reply| {
            tx.send(reply).expect("reply");
        });
        assert!(res.is_err());
        let reply = rx.recv().expect("reply");
        assert!(
            reply.gone(),
            "a job that has not printed yet must not start"
        );
    }

    #[test]
    fn an_answer_in_time_reaches_the_caller_and_keeps_the_file() {
        let dir = scratch("in-time");
        let out = dir.0.join(".Family.pdf.partial");
        let res = wait_for_ui_thread(&out, Duration::from_secs(5), |reply| {
            std::fs::write(reply.out(), b"%PDF-1.7").expect("write");
            assert!(!reply.gone());
            reply.send(Ok(()));
        });
        assert_eq!(res, Ok(()));
        assert!(out.exists());
    }

    #[test]
    fn a4_is_210_by_297_millimetres() {
        assert!((A4_POINTS.0 - 595.28).abs() < 0.01);
        assert!((A4_POINTS.1 - 841.89).abs() < 0.01);
    }
}
