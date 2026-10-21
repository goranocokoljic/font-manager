//! The core rasterizer (E4.2): a line of text in one face, as pixels or a PNG.
//!
//! Grid thumbnails, the CLI and the MCP `render_preview` tool all draw here;
//! the UI font list does not (it renders with the webview's own engine through
//! the asset protocol). Text is shaped with `harfrust` (HarfBuzz rules, so
//! ligatures, kerning, marks and complex scripts come out as the OS shapes
//! them) and each glyph is rasterized with `swash`, unhinted and grayscale
//! antialiased, at the variation coordinates asked for. Colour glyphs (COLR,
//! embedded colour bitmaps) keep their colours; everything else takes the
//! foreground colour.
//!
//! The caller resolves the face to its file; this module never writes, moves
//! or keeps the file. Malformed font data is an error, never a panic.

pub mod cache;
mod png;

use std::borrow::Cow;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use fm_api::common::{FontFormat, Metrics};
use fm_api::preview::{PreviewImage, PreviewRequest};
use read_fonts::types::Tag;
use read_fonts::{FileRef, TableProvider};
use swash::FontRef;
use swash::scale::image::{Content, Image};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::text::{BidiClass, Codepoint as _};
use swash::zeno::{Format, Vector};

use crate::parser::decompress_web;
use crate::{FmError, FmResult};

/// The smallest and largest size a preview may ask for, in pixels.
pub const MIN_SIZE_PX: f32 = 8.0;
/// See [`MIN_SIZE_PX`].
pub const MAX_SIZE_PX: f32 = 300.0;
/// The longest text a preview renders, in characters.
pub const MAX_TEXT_CHARS: usize = 10_000;
/// The widest and tallest image a preview produces; longer lines are clipped.
pub const MAX_SIDE_PX: u32 = 8_192;

/// A rendered line of text: straight (not premultiplied) 8-bit RGBA, row by
/// row from the top. Thumbnails that composite or scale the pixels use this;
/// everything else takes the PNG from [`render_preview`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The distance from the top row to the baseline, in pixels.
    pub baseline: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl Raster {
    /// The pixel at `(x, y)` as `[r, g, b, a]`, or `None` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = (y as usize * self.width as usize + x as usize) * 4;
        let p = self.rgba.get(at..at + 4)?;
        Some([p[0], p[1], p[2], p[3]])
    }

    /// Encodes the raster as a PNG.
    pub fn to_png(&self) -> Vec<u8> {
        png::encode_rgba(self.width, self.height, &self.rgba)
    }
}

/// Renders `req` from the font file at `path` (face `collection_index` of a
/// collection, 0 otherwise) and encodes it as a PNG. Reads the file; never
/// writes it.
pub fn render_preview_file(
    path: &Path,
    collection_index: u32,
    req: &PreviewRequest,
) -> FmResult<PreviewImage> {
    let data =
        fs::read(path).map_err(|e| FmError::io("The font file could not be read.", path, &e))?;
    render_preview(&data, collection_index, req).map_err(|e| match e.detail() {
        Some(_) => e,
        None => e.with_detail(path.display().to_string()),
    })
}

/// Renders `req` from font data held in memory (TrueType, OpenType, a
/// collection, WOFF or WOFF2) and encodes it as a PNG.
pub fn render_preview(
    data: &[u8],
    collection_index: u32,
    req: &PreviewRequest,
) -> FmResult<PreviewImage> {
    let raster = rasterize(data, collection_index, req)?;
    Ok(PreviewImage {
        width: raster.width,
        height: raster.height,
        png: raster.to_png(),
    })
}

/// Renders `req` from font data held in memory into RGBA pixels. `req.face`
/// is not read: the caller has already resolved it to `data` and
/// `collection_index`.
///
/// Errors: a size outside 8–300 px, empty or over-long text, a colour that is
/// not CSS hex, or a tag that is not four ASCII characters is
/// [`fm_api::ErrorCode::InvalidRequest`]; a face index past the end of the
/// file is [`fm_api::ErrorCode::NotFound`]; data that is not a font the
/// rasterizer can read is [`fm_api::ErrorCode::Unsupported`].
pub fn rasterize(data: &[u8], collection_index: u32, req: &PreviewRequest) -> FmResult<Raster> {
    let style = Style::from_request(req)?;
    let sfnt = sfnt_data(data)?;
    check_face(&sfnt, collection_index)?;
    // The shaper and the scaler are well tested but not fuzzed against every
    // malformed font; a panic in either is a bad font, not a crash.
    panic::catch_unwind(AssertUnwindSafe(|| draw(&sfnt, collection_index, &style)))
        .unwrap_or_else(|_| Err(malformed("the shaper or scaler stopped on malformed data")))
}

