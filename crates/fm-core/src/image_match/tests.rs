use std::path::{Path, PathBuf};

use fm_api::ErrorCode;
use fm_api::FaceId;
use fm_api::preview::PreviewRequest;

use super::*;
use crate::preview::{self, Raster};

pub(crate) fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel)
}

/// `text` drawn from the font file at `rel` by the core rasterizer at
/// `size_px`, `fg` on `bg`, with a margin of half the size around it.
pub(crate) fn render(rel: &str, text: &str, size_px: f32, fg: &str, bg: &str) -> Raster {
    let data = std::fs::read(fixture(rel)).expect("fixture");
    let line = preview::rasterize(
        &data,
        0,
        &PreviewRequest {
            face: FaceId(0),
            text: text.into(),
            size_px,
            variations: Vec::new(),
            features: Vec::new(),
            foreground: Some(fg.into()),
            background: Some(bg.into()),
        },
    )
    .expect("render");
    pad(&line, (size_px / 2.0) as u32, bg)
}

/// `raster` inside a margin of `m` pixels of the colour `bg` (`#rrggbb`).
fn pad(raster: &Raster, m: u32, bg: &str) -> Raster {
    let hex = |i: usize| u8::from_str_radix(&bg[i..i + 2], 16).expect("hex");
    let fill = [hex(1), hex(3), hex(5), 255];
    let (w, h) = (raster.width + 2 * m, raster.height + 2 * m);
    let mut rgba: Vec<u8> = fill.repeat((w * h) as usize);
    for y in 0..raster.height {
        let src =
            &raster.rgba[(y * raster.width * 4) as usize..((y + 1) * raster.width * 4) as usize];
        let at = (((y + m) * w + m) * 4) as usize;
        rgba[at..at + src.len()].copy_from_slice(src);
    }
    Raster {
        width: w,
        height: h,
        baseline: raster.baseline + m,
        rgba,
    }
}

fn boxes_of(png: &[u8]) -> Vec<Vec<GlyphBox>> {
    let pixels = png_decode::decode(png).expect("decode");
    lines(&coverage(&pixels).expect("coverage"))
}

const UPRIGHT: &str = "google-repo/ubuntu/Ubuntu-Regular.ttf";

/// Criterion 3: the sample in an upright face gives one box per letter in
/// reading order, the dot of the `i` joined to its stem.
#[test]
fn hamburgefonstiv_gives_15_boxes_in_reading_order() {
    let png = render(UPRIGHT, fingerprint::SAMPLE, 48.0, "#000000", "#ffffff").to_png();
    let lines = boxes_of(&png);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let line = &lines[0];
    assert_eq!(line.len(), 15, "{line:?}");
    assert!(line.windows(2).all(|p| p[0].left < p[1].left), "{line:?}");
    // The `i` (14th) is taller than the `v` beside it: the dot is part of it.
    let (i, v) = (line[13], line[14]);
    assert!(i.top + 5 < v.top, "i {i:?} v {v:?}");
    assert!(i.bottom.abs_diff(v.bottom) <= 1, "i {i:?} v {v:?}");
}

/// The boxes without their ink counts, which antialiasing against another
/// background moves by a pixel or two.
fn frames(lines: &[Vec<GlyphBox>]) -> Vec<Vec<(u32, u32, u32, u32)>> {
    lines
        .iter()
        .map(|l| {
            l.iter()
                .map(|b| (b.left, b.top, b.right, b.bottom))
                .collect()
        })
        .collect()
}

