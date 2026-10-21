use std::path::PathBuf;

use fm_api::FaceId;
use read_fonts::types::Tag;

use super::*;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel)
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(fixture(rel)).expect("fixture")
}

/// TrueType, glyph names in `post` (`uni0416`).
const ROBOTO_SLAB: &str = "google-static/robotoslab/RobotoSlab-ExtraBold.ttf";
/// CFF, glyph names in the charset (`Zhecyr`).
const NUNITO_OTF: &str = "fontshare/nunito/Nunito-ExtraLightItalic.otf";
/// `post` version 3: no glyph names.
const OXYGEN: &str = "google-repo/oxygen/Oxygen-Light.ttf";
const WOFF2: &str = "fontshare/jet-brains-mono/JetBrainsMono-Variable.woff2";
const TTC: &str = "derived/agdasima/agdasima.ttc";

fn req(query: Option<&str>, block: Option<&str>) -> GlyphsRequest {
    GlyphsRequest {
        face: FaceId(1),
        block: block.map(Into::into),
        query: query.map(Into::into),
        page: Page {
            offset: 0,
            limit: MAX_PAGE,
        },
    }
}

fn page(data: &[u8], req: &GlyphsRequest) -> GlyphPage {
    glyph_page(data, 0, req).expect("a page")
}

fn found(data: &[u8], query: &str) -> Vec<(u32, Option<String>)> {
    page(data, &req(Some(query), None))
        .items
        .into_iter()
        .map(|g| (g.code_point, g.name))
        .collect()
}

/// Every page of the whole list, one after another.
fn every_item(data: &[u8], limit: u32) -> Vec<GlyphInfo> {
    let mut out = Vec::new();
    loop {
        let mut r = req(None, None);
        r.page = Page {
            offset: out.len() as u32,
            limit,
        };
        let p = page(data, &r);
        if p.items.is_empty() {
            return out;
        }
        out.extend(p.items);
    }
}

#[test]
fn characters_come_with_code_point_glyph_id_name_and_block() {
    let data = read(ROBOTO_SLAB);
    let p = page(&data, &req(None, None));
    assert_eq!(p.health, Health::Ok);
    assert_eq!(p.unicode_version, "17.0.0");
    let zhe = every_item(&data, MAX_PAGE)
        .into_iter()
        .find(|g| g.code_point == 0x416)
        .expect("Ж is mapped");
    assert_eq!(zhe.name.as_deref(), Some("uni0416"));
    assert_eq!(zhe.block, "Cyrillic");
    let font = FontRef::new(&data).expect("font");
    assert_eq!(
        font.charmap().map('Ж').map(GlyphId::to_u32),
        Some(zhe.glyph_id),
        "the glyph id is the one cmap maps"
    );
    let a = &p.items.iter().find(|g| g.code_point == 0x41).expect("A");
    assert_eq!(
        (a.name.as_deref(), a.block.as_str()),
        (Some("A"), "Basic Latin")
    );
}

#[test]
fn the_list_is_sorted_and_grouped_by_block_with_counts() {
    let data = read(ROBOTO_SLAB);
    let all = every_item(&data, 137);
    let p = page(&data, &req(None, None));
    assert_eq!(all.len() as u32, p.total);
    assert!(all.windows(2).all(|w| w[0].code_point < w[1].code_point));
    // The blocks are the list: each block's count is its run of characters.
    let mut at = 0usize;
    for b in &p.blocks {
        let run = &all[at..at + b.count as usize];
        assert!(run.iter().all(|g| g.block == b.name), "{}", b.name);
        assert!(
            run.iter()
                .all(|g| (b.first..=b.last).contains(&g.code_point))
        );
        at += b.count as usize;
    }
    assert_eq!(at, all.len());
    assert!(p.blocks.iter().any(|b| b.name == "Cyrillic"));
    let latin = &p.blocks[0];
    assert_eq!(
        (latin.name.as_str(), latin.first, latin.last),
        ("Basic Latin", 0, 0x7F)
    );
}

