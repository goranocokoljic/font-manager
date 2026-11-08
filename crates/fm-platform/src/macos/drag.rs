//! The macOS file drag (E44.4, ADR 0022): one `NSDraggingItem` per file, its
//! pasteboard writer the file's URL (`public.file-url`), the file's Finder
//! icon as the image, begun from the key window's content view. The source
//! allows only `NSDragOperationCopy`, so Finder and every other target copy
//! the files; the originals stay where they are.

use std::cell::OnceCell;
use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{AllocAnyThread, MainThreadMarker, MainThreadOnly, define_class};
use objc2_app_kit::{
    NSApplication, NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession,
    NSDraggingSource, NSEvent, NSEventModifierFlags, NSEventType, NSPasteboard,
    NSPasteboardTypeFileURL, NSPasteboardWriting, NSWindow, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSObject, NSPoint, NSProcessInfo, NSRect, NSSize, NSString, NSURL,
};

use crate::drag::{DragError, DragSupport, FileDrag, Received, check_files};

/// The only operation the drag allows: the target copies the files.
pub(crate) const ALLOWED: NSDragOperation = NSDragOperation::Copy;

/// The side of a file's icon under the pointer, in points.
const ICON: f64 = 32.0;

/// The macOS file drag.
#[derive(Debug, Clone, Copy, Default)]
pub struct MacosFileDrag;

impl FileDrag for MacosFileDrag {
    fn support(&self) -> DragSupport {
        DragSupport::Available
    }

    fn start(&self, files: &[PathBuf]) -> Result<(), DragError> {
        check_files(files)?;
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| DragError::new("a file drag must start on the main thread"))?;
        let app = NSApplication::sharedApplication(mtm);
        let window = app
            .keyWindow()
            .or_else(|| app.mainWindow())
            .ok_or_else(|| DragError::new("there is no window to drag from"))?;
        let view = window
            .contentView()
            .ok_or_else(|| DragError::new("the window has no content view"))?;
        let event = drag_event(&app, &window)?;
        let at = view.convertPoint_fromView(event.locationInWindow(), None);
        let workspace = NSWorkspace::sharedWorkspace();
        let mut items = Vec::with_capacity(files.len());
        for (i, (file, url)) in files.iter().zip(writers(files)?).enumerate() {
            let item = NSDraggingItem::initWithPasteboardWriter(
                NSDraggingItem::alloc(),
                ProtocolObject::from_ref(&*url),
            );
            let path = ns_path(file)?;
            let icon = workspace.iconForFile(&path);
            let step = 6.0 * f64::from(u16::try_from(i.min(8)).unwrap_or(8));
            let frame = NSRect::new(
                NSPoint::new(at.x - ICON / 2.0 + step, at.y - ICON / 2.0 - step),
                NSSize::new(ICON, ICON),
            );
            let contents: &AnyObject = &icon;
            // SAFETY: an NSImage is a valid dragging item's contents.
            unsafe { item.setDraggingFrame_contents(frame, Some(contents)) };
            items.push(item);
        }
        let items = NSArray::from_retained_slice(&items);
        let source = SOURCE.with(|s| s.get_or_init(|| DragSource::new(mtm)).clone());
        let _session = view.beginDraggingSessionWithItems_event_source(
            &items,
            &event,
            ProtocolObject::from_ref(&*source),
        );
        Ok(())
    }
}

thread_local! {
    /// The drag source, made once on the main thread and kept: a dragging
    /// session does not retain its source.
    static SOURCE: OnceCell<Retained<DragSource>> = const { OnceCell::new() };
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the class has no `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FmFileDragSource"]
    struct DragSource;

    unsafe impl NSObjectProtocol for DragSource {}

    unsafe impl NSDraggingSource for DragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn operation_mask(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            ALLOWED
        }
    }
);

impl DragSource {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's `init` on a freshly allocated instance.
        unsafe { objc2::msg_send![super(this), init] }
    }
}

/// The mouse event the drag begins with: the current one while the button is
/// down, else a dragged event at the pointer (the call came through the
/// app's main-thread queue, between events).
fn drag_event(app: &NSApplication, window: &NSWindow) -> Result<Retained<NSEvent>, DragError> {
    if let Some(event) = app.currentEvent() {
        let kind = event.r#type();
        if kind == NSEventType::LeftMouseDown || kind == NSEventType::LeftMouseDragged {
            return Ok(event);
        }
    }
    NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::LeftMouseDragged,
        window.mouseLocationOutsideOfEventStream(),
        NSEventModifierFlags::empty(),
        NSProcessInfo::processInfo().systemUptime(),
        window.windowNumber(),
        None,
        0,
        1,
        1.0,
    )
    .ok_or_else(|| DragError::new("no mouse event to begin the drag with"))
}

/// The path as an `NSString`; macOS paths are UTF-8.
fn ns_path(file: &Path) -> Result<Retained<NSString>, DragError> {
    file.to_str()
        .map(NSString::from_str)
        .ok_or_else(|| DragError::new(format!("{} is not a UTF-8 path", file.display())))
}

/// The pasteboard writers of the drag: each file's URL.
pub(crate) fn writers(files: &[PathBuf]) -> Result<Vec<Retained<NSURL>>, DragError> {
    files
        .iter()
        .map(|f| ns_path(f).map(|p| NSURL::fileURLWithPath(&p)))
        .collect()
}

/// Writes the drag's pasteboard writers for `files` to a new named
/// pasteboard and runs `read` with its name.
pub(crate) fn share<T>(files: &[PathBuf], read: impl FnOnce(String) -> T) -> Result<T, DragError> {
    let urls = writers(files)?;
    let objects: Vec<&ProtocolObject<dyn NSPasteboardWriting>> = urls
        .iter()
        .map(|u| ProtocolObject::from_ref(&**u))
        .collect();
    let pasteboard = NSPasteboard::pasteboardWithUniqueName();
    pasteboard.clearContents();
    if !pasteboard.writeObjects(&NSArray::from_slice(&objects)) {
        return Err(DragError::new("the pasteboard did not take the file URLs"));
    }
    let out = read(pasteboard.name().to_string());
    pasteboard.clearContents();
    Ok(out)
}

/// Reads the file URLs from the named pasteboard `channel`.
pub(crate) fn receive(channel: &str) -> Result<Received, DragError> {
    let pasteboard = NSPasteboard::pasteboardWithName(&NSString::from_str(channel));
    let items = pasteboard
        .pasteboardItems()
        .ok_or_else(|| DragError::new("the pasteboard is empty"))?;
    let mut paths = Vec::new();
    for i in 0..items.count() {
        let item = items.objectAtIndex(i);
        // SAFETY: a constant AppKit defines for the life of the process.
        let kind = unsafe { NSPasteboardTypeFileURL };
        let text = item
            .stringForType(kind)
            .ok_or_else(|| DragError::new("an item has no file URL"))?;
        let path = NSURL::URLWithString(&text)
            .and_then(|u| u.path())
            .ok_or_else(|| DragError::new(format!("{text} is not a file URL")))?;
        paths.push(PathBuf::from(path.to_string()));
    }
    Ok(Received { paths, copy: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_copy_operation_is_allowed() {
        assert_eq!(ALLOWED, NSDragOperation::Copy);
    }

    #[test]
    fn each_file_is_written_as_its_file_url() {
        let urls = writers(&[PathBuf::from("/tmp/Fonts/A b.ttf")]).expect("urls");
        assert_eq!(
            urls[0].absoluteString().map(|s| s.to_string()).as_deref(),
            Some("file:///tmp/Fonts/A%20b.ttf")
        );
    }
}
