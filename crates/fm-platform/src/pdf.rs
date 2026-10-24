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
use std::path::Path;
use std::sync::Arc;
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

    #[test]
    fn a4_is_210_by_297_millimetres() {
        assert!((A4_POINTS.0 - 595.28).abs() < 0.01);
        assert!((A4_POINTS.1 - 841.89).abs() < 0.01);
    }
}
