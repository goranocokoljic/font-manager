//! Body-text layouts (E12.1): the six presets and the ranges every block
//! value is clamped into; [`store`] keeps layouts as template files (E12.5).
//! The UI renders a layout as live text in the webview; this module only
//! decides what the blocks are.

use fm_api::layout::{
    BlockAlign, BlockOverflow, BlockRange, ColumnRange, Layout, LayoutBounds, LayoutPreset,
    TextBlock,
};

pub mod store;

/// The range of a block value and what a value that is not a number becomes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Smallest value.
    pub min: f32,
    /// Largest value.
    pub max: f32,
    /// The value for NaN or infinity.
    pub default: f32,
}

impl Bounds {
    /// `value` inside the range; NaN and infinities give the default.
    /// (Not `f32::clamp`, which panics when `min > max`.)
    pub fn clamp(self, value: f32) -> f32 {
        if value.is_finite() {
            value.max(self.min).min(self.max)
        } else {
            self.default
        }
    }
}

/// Font size in points.
pub const SIZE_PT: Bounds = Bounds {
    min: 8.0,
    max: 600.0,
    default: 12.0,
};
/// Line height as a multiple of the size.
pub const LINE_HEIGHT: Bounds = Bounds {
    min: 0.1,
    max: 3.0,
    default: 1.3,
};
/// Letter spacing in thousandths of an em.
pub const TRACKING: Bounds = Bounds {
    min: -200.0,
    max: 1000.0,
    default: 0.0,
};
/// Space after a paragraph in points.
pub const PARAGRAPH_SPACING_PT: Bounds = Bounds {
    min: 0.0,
    max: 600.0,
    default: 0.0,
};
/// First-line indent in em.
pub const FIRST_LINE_INDENT_EM: Bounds = Bounds {
    min: 0.0,
    max: 20.0,
    default: 0.0,
};
/// One arrow-key press on a block: 1 pt of size.
pub const SIZE_PT_STEP: f32 = 1.0;
/// One pixel of horizontal drag on a block: half a point of size.
pub const SIZE_PT_PER_PX: f32 = 0.5;
/// One arrow-key press on a block: 0.05 of line height.
pub const LINE_HEIGHT_STEP: f32 = 0.05;
/// One pixel of vertical drag on a block: 0.01 of line height.
pub const LINE_HEIGHT_PER_PX: f32 = 0.01;
/// One arrow-key press on tracking: 10 thousandths of an em.
pub const TRACKING_STEP: f32 = 10.0;
/// One pixel of drag on tracking.
pub const TRACKING_PER_PX: f32 = 2.0;
/// One arrow-key press on paragraph spacing: 1 pt.
pub const PARAGRAPH_SPACING_PT_STEP: f32 = 1.0;
/// One pixel of drag on paragraph spacing.
pub const PARAGRAPH_SPACING_PT_PER_PX: f32 = 0.5;
/// One arrow-key press on the first-line indent: half an em.
pub const FIRST_LINE_INDENT_EM_STEP: f32 = 0.5;
/// One pixel of drag on the first-line indent.
pub const FIRST_LINE_INDENT_EM_PER_PX: f32 = 0.05;
/// Columns a block flows into.
pub const COLUMNS: std::ops::RangeInclusive<u8> = 1..=3;

/// Every preset, in the order the UI offers them.
pub const PRESETS: [LayoutPreset; 6] = [
    LayoutPreset::Book,
    LayoutPreset::Columns,
    LayoutPreset::Magazine,
    LayoutPreset::Quote,
    LayoutPreset::Characters,
    LayoutPreset::Waterfall,
];

/// The sizes of the waterfall preset, largest first.
pub const WATERFALL_SIZES_PT: [f32; 8] = [72.0, 48.0, 36.0, 24.0, 18.0, 14.0, 12.0, 9.0];

const HEADING: &str = "Chapter One";
const BODY: &str = "Typography is the craft of endowing human language with a durable visual form. \
It exists to honor content, and a page set with care invites the reader in and keeps them reading.\n\
The best type does not call attention to itself. Letters sit evenly on the line, words keep a steady \
rhythm, and the eye moves from one line to the next without effort or doubt.\n\
A good body face holds up at small sizes and over long passages. Its spacing is even, its figures \
line up, and its italic and bold follow the voice of the text rather than shouting over it.";
const HEADLINE: &str = "The Quiet Return of the Printed Page";
const STANDFIRST: &str = "Readers are going back to paper, and designers are relearning what makes \
a page worth turning.";
const QUOTE: &str =
    "\u{201c}Type is a beautiful group of letters, not a group of beautiful letters.\u{201d}";
