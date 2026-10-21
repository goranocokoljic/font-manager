use fm_api::common::Axis;
use fm_api::search::{WeightRange, WidthRange};

use super::*;
use crate::search::FIELD_COUNT;

fn text(weight: u16, width: u16, slant: Slant, axes: Vec<Axis>) -> FaceText {
    FaceText::new([None; FIELD_COUNT], weight, width, slant, axes)
}

fn upright(weight: u16) -> FaceText {
    text(weight, 5, Slant::Upright, Vec::new())
}

fn axis(tag: &str, min: f32, max: f32) -> Axis {
    Axis {
        tag: tag.to_owned(),
        name: tag.to_owned(),
        min,
        default: min,
        max,
    }
}

/// A face with a value for every filter, each different from the default.
fn rich() -> FaceFacts {
    FaceFacts {
        classification: Some(Classification::Serif),
        is_variable: false,
        color: true,
        format: Some(FontFormat::Otf),
        features: vec![*b"kern", *b"liga", *b"smcp"],
        license: Some(LicenseType::Web),
        foundry: Some("Adobe".into()),
        source: SourceId(2),
        tags: vec![TagId(5)],
        collections: vec![CollectionId(9)],
        active: true,
        favorite: true,
        rating: Some(4),
        problems: true,
        embedding: Embedding::PreviewAndPrint,
        open_license: Some(OpenLicense::Ofl),
    }
}

/// Every filter set to the value `rich()` has, and its bit when set to one
/// it does not have.
fn each_filter() -> Vec<(Filters, Filters, Dim)> {
    let pass = |set: fn(&mut Filters)| {
        let mut f = Filters::default();
        set(&mut f);
        f
    };
    vec![
        (
            pass(|f| f.classification = vec![Classification::Sans, Classification::Serif]),
            pass(|f| f.classification = vec![Classification::Sans]),
            Dim::Classification,
        ),
        (
            pass(|f| f.weight = Some(WeightRange { min: 300, max: 500 })),
            pass(|f| f.weight = Some(WeightRange { min: 600, max: 900 })),
            Dim::Weight,
        ),
        (
            pass(|f| f.width = Some(WidthRange { min: 5, max: 5 })),
            pass(|f| f.width = Some(WidthRange { min: 1, max: 4 })),
            Dim::Width,
        ),
        (
            pass(|f| f.slant = Some(Slant::Upright)),
            pass(|f| f.slant = Some(Slant::Italic)),
            Dim::Slant,
        ),
        (
            pass(|f| f.variable = Some(false)),
            pass(|f| f.variable = Some(true)),
            Dim::Variable,
        ),
        (
            pass(|f| f.color = Some(true)),
            pass(|f| f.color = Some(false)),
            Dim::Color,
        ),
        (
            pass(|f| f.formats = vec![FontFormat::Ttf, FontFormat::Otf]),
            pass(|f| f.formats = vec![FontFormat::Woff2]),
            Dim::Format,
        ),
        (
            pass(|f| f.features = vec!["smcp".into(), "kern".into()]),
            pass(|f| f.features = vec!["smcp".into(), "onum".into()]),
            Dim::Features,
        ),
        (
            pass(|f| f.license = Some(LicenseType::Web)),
            pass(|f| f.license = Some(LicenseType::Desktop)),
            Dim::License,
        ),
        (
            pass(|f| f.foundry = Some("Adobe".into())),
            pass(|f| f.foundry = Some("Google".into())),
            Dim::Foundry,
        ),
        (
            pass(|f| f.sources = vec![SourceId(1), SourceId(2)]),
            pass(|f| f.sources = vec![SourceId(1)]),
            Dim::Sources,
        ),
        (
            pass(|f| f.tags = vec![TagId(5)]),
            pass(|f| f.tags = vec![TagId(5), TagId(6)]),
            Dim::Tags,
        ),
        (
            pass(|f| f.collection = Some(CollectionId(9))),
            pass(|f| f.collection = Some(CollectionId(8))),
            Dim::Collection,
        ),
        (
            pass(|f| f.active = Some(true)),
            pass(|f| f.active = Some(false)),
            Dim::Active,
        ),
        (
            pass(|f| f.favorite = Some(true)),
            pass(|f| f.favorite = Some(false)),
            Dim::Favorite,
        ),
        (
            pass(|f| f.min_rating = Some(4)),
            pass(|f| f.min_rating = Some(5)),
            Dim::Rating,
        ),
        (
            pass(|f| f.problems = Some(true)),
            pass(|f| f.problems = Some(false)),
            Dim::Problems,
        ),
        (
            pass(|f| f.embedding = Some(Embedding::PreviewAndPrint)),
            pass(|f| f.embedding = Some(Embedding::Installable)),
            Dim::Embedding,
        ),
        (
            pass(|f| f.open_license = Some(true)),
            pass(|f| f.open_license = Some(false)),
            Dim::OpenLicense,
        ),
        (
            pass(|f| f.web_ok = Some(true)),
            pass(|f| f.web_ok = Some(false)),
            Dim::WebOk,
        ),
    ]
}

