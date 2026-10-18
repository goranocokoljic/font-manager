//! Renders one line of text with the core rasterizer and writes the PNG.
//!
//! ```text
//! cargo run -p fm-core --example render_preview -- <font> <text> <size-px> <out.png> [index] [wght=700,wdth=90]
//! ```
//!
//! Used by `docs/previews/compare-os.ps1` for the rasterizer-vs-OS comparison.

use std::path::Path;
use std::process::ExitCode;

use fm_api::common::AxisValue;
use fm_api::ids::FaceId;
use fm_api::preview::PreviewRequest;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [font, text, size, out, rest @ ..] = args.as_slice() else {
        eprintln!(
            "usage: render_preview <font> <text> <size-px> <out.png> [index] [tag=value,...]"
        );
        return ExitCode::from(2);
    };
    let Ok(size_px) = size.parse::<f32>() else {
        eprintln!("size must be a number");
        return ExitCode::from(2);
    };
    let index = rest.first().and_then(|i| i.parse().ok()).unwrap_or(0);
    let variations = rest
        .get(1)
        .map(|list| {
            list.split(',')
                .filter_map(|pair| {
                    let (tag, value) = pair.split_once('=')?;
                    Some(AxisValue {
                        tag: tag.to_string(),
                        value: value.parse().ok()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let req = PreviewRequest {
        face: FaceId(0),
        text: text.clone(),
        size_px,
        variations,
        features: Vec::new(),
        foreground: Some("#000000".into()),
        background: Some("#ffffff".into()),
    };
    match fm_core::preview::render_preview_file(Path::new(font), index, &req) {
        Ok(image) => match std::fs::write(out, &image.png) {
            Ok(()) => {
                println!("{out}: {}x{}", image.width, image.height);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{out}: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
