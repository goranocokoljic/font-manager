//! Pairing Studio contexts (E45.1): five layouts, each set on a real page. A
//! context layout is an ordinary layout template; it only carries a
//! [`LayoutPage`] and the [`StudioContext`] it started from, so the Studio can
//! show it at its real size and Reset can bring back the shipped one.
//!
//! Sizes stay in points, like every block: a page in CSS pixels shows `s` px
//! of type at `s * 0.75` pt. The web hero, the phone and the video frame are
//! designed in pixels, so their sizes below say both.

use fm_api::layout::{
    BlockAlign, BlockRole, Layout, LayoutPage, LayoutRole, StudioContext, TextBlock,
};

use super::{BODY, HEADLINE, STANDFIRST, block};

/// Every context, in the order the Studio's switch offers them.
pub const CONTEXTS: [StudioContext; 5] = [
    StudioContext::Magazine,
    StudioContext::WebHero,
    StudioContext::MobileUi,
    StudioContext::VideoTitle,
    StudioContext::Poster,
];

/// A4 in CSS pixels: 210 × 297 mm at 96 px per inch, rounded.
pub const A4_PX: (u32, u32) = (794, 1123);

/// The safe-area guides of a video frame (SMPTE RP 2046-1): action safe keeps
/// 93 % of each side, title safe 90 %; each is the inset from every edge.
pub const VIDEO_SAFE_AREAS: [f32; 2] = [0.035, 0.05];

/// Points per CSS pixel: a size designed as `s` px is stored as `s * PT_PER_PX` pt.
/// An iOS point is one CSS pixel on a 390 × 844 page.
pub const PT_PER_PX: f32 = 72.0 / 96.0;

/// The smallest and largest side of a page in CSS pixels.
pub const PAGE_PX: std::ops::RangeInclusive<u32> = 100..=20_000;
/// The largest safe-area inset: a quarter of the page from every edge.
pub const SAFE_AREA_MAX: f32 = 0.25;

const KICKER: &str = "ISSUE 14 \u{b7} TYPOGRAPHY";
const CAPTION: &str = "Fig. 3 \u{b7} The headline and body faces of this page";
const POSTER_HEADLINE: &str = "Type\nMatters";
const POSTER_DETAILS: &str =
    "An evening of letters, ink and paper\nFriday 14 March \u{b7} 19:00 \u{b7} The Print Room";
const HERO_EYEBROW: &str = "NEW IN VERSION 2";
const HERO_HEADLINE: &str = "Typefaces that work as hard as you do";
const HERO_SUBHEAD: &str =
    "Browse, compare and activate every font you own, offline and in one place.";
const HERO_ACTION: &str = "Get started \u{2192}";
const MOBILE_TITLE: &str = "Library";
const MOBILE_HEADLINE: &str = "Recently added";
const MOBILE_BODY: &str = "Twelve families arrived this week. Pin the ones you use to keep \
them close; the rest stay out of the way until you search for them.";
const MOBILE_FOOTNOTE: &str = "Updated just now";
const VIDEO_TITLE: &str = "The Printed Page";
const VIDEO_SUBTITLE: &str = "Episode 3 \u{b7} A story of type";

/// The name of a context's template. The magazine one is "Magazine page", so the
/// layout list never shows it under the Magazine preset's name.
pub fn context_name(context: StudioContext) -> &'static str {
    match context {
        StudioContext::Magazine => "Magazine page",
        StudioContext::WebHero => "Web hero",
        StudioContext::MobileUi => "Mobile UI",
        StudioContext::VideoTitle => "Video title",
        StudioContext::Poster => "Poster",
    }
}