#[test]
fn every_filter_passes_its_value_and_fails_another() {
    let face = upright(400);
    let facts = rich();
    assert_eq!(failed(&Filters::default(), &face, &facts), 0);
    for (pass, fail, dim) in each_filter() {
        assert_eq!(failed(&pass, &face, &facts), 0, "{dim:?} should pass");
        assert_eq!(
            failed(&fail, &face, &facts),
            dim.bit(),
            "{dim:?} should fail"
        );
    }
}

#[test]
fn filters_compose_with_and() {
    let face = upright(400);
    let facts = rich();
    let mut all = Filters::default();
    let mut expected = 0;
    for (_, fail, dim) in each_filter() {
        merge(&mut all, &fail);
        expected |= dim.bit();
    }
    assert_eq!(failed(&all, &face, &facts), expected);

    // Every filter at a passing value, then one at a failing value: the face
    // fails that one only.
    let mut passing = Filters::default();
    for (pass, _, _) in each_filter() {
        merge(&mut passing, &pass);
    }
    assert_eq!(failed(&passing, &face, &facts), 0);
    passing.color = Some(false);
    assert_eq!(failed(&passing, &face, &facts), Dim::Color.bit());
}

/// Copies every field `from` sets into `into`.
fn merge(into: &mut Filters, from: &Filters) {
    let d = Filters::default();
    if from.classification != d.classification {
        into.classification.clone_from(&from.classification);
    }
    into.weight = from.weight.or(into.weight);
    into.width = from.width.or(into.width);
    into.slant = from.slant.or(into.slant);
    into.variable = from.variable.or(into.variable);
    into.color = from.color.or(into.color);
    if !from.formats.is_empty() {
        into.formats.clone_from(&from.formats);
    }
    if !from.features.is_empty() {
        into.features.clone_from(&from.features);
    }
    into.license = from.license.or(into.license);
    if from.foundry.is_some() {
        into.foundry.clone_from(&from.foundry);
    }
    if !from.sources.is_empty() {
        into.sources.clone_from(&from.sources);
    }
    if !from.tags.is_empty() {
        into.tags.clone_from(&from.tags);
    }
    into.collection = from.collection.or(into.collection);
    into.active = from.active.or(into.active);
    into.favorite = from.favorite.or(into.favorite);
    into.min_rating = from.min_rating.or(into.min_rating);
    into.problems = from.problems.or(into.problems);
    into.embedding = from.embedding.or(into.embedding);
    into.open_license = from.open_license.or(into.open_license);
    into.web_ok = from.web_ok.or(into.web_ok);
}

#[test]
fn an_unclassified_face_fails_a_classification_filter() {
    let facts = FaceFacts::new(SourceId(1));
    let f = Filters {
        classification: vec![Classification::Sans],
        ..Filters::default()
    };
    assert_eq!(failed(&f, &upright(400), &facts), Dim::Classification.bit());
}

#[test]
fn a_variable_face_passes_the_ranges_its_axes_reach() {
    let facts = FaceFacts {
        is_variable: true,
        ..FaceFacts::new(SourceId(1))
    };
    let face = text(
        400,
        5,
        Slant::Upright,
        vec![
            axis("wght", 100.0, 900.0),
            axis("wdth", 75.0, 100.0),
            axis("slnt", -10.0, 0.0),
        ],
    );
    let range = |min, max| Filters {
        weight: Some(WeightRange { min, max }),
        ..Filters::default()
    };
    assert_eq!(failed(&range(800, 900), &face, &facts), 0);
    assert_eq!(failed(&range(950, 1000), &face, &facts), Dim::Weight.bit());
    let width = |min, max| Filters {
        width: Some(WidthRange { min, max }),
        ..Filters::default()
    };
    assert_eq!(failed(&width(3, 3), &face, &facts), 0);
    assert_eq!(failed(&width(7, 9), &face, &facts), Dim::Width.bit());
    let slant = |s| Filters {
        slant: Some(s),
        ..Filters::default()
    };
    assert_eq!(failed(&slant(Slant::Oblique), &face, &facts), 0);
    assert_eq!(
        failed(&slant(Slant::Italic), &face, &facts),
        Dim::Slant.bit()
    );
}