#[test]
fn a_page_is_capped_and_offsets_walk_the_list() {
    let data = read(WOFF2);
    let mut r = req(None, None);
    r.page = Page {
        offset: 0,
        limit: 10_000,
    };
    let p = page(&data, &r);
    assert!(p.total > MAX_PAGE, "fixture has more than a page");
    assert_eq!(p.items.len() as u32, MAX_PAGE);
    r.page = Page {
        offset: p.total - 3,
        limit: 10,
    };
    assert_eq!(page(&data, &r).items.len(), 3);
    r.page.offset = p.total + 5;
    assert!(page(&data, &r).items.is_empty());
}

#[test]
fn one_block_lists_only_its_characters() {
    let data = read(ROBOTO_SLAB);
    let p = page(&data, &req(None, Some("Cyrillic")));
    assert!(p.total > 0);
    assert!(p.items.iter().all(|g| g.block == "Cyrillic"));
    assert_eq!(p.blocks.len(), 1);
    assert_eq!(p.blocks[0].count, p.total);
    let none = page(&data, &req(None, Some("Klingon")));
    assert_eq!((none.total, none.blocks.len()), (0, 0));
}

#[test]
fn search_finds_a_glyph_by_the_character_itself() {
    let data = read(ROBOTO_SLAB);
    assert_eq!(found(&data, "Ж"), [(0x416, Some("uni0416".into()))]);
    assert_eq!(found(&data, " Ж "), [(0x416, Some("uni0416".into()))]);
    // One letter is that letter, not every name holding it.
    assert_eq!(found(&data, "a"), [(0x61, Some("a".into()))]);
}

#[test]
fn search_finds_a_glyph_by_code_point() {
    let data = read(ROBOTO_SLAB);
    for q in ["U+0416", "u+416", "0x0416", "0416", "U+416"] {
        assert_eq!(
            found(&data, q).first().map(|g| g.0),
            Some(0x416),
            "query {q:?}"
        );
    }
    assert!(
        found(&data, "U+110000").is_empty(),
        "past the last code point"
    );
}

#[test]
fn search_finds_a_glyph_by_glyph_name() {
    let data = read(ROBOTO_SLAB);
    assert_eq!(found(&data, "uni0416"), [(0x416, Some("uni0416".into()))]);
    assert_eq!(found(&data, "UNI0416"), [(0x416, Some("uni0416".into()))]);
    // CFF charset names.
    let cff = read(NUNITO_OTF);
    // A name, or a part of one, finds every glyph holding it, in any case.
    let zhe = found(&cff, "Zhecyr");
    assert_eq!(
        zhe,
        [
            (0x40F, Some("Dzhecyr".into())),
            (0x416, Some("Zhecyr".into())),
            (0x436, Some("zhecyr".into())),
            (0x45F, Some("dzhecyr".into())),
        ]
    );
    assert_eq!(found(&cff, "dzhe"), [zhe[0].clone(), zhe[3].clone()]);
}

#[test]
fn search_finds_a_dashed_name_like_zhe_cy() {
    // No fixture names its glyphs the Glyphs-app way, so rename uni0416 to
    // Zhe-cy in the post table of a copy.
    let data = renamed(&read(ROBOTO_SLAB), b"uni0416", b"Zhe-cy");
    assert_eq!(found(&data, "Zhe-cy"), [(0x416, Some("Zhe-cy".into()))]);
    assert_eq!(found(&data, "zhe-cy"), [(0x416, Some("Zhe-cy".into()))]);
    // The AGL name still spells the code point, though no glyph has it.
    assert_eq!(found(&data, "uni0416"), [(0x416, Some("Zhe-cy".into()))]);
}

/// The font with the post-table name `from` changed to the shorter `to`;
/// the table keeps its length (the bytes after the name move up and an empty
/// name pads the end).
fn renamed(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    assert!(to.len() < from.len());
    let font = FontRef::new(data).expect("font");
    let record = font
        .table_directory()
        .table_records()
        .iter()
        .find(|r| r.tag() == Tag::new(b"post"))
        .expect("post");
    let (start, len) = (record.offset() as usize, record.length() as usize);
    let table = &data[start..start + len];
    let mut needle = vec![from.len() as u8];
    needle.extend_from_slice(from);
    let at = table
        .windows(needle.len())
        .position(|w| w == needle.as_slice())
        .expect("name in post");
    let mut patched = table[..at].to_vec();
    patched.push(to.len() as u8);
    patched.extend_from_slice(to);
    patched.extend_from_slice(&table[at + needle.len()..]);
    patched.resize(len, 0);
    let mut out = data.to_vec();
    out[start..start + len].copy_from_slice(&patched);
    out
}

