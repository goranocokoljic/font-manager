//! Test support for the interface size (E58.4, ADR 0029): renders a local
//! HTML page in an offscreen webview of the OS — WebView2, WKWebView,
//! WebKitGTK, the engines the app ships (ADR 0001) — at a page zoom, and
//! returns what it drew as pixels. A test measures the ink of a specimen in
//! those pixels, so "the same size" means the same pixels on the screen, not
//! the same CSS numbers, which each engine reports in its own way under zoom.
//!
//! The page zoom is the call Tauri's `WebviewWindow::set_zoom` makes on each
//! OS (wry): `ICoreWebView2Controller::SetZoomFactor`, `WKWebView.pageZoom`,
//! `webkit_web_view_set_zoom_level`. The pixels are taken at a scale of 1, so
//! one pixel is one CSS pixel at 100 per cent.
//!
//! This is not an app feature: nothing in the app calls it. Each OS has its
//! own file under `src/{windows,macos,linux}/webview_probe.rs`. The threading
//! rules are those of the PDF printer (ADR 0013): call it from a worker
//! thread while [`crate::pdf::run_ui_loop`] runs the UI loop.

use std::fmt;
use std::path::Path;
use std::time::Duration;

/// How long one snapshot may take before the probe gives up.
pub const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(60);

/// Why a page was not drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeError(pub String);

impl ProbeError {
    /// An error with this developer detail.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProbeError {}

/// What the webview drew: `width × height` pixels, one byte of luminance
/// each (0 black, 255 white), row by row from the top.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Pixels per row.
    pub width: u32,
    /// Rows.
    pub height: u32,
    /// `width * height` luminance bytes.
    pub luma: Vec<u8>,
    /// The page's `window.innerWidth` in CSS pixels when it was drawn: the
    /// view's width divided by the zoom the webview applied, which proves
    /// the zoom took effect.
    pub inner_width: f64,
}

/// A box of pixels: `x`, `y` of its top left corner, its width and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelBox {
    /// Left edge, in pixels.
    pub x: u32,
    /// Top edge, in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// A pixel darker than this is ink. Anti-aliased edges lighter than it
/// are not, so one threshold measures every engine alike.
pub const INK: u8 = 128;

impl Snapshot {
    /// A snapshot from RGBA or BGRA bytes (`stride` bytes per row, four per
    /// pixel); `bgra` says which order the channels are in. Alpha is ignored:
    /// the probe's pages are opaque.
    pub fn from_rgba(width: u32, height: u32, stride: usize, bytes: &[u8], bgra: bool) -> Self {
        let (r, b) = if bgra { (2, 0) } else { (0, 2) };
        let mut luma = Vec::with_capacity(width as usize * height as usize);
        for row in 0..height as usize {
            let line = &bytes[row * stride..];
            for col in 0..width as usize {
                let p = &line[col * 4..col * 4 + 4];
                let y =
                    (u32::from(p[r]) * 299 + u32::from(p[1]) * 587 + u32::from(p[b]) * 114) / 1000;
                luma.push(u8::try_from(y).unwrap_or(u8::MAX));
            }
        }
        Self {
            width,
            height,
            luma,
            inner_width: 0.0,
        }
    }

    fn ink(&self, x: u32, y: u32) -> bool {
        self.luma[y as usize * self.width as usize + x as usize] < INK
    }