/// Criterion 3: light on dark gives the same boxes as dark on light, and
/// about the same fingerprint.
#[test]
fn light_on_dark_reads_as_dark_on_light() {
    let dark = render(UPRIGHT, fingerprint::SAMPLE, 48.0, "#000000", "#ffffff").to_png();
    let light = render(UPRIGHT, fingerprint::SAMPLE, 48.0, "#ffffff", "#000000").to_png();
    let tinted = render(UPRIGHT, fingerprint::SAMPLE, 48.0, "#f0e8d0", "#203040").to_png();
    assert_eq!(frames(&boxes_of(&light)), frames(&boxes_of(&dark)));
    assert_eq!(frames(&boxes_of(&tinted)), frames(&boxes_of(&dark)));
    let a = fingerprint_png(&dark).expect("dark").fingerprint;
    for other in [&light, &tinted] {
        let b = fingerprint_png(other).expect("light").fingerprint;
        let most = a
            .vector
            .iter()
            .zip(&b.vector)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(most < 0.02, "{a:?}\n{b:?}");
        assert!((a.stroke_width - b.stroke_width).abs() < 0.02);
        assert_eq!(a.x_height_ratio, b.x_height_ratio);
    }
}

/// Criterion 3: specks around the text are dropped.
#[test]
fn specks_are_dropped() {
    let mut raster = render(UPRIGHT, fingerprint::SAMPLE, 48.0, "#000000", "#ffffff");
    let clean = boxes_of(&raster.to_png());
    for (x, y) in [
        (3, 3),
        (10, 60),
        (200, 2),
        (raster.width - 4, raster.height - 3),
    ] {
        let at = ((y * raster.width + x) * 4) as usize;
        raster.rgba[at..at + 3].copy_from_slice(&[0, 0, 0]);
    }
    assert_eq!(boxes_of(&raster.to_png()), clean);
}

/// Two lines come out top to bottom, each left to right.
#[test]
fn two_lines_come_out_in_reading_order() {
    let top = render(UPRIGHT, "Hab", 48.0, "#000000", "#ffffff");
    let bottom = render(UPRIGHT, "xyzw", 48.0, "#000000", "#ffffff");
    let width = top.width.max(bottom.width);
    let mut rgba = Vec::new();
    for r in [&top, &bottom] {
        for y in 0..r.height {
            let row = &r.rgba[(y * r.width * 4) as usize..((y + 1) * r.width * 4) as usize];
            rgba.extend_from_slice(row);
            rgba.extend(std::iter::repeat_n(255, ((width - r.width) * 4) as usize));
        }
    }
    let both = Raster {
        width,
        height: top.height + bottom.height,
        baseline: 0,
        rgba,
    };
    let lines = boxes_of(&both.to_png());
    assert_eq!(lines.iter().map(Vec::len).collect::<Vec<_>>(), [3, 4]);
    assert!(lines[0][0].bottom < lines[1][0].top);
}

/// White text on a transparent background (an RGBA screenshot of dark-mode
/// text) is read over black.
#[test]
fn white_text_on_transparent_is_read() {
    let mut raster = render(UPRIGHT, fingerprint::SAMPLE, 48.0, "#000000", "#ffffff");
    for p in raster.rgba.as_chunks_mut::<4>().0 {
        let ink = 255 - p[0];
        p.copy_from_slice(&[255, 255, 255, ink]);
    }
    assert_eq!(boxes_of(&raster.to_png())[0].len(), 15);
}

/// Criterion 5: a blank image, one without text and a corrupt PNG are
/// plain errors.
#[test]
fn a_blank_image_or_one_without_text_is_a_plain_error() {
    let blank = Raster {
        width: 300,
        height: 100,
        baseline: 0,
        rgba: [250, 250, 250, 255].repeat(300 * 100),
    };
    // A big dark square: one shape, not a line of text.
    let mut square = blank.clone();
    for y in 20..80 {
        for x in 100..160 {
            let at = ((y * 300 + x) * 4) as usize;
            square.rgba[at..at + 3].copy_from_slice(&[0, 0, 0]);
        }
    }
    // Noise: every pixel random grey.
    let mut noise = blank.clone();
    let mut seed = 0x1234_5678u32;
    for p in noise.rgba.as_chunks_mut::<4>().0 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let g = (seed >> 24) as u8;
        p[..3].copy_from_slice(&[g, g, g]);
    }
    // A gradient, left to right.
    let mut gradient = blank.clone();
    for (i, p) in gradient.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let g = ((i % 300) * 255 / 299) as u8;
        p[..3].copy_from_slice(&[g, g, g]);
    }
    for (what, image) in [
        ("blank", blank),
        ("square", square),
        ("noise", noise),
        ("gradient", gradient),
    ] {
        let e = fingerprint_png(&image.to_png()).expect_err(what);
        assert_eq!(e.code(), ErrorCode::InvalidRequest, "{what}: {e:?}");
        assert_eq!(e.message(), "No text was found in this image.", "{what}");
    }
    let mut corrupt = render(UPRIGHT, "Ham", 24.0, "#000000", "#ffffff").to_png();
    let mid = corrupt.len() / 2;
    corrupt[mid] ^= 0xff;
    let e = fingerprint_png(&corrupt).expect_err("corrupt");
    assert_eq!(e.message(), "This image could not be read.");
}

