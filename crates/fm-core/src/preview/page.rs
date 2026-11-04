//! A body-text layout on an A4 page, with the same shaper and scaler as a
//! preview line (E45.5).
//!
//! The Studio sets a layout as live text in the webview (`LayoutBlock.vue`);
//! this module follows the same rules so an export looks like the screen:
//! CSS `white-space: normal`, greedy line breaks at UAX #14 opportunities with
//! `overflow-wrap: break-word`, balanced columns with two-line orphans and
//! widows, half-leading above each line, letter spacing per character with
//! ligatures off. `fixtures/layouts/` holds Chromium renders of the presets
//! and a test keeps the difference under 2 %.
//!
//! The layout is computed once in CSS pixels; `scale` only multiplies it, so
//! line breaks are the same at 1x and 2x. A character the face does not have
//! draws its `.notdef`: unlike the webview, there is no fallback font.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use fm_api::common::{AxisValue, FeatureSetting};
use fm_api::layout::{
    BlockAlign, BlockOverflow, ExportLayoutPngRequest, Layout, LayoutFont, LayoutSurface,
    SavedLayout, TextBlock,
};
use read_fonts::types::Tag;
use swash::FontRef;
use swash::scale::image::Image;
use swash::scale::{Render, ScaleContext};
use swash::text::cluster::Boundary;
use swash::zeno::{Format, Vector};

use super::{
    Canvas, GLYPH_SOURCES, Raster, Shaped, check_face, color, first_strong_rtl, malformed,
    sfnt_data, shape_runs, tag,
};
use crate::layout::{PX_PER_PT, layout_width};
use crate::{FmError, FmResult};

/// A4 at 96 CSS pixels per inch: 210 mm.
pub const A4_WIDTH_PX: u32 = 794;
/// A4 at 96 CSS pixels per inch: 297 mm.
pub const A4_HEIGHT_PX: u32 = 1123;
/// The white space around the text on every side, in CSS pixels.
pub const PAGE_MARGIN_PX: f32 = 48.0;
/// The gap between columns (`column-gap` in `LayoutBlock.vue`), in em.
const COLUMN_GAP_EM: f32 = 2.0;
/// The fewest lines a column break leaves on either side (CSS `orphans`, `widows`).
const MIN_LINES_AT_BREAK: usize = 2;
/// How finely column balancing looks for the shortest height, in CSS pixels.
const BALANCE_STEP_PX: f32 = 0.25;
/// Column balancing tries at most this many heights, so very long text stays fast.
const MAX_BALANCE_STEPS: f32 = 4000.0;
/// Room for rounding when a width or height is compared, in CSS pixels.
const FIT_SLACK_PX: f32 = 0.01;
/// Glyph positions snap to this fraction of a pixel, as Skia's do; it also lets
/// repeated glyphs share one image.
const SUBPIXEL_STEPS: f32 = 4.0;

/// One font of a page: font data held in memory and how it is set.
#[derive(Debug, Clone, Copy)]
pub struct PageFont<'a> {
    /// TrueType, OpenType, a collection, WOFF or WOFF2.
    pub data: &'a [u8],
    /// The face inside a collection; 0 otherwise.
    pub collection_index: u32,
    /// Variation-axis values.
    pub variations: &'a [AxisValue],
    /// OpenType features.
    pub features: &'a [FeatureSetting],
}

/// What [`render_page`] draws.
#[derive(Debug, Clone, Copy)]
pub struct Page<'a> {
    /// The layout, with its values clamped.
    pub layout: &'a Layout,
    /// The fonts.
    pub fonts: &'a [PageFont<'a>],
    /// For each block, its index in `fonts`.
    pub block_fonts: &'a [usize],
    /// Device pixels per CSS pixel.
    pub scale: u32,
    /// The text colour, CSS hex.
    pub foreground: &'a str,
    /// The page colour, CSS hex.
    pub background: &'a str,
}