const ATTRIBUTION: &str = "Matthew Carter";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const FIGURES: &str = "0123456789 &@#%?!.,:;\u{2018}\u{2019}\u{201c}\u{201d}()[]{}/*+-=";
const WATERFALL: &str = "The quick brown fox jumps over the lazy dog";

/// A wrapping, start-aligned, one-column block with no extra spacing.
fn block(text: &str, size_pt: f32, line_height: f32) -> TextBlock {
    TextBlock {
        text: text.to_owned(),
        size_pt,
        line_height,
        tracking: 0.0,
        paragraph_spacing_pt: 0.0,
        first_line_indent_em: 0.0,
        columns: 1,
        align: BlockAlign::Start,
        overflow: BlockOverflow::Wrap,
    }
}

/// The display name of a preset.
pub fn preset_name(preset: LayoutPreset) -> &'static str {
    match preset {
        LayoutPreset::Book => "Book",
        LayoutPreset::Columns => "Columns",
        LayoutPreset::Magazine => "Magazine",
        LayoutPreset::Quote => "Quote",
        LayoutPreset::Characters => "Characters",
        LayoutPreset::Waterfall => "Waterfall",
    }
}

/// The blocks of a preset.
pub fn preset(preset: LayoutPreset) -> Layout {
    let blocks = match preset {
        LayoutPreset::Book => vec![
            TextBlock {
                align: BlockAlign::Center,
                paragraph_spacing_pt: 18.0,
                ..block(HEADING, 24.0, 1.2)
            },
            TextBlock {
                first_line_indent_em: 1.5,
                align: BlockAlign::Justify,
                ..block(BODY, 11.0, 1.5)
            },
        ],
        LayoutPreset::Columns => vec![
            TextBlock {
                paragraph_spacing_pt: 12.0,
                ..block(HEADING, 18.0, 1.2)
            },
            TextBlock {
                paragraph_spacing_pt: 6.0,
                columns: 3,
                align: BlockAlign::Justify,
                ..block(BODY, 10.0, 1.4)
            },
        ],
        LayoutPreset::Magazine => vec![
            TextBlock {
                tracking: -20.0,
                paragraph_spacing_pt: 12.0,
                ..block(HEADLINE, 48.0, 1.05)
            },
            TextBlock {
                paragraph_spacing_pt: 18.0,
                ..block(STANDFIRST, 16.0, 1.35)
            },
            TextBlock {
                first_line_indent_em: 1.0,
                columns: 2,
                align: BlockAlign::Justify,
                ..block(BODY, 10.0, 1.45)
            },
        ],
        LayoutPreset::Quote => vec![
            TextBlock {
                align: BlockAlign::Center,
                paragraph_spacing_pt: 18.0,
                ..block(QUOTE, 36.0, 1.15)
            },
            TextBlock {
                tracking: 100.0,
                align: BlockAlign::Center,
                ..block(ATTRIBUTION, 12.0, 1.3)
            },
        ],
        LayoutPreset::Characters => [UPPER, LOWER, FIGURES]
            .into_iter()
            .map(|text| TextBlock {
                tracking: 50.0,
                paragraph_spacing_pt: 12.0,
                ..block(text, 36.0, 1.2)
            })
            .collect(),
        LayoutPreset::Waterfall => WATERFALL_SIZES_PT
            .into_iter()
            .map(|size| TextBlock {
                overflow: BlockOverflow::Clip,
                ..block(WATERFALL, size, 1.2)
            })
            .collect(),
    };
    Layout {
        preset: Some(preset),
        name: preset_name(preset).to_owned(),
        blocks,
    }
}

/// The six presets, in [`PRESETS`] order.
pub fn presets() -> Vec<Layout> {
    PRESETS.into_iter().map(preset).collect()
}

/// A block with every value inside its range.
pub fn clamp_block(block: TextBlock) -> TextBlock {
    TextBlock {
        size_pt: SIZE_PT.clamp(block.size_pt),
        line_height: LINE_HEIGHT.clamp(block.line_height),
        tracking: TRACKING.clamp(block.tracking),
        paragraph_spacing_pt: PARAGRAPH_SPACING_PT.clamp(block.paragraph_spacing_pt),
        first_line_indent_em: FIRST_LINE_INDENT_EM.clamp(block.first_line_indent_em),
        columns: block.columns.max(*COLUMNS.start()).min(*COLUMNS.end()),
        ..block
    }
}

