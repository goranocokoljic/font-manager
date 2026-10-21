//! The `CPAL` palettes of a `COLR` color face (E16.3), for the inspector's
//! palette picker. The webview applies a palette itself (CSS `font-palette`);
//! this module only reports what the font offers.

use fm_api::library::Palette;
use read_fonts::tables::cpal::PaletteType;
use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use skrifa::string::StringId;

use crate::preview::sfnt_data;
use crate::{FmError, FmResult};

/// The palettes of face `collection_index` in the font file `data`.
///
/// A face without a `COLR` table, or without a `CPAL` table, has none: only
/// `COLR` glyphs take their colors from a palette. A palette whose color
/// records lie outside the table is skipped; a malformed `CPAL` gives none.
/// A file that is not a font is an error.
pub fn face_palettes(data: &[u8], collection_index: u32) -> FmResult<Vec<Palette>> {
    let sfnt = sfnt_data(data)?;
    let font = FontRef::from_index(&sfnt, collection_index).map_err(|e| {
        FmError::unsupported("The font's palettes cannot be read: its font data is malformed.")
            .with_detail(format!("the font header: {e}"))
    })?;
    Ok(palettes(&font))
}

fn palettes(font: &FontRef<'_>) -> Vec<Palette> {
    if font.colr().is_err() {
        return Vec::new();
    }
    let Ok(cpal) = font.cpal() else {
        return Vec::new();
    };
    let Some(Ok(records)) = cpal.color_records_array() else {
        return Vec::new();
    };
    let entries = usize::from(cpal.num_palette_entries());
    let types = cpal.palette_types_array().and_then(Result::ok);
    let labels = cpal.palette_labels_array().and_then(Result::ok);
    cpal.color_record_indices()
        .iter()
        .take(usize::from(cpal.num_palettes()))
        .enumerate()
        .filter_map(|(i, first)| {
            let first = usize::from(first.get());
            let colors = records.get(first..first.checked_add(entries)?)?;
            let flags = types
                .and_then(|t| t.get(i))
                .map_or(PaletteType::empty(), |t| t.get());
            let name = labels
                .and_then(|l| l.get(i))
                .map(|id| id.get().to_u16())
                // 0xFFFF is "no label".
                .filter(|&id| id != 0xFFFF)
                .and_then(|id| {
                    font.localized_strings(StringId::new(id))
                        .english_or_first()
                        .map(|s| s.to_string().trim().to_owned())
                })
                .filter(|s| !s.is_empty());
            Some(Palette {
                index: u16::try_from(i).ok()?,
                name,
                colors: colors
                    .iter()
                    .map(|c| hex(c.red(), c.green(), c.blue(), c.alpha()))
                    .collect(),
                light_background: flags.contains(PaletteType::USABLE_WITH_LIGHT_BACKGROUND),
                dark_background: flags.contains(PaletteType::USABLE_WITH_DARK_BACKGROUND),
            })
        })
        .collect()
}