/// A face ready to count: family number, style, facts and slots.
type Prepared<'a> = (u32, &'a FaceText, &'a FaceFacts, Vec<u32>);

/// Families of (face style, facts), counted with `filters`.
fn tally(filters: &Filters, families: &[Vec<(FaceText, FaceFacts)>]) -> FilterCounts {
    let mut slots = Slots::default();
    let faces: Vec<Vec<Prepared<'_>>> = families
        .iter()
        .enumerate()
        .map(|(n, faces)| {
            faces
                .iter()
                .map(|(t, f)| {
                    let n = u32::try_from(n).expect("small");
                    (n, t, f, slots.intern(&values(t, f)))
                })
                .collect()
        })
        .collect();
    let mut tally = Tally::new(&slots);
    for family in &faces {
        for (n, t, f, s) in family {
            tally.add(*n, failed(filters, t, f), s);
        }
    }
    tally.finish()
}

fn count<T: PartialEq>(list: &[FilterCount<T>], value: &T) -> u32 {
    list.iter()
        .find(|c| &c.value == value)
        .map_or(0, |c| c.count)
}

/// Sans (Regular, Bold Italic), Serif (Bold, variable 100–900), Mono (Regular).
fn three_families() -> Vec<Vec<(FaceText, FaceFacts)>> {
    let facts = |c, source: u32, features: &[&str]| FaceFacts {
        classification: Some(c),
        format: Some(FontFormat::Ttf),
        features: features.iter().filter_map(|s| tag(s)).collect(),
        ..FaceFacts::new(SourceId(source))
    };
    let mut variable = facts(Classification::Serif, 2, &["kern", "smcp"]);
    variable.is_variable = true;
    variable.format = Some(FontFormat::Woff2);
    vec![
        vec![
            (upright(400), facts(Classification::Sans, 1, &["kern"])),
            (
                text(700, 5, Slant::Italic, Vec::new()),
                FaceFacts {
                    active: true,
                    ..facts(Classification::Sans, 1, &["kern", "liga"])
                },
            ),
        ],
        vec![
            (upright(700), facts(Classification::Serif, 2, &["kern"])),
            (
                text(400, 5, Slant::Upright, vec![axis("wght", 100.0, 900.0)]),
                variable,
            ),
        ],
        vec![(
            text(400, 3, Slant::Upright, Vec::new()),
            FaceFacts {
                problems: true,
                ..facts(Classification::Mono, 1, &[])
            },
        )],
    ]
}

