use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use super::*;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

fn read_font(rel: &str) -> Vec<u8> {
    std::fs::read(fixture("fonts").join(rel)).expect("fixture font")
}

#[derive(Deserialize)]
struct Labels {
    families: Vec<Labelled>,
}

#[derive(Deserialize)]
struct Labelled {
    family: String,
    file: String,
    traits: BTreeMap<String, String>,
}

fn labels() -> Vec<Labelled> {
    let text = std::fs::read_to_string(fixture("glyph-traits-labels.json")).expect("labels");
    serde_json::from_str::<Labels>(&text)
        .expect("labels parse")
        .families
}

const MULISH: &str = "google-repo/mulish/Mulish[wght].ttf";

fn r_of(rel: &str) -> Descriptor {
    describe(&read_font(rel), 0, 'R' as u32).expect("R described")
}

/// Criterion 3: for the labelled corpus, the nearest neighbour of a
/// straight-leg R (among the other labelled fonts) is a straight-leg R in at
/// least 85 % of cases.
#[test]
fn nearest_neighbour_of_a_straight_leg_r_is_a_straight_leg_r() {
    let fonts: Vec<(String, String, Prepared)> = labels()
        .into_iter()
        .filter_map(|l| {
            let leg = l.traits.get("R.leg")?.clone();
            let d = describe(&read_font(&l.file), 0, 'R' as u32)?;
            Some((l.family, leg, Prepared::new(d)))
        })
        .collect();
    assert!(fonts.len() >= 60, "{} labelled Rs described", fonts.len());
    let straight: Vec<usize> = (0..fonts.len())
        .filter(|&i| fonts[i].1 == "straight")
        .collect();
    let mut agree = 0;
    let mut misses = Vec::new();
    for &i in &straight {
        let (j, dist) = (0..fonts.len())
            .filter(|&j| j != i)
            .map(|j| (j, fonts[i].2.distance(&fonts[j].2)))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
            .expect("a neighbour");
        if fonts[j].1 == "straight" {
            agree += 1;
        } else {
            misses.push(format!(
                "{} → {} ({}, {dist:.3})",
                fonts[i].0, fonts[j].0, fonts[j].1
            ));
        }
    }
    let share = f64::from(agree) / straight.len() as f64;
    eprintln!(
        "R.leg: {agree}/{} straight-leg nearest neighbours ({:.1} %), misses {misses:?}",
        straight.len(),
        share * 100.0
    );
    assert!(
        share >= 0.85,
        "{agree}/{} straight-leg Rs have a straight-leg nearest neighbour ({:.1} %): {misses:#?}",
        straight.len(),
        share * 100.0
    );
}

#[test]
fn a_glyph_is_at_distance_zero_from_itself_and_the_distance_is_symmetric() {
    let a = r_of(MULISH);
    let b = r_of("google-repo/zeyada/Zeyada.ttf");
    assert_eq!(distance(&a, &a), 0.0);
    assert_eq!(distance(&a, &b), distance(&b, &a));
    let d = distance(&a, &b);
    assert!(d > 0.0 && d <= 1.0, "{d}");
}

#[test]
fn a_descriptor_is_len_bytes_with_the_largest_255() {
    let d = r_of(MULISH);
    assert_eq!(d.as_bytes().len(), LEN);
    assert_eq!(d.as_bytes().iter().copied().max(), Some(255));
    assert_eq!(
        Descriptor::from_bytes(d.as_bytes().to_vec()),
        Some(d.clone())
    );
    assert_eq!(Descriptor::from_bytes(vec![0; LEN - 1]), None);
}

#[test]
fn the_same_file_gives_the_same_descriptor() {
    let data = read_font(MULISH);
    assert_eq!(
        describe(&data, 0, 'g' as u32),
        describe(&data, 0, 'g' as u32)
    );
}

#[test]
fn different_letters_of_one_face_are_further_apart_than_one_letter_of_two_sans() {
    let mulish = read_font(MULISH);
    let open = read_font("google-repo/opensanshebrew/OpenSansHebrew-Bold.ttf");
    let r_mulish = describe(&mulish, 0, 'R' as u32).expect("R");
    let o_mulish = describe(&mulish, 0, 'O' as u32).expect("O");
    let r_open = describe(&open, 0, 'R' as u32).expect("R");
    assert!(
        distance(&r_mulish, &r_open) < distance(&r_mulish, &o_mulish),
        "R–R {} vs R–O {}",
        distance(&r_mulish, &r_open),
        distance(&r_mulish, &o_mulish)
    );
}

#[test]
fn many_keeps_the_order_and_marks_a_character_the_face_lacks() {
    let data = read_font(MULISH);
    let found = describe_many(&data, 0, &['R' as u32, 0x10_FFFD, 'O' as u32, 0x11_0000])
        .expect("face reads");
    assert_eq!(found.len(), 4);
    assert_eq!(found[0], describe(&data, 0, 'R' as u32));
    assert_eq!(found[1], None, "unmapped character");
    assert_eq!(found[2], describe(&data, 0, 'O' as u32));
    assert_eq!(found[3], None, "not a code point");
}

#[test]
fn a_glyph_without_an_outline_has_no_descriptor() {
    let data = read_font(MULISH);
    assert_eq!(describe(&data, 0, ' ' as u32), None);
}

#[test]
fn a_woff2_file_is_described() {
    let data = read_font("fontshare/archivo/Archivo-Bold.woff2");
    let found = describe_many(&data, 0, &['R' as u32, 'a' as u32]).expect("face reads");
    assert!(found.iter().all(Option::is_some), "R and a described");
}

#[test]
fn malformed_data_is_no_descriptor_and_no_panic() {
    assert_eq!(describe(b"", 0, 'R' as u32), None);
    assert_eq!(describe(b"not a font at all", 0, 'R' as u32), None);
    let data = read_font(MULISH);
    assert_eq!(describe(&data, 5, 'R' as u32), None, "no face 5");
    for cut in [12, 200, data.len() / 3, data.len() / 2] {
        let _ = describe_many(&data[..cut], 0, &['R' as u32, 'g' as u32]);
    }
    let mut flipped = data.clone();
    for i in (300..flipped.len()).step_by(97) {
        flipped[i] ^= 0xA5;
    }
    let _ = describe_many(&flipped, 0, &TRAIT_GLYPHS.map(u32::from));
}

#[test]
fn a_blank_canvas_is_a_zero_descriptor_at_distance_one_from_a_drawing() {
    let blank = from_canvas(&[0; SIZE * SIZE]);
    assert!(blank.as_bytes().iter().all(|&b| b == 0));
    assert_eq!(distance(&blank, &blank), 0.0);
    assert_eq!(distance(&blank, &r_of(MULISH)), 1.0);
}

#[test]
fn the_pass_describes_every_trait_glyph_and_every_glyph_a_detector_reads() {
    for ch in crate::glyphs::geometry::TRAIT_GLYPHS {
        assert!(TRAIT_GLYPHS.contains(&ch), "{ch}");
    }
    for id in crate::glyphs::traits::trait_ids() {
        let ch = crate::glyph_traits::find(id)
            .expect("catalog")
            .map(|t| t.glyph)
            .expect("trait in catalog");
        assert!(TRAIT_GLYPHS.contains(&ch), "{id} reads {ch}");
    }
}