/// The page of a context: A4 for print, the screen's size in CSS pixels for the others.
pub fn context_page(context: StudioContext) -> LayoutPage {
    let page = |name: &str, (width_px, height_px): (u32, u32), margin_px: f32| LayoutPage {
        name: name.to_owned(),
        width_px,
        height_px,
        margin_px,
        safe_areas: Vec::new(),
    };
    match context {
        // 20 mm margins.
        StudioContext::Magazine => page("A4", A4_PX, 76.0),
        // 15 mm margins.
        StudioContext::Poster => page("A4", A4_PX, 57.0),
        StudioContext::WebHero => page("1440 \u{d7} 900", (1440, 900), 120.0),
        // The iOS layout margin.
        StudioContext::MobileUi => page("390 \u{d7} 844", (390, 844), 16.0),
        // The text keeps inside title safe: 5 % of the width.
        StudioContext::VideoTitle => LayoutPage {
            safe_areas: VIDEO_SAFE_AREAS.to_vec(),
            ..page("1920 \u{d7} 1080", (1920, 1080), 96.0)
        },
    }
}

/// A block designed in CSS pixels: `size_px` of type on `leading_px` lines.
fn px_block(text: &str, size_px: f32, leading_px: f32) -> TextBlock {
    block(text, size_px * PT_PER_PX, leading_px / size_px)
}

/// The layout of a context: its blocks on its page, every block inside the ranges.
pub fn context_layout(context: StudioContext) -> Layout {
    let blocks = match context {
        StudioContext::Magazine => vec![
            TextBlock {
                tracking: 100.0,
                paragraph_spacing_pt: 18.0,
                ..block(KICKER, 8.0, 1.3)
            },
            TextBlock {
                tracking: -20.0,
                paragraph_spacing_pt: 14.0,
                ..block(HEADLINE, 40.0, 1.05)
            },
            TextBlock {
                paragraph_spacing_pt: 18.0,
                ..block(STANDFIRST, 15.0, 1.35)
            },
            TextBlock {
                first_line_indent_em: 1.0,
                columns: 2,
                align: BlockAlign::Justify,
                paragraph_spacing_pt: 6.0,
                ..block(BODY, 9.5, 1.45)
            },
            block(CAPTION, 8.0, 1.4),
        ],
        StudioContext::Poster => vec![
            TextBlock {
                tracking: -30.0,
                paragraph_spacing_pt: 36.0,
                ..block(POSTER_HEADLINE, 120.0, 0.9)
            },
            TextBlock {
                paragraph_spacing_pt: 24.0,
                ..block(STANDFIRST, 20.0, 1.25)
            },
            TextBlock {
                tracking: 50.0,
                ..block(POSTER_DETAILS, 11.0, 1.5)
            },
        ],
        StudioContext::WebHero => vec![
            TextBlock {
                tracking: 80.0,
                paragraph_spacing_pt: 12.0,
                ..px_block(HERO_EYEBROW, 14.0, 18.0)
            },
            TextBlock {
                tracking: -20.0,
                paragraph_spacing_pt: 18.0,
                ..px_block(HERO_HEADLINE, 72.0, 76.0)
            },
            TextBlock {
                paragraph_spacing_pt: 24.0,
                ..px_block(HERO_SUBHEAD, 20.0, 29.0)
            },
            px_block(HERO_ACTION, 16.0, 20.0),
        ],
        // The iOS text styles at the default size: Large Title 34/41, Headline 17/22,
        // Body 17/22, Footnote 13/18.
        StudioContext::MobileUi => vec![
            TextBlock {
                paragraph_spacing_pt: 12.0,
                ..px_block(MOBILE_TITLE, 34.0, 41.0)
            },
            TextBlock {
                paragraph_spacing_pt: 4.0,
                ..px_block(MOBILE_HEADLINE, 17.0, 22.0)
            },
            TextBlock {
                paragraph_spacing_pt: 8.0,
                ..px_block(MOBILE_BODY, 17.0, 22.0)
            },
            px_block(MOBILE_FOOTNOTE, 13.0, 18.0),
        ],
        StudioContext::VideoTitle => vec![
            TextBlock {
                align: BlockAlign::Center,
                paragraph_spacing_pt: 18.0,
                ..px_block(VIDEO_TITLE, 120.0, 126.0)
            },
            TextBlock {
                align: BlockAlign::Center,
                tracking: 60.0,
                ..px_block(VIDEO_SUBTITLE, 40.0, 48.0)
            },
        ],
    };
    Layout {
        preset: None,
        name: context_name(context).to_owned(),
        blocks,
        width_locked: true,
        width_px: None,
        page: Some(context_page(context)),
        context: Some(context),
        roles: context_roles(context),
    }
}