/// A color as CSS hex: `#rrggbb`, or `#rrggbbaa` when it is not opaque.
fn hex(r: u8, g: u8, b: u8, a: u8) -> String {
    if a == 0xFF {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(rel: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts")
            .join(rel);
        std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    const CAIRO: &str = "google-repo/cairoplay/CairoPlay[slnt,wght].ttf";

    #[test]
    fn cairo_play_has_its_one_palette_as_css_hex() {
        let palettes = face_palettes(&fixture(CAIRO), 0).expect("palettes");
        assert_eq!(
            palettes,
            vec![Palette {
                index: 0,
                // A version 0 CPAL has no names and no background flags.
                name: None,
                colors: ["#000000", "#e53a35", "#8726a2", "#ffab01", "#72bf80"]
                    .map(String::from)
                    .to_vec(),
                light_background: false,
                dark_background: false,
            }]
        );
    }

    /// CairoPlay with its CPAL replaced by a version 1 table of two palettes:
    /// the first labeled with name ID 1 and for light backgrounds, the second
    /// unlabeled (0xFFFF), for dark backgrounds, its first color half
    /// transparent. The new table goes at the end of the file.
    fn cairo_with_two_palettes() -> Vec<u8> {
        let mut cpal = Vec::new();
        let u16s = |v: &mut Vec<u8>, xs: &[u16]| xs.iter().for_each(|x| v.extend(x.to_be_bytes()));
        let u32s = |v: &mut Vec<u8>, xs: &[u32]| xs.iter().for_each(|x| v.extend(x.to_be_bytes()));
        u16s(&mut cpal, &[1, 5, 2, 10]); // version, entries, palettes, records
        u32s(&mut cpal, &[28]); // color records
        u16s(&mut cpal, &[0, 5]); // first record of each palette
        u32s(&mut cpal, &[68, 76, 0]); // types, labels, entry labels
        for p in 0..2u8 {
            for e in 0..5u8 {
                let alpha = if p == 1 && e == 0 { 0x80 } else { 0xFF };
                cpal.extend([e * 16, p * 16, 0xA0 + e, alpha]); // B, G, R, A
            }
        }
        u32s(&mut cpal, &[1, 2]);
        u16s(&mut cpal, &[1, 0xFFFF]);
        assert_eq!(cpal.len(), 80);

        let mut data = fixture(CAIRO);
        while !data.len().is_multiple_of(4) {
            data.push(0);
        }
        let offset = u32::try_from(data.len()).expect("small file");
        let tables = usize::from(u16::from_be_bytes([data[4], data[5]]));
        let entry = (0..tables)
            .map(|i| 12 + 16 * i)
            .find(|&at| &data[at..at + 4] == b"CPAL")
            .expect("CairoPlay has a CPAL");
        data[entry + 8..entry + 12].copy_from_slice(&offset.to_be_bytes());
        data[entry + 12..entry + 16].copy_from_slice(&80u32.to_be_bytes());
        data.extend(cpal);
        data
    }

    #[test]
    fn names_and_background_flags_come_from_a_version_1_cpal() {
        let palettes = face_palettes(&cairo_with_two_palettes(), 0).expect("palettes");
        assert_eq!(palettes.len(), 2, "{palettes:?}");
        let (first, second) = (&palettes[0], &palettes[1]);
        assert_eq!((first.index, second.index), (0, 1));
        assert_eq!(first.name.as_deref(), Some("Cairo Play"));
        assert_eq!(second.name, None, "0xFFFF is no label");
        assert_eq!(
            (first.light_background, first.dark_background),
            (true, false)
        );
        assert_eq!(
            (second.light_background, second.dark_background),
            (false, true)
        );
        assert_eq!(first.colors[0], "#a00000");
        assert_eq!(first.colors[4], "#a40040");
        assert_eq!(second.colors[0], "#a0100080");
        assert_eq!(second.colors.len(), 5);
    }

    #[test]
    fn a_palette_whose_records_lie_outside_the_table_is_skipped() {
        let mut data = cairo_with_two_palettes();
        // The second palette's first record index (header byte 14) points past
        // the 10 records.
        let at = data.len() - 80 + 14;
        data[at..at + 2].copy_from_slice(&8u16.to_be_bytes());
        let palettes = face_palettes(&data, 0).expect("palettes");
        assert_eq!(palettes.iter().map(|p| p.index).collect::<Vec<_>>(), [0]);
    }

    /// `data` with the table tag `from` renamed `to` in its table directory.
    fn retagged(mut data: Vec<u8>, from: &[u8; 4], to: &[u8; 4]) -> Vec<u8> {
        let tables = usize::from(u16::from_be_bytes([data[4], data[5]]));
        for i in 0..tables {
            let at = 12 + 16 * i;
            if &data[at..at + 4] == from {
                data[at..at + 4].copy_from_slice(to);
            }
        }
        data
    }

    #[test]
    fn a_face_without_colr_or_cpal_has_no_palettes() {
        let plain = fixture("google-static/anekmalayalam/AnekMalayalam_Condensed-ExtraLight.ttf");
        assert_eq!(face_palettes(&plain, 0).expect("palettes"), Vec::new());
        let cairo = fixture("google-repo/cairoplay/CairoPlay[slnt,wght].ttf");
        // CPAL without COLR: nothing takes its colors from a palette.
        let no_colr = retagged(cairo.clone(), b"COLR", b"COLQ");
        assert_eq!(face_palettes(&no_colr, 0).expect("palettes"), Vec::new());
        // COLR without CPAL.
        let no_cpal = retagged(cairo, b"CPAL", b"CPAQ");
        assert_eq!(face_palettes(&no_cpal, 0).expect("palettes"), Vec::new());
    }

    #[test]
    fn a_file_that_is_not_a_font_is_an_error_not_a_panic() {
        assert!(face_palettes(b"not a font at all", 0).is_err());
        assert!(face_palettes(&[], 0).is_err());
    }

    #[test]
    fn colors_are_hex_with_alpha_only_when_not_opaque() {
        assert_eq!(hex(0x12, 0xab, 0x00, 0xff), "#12ab00");
        assert_eq!(hex(0x12, 0xab, 0x00, 0x80), "#12ab0080");
    }
}