#[test]
fn text_that_is_too_small_is_a_plain_error() {
    let png = render(UPRIGHT, fingerprint::SAMPLE, 8.0, "#000000", "#ffffff").to_png();
    let e = fingerprint_png(&png).expect_err("tiny");
    assert_eq!(e.code(), ErrorCode::InvalidRequest);
}

/// Criterion 4: the image of the sample gives about the face's own
/// fingerprint, so the E23.2 distance finds it.
#[test]
fn the_sample_image_gives_about_the_faces_fingerprint() {
    let data = std::fs::read(fixture(UPRIGHT)).expect("fixture");
    let own = fingerprint::fingerprint(&data, 0, Params::default()).expect("own");
    let png = render(UPRIGHT, fingerprint::SAMPLE, 48.0, "#000000", "#ffffff").to_png();
    let seen = fingerprint_png(&png).expect("image");
    assert!(!seen.shapes_only);
    let seen = seen.fingerprint;
    let diff: f32 = own
        .vector
        .iter()
        .zip(&seen.vector)
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / own.vector.len() as f32;
    eprintln!("own {own:?}\nseen {seen:?}\nmean cell difference {diff}");
    assert!(diff < 0.04, "{diff}");
    assert!((own.x_height_ratio - seen.x_height_ratio).abs() < 0.03);
    assert!((own.stroke_width - seen.stroke_width).abs() < 0.03);
    assert!((own.advance_ratio - seen.advance_ratio).abs() < 0.05);
}

/// The file is only read: its bytes and time stamp stay.
#[test]
fn the_file_is_only_read() {
    let dir = crate::test_support::TempDir::new("image-match");
    let path = dir.path().join("shot.png");
    let png = render(UPRIGHT, fingerprint::SAMPLE, 48.0, "#000000", "#ffffff").to_png();
    std::fs::write(&path, &png).expect("write");
    let modified = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .expect("mtime");
    fingerprint_file(&path).expect("fingerprint");
    assert_eq!(std::fs::read(&path).expect("read"), png);
    assert_eq!(
        std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .expect("mtime"),
        modified
    );
    let e = fingerprint_file(&dir.path().join("gone.png")).expect_err("missing");
    assert_eq!(e.code(), ErrorCode::NotFound);
}

/// A script's joined letters are fewer glyphs than the sample has letters:
/// only the drawing and the width are measured.
#[test]
fn joined_letters_are_measured_by_shape_and_width_only() {
    let rel = "google-repo/clickerscript/ClickerScript-Regular.ttf";
    let data = std::fs::read(fixture(rel)).expect("fixture");
    let own = fingerprint::fingerprint(&data, 0, Params::default()).expect("own");
    let png = render(rel, fingerprint::SAMPLE, 48.0, "#000000", "#ffffff").to_png();
    assert!(boxes_of(&png)[0].len() < 15);
    let seen = fingerprint_png(&png).expect("image");
    assert!(seen.shapes_only);
    assert_eq!(seen.fingerprint.stroke_width, 0.0);
    assert!((own.advance_ratio - seen.fingerprint.advance_ratio).abs() < 0.1);
}
