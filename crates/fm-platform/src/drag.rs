//! Dragging font files out of the window into another app (E44.4, ADR 0022):
//! a native OS file drag of the files where the user keeps them, never a copy
//! (rule 2). The app that receives the drop gets the files as a copy: the drag
//! offers only the copy operation. One implementation per OS:
//!
//! - Windows (`src/windows/drag.rs`): the shell's data object for the files
//!   (`CF_HDROP` and the shell ID list) through `SHDoDragDrop`.
//! - macOS (`src/macos/drag.rs`): one `NSDraggingItem` per file, its
//!   pasteboard writer the file URL, from the key window's content view.
//! - Linux (`src/linux/drag.rs`): a GTK drag from the active window offering
//!   `text/uri-list` (X11 and Wayland alike).
//!
//! [`FileDrag::start`] runs on the UI thread, while the primary mouse button
//! is down: the app calls it from its main thread when the pointer leaves the
//! window. On Windows it returns once the files are dropped or the drag is
//! cancelled; on macOS and Linux once the drag has begun.

use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Whether files can be dragged out of the window here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DragSupport {
    /// The drag works.
    Available,
    /// It does not, and why (one sentence for a tooltip).
    Unavailable(String),
}

/// Why a drag did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragError(pub String);

impl DragError {
    /// An error with this developer detail.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }
}

impl fmt::Display for DragError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DragError {}

/// Starts a native file drag out of the window.
pub trait FileDrag: Send + Sync + fmt::Debug {
    /// Whether the drag works on this system.
    fn support(&self) -> DragSupport;

    /// Starts a drag of `files` (absolute paths, each once) that other apps
    /// receive as a copy. Call it on the UI thread while the primary mouse
    /// button is down. An empty list is an error; nothing is copied or moved.
    fn start(&self, files: &[PathBuf]) -> Result<(), DragError>;
}

/// The OS file drag.
pub fn system() -> Arc<dyn FileDrag> {
    #[cfg(windows)]
    return Arc::new(crate::windows::WindowsFileDrag);
    #[cfg(target_os = "macos")]
    return Arc::new(crate::macos::MacosFileDrag);
    #[cfg(target_os = "linux")]
    return Arc::new(crate::linux::LinuxFileDrag);
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    Arc::new(MockFileDrag::unavailable(
        "Dragging font files out is not supported on this system.",
    ))
}

/// Refuses an empty drag, the same on every OS.
pub(crate) fn check_files(files: &[PathBuf]) -> Result<(), DragError> {
    if files.is_empty() {
        return Err(DragError::new("there are no files to drag"));
    }
    if let Some(relative) = files.iter().find(|f| !f.is_absolute()) {
        return Err(DragError::new(format!(
            "{} is not an absolute path",
            relative.display()
        )));
    }
    Ok(())
}

/// A `text/uri-list` body (RFC 2483) for `files`: one `file://` URL per
/// line, each line ending in CRLF.
pub fn uri_list(files: &[PathBuf]) -> String {
    let mut out = String::new();
    for f in files {
        let _ = write!(out, "{}\r\n", file_uri(f));
    }
    out
}

/// The `file://` URL of `path`, as a URI list carries it: every byte outside
/// the unreserved set, `/` and `:` percent-encoded. On Unix the path's own
/// bytes are encoded, so a name that is not UTF-8 survives the drag; on
/// Windows `C:\a b` is `file:///C:/a%20b`.
pub fn file_uri(path: &Path) -> String {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt as _;
        path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(not(unix))]
    let bytes = path.to_string_lossy().replace('\\', "/").into_bytes();
    let mut url = String::from("file://");
    if bytes.first() != Some(&b'/') {
        url.push('/');
    }
    for b in bytes {
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

/// A drag that records what it was asked to drag; for tests and hosts
/// without a window.
#[derive(Debug)]
pub struct MockFileDrag {
    support: DragSupport,
    started: Mutex<Vec<Vec<PathBuf>>>,
}

impl Default for MockFileDrag {
    fn default() -> Self {
        Self {
            support: DragSupport::Available,
            started: Mutex::new(Vec::new()),
        }
    }
}

impl MockFileDrag {
    /// A drag that is not available, for `reason`.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            support: DragSupport::Unavailable(reason.into()),
            started: Mutex::new(Vec::new()),
        }
    }

    /// The file lists of the drags started so far.
    pub fn started(&self) -> Vec<Vec<PathBuf>> {
        self.started
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|e| e.into_inner().clone())
    }
}

impl FileDrag for MockFileDrag {
    fn support(&self) -> DragSupport {
        self.support.clone()
    }

    fn start(&self, files: &[PathBuf]) -> Result<(), DragError> {
        if let DragSupport::Unavailable(reason) = &self.support {
            return Err(DragError::new(reason.clone()));
        }
        check_files(files)?;
        self.started
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(files.to_vec());
        Ok(())
    }
}