/// The roles of a context (E45.2), in the order the rail lists them, with the
/// blocks of [`context_layout`] that have each one. The magazine's kicker and
/// caption are both small print, so they share the Caption role.
pub fn context_roles(context: StudioContext) -> Vec<LayoutRole> {
    use BlockRole::*;
    let roles: &[(BlockRole, &[u32])] = match context {
        StudioContext::Magazine => &[
            (Headline, &[1]),
            (Deck, &[2]),
            (Body, &[3]),
            (Caption, &[0, 4]),
        ],
        StudioContext::WebHero => &[
            (Eyebrow, &[0]),
            (Headline, &[1]),
            (Body, &[2]),
            (Action, &[3]),
        ],
        StudioContext::MobileUi => &[
            (Title, &[0]),
            (Headline, &[1]),
            (Body, &[2]),
            (Footnote, &[3]),
        ],
        StudioContext::VideoTitle => &[(Headline, &[0]), (Subtitle, &[1])],
        StudioContext::Poster => &[(Headline, &[0]), (Deck, &[1]), (Details, &[2])],
    };
    roles
        .iter()
        .map(|(role, blocks)| LayoutRole {
            role: *role,
            blocks: blocks.to_vec(),
        })
        .collect()
}

/// The roles of a layout with `block_count` blocks, made consistent: a block
/// index past the last block, or one an earlier role already has, is dropped,
/// and so is a role then left without blocks or listed a second time. A context
/// layout saved before roles existed (no roles at all) gets its context's
/// roles, so the Studio's rail is never empty for it.
pub fn clamp_roles(
    roles: Vec<LayoutRole>,
    context: Option<StudioContext>,
    block_count: usize,
) -> Vec<LayoutRole> {
    let roles = match (roles.is_empty(), context) {
        (true, Some(c)) => context_roles(c),
        _ => roles,
    };
    let mut taken = std::collections::HashSet::new();
    let mut out: Vec<LayoutRole> = Vec::new();
    for r in roles {
        if out.iter().any(|o| o.role == r.role) {
            continue;
        }
        let blocks: Vec<u32> = r
            .blocks
            .into_iter()
            .filter(|&i| (i as usize) < block_count && taken.insert(i))
            .collect();
        if !blocks.is_empty() {
            out.push(LayoutRole { blocks, ..r });
        }
    }
    out
}

/// The five context layouts, in [`CONTEXTS`] order.
pub fn context_layouts() -> Vec<Layout> {
    CONTEXTS.into_iter().map(context_layout).collect()
}

/// A page with its sides inside [`PAGE_PX`], its margin less than half of the
/// smaller side and every safe-area inset inside `0..=SAFE_AREA_MAX`. A margin
/// that is not a number becomes 0; an inset that is not a number is dropped.
pub fn clamp_page(page: LayoutPage) -> LayoutPage {
    let side = |v: u32| v.clamp(*PAGE_PX.start(), *PAGE_PX.end());
    let (width_px, height_px) = (side(page.width_px), side(page.height_px));
    let half = width_px.min(height_px) as f32 / 2.0 - 1.0;
    LayoutPage {
        width_px,
        height_px,
        margin_px: if page.margin_px.is_finite() {
            page.margin_px.clamp(0.0, half)
        } else {
            0.0
        },
        safe_areas: page
            .safe_areas
            .into_iter()
            .filter(|s| s.is_finite())
            .map(|s| s.clamp(0.0, SAFE_AREA_MAX))
            .collect(),
        ..page
    }
}