/// The validated request.
#[derive(Debug)]
struct Style<'a> {
    text: &'a str,
    size: f32,
    variations: Vec<([u8; 4], f32)>,
    features: Vec<([u8; 4], bool)>,
    foreground: [f32; 4],
    background: [f32; 4],
}

impl<'a> Style<'a> {
    fn from_request(req: &'a PreviewRequest) -> FmResult<Self> {
        if !(MIN_SIZE_PX..=MAX_SIZE_PX).contains(&req.size_px) {
            return Err(FmError::invalid(
                "Preview size must be between 8 and 300 px.",
            ));
        }
        if req.text.is_empty() {
            return Err(FmError::invalid("Type some text to preview."));
        }
        if req.text.chars().count() > MAX_TEXT_CHARS {
            return Err(FmError::invalid(
                "Preview text must be at most 10,000 characters.",
            ));
        }
        let variations = req
            .variations
            .iter()
            .map(|v| Ok((tag(&v.tag)?, v.value)))
            .collect::<FmResult<Vec<_>>>()?;
        if req.variations.iter().any(|v| !v.value.is_finite()) {
            return Err(FmError::invalid("An axis value must be a number."));
        }
        let features = req
            .features
            .iter()
            .map(|f| Ok((tag(&f.tag)?, f.on)))
            .collect::<FmResult<Vec<_>>>()?;
        let foreground = match &req.foreground {
            Some(hex) => color(hex)?,
            None => [0.0, 0.0, 0.0, 1.0],
        };
        let background = match &req.background {
            Some(hex) => color(hex)?,
            None => [0.0; 4],
        };
        Ok(Self {
            text: &req.text,
            size: req.size_px,
            variations,
            features,
            foreground,
            background,
        })
    }
}

/// A four-character OpenType tag (`wght`, `liga`).
pub(crate) fn tag(s: &str) -> FmResult<[u8; 4]> {
    <[u8; 4]>::try_from(s.as_bytes())
        .ok()
        .filter(|b| b.iter().all(|c| (0x20..=0x7E).contains(c)))
        .ok_or_else(|| {
            FmError::invalid("An axis or feature tag must be four characters, like wght.")
                .with_detail(format!("tag {s:?}"))
        })
}

/// A CSS hex colour (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`) as straight RGBA
/// in 0–1.
fn color(hex: &str) -> FmResult<[f32; 4]> {
    let bad = || {
        FmError::invalid("A colour must be CSS hex, like #1a1a1a.")
            .with_detail(format!("colour {hex:?}"))
    };
    let digits = hex.strip_prefix('#').ok_or_else(bad)?;
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let nibble = |i: usize| u8::from_str_radix(&digits[i..=i], 16).map_err(|_| bad());
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).map_err(|_| bad());
    let rgba = match digits.len() {
        3 | 4 => {
            let mut out = [255u8; 4];
            for (i, slot) in out.iter_mut().enumerate().take(digits.len()) {
                *slot = nibble(i)? * 17;
            }
            out
        }
        6 | 8 => {
            let mut out = [255u8; 4];
            for (i, slot) in out.iter_mut().enumerate().take(digits.len() / 2) {
                *slot = byte(i * 2)?;
            }
            out
        }
        _ => return Err(bad()),
    };
    Ok(rgba.map(|c| f32::from(c) / 255.0))
}

/// The vertical metrics of face `collection_index` of font data held in
/// memory (see [`crate::metrics::read`]). Malformed data is an error, never a
/// panic.
pub fn face_metrics(data: &[u8], collection_index: u32) -> FmResult<Metrics> {
    let sfnt = sfnt_data(data)?;
    let font = read_fonts::FontRef::from_index(&sfnt, collection_index)
        .map_err(|e| malformed(format!("the font header: {e}")))?;
    font.head()
        .map_err(|e| malformed(format!("the head table: {e}")))?;
    Ok(crate::metrics::read(&font))
}

fn malformed(detail: impl Into<String>) -> FmError {
    FmError::unsupported("This face cannot be rendered: its font data is malformed.")
        .with_detail(detail)
}

/// The TrueType or OpenType bytes: the data itself, or the font inside a
/// WOFF or WOFF2 file.
fn sfnt_data(data: &[u8]) -> FmResult<Cow<'_, [u8]>> {
    let web = match data.get(..4) {
        Some(b"wOFF") => FontFormat::Woff,
        Some(b"wOF2") => FontFormat::Woff2,
        _ => return Ok(Cow::Borrowed(data)),
    };
    decompress_web(data, web)
        .map(Cow::Owned)
        .ok_or_else(|| malformed("the WOFF data cannot be decompressed"))
}