/// Draws the page. Malformed font data is an error, never a panic.
pub fn render_page(page: &Page<'_>) -> FmResult<Raster> {
    let sfnts = page
        .fonts
        .iter()
        .map(|f| {
            let sfnt = sfnt_data(f.data)?;
            check_face(&sfnt, f.collection_index)?;
            Ok(sfnt)
        })
        .collect::<FmResult<Vec<Cow<'_, [u8]>>>>()?;
    // The shaper and the scaler are well tested but not fuzzed against every
    // malformed font; a panic in either is a bad font, not a crash.
    panic::catch_unwind(AssertUnwindSafe(|| compose(page, &sfnts)))
        .unwrap_or_else(|_| Err(malformed("the shaper or scaler stopped on malformed data")))
}

/// One font, ready to shape and scale.
struct Face<'s, 'a, 'f> {
    shaper: &'s harfrust::ShaperFont<'a, 'f>,
    font: FontRef<'s>,
    coords: Vec<i16>,
    /// The features as asked.
    features: Vec<harfrust::Feature>,
    /// The same, with ligatures off unless asked for: letter-spaced text has none.
    spaced_features: Vec<harfrust::Feature>,
    units_per_em: f32,
}

/// One line of one paragraph, in CSS pixels.
struct Line {
    glyphs: Shaped,
    /// The offset of the line's start from the column's left edge.
    x: f32,
    /// Added to the advance of each space (justified text).
    word_gap: f32,
    /// The clusters of the spaces in `glyphs`.
    spaces: Vec<u32>,
}

fn compose(page: &Page<'_>, sfnts: &[Cow<'_, [u8]>]) -> FmResult<Raster> {
    let bases = page
        .fonts
        .iter()
        .zip(sfnts)
        .map(|(f, sfnt)| {
            let base = harfrust::Font::new(sfnt.to_vec(), f.collection_index)
                .ok_or_else(|| malformed("harfrust cannot read the face"))?;
            if f.variations.is_empty() {
                return Ok(base);
            }
            let settings = f
                .variations
                .iter()
                .map(|v| Ok((Tag::new(&tag(&v.tag)?), v.value)))
                .collect::<FmResult<Vec<_>>>()?;
            Ok(base.instance_builder().variations(settings).build())
        })
        .collect::<FmResult<Vec<_>>>()?;
    let shapers: Vec<_> = bases.iter().map(harfrust::ShaperFont::new).collect();
    let faces = page
        .fonts
        .iter()
        .enumerate()
        .map(|(i, f)| {
            load_face(
                f,
                &sfnts[i],
                &shapers[i],
                f32::from(bases[i].units_per_em()),
            )
        })
        .collect::<FmResult<Vec<_>>>()?;

    let scale = page.scale as f32;
    let foreground = color(page.foreground)?;
    let mut canvas = Canvas::new(
        A4_WIDTH_PX * page.scale,
        A4_HEIGHT_PX * page.scale,
        color(page.background)?,
    );
    let fg = foreground.map(|c| (c * 255.0).round() as u8);

    let available = A4_WIDTH_PX as f32 - 2.0 * PAGE_MARGIN_PX;
    let width = layout_width(page.layout).map_or(available, |w| w.min(available));
    let left = (A4_WIDTH_PX as f32 - width) / 2.0;
    let mut top = PAGE_MARGIN_PX;
    let mut context = ScaleContext::new();

    for (i, block) in page.layout.blocks.iter().enumerate() {
        let face = page
            .block_fonts
            .get(i)
            .and_then(|at| faces.get(*at))
            .ok_or_else(|| FmError::invalid("A block has no font to be set in."))?;
        let size = block.size_pt * PX_PER_PT;
        let columns = f32::from(block.columns.max(1));
        let gap = COLUMN_GAP_EM * size;
        let column_width = ((width - (columns - 1.0) * gap) / columns).max(1.0);
        let paragraphs = block
            .text
            .split('\n')
            .map(|text| lay_out_paragraph(face, block, &collapse_spaces(text), column_width, size))
            .collect::<FmResult<Vec<_>>>()?;
        let line_height = layout_unit(size * block.line_height);
        let margin = layout_unit(block.paragraph_spacing_pt * PX_PER_PT);
        let counts: Vec<usize> = paragraphs.iter().map(Vec::len).collect();
        let placed = place_lines(&counts, line_height, margin, block.columns.max(1));

        let metrics = face.font.metrics(&face.coords).scale(size);
        let ascent = metrics.ascent.abs().round();
        let descent = metrics.descent.abs().round();
        // As Blink: half of the leading above the line, rounded down to a pixel.
        let half_leading = ((line_height - (ascent + descent)) / 2.0).floor();

        let mut scaler = context
            .builder(face.font)
            .size(size * scale)
            .hint(false)
            .normalized_coords(&face.coords)
            .build();
        let mut images: HashMap<(u16, u8), Option<Image>> = HashMap::new();
        let unit = size / face.units_per_em;
        let spacing = block.tracking / 1000.0 * size;
        for spot in &placed.lines {
            let line_top = top + spot.y;
            if line_top > A4_HEIGHT_PX as f32 {
                continue;
            }
            let line = &paragraphs[spot.paragraph][spot.line];
            let column_left = left + spot.column as f32 * (column_width + gap);
            let baseline = ((line_top + half_leading + ascent) * scale).round() as i32;
            let clip = if block.overflow == BlockOverflow::Clip {
                [
                    (column_left * scale).floor() as i32,
                    (line_top * scale).floor() as i32,
                    ((column_left + column_width) * scale).ceil() as i32,
                    ((line_top + line_height) * scale).ceil() as i32,
                ]
            } else {
                canvas.bounds()
            };
            let mut pen = column_left + line.x;
            let mut last_cluster = None;
            for (info, pos) in &line.glyphs {
                if last_cluster.is_some_and(|c| c != info.cluster) {
                    pen += spacing;
                }
                last_cluster = Some(info.cluster);
                let x = (pen + pos.x_offset as f32 * unit) * scale;
                let y = baseline - (pos.y_offset as f32 * unit * scale).round() as i32;
                pen += pos.x_advance as f32 * unit;
                if line.spaces.contains(&info.cluster) {
                    pen += line.word_gap;
                }
                let Ok(id) = u16::try_from(info.glyph_id) else {
                    continue;
                };
                let whole = x.floor();
                let steps = ((x - whole) * SUBPIXEL_STEPS).round() as u8;
                let image = images.entry((id, steps)).or_insert_with(|| {
                    Render::new(&GLYPH_SOURCES)
                        .format(Format::Alpha)
                        .offset(Vector::new(f32::from(steps) / SUBPIXEL_STEPS, 0.0))
                        .default_color(fg)
                        .render(&mut scaler, id)
                        .filter(|i| i.placement.width > 0 && i.placement.height > 0)
                });
                if let Some(image) = image {
                    canvas.blend_within(
                        whole as i32 + image.placement.left,
                        y - image.placement.top,
                        image,
                        foreground,
                        clip,
                    );
                }
            }
        }
        top += placed.height;
    }
    let (width, height) = (canvas.width, canvas.height);
    Ok(Raster {
        width,
        height,
        baseline: 0,
        rgba: canvas.into_rgba(),
    })
}

