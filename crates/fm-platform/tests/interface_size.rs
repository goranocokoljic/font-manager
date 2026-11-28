//! E58.4 (ADR 0029): the interface size, measured in the webview of each OS.
//!
//! The app shows its interface at 100, 125 and 150 per cent. A surface that
//! shows a font takes the zoom back (`zoom: 1/z` on the surface), so the font
//! is laid out in true pixels at every size. This test draws six such
//! surfaces — a specimen on the wall, the family page's hero, the waterfall's
//! 10 to 96 px ladder, a glyph cell, Compare's overlay (HTML and SVG text) and
//! the Studio paper — in an offscreen webview of the OS, at each size, in the
//! two ways the app could zoom:
//!
//! - `webview`: the webview's own zoom, the call Tauri's `set_zoom` makes;
//! - `css`: `zoom` on the root element.
//!
//! It measures the ink in the pixels the webview drew and asserts that each
//! surface is the size it is at 100 per cent, to the pixel. A control page
//! that does not take the zoom back must grow by the zoom, so a zoom that did
//! not happen cannot pass for a surface that held its size.
//!
//! Every measurement is printed (`interface-size …` lines), so the CI log of
//! each OS carries its numbers. AppKit needs the main thread, so the file has
//! its own `main` (`harness = false`), like `fm-core`'s `specimen_pdf`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a test binary

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fm_platform::webview_probe::{PixelBox, Snapshot, snapshot_html};

/// The tests of this binary, by name.
const TESTS: [(&str, fn()); 3] = [
    ("base64_matches_the_rfc_vectors", base64_vectors),
    (
        "a_specimen_keeps_its_size_at_every_interface_size",
        one_specimen,
    ),
    (
        "every_font_surface_keeps_its_size_at_every_interface_size_in_both_ways",
        every_surface,
    ),
];

/// The interface sizes (fm-api `InterfaceSize`): 100, 125 and 150 per cent.
const SIZES: [f64; 3] = [1.0, 1.25, 1.5];
/// The view, in CSS pixels at 100 per cent.
const VIEW: (u32, u32) = (1000, 700);
/// The waterfall's ladder (`FAMILY_WATERFALL_SIZES`, apps/desktop/src/list/waterfall.ts).
const LADDER: [u32; 10] = [10, 12, 14, 17, 21, 28, 36, 48, 64, 96];
/// The fixture font every surface shows.
const FONT: &str = "fontshare/public-sans/PublicSans-Bold.ttf";
/// How far a measurement may be from 100 per cent: one pixel, the rounding
/// of a sub-pixel origin to the pixel grid.
const TOLERANCE: i64 = 1;

/// How the interface is zoomed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    /// The webview's own zoom (`set_zoom`).
    Webview,
    /// `zoom` on the root element.
    Css,
}

impl Way {
    fn name(self) -> &'static str {
        match self {
            Self::Webview => "webview",
            Self::Css => "css",
        }
    }
}

/// A surface that shows a font: its name and its HTML, in a box of class `s`.
struct Surface {
    name: &'static str,
    html: String,
    /// Whether each line is measured on its own (the waterfall's ladder).
    bands: bool,
}

