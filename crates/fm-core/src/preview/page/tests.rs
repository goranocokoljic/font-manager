use std::fs;
use std::path::PathBuf;

use fm_api::ErrorCode;
use fm_api::common::{AxisValue, FeatureSetting};
use fm_api::ids::FaceId;
use fm_api::layout::{
    BlockAlign, BlockOverflow, ExportLayoutPngRequest, Layout, LayoutFont, LayoutPreset,
    LayoutSurface, SavedLayout, TextBlock,
};

use super::*;
use crate::layout::store::preset_id;
use crate::layout::{PRESETS, preset};
use crate::preview::png_decode;
use crate::test_support::TempDir;

const DM_SANS: &str = "google-static/dmsans/DMSans_36pt-Regular.ttf";
const ASAP: &str = "google-repo/asapsharp/AsapSharp[wdth,wght].ttf";
const HEBREW: &str = "google-static/notorashihebrew";
const TRUNCATED: &str = "derived/agdasima/agdasima-truncated.ttf";
const FEATURES_TEXT: &str = "fi fl ffi 0123";
const PAPER: (&str, &str) = ("#1a1a1a", "#ffffff");

/// A pixel differs when its luminance is off by more than this, out of 255:
/// `pixelmatch`'s default, in effect. Below it are the edges of a glyph, which
/// Skia draws with gamma and swash without.
const DIFF_CHANNEL: i32 = 32;
/// Criterion 2 of E45.5.
const MAX_DIFF: f64 = 0.02;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn font(rel: &str) -> Vec<u8> {
    fs::read(root().join("fonts").join(rel)).unwrap()
}

fn hebrew() -> Vec<u8> {
    let dir = root().join("fonts").join(HEBREW);
    let file = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "ttf"))
        .unwrap();
    fs::read(file).unwrap()
}

fn plain(data: &[u8]) -> PageFont<'_> {
    PageFont {
        data,
        collection_index: 0,
        variations: &[],
        features: &[],
    }
}

fn draw(layout: &Layout, font: PageFont<'_>, scale: u32, colors: (&str, &str)) -> Raster {
    let block_fonts = vec![0; layout.blocks.len()];
    render_page(&Page {
        layout,
        fonts: &[font],
        block_fonts: &block_fonts,
        scale,
        foreground: colors.0,
        background: colors.1,
    })
    .unwrap()
}

fn block(text: &str, size_pt: f32) -> TextBlock {
    TextBlock {
        text: text.to_owned(),
        size_pt,
        line_height: 1.2,
        tracking: 0.0,
        paragraph_spacing_pt: 0.0,
        first_line_indent_em: 0.0,
        columns: 1,
        align: BlockAlign::Start,
        overflow: BlockOverflow::Wrap,
    }
}

fn layout_of(blocks: Vec<TextBlock>) -> Layout {
    Layout {
        preset: None,
        name: "Test".into(),
        blocks,
        width_locked: true,
        width_px: Some(600.0),
    }
}

/// The share of pixels that differ by more than [`DIFF_CHANNEL`] from a golden's luminance.
fn diff(raster: &Raster, golden: &[u8]) -> f64 {
    let got = png_decode::decode(&raster.to_png()).unwrap().grey;
    assert_eq!(got.len(), golden.len());
    let differ = got
        .iter()
        .zip(golden)
        .filter(|(p, q)| (i32::from(**p) - i32::from(**q)).abs() > DIFF_CHANNEL)
        .count();
    differ as f64 / got.len() as f64
}

fn golden(p: LayoutPreset) -> Vec<u8> {
    let path = root().join("layouts").join(format!("{}.png", preset_id(p)));
    let image = png_decode::decode(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        (image.width, image.height),
        (A4_WIDTH_PX, A4_HEIGHT_PX),
        "{p:?}"
    );
    image.grey
}

/// Dark pixels: how much type an image holds.
fn dark(r: &Raster) -> usize {
    r.rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] < 128)
        .count()
}

/// The first and last column with a dark pixel.
fn ink_columns(r: &Raster) -> (u32, u32) {
    let columns: Vec<u32> = (0..r.width)
        .filter(|&x| (0..r.height).any(|y| r.pixel(x, y).unwrap()[0] < 128))
        .collect();
    (columns[0], *columns.last().unwrap())
}