/// The face exists and holds the tables a rasterizer needs, each inside the
/// file. The shaper and the scaler skip a missing or cut-off table silently
/// and would draw an empty image; a font the scan marks broken is an error
/// here too.
fn check_face(sfnt: &[u8], index: u32) -> FmResult<()> {
    let count = match FileRef::new(sfnt) {
        Ok(FileRef::Font(_)) => 1,
        Ok(FileRef::Collection(c)) => c.len(),
        Err(e) => return Err(malformed(format!("the font header: {e}"))),
    };
    if index >= count {
        return Err(FmError::not_found(format!("Face {index} of the font file"))
            .with_detail(format!("the file holds {count} face(s)")));
    }
    let font = read_fonts::FontRef::from_index(sfnt, index)
        .map_err(|e| malformed(format!("the face header: {e}")))?;
    let has = |tag: &[u8; 4]| font.table_data(Tag::new(tag)).is_some();
    for required in [b"head", b"hhea", b"maxp", b"cmap", b"hmtx"] {
        if !has(required) {
            return Err(malformed(format!(
                "the {} table is missing or cut off",
                String::from_utf8_lossy(required)
            )));
        }
    }
    let glyphs = (has(b"glyf") && has(b"loca"))
        || has(b"CFF ")
        || has(b"CFF2")
        || has(b"CBDT")
        || has(b"sbix");
    if !glyphs {
        return Err(malformed("the face has no outline or bitmap glyph table"));
    }
    font.head()
        .map(|_| ())
        .and(font.maxp().map(|_| ()))
        .and(font.cmap().map(|_| ()))
        .map_err(|e| malformed(format!("a required table is malformed: {e}")))
}

/// A run of text in one direction: a byte range of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    start: usize,
    end: usize,
    rtl: bool,
}

/// Splits one line of text into direction runs, in the order they are drawn
/// left to right: the Unicode bidirectional algorithm for a single line
/// without explicit embeddings (rules P2–P3, W1, W7, N1–N2, I1–I2, L2).
/// That is what a one-line preview needs; isolates and embedding controls
/// are treated as neutrals.
fn visual_runs(text: &str) -> Vec<Run> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Kind {
        Ltr,
        Rtl,
        Number,
        Neutral,
    }
    let mut chars: Vec<(usize, Kind)> = Vec::with_capacity(text.len());
    let mut prev = Kind::Neutral;
    for (at, ch) in text.char_indices() {
        let kind = match ch.bidi_class() {
            BidiClass::L => Kind::Ltr,
            BidiClass::R | BidiClass::AL => Kind::Rtl,
            BidiClass::EN | BidiClass::AN => Kind::Number,
            // W1: a combining mark takes the type of the character before it.
            BidiClass::NSM => prev,
            _ => Kind::Neutral,
        };
        chars.push((at, kind));
        prev = kind;
    }
    let para_rtl = chars
        .iter()
        .find_map(|&(_, k)| match k {
            Kind::Ltr => Some(false),
            Kind::Rtl => Some(true),
            _ => None,
        })
        .unwrap_or(false);
    let para = if para_rtl { Kind::Rtl } else { Kind::Ltr };

    // W7: a number after left-to-right text (or at the start of a
    // left-to-right line) reads as left-to-right.
    let mut last_strong = para;
    for (_, kind) in &mut chars {
        match *kind {
            Kind::Ltr | Kind::Rtl => last_strong = *kind,
            Kind::Number if last_strong == Kind::Ltr => *kind = Kind::Ltr,
            _ => {}
        }
    }
    // N1–N2: neutrals between two runs of the same direction (numbers count
    // as right-to-left) take that direction, all others the paragraph's.
    let strong = |k: Kind| match k {
        Kind::Ltr => Some(Kind::Ltr),
        Kind::Rtl | Kind::Number => Some(Kind::Rtl),
        Kind::Neutral => None,
    };
    let mut i = 0;
    while i < chars.len() {
        if chars[i].1 != Kind::Neutral {
            i += 1;
            continue;
        }
        let end = (i..chars.len())
            .find(|&j| chars[j].1 != Kind::Neutral)
            .unwrap_or(chars.len());
        let before = if i == 0 {
            Some(para)
        } else {
            strong(chars[i - 1].1)
        };
        let after = chars.get(end).map_or(Some(para), |c| strong(c.1));
        let resolved = if before == after {
            before.unwrap_or(para)
        } else {
            para
        };
        for c in &mut chars[i..end] {
            c.1 = resolved;
        }
        i = end;
    }
    // I1–I2: embedding levels.
    let level = |k: Kind| -> u8 {
        match (para_rtl, k) {
            (false, Kind::Ltr) => 0,
            (false, Kind::Rtl) => 1,
            (true, Kind::Rtl) => 1,
            _ => 2,
        }
    };
    let mut runs: Vec<(Run, u8)> = Vec::new();
    for (n, &(at, kind)) in chars.iter().enumerate() {
        let lv = level(kind);
        let end = chars.get(n + 1).map_or(text.len(), |c| c.0);
        match runs.last_mut() {
            Some((run, l)) if *l == lv => run.end = end,
            _ => runs.push((
                Run {
                    start: at,
                    end,
                    rtl: lv % 2 == 1,
                },
                lv,
            )),
        }
    }
    // L2: from the highest level down to the lowest odd one, reverse every
    // sequence of runs at that level or higher.
    let max = runs.iter().map(|r| r.1).max().unwrap_or(0);
    for k in (1..=max).rev() {
        let mut i = 0;
        while i < runs.len() {
            if runs[i].1 < k {
                i += 1;
                continue;
            }
            let end = (i..runs.len())
                .find(|&j| runs[j].1 < k)
                .unwrap_or(runs.len());
            runs[i..end].reverse();
            i = end;
        }
    }
    runs.into_iter().map(|(run, _)| run).collect()
}

