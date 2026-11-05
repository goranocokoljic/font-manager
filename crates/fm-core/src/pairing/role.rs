//! Role-aware pairing (E42.13, ADR 0021): how well a face plays a role next to
//! its partner, and why in plain words.
//!
//! [`fit`] looks at four facts, each worth a share of the score that depends
//! on the role ([`weights`]):
//!
//! - **Stroke contrast**: a headline may have more of it than its partner; body
//!   and caption text read better with less.
//! - **x-height**: close to the partner's; for a caption, a larger one is as
//!   good, because small type needs it.
//! - **Width**: a headline may differ from its partner; body and caption text
//!   read better near normal width.
//! - **Era**: years close together, from the [`copyright_year`] of each face.
//!
//! A fact one of the faces lacks scores nothing and gives no reason. A reason
//! that would fit every result says nothing, so a width reason needs the
//! partner to differ.
//!
//! The rules of [`super::pair`] still decide what is a pairing: [`blend`] only
//! moves the pairings they let through.

use fm_api::search::PairingRole;

use super::{Pairing, X_HEIGHT_REASON, close_x_heights};

/// The role's share of a blended score; the plain score has the rest.
pub const ROLE_SHARE: f32 = 0.5;
/// A stroke contrast gap of this much earns the full contrast share.
pub const CONTRAST_SPAN: f32 = 0.3;
/// A stroke contrast gap needs to be this much to give a reason.
pub const CONTRAST_MARGIN: f32 = 0.05;
/// An x-height gap of this much, as a share of the smaller, earns nothing.
pub const X_HEIGHT_SPAN: f32 = 0.2;
/// The width classes between normal and the ends of the scale.
pub const WIDTH_SPAN: f32 = 4.0;
/// `OS/2.usWidthClass` of a normal width.
pub const NORMAL_WIDTH: u16 = 5;
/// Years apart that earn nothing of the era share.
pub const ERA_SPAN: f32 = 50.0;
/// Years apart that still count as the same era in a reason.
pub const ERA_TOLERANCE: u16 = 5;

/// What a role looks at in one face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoleFacts {
    /// Stroke contrast (`0.0..=1.0`), when the face has a fingerprint.
    pub stroke_contrast: Option<f32>,
    /// x-height as a share of the em, when known.
    pub x_height: Option<f32>,
    /// `OS/2.usWidthClass`.
    pub width: u16,
    /// The earliest year in its copyright, when it states one.
    pub year: Option<u16>,
}

/// What each fact is worth for a role. Sums to one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    /// Stroke contrast.
    pub contrast: f32,
    /// x-height.
    pub x_height: f32,
    /// Width.
    pub width: f32,
    /// Era.
    pub era: f32,
}

/// The weights of `role` (the table in ADR 0021).
pub const fn weights(role: PairingRole) -> Weights {
    match role {
        PairingRole::Headline => Weights {
            contrast: 0.45,
            x_height: 0.10,
            width: 0.25,
            era: 0.20,
        },
        PairingRole::Body => Weights {
            contrast: 0.30,
            x_height: 0.40,
            width: 0.15,
            era: 0.15,
        },
        PairingRole::Caption => Weights {
            contrast: 0.20,
            x_height: 0.45,
            width: 0.30,
            era: 0.05,
        },
    }
}

fn word(role: PairingRole) -> &'static str {
    match role {
        PairingRole::Headline => "headline",
        PairingRole::Body => "body",
        PairingRole::Caption => "caption",
    }
}

/// How well `other` plays `role` next to `base`: a score in `0.0..=1.0` and
/// the role's reasons. No reason is not "no pairing": see [`blend`].
pub fn fit(role: PairingRole, base: &RoleFacts, other: &RoleFacts) -> Pairing {
    let w = weights(role);
    let mut score = 0.0;
    let mut reasons = Vec::new();

    if let (Some(a), Some(b)) = (base.stroke_contrast, other.stroke_contrast) {
        let (gain, more) = match role {
            PairingRole::Headline => (b - a, "higher"),
            PairingRole::Body | PairingRole::Caption => (a - b, "lower"),
        };
        score += w.contrast * (gain / CONTRAST_SPAN).clamp(0.0, 1.0);
        if gain >= CONTRAST_MARGIN {
            reasons.push(format!("{more} contrast for {}", word(role)));
        }
    }

    if let (Some(a), Some(b)) = (base.x_height, other.x_height)
        && a > 0.0
        && b > 0.0
    {
        let gap = a.max(b) / a.min(b) - 1.0;
        let larger_helps = role == PairingRole::Caption && b >= a;
        let closeness = if larger_helps {
            1.0
        } else {
            (1.0 - gap / X_HEIGHT_SPAN).clamp(0.0, 1.0)
        };
        score += w.x_height * closeness;
        if close_x_heights(a, b) {
            reasons.push(format!("x-height within {} %", whole_percent(gap)));
        } else if larger_helps {
            reasons.push(format!("larger x-height for {}", word(role)));
        }
    }

    let from_normal = |width: u16| f32::from(width.abs_diff(NORMAL_WIDTH));
    match role {
        PairingRole::Headline => {
            let apart = f32::from(other.width.abs_diff(base.width));
            score += w.width * (apart / WIDTH_SPAN).min(1.0);
            if other.width < base.width {
                reasons.push(format!("narrower for {}", word(role)));
            } else if other.width > base.width {
                reasons.push(format!("wider for {}", word(role)));
            }
        }
        PairingRole::Body | PairingRole::Caption => {
            score += w.width * (1.0 - from_normal(other.width) / WIDTH_SPAN).max(0.0);
            if other.width == NORMAL_WIDTH && base.width != NORMAL_WIDTH {
                reasons.push(format!("normal width for {}", word(role)));
            }
        }
    }

    if let (Some(a), Some(b)) = (base.year, other.year) {
        let apart = a.abs_diff(b);
        score += w.era * (1.0 - f32::from(apart) / ERA_SPAN).max(0.0);
        if apart <= ERA_TOLERANCE {
            reasons.push("same era".to_owned());
        }
    }

    Pairing {
        score: score.min(1.0),
        reasons,
    }
}

/// `plain` (what the rules and the model say) with the role's `fit`: the score
/// shared by [`ROLE_SHARE`], the role's reasons first. An x-height reason of
/// the role takes the place of the rules' one.
pub fn blend(plain: Pairing, fit: Pairing) -> Pairing {
    let mut reasons = fit.reasons;
    reasons.extend(plain.reasons.into_iter().filter(|r| r != X_HEIGHT_REASON));
    Pairing {
        score: (1.0 - ROLE_SHARE) * plain.score.min(1.0) + ROLE_SHARE * fit.score,
        reasons,
    }
}

/// `gap` (a share) in whole percent, rounded up and at least 1: what "within"
/// can say truthfully. The small amount taken off keeps a value such as
/// `0.040000003`, which is 4 % in float rounding, from reading 5 %.
fn whole_percent(gap: f32) -> u32 {
    ((gap * 100.0 - 0.001).ceil() as u32).max(1)
}

/// The earliest year a copyright text states: a run of exactly four digits
/// from 1400 to 2100. Longer numbers and other runs are not years.
pub fn copyright_year(text: &str) -> Option<u16> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|run| run.len() == 4)
        .filter_map(|run| run.parse::<u16>().ok())
        .filter(|year| (1400..=2100).contains(year))
        .min()
}

#[cfg(test)]
mod tests;