#[test]
fn counts_are_families_per_value_without_their_own_filter() {
    let counts = tally(&Filters::default(), &three_families());
    assert_eq!(count(&counts.classification, &Classification::Sans), 1);
    assert_eq!(count(&counts.classification, &Classification::Serif), 1);
    assert_eq!(count(&counts.classification, &Classification::Slab), 0);
    // Two Serif faces reach 700: the family counts once.
    assert_eq!(count(&counts.weight, &700), 2);
    // Only the variable face reaches 100.
    assert_eq!(count(&counts.weight, &100), 1);
    assert_eq!(count(&counts.width, &5), 2);
    assert_eq!(count(&counts.width, &3), 1);
    assert_eq!(count(&counts.slant, &Slant::Italic), 1);
    assert_eq!(count(&counts.slant, &Slant::Upright), 3);
    assert_eq!(count(&counts.variable, &true), 1);
    assert_eq!(count(&counts.variable, &false), 3);
    assert_eq!(count(&counts.color, &false), 3);
    assert_eq!(count(&counts.color, &true), 0);
    assert_eq!(count(&counts.formats, &FontFormat::Woff2), 1);
    assert_eq!(count(&counts.formats, &FontFormat::Ttf), 3);
    assert_eq!(count(&counts.features, &"kern".to_owned()), 2);
    assert_eq!(count(&counts.features, &"smcp".to_owned()), 1);
    assert_eq!(count(&counts.sources, &SourceId(1)), 2);
    assert_eq!(count(&counts.problems, &true), 1);
    assert_eq!(count(&counts.active, &true), 1);
    assert_eq!(count(&counts.active, &false), 3);
    // Values with no result are left out.
    assert!(counts.color.iter().all(|c| c.count > 0));
    assert!(counts.license.is_empty());

    // With classification = Sans, the other filters count inside Sans, and
    // classification itself still counts every class.
    let sans = Filters {
        classification: vec![Classification::Sans],
        ..Filters::default()
    };
    let counts = tally(&sans, &three_families());
    assert_eq!(count(&counts.classification, &Classification::Serif), 1);
    assert_eq!(count(&counts.classification, &Classification::Mono), 1);
    assert_eq!(count(&counts.formats, &FontFormat::Ttf), 1);
    assert_eq!(count(&counts.formats, &FontFormat::Woff2), 0);
    assert_eq!(count(&counts.sources, &SourceId(2)), 0);
    assert_eq!(count(&counts.slant, &Slant::Italic), 1);
}

#[test]
fn counts_count_the_faces_that_pass_the_other_filters() {
    // Italic only: the Sans family counts under 700 (its Bold Italic), not 400.
    let italic = Filters {
        slant: Some(Slant::Italic),
        ..Filters::default()
    };
    let counts = tally(&italic, &three_families());
    assert_eq!(count(&counts.weight, &700), 1);
    assert_eq!(count(&counts.weight, &400), 0);
    assert_eq!(count(&counts.features, &"liga".to_owned()), 1);
    assert_eq!(count(&counts.slant, &Slant::Upright), 3);
}

#[test]
fn feature_counts_keep_the_features_already_chosen() {
    let kern = Filters {
        features: vec!["kern".into()],
        ..Filters::default()
    };
    let counts = tally(&kern, &three_families());
    assert_eq!(count(&counts.features, &"kern".to_owned()), 2);
    assert_eq!(count(&counts.features, &"smcp".to_owned()), 1);
    let smcp = Filters {
        features: vec!["smcp".into()],
        ..Filters::default()
    };
    let counts = tally(&smcp, &three_families());
    // kern + smcp: only the Serif variable face has both.
    assert_eq!(count(&counts.features, &"kern".to_owned()), 1);
    assert_eq!(count(&counts.features, &"liga".to_owned()), 0);
}

/// One family per face: (tags, favorite, rating).
fn rated(faces: &[(&[u32], bool, Option<u8>)]) -> Vec<Vec<(FaceText, FaceFacts)>> {
    faces
        .iter()
        .map(|(tags, favorite, rating)| {
            vec![(
                upright(400),
                FaceFacts {
                    tags: tags.iter().copied().map(TagId).collect(),
                    favorite: *favorite,
                    rating: *rating,
                    ..FaceFacts::new(SourceId(1))
                },
            )]
        })
        .collect()
}

#[test]
fn tag_favorite_and_rating_counts() {
    let families = rated(&[
        (&[1, 2], true, Some(5)),
        (&[1], false, Some(2)),
        (&[], true, None),
    ]);
    let counts = tally(&Filters::default(), &families);
    assert_eq!(count(&counts.tags, &TagId(1)), 2);
    assert_eq!(count(&counts.tags, &TagId(2)), 1);
    assert_eq!(count(&counts.favorite, &true), 2);
    assert_eq!(count(&counts.favorite, &false), 1);
    // "At least n stars": the 5-star face counts under every value, the
    // 2-star one under 1 and 2, the unrated one under none.
    assert_eq!(count(&counts.min_rating, &1), 2);
    assert_eq!(count(&counts.min_rating, &2), 2);
    assert_eq!(count(&counts.min_rating, &3), 1);
    assert_eq!(count(&counts.min_rating, &5), 1);
    let values: Vec<u8> = counts.min_rating.iter().map(|c| c.value).collect();
    assert_eq!(values, [1, 2, 3, 4, 5]);

    // Favorites only: the other filters count inside favorites, favorite
    // itself still counts both values.
    let favorites = Filters {
        favorite: Some(true),
        ..Filters::default()
    };
    let counts = tally(&favorites, &families);
    assert_eq!(count(&counts.tags, &TagId(1)), 1);
    assert_eq!(count(&counts.min_rating, &2), 1);
    assert_eq!(count(&counts.favorite, &false), 1);

    // At least 3 stars: rating counts every value, the rest count inside it.
    let three = Filters {
        min_rating: Some(3),
        ..Filters::default()
    };
    let counts = tally(&three, &families);
    assert_eq!(count(&counts.min_rating, &1), 2);
    assert_eq!(count(&counts.favorite, &true), 1);
    assert_eq!(count(&counts.favorite, &false), 0);
}