/// The first and last row with a dark pixel.
fn ink_rows(r: &Raster) -> (u32, u32) {
    let rows: Vec<u32> = (0..r.height)
        .filter(|&y| (0..r.width).any(|x| r.pixel(x, y).unwrap()[0] < 128))
        .collect();
    (rows[0], *rows.last().unwrap())
}

#[test]
fn every_preset_matches_the_webview_within_two_percent() {
    let data = font(DM_SANS);
    for p in PRESETS {
        let got = draw(&preset(p), plain(&data), 1, PAPER);
        let d = diff(&got, &golden(p));
        println!("{p:?}: {:.3} %", d * 100.0);
        assert!(d < MAX_DIFF, "{p:?} differs on {:.2} %", d * 100.0);
    }
}

/// The tolerance has to catch a page that is really different: the wrong font,
/// or the same text one line lower.
#[test]
fn a_different_page_is_outside_the_tolerance() {
    let other = font(ASAP);
    let dm = font(DM_SANS);
    for p in [LayoutPreset::Magazine, LayoutPreset::Characters] {
        let wrong = draw(&preset(p), plain(&other), 1, PAPER);
        let d = diff(&wrong, &golden(p));
        assert!(d >= MAX_DIFF, "{p:?}: the wrong font differs on only {d}");
    }
    let mut shifted = preset(LayoutPreset::Characters);
    shifted.blocks.insert(0, block("x", 12.0));
    let moved = draw(&shifted, plain(&dm), 1, PAPER);
    let d = diff(&moved, &golden(LayoutPreset::Characters));
    assert!(d >= MAX_DIFF, "moved text differs on only {d}");
}

#[test]
fn the_golden_inputs_are_the_core_presets() {
    let text = fs::read_to_string(root().join("layouts/presets.json")).unwrap();
    let on_disk: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    assert_eq!(on_disk.len(), PRESETS.len());
    for (entry, p) in on_disk.iter().zip(PRESETS) {
        assert_eq!(entry["id"], preset_id(p));
        let layout: Layout = serde_json::from_value(entry["layout"].clone()).unwrap();
        assert_eq!(layout, preset(p), "regenerate fixtures/layouts");
        let width = entry["widthPx"].as_f64().unwrap() as f32;
        assert_eq!(Some(width), crate::layout::layout_width(&layout));
    }
}

#[test]
fn two_times_scale_doubles_the_page_and_keeps_the_line_breaks() {
    let data = font(DM_SANS);
    let layout = preset(LayoutPreset::Book);
    let one = draw(&layout, plain(&data), 1, PAPER);
    let two = draw(&layout, plain(&data), 2, PAPER);
    assert_eq!((one.width, one.height), (A4_WIDTH_PX, A4_HEIGHT_PX));
    assert_eq!((two.width, two.height), (2 * A4_WIDTH_PX, 2 * A4_HEIGHT_PX));
    // The same text in the same place: first and last ink twice as far.
    let (a, b) = (ink_rows(&one), ink_rows(&two));
    assert!(b.0.abs_diff(2 * a.0) <= 2, "{a:?} {b:?}");
    assert!(b.1.abs_diff(2 * a.1) <= 3, "{a:?} {b:?}");
    let ratio = dark(&two) as f64 / dark(&one) as f64;
    assert!((3.5..4.5).contains(&ratio), "{ratio}");
}

#[test]
fn the_surface_colours_fill_the_page_and_the_text() {
    let data = font(DM_SANS);
    let layout = preset(LayoutPreset::Quote);
    let ink = surface_colors(LayoutSurface::Ink);
    let paper = surface_colors(LayoutSurface::Paper);
    assert_eq!(paper, PAPER);
    let on_paper = draw(&layout, plain(&data), 1, paper);
    let on_ink = draw(&layout, plain(&data), 1, ink);
    assert_eq!(on_paper.pixel(0, 0), Some([0xff, 0xff, 0xff, 0xff]));
    assert_eq!(on_ink.pixel(0, 0), Some([0x1c, 0x1c, 0x1e, 0xff]));
    let lightest = |r: &Raster| {
        r.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[0])
            .max()
            .unwrap()
    };
    let darkest = |r: &Raster| {
        r.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[0])
            .min()
            .unwrap()
    };
    assert_eq!(darkest(&on_paper), 0x1a);
    assert_eq!(lightest(&on_ink), 0xec);
}