/// A length as Blink keeps it: rounded down to 1/64 of a pixel. Over a page the
/// difference decides which pixel a baseline rounds to.
fn layout_unit(px: f32) -> f32 {
    (px * 64.0).floor() / 64.0
}

/// The swash and harfrust views of one font.
fn load_face<'s, 'a, 'f>(
    font: &PageFont<'_>,
    sfnt: &'s [u8],
    shaper: &'s harfrust::ShaperFont<'a, 'f>,
    units_per_em: f32,
) -> FmResult<Face<'s, 'a, 'f>> {
    if font.variations.iter().any(|v| !v.value.is_finite()) {
        return Err(FmError::invalid("An axis value must be a number."));
    }
    if units_per_em <= 0.0 {
        return Err(malformed("units per em is not positive"));
    }
    let face = FontRef::from_index(sfnt, font.collection_index as usize)
        .ok_or_else(|| malformed("swash cannot read the face"))?;
    let settings = font
        .variations
        .iter()
        .map(|v| Ok((tag(&v.tag)?, v.value)))
        .collect::<FmResult<Vec<_>>>()?;
    let named: Vec<(&str, f32)> = settings
        .iter()
        .filter_map(|(t, v)| Some((std::str::from_utf8(t).ok()?, *v)))
        .collect();
    let coords: Vec<i16> = face
        .variations()
        .normalized_coords(named.iter().copied())
        .collect();
    let asked = font
        .features
        .iter()
        .map(|f| Ok((tag(&f.tag)?, f.on)))
        .collect::<FmResult<Vec<_>>>()?;
    let features: Vec<harfrust::Feature> = asked
        .iter()
        .map(|(t, on)| harfrust::Feature::new(Tag::new(t), u32::from(*on), ..))
        .collect();
    let mut spaced_features = features.clone();
    for ligature in [b"liga", b"clig"] {
        if !asked.iter().any(|(t, _)| t == ligature) {
            spaced_features.push(harfrust::Feature::new(Tag::new(ligature), 0, ..));
        }
    }
    Ok(Face {
        shaper,
        font: face,
        coords,
        features,
        spaced_features,
        units_per_em,
    })
}