#[test]
fn tag_counts_keep_the_tags_already_chosen() {
    let families = rated(&[
        (&[1, 2], false, None),
        (&[1], false, None),
        (&[2], false, None),
    ]);
    let one = Filters {
        tags: vec![TagId(1)],
        ..Filters::default()
    };
    let counts = tally(&one, &families);
    // Tag 1 and 2: only the first family has both.
    assert_eq!(count(&counts.tags, &TagId(1)), 2);
    assert_eq!(count(&counts.tags, &TagId(2)), 1);
}

#[test]
fn counts_list_values_in_a_fixed_order() {
    let counts = tally(&Filters::default(), &three_families());
    let weights: Vec<u16> = counts.weight.iter().map(|c| c.value).collect();
    let mut sorted = weights.clone();
    sorted.sort_unstable();
    assert_eq!(weights, sorted);
    let features: Vec<&str> = counts.features.iter().map(|c| c.value.as_str()).collect();
    assert_eq!(features, ["kern", "liga", "smcp"]);
    let classes: Vec<Classification> = counts.classification.iter().map(|c| c.value).collect();
    assert_eq!(
        classes,
        [
            Classification::Serif,
            Classification::Sans,
            Classification::Mono
        ]
    );
}

#[test]
fn every_license_type_is_counted() {
    for license in LICENSES {
        let facts = FaceFacts {
            license: Some(license),
            ..FaceFacts::new(SourceId(1))
        };
        let counts = tally(&Filters::default(), &[vec![(upright(400), facts)]]);
        assert_eq!(count(&counts.license, &license), 1, "{license:?}");
    }
}

#[test]
fn a_weight_class_counts_the_weights_that_round_to_it() {
    assert_eq!(weight_span(100), (1, 149));
    assert_eq!(weight_span(400), (350, 449));
    assert_eq!(weight_span(900), (850, 1000));
    let counts = tally(
        &Filters::default(),
        &[vec![(upright(350), FaceFacts::new(SourceId(1)))]],
    );
    assert_eq!(count(&counts.weight, &400), 1);
    assert_eq!(count(&counts.weight, &300), 0);
}

#[test]
fn added_within_keeps_faces_added_at_the_cutoff_or_later() {
    let cutoff = Some("2026-10-13T09:00:00Z");
    assert_eq!(failed_added(cutoff, Some("2026-10-13T09:00:00Z")), 0);
    assert_eq!(failed_added(cutoff, Some("2026-10-20T00:00:00Z")), 0);
    assert_eq!(
        failed_added(cutoff, Some("2026-10-13T08:59:59Z")),
        Dim::Added.bit()
    );
    assert_eq!(failed_added(cutoff, None), Dim::Added.bit());
    assert_eq!(failed_added(None, None), 0);
}

#[test]
fn the_collection_bit_follows_what_the_rule_decided() {
    let mask = Dim::Tags.bit() | Dim::Collection.bit();
    assert_eq!(with_collection(mask, true), Dim::Tags.bit());
    assert_eq!(with_collection(Dim::Tags.bit(), false), mask);
}

#[test]
fn every_embedding_permission_is_counted() {
    for embedding in EMBEDDINGS {
        let facts = FaceFacts {
            embedding,
            ..FaceFacts::new(SourceId(1))
        };
        let counts = tally(&Filters::default(), &[vec![(upright(400), facts)]]);
        assert_eq!(count(&counts.embedding, &embedding), 1, "{embedding:?}");
    }
}