#[test]
fn axis_values_change_the_weight_and_the_widths() {
    let data = font(ASAP);
    let layout = layout_of(vec![block("Hamburgefonstiv", 36.0)]);
    let weight = |w: f32| {
        let variations = [AxisValue {
            tag: "wght".into(),
            value: w,
        }];
        draw(
            &layout,
            PageFont {
                variations: &variations,
                ..plain(&data)
            },
            1,
            PAPER,
        )
    };
    let light = weight(200.0);
    let bold = weight(800.0);
    assert!(dark(&bold) as f64 > dark(&light) as f64 * 1.3);
    // The advances change with the outlines: the shaper reads the same instance.
    assert!(ink_columns(&bold).1 > ink_columns(&light).1);
}

#[test]
fn features_change_the_glyphs_and_letter_spacing_turns_ligatures_off() {
    let data = font(DM_SANS);
    let layout = layout_of(vec![block(FEATURES_TEXT, 36.0)]);
    let draw_with = |layout: &Layout, on: bool| {
        let features = [FeatureSetting {
            tag: "liga".into(),
            on,
        }];
        draw(
            layout,
            PageFont {
                features: &features,
                ..plain(&data)
            },
            1,
            PAPER,
        )
    };
    let on = draw_with(&layout, true);
    let off = draw_with(&layout, false);
    assert_ne!(on.rgba, off.rgba, "liga has to change the picture");
    assert_eq!(draw(&layout, plain(&data), 1, PAPER).rgba, on.rgba);

    // Tracked text has no ligatures, as in the webview, unless they are asked for.
    let mut spaced = block(FEATURES_TEXT, 36.0);
    spaced.tracking = 1.0;
    let spaced = layout_of(vec![spaced]);
    let default = draw(&spaced, plain(&data), 1, PAPER);
    assert_eq!(default.rgba, draw_with(&spaced, false).rgba);
    assert_ne!(default.rgba, draw_with(&spaced, true).rgba);
}

#[test]
fn each_block_is_set_in_its_own_font() {
    let dm = font(DM_SANS);
    let asap = font(ASAP);
    let layout = layout_of(vec![
        block("Hamburgefonstiv", 30.0),
        block("Hamburgefonstiv", 30.0),
    ]);
    let both = render_page(&Page {
        layout: &layout,
        fonts: &[plain(&dm), plain(&asap)],
        block_fonts: &[0, 1],
        scale: 1,
        foreground: PAPER.0,
        background: PAPER.1,
    })
    .unwrap();
    let same = draw(&layout, plain(&dm), 1, PAPER);
    let row = |r: &Raster, y: u32| -> Vec<u8> {
        let at = (y * r.width) as usize * 4;
        r.rgba[at..at + r.width as usize * 4].to_vec()
    };
    // The first block's rows are the same; the second block's are not.
    let split = PAGE_MARGIN_PX as u32 + 53;
    for y in 0..split {
        assert_eq!(row(&both, y), row(&same, y), "row {y}");
    }
    assert!((split..split + 53).any(|y| row(&both, y) != row(&same, y)));
}

#[test]
fn a_clip_block_keeps_one_line_and_cuts_it_at_the_column() {
    let data = font(DM_SANS);
    let long = "wide words that go on and on and on and on and on and on";
    let wrap = layout_of(vec![block(long, 48.0)]);
    let mut clipped = wrap.clone();
    clipped.blocks[0].overflow = BlockOverflow::Clip;
    let wrapped = draw(&wrap, plain(&data), 1, PAPER);
    let clip = draw(&clipped, plain(&data), 1, PAPER);
    assert!(
        ink_rows(&clip).1 < 48 + 80,
        "one line: {:?}",
        ink_rows(&clip)
    );
    assert!(ink_rows(&wrapped).1 > 48 + 80, "several lines");
    let right = (A4_WIDTH_PX as f32 + 600.0) / 2.0;
    assert!((ink_columns(&clip).1 as f32) < right + 1.0);
    assert!(
        (ink_columns(&clip).1 as f32) > right - 40.0,
        "up to the edge"
    );
}

