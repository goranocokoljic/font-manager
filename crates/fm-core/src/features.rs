//! OpenType layout features by name (E10.3): the friendly name of every tag
//! in the OpenType feature registry, which features the shaper applies by
//! itself, and which are on in default text rendering.
//!
//! [`REGISTRY`] is the registered tags, sorted, except the numbered ones:
//! `ss01`–`ss20` are "Stylistic set N" and `cv01`–`cv99` "Character variant N",
//! unless the font names them itself (E10.4, [`named_by_font`]). A tag that is
//! not registered has no name; the UI shows the tag.
//!
//! "Automatic" and "default on" follow what browsers do (HarfBuzz): the
//! automatic features are required for correct text in some script (they
//! compose marks, pick contextual forms, place marks), so the Features tab
//! lists them but does not offer a switch. The default-on features are on
//! unless `font-feature-settings` turns them off.

use fm_api::common::FeatureInfo;

/// Registered feature tags and their friendly names, sorted by tag.
pub const REGISTRY: &[(&str, &str)] = &[
    ("aalt", "Access all alternates"),
    ("abvf", "Above-base forms"),
    ("abvm", "Above-base mark positioning"),
    ("abvs", "Above-base substitutions"),
    ("afrc", "Alternative fractions"),
    ("akhn", "Akhand"),
    ("apkn", "Kerning for alternate proportional widths"),
    ("blwf", "Below-base forms"),
    ("blwm", "Below-base mark positioning"),
    ("blws", "Below-base substitutions"),
    ("c2pc", "Petite capitals from capitals"),
    ("c2sc", "Small capitals from capitals"),
    ("calt", "Contextual alternates"),
    ("case", "Case-sensitive forms"),
    ("ccmp", "Glyph composition and decomposition"),
    ("cfar", "Conjunct form after Ro"),
    ("chws", "Contextual half-width spacing"),
    ("cjct", "Conjunct forms"),
    ("clig", "Contextual ligatures"),
    ("cpct", "Centered CJK punctuation"),
    ("cpsp", "Capital spacing"),
    ("cswh", "Contextual swash"),
    ("curs", "Cursive positioning"),
    ("dist", "Distances"),
    ("dlig", "Discretionary ligatures"),
    ("dnom", "Denominators"),
    ("dtls", "Dotless forms"),
    ("expt", "Expert forms"),
    ("falt", "Final glyph on line alternates"),
    ("fin2", "Terminal forms #2"),
    ("fin3", "Terminal forms #3"),
    ("fina", "Terminal forms"),
    ("flac", "Flattened accent forms"),
    ("frac", "Fractions"),
    ("fwid", "Full widths"),
    ("half", "Half forms"),
    ("haln", "Halant forms"),
    ("halt", "Alternate half widths"),
    ("hist", "Historical forms"),
    ("hkna", "Horizontal kana alternates"),
    ("hlig", "Historical ligatures"),
    ("hngl", "Hangul"),
    ("hojo", "Hojo kanji forms"),
    ("hwid", "Half widths"),
    ("init", "Initial forms"),
    ("isol", "Isolated forms"),
    ("ital", "Italics"),
    ("jalt", "Justification alternates"),
    ("jp04", "JIS2004 forms"),
    ("jp78", "JIS78 forms"),
    ("jp83", "JIS83 forms"),
    ("jp90", "JIS90 forms"),
    ("kern", "Kerning"),
    ("lfbd", "Left bounds"),
    ("liga", "Standard ligatures"),
    ("ljmo", "Leading jamo forms"),
    ("lnum", "Lining figures"),
    ("locl", "Localized forms"),
    ("ltra", "Left-to-right alternates"),
    ("ltrm", "Left-to-right mirrored forms"),
    ("mark", "Mark positioning"),
    ("med2", "Medial forms #2"),
    ("medi", "Medial forms"),
    ("mgrk", "Mathematical Greek"),
    ("mkmk", "Mark to mark positioning"),
    ("mset", "Mark positioning via substitution"),
    ("nalt", "Alternate annotation forms"),
    ("nlck", "NLC kanji forms"),
    ("nukt", "Nukta forms"),
    ("numr", "Numerators"),
    ("onum", "Oldstyle figures"),
    ("opbd", "Optical bounds"),
    ("ordn", "Ordinals"),
    ("ornm", "Ornaments"),
    ("palt", "Proportional alternate widths"),
    ("pcap", "Petite capitals"),
    ("pkna", "Proportional kana"),
    ("pnum", "Proportional figures"),
    ("pref", "Pre-base forms"),
    ("pres", "Pre-base substitutions"),
    ("pstf", "Post-base forms"),
    ("psts", "Post-base substitutions"),
    ("pwid", "Proportional widths"),
    ("qwid", "Quarter widths"),
    ("rand", "Randomize"),
    ("rclt", "Required contextual alternates"),
    ("rkrf", "Rakar forms"),
    ("rlig", "Required ligatures"),
    ("rphf", "Reph form"),
    ("rtbd", "Right bounds"),
    ("rtla", "Right-to-left alternates"),
    ("rtlm", "Right-to-left mirrored forms"),
    ("ruby", "Ruby notation forms"),
    ("rvrn", "Required variation alternates"),
    ("salt", "Stylistic alternates"),
    ("sinf", "Scientific inferiors"),
    ("size", "Optical size"),
    ("smcp", "Small capitals"),
    ("smpl", "Simplified forms"),
    ("ssty", "Math script-style alternates"),
    ("stch", "Stretching glyph decomposition"),
    ("subs", "Subscript"),
    ("sups", "Superscript"),
    ("swsh", "Swash"),
    ("titl", "Titling"),
    ("tjmo", "Trailing jamo forms"),
    ("tnam", "Traditional name forms"),
    ("tnum", "Tabular figures"),
    ("trad", "Traditional forms"),
    ("twid", "Third widths"),
    ("unic", "Unicase"),
    ("valt", "Alternate vertical metrics"),
    (
        "vapk",
        "Kerning for alternate proportional vertical metrics",
    ),
    ("vatu", "Vattu variants"),
    ("vchw", "Vertical contextual half-width spacing"),
    ("vert", "Vertical alternates"),
    ("vhal", "Alternate vertical half metrics"),
    ("vjmo", "Vowel jamo forms"),
    ("vkna", "Vertical kana alternates"),
    ("vkrn", "Vertical kerning"),
    ("vpal", "Proportional alternate vertical metrics"),
    ("vrt2", "Vertical alternates and rotation"),
    ("vrtr", "Vertical alternates for rotation"),
    ("zero", "Slashed zero"),
];