#[test]
fn embedding_and_open_license_counts_leave_out_their_own_filter() {
    // Family 1: an open-licensed installable face and a restricted one;
    // family 2: one restricted face with no open license.
    let face = |embedding, open_license| FaceFacts {
        embedding,
        open_license,
        ..FaceFacts::new(SourceId(1))
    };
    let families = vec![
        vec![
            (
                upright(400),
                face(Embedding::Installable, Some(OpenLicense::Ofl)),
            ),
            (upright(700), face(Embedding::Restricted, None)),
        ],
        vec![(upright(400), face(Embedding::Restricted, None))],
    ];
    let counts = tally(&Filters::default(), &families);
    assert_eq!(count(&counts.open_license, &true), 1);
    assert_eq!(count(&counts.open_license, &false), 2);
    assert_eq!(count(&counts.embedding, &Embedding::Restricted), 2);
    assert_eq!(count(&counts.embedding, &Embedding::Installable), 1);
    assert_eq!(count(&counts.embedding, &Embedding::Editable), 0);

    // Open license chosen: embedding counts inside it, open license still
    // counts both values.
    let open = Filters {
        open_license: Some(true),
        ..Filters::default()
    };
    let counts = tally(&open, &families);
    assert_eq!(count(&counts.embedding, &Embedding::Installable), 1);
    assert_eq!(count(&counts.embedding, &Embedding::Restricted), 0);
    assert_eq!(count(&counts.open_license, &false), 2);

    // Restricted chosen: only family 1's restricted face has no open license.
    let restricted = Filters {
        embedding: Some(Embedding::Restricted),
        ..Filters::default()
    };
    let counts = tally(&restricted, &families);
    assert_eq!(count(&counts.open_license, &true), 0);
    assert_eq!(count(&counts.open_license, &false), 2);
}

#[test]
fn the_web_filter_follows_the_license_rule() {
    // (embedding, open license, declared type, OK for web)
    let cases = [
        (Embedding::Installable, Some(OpenLicense::Ofl), None, true),
        (Embedding::Restricted, None, Some(LicenseType::Web), true),
        (Embedding::Restricted, Some(OpenLicense::Ofl), None, false),
        (
            Embedding::Installable,
            Some(OpenLicense::Ofl),
            Some(LicenseType::Desktop),
            false,
        ),
        (Embedding::Installable, None, None, false),
    ];
    for (embedding, open_license, license, ok) in cases {
        let facts = FaceFacts {
            embedding,
            open_license,
            license,
            ..FaceFacts::new(SourceId(1))
        };
        let text = upright(400);
        for (want, bit) in [(ok, 0), (!ok, Dim::WebOk.bit())] {
            let f = Filters {
                web_ok: Some(want),
                ..Filters::default()
            };
            assert_eq!(failed(&f, &text, &facts), bit, "{facts:?} web_ok={want}");
            assert_eq!(
                passes(&f, &text, &facts),
                bit == 0,
                "{facts:?} web_ok={want}"
            );
        }
        assert!(
            values(&text, &facts).contains(&Value::WebOk(ok)),
            "{facts:?}"
        );
    }
}

#[test]
fn web_counts_leave_out_their_own_filter() {
    // Family 1: an OFL face and a desktop-declared one; family 2: one
    // restricted face; family 3: one face declared Web.
    let face = |embedding, open_license, license| FaceFacts {
        embedding,
        open_license,
        license,
        ..FaceFacts::new(SourceId(1))
    };
    let families = vec![
        vec![
            (
                upright(400),
                face(Embedding::Installable, Some(OpenLicense::Ofl), None),
            ),
            (
                upright(700),
                face(Embedding::Installable, None, Some(LicenseType::Desktop)),
            ),
        ],
        vec![(upright(400), face(Embedding::Restricted, None, None))],
        vec![(
            upright(400),
            face(Embedding::Restricted, None, Some(LicenseType::Web)),
        )],
    ];
    let counts = tally(&Filters::default(), &families);
    assert_eq!(count(&counts.web_ok, &true), 2);
    assert_eq!(count(&counts.web_ok, &false), 2);

    // OK for web chosen: its own counts are unchanged, others count inside it.
    let web = Filters {
        web_ok: Some(true),
        ..Filters::default()
    };
    let counts = tally(&web, &families);
    assert_eq!(count(&counts.web_ok, &true), 2);
    assert_eq!(count(&counts.web_ok, &false), 2);
    assert_eq!(count(&counts.embedding, &Embedding::Restricted), 1);
    assert_eq!(count(&counts.open_license, &true), 1);
}
