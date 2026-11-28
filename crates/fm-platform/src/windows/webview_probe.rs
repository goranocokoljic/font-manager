//! Windows: draws a page in an offscreen WebView2 and reads its pixels
//! (E58.4, ADR 0029). Test support; see [`crate::webview_probe`].
//!
//! Like the PDF printer (ADR 0013) every snapshot runs on a thread of its
//! own in a single-threaded apartment, with a window that is never on
//! screen. The controller's rasterization scale is pinned to 1, so one pixel
//! of the capture is one CSS pixel at 100 per cent whatever the monitor's
//! scale. The webview's zoom is `ICoreWebView2Controller::SetZoomFactor`, the
//! call Tauri's `set_zoom` makes. `CapturePreview` writes a PNG, which WIC
//! decodes to BGRA.

use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, ICoreWebView2, ICoreWebView2Controller3,
};
use webview2_com::{CapturePreviewCompletedHandler, ExecuteScriptCompletedHandler};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IStream, STREAM_SEEK_SET,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, SW_SHOWNOACTIVATE, ShowWindow, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{HSTRING, Interface, w};

use super::pdf::{Controller, Window, controller, environment, navigate, wait_for_fonts};
use crate::webview_probe::{ProbeError, SNAPSHOT_TIMEOUT, Snapshot};

/// Counts three animation frames after the fonts loaded, so what is captured
/// is what the page drew after its last layout.
const FRAMES: &str = "window.__fmFrames=0;(function t(){window.__fmFrames++;\
if(window.__fmFrames<3)requestAnimationFrame(t)})();0";

pub(crate) fn snapshot_html(
    html: &Path,
    view: (u32, u32),
    page_zoom: f64,
) -> Result<Snapshot, ProbeError> {
    let dir = std::env::temp_dir().join(format!("fm-webview-probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir)
        .map_err(|e| ProbeError::new(format!("{}: {e}", dir.display())))?;
    let html = html.to_path_buf();
    std::thread::Builder::new()
        .name("fm-webview-probe".into())
        .spawn(move || {
            // SAFETY: this thread enters an STA once and leaves it below;
            // every COM object is dropped before `CoUninitialize`.
            let entered = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
            let result = snapshot_on_this_thread(&dir, &html, view, page_zoom);
            if entered {
                // SAFETY: balances the successful `CoInitializeEx` above.
                unsafe { CoUninitialize() };
            }
            result
        })
        .map_err(|e| ProbeError::new(format!("no probe thread: {e}")))?
        .join()
        .map_err(|_| ProbeError::new("the probe thread panicked"))?
}

fn probe_err(e: crate::pdf::PrintError) -> ProbeError {
    ProbeError::new(e.0)
}

fn snapshot_on_this_thread(
    dir: &Path,
    html: &Path,
    view: (u32, u32),
    page_zoom: f64,
) -> Result<Snapshot, ProbeError> {
    let (w, h) = (
        i32::try_from(view.0).map_err(|_| ProbeError::new("view too wide"))?,
        i32::try_from(view.1).map_err(|_| ProbeError::new("view too tall"))?,
    );
    // SAFETY: the predefined STATIC class needs no registration; the window
    // sits far off every monitor, never takes the focus, and is destroyed by
    // `Window`'s drop on this thread. It is shown, so the webview draws.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("STATIC"),
            w!("Font Fu probe"),
            WS_POPUP,
            -32000,
            -32000,
            w,
            h,
            None,
            None,
            None,
            None,
        )
    }
    .map_err(|e| ProbeError::new(format!("no probe window: {e}")))?;
    let window = Window(hwnd);
    // SAFETY: the window was made on this thread just above.
    let _ = unsafe { ShowWindow(window.0, SW_SHOWNOACTIVATE) };

    let env = environment(dir).map_err(probe_err)?;
    let controller = Controller(controller(&env, window.0).map_err(probe_err)?);
    let c3: ICoreWebView2Controller3 = controller
        .0
        .cast()
        .map_err(|e| ProbeError::new(format!("WebView2 is too old to pin the scale: {e}")))?;
    // SAFETY: calls on live interfaces of this thread.
    let webview = unsafe {
        c3.SetShouldDetectMonitorScaleChanges(false)
            .and_then(|()| c3.SetRasterizationScale(1.0))
            .map_err(|e| ProbeError::new(format!("WebView2 scale: {e}")))?;
        let _ = controller.0.SetBounds(RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        });
        controller
            .0
            .SetIsVisible(true)
            .map_err(|e| ProbeError::new(format!("WebView2 show: {e}")))?;
        controller.0.CoreWebView2()
    }
    .map_err(|e| ProbeError::new(format!("WebView2 view: {e}")))?;

    navigate(&webview, &crate::pdf::file_url(html)).map_err(probe_err)?;
    // SAFETY: a call on the live controller of this thread.
    unsafe { controller.0.SetZoomFactor(page_zoom) }
        .map_err(|e| ProbeError::new(format!("WebView2 zoom: {e}")))?;
    wait_for_fonts(&webview).map_err(probe_err)?;
    script(&webview, FRAMES)?;
    let deadline = Instant::now() + SNAPSHOT_TIMEOUT;
    while script(&webview, "window.__fmFrames")? != "3" {
        if Instant::now() > deadline {
            return Err(ProbeError::new("the page drew no frames in time"));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let inner = script(&webview, "window.innerWidth")?;
    let inner_width = inner
        .parse::<f64>()
        .map_err(|_| ProbeError::new(format!("innerWidth: {inner}")))?;
    let mut snap = capture(&webview)?;
    snap.inner_width = inner_width;
    Ok(snap)
}

/// Runs `js` and returns its result as JSON.
fn script(webview: &ICoreWebView2, js: &str) -> Result<String, ProbeError> {
    let (tx, rx) = mpsc::channel();
    let view = webview.clone();
    let js = HSTRING::from(js);
    ExecuteScriptCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| {
            // SAFETY: the script string and the handler outlive the call.
            unsafe { view.ExecuteScript(&js, &handler) }.map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |hr, json| {
            hr?;
            let _ = tx.send(json);
            Ok(())
        }),
    )
    .map_err(|e| ProbeError::new(format!("WebView2 script: {e}")))?;
    rx.recv()
        .map_err(|_| ProbeError::new("WebView2 script: no answer"))
}

