//! Family grouping: how parsed faces become the families and styles the user
//! sees.
//!
//! [`group_families`] puts every face under its typographic family (name ID
//! 16), falling back to the family name (name ID 1), and to the file name when
//! a face has neither. Names match ignoring case and runs of white space.
//!
//! Each family lists its styles sorted by weight, then width, then slant. A
//! variable face lists one style per named instance, with the weight, width
//! and slant read from the instance's `wght`, `wdth`, `ital` and `slnt`
//! coordinates; a variable face without named instances is one style.
//!
//! The result does not depend on the order of the input: faces are taken in
//! (path, face index) order, and ties in the sort fall back to the style name
//! and the source.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fm_api::common::{AxisValue, Slant};

use crate::parser::ParsedFace;
use crate::scanner::FileIndex;

/// Weight used when a face declares none: OS/2 "Regular".
const DEFAULT_WEIGHT: u16 = 400;
/// Width used when a face declares none: OS/2 "Medium (normal)".
const DEFAULT_WIDTH: u16 = 5;
/// Style name used when a face or instance has none.
const DEFAULT_STYLE: &str = "Regular";

/// One family and its styles.
#[derive(Debug, Clone, PartialEq)]
pub struct FamilyGroup {
    /// Display name: the typographic family name, else the family name, else
    /// the file name, as written by the first face of the family.
    pub name: String,
    /// Styles sorted by weight, then width, then slant.
    pub styles: Vec<Style>,
}

/// One style of a family: a face, or a named instance of a variable face.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Style name: the instance name, else name ID 17, else name ID 2.
    pub name: String,
    /// Weight on the OS/2 scale (1–1000).
    pub weight: u16,
    /// Width on the OS/2 scale (1–9).
    pub width: u16,
    /// Slant.
    pub slant: Slant,
    /// The font file.
    pub path: PathBuf,
    /// The face's index in the file.
    pub face_index: u32,
    /// The named instance's index in `fvar`; `None` for a face itself.
    pub instance: Option<usize>,
    /// The face's PostScript name.
    pub postscript_name: Option<String>,
}

/// Groups the faces of every file in `index` that is not gone.
pub fn group_index(index: &FileIndex) -> Vec<FamilyGroup> {
    group_families(index.files().filter(|file| !file.gone).flat_map(|file| {
        file.parsed
            .faces
            .iter()
            .map(|face| (file.path.as_path(), face))
    }))
}

/// Groups faces into families. Families are sorted by name, ignoring case.
pub fn group_families<'a>(
    faces: impl IntoIterator<Item = (&'a Path, &'a ParsedFace)>,
) -> Vec<FamilyGroup> {
    let mut faces: Vec<_> = faces.into_iter().collect();
    faces.sort_by(|(a_path, a), (b_path, b)| a_path.cmp(b_path).then(a.index.cmp(&b.index)));

    let mut families: BTreeMap<String, FamilyGroup> = BTreeMap::new();
    for (path, face) in faces {
        let name = family_name(path, face);
        let group = families
            .entry(family_key(&name))
            .or_insert_with(|| FamilyGroup {
                name,
                styles: Vec::new(),
            });
        group.styles.extend(styles_of(path, face));
    }

    // The map is keyed by `family_key`, so the families come out sorted.
    let mut families: Vec<FamilyGroup> = families.into_values().collect();
    for family in &mut families {
        family.styles.sort_by(|a, b| {
            (a.weight, a.width, slant_rank(a.slant))
                .cmp(&(b.weight, b.width, slant_rank(b.slant)))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.face_index.cmp(&b.face_index))
                .then_with(|| a.instance.cmp(&b.instance))
        });
    }
    families
}

/// The name a face is grouped under: name ID 16, else 1, else the file stem.
pub(crate) fn family_name(path: &Path, face: &ParsedFace) -> String {
    first_name([&face.typographic_family, &face.family]).unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default()
    })
}

/// The first of `names` that is not blank, with white space collapsed.
fn first_name<const N: usize>(names: [&Option<String>; N]) -> Option<String> {
    names
        .into_iter()
        .flatten()
        .map(|name| collapse_spaces(name))
        .find(|name| !name.is_empty())
}

