//! Tools for the layout goldens in `fixtures/layouts/` (E45.5).
//!
//! ```text
//! cargo run -p fm-core --example layout_page -- presets > fixtures/layouts/presets.json
//! cargo run -p fm-core --example layout_page --release -- render <font> <preset id> <scale> <out.png>
//! ```

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use fm_core::layout::store::preset_id;
use fm_core::layout::{PRESETS, layout_width, preset};
use fm_core::preview::page::{Page, PageFont, render_page};

fn presets() -> Result<(), String> {
    let all: Vec<_> = PRESETS
        .into_iter()
        .map(|p| {
            let layout = preset(p);
            serde_json::json!({
                "id": preset_id(p),
                "widthPx": layout_width(&layout),
                "layout": layout,
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    println!("{text}");
    Ok(())
}

fn render(font: &str, id: &str, scale: &str, out: &str) -> Result<(), String> {
    let data = std::fs::read(Path::new(font)).map_err(|e| e.to_string())?;
    let chosen = PRESETS
        .into_iter()
        .find(|p| preset_id(*p) == id)
        .ok_or_else(|| format!("no preset {id}"))?;
    let layout = preset(chosen);
    let fonts = [PageFont {
        data: &data,
        collection_index: 0,
        variations: &[],
        features: &[],
    }];
    let block_fonts = vec![0; layout.blocks.len()];
    let start = Instant::now();
    let raster = render_page(&Page {
        layout: &layout,
        fonts: &fonts,
        block_fonts: &block_fonts,
        scale: scale.parse().map_err(|_| format!("bad scale {scale}"))?,
        foreground: "#1a1a1a",
        background: "#ffffff",
    })
    .map_err(|e| e.to_string())?;
    let drawn = start.elapsed();
    let png = raster.to_compressed_png();
    std::fs::write(out, png).map_err(|e| e.to_string())?;
    println!(
        "{}x{} drawn in {:.1} ms, written after {:.1} ms",
        raster.width,
        raster.height,
        drawn.as_secs_f64() * 1000.0,
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["presets"] => presets(),
        ["render", font, id, scale, out] => render(font, id, scale, out),
        _ => Err("usage: layout_page presets | render <font> <preset id> <scale> <out.png>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