/// Captures the visible page as a PNG in memory and decodes it.
fn capture(webview: &ICoreWebView2) -> Result<Snapshot, ProbeError> {
    // SAFETY: a new stream that frees its memory on release.
    let stream: IStream = unsafe { CreateStreamOnHGlobal(Default::default(), true) }
        .map_err(|e| ProbeError::new(format!("no stream: {e}")))?;
    let (view, target) = (webview.clone(), stream.clone());
    CapturePreviewCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| {
            // SAFETY: the stream and the handler outlive the call.
            unsafe {
                view.CapturePreview(
                    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                    &target,
                    &handler,
                )
            }
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(|hr| hr),
    )
    .map_err(|e| ProbeError::new(format!("WebView2 capture: {e}")))?;
    decode_png(&stream)
}

fn decode_png(stream: &IStream) -> Result<Snapshot, ProbeError> {
    let wic = |e: windows::core::Error| ProbeError::new(format!("WIC: {e}"));
    // SAFETY: COM calls on this thread's apartment; `pixels` is sized to
    // `stride * height` before `CopyPixels` writes into it.
    unsafe {
        stream.Seek(0, STREAM_SEEK_SET, None).map_err(wic)?;
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(wic)?;
        let decoder = factory
            .CreateDecoderFromStream(stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .map_err(wic)?;
        let frame = decoder.GetFrame(0).map_err(wic)?;
        let converter = factory.CreateFormatConverter().map_err(wic)?;
        converter
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(wic)?;
        let (mut width, mut height) = (0u32, 0u32);
        converter.GetSize(&mut width, &mut height).map_err(wic)?;
        let stride = width as usize * 4;
        let mut pixels = vec![0u8; stride * height as usize];
        converter
            .CopyPixels(
                std::ptr::null(),
                u32::try_from(stride).map_err(|_| ProbeError::new("image too wide"))?,
                &mut pixels,
            )
            .map_err(wic)?;
        Ok(Snapshot::from_rgba(width, height, stride, &pixels, true))
    }
}