#[test]
fn alignment_moves_a_short_line_and_justify_fills_all_but_the_last() {
    let data = font(DM_SANS);
    let aligned = |align: BlockAlign, text: &str| {
        let layout = layout_of(vec![TextBlock {
            align,
            ..block(text, 24.0)
        }]);
        ink_columns(&draw(&layout, plain(&data), 1, PAPER))
    };
    let (left, right) = (97.0, 697.0);
    let start = aligned(BlockAlign::Start, "Short");
    let end = aligned(BlockAlign::End, "Short");
    let center = aligned(BlockAlign::Center, "Short");
    assert!((start.0 as f32 - left).abs() < 5.0, "{start:?}");
    assert!((end.1 as f32 - right).abs() < 5.0, "{end:?}");
    assert!(
        ((center.0 + center.1) as f32 / 2.0 - 397.0).abs() < 2.0,
        "{center:?}"
    );
    // A one-line paragraph is its last line: justify leaves it at the start.
    assert_eq!(aligned(BlockAlign::Justify, "Two words"), {
        aligned(BlockAlign::Start, "Two words")
    });
    // A wrapped paragraph reaches the right edge on its first line.
    let long = "justified text runs from edge to edge on all of its lines but the last one here";
    let wrapped = aligned(BlockAlign::Justify, long);
    assert!((wrapped.1 as f32 - right).abs() < 5.0, "{wrapped:?}");
}

#[test]
fn right_to_left_text_starts_at_the_right_and_indents_from_there() {
    let data = hebrew();
    let layout = layout_of(vec![TextBlock {
        first_line_indent_em: 2.0,
        ..block(
            "\u{05e9}\u{05dc}\u{05d5}\u{05dd} \u{05e2}\u{05d5}\u{05dc}\u{05dd}",
            24.0,
        )
    }]);
    let (first, last) = ink_columns(&draw(&layout, plain(&data), 1, PAPER));
    let right = 697.0;
    let indent = 2.0 * 24.0 * PX_PER_PT;
    assert!((right - indent - last as f32).abs() < 6.0, "{first} {last}");
    assert!(first > 397, "it stays on the right: {first}");
}

#[test]
fn malformed_or_unknown_input_is_an_error_not_a_panic() {
    let layout = preset(LayoutPreset::Quote);
    let block_fonts = vec![0; layout.blocks.len()];
    let page = |fonts: &[PageFont<'_>], block_fonts: &[usize]| {
        render_page(&Page {
            layout: &layout,
            fonts,
            block_fonts,
            scale: 1,
            foreground: PAPER.0,
            background: PAPER.1,
        })
        .unwrap_err()
        .code()
    };
    let bad = font(TRUNCATED);
    assert_eq!(page(&[plain(&bad)], &block_fonts), ErrorCode::Unsupported);
    assert_eq!(
        page(&[plain(b"not a font")], &block_fonts),
        ErrorCode::Unsupported
    );
    let good = font(DM_SANS);
    assert_eq!(
        page(
            &[PageFont {
                collection_index: 7,
                ..plain(&good)
            }],
            &block_fonts
        ),
        ErrorCode::NotFound
    );
    let bad_tag = [AxisValue {
        tag: "toolong".into(),
        value: 1.0,
    }];
    assert_eq!(
        page(
            &[PageFont {
                variations: &bad_tag,
                ..plain(&good)
            }],
            &block_fonts
        ),
        ErrorCode::InvalidRequest
    );
    let nan = [AxisValue {
        tag: "wght".into(),
        value: f32::NAN,
    }];
    assert_eq!(
        page(
            &[PageFont {
                variations: &nan,
                ..plain(&good)
            }],
            &block_fonts
        ),
        ErrorCode::InvalidRequest
    );
    // A block without a font to be set in.
    assert_eq!(page(&[plain(&good)], &[]), ErrorCode::InvalidRequest);
}

#[test]
fn an_empty_layout_is_a_blank_page() {
    let data = font(DM_SANS);
    let page = draw(&layout_of(Vec::new()), plain(&data), 1, PAPER);
    assert_eq!(
        page.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] != 0xff)
            .count(),
        0
    );
    let empty = draw(&layout_of(vec![block("", 12.0)]), plain(&data), 1, PAPER);
    assert_eq!(empty.rgba, page.rgba);
}

#[test]
fn spaces_collapse_as_in_css() {
    assert_eq!(collapse_spaces("  a \t  b  "), "a b");
    assert_eq!(collapse_spaces("   "), "");
    assert_eq!(collapse_spaces("a\u{a0}\u{a0}b"), "a\u{a0}\u{a0}b");
}