/// The style of the face itself, not of its named instances: name ID 17,
/// else 2, else "Regular", with the OS/2 weight and width and the slant.
pub(crate) fn face_style(path: &Path, face: &ParsedFace) -> Style {
    let name = first_name([&face.typographic_subfamily, &face.subfamily])
        .unwrap_or_else(|| DEFAULT_STYLE.to_owned());
    Style {
        slant: static_slant(face.italic_angle, &name),
        name,
        weight: face.weight.unwrap_or(DEFAULT_WEIGHT),
        width: face.width.unwrap_or(DEFAULT_WIDTH),
        path: path.to_path_buf(),
        face_index: face.index,
        instance: None,
        postscript_name: face.postscript_name.clone(),
    }
}

/// Names that differ only in case or white space are one family.
pub(crate) fn family_key(name: &str) -> String {
    collapse_spaces(name).to_lowercase()
}

fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The styles a face contributes: one per named instance of a variable face,
/// else the face itself.
fn styles_of(path: &Path, face: &ParsedFace) -> Vec<Style> {
    let base = face_style(path, face);
    let face_slant = base.slant;
    if !face.is_variable() || face.named_instances.is_empty() {
        return vec![base];
    }
    face.named_instances
        .iter()
        .enumerate()
        .map(|(i, instance)| {
            let name = Some(collapse_spaces(&instance.name))
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| DEFAULT_STYLE.to_owned());
            let coord = |tag: &str| coordinate(&instance.coordinates, tag);
            Style {
                weight: coord("wght").map_or(base.weight, weight_class),
                width: coord("wdth").map_or(base.width, width_class),
                slant: instance_slant(coord("ital"), coord("slnt"), face_slant, &name),
                name,
                instance: Some(i),
                ..base.clone()
            }
        })
        .collect()
}

fn coordinate(coordinates: &[AxisValue], tag: &str) -> Option<f32> {
    coordinates
        .iter()
        .find(|c| c.tag == tag)
        .map(|c| c.value)
        .filter(|v| v.is_finite())
}

/// A `wght` coordinate on the OS/2 weight scale.
fn weight_class(wght: f32) -> u16 {
    // Clamped to 1..=1000 first, so the cast cannot truncate.
    wght.round().clamp(1.0, 1000.0) as u16
}

/// A `wdth` coordinate (percent of normal) as the nearest OS/2 width class.
fn width_class(wdth: f32) -> u16 {
    // OpenType OS/2 usWidthClass: class n is PERCENT[n - 1] of normal.
    const PERCENT: [f32; 9] = [50.0, 62.5, 75.0, 87.5, 100.0, 112.5, 125.0, 150.0, 200.0];
    let nearest = PERCENT
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - wdth).abs().total_cmp(&(*b - wdth).abs()))
        .map_or(4, |(i, _)| i);
    // `nearest` is an index into a 9-element table.
    nearest as u16 + 1
}

/// The slant of a static face, from `post.italicAngle` and the style name.
fn static_slant(italic_angle: Option<f32>, style: &str) -> Slant {
    let style = style.to_lowercase();
    if style.contains("oblique") || style.contains("slanted") {
        Slant::Oblique
    } else if style.contains("italic") || italic_angle.is_some_and(|a| a != 0.0) {
        Slant::Italic
    } else {
        Slant::Upright
    }
}

/// The slant of a named instance: `ital` ≥ 0.5 is italic, a non-zero `slnt`
/// is oblique; otherwise the name, then the face's own slant, decide.
fn instance_slant(ital: Option<f32>, slnt: Option<f32>, face: Slant, name: &str) -> Slant {
    let lower = name.to_lowercase();
    if ital.is_some_and(|v| v >= 0.5) || lower.contains("italic") {
        Slant::Italic
    } else if slnt.is_some_and(|v| v != 0.0) || lower.contains("oblique") {
        Slant::Oblique
    } else if ital.is_some() || slnt.is_some() {
        Slant::Upright
    } else {
        face
    }
}