/// CSS `white-space: normal` on one paragraph: runs of spaces and tabs become
/// one space, none at the ends. A no-break space stays.
fn collapse_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if !matches!(ch, ' ' | '\t' | '\r') {
            out.push(ch);
        } else if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// The width of shaped text in CSS pixels, letter spacing after each character included.
fn advance(glyphs: &Shaped, unit: f32, spacing: f32) -> f32 {
    let mut clusters = 0u32;
    let mut last = None;
    let mut total = 0.0;
    for (info, pos) in glyphs {
        if last != Some(info.cluster) {
            clusters += 1;
            last = Some(info.cluster);
        }
        total += pos.x_advance as f32 * unit;
    }
    total + clusters as f32 * spacing
}

/// Breaks one paragraph into lines of `width` and aligns them. An empty
/// paragraph has no lines, as an empty `<p>` has no height.
fn lay_out_paragraph(
    face: &Face<'_, '_, '_>,
    block: &TextBlock,
    text: &str,
    width: f32,
    size: f32,
) -> FmResult<Vec<Line>> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let rtl = first_strong_rtl(text);
    let unit = size / face.units_per_em;
    let spacing = block.tracking / 1000.0 * size;
    let features: &[harfrust::Feature] = if block.tracking == 0.0 {
        &face.features
    } else {
        &face.spaced_features
    };
    let shape = |part: &str| shape_runs(face.shaper, part, rtl, features);
    let indent = block.first_line_indent_em * size;

    let ranges: Vec<std::ops::Range<usize>> = if block.overflow == BlockOverflow::Clip {
        std::iter::once(0..text.len()).collect()
    } else {
        break_lines(text, width, indent, |part| {
            Ok(advance(&shape(part)?, unit, spacing))
        })?
    };

    let count = ranges.len();
    let mut lines = Vec::with_capacity(count);
    for (n, range) in ranges.into_iter().enumerate() {
        let part = text[range].trim_end_matches(' ');
        let glyphs = shape(part)?;
        let line_width = advance(&glyphs, unit, spacing);
        let inset = if n == 0 { indent } else { 0.0 };
        let free = (width - inset - line_width).max(0.0);
        let spaces: Vec<u32> = part
            .char_indices()
            .filter(|(_, c)| *c == ' ')
            .map(|(at, _)| at as u32)
            .collect();
        let justify =
            block.align == BlockAlign::Justify && n + 1 < count && !spaces.is_empty() && free > 0.0;
        let word_gap = if justify {
            free / spaces.len() as f32
        } else {
            0.0
        };
        // The indent is on the side the text starts from.
        let left_inset = if rtl { 0.0 } else { inset };
        let x = match (block.align, rtl) {
            (BlockAlign::Center, _) => left_inset + free / 2.0,
            (BlockAlign::Start | BlockAlign::Justify, false) | (BlockAlign::End, true) => {
                left_inset
            }
            (BlockAlign::Justify, true) if justify => left_inset,
            (BlockAlign::Start | BlockAlign::Justify, true) | (BlockAlign::End, false) => {
                left_inset + free
            }
        };
        lines.push(Line {
            glyphs,
            x,
            word_gap,
            spaces,
        });
    }
    Ok(lines)
}