/// The ranges [`clamp_block`] clamps into, with the steps the properties
/// editor (E12.3) and the gestures move each value by.
pub fn bounds() -> LayoutBounds {
    let range = |b: Bounds, step: f32, per_px: f32| BlockRange {
        min: b.min,
        max: b.max,
        default: b.default,
        step,
        per_px,
    };
    LayoutBounds {
        size_pt: range(SIZE_PT, SIZE_PT_STEP, SIZE_PT_PER_PX),
        line_height: range(LINE_HEIGHT, LINE_HEIGHT_STEP, LINE_HEIGHT_PER_PX),
        tracking: range(TRACKING, TRACKING_STEP, TRACKING_PER_PX),
        paragraph_spacing_pt: range(
            PARAGRAPH_SPACING_PT,
            PARAGRAPH_SPACING_PT_STEP,
            PARAGRAPH_SPACING_PT_PER_PX,
        ),
        first_line_indent_em: range(
            FIRST_LINE_INDENT_EM,
            FIRST_LINE_INDENT_EM_STEP,
            FIRST_LINE_INDENT_EM_PER_PX,
        ),
        columns: ColumnRange {
            min: *COLUMNS.start(),
            max: *COLUMNS.end(),
        },
    }
}

/// A layout with every block clamped.
pub fn clamp_layout(layout: Layout) -> Layout {
    Layout {
        blocks: layout.blocks.into_iter().map(clamp_block).collect(),
        ..layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_range(b: &TextBlock) -> bool {
        clamp_block(b.clone()) == *b
    }

    #[test]
    fn there_are_six_presets_in_picker_order_with_their_names() {
        let names: Vec<_> = presets().into_iter().map(|l| l.name).collect();
        assert_eq!(
            names,
            [
                "Book",
                "Columns",
                "Magazine",
                "Quote",
                "Characters",
                "Waterfall"
            ]
        );
        for p in PRESETS {
            assert_eq!(preset(p).preset, Some(p));
        }
    }

    #[test]
    fn every_preset_block_is_inside_the_ranges_and_has_text() {
        for layout in presets() {
            assert!(!layout.blocks.is_empty(), "{}", layout.name);
            for b in &layout.blocks {
                assert!(in_range(b), "{}: {b:?}", layout.name);
                assert!(!b.text.trim().is_empty(), "{}", layout.name);
            }
        }
    }

    #[test]
    fn presets_differ_from_each_other() {
        let all = presets();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.blocks, b.blocks, "{} = {}", a.name, b.name);
            }
        }
    }

    #[test]
    fn waterfall_is_one_text_at_six_to_eight_falling_sizes_that_clip() {
        let w = preset(LayoutPreset::Waterfall);
        assert!((6..=8).contains(&w.blocks.len()), "{}", w.blocks.len());
        let first = &w.blocks[0].text;
        assert!(w.blocks.iter().all(|b| &b.text == first));
        assert!(w.blocks.windows(2).all(|p| p[0].size_pt > p[1].size_pt));
        assert!(w.blocks.iter().all(|b| b.overflow == BlockOverflow::Clip));
    }

    #[test]
    fn columns_flows_body_text_into_two_or_three_columns() {
        let c = preset(LayoutPreset::Columns);
        let body = c.blocks.iter().max_by_key(|b| b.text.len()).expect("body");
        assert!((2..=3).contains(&body.columns));
        assert!(body.text.contains('\n'), "more than one paragraph");
        assert_eq!(body.overflow, BlockOverflow::Wrap);
    }

    #[test]
    fn book_indents_its_paragraphs_and_magazine_sets_body_in_columns() {
        let book = preset(LayoutPreset::Book);
        assert!(book.blocks.iter().any(|b| b.first_line_indent_em > 0.0));
        let mag = preset(LayoutPreset::Magazine);
        assert!(mag.blocks.iter().any(|b| b.columns > 1));
        assert!(mag.blocks[0].size_pt > mag.blocks[1].size_pt);
    }

    #[test]
    fn quote_is_centered_and_characters_shows_the_alphabet() {
        let q = preset(LayoutPreset::Quote);
        assert!(q.blocks.iter().all(|b| b.align == BlockAlign::Center));
        let c = preset(LayoutPreset::Characters);
        let all: String = c.blocks.iter().map(|b| b.text.as_str()).collect();
        for ch in ('A'..='Z').chain('a'..='z').chain('0'..='9') {
            assert!(all.contains(ch), "{ch}");
        }
    }

    #[test]
    fn values_out_of_range_are_clamped_to_the_nearest_edge() {
        let high = clamp_block(TextBlock {
            size_pt: 9000.0,
            line_height: 7.0,
            tracking: 5000.0,
            paragraph_spacing_pt: 1e9,
            first_line_indent_em: 99.0,
            columns: 12,
            ..block("x", 12.0, 1.0)
        });
        assert_eq!(high.size_pt, 600.0);
        assert_eq!(high.line_height, 3.0);
        assert_eq!(high.tracking, 1000.0);
        assert_eq!(high.paragraph_spacing_pt, 600.0);
        assert_eq!(high.first_line_indent_em, 20.0);
        assert_eq!(high.columns, 3);
        let low = clamp_block(TextBlock {
            size_pt: 1.0,
            line_height: 0.0,
            tracking: -900.0,
            paragraph_spacing_pt: -4.0,
            first_line_indent_em: -1.0,
            columns: 0,
            ..block("x", 12.0, 1.0)
        });
        assert_eq!(low.size_pt, 8.0);
        assert_eq!(low.line_height, 0.1);
        assert_eq!(low.tracking, -200.0);
        assert_eq!(low.paragraph_spacing_pt, 0.0);
        assert_eq!(low.first_line_indent_em, 0.0);
        assert_eq!(low.columns, 1);
    }

    #[test]
    fn bounds_are_the_ranges_clamping_uses_with_steps_inside_them() {
        let r = bounds();
        let wild = |v: f32, columns: u8| TextBlock {
            size_pt: v,
            line_height: v,
            tracking: v,
            paragraph_spacing_pt: v,
            first_line_indent_em: v,
            columns,
            ..block("x", 12.0, 1.2)
        };
        let high = clamp_block(wild(1.0e6, u8::MAX));
        assert_eq!(high.size_pt, r.size_pt.max);
        assert_eq!(high.line_height, r.line_height.max);
        assert_eq!(high.tracking, r.tracking.max);
        assert_eq!(high.paragraph_spacing_pt, r.paragraph_spacing_pt.max);
        assert_eq!(high.first_line_indent_em, r.first_line_indent_em.max);
        assert_eq!(high.columns, r.columns.max);
        let low = clamp_block(wild(-1.0e6, 0));
        assert_eq!(low.size_pt, r.size_pt.min);
        assert_eq!(low.line_height, r.line_height.min);
        assert_eq!(low.tracking, r.tracking.min);
        assert_eq!(low.paragraph_spacing_pt, r.paragraph_spacing_pt.min);
        assert_eq!(low.first_line_indent_em, r.first_line_indent_em.min);
        assert_eq!(low.columns, r.columns.min);
        let nan = clamp_block(wild(f32::NAN, 2));
        assert_eq!(nan.size_pt, r.size_pt.default);
        assert_eq!(nan.line_height, r.line_height.default);
        assert_eq!(nan.tracking, r.tracking.default);
        // Every edge is itself in range: clamping it changes nothing.
        let edge = wild(r.size_pt.min, r.columns.max);
        assert_eq!(clamp_block(edge.clone()).size_pt, edge.size_pt);
        assert_eq!((r.columns.min, r.columns.max), (1, 3));
        for b in [
            r.size_pt,
            r.line_height,
            r.tracking,
            r.paragraph_spacing_pt,
            r.first_line_indent_em,
        ] {
            assert!(
                b.min < b.max && b.min <= b.default && b.default <= b.max,
                "{b:?}"
            );
            assert!(b.step > 0.0 && b.step * 20.0 <= b.max - b.min, "{b:?}");
            assert!(b.per_px > 0.0 && b.per_px * 20.0 <= b.max - b.min, "{b:?}");
        }
    }

    #[test]
    fn values_that_are_not_numbers_take_the_default() {
        let b = clamp_block(TextBlock {
            size_pt: f32::NAN,
            line_height: f32::INFINITY,
            tracking: f32::NEG_INFINITY,
            paragraph_spacing_pt: f32::NAN,
            first_line_indent_em: f32::NAN,
            ..block("x", 12.0, 1.0)
        });
        assert_eq!(b.size_pt, SIZE_PT.default);
        assert_eq!(b.line_height, LINE_HEIGHT.default);
        assert_eq!(b.tracking, TRACKING.default);
        assert_eq!(b.paragraph_spacing_pt, 0.0);
        assert_eq!(b.first_line_indent_em, 0.0);
    }

    #[test]
    fn clamping_keeps_text_alignment_overflow_name_and_preset() {
        let l = Layout {
            preset: None,
            name: "Mine".into(),
            blocks: vec![TextBlock {
                align: BlockAlign::End,
                overflow: BlockOverflow::Clip,
                size_pt: 2.0,
                ..block("Hello\nworld", 12.0, 1.0)
            }],
        };
        let c = clamp_layout(l);
        assert_eq!(c.name, "Mine");
        assert_eq!(c.preset, None);
        assert_eq!(c.blocks[0].text, "Hello\nworld");
        assert_eq!(c.blocks[0].align, BlockAlign::End);
        assert_eq!(c.blocks[0].overflow, BlockOverflow::Clip);
        assert_eq!(c.blocks[0].size_pt, 8.0);
    }
}