/// Every character is 1 unit wide.
fn by_chars(part: &str) -> FmResult<f32> {
    Ok(part.chars().count() as f32)
}

fn lines_of(text: &str, width: f32, indent: f32) -> Vec<&str> {
    break_lines(text, width, indent, by_chars)
        .unwrap()
        .into_iter()
        .map(|r| &text[r])
        .collect()
}

#[test]
fn lines_break_greedily_with_a_hanging_space() {
    assert_eq!(lines_of("aaa bbb ccc", 7.0, 0.0), ["aaa bbb ", "ccc"]);
    // The space after "bbb" hangs past the edge; the word still fits.
    assert_eq!(lines_of("aaa bbb ccc", 7.0, 0.0)[0].trim_end().len(), 7);
    assert_eq!(lines_of("aaa bbb ccc", 6.0, 0.0), ["aaa ", "bbb ", "ccc"]);
    assert_eq!(lines_of("aaa bbb", 100.0, 0.0), ["aaa bbb"]);
}

#[test]
fn the_indent_takes_room_on_the_first_line_only() {
    assert_eq!(lines_of("aaa bbb ccc", 7.0, 2.0), ["aaa ", "bbb ccc"]);
}

#[test]
fn a_word_wider_than_the_line_breaks_between_its_characters() {
    assert_eq!(lines_of("abcdefgh", 3.0, 0.0), ["abc", "def", "gh"]);
    assert_eq!(
        lines_of("xx abcdefgh y", 3.0, 0.0),
        ["xx ", "abc", "def", "gh ", "y"]
    );
    // Never an endless loop or an empty line when nothing fits.
    assert_eq!(lines_of("ab", 0.5, 0.0), ["a", "b"]);
    assert_eq!(lines_of("héllo", 2.0, 0.0), ["hé", "ll", "o"]);
}

fn columns_of(placed: &Placed) -> Vec<Vec<(usize, usize)>> {
    let most = placed.lines.iter().map(|s| s.column).max().unwrap_or(0);
    (0..=most)
        .map(|c| {
            placed
                .lines
                .iter()
                .filter(|s| s.column == c)
                .map(|s| (s.paragraph, s.line))
                .collect()
        })
        .collect()
}

#[test]
fn one_column_stacks_lines_and_margins() {
    let placed = place_lines(&[2, 1], 10.0, 5.0, 1);
    let ys: Vec<f32> = placed.lines.iter().map(|s| s.y).collect();
    assert_eq!(ys, [0.0, 10.0, 25.0]);
    assert_eq!(placed.height, 40.0);
}

#[test]
fn columns_balance_to_the_shortest_height() {
    let placed = place_lines(&[9], 10.0, 0.0, 3);
    let columns = columns_of(&placed);
    assert_eq!(columns.iter().map(Vec::len).collect::<Vec<_>>(), [3, 3, 3]);
    assert_eq!(placed.height, 30.0);
}

#[test]
fn a_column_break_keeps_two_lines_on_each_side() {
    // Three lines in two columns: a 2 + 1 split would leave a widow, so the
    // column grows until all three stay together.
    let placed = place_lines(&[3], 10.0, 0.0, 2);
    assert_eq!(columns_of(&placed).len(), 1);
    // A first paragraph fills the column but one line: the next one moves whole.
    let placed = place_lines(&[3, 4], 10.0, 0.0, 2);
    let columns = columns_of(&placed);
    assert_eq!(columns[0], [(0, 0), (0, 1), (0, 2)]);
    assert_eq!(columns[1][0], (1, 0));
    // Long paragraphs do break, with two lines or more on each side.
    let placed = place_lines(&[8], 10.0, 0.0, 2);
    assert_eq!(
        columns_of(&placed).iter().map(Vec::len).collect::<Vec<_>>(),
        [4, 4]
    );
}

#[test]
fn a_block_with_no_lines_has_only_its_margins() {
    assert_eq!(place_lines(&[], 10.0, 5.0, 3).height, 0.0);
    let placed = place_lines(&[0, 0], 10.0, 5.0, 2);
    assert!(placed.lines.is_empty());
    assert_eq!(placed.height, 10.0);
}