/// Greedy line breaking: as much as fits, at UAX #14 opportunities; a space at
/// the end of a line hangs; a word wider than the line breaks between
/// characters (`overflow-wrap: break-word`). The first line starts after
/// `indent`. `measure` gives the width of a piece of text.
fn break_lines(
    text: &str,
    width: f32,
    indent: f32,
    mut measure: impl FnMut(&str) -> FmResult<f32>,
) -> FmResult<Vec<std::ops::Range<usize>>> {
    let mut widths: HashMap<String, f32> = HashMap::new();
    let mut measured = |part: &str| -> FmResult<f32> {
        if let Some(w) = widths.get(part) {
            return Ok(*w);
        }
        let w = measure(part)?;
        widths.insert(part.to_owned(), w);
        Ok(w)
    };
    let mut ends: Vec<usize> = text
        .char_indices()
        .zip(swash::text::analyze(text.chars()))
        .filter(|((at, _), (_, boundary))| {
            *at > 0 && matches!(boundary, Boundary::Line | Boundary::Mandatory)
        })
        .map(|((at, _), _)| at)
        .collect();
    ends.push(text.len());

    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut used = indent;
    let mut segment_start = 0usize;
    for segment_end in ends {
        let segment = &text[segment_start..segment_end];
        let trimmed = segment.trim_end_matches(' ');
        let bare = measured(trimmed)?;
        if used + bare > width + FIT_SLACK_PX && segment_start > line_start {
            lines.push(line_start..segment_start);
            line_start = segment_start;
            used = 0.0;
        }
        if used + bare > width + FIT_SLACK_PX {
            // One word wider than a whole line: break between its characters.
            for (offset, ch) in trimmed.char_indices() {
                let at = segment_start + offset;
                let w = measured(&text[at..at + ch.len_utf8()])?;
                if used + w > width + FIT_SLACK_PX && at > line_start {
                    lines.push(line_start..at);
                    line_start = at;
                    used = 0.0;
                }
                used += w;
            }
            used += measured(&segment[trimmed.len()..])?;
        } else {
            used += measured(segment)?;
        }
        segment_start = segment_end;
    }
    if line_start < text.len() {
        lines.push(line_start..text.len());
    }
    Ok(lines)
}

/// Where a line goes.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Spot {
    paragraph: usize,
    line: usize,
    column: u32,
    /// From the top of the block, in CSS pixels.
    y: f32,
}

/// The lines of a block, placed.
#[derive(Debug, Clone, PartialEq)]
struct Placed {
    lines: Vec<Spot>,
    height: f32,
    /// A break had to leave fewer than two lines on one side.
    strained: bool,
}

/// Places paragraphs of `counts` lines one under another, balanced over
/// `columns`: the shortest column height that holds them all.
fn place_lines(counts: &[usize], line_height: f32, margin: f32, columns: u8) -> Placed {
    if columns <= 1 {
        return fill_columns(counts, line_height, margin, f32::INFINITY);
    }
    let total: f32 = counts
        .iter()
        .map(|n| *n as f32 * line_height + margin)
        .sum();
    let columns = u32::from(columns);
    let step = BALANCE_STEP_PX.max(total / MAX_BALANCE_STEPS);
    let mut height = (total / columns as f32).max(line_height);
    loop {
        let placed = fill_columns(counts, line_height, margin, height);
        if !placed.strained && placed.lines.iter().all(|spot| spot.column < columns) {
            return placed;
        }
        height += step;
    }
}

/// Fills columns of `height` in order; the placed height is the tallest column.
fn fill_columns(counts: &[usize], line_height: f32, margin: f32, height: f32) -> Placed {
    let mut lines = Vec::new();
    let (mut column, mut y, mut tallest) = (0u32, 0.0f32, 0.0f32);
    let mut strained = false;
    for (paragraph, &count) in counts.iter().enumerate() {
        let mut line = 0;
        while line < count {
            let left = count - line;
            let fits = if height.is_finite() {
                ((height - y + FIT_SLACK_PX) / line_height).floor().max(0.0) as usize
            } else {
                left
            };
            let take = if fits >= left {
                left
            } else {
                // Keep two lines on each side of a break, or move on whole.
                let most = fits.min(left - MIN_LINES_AT_BREAK.min(left));
                let least = if line == 0 { MIN_LINES_AT_BREAK } else { 1 };
                if most >= least { most } else { 0 }
            };
            // An empty column takes at least one line, or nothing would move;
            // the height is then too small, and the caller tries a taller one.
            let take = if take == 0 && y <= 0.0 {
                strained = true;
                fits.clamp(1, left)
            } else {
                take
            };
            for k in 0..take {
                lines.push(Spot {
                    paragraph,
                    line: line + k,
                    column,
                    y: y + k as f32 * line_height,
                });
            }
            line += take;
            y += take as f32 * line_height;
            tallest = tallest.max(y);
            if line < count {
                column += 1;
                y = 0.0;
            }
        }
        y += margin;
        tallest = tallest.max(y);
    }
    Placed {
        lines,
        height: tallest,
        strained,
    }
}