/// A glyph image and where its top-left pixel goes, relative to the pen
/// origin (x right, y down from the baseline).
struct Placed {
    x: i32,
    y: i32,
    image: Image,
}

fn draw(sfnt: &[u8], index: u32, style: &Style<'_>) -> FmResult<Raster> {
    // harfrust reads through its own `Font`, which owns its bytes; the copy
    // lives only for this render.
    let base = harfrust::Font::new(sfnt.to_vec(), index)
        .ok_or_else(|| malformed("harfrust cannot read the face"))?;
    let font = FontRef::from_index(sfnt, index as usize)
        .ok_or_else(|| malformed("swash cannot read the face"))?;

    let face = if style.variations.is_empty() {
        base
    } else {
        base.instance_builder()
            .variations(
                style
                    .variations
                    .iter()
                    .map(|(t, value)| (Tag::new(t), *value)),
            )
            .build()
    };
    let shaper = harfrust::ShaperFont::new(&face);
    let features: Vec<harfrust::Feature> = style
        .features
        .iter()
        .map(|(t, on)| harfrust::Feature::new(Tag::new(t), u32::from(*on), ..))
        .collect();

    // Each direction run is shaped on its own; harfrust returns the glyphs
    // of a right-to-left run already in visual order. Positions come back in
    // font units (no scale is set).
    let mut glyphs: Vec<(harfrust::GlyphInfo, harfrust::GlyphPosition)> = Vec::new();
    for run in visual_runs(style.text) {
        let mut buffer = harfrust::Buffer::new();
        buffer.push_str(&style.text[run.start..run.end]);
        buffer.set_direction(if run.rtl {
            harfrust::Direction::RightToLeft
        } else {
            harfrust::Direction::LeftToRight
        });
        buffer.guess_segment_properties();
        harfrust::shape(
            &shaper,
            &mut buffer,
            harfrust::ShapeOptions::new().features(&features),
        )
        .map_err(|e| malformed(format!("harfrust cannot shape the text: {e:?}")))?;
        glyphs.extend(
            buffer
                .glyph_infos()
                .iter()
                .copied()
                .zip(buffer.glyph_positions().iter().copied()),
        );
    }

    let upem = face.units_per_em();
    if upem == 0 {
        return Err(malformed("units per em is not positive"));
    }
    let scale = style.size / upem as f32;

    let settings: Vec<(&str, f32)> = style
        .variations
        .iter()
        .filter_map(|(t, v)| Some((std::str::from_utf8(t).ok()?, *v)))
        .collect();
    let coords: Vec<i16> = font
        .variations()
        .normalized_coords(settings.iter().copied())
        .collect();
    let metrics = font.metrics(&coords).scale(style.size);
    let ascent = metrics.ascent.abs().ceil() as i32;
    let descent = metrics.descent.abs().ceil() as i32;

    let mut context = ScaleContext::new();
    let mut scaler = context
        .builder(font)
        .size(style.size)
        .hint(false)
        .normalized_coords(&coords)
        .build();
    let sources = [
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ];
    let fg = style.foreground.map(|c| (c * 255.0).round() as u8);

    let mut placed = Vec::with_capacity(glyphs.len());
    let mut pen = 0.0f32;
    for (info, pos) in &glyphs {
        let x = pen + pos.x_offset as f32 * scale;
        let y = -(pos.y_offset as f32 * scale).round() as i32;
        pen += pos.x_advance as f32 * scale;
        // Past the widest canvas nothing is drawn; skip the rasterizing.
        if x > MAX_SIDE_PX as f32 + style.size {
            continue;
        }
        let Ok(id) = u16::try_from(info.glyph_id) else {
            continue;
        };
        let image = Render::new(&sources)
            .format(Format::Alpha)
            .offset(Vector::new(x.fract(), 0.0))
            .default_color(fg)
            .render(&mut scaler, id);
        if let Some(image) = image.filter(|i| i.placement.width > 0 && i.placement.height > 0) {
            placed.push(Placed {
                x: x.floor() as i32 + image.placement.left,
                y: y - image.placement.top,
                image,
            });
        }
    }

    // The canvas covers the line box (advance width by ascent + descent) and
    // every pixel of ink, so overhangs and tall marks are never cut off.
    let mut min_x = 0i32;
    let mut max_x = pen.ceil().max(1.0) as i32;
    let mut min_y = -ascent;
    let mut max_y = descent.max(min_y + 1);
    for p in &placed {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x + p.image.placement.width as i32);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y + p.image.placement.height as i32);
    }
    let width = ((max_x - min_x) as u32).clamp(1, MAX_SIDE_PX);
    let height = ((max_y - min_y) as u32).clamp(1, MAX_SIDE_PX);

    let mut canvas = Canvas::new(width, height, style.background);
    for p in &placed {
        canvas.blend(p.x - min_x, p.y - min_y, &p.image, style.foreground);
    }
    Ok(Raster {
        width,
        height,
        baseline: (-min_y) as u32,
        rgba: canvas.into_rgba(),
    })
}