/// Features the shaper applies by itself where the script needs them, sorted.
pub const AUTOMATIC: &[&str] = &[
    "abvf", "abvm", "abvs", "akhn", "blwf", "blwm", "blws", "ccmp", "cfar", "cjct", "curs", "dist",
    "fin2", "fin3", "fina", "half", "haln", "init", "isol", "ljmo", "locl", "ltra", "ltrm", "mark",
    "med2", "medi", "mkmk", "mset", "nukt", "pref", "pres", "pstf", "psts", "rclt", "rkrf", "rlig",
    "rphf", "rtla", "rtlm", "rvrn", "stch", "tjmo", "vatu", "vjmo",
];

/// Features a user can switch that are on in default text rendering, sorted.
pub const DEFAULT_ON: &[&str] = &["calt", "clig", "kern", "liga"];

/// The number `N` of a `{prefix}NN` tag, when it is in `1..=max`.
fn numbered(tag: &str, prefix: &str, max: u8) -> Option<u8> {
    let digits = tag.strip_prefix(prefix)?;
    if digits.len() != 2 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u8 = digits.parse().ok()?;
    (1..=max).contains(&n).then_some(n)
}

/// Whether the font can name `tag` itself: the stylistic sets `ss01`–`ss20`
/// (FeatureParams UINameID) and the character variants `cv01`–`cv99`
/// (FeatureUINameID).
pub fn named_by_font(tag: &str) -> bool {
    numbered(tag, "ss", 20).is_some() || numbered(tag, "cv", 99).is_some()
}

/// The friendly name of `tag`, or `None` when the tag is not registered.
pub fn feature_name(tag: &str) -> Option<String> {
    if let Ok(i) = REGISTRY.binary_search_by(|(t, _)| (*t).cmp(tag)) {
        return Some(REGISTRY[i].1.to_owned());
    }
    if let Some(n) = numbered(tag, "ss", 20) {
        return Some(format!("Stylistic set {n}"));
    }
    numbered(tag, "cv", 99).map(|n| format!("Character variant {n}"))
}

/// What the Features tab needs to know about `tag`.
pub fn feature_info(tag: &str) -> FeatureInfo {
    feature_info_named(tag, None)
}