/// The text and page colours of a surface: the light and dark theme's
/// `--fm-fg` and `--fm-bg` (`styles/theme.css`).
fn surface_colors(surface: LayoutSurface) -> (&'static str, &'static str) {
    match surface {
        LayoutSurface::Paper => ("#1a1a1a", "#ffffff"),
        LayoutSurface::Ink => ("#ececec", "#1c1c1e"),
    }
}

/// The distinct fonts of a request, its base font first, and each block's index among them.
pub fn request_fonts(
    req: &ExportLayoutPngRequest,
    blocks: usize,
) -> FmResult<(Vec<&LayoutFont>, Vec<usize>)> {
    if !matches!(req.scale, 1 | 2) {
        return Err(FmError::invalid("A layout exports at 1x or 2x.")
            .with_detail(format!("scale {}", req.scale)));
    }
    if req.block_fonts.len() > blocks {
        return Err(FmError::invalid(
            "A font was given for a block that does not exist.",
        ));
    }
    let mut fonts = vec![&req.font];
    let mut at = Vec::with_capacity(blocks);
    for i in 0..blocks {
        at.push(match req.block_fonts.get(i).and_then(Option::as_ref) {
            None => 0,
            Some(font) => fonts.iter().position(|f| *f == font).unwrap_or_else(|| {
                fonts.push(font);
                fonts.len() - 1
            }),
        });
    }
    Ok((fonts, at))
}

/// Renders a saved layout from the files of its fonts (path and collection
/// index, in the order of [`request_fonts`]) to a PNG file in `dir`. Reads the
/// font files; never writes them.
pub fn export_layout(
    saved: &SavedLayout,
    req: &ExportLayoutPngRequest,
    files: &[(&Path, u32)],
    dir: &Path,
) -> FmResult<PathBuf> {
    let (fonts, block_fonts) = request_fonts(req, saved.layout.blocks.len())?;
    let data = files
        .iter()
        .map(|(path, _)| {
            fs::read(path).map_err(|e| FmError::io("The font file could not be read.", path, &e))
        })
        .collect::<FmResult<Vec<_>>>()?;
    let page_fonts: Vec<PageFont<'_>> = fonts
        .iter()
        .zip(files)
        .zip(&data)
        .map(|((font, (_, index)), bytes)| PageFont {
            data: bytes,
            collection_index: *index,
            variations: &font.variations,
            features: &font.features,
        })
        .collect();
    let (foreground, background) = surface_colors(req.surface);
    let raster = render_page(&Page {
        layout: &saved.layout,
        fonts: &page_fonts,
        block_fonts: &block_fonts,
        scale: u32::from(req.scale),
        foreground,
        background,
    })?;
    let surface = match req.surface {
        LayoutSurface::Paper => "paper",
        LayoutSurface::Ink => "ink",
    };
    write_new(
        dir,
        &format!("{}-{surface}-{}x", saved.id, req.scale),
        &raster.to_compressed_png(),
    )
}

/// Writes `bytes` to `<stem>.png` in `dir`, or to the first free `<stem>-<n>.png`:
/// an earlier export is never replaced. A write that fails leaves no file.
fn write_new(dir: &Path, stem: &str, bytes: &[u8]) -> FmResult<PathBuf> {
    let failed = "The image could not be saved.";
    fs::create_dir_all(dir).map_err(|e| FmError::io(failed, dir, &e))?;
    let mut n = 1u32;
    loop {
        let name = if n == 1 {
            format!("{stem}.png")
        } else {
            format!("{stem}-{n}.png")
        };
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                return match file.write_all(bytes) {
                    Ok(()) => Ok(path),
                    Err(e) => {
                        let _ = fs::remove_file(&path);
                        Err(FmError::io(failed, &path, &e))
                    }
                };
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(FmError::io(failed, &path, &e)),
        }
    }
}

#[cfg(test)]
#[path = "page/tests.rs"]
mod tests;