#[cfg(test)]
mod tests {
    use super::super::{clamp_block, clamp_layout};
    use super::*;

    #[test]
    fn there_are_five_contexts_in_switch_order_with_their_names() {
        let names: Vec<_> = context_layouts().into_iter().map(|l| l.name).collect();
        assert_eq!(
            names,
            [
                "Magazine page",
                "Web hero",
                "Mobile UI",
                "Video title",
                "Poster"
            ]
        );
        for c in CONTEXTS {
            let l = context_layout(c);
            assert_eq!(l.context, Some(c));
            assert_eq!(l.preset, None, "a context is not one of the six presets");
        }
    }

    #[test]
    fn each_context_is_on_its_page() {
        let size = |c| {
            let p = context_page(c);
            (p.name, p.width_px, p.height_px)
        };
        assert_eq!(size(StudioContext::Magazine), ("A4".into(), 794, 1123));
        assert_eq!(size(StudioContext::Poster), ("A4".into(), 794, 1123));
        assert_eq!(
            size(StudioContext::WebHero),
            ("1440 \u{d7} 900".into(), 1440, 900)
        );
        assert_eq!(
            size(StudioContext::MobileUi),
            ("390 \u{d7} 844".into(), 390, 844)
        );
        assert_eq!(
            size(StudioContext::VideoTitle),
            ("1920 \u{d7} 1080".into(), 1920, 1080)
        );
        for c in CONTEXTS {
            assert_eq!(context_layout(c).page, Some(context_page(c)));
        }
    }

    #[test]
    fn only_the_video_title_has_safe_area_guides_and_its_text_keeps_inside_title_safe() {
        for c in CONTEXTS {
            let p = context_page(c);
            if c == StudioContext::VideoTitle {
                assert_eq!(p.safe_areas, [0.035, 0.05]);
                let title_safe = p.width_px as f32 * 0.05;
                assert!(p.margin_px >= title_safe, "{} < {title_safe}", p.margin_px);
            } else {
                assert!(p.safe_areas.is_empty(), "{c:?}");
            }
        }
    }

    #[test]
    fn the_mobile_context_uses_the_ios_text_styles() {
        let l = context_layout(StudioContext::MobileUi);
        // Shown in CSS pixels on the 390 px page: pt / 0.75.
        let px: Vec<f32> = l
            .blocks
            .iter()
            .map(|b| (b.size_pt / PT_PER_PX * 100.0).round() / 100.0)
            .collect();
        assert_eq!(px, [34.0, 17.0, 17.0, 13.0]);
        let leading: Vec<f32> = l
            .blocks
            .iter()
            .map(|b| (b.size_pt / PT_PER_PX * b.line_height).round())
            .collect();
        assert_eq!(leading, [41.0, 22.0, 22.0, 18.0]);
    }

    #[test]
    fn every_context_block_is_inside_the_ranges_and_the_layout_survives_clamping() {
        for l in context_layouts() {
            assert!(!l.blocks.is_empty(), "{}", l.name);
            for b in &l.blocks {
                assert_eq!(clamp_block(b.clone()), *b, "{}: {b:?}", l.name);
                assert!(!b.text.trim().is_empty(), "{}", l.name);
            }
            assert_eq!(clamp_layout(l.clone()), l);
        }
    }

    #[test]
    fn a_layout_on_a_page_is_set_at_the_page_inner_width_locked_or_not() {
        let mut l = context_layout(StudioContext::WebHero);
        assert_eq!(super::super::layout_width(&l), Some(1440.0 - 240.0));
        l.width_locked = false;
        l.width_px = Some(500.0);
        assert_eq!(super::super::layout_width(&l), Some(1200.0));
        l.page = None;
        assert_eq!(
            super::super::layout_width(&l),
            None,
            "no page: the lock decides"
        );
    }