#[test]
fn a_font_without_glyph_names_reports_none() {
    let data = read(OXYGEN);
    let p = page(&data, &req(None, None));
    assert_eq!(p.health, Health::Ok);
    assert!(p.total > 0);
    assert!(p.items.iter().all(|g| g.name.is_none()), "no gidN names");
    // Searching by code point still works.
    assert_eq!(found(&data, "U+0041"), [(0x41, None)]);
}

#[test]
fn web_fonts_and_collections_are_read() {
    let woff2 = read(WOFF2);
    assert_eq!(found(&woff2, "U+0416"), [(0x416, Some("uni0416".into()))]);
    let ttc = read(TTC);
    let second = glyph_page(&ttc, 1, &req(None, None)).expect("face 1");
    assert_eq!(second.health, Health::Ok);
    assert!(second.total > 0);
    let missing = glyph_page(&ttc, 9, &req(None, None)).expect("a page");
    assert!(matches!(missing.health, Health::Broken(_)));
}

fn broken_reason(data: &[u8]) -> String {
    match glyph_page(data, 0, &req(None, None)) {
        Ok(GlyphPage {
            health: Health::Broken(reason),
            items,
            ..
        }) if items.is_empty() => reason,
        other => panic!("expected a broken page, got {other:?}"),
    }
}

#[test]
fn a_truncated_font_is_broken_not_a_panic() {
    let reason = broken_reason(&read("derived/agdasima/agdasima-truncated.ttf"));
    assert!(reason.contains("cut off"), "{reason}");
}

#[test]
fn a_malformed_cmap_is_broken_not_a_panic() {
    let mut data = read(ROBOTO_SLAB);
    let font = FontRef::new(&data).expect("font");
    let cmap = font
        .table_directory()
        .table_records()
        .iter()
        .find(|r| r.tag() == Tag::new(b"cmap"))
        .expect("cmap")
        .offset() as usize;
    // The first encoding record's subtable offset points past the table.
    data[cmap + 8..cmap + 12].copy_from_slice(&0xFFFF_FFF0u32.to_be_bytes());
    let reason = broken_reason(&data);
    assert!(reason.contains("cmap"), "{reason}");
}

#[test]
fn data_that_is_not_a_font_is_broken() {
    for data in [&b""[..], b"wOF2garbage-garbage", b"not a font at all"] {
        broken_reason(data);
    }
}

#[test]
fn every_fixture_answers_without_a_panic() {
    let root = fixture("");
    let manifest = std::fs::read_to_string(root.join("manifest.tsv")).expect("manifest");
    for line in manifest.lines().skip(1) {
        let rel = line.split('\t').nth(4).expect("path column");
        let p = glyph_page(&read(rel), 0, &req(Some("U+0041"), None)).expect(rel);
        assert!(p.items.len() <= 1, "{rel}");
    }
}

#[test]
fn queries_parse_as_the_criterion_lists() {
    let cps = |q: &str| Query::parse(q).code_points;
    assert!(Query::parse("  ").all);
    assert_eq!(cps("Ж"), [0x416]);
    assert_eq!(cps("U+0416"), [0x416]);
    assert_eq!(cps("0416"), [0x416]);
    assert_eq!(cps("uni0416"), [0x416]);
    assert_eq!(cps("u1F600"), [0x1F600]);
    assert_eq!(cps("0x1F600"), [0x1F600]);
    // Two or three hex letters are a name, not a code point.
    assert!(cps("fe").is_empty());
    assert!(cps("Zhe-cy").is_empty());
    assert_eq!(Query::parse("Zhe-cy").name.as_deref(), Some("zhe-cy"));
    assert!(cps("U+").is_empty());
    assert!(cps("U+FFFFFFF").is_empty());
}

const EPILOGUE_VARIABLE: &str = "google-repo/epilogue/Epilogue[wght].ttf";

fn detail(data: &[u8], code_point: u32) -> GlyphDetail {
    glyph_detail(data, 0, code_point).expect("a detail")
}