fn surfaces() -> Vec<Surface> {
    let ladder = LADDER.iter().fold(String::new(), |mut out, px| {
        let _ = write!(
            out,
            r#"<div style="font-size:{px}px;line-height:1.5;white-space:nowrap">Hamburg {px}</div>"#
        );
        out
    });
    vec![
        Surface {
            name: "wall-specimen",
            html: r#"<div class="s" style="font-size:40px;padding:16px">Hamburgefonstiv</div>"#
                .into(),
            bands: false,
        },
        Surface {
            name: "family-hero",
            html: r#"<div class="s" style="font-size:96px;line-height:1.1;padding:24px">Aa Qq Rg</div>"#
                .into(),
            bands: false,
        },
        Surface {
            name: "waterfall-ladder",
            html: format!(r#"<div class="s" style="padding:8px">{ladder}</div>"#),
            bands: true,
        },
        Surface {
            name: "glyph-cell",
            html: r#"<div class="s" style="padding:8px"><div style="width:96px;height:96px;border:1px solid #000;display:grid;place-items:center;font-size:64px;line-height:1">g</div></div>"#
                .into(),
            bands: false,
        },
        Surface {
            name: "compare-overlay",
            html: r#"<div class="s" style="position:relative;width:420px;height:200px">
<div style="position:absolute;left:20px;top:20px;font-size:120px;line-height:1">Rg</div>
<svg style="position:absolute;left:0;top:0" width="420" height="200" viewBox="0 0 420 200"><text x="200" y="150" font-family="Probe" font-size="120">Rg</text></svg>
</div>"#
                .into(),
            bands: false,
        },
        Surface {
            name: "studio-paper",
            html: r#"<div class="s" style="padding:10px"><div style="width:420px;height:594px;border:1px solid #000;box-sizing:border-box;padding:40px">
<div style="font-size:32px;line-height:1.2">Studio</div><div style="font-size:12px;line-height:1.5;margin-top:12px">A paragraph of text at twelve pixels on the paper.</div></div></div>"#
                .into(),
            bands: false,
        },
    ]
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--list") {
        if !has("--ignored") {
            for (name, _) in TESTS {
                println!("{name}: test");
            }
        }
        return ExitCode::SUCCESS;
    }
    let filters: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    let selected: Vec<(&str, fn())> = TESTS
        .into_iter()
        .filter(|(name, _)| {
            filters.is_empty()
                || filters.iter().any(|f| {
                    if has("--exact") {
                        *f == name
                    } else {
                        name.contains(f.as_str())
                    }
                })
        })
        .collect();
    if selected.is_empty() || has("--ignored") {
        println!("running 0 tests");
        return ExitCode::SUCCESS;
    }
    println!("running {} test(s)", selected.len());
    let passed = fm_platform::pdf::run_ui_loop(move || {
        let mut passed = true;
        for (name, test) in selected {
            let ok = std::panic::catch_unwind(test).is_ok();
            println!("test {name} ... {}", if ok { "ok" } else { "FAILED" });
            passed &= ok;
        }
        passed
    });
    if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A scratch folder, removed at the end.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(label: &str) -> Scratch {
    let dir =
        std::env::temp_dir().join(format!("fm-interface-size-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    Scratch(dir)
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ABC[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A page with `body`, the fixture font as `Probe`, zoomed the `way` way by
/// `z`; `counter` says whether the surface (`.s`) takes the zoom back.
fn page(body: &str, way: Way, z: f64, counter: bool) -> String {
    let font = std::fs::read(fixtures().join(FONT)).expect("fixture font");
    let root = if way == Way::Css && z != 1.0 {
        format!("html{{zoom:{z}}}")
    } else {
        String::new()
    };
    let back = if counter && z != 1.0 {
        format!(".s{{zoom:{:.10}}}", 1.0 / z)
    } else {
        String::new()
    };
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><style>
@font-face{{font-family:Probe;src:url(data:font/ttf;base64,{font})}}
{root}
html,body{{margin:0;background:#fff;color:#000;overflow:hidden}}
body{{font-family:Probe;-webkit-font-smoothing:antialiased}}
.s{{display:inline-block}}
{back}
</style></head><body>{body}</body></html>"#,
        font = base64(&font)
    )
}

/// What one page measured.
struct Measured {
    ink: PixelBox,
    bands: Vec<PixelBox>,
    inner_width: f64,
}

fn render(dir: &Path, name: &str, html: &str, page_zoom: f64) -> Measured {
    let file = dir.join(format!("{name}.html"));
    std::fs::write(&file, html).expect("page");
    let snap: Snapshot = snapshot_html(&file, VIEW, page_zoom)
        .unwrap_or_else(|e| panic!("{name}: the webview drew nothing: {e}"));
    let ink = snap
        .ink_box()
        .unwrap_or_else(|| panic!("{name}: the page shows no ink"));
    Measured {
        ink,
        bands: snap.ink_bands(),
        inner_width: snap.inner_width,
    }
}

fn os() -> &'static str {
    std::env::consts::OS
}

fn close(a: u32, b: u32) -> bool {
    (i64::from(a) - i64::from(b)).abs() <= TOLERANCE
}

/// Draws `surface` at 100 per cent and at every size in every way, prints
/// the numbers, and returns what does not hold.
fn measure(dir: &Path, surface: &Surface, ways: &[Way]) -> Vec<String> {
    let mut wrong = Vec::new();
    let base = render(
        dir,
        &format!("{}-base", surface.name),
        &page(&surface.html, Way::Css, 1.0, true),
        1.0,
    );
    println!(
        "interface-size {} {} base 100 w={} h={} bands={:?}",
        os(),
        surface.name,
        base.ink.width,
        base.ink.height,
        base.bands
            .iter()
            .map(|b| (b.width, b.height))
            .collect::<Vec<_>>()
    );
    for &way in ways {
        for z in SIZES.into_iter().filter(|z| *z != 1.0) {
            let pct = (z * 100.0).round();
            let page_zoom = if way == Way::Webview { z } else { 1.0 };
            let label = format!("{}-{}-{pct}", surface.name, way.name());
            let m = render(dir, &label, &page(&surface.html, way, z, true), page_zoom);
            let (dw, dh) = (
                i64::from(m.ink.width) - i64::from(base.ink.width),
                i64::from(m.ink.height) - i64::from(base.ink.height),
            );
            let lines = if surface.bands {
                format!(
                    " bands={:?}",
                    m.bands
                        .iter()
                        .map(|b| (b.width, b.height))
                        .collect::<Vec<_>>()
                )
            } else {
                String::new()
            };
            println!(
                "interface-size {} {} {} {pct} w={} h={} dw={dw} dh={dh} innerWidth={}{lines}",
                os(),
                surface.name,
                way.name(),
                m.ink.width,
                m.ink.height,
                m.inner_width
            );
            // The zoom happened: the webview's zoom narrows the page in CSS
            // pixels, the root's zoom does not.
            let want_inner = if way == Way::Webview {
                f64::from(VIEW.0) / z
            } else {
                f64::from(VIEW.0)
            };
            if (m.inner_width - want_inner).abs() > 1.0 {
                wrong.push(format!(
                    "{label}: innerWidth {} where {want_inner} was expected; the zoom did not apply",
                    m.inner_width
                ));
            }
            if !close(m.ink.width, base.ink.width) || !close(m.ink.height, base.ink.height) {
                wrong.push(format!(
                    "{label}: {}×{} px where 100 per cent is {}×{}",
                    m.ink.width, m.ink.height, base.ink.width, base.ink.height
                ));
            }
            if surface.bands {
                let same = m.bands.len() == base.bands.len()
                    && m.bands
                        .iter()
                        .zip(&base.bands)
                        .all(|(a, b)| close(a.width, b.width) && close(a.height, b.height));
                if !same {
                    wrong.push(format!(
                        "{label}: lines {:?} where 100 per cent is {:?}",
                        m.bands
                            .iter()
                            .map(|b| (b.width, b.height))
                            .collect::<Vec<_>>(),
                        base.bands
                            .iter()
                            .map(|b| (b.width, b.height))
                            .collect::<Vec<_>>()
                    ));
                }
            }
            // The control: without the surface's counter-zoom it grows by z.
            let control = render(
                dir,
                &format!("{label}-control"),
                &page(&surface.html, way, z, false),
                page_zoom,
            );
            let grew = f64::from(control.ink.width) / f64::from(base.ink.width);
            println!(
                "interface-size {} {} {} {pct} control w={} grew={grew:.3}",
                os(),
                surface.name,
                way.name(),
                control.ink.width
            );
            if (grew - z).abs() > 0.03 {
                wrong.push(format!(
                    "{label}: without its counter-zoom the surface grew {grew:.3}×, not {z}×"
                ));
            }
        }
    }
    wrong
}

/// Criterion 5: one specimen at each size, in the way the ADR chose.
fn one_specimen() {
    let dir = scratch("one");
    let wall = surfaces().into_iter().next().expect("the wall specimen");
    let wrong = measure(&dir.0, &wall, &[Way::Webview]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Criterion 2: the six surfaces, both ways.
fn every_surface() {
    let dir = scratch("all");
    let mut wrong = Vec::new();
    for surface in surfaces() {
        wrong.extend(measure(&dir.0, &surface, &[Way::Webview, Way::Css]));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

fn base64_vectors() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}