    #[test]
    fn a_page_is_clamped_into_its_ranges() {
        let wild = LayoutPage {
            name: "Wild".into(),
            width_px: 5,
            height_px: 900_000,
            margin_px: f32::NAN,
            safe_areas: vec![f32::INFINITY, -1.0, 0.9, 0.05],
        };
        let p = clamp_page(wild);
        assert_eq!((p.width_px, p.height_px), (100, 20_000));
        assert_eq!(p.margin_px, 0.0);
        assert_eq!(p.safe_areas, [0.0, SAFE_AREA_MAX, 0.05]);
        let wide_margin = clamp_page(LayoutPage {
            margin_px: 400.0,
            ..context_page(StudioContext::MobileUi)
        });
        assert!(wide_margin.margin_px < 195.0, "{}", wide_margin.margin_px);
        assert_eq!(wide_margin.name, "390 \u{d7} 844");
    }
    #[test]
    fn each_context_defines_its_own_roles_in_rail_order() {
        use BlockRole::*;
        let roles =
            |c| -> Vec<BlockRole> { context_roles(c).into_iter().map(|r| r.role).collect() };
        assert_eq!(
            roles(StudioContext::Magazine),
            [Headline, Deck, Body, Caption]
        );
        assert_eq!(
            roles(StudioContext::WebHero),
            [Eyebrow, Headline, Body, Action]
        );
        assert_eq!(
            roles(StudioContext::MobileUi),
            [Title, Headline, Body, Footnote]
        );
        assert_eq!(roles(StudioContext::VideoTitle), [Headline, Subtitle]);
        assert_eq!(roles(StudioContext::Poster), [Headline, Deck, Details]);
        // The magazine's kicker and caption are both small print.
        assert_eq!(context_roles(StudioContext::Magazine)[3].blocks, [0, 4]);
    }

    #[test]
    fn every_block_of_a_context_has_exactly_one_role() {
        for layout in context_layouts() {
            let mut blocks: Vec<u32> = layout.roles.iter().flat_map(|r| r.blocks.clone()).collect();
            blocks.sort_unstable();
            let all: Vec<u32> = (0..layout.blocks.len() as u32).collect();
            assert_eq!(blocks, all, "{}", layout.name);
            assert_eq!(
                clamp_layout(layout.clone()).roles,
                layout.roles,
                "{}",
                layout.name
            );
        }
    }

    #[test]
    fn a_context_layout_saved_without_roles_gets_its_contexts_roles_back() {
        let mut layout = context_layout(StudioContext::Magazine);
        layout.roles.clear();
        let json = serde_json::to_string(&layout)
            .unwrap()
            .replace(",\"roles\":[]", "");
        assert!(!json.contains("roles"));
        let read: Layout = serde_json::from_str(&json).unwrap();
        assert_eq!(
            clamp_layout(read).roles,
            context_roles(StudioContext::Magazine)
        );
    }

    #[test]
    fn a_layout_without_a_context_has_no_roles_unless_it_names_them() {
        assert!(clamp_roles(Vec::new(), None, 3).is_empty());
        let own = vec![LayoutRole {
            role: BlockRole::Body,
            blocks: vec![2],
        }];
        assert_eq!(clamp_roles(own.clone(), None, 3), own);
    }

    #[test]
    fn roles_are_made_consistent_with_the_blocks() {
        use BlockRole::*;
        let role = |role, blocks: &[u32]| LayoutRole {
            role,
            blocks: blocks.to_vec(),
        };
        let clamped = clamp_roles(
            vec![
                // Block 9 is past the last block.
                role(Headline, &[0, 9]),
                // Block 0 is the headline's already.
                role(Body, &[0, 1]),
                // Body again: dropped, its block stays free.
                role(Body, &[2]),
                // Nothing left: dropped.
                role(Caption, &[0, 7]),
                role(Deck, &[2]),
            ],
            Some(StudioContext::Magazine),
            3,
        );
        assert_eq!(
            clamped,
            [role(Headline, &[0]), role(Body, &[1]), role(Deck, &[2])]
        );
    }
}