/// Upright, then italic, then oblique: the order of [`Slant`]'s variants.
pub(crate) fn slant_rank(slant: Slant) -> u8 {
    match slant {
        Slant::Upright => 0,
        Slant::Italic => 1,
        Slant::Oblique => 2,
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::fs;

    use fm_api::common::{Axis, Health, NamedInstance};

    use super::*;
    use crate::scanner::{FileIndex, discover};

    fn face(index: u32) -> ParsedFace {
        ParsedFace::empty(index, Health::Ok)
    }

    fn named(family: &str, typographic: Option<&str>, style: &str, weight: u16) -> ParsedFace {
        ParsedFace {
            family: Some(family.to_owned()),
            typographic_family: typographic.map(str::to_owned),
            subfamily: Some(style.to_owned()),
            weight: Some(weight),
            width: Some(5),
            ..face(0)
        }
    }

    fn axis(tag: &str, min: f32, default: f32, max: f32) -> Axis {
        Axis {
            tag: tag.to_owned(),
            name: tag.to_owned(),
            min,
            default,
            max,
        }
    }

    fn instance(name: &str, coords: &[(&str, f32)]) -> NamedInstance {
        NamedInstance {
            name: name.to_owned(),
            coordinates: coords
                .iter()
                .map(|(tag, value)| AxisValue {
                    tag: (*tag).to_owned(),
                    value: *value,
                })
                .collect(),
        }
    }

    fn names(families: &[FamilyGroup]) -> Vec<(&str, Vec<&str>)> {
        families
            .iter()
            .map(|f| {
                (
                    f.name.as_str(),
                    f.styles.iter().map(|s| s.name.as_str()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn faces_group_by_typographic_family() {
        let regular = named("Asap", Some("Asap"), "Regular", 400);
        let semibold = named("Asap SemiBold", Some("Asap"), "Regular", 600);
        let families = group_families([
            (Path::new("b/Asap-SemiBold.otf"), &semibold),
            (Path::new("a/Asap-Regular.otf"), &regular),
        ]);
        assert_eq!(names(&families), [("Asap", vec!["Regular", "Regular"])]);
        assert_eq!(families[0].styles[1].weight, 600);
    }

    #[test]
    fn the_family_name_is_the_fallback() {
        let a = named("Castoro", None, "Regular", 400);
        let b = named("Castoro", None, "Italic", 400);
        let c = named("Other", Some("  "), "Regular", 400);
        let families = group_families([
            (Path::new("a.ttf"), &a),
            (Path::new("b.ttf"), &b),
            (Path::new("c.ttf"), &c),
        ]);
        assert_eq!(
            names(&families),
            [
                ("Castoro", vec!["Regular", "Italic"]),
                ("Other", vec!["Regular"])
            ]
        );
    }

    #[test]
    fn a_face_without_names_groups_under_its_file_name() {
        let broken = ParsedFace::empty(0, Health::Broken("cut off".into()));
        let families = group_families([(Path::new("dir/agdasima-truncated.ttf"), &broken)]);
        assert_eq!(names(&families), [("agdasima-truncated", vec!["Regular"])]);
        let style = &families[0].styles[0];
        assert_eq!(
            (style.weight, style.width, style.slant),
            (400, 5, Slant::Upright)
        );
    }

    #[test]
    fn family_names_match_ignoring_case_and_spaces() {
        let a = named("Open  Sans", None, "Regular", 400);
        let b = named("open sans", None, "Bold", 700);
        let families = group_families([(Path::new("b.ttf"), &b), (Path::new("a.ttf"), &a)]);
        assert_eq!(names(&families), [("Open Sans", vec!["Regular", "Bold"])]);
    }

    #[test]
    fn families_sort_by_name_ignoring_case() {
        let z = named("zeta", None, "Regular", 400);
        let a = named("Alpha", None, "Regular", 400);
        let m = named("Mu", None, "Regular", 400);
        let families = group_families([
            (Path::new("1.ttf"), &z),
            (Path::new("2.ttf"), &m),
            (Path::new("3.ttf"), &a),
        ]);
        let order: Vec<_> = families.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(order, ["Alpha", "Mu", "zeta"]);
    }

    #[test]
    fn styles_sort_by_weight_then_width_then_slant() {
        let mut faces = Vec::new();
        for (style, weight, width, angle) in [
            ("Bold Italic", 700, 5, -12.0),
            ("Condensed", 400, 3, 0.0),
            ("Bold", 700, 5, 0.0),
            ("Italic", 400, 5, -12.0),
            ("Regular", 400, 5, 0.0),
            ("Light", 300, 5, 0.0),
            ("Oblique", 400, 5, -10.0),
            // Narrower but slanted: width must win over slant.
            ("Condensed Italic", 400, 3, -12.0),
        ] {
            faces.push(ParsedFace {
                width: Some(width),
                italic_angle: Some(angle),
                ..named("Fam", None, style, weight)
            });
        }
        let paths: Vec<PathBuf> = (0..faces.len())
            .map(|i| PathBuf::from(format!("{i}.otf")))
            .collect();
        let families = group_families(paths.iter().map(PathBuf::as_path).zip(&faces));
        assert_eq!(
            names(&families),
            [(
                "Fam",
                vec![
                    "Light",
                    "Condensed",
                    "Condensed Italic",
                    "Regular",
                    "Italic",
                    "Oblique",
                    "Bold",
                    "Bold Italic"
                ]
            )]
        );
    }

    #[test]
    fn a_static_face_slant_comes_from_angle_and_name() {
        assert_eq!(static_slant(Some(0.0), "Regular"), Slant::Upright);
        assert_eq!(static_slant(None, "Regular"), Slant::Upright);
        assert_eq!(static_slant(Some(-11.0), "Regular"), Slant::Italic);
        assert_eq!(static_slant(Some(0.0), "Bold Italic"), Slant::Italic);
        assert_eq!(static_slant(Some(-8.0), "Oblique"), Slant::Oblique);
    }

    #[test]
    fn named_instances_are_styles_of_their_family() {
        let roman = ParsedFace {
            axes: vec![axis("wght", 100.0, 400.0, 900.0)],
            named_instances: vec![
                instance("Black", &[("wght", 900.0)]),
                instance("Thin", &[("wght", 100.0)]),
                instance("Regular", &[("wght", 400.0)]),
            ],
            ..named("Albert Sans", None, "Regular", 400)
        };
        let italic = ParsedFace {
            italic_angle: Some(-12.0),
            axes: vec![axis("wght", 100.0, 400.0, 900.0)],
            named_instances: vec![
                instance("Bold Italic", &[("wght", 700.0)]),
                instance("Italic", &[("wght", 400.0)]),
            ],
            ..named("Albert Sans", None, "Italic", 400)
        };
        let families = group_families([
            (Path::new("AlbertSans[wght].ttf"), &roman),
            (Path::new("AlbertSans-Italic[wght].ttf"), &italic),
        ]);
        assert_eq!(
            names(&families),
            [(
                "Albert Sans",
                vec!["Thin", "Regular", "Italic", "Bold Italic", "Black"]
            )]
        );
        let black = &families[0].styles[4];
        assert_eq!(black.instance, Some(0));
        assert_eq!(black.path, Path::new("AlbertSans[wght].ttf"));
        assert_eq!(families[0].styles[3].slant, Slant::Italic);
    }

    #[test]
    fn a_variable_face_without_instances_is_one_style() {
        let face = ParsedFace {
            axes: vec![axis("wght", 100.0, 400.0, 900.0)],
            ..named("Var", None, "Regular", 400)
        };
        let families = group_families([(Path::new("Var.ttf"), &face)]);
        assert_eq!(names(&families), [("Var", vec!["Regular"])]);
        assert_eq!(families[0].styles[0].instance, None);
    }

    #[test]
    fn instance_coordinates_set_weight_width_and_slant() {
        let face = ParsedFace {
            axes: vec![
                axis("wdth", 75.0, 100.0, 125.0),
                axis("wght", 100.0, 400.0, 900.0),
                axis("slnt", -10.0, 0.0, 0.0),
                axis("ital", 0.0, 0.0, 1.0),
            ],
            named_instances: vec![
                instance(
                    "Condensed Bold",
                    &[
                        ("wdth", 75.0),
                        ("wght", 700.0),
                        ("slnt", 0.0),
                        ("ital", 0.0),
                    ],
                ),
                instance(
                    "Wide Slanted",
                    &[
                        ("wdth", 125.0),
                        ("wght", 400.0),
                        ("slnt", -10.0),
                        ("ital", 0.0),
                    ],
                ),
                instance(
                    "Semi Italic",
                    &[
                        ("wdth", 90.0),
                        ("wght", 612.4),
                        ("slnt", 0.0),
                        ("ital", 1.0),
                    ],
                ),
                instance(
                    "Odd",
                    &[
                        ("wdth", 1000.0),
                        ("wght", 5000.0),
                        ("slnt", 0.0),
                        ("ital", 0.0),
                    ],
                ),
            ],
            ..named("Var", None, "Regular", 400)
        };
        let families = group_families([(Path::new("Var.ttf"), &face)]);
        let styles: Vec<_> = families[0]
            .styles
            .iter()
            .map(|s| (s.name.as_str(), s.weight, s.width, s.slant))
            .collect();
        assert_eq!(
            styles,
            [
                ("Wide Slanted", 400, 7, Slant::Oblique),
                ("Semi Italic", 612, 4, Slant::Italic),
                ("Condensed Bold", 700, 3, Slant::Upright),
                ("Odd", 1000, 9, Slant::Upright),
            ]
        );
    }

    #[test]
    fn the_result_does_not_depend_on_input_order() {
        let a = named("Fam", None, "Regular", 400);
        let b = named("Fam", None, "Regular", 400);
        let forward = group_families([(Path::new("a.ttf"), &a), (Path::new("b.ttf"), &b)]);
        let backward = group_families([(Path::new("b.ttf"), &b), (Path::new("a.ttf"), &a)]);
        assert_eq!(forward, backward);
    }

    #[test]
    fn gone_files_are_not_grouped() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts/derived");
        let mut index = FileIndex::new();
        index.rescan(&root, &discover(&root).expect("discover"));
        assert_eq!(names(&group_index(&index)).len(), 2);
        index.rescan(&root, &crate::scanner::Discovery::default());
        assert!(group_index(&index).is_empty());
    }

    /// One line per family, one indented line per style; paths relative to
    /// the corpus with `/` separators, so the text is the same on every OS.
    fn render(root: &Path, families: &[FamilyGroup]) -> String {
        let mut out = String::new();
        for family in families {
            let _ = writeln!(out, "{}", family.name);
            for style in &family.styles {
                let rel = style.path.strip_prefix(root).unwrap_or(&style.path);
                let rel: Vec<_> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect();
                let instance = style
                    .instance
                    .map_or(String::new(), |i| format!(" instance {i}"));
                let _ = writeln!(
                    out,
                    "  {} | {} {} {:?} | {}#{}{}",
                    style.name,
                    style.weight,
                    style.width,
                    style.slant,
                    rel.join("/"),
                    style.face_index,
                    instance
                );
            }
        }
        out
    }

    #[test]
    fn the_fixture_corpus_groups_into_the_expected_families() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest.join("../../fixtures/fonts");
        let mut index = FileIndex::new();
        let rescan = index.rescan(&root, &discover(&root).expect("discover"));
        assert!(rescan.failed.is_empty(), "{:?}", rescan.failed);
        let actual = render(&root, &group_index(&index));

        let snapshot = manifest.join("src/snapshots/fixture-families.txt");
        if std::env::var_os("FM_UPDATE_SNAPSHOTS").is_some() {
            fs::write(&snapshot, &actual).expect("write snapshot");
        }
        let expected = fs::read_to_string(&snapshot)
            .expect("read snapshot")
            .replace("\r\n", "\n");
        assert!(
            actual == expected,
            "the fixture families changed; review and rerun with FM_UPDATE_SNAPSHOTS=1\n{actual}"
        );
    }
}