/// Premultiplied RGBA in 0–1, composited with source-over.
struct Canvas {
    width: u32,
    height: u32,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(width: u32, height: u32, background: [f32; 4]) -> Self {
        let [r, g, b, a] = background;
        Self {
            width,
            height,
            px: vec![[r * a, g * a, b * a, a]; width as usize * height as usize],
        }
    }

    fn blend(&mut self, left: i32, top: i32, image: &Image, foreground: [f32; 4]) {
        let w = image.placement.width as usize;
        let h = image.placement.height as usize;
        let channels = match image.content {
            Content::Mask => 1,
            Content::SubpixelMask | Content::Color => 4,
        };
        for row in 0..h {
            let y = top + row as i32;
            if y < 0 || y >= self.height as i32 {
                continue;
            }
            for col in 0..w {
                let x = left + col as i32;
                if x < 0 || x >= self.width as i32 {
                    continue;
                }
                let at = (row * w + col) * channels;
                let Some(src) = image.data.get(at..at + channels) else {
                    continue;
                };
                let [r, g, b, a] = match image.content {
                    Content::Mask => {
                        let cover = f32::from(src[0]) / 255.0;
                        let [r, g, b, a] = foreground;
                        [r, g, b, a * cover]
                    }
                    Content::SubpixelMask => {
                        let cover = (u32::from(src[0]) + u32::from(src[1]) + u32::from(src[2]))
                            as f32
                            / (3.0 * 255.0);
                        let [r, g, b, a] = foreground;
                        [r, g, b, a * cover]
                    }
                    Content::Color => {
                        [src[0], src[1], src[2], src[3]].map(|c| f32::from(c) / 255.0)
                    }
                };
                let dst = &mut self.px[y as usize * self.width as usize + x as usize];
                let keep = 1.0 - a;
                *dst = [
                    r * a + dst[0] * keep,
                    g * a + dst[1] * keep,
                    b * a + dst[2] * keep,
                    a + dst[3] * keep,
                ];
            }
        }
    }