fn request(scale: u8, surface: LayoutSurface) -> ExportLayoutPngRequest {
    ExportLayoutPngRequest {
        layout: "book".into(),
        scale,
        surface,
        font: LayoutFont {
            face: FaceId(1),
            variations: Vec::new(),
            features: Vec::new(),
        },
        block_fonts: Vec::new(),
    }
}

#[test]
fn fonts_are_listed_once_with_each_blocks_index() {
    let mut req = request(1, LayoutSurface::Paper);
    let other = LayoutFont {
        face: FaceId(2),
        variations: Vec::new(),
        features: Vec::new(),
    };
    req.block_fonts = vec![
        None,
        Some(other.clone()),
        Some(req.font.clone()),
        Some(other),
    ];
    let (fonts, at) = request_fonts(&req, 5).unwrap();
    let faces: Vec<FaceId> = fonts.iter().map(|f| f.face).collect();
    assert_eq!(faces, [FaceId(1), FaceId(2)]);
    assert_eq!(at, [0, 1, 0, 1, 0]);
}

#[test]
fn the_same_face_on_other_axes_is_another_font() {
    let mut req = request(1, LayoutSurface::Paper);
    let mut bold = req.font.clone();
    bold.variations.push(AxisValue {
        tag: "wght".into(),
        value: 700.0,
    });
    req.block_fonts = vec![Some(bold)];
    let (fonts, at) = request_fonts(&req, 2).unwrap();
    assert_eq!(fonts.len(), 2);
    assert_eq!(at, [1, 0]);
}

#[test]
fn only_one_and_two_times_scale_are_exported() {
    for scale in [0, 3, 255] {
        let err = request_fonts(&request(scale, LayoutSurface::Paper), 1).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidRequest, "{scale}");
    }
    for scale in [1, 2] {
        assert!(request_fonts(&request(scale, LayoutSurface::Paper), 1).is_ok());
    }
}

#[test]
fn a_font_for_a_block_that_is_not_there_is_refused() {
    let mut req = request(1, LayoutSurface::Paper);
    req.block_fonts = vec![None, None];
    let err = request_fonts(&req, 1).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
}

#[test]
fn an_export_is_a_new_png_that_never_replaces_an_earlier_one() {
    let dir = TempDir::new("layout-export");
    let out = dir.path().join("not-yet").join("exports");
    let file = root().join("fonts").join(DM_SANS);
    let saved = SavedLayout {
        id: "book".into(),
        layout: preset(LayoutPreset::Book),
    };
    let files = [(file.as_path(), 0)];
    let req = request(2, LayoutSurface::Ink);
    let first = export_layout(&saved, &req, &files, &out).unwrap();
    assert_eq!(first, out.join("book-ink-2x.png"));
    let bytes = fs::read(&first).unwrap();
    let raw = (2 * A4_WIDTH_PX * 2 * A4_HEIGHT_PX * 4) as usize;
    assert!(bytes.len() < raw / 4, "compressed: {} bytes", bytes.len());
    let image = png_decode::decode(&bytes).unwrap();
    assert_eq!(
        (image.width, image.height),
        (2 * A4_WIDTH_PX, 2 * A4_HEIGHT_PX)
    );
    assert_eq!(image.grey[0], 0x1c, "the ink surface");

    fs::write(&first, b"an earlier export").unwrap();
    let second = export_layout(&saved, &req, &files, &out).unwrap();
    assert_eq!(second, out.join("book-ink-2x-2.png"));
    assert_eq!(fs::read(&first).unwrap(), b"an earlier export");
    let third = export_layout(&saved, &req, &files, &out).unwrap();
    assert_eq!(third, out.join("book-ink-2x-3.png"));
    let paper = export_layout(&saved, &request(1, LayoutSurface::Paper), &files, &out).unwrap();
    assert_eq!(paper, out.join("book-paper-1x.png"));
}

#[test]
fn an_export_reports_a_font_file_it_cannot_read_and_writes_nothing() {
    let dir = TempDir::new("layout-export-missing");
    let out = dir.path().join("exports");
    let gone = dir.path().join("gone.ttf");
    let saved = SavedLayout {
        id: "book".into(),
        layout: preset(LayoutPreset::Book),
    };
    let err = export_layout(
        &saved,
        &request(1, LayoutSurface::Paper),
        &[(gone.as_path(), 0)],
        &out,
    )
    .unwrap_err();
    assert_eq!(err.message(), "The font file could not be read.");
    assert!(!out.exists());
}