/// [`feature_info`] with the font's own name for `tag` ([`named_by_font`]),
/// which replaces the generic "Stylistic set N" when there is one.
pub fn feature_info_named(tag: &str, font_name: Option<&str>) -> FeatureInfo {
    FeatureInfo {
        tag: tag.to_owned(),
        name: font_name.map(str::to_owned).or_else(|| feature_name(tag)),
        automatic: AUTOMATIC.binary_search(&tag).is_ok(),
        default_on: DEFAULT_ON.binary_search(&tag).is_ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted_unique(tags: &[&str]) -> bool {
        tags.windows(2).all(|w| w[0] < w[1])
    }

    #[test]
    fn every_registry_tag_has_a_name() {
        let tags: Vec<&str> = REGISTRY.iter().map(|(t, _)| *t).collect();
        assert!(
            sorted_unique(&tags),
            "REGISTRY must be sorted for binary search"
        );
        for (tag, name) in REGISTRY {
            assert_eq!(tag.len(), 4, "{tag}");
            assert!(!name.is_empty(), "{tag}");
            assert_eq!(feature_name(tag).as_deref(), Some(*name), "{tag}");
            assert_eq!(feature_info(tag).name.as_deref(), Some(*name), "{tag}");
        }
    }

    #[test]
    fn named_examples_from_the_story() {
        assert_eq!(feature_name("smcp").as_deref(), Some("Small capitals"));
        assert_eq!(feature_name("onum").as_deref(), Some("Oldstyle figures"));
        assert_eq!(feature_name("liga").as_deref(), Some("Standard ligatures"));
        assert_eq!(feature_name("kern").as_deref(), Some("Kerning"));
    }

    #[test]
    fn stylistic_sets_and_character_variants_are_numbered() {
        for n in 1..=20 {
            assert_eq!(
                feature_name(&format!("ss{n:02}")),
                Some(format!("Stylistic set {n}"))
            );
        }
        for n in 1..=99 {
            assert_eq!(
                feature_name(&format!("cv{n:02}")),
                Some(format!("Character variant {n}"))
            );
        }
    }

    #[test]
    fn only_numbered_sets_and_variants_are_named_by_the_font() {
        for tag in ["ss01", "ss20", "cv01", "cv99"] {
            assert!(named_by_font(tag), "{tag}");
        }
        for tag in [
            "ss00", "ss21", "ssty", "cv00", "cv100", "cv1", "smcp", "size", "",
        ] {
            assert!(!named_by_font(tag), "{tag:?}");
        }
    }

    #[test]
    fn the_font_name_replaces_the_generic_one() {
        let named = feature_info_named("ss01", Some("Single-storey a"));
        assert_eq!(named.name.as_deref(), Some("Single-storey a"));
        assert!(!named.automatic && !named.default_on);
        assert_eq!(feature_info_named("ss01", None), feature_info("ss01"));
        assert_eq!(
            feature_info("ss01").name.as_deref(),
            Some("Stylistic set 1")
        );
    }

    #[test]
    fn unregistered_tags_have_no_name() {
        for tag in [
            "ss00", "ss21", "ss99", "cv00", "cv1", "cv100", "ssxx", "cv+1", "abcd", "SMCP", "",
            "kern ",
        ] {
            let info = feature_info(tag);
            assert_eq!(info.name, None, "{tag:?}");
            assert_eq!(info.tag, tag);
            assert!(!info.automatic && !info.default_on, "{tag:?}");
        }
    }

    #[test]
    fn shaper_features_are_automatic_and_registered() {
        assert!(sorted_unique(AUTOMATIC));
        for tag in AUTOMATIC {
            let info = feature_info(tag);
            assert!(info.automatic, "{tag}");
            assert!(!info.default_on, "{tag}");
            assert!(info.name.is_some(), "{tag}");
        }
        for tag in [
            "ccmp", "locl", "mark", "mkmk", "rlig", "init", "medi", "fina", "rvrn",
        ] {
            assert!(feature_info(tag).automatic, "{tag}");
        }
    }

    #[test]
    fn browser_defaults_are_on_and_the_rest_off() {
        assert!(sorted_unique(DEFAULT_ON));
        for tag in ["kern", "liga", "calt"] {
            let info = feature_info(tag);
            assert!(info.default_on && !info.automatic, "{tag}");
        }
        for tag in [
            "smcp", "onum", "ss01", "cv01", "dlig", "tnum", "zero", "frac",
        ] {
            let info = feature_info(tag);
            assert!(!info.default_on && !info.automatic, "{tag}");
        }
    }
}
