//! Windows: prints a page to PDF with WebView2's `PrintToPdf` (ADR 0013).
//!
//! Every print runs on a thread of its own: it enters a single-threaded COM
//! apartment, makes a hidden window, a WebView2 environment over the
//! printer's data folder and a controller in that window, loads the page,
//! waits for its fonts, prints, and closes everything again. WebView2 calls
//! back through window messages, so the thread pumps them while it waits
//! ([`webview2_com::wait_with_pump`]). The app's own webview is never used,
//! so the UI thread never waits on a print.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT, CreateCoreWebView2EnvironmentWithOptions,
    ICoreWebView2, ICoreWebView2_7, ICoreWebView2Controller, ICoreWebView2Environment,
    ICoreWebView2Environment6, ICoreWebView2EnvironmentOptions,
};
use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    ExecuteScriptCompletedHandler, NavigationCompletedEventHandler, PrintToPdfCompletedHandler,
};
use windows::Win32::Foundation::{E_POINTER, HWND, RECT};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
};
use windows::core::{BOOL, HSTRING, Interface, PCWSTR, w};

use crate::pdf::{A4_INCHES, MARGIN_MM, PRINT_TIMEOUT, PdfPrinter, PrintError, file_url};

/// The page's viewport while it is printed, in pixels.
const VIEW: (i32, i32) = (1280, 800);

/// Prints through WebView2, its data in `data_dir`.
#[derive(Debug, Clone)]
pub struct WindowsPdfPrinter {
    data_dir: PathBuf,
}

impl WindowsPdfPrinter {
    /// A printer whose WebView2 profile lives in `data_dir`; it is made on
    /// the first print.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }
}

impl PdfPrinter for WindowsPdfPrinter {
    fn print_html(&self, html: &Path, out: &Path) -> Result<(), PrintError> {
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| PrintError::new(format!("{}: {e}", self.data_dir.display())))?;
        let (dir, html, out) = (self.data_dir.clone(), html.to_path_buf(), out.to_path_buf());
        std::thread::Builder::new()
            .name("fm-pdf".into())
            .spawn(move || {
                // SAFETY: this thread enters an STA once and leaves it below;
                // every COM object is dropped before `CoUninitialize`.
                let entered = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
                let result = print_on_this_thread(&dir, &html, &out);
                if entered {
                    // SAFETY: balances the successful `CoInitializeEx` above.
                    unsafe { CoUninitialize() };
                }
                result
            })
            .map_err(|e| PrintError::new(format!("no print thread: {e}")))?
            .join()
            .map_err(|_| PrintError::new("the print thread panicked"))?
    }
}

/// A hidden top-level window, destroyed on drop.
struct Window(HWND);

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: the window was made on this thread and is destroyed once.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

/// The controller, closed on drop so the browser process ends.
struct Controller(ICoreWebView2Controller);

impl Drop for Controller {
    fn drop(&mut self) {
        // SAFETY: a live controller of this thread.
        let _ = unsafe { self.0.Close() };
    }
}

fn err(what: &str) -> impl Fn(webview2_com::Error) -> PrintError + '_ {
    move |e| PrintError::new(format!("WebView2 {what}: {e}"))
}

fn print_on_this_thread(dir: &Path, html: &Path, out: &Path) -> Result<(), PrintError> {
    // SAFETY: the predefined STATIC class needs no registration; the window
    // is never shown and is destroyed by `Window`'s drop on this thread.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("Font Manager PDF"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            VIEW.0,
            VIEW.1,
            None,
            None,
            None,
            None,
        )
    }
    .map_err(|e| PrintError::new(format!("no print window: {e}")))?;
    let window = Window(hwnd);

    let env = environment(dir)?;
    let controller = Controller(controller(&env, window.0)?);
    // SAFETY: calls on live interfaces of this thread.
    let webview = unsafe {
        let _ = controller.0.SetBounds(RECT {
            left: 0,
            top: 0,
            right: VIEW.0,
            bottom: VIEW.1,
        });
        controller.0.CoreWebView2()
    }
    .map_err(|e| PrintError::new(format!("WebView2 view: {e}")))?;

    navigate(&webview, &file_url(html))?;
    wait_for_fonts(&webview)?;
    print(&env, &webview, out)
}

fn environment(dir: &Path) -> Result<ICoreWebView2Environment, PrintError> {
    let (tx, rx) = mpsc::channel();
    let folder = HSTRING::from(dir.as_os_str());
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| {
            // SAFETY: the strings and the handler outlive the call.
            unsafe {
                CreateCoreWebView2EnvironmentWithOptions(
                    PCWSTR::null(),
                    &folder,
                    None::<&ICoreWebView2EnvironmentOptions>,
                    &handler,
                )
            }
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |hr, env| {
            hr?;
            let _ = tx.send(env.ok_or_else(|| windows::core::Error::from(E_POINTER)));
            Ok(())
        }),
    )
    .map_err(err("environment"))?;
    rx.recv()
        .map_err(|_| PrintError::new("WebView2 environment: no answer"))?
        .map_err(|e| PrintError::new(format!("WebView2 environment: {e}")))
}

