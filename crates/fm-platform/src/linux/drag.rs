//! The Linux file drag (E44.4, ADR 0022): a GTK 3 drag from the app's active
//! window offering `text/uri-list`, the target every file manager and design
//! app reads. GTK speaks XDND on X11 and the data-device protocol on Wayland,
//! so no portal is needed. The drag allows only `GDK_ACTION_COPY`, so
//! Nautilus and every other target copy the files; the originals stay where
//! they are.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::drag::{DragError, DragSupport, FileDrag, Received, check_files, file_uri};

/// The only operation the drag allows: the target copies the files.
pub(crate) const ALLOWED: gdk::DragAction = gdk::DragAction::COPY;

/// The one target the drag offers.
const URI_LIST: &str = "text/uri-list";

/// The Linux file drag.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxFileDrag;

impl FileDrag for LinuxFileDrag {
    fn support(&self) -> DragSupport {
        support_with(|name| std::env::var_os(name))
    }

    fn start(&self, files: &[PathBuf]) -> Result<(), DragError> {
        check_files(files)?;
        if !gtk::is_initialized_main_thread() {
            return Err(DragError::new("a file drag must start on the GTK thread"));
        }
        let window =
            active_window().ok_or_else(|| DragError::new("there is no window to drag from"))?;
        let uris = uris(files);
        let handlers: Rc<RefCell<Vec<glib::SignalHandlerId>>> = Rc::default();
        let get = window.connect_drag_data_get(move |_, _, data, _, _| {
            fill(data, &uris);
        });
        let weak = window.downgrade();
        let ids = Rc::clone(&handlers);
        let end = window.connect_drag_end(move |_, _| {
            // The drag is over (dropped or cancelled): stop answering for it.
            if let Some(window) = weak.upgrade() {
                for id in ids.borrow_mut().drain(..) {
                    window.disconnect(id);
                }
            }
        });
        handlers.borrow_mut().extend([get, end]);
        let targets = gtk::TargetList::new(&[gtk::TargetEntry::new(
            URI_LIST,
            gtk::TargetFlags::OTHER_APP,
            0,
        )]);
        // Button 1, no event: GTK takes the pointer's current device and position.
        if window
            .drag_begin_with_coordinates(&targets, ALLOWED, 1, None, -1, -1)
            .is_none()
        {
            for id in handlers.borrow_mut().drain(..) {
                window.disconnect(id);
            }
            return Err(DragError::new("GTK did not start the drag"));
        }
        Ok(())
    }
}

/// Available with an X11 or Wayland display, `var` reading the environment.
pub(crate) fn support_with(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> DragSupport {
    let set = |name: &str| var(name).is_some_and(|v| !v.is_empty());
    if set("WAYLAND_DISPLAY") || set("DISPLAY") {
        DragSupport::Available
    } else {
        DragSupport::Unavailable(
            "Dragging font files out needs a graphical session (X11 or Wayland).".into(),
        )
    }
}

/// The app's window the pointer drags in: the active toplevel, else the
/// first visible one.
fn active_window() -> Option<gtk::Window> {
    let windows: Vec<gtk::Window> = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .filter(|w| w.is_visible())
        .collect();
    windows
        .iter()
        .find(|w| w.is_active())
        .or_else(|| windows.first())
        .cloned()
}

fn uris(files: &[PathBuf]) -> Vec<String> {
    files.iter().map(|f| file_uri(f)).collect()
}

/// Answers a request for the drag's data with the files' URIs.
fn fill(data: &gtk::SelectionData, uris: &[String]) {
    let refs: Vec<&str> = uris.iter().map(String::as_str).collect();
    let _ = data.set_uris(&refs);
}

/// Serves the drag's `text/uri-list` answer for `files` on a named X
/// selection and runs `read` with its name, running the GTK loop meanwhile.
pub(crate) fn share<T: Send + 'static>(
    files: &[PathBuf],
    read: impl FnOnce(String) -> T + Send + 'static,
) -> Result<T, DragError> {
    gtk::init().map_err(|e| DragError::new(format!("GTK did not start: {e}")))?;
    let name = format!("FM_FILE_DRAG_PROBE_{}", std::process::id());
    let clipboard = gtk::Clipboard::get(&gdk::Atom::intern(&name));
    let uris = uris(files);
    let served = clipboard.set_with_data(
        &[gtk::TargetEntry::new(
            URI_LIST,
            gtk::TargetFlags::empty(),
            0,
        )],
        move |_, data, _| fill(data, &uris),
    );
    if !served {
        return Err(DragError::new("the selection could not be taken"));
    }
    let ctx = glib::MainContext::default();
    let worker = std::thread::spawn(move || read(name));
    while !worker.is_finished() {
        if !ctx.iteration(false) {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    clipboard.clear();
    match worker.join() {
        Ok(value) => Ok(value),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// Reads the URI list served on the selection `channel`.
pub(crate) fn receive(channel: &str) -> Result<Received, DragError> {
    gtk::init().map_err(|e| DragError::new(format!("GTK did not start: {e}")))?;
    let clipboard = gtk::Clipboard::get(&gdk::Atom::intern(channel));
    let uris = clipboard.wait_for_uris();
    if uris.is_empty() {
        return Err(DragError::new("no URI list on the selection"));
    }
    let paths = uris
        .iter()
        .map(|u| {
            glib::filename_from_uri(u)
                .map(|(path, _host)| path)
                .map_err(|e| DragError::new(format!("{u} is not a file URI: {e}")))
        })
        .collect::<Result<_, _>>()?;
    Ok(Received { paths, copy: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_copy_operation_is_allowed() {
        assert_eq!(ALLOWED, gdk::DragAction::COPY);
    }

    #[test]
    fn without_a_display_the_drag_is_unavailable_and_says_why() {
        assert!(matches!(
            support_with(|_| None),
            DragSupport::Unavailable(reason) if reason.contains("graphical session")
        ));
        let wayland = |n: &str| (n == "WAYLAND_DISPLAY").then(|| "wayland-0".into());
        assert_eq!(support_with(wayland), DragSupport::Available);
        let x11 = |n: &str| (n == "DISPLAY").then(|| ":0".into());
        assert_eq!(support_with(x11), DragSupport::Available);
        let empty = |n: &str| (n == "DISPLAY").then(std::ffi::OsString::new);
        assert!(matches!(support_with(empty), DragSupport::Unavailable(_)));
    }
}