/// What another process read from a drag's data ([`probe`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// The files, in the order the drag offered them.
    pub paths: Vec<PathBuf>,
    /// Whether the data asks the target to copy (Windows' "Preferred
    /// DropEffect"); `None` where the data does not say (macOS and Linux
    /// state the copy operation in the drag itself).
    pub copy: Option<bool>,
}

/// The cross-process check of the drag's data (E44.4): one process offers
/// exactly the data a drag of the files carries where a second process can
/// read it, the way the drop target reads it; the second process reads it
/// back. A real mouse drag cannot be driven on a CI runner, so the tests
/// check the data the drag hands to the OS instead.
///
/// - Windows: the drag's data object on the OLE clipboard (`CF_HDROP` and
///   "Preferred DropEffect").
/// - macOS: the drag items' pasteboard writers on a named pasteboard
///   (`public.file-url`).
/// - Linux: the drag's `text/uri-list` answer served on a named X selection.
#[doc(hidden)]
pub mod probe {
    use super::{DragError, Received};
    use std::path::PathBuf;

    /// Offers the data a drag of `files` carries and runs `read` (which
    /// starts the second process) with the channel name to read it from,
    /// serving the data until `read` returns. Call it from a test's thread
    /// (it runs the GTK loop on Linux, so only once per process there).
    pub fn share<T: Send + 'static>(
        files: &[PathBuf],
        read: impl FnOnce(String) -> T + Send + 'static,
    ) -> Result<T, DragError> {
        super::check_files(files)?;
        #[cfg(windows)]
        return crate::windows::drag::share(files, read);
        #[cfg(target_os = "macos")]
        return crate::macos::drag::share(files, read);
        #[cfg(target_os = "linux")]
        return crate::linux::drag::share(files, read);
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            let _ = read;
            Err(DragError::new("no file drag on this OS"))
        }
    }

    /// Reads what [`share`] offers on `channel`, in another process.
    pub fn receive(channel: &str) -> Result<Received, DragError> {
        #[cfg(windows)]
        return crate::windows::drag::receive(channel);
        #[cfg(target_os = "macos")]
        return crate::macos::drag::receive(channel);
        #[cfg(target_os = "linux")]
        return crate::linux::drag::receive(channel);
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            let _ = channel;
            Err(DragError::new("no file drag on this OS"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(p: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!(r"C:\{}", p.replace('/', r"\")))
        } else {
            PathBuf::from(format!("/{p}"))
        }
    }

    #[test]
    fn an_empty_or_relative_drag_is_refused() {
        assert_eq!(
            check_files(&[]),
            Err(DragError::new("there are no files to drag"))
        );
        assert!(check_files(&[PathBuf::from("Fonts/a.ttf")]).is_err());
        assert_eq!(check_files(&[abs("Fonts/a.ttf")]), Ok(()));
    }

    #[test]
    fn a_uri_list_has_one_crlf_terminated_file_url_per_file() {
        let list = uri_list(&[abs("Fonts/A b.ttf"), abs("Fonts/ü.otf")]);
        let lines: Vec<&str> = list.split_terminator("\r\n").collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("file:///") && lines[0].ends_with("/Fonts/A%20b.ttf"));
        assert!(lines[1].ends_with("/Fonts/%C3%BC.otf"));
        assert!(list.ends_with("\r\n"));
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_utf8_keeps_its_bytes() {
        use std::os::unix::ffi::OsStrExt as _;
        let path = Path::new(std::ffi::OsStr::from_bytes(b"/tmp/a\xffb.ttf"));
        assert_eq!(file_uri(path), "file:///tmp/a%FFb.ttf");
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_path_becomes_a_three_slash_url() {
        assert_eq!(
            file_uri(Path::new(r"C:\Users\A B\x#1.ttf")),
            "file:///C:/Users/A%20B/x%231.ttf"
        );
    }

    #[test]
    fn the_mock_records_drags_and_an_unavailable_one_refuses() {
        let drag = MockFileDrag::default();
        assert_eq!(drag.support(), DragSupport::Available);
        drag.start(&[abs("a.ttf")]).expect("started");
        assert_eq!(drag.started(), vec![vec![abs("a.ttf")]]);
        let off = MockFileDrag::unavailable("no display");
        assert_eq!(
            off.start(&[abs("a.ttf")]),
            Err(DragError::new("no display"))
        );
        assert!(off.started().is_empty());
    }

    #[test]
    fn the_system_drag_is_available_on_the_three_platforms() {
        if cfg!(any(windows, target_os = "macos")) {
            assert_eq!(system().support(), DragSupport::Available);
        }
    }
}