fn controller(
    env: &ICoreWebView2Environment,
    hwnd: HWND,
) -> Result<ICoreWebView2Controller, PrintError> {
    let (tx, rx) = mpsc::channel();
    let env = env.clone();
    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| {
            // SAFETY: the window and the handler outlive the call.
            unsafe { env.CreateCoreWebView2Controller(hwnd, &handler) }
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |hr, controller| {
            hr?;
            let _ = tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)));
            Ok(())
        }),
    )
    .map_err(err("controller"))?;
    rx.recv()
        .map_err(|_| PrintError::new("WebView2 controller: no answer"))?
        .map_err(|e| PrintError::new(format!("WebView2 controller: {e}")))
}

/// Loads `url` and waits for its `NavigationCompleted`; a failed load is an error.
fn navigate(webview: &ICoreWebView2, url: &str) -> Result<(), PrintError> {
    let (tx, rx) = mpsc::channel();
    let handler = NavigationCompletedEventHandler::create(Box::new(move |_, args| {
        let mut ok = BOOL(0);
        if let Some(args) = args {
            // SAFETY: `ok` outlives the call.
            unsafe { args.IsSuccess(&mut ok) }?;
        }
        let _ = tx.send(ok.as_bool());
        Ok(())
    }));
    let mut token = 0i64;
    // SAFETY: the handler and `token` outlive the calls.
    unsafe {
        webview
            .add_NavigationCompleted(&handler, &mut token)
            .map_err(|e| PrintError::new(format!("WebView2 navigation event: {e}")))?;
        webview
            .Navigate(&HSTRING::from(url))
            .map_err(|e| PrintError::new(format!("WebView2 navigate: {e}")))?;
    }
    let ok = webview2_com::wait_with_pump(rx).map_err(err("navigation"))?;
    // SAFETY: removes the handler added above.
    let _ = unsafe { webview.remove_NavigationCompleted(token) };
    if ok {
        Ok(())
    } else {
        Err(PrintError::new(format!("the page did not load: {url}")))
    }
}

/// Polls `document.fonts.status` until every face is loaded.
fn wait_for_fonts(webview: &ICoreWebView2) -> Result<(), PrintError> {
    let deadline = Instant::now() + PRINT_TIMEOUT;
    loop {
        let (tx, rx) = mpsc::channel();
        let view = webview.clone();
        ExecuteScriptCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                // SAFETY: the script string and the handler outlive the call.
                unsafe { view.ExecuteScript(&HSTRING::from("document.fonts.status"), &handler) }
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |hr, json| {
                hr?;
                let _ = tx.send(json);
                Ok(())
            }),
        )
        .map_err(err("script"))?;
        if rx.recv().is_ok_and(|json| json == "\"loaded\"") {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err(PrintError::new("the page's fonts did not load in time"));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn print(
    env: &ICoreWebView2Environment,
    webview: &ICoreWebView2,
    out: &Path,
) -> Result<(), PrintError> {
    let env6: ICoreWebView2Environment6 = env
        .cast()
        .map_err(|e| PrintError::new(format!("WebView2 is too old to print: {e}")))?;
    let view7: ICoreWebView2_7 = webview
        .cast()
        .map_err(|e| PrintError::new(format!("WebView2 is too old to print: {e}")))?;
    let margin = MARGIN_MM / 25.4;
    // SAFETY: calls on a live print-settings object of this thread.
    let settings = (|| -> windows::core::Result<_> {
        let s = unsafe { env6.CreatePrintSettings() }?;
        // SAFETY: as above.
        unsafe {
            s.SetOrientation(COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT)?;
            s.SetPageWidth(A4_INCHES.0)?;
            s.SetPageHeight(A4_INCHES.1)?;
            s.SetMarginTop(margin)?;
            s.SetMarginBottom(margin)?;
            s.SetMarginLeft(margin)?;
            s.SetMarginRight(margin)?;
            s.SetShouldPrintBackgrounds(true)?;
            s.SetShouldPrintHeaderAndFooter(false)?;
        }
        Ok(s)
    })()
    .map_err(|e| PrintError::new(format!("WebView2 print settings: {e}")))?;
    let (tx, rx) = mpsc::channel();
    let path = HSTRING::from(out.as_os_str());
    PrintToPdfCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| {
            // SAFETY: the path, the settings and the handler outlive the call.
            unsafe { view7.PrintToPdf(&path, &settings, &handler) }
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |hr, ok| {
            hr?;
            let _ = tx.send(ok);
            Ok(())
        }),
    )
    .map_err(err("print"))?;
    if rx.recv().unwrap_or(false) {
        Ok(())
    } else {
        Err(PrintError::new(format!(
            "WebView2 could not print to {}",
            out.display()
        )))
    }
}