    /// The smallest box that holds every ink pixel, or `None` on a blank page.
    pub fn ink_box(&self) -> Option<PixelBox> {
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.ink(x, y) {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        (x0 != u32::MAX).then(|| PixelBox {
            x: x0,
            y: y0,
            width: x1 - x0 + 1,
            height: y1 - y0 + 1,
        })
    }

    /// The bands of rows that hold ink, top to bottom, each with the width
    /// of its own ink: one band per line of text on a page whose lines do
    /// not touch (the waterfall's ladder, one size per band).
    pub fn ink_bands(&self) -> Vec<PixelBox> {
        let mut bands = Vec::new();
        let mut open: Option<(u32, u32, u32)> = None; // top, left, right
        for y in 0..=self.height {
            let row = (y < self.height)
                .then(|| {
                    let xs = (0..self.width).filter(|&x| self.ink(x, y));
                    xs.fold(None, |acc: Option<(u32, u32)>, x| {
                        Some(acc.map_or((x, x), |(l, r)| (l.min(x), r.max(x))))
                    })
                })
                .flatten();
            match (row, open) {
                (Some((l, r)), Some((top, ol, or))) => open = Some((top, ol.min(l), or.max(r))),
                (Some((l, r)), None) => open = Some((y, l, r)),
                (None, Some((top, l, r))) => {
                    bands.push(PixelBox {
                        x: l,
                        y: top,
                        width: r - l + 1,
                        height: y - top,
                    });
                    open = None;
                }
                (None, None) => {}
            }
        }
        bands
    }
}

/// Loads `html` (a local file; everything it shows is in it) in an offscreen
/// webview of `view` CSS pixels at 100 per cent, sets the webview's own zoom
/// to `page_zoom` (1.0 is none), waits until the page stopped loading and its
/// fonts are loaded, and returns the visible part as pixels at a scale of 1.
/// Blocks; never call it from the UI thread.
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
pub fn snapshot_html(
    html: &Path,
    view: (u32, u32),
    page_zoom: f64,
) -> Result<Snapshot, ProbeError> {
    #[cfg(windows)]
    return crate::windows::webview_probe::snapshot_html(html, view, page_zoom);
    #[cfg(target_os = "macos")]
    return crate::macos::webview_probe::snapshot_html(html, view, page_zoom);
    #[cfg(target_os = "linux")]
    return crate::linux::webview_probe::snapshot_html(html, view, page_zoom);
}

/// No webview on an OS the app does not support.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn snapshot_html(
    _html: &Path,
    _view: (u32, u32),
    _page_zoom: f64,
) -> Result<Snapshot, ProbeError> {
    Err(ProbeError::new("no webview on this OS"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A white page of `w × h` with black rectangles `(x, y, w, h)`.
    fn page(w: u32, h: u32, rects: &[(u32, u32, u32, u32)]) -> Snapshot {
        let mut luma = vec![255u8; (w * h) as usize];
        for &(rx, ry, rw, rh) in rects {
            for y in ry..ry + rh {
                for x in rx..rx + rw {
                    luma[(y * w + x) as usize] = 0;
                }
            }
        }
        Snapshot {
            width: w,
            height: h,
            luma,
            inner_width: 0.0,
        }
    }

    #[test]
    fn the_ink_box_holds_every_dark_pixel() {
        let s = page(20, 10, &[(2, 3, 4, 2), (10, 6, 3, 3)]);
        assert_eq!(
            s.ink_box(),
            Some(PixelBox {
                x: 2,
                y: 3,
                width: 11,
                height: 6
            })
        );
    }

    #[test]
    fn a_blank_page_has_no_ink() {
        assert_eq!(page(5, 5, &[]).ink_box(), None);
        assert!(page(5, 5, &[]).ink_bands().is_empty());
    }

    #[test]
    fn bands_split_at_blank_rows_and_close_at_the_bottom() {
        let s = page(20, 12, &[(1, 0, 2, 2), (5, 4, 6, 3), (0, 9, 20, 3)]);
        let bands = s.ink_bands();
        assert_eq!(bands.len(), 3);
        assert_eq!((bands[0].y, bands[0].height, bands[0].width), (0, 2, 2));
        assert_eq!((bands[1].x, bands[1].width, bands[1].height), (5, 6, 3));
        assert_eq!((bands[2].y, bands[2].height, bands[2].width), (9, 3, 20));
    }

    #[test]
    fn grey_anti_aliasing_lighter_than_the_threshold_is_not_ink() {
        let mut s = page(4, 1, &[(0, 0, 1, 1)]);
        s.luma[1] = INK; // exactly the threshold: not ink
        s.luma[2] = INK - 1;
        assert_eq!(s.ink_box().map(|b| b.width), Some(3));
    }

    #[test]
    fn rgba_and_bgra_rows_with_padding_read_alike() {
        // Two pixels per row, a stride of 12 (4 bytes of padding), two rows.
        let red_white = [255, 0, 0, 255, 255, 255, 255, 255, 9, 9, 9, 9];
        let rgba: Vec<u8> = [red_white, red_white].concat();
        let blue_white = [0, 0, 255, 255, 255, 255, 255, 255, 9, 9, 9, 9];
        let bgra: Vec<u8> = [blue_white, blue_white].concat();
        let a = Snapshot::from_rgba(2, 2, 12, &rgba, false);
        let b = Snapshot::from_rgba(2, 2, 12, &bgra, true);
        assert_eq!(a, b);
        assert_eq!(a.luma, vec![76, 255, 76, 255]);
    }
}