    fn into_rgba(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for [r, g, b, a] in self.px {
            let unmul = |c: f32| {
                if a > 0.0 {
                    ((c / a).clamp(0.0, 1.0) * 255.0).round() as u8
                } else {
                    0
                }
            };
            out.extend_from_slice(&[
                unmul(r),
                unmul(g),
                unmul(b),
                (a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use fm_api::ErrorCode;
    use fm_api::common::{AxisValue, FeatureSetting};
    use fm_api::ids::FaceId;
    use read_fonts::TableProvider;
    use std::path::PathBuf;

    fn fixture(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts")
            .join(rel)
    }

    fn bytes(rel: &str) -> Vec<u8> {
        fs::read(fixture(rel)).unwrap()
    }

    const ASAP: &str = "google-repo/asapsharp/AsapSharp[wdth,wght].ttf";
    const TTC: &str = "derived/agdasima/agdasima.ttc";
    const WOFF2: &str = "fontshare/archivo/Archivo-Bold.woff2";
    const TRUNCATED: &str = "derived/agdasima/agdasima-truncated.ttf";

    fn req(text: &str, size_px: f32) -> PreviewRequest {
        PreviewRequest {
            face: FaceId(1),
            text: text.to_string(),
            size_px,
            variations: Vec::new(),
            features: Vec::new(),
            foreground: None,
            background: None,
        }
    }

    /// Total alpha of the image: how much ink it holds.
    fn ink(r: &Raster) -> u64 {
        r.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| u64::from(p[3]))
            .sum()
    }

    /// The first and last rows with ink, top down.
    fn ink_rows(r: &Raster) -> (u32, u32) {
        let rows: Vec<u32> = (0..r.height)
            .filter(|&y| (0..r.width).any(|x| r.pixel(x, y).unwrap()[3] > 127))
            .collect();
        (rows[0], *rows.last().unwrap())
    }

    fn code(result: FmResult<Raster>) -> ErrorCode {
        result.unwrap_err().code()
    }

    #[test]
    fn draws_text_in_the_foreground_colour_on_a_transparent_background() {
        let mut r = req("Hamburg", 48.0);
        r.foreground = Some("#ff0000".into());
        let out = rasterize(&bytes(ASAP), 0, &r).unwrap();
        assert!(
            out.width > 100 && out.height >= 48,
            "{}x{}",
            out.width,
            out.height
        );
        assert_eq!(out.rgba.len(), (out.width * out.height * 4) as usize);
        assert_eq!(out.pixel(0, 0).unwrap()[3], 0, "corner is transparent");
        let solid: Vec<&[u8; 4]> = out
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] == 255)
            .collect();
        assert!(solid.len() > 200, "solid ink pixels: {}", solid.len());
        assert!(solid.iter().all(|p| **p == [255, 0, 0, 255]));
    }

    #[test]
    fn fills_the_background_colour() {
        let mut r = req("Hamburg", 24.0);
        r.foreground = Some("#fff".into());
        r.background = Some("#00ff0080".into());
        let out = rasterize(&bytes(ASAP), 0, &r).unwrap();
        assert_eq!(out.pixel(0, 0).unwrap(), [0, 255, 0, 128]);
        assert!(out.rgba.as_chunks::<4>().0.iter().all(|p| p[3] >= 128));
    }

    #[test]
    fn the_image_scales_with_the_size() {
        let small = rasterize(&bytes(ASAP), 0, &req("Hamburg", 40.0)).unwrap();
        let big = rasterize(&bytes(ASAP), 0, &req("Hamburg", 120.0)).unwrap();
        let ratio = big.width as f32 / small.width as f32;
        assert!((2.8..=3.2).contains(&ratio), "width ratio {ratio}");
        let ratio = big.height as f32 / small.height as f32;
        assert!((2.8..=3.2).contains(&ratio), "height ratio {ratio}");
    }

    #[test]
    fn axis_values_change_the_outlines() {
        let at = |wght: f32| {
            let mut r = req("Hamburg", 64.0);
            r.variations = vec![AxisValue {
                tag: "wght".into(),
                value: wght,
            }];
            rasterize(&bytes(ASAP), 0, &r).unwrap()
        };
        let (thin, black) = (ink(&at(100.0)), ink(&at(900.0)));
        assert!(
            black as f64 > thin as f64 * 1.5,
            "thin {thin}, black {black}"
        );
        // An axis the font does not have is ignored, like the OS does.
        let mut r = req("Hamburg", 64.0);
        r.variations = vec![AxisValue {
            tag: "ZZZZ".into(),
            value: 3.0,
        }];
        assert_eq!(
            rasterize(&bytes(ASAP), 0, &r).unwrap(),
            rasterize(&bytes(ASAP), 0, &req("Hamburg", 64.0)).unwrap()
        );
    }

    #[test]
    fn features_switch_shaping_on_and_off() {
        let with = |on: bool| {
            let mut r = req("AVAVAVTyTy", 64.0);
            r.features = vec![FeatureSetting {
                tag: "kern".into(),
                on,
            }];
            rasterize(&bytes(ASAP), 0, &r).unwrap().width
        };
        let (kerned, plain) = (with(true), with(false));
        assert!(kerned < plain, "kerned {kerned}, unkerned {plain}");
    }

    /// Criterion 2: the rasterizer places glyphs with the font's own metrics,
    /// as OS renderers do — cap height from OS/2, advance widths from hmtx.
    #[test]
    fn metrics_match_the_font() {
        let data = bytes("google-static/robotoslab/RobotoSlab-ExtraBold.ttf");
        let font = read_fonts::FontRef::new(&data).unwrap();
        let upem = f32::from(font.head().unwrap().units_per_em());
        let cap = f32::from(font.os2().unwrap().s_cap_height().unwrap());
        let size = 100.0;

        let out = rasterize(&data, 0, &req("H", size)).unwrap();
        let (top, bottom) = ink_rows(&out);
        let ink_height = (bottom + 1 - top) as f32;
        let expected = cap * size / upem;
        assert!(
            (ink_height - expected).abs() <= 1.0,
            "cap {ink_height} vs {expected}"
        );
        assert_eq!(bottom + 1, out.baseline, "H sits on the baseline");

        // Ten capital Hs advance by exactly ten hmtx advances (no kerning
        // between identical Hs in this font).
        let gid = font.cmap().unwrap().map_codepoint('H').unwrap();
        let advance = f32::from(font.hmtx().unwrap().advance(gid).unwrap());
        let ten = rasterize(&data, 0, &req("HHHHHHHHHH", size)).unwrap();
        let expected = (advance * 10.0 * size / upem).ceil() as u32;
        assert!(
            ten.width.abs_diff(expected) <= 1,
            "{} vs {expected}",
            ten.width
        );
    }

    #[test]
    fn each_face_of_a_collection_renders_on_its_own() {
        let data = bytes(TTC);
        let bold = rasterize(&data, 0, &req("Agdasima", 48.0)).unwrap();
        let regular = rasterize(&data, 1, &req("Agdasima", 48.0)).unwrap();
        assert_ne!(bold, regular);
        assert_eq!(
            code(rasterize(&data, 2, &req("Agdasima", 48.0))),
            ErrorCode::NotFound
        );
        assert_eq!(
            code(rasterize(&bytes(ASAP), 1, &req("Agdasima", 48.0))),
            ErrorCode::NotFound
        );
    }

    #[test]
    fn renders_woff2_files() {
        let out = rasterize(&bytes(WOFF2), 0, &req("Archivo", 48.0)).unwrap();
        assert!(ink(&out) > 0);
    }

    #[test]
    fn malformed_font_data_is_unsupported_not_a_panic() {
        let r = req("Hello", 48.0);
        assert_eq!(
            code(rasterize(&bytes(TRUNCATED), 0, &r)),
            ErrorCode::Unsupported
        );
        assert_eq!(
            code(rasterize(b"hello world, not a font", 0, &r)),
            ErrorCode::Unsupported
        );
        assert_eq!(code(rasterize(&[], 0, &r)), ErrorCode::Unsupported);
        let mut woff = bytes(WOFF2);
        woff.truncate(200);
        assert_eq!(code(rasterize(&woff, 0, &r)), ErrorCode::Unsupported);
    }

    #[test]
    fn invalid_requests_are_refused() {
        let data = bytes(ASAP);
        for size in [7.9, 300.1, f32::NAN] {
            assert_eq!(
                code(rasterize(&data, 0, &req("A", size))),
                ErrorCode::InvalidRequest
            );
        }
        assert_eq!(
            code(rasterize(&data, 0, &req("", 20.0))),
            ErrorCode::InvalidRequest
        );
        let long = "a".repeat(MAX_TEXT_CHARS + 1);
        assert_eq!(
            code(rasterize(&data, 0, &req(&long, 20.0))),
            ErrorCode::InvalidRequest
        );
        for hex in ["red", "#12", "#gggggg", "123456", "#12345", "#\u{fc}12"] {
            let mut r = req("A", 20.0);
            r.foreground = Some(hex.into());
            assert_eq!(
                code(rasterize(&data, 0, &r)),
                ErrorCode::InvalidRequest,
                "{hex}"
            );
        }
        let mut r = req("A", 20.0);
        r.variations = vec![AxisValue {
            tag: "wg".into(),
            value: 400.0,
        }];
        assert_eq!(code(rasterize(&data, 0, &r)), ErrorCode::InvalidRequest);
        r.variations[0] = AxisValue {
            tag: "wght".into(),
            value: f32::INFINITY,
        };
        assert_eq!(code(rasterize(&data, 0, &r)), ErrorCode::InvalidRequest);
        let mut r = req("A", 20.0);
        r.features = vec![FeatureSetting {
            tag: "ligatures".into(),
            on: false,
        }];
        assert_eq!(code(rasterize(&data, 0, &r)), ErrorCode::InvalidRequest);
    }

    #[test]
    fn css_hex_colours_parse() {
        assert_eq!(color("#000").unwrap(), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(color("#fFf8").unwrap(), [1.0, 1.0, 1.0, 136.0 / 255.0]);
        assert_eq!(color("#ff0000").unwrap(), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(color("#00000000").unwrap(), [0.0; 4]);
    }

    #[test]
    fn a_long_line_is_clipped_not_unbounded() {
        let out = rasterize(&bytes(ASAP), 0, &req(&"W".repeat(2_000), 300.0)).unwrap();
        assert_eq!(out.width, MAX_SIDE_PX);
        assert_eq!(out.rgba.len(), (out.width * out.height * 4) as usize);
    }

    #[test]
    fn whitespace_renders_an_empty_line() {
        let out = rasterize(&bytes(ASAP), 0, &req("   ", 20.0)).unwrap();
        assert!(out.width >= 1 && out.height >= 1);
        assert_eq!(ink(&out), 0);
    }

    #[test]
    fn complex_scripts_are_shaped() {
        // Arabic joins: the shaped word is narrower than its letters drawn
        // one by one in their isolated forms.
        let data = bytes("google-repo/cairoplay/CairoPlay[slnt,wght].ttf");
        let word = rasterize(&data, 0, &req("\u{633}\u{644}\u{627}\u{645}", 48.0)).unwrap();
        let apart = rasterize(&data, 0, &req("\u{633} \u{644} \u{627} \u{645}", 48.0)).unwrap();
        assert!(ink(&word) > 0);
        assert!(
            word.width < apart.width,
            "{} vs {}",
            word.width,
            apart.width
        );
    }

    /// Criterion 3: the one entry the CLI, the MCP tool and thumbnails call
    /// returns the `fm-api` type, from a file on any path.
    #[test]
    fn renders_from_a_file_into_the_api_type() {
        let dir = TempDir::new("preview-file");
        let path = dir.path().join("Schrift-\u{fc} \u{5b57}\u{4f53}.ttf");
        fs::copy(fixture(ASAP), &path).unwrap();
        let r = req("Hamburg", 32.0);
        let image = render_preview_file(&path, 0, &r).unwrap();
        let raster = rasterize(&bytes(ASAP), 0, &r).unwrap();
        let decoded = png::tests::decode(&image.png);
        assert_eq!((image.width, image.height), (raster.width, raster.height));
        assert_eq!(
            (decoded.width, decoded.height),
            (raster.width, raster.height)
        );
        assert_eq!(decoded.rgba, raster.rgba);
        assert_eq!(
            fs::read(&path).unwrap(),
            bytes(ASAP),
            "the file is untouched"
        );

        let err = render_preview_file(&dir.path().join("gone.ttf"), 0, &r).unwrap_err();
        assert_eq!(err.code(), ErrorCode::NotFound);
        let junk = dir.path().join("junk.ttf");
        fs::write(&junk, b"junk").unwrap();
        let err = render_preview_file(&junk, 0, &r).unwrap_err();
        assert_eq!(err.code(), ErrorCode::Unsupported);
    }

    #[test]
    fn colour_glyphs_keep_their_colours() {
        // CairoPlay is a COLR v0 font: its layers come from the palette.
        let data = bytes("google-repo/cairoplay/CairoPlay[slnt,wght].ttf");
        let mut r = req("\u{628}\u{633}\u{645} Cairo", 64.0);
        r.foreground = Some("#000000".into());
        let out = rasterize(&data, 0, &r).unwrap();
        // The palette paints the dots red; the black foreground has no hue.
        let coloured = out
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 128 && p[0].abs_diff(p[1]) > 80)
            .count();
        assert!(coloured > 20, "coloured pixels: {coloured}");
    }

    fn runs(text: &str) -> Vec<(&str, bool)> {
        visual_runs(text)
            .into_iter()
            .map(|r| (&text[r.start..r.end], r.rtl))
            .collect()
    }

    #[test]
    fn mixed_direction_text_is_ordered_like_the_os_orders_it() {
        let salam = "\u{633}\u{644}\u{627}\u{645}";
        assert_eq!(runs("Hamburg"), [("Hamburg", false)]);
        assert_eq!(runs(salam), [(salam, true)]);
        // A left-to-right line keeps its order; the Arabic word runs right
        // to left inside it.
        let text = format!("Hi {salam} you");
        assert_eq!(
            runs(&text),
            [("Hi ", false), (salam, true), (" you", false)]
        );
        // A right-to-left line puts its first word on the right.
        let text = format!("{salam} Cairo");
        assert_eq!(
            runs(&text),
            [("Cairo", false), (&*format!("{salam} "), true)]
        );
        // Numbers inside right-to-left text still read left to right.
        let text = format!("{salam} 2026");
        assert_eq!(
            runs(&text),
            [("2026", false), (&*format!("{salam} "), true)]
        );
        assert_eq!(runs("   "), [("   ", false)]);
    }

    #[test]
    fn a_right_to_left_line_draws_its_latin_word_on_the_left() {
        // In "سلام Cairo" the Latin word is drawn first (on the left) and
        // reads forwards: its glyphs match "Cairo" drawn alone.
        let data = bytes("google-repo/cairoplay/CairoPlay[slnt,wght].ttf");
        let mixed = rasterize(&data, 0, &req("\u{633}\u{644}\u{627}\u{645} Cairo", 48.0)).unwrap();
        let latin = rasterize(&data, 0, &req("Cairo", 48.0)).unwrap();
        let column = |r: &Raster, x: u32| -> Vec<u8> {
            (0..r.height.min(latin.height))
                .map(|y| r.pixel(x, y).unwrap()[3])
                .collect()
        };
        assert_eq!(mixed.baseline, latin.baseline);
        for x in 0..latin.width.saturating_sub(2) {
            assert_eq!(column(&mixed, x), column(&latin, x), "column {x}");
        }
    }

    #[test]
    fn colr_v1_fonts_fall_back_to_their_outlines() {
        // swash paints COLR v0 only; a COLR v1 font still renders, in the
        // foreground colour, from its plain outlines.
        let data = bytes("google-repo/arefruqaaink/ArefRuqaaInk-Regular.ttf");
        let out = rasterize(&data, 0, &req("\u{628}\u{633}\u{645}", 64.0)).unwrap();
        assert!(ink(&out) > 0);
    }
}
