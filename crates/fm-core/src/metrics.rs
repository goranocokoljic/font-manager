//! Vertical metrics of a face and the auto size factor built on them (E16.2).
//!
//! A scan reads the metrics once ([`read`]) and the index stores them with
//! the face (`face.metrics`), so the inspector and auto size never open the
//! file again. The x-height and cap height are `OS/2.sxHeight` and
//! `OS/2.sCapHeight`; when the table is older than version 2 or a value is
//! zero, they are measured from the top of the `x` and `H` outlines.
//!
//! Auto size scales each face so that its x-height (or cap height) is a fixed
//! part of the em: [`X_HEIGHT_EM`] or [`CAP_HEIGHT_EM`]. At a toolbar size of
//! `s` pixels every face then has an x-height of `s × 0.5` pixels, whatever
//! page of the list it is on.

use fm_api::common::Metrics;
use fm_api::preview::SizeBasis;
use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};

/// The x-height auto size gives every face, in em.
pub const X_HEIGHT_EM: f32 = 0.5;

/// The cap height auto size gives every face, in em.
pub const CAP_HEIGHT_EM: f32 = 0.7;

/// The range, in em, of an x-height or cap height auto size trusts. A value
/// outside it is damaged data or a face with no such letters (a symbol
/// font's `H` slot), and scaling by it would make the row unreadable.
pub const USABLE_EM: (f32, f32) = (0.2, 1.2);

/// The vertical metrics of `font`: `hhea` ascender and descender (the `OS/2`
/// typographic ones without `hhea`, zero without either), units per em from
/// `head` (zero without it), and the x-height and cap height from `OS/2` or,
/// when it has none or zero, measured from the `x` and `H` outlines. A metric
/// that is neither declared nor measurable is `None`. Never panics on
/// malformed data.
pub fn read(font: &FontRef<'_>) -> Metrics {
    let os2 = font.os2().ok();
    let (ascender, descender) = match font.hhea() {
        Ok(hhea) => (hhea.ascender().to_i16(), hhea.descender().to_i16()),
        Err(_) => os2
            .as_ref()
            .map_or((0, 0), |o| (o.s_typo_ascender(), o.s_typo_descender())),
    };
    let declared = |v: Option<i16>| v.filter(|&v| v > 0);
    let x_height =
        declared(os2.as_ref().and_then(|o| o.sx_height())).or_else(|| outline_top(font, 'x'));
    let cap_height =
        declared(os2.as_ref().and_then(|o| o.s_cap_height())).or_else(|| outline_top(font, 'H'));
    Metrics {
        units_per_em: font.head().map_or(0, |h| h.units_per_em()),
        ascender,
        descender,
        x_height,
        cap_height,
    }
}

/// The factor that scales a face with `metrics` to the auto size of `basis`:
/// [`X_HEIGHT_EM`] (or [`CAP_HEIGHT_EM`]) over the face's own value in em.
/// `1.0` when the face has no usable value (none, no units per em, or outside
/// [`USABLE_EM`]).
pub fn size_factor(metrics: &Metrics, basis: SizeBasis) -> f32 {
    let (value, target) = match basis {
        SizeBasis::XHeight => (metrics.x_height, X_HEIGHT_EM),
        SizeBasis::CapHeight => (metrics.cap_height, CAP_HEIGHT_EM),
    };
    let upem = f32::from(metrics.units_per_em);
    match value {
        Some(v) if upem > 0.0 => {
            let em = f32::from(v) / upem;
            if (USABLE_EM.0..=USABLE_EM.1).contains(&em) {
                target / em
            } else {
                1.0
            }
        }
        _ => 1.0,
    }
}

/// The top of the outline of `c` at the default location, in font units,
/// rounded; `None` when the face does not map `c`, draws nothing for it, or
/// its top is not above the baseline.
fn outline_top(font: &FontRef<'_>, c: char) -> Option<i16> {
    let gid = font.charmap().map(c)?;
    let outline = font.outline_glyphs().get(gid)?;
    let mut pen = Top(None);
    outline
        .draw(
            DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
            &mut pen,
        )
        .ok()?;
    let top = pen.0?.round();
    (top > 0.0 && top <= f32::from(i16::MAX)).then_some(top as i16)
}

/// The highest on-curve point of an outline. The tops of `x` and `H` are
/// flat, so their on-curve points reach the top; control points of round
/// letters are not used, as they lie outside the ink.
struct Top(Option<f32>);

impl Top {
    fn point(&mut self, y: f32) {
        if y.is_finite() {
            self.0 = Some(self.0.map_or(y, |t| t.max(y)));
        }
    }
}

impl OutlinePen for Top {
    fn move_to(&mut self, _x: f32, y: f32) {
        self.point(y);
    }

    fn line_to(&mut self, _x: f32, y: f32) {
        self.point(y);
    }

    fn quad_to(&mut self, _cx0: f32, _cy0: f32, _x: f32, y: f32) {
        self.point(y);
    }

    fn curve_to(&mut self, _cx0: f32, _cy0: f32, _cx1: f32, _cy1: f32, _x: f32, y: f32) {
        self.point(y);
    }

    fn close(&mut self) {}
}

#[cfg(test)]
mod tests;