#[test]
fn the_detail_has_code_point_glyph_id_name_and_advance() {
    let data = read(ROBOTO_SLAB);
    let d = detail(&data, 0x416);
    let font = FontRef::new(&data).expect("font");
    let gid = font.charmap().map(0x416u32).expect("Ж is mapped");
    assert_eq!(d.health, Health::Ok);
    assert_eq!(d.code_point, 0x416);
    assert_eq!(d.glyph_id, gid.to_u32());
    assert_eq!(d.name.as_deref(), Some("uni0416"));
    let hmtx = font.hmtx().expect("hmtx");
    assert_eq!(
        d.advance_width,
        hmtx.advance(gid).map(u32::from),
        "the hmtx advance in font units"
    );
}

#[test]
fn a_variable_face_has_the_advance_of_its_default_location() {
    let data = read(EPILOGUE_VARIABLE);
    let font = FontRef::new(&data).expect("font");
    assert!(font.fvar().is_ok(), "a variable fixture");
    for c in ['a', 'W', '\u{FB01}'] {
        let gid = font.charmap().map(c).expect("mapped");
        let hmtx = font.hmtx().expect("hmtx").advance(gid).map(u32::from);
        assert_eq!(detail(&data, c as u32).advance_width, hmtx, "{c}");
    }
}

#[test]
fn the_detail_lists_the_features_of_the_glyph() {
    let data = read(EPILOGUE_VARIABLE);
    let fi = detail(&data, 0xFB01);
    assert_eq!(fi.name.as_deref(), Some("fi"));
    let produced: Vec<_> = fi.produced_by.iter().map(|f| f.tag.as_str()).collect();
    assert_eq!(produced, ["liga"]);
    let a = detail(&data, 'a' as u32);
    let replaced: Vec<_> = a.replaced_by.iter().map(|f| f.tag.as_str()).collect();
    assert!(
        replaced.contains(&"smcp") && replaced.contains(&"ss01"),
        "{replaced:?}"
    );
}

#[test]
fn cff_web_fonts_and_collections_have_a_detail() {
    let otf = detail(&read(NUNITO_OTF), 0x416);
    assert_eq!(otf.name.as_deref(), Some("Zhecyr"));
    assert!(otf.advance_width.is_some_and(|w| w > 0));
    let woff2 = detail(&read(WOFF2), 'A' as u32);
    assert_eq!(woff2.health, Health::Ok);
    assert!(woff2.advance_width.is_some_and(|w| w > 0));
    let ttc = read(TTC);
    let second = glyph_detail(&ttc, 1, 'A' as u32).expect("face 1");
    assert_eq!(second.health, Health::Ok);
    assert!(second.glyph_id > 0);
}

#[test]
fn a_font_without_glyph_names_has_a_nameless_detail() {
    let d = detail(&read(OXYGEN), 'A' as u32);
    assert_eq!(d.name, None);
    assert!(d.glyph_id > 0);
}

#[test]
fn a_character_the_face_does_not_map_is_not_found() {
    let data = read(ROBOTO_SLAB);
    let err = glyph_detail(&data, 0, 0x10_FFFD).expect_err("unmapped");
    assert_eq!(err.code(), fm_api::ErrorCode::NotFound);
    assert!(err.message().contains("U+10FFFD"), "{}", err.message());
}

#[test]
fn a_broken_font_is_a_broken_detail_not_a_panic() {
    for data in [
        read("derived/agdasima/agdasima-truncated.ttf"),
        b"not a font at all".to_vec(),
        b"wOF2garbage-garbage".to_vec(),
    ] {
        let d = detail(&data, 'A' as u32);
        assert!(matches!(d.health, Health::Broken(_)), "{:?}", d.health);
        assert_eq!(d.glyph_id, 0);
        assert!(d.produced_by.is_empty() && d.replaced_by.is_empty());
    }
}

#[test]
fn every_fixture_has_a_detail_without_a_panic() {
    let root = fixture("");
    let manifest = std::fs::read_to_string(root.join("manifest.tsv")).expect("manifest");
    for line in manifest.lines().skip(1) {
        let rel = line.split('\t').nth(4).expect("path column");
        let data = read(rel);
        for cp in [0x41, 0xFB01, 0x416] {
            match glyph_detail(&data, 0, cp) {
                Ok(d) => assert_eq!(d.code_point, cp, "{rel}"),
                Err(e) => assert_eq!(e.code(), fm_api::ErrorCode::NotFound, "{rel}"),
            }
        }
    }
}
