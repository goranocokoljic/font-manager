//! `fonts.css` for a web export (E20.3): one `@font-face` block per face,
//! built beside [`crate::css`] and never in an adapter.
//!
//! - **Family**: the typographic family (name ID 16, else 1), as a CSS
//!   string: `"` and `\` are escaped, a control character is a hex escape.
//! - **Sources**: relative `url()`s of the written files, WOFF 2 before
//!   WOFF whatever order the request lists them in, each with `format()`.
//! - **Style**: `font-display: swap`. A static face has one `font-weight`
//!   (`OS/2.usWeightClass`) and the `font-style` of its slant. A variable
//!   face writes ranges from its axes: `font-weight: <min> <max>` from
//!   `wght`, `font-stretch: <min>% <max>%` from `wdth`, and
//!   `font-style: oblique <a>deg <b>deg` from `slnt`. CSS oblique angles lean
//!   right when positive and `slnt` leans right when negative, so the range
//!   is the axis range negated (`slnt` −10…0 is `oblique 0deg 10deg`).
//! - **Subset**: with a subset, `unicode-range` lists the code points the
//!   written file maps, collapsed into ranges; without one it is left out.

use fm_api::common::{Axis, Slant};
use fm_api::export::WebFormat;

use crate::css::number;
use crate::web_font::extension;

/// What a face's `@font-face` block says about it.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceStyle {
    /// The typographic family.
    pub family: String,
    /// The face's slant.
    pub slant: Slant,
    /// `OS/2.usWeightClass`.
    pub weight: u16,
    /// Variation axes; empty for a static face.
    pub axes: Vec<Axis>,
}

/// One `@font-face` block for `style`, whose files are `file_stem` in
/// `formats`, mapping `code_points` when the export was subset.
pub fn font_face(
    style: &FaceStyle,
    file_stem: &str,
    formats: &[WebFormat],
    code_points: Option<&[u32]>,
) -> String {
    let mut out = String::from("@font-face {\n");
    out.push_str(&format!("  font-family: {};\n", string(&style.family)));
    out.push_str(&format!("  font-style: {};\n", font_style(style)));
    out.push_str(&format!("  font-weight: {};\n", font_weight(style)));
    if let Some((lo, hi)) = axis(style, "wdth") {
        out.push_str(&format!("  font-stretch: {};\n", range(lo, hi, "%")));
    }
    out.push_str("  font-display: swap;\n");
    let sources: Vec<String> = [WebFormat::Woff2, WebFormat::Woff]
        .into_iter()
        .filter(|f| formats.contains(f))
        .map(|f| {
            let ext = extension(f);
            format!(
                "url({}) format(\"{ext}\")",
                string(&format!("{file_stem}.{ext}"))
            )
        })
        .collect();
    out.push_str(&format!("  src: {};\n", sources.join(", ")));
    if let Some(cps) = code_points {
        out.push_str(&format!("  unicode-range: {};\n", unicode_range(cps)));
    }
    out.push_str("}\n");
    out
}

/// The `font-style` of `style`: the negated `slnt` range of a variable
/// face, else its slant (`normal`, `italic`, `oblique`).
pub fn font_style(style: &FaceStyle) -> String {
    match axis(style, "slnt") {
        Some((lo, hi)) => format!("oblique {}", range(-hi, -lo, "deg")),
        None => match style.slant {
            Slant::Upright => "normal".to_owned(),
            Slant::Italic => "italic".to_owned(),
            Slant::Oblique => "oblique".to_owned(),
        },
    }
}

/// The `font-weight` of `style`: the `wght` range of a variable face
/// (`100 900`), else its weight class.
pub fn font_weight(style: &FaceStyle) -> String {
    axis(style, "wght").map_or_else(|| style.weight.to_string(), |(lo, hi)| range(lo, hi, ""))
}

/// The finite range of the axis `tag` of `style`, low end first.
fn axis(style: &FaceStyle, tag: &str) -> Option<(f32, f32)> {
    style
        .axes
        .iter()
        .find(|a| a.tag == tag && a.min.is_finite() && a.max.is_finite())
        .map(|a| (a.min.min(a.max), a.min.max(a.max)))
}

/// The stylesheet of `blocks`, one blank line between them.
pub fn stylesheet(blocks: &[String]) -> String {
    blocks.join("\n")
}

/// `code_points` collapsed into sorted, merged ranges.
pub fn ranges(code_points: &[u32]) -> Vec<(u32, u32)> {
    let mut sorted = code_points.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut out: Vec<(u32, u32)> = Vec::new();
    for cp in sorted {
        match out.last_mut() {
            Some(last) if last.1.checked_add(1) == Some(cp) => last.1 = cp,
            _ => out.push((cp, cp)),
        }
    }
    out
}

/// A `unicode-range` value for `code_points` (`U+0020, U+0041-005A`).
pub fn unicode_range(code_points: &[u32]) -> String {
    ranges(code_points)
        .into_iter()
        .map(|(a, b)| {
            if a == b {
                format!("U+{a:04X}")
            } else {
                format!("U+{a:04X}-{b:04X}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `lo` and `hi` with `unit`, or one value when they are equal.
fn range(lo: f32, hi: f32, unit: &str) -> String {
    let (lo, hi) = (number(lo), number(hi));
    if lo == hi {
        format!("{lo}{unit}")
    } else {
        format!("{lo}{unit} {hi}{unit}")
    }
}

/// `s` as a CSS string: quoted, `"` and `\` escaped, a control character as
/// a hex escape followed by a space.
fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_control() => out.push_str(&format!("\\{:X} ", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests;
