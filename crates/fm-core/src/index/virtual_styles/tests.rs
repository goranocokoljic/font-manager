use std::path::PathBuf;

use fm_api::common::{Axis, AxisValue, FontFormat, Health};
use fm_api::library::SourceKind;
use fm_api::organization::{CreateCollectionRequest, CreateSetRequest};
use fm_api::search::Filters;

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};

fn axis(tag: &str, name: &str, min: f32, default: f32, max: f32) -> Axis {
    Axis {
        tag: tag.into(),
        name: name.into(),
        min,
        default,
        max,
    }
}

fn value(tag: &str, value: f32) -> AxisValue {
    AxisValue {
        tag: tag.into(),
        value,
    }
}

/// A width axis then a weight axis, in that order, as `fvar` lists them.
fn axes() -> Vec<Axis> {
    vec![
        axis("wdth", "Width", 75.0, 100.0, 125.0),
        axis("wght", "Weight", 100.0, 400.0, 900.0),
    ]
}

fn file(path: &str, postscript: &str, axes: Vec<Axis>) -> IndexedFile {
    let mut face = ParsedFace::empty(0, Health::Ok);
    face.family = Some("Asap".into());
    face.subfamily = Some(postscript.rsplit('-').next().unwrap_or("Regular").into());
    face.postscript_name = Some(postscript.into());
    face.weight = Some(400);
    face.width = Some(5);
    face.axes = axes;
    IndexedFile {
        path: PathBuf::from(path),
        size: 1000,
        modified: None,
        hash: FileHash::from([3; 32]),
        parsed: ParsedFile {
            format: Some(FontFormat::Ttf),
            health: Health::Ok,
            faces: vec![face],
        },
        gone: false,
    }
}

/// An index with a variable face and a static one, a manual collection, a
/// smart one and a set.
struct Lib {
    index: Index,
    variable: FaceId,
    fixed: FaceId,
    collection: CollectionId,
    smart: CollectionId,
    set: SetId,
}

impl Lib {
    fn new() -> Self {
        let mut index = Index::open_in_memory().expect("open");
        let source = index
            .add_source("/fonts", SourceKind::Watched)
            .expect("source")
            .id;
        index
            .persist_files(
                source,
                &[
                    file("/fonts/Asap[wdth,wght].ttf", "Asap-Regular", axes()),
                    file("/fonts/Asap-Bold.ttf", "Asap-Bold", Vec::new()),
                ],
            )
            .expect("persist");
        let face = |ps: &str| {
            index
                .conn
                .query_row(
                    "SELECT id FROM face WHERE postscript_name = ?1",
                    [ps],
                    |r| r.get(0),
                )
                .map(FaceId)
                .expect("face")
        };
        let (variable, fixed) = (face("Asap-Regular"), face("Asap-Bold"));
        let collection = index
            .create_collection(&CreateCollectionRequest {
                rule_group: None,
                name: "Brand".into(),
                parent: None,
                rules: None,
            })
            .expect("collection")
            .id;
        let smart = index
            .create_collection(&CreateCollectionRequest {
                rule_group: None,
                name: "Variable".into(),
                parent: None,
                rules: Some(Filters {
                    variable: Some(true),
                    ..Filters::default()
                }),
            })
            .expect("smart")
            .id;
        let set = index
            .create_set(&CreateSetRequest {
                name: "Client".into(),
                faces: Vec::new(),
            })
            .expect("set")
            .id;
        Self {
            index,
            variable,
            fixed,
            collection,
            smart,
            set,
        }
    }

    fn pin(
        &mut self,
        face: FaceId,
        name: &str,
        coordinates: Vec<AxisValue>,
        collection: Option<CollectionId>,
        set: Option<SetId>,
    ) -> FmResult<VirtualStyle> {
        self.index.pin_instance(&PinInstanceRequest {
            face,
            name: name.into(),
            coordinates,
            collection,
            set,
        })
    }

    fn in_collection(&self) -> Vec<VirtualStyle> {
        self.index
            .virtual_styles(&VirtualStylesRequest {
                collection: Some(self.collection),
                set: None,
            })
            .expect("list")
    }

    fn in_set(&self) -> Vec<VirtualStyle> {
        self.index
            .virtual_styles(&VirtualStylesRequest {
                collection: None,
                set: Some(self.set),
            })
            .expect("list")
    }
}

#[test]
fn coordinates_left_out_are_the_axis_default_in_the_fonts_axis_order() {
    let got = resolve_coordinates("Asap Regular", &axes(), &[value("wght", 650.0)]).expect("ok");
    assert_eq!(got, vec![value("wdth", 100.0), value("wght", 650.0)]);
    let got = resolve_coordinates("Asap Regular", &axes(), &[]).expect("ok");
    assert_eq!(got, vec![value("wdth", 100.0), value("wght", 400.0)]);
    // The ends of an axis are inside it.
    let got = resolve_coordinates(
        "Asap Regular",
        &axes(),
        &[value("wght", 900.0), value("wdth", 75.0)],
    )
    .expect("ok");
    assert_eq!(got, vec![value("wdth", 75.0), value("wght", 900.0)]);
}

#[test]
fn a_static_face_an_unknown_axis_or_an_out_of_range_value_is_refused() {
    let err = resolve_coordinates("Asap Bold", &[], &[]).expect_err("static");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert!(err.message().contains("not a variable font"), "{err:?}");

    let err = resolve_coordinates("Asap Regular", &axes(), &[value("slnt", -5.0)])
        .expect_err("unknown axis");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert!(err.message().contains("“slnt”"), "{err:?}");
    assert!(err.message().contains("wdth, wght"), "{err:?}");

    for bad in [950.0, 99.5, f32::NAN, f32::INFINITY] {
        let err = resolve_coordinates("Asap Regular", &axes(), &[value("wght", bad)])
            .expect_err("out of range");
        assert_eq!(err.code(), ErrorCode::InvalidRequest, "{bad}");
        assert!(
            err.message().contains("Weight (wght) goes from 100 to 900"),
            "{err:?}"
        );
    }

    let err = resolve_coordinates(
        "Asap Regular",
        &axes(),
        &[value("wght", 500.0), value("wght", 600.0)],
    )
    .expect_err("twice");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
}

#[test]
fn a_pin_is_stored_and_listed_by_its_holder() {
    let mut lib = Lib::new();
    let (variable, collection, set) = (lib.variable, lib.collection, lib.set);
    let semibold = lib
        .pin(
            variable,
            "  Condensed Semibold ",
            vec![value("wght", 600.0), value("wdth", 80.0)],
            Some(collection),
            None,
        )
        .expect("pin");
    assert_eq!(semibold.face, variable);
    assert_eq!(semibold.name, "Condensed Semibold");
    assert_eq!(
        semibold.coordinates,
        vec![value("wdth", 80.0), value("wght", 600.0)]
    );
    assert_eq!(semibold.collection, Some(collection));
    assert_eq!(semibold.set, None);
    let book = lib
        .pin(
            variable,
            "Book",
            vec![value("wght", 350.0)],
            Some(collection),
            None,
        )
        .expect("pin");
    let in_set = lib
        .pin(variable, "Book", Vec::new(), None, Some(set))
        .expect("same name, other holder");

    // A to Z, and only the holder's own.
    assert_eq!(lib.in_collection(), vec![book.clone(), semibold.clone()]);
    assert_eq!(lib.in_set(), vec![in_set.clone()]);

    lib.index.delete_virtual_style(book.id).expect("delete");
    assert_eq!(lib.in_collection(), vec![semibold]);
    let err = lib.index.delete_virtual_style(book.id).expect_err("gone");
    assert_eq!(err.code(), ErrorCode::NotFound);
    // The face is still there.
    assert!(lib.index.face_detail(variable).is_ok());
}

#[test]
fn a_bad_pin_is_an_error_with_a_message_and_stores_nothing() {
    let mut lib = Lib::new();
    let (variable, fixed, collection, smart, set) =
        (lib.variable, lib.fixed, lib.collection, lib.smart, lib.set);
    let cases: Vec<(FmResult<VirtualStyle>, ErrorCode, &str)> = vec![
        (
            lib.pin(fixed, "Light", Vec::new(), Some(collection), None),
            ErrorCode::InvalidRequest,
            "“Asap Bold” is not a variable font",
        ),
        (
            lib.pin(
                variable,
                "X",
                vec![value("ital", 1.0)],
                Some(collection),
                None,
            ),
            ErrorCode::InvalidRequest,
            "no axis “ital”",
        ),
        (
            lib.pin(
                variable,
                "X",
                vec![value("wdth", 130.0)],
                Some(collection),
                None,
            ),
            ErrorCode::InvalidRequest,
            "Width (wdth) goes from 75 to 125; 130 is outside it.",
        ),
        (
            lib.pin(variable, "   ", Vec::new(), Some(collection), None),
            ErrorCode::InvalidRequest,
            "Give the style a name.",
        ),
        (
            lib.pin(variable, "X", Vec::new(), None, None),
            ErrorCode::InvalidRequest,
            "one collection or one set",
        ),
        (
            lib.pin(variable, "X", Vec::new(), Some(collection), Some(set)),
            ErrorCode::InvalidRequest,
            "one collection or one set",
        ),
        (
            lib.pin(variable, "X", Vec::new(), Some(smart), None),
            ErrorCode::InvalidRequest,
            "not a smart collection",
        ),
        (
            lib.pin(variable, "X", Vec::new(), Some(CollectionId(9_999)), None),
            ErrorCode::NotFound,
            "Collection 9999",
        ),
        (
            lib.pin(variable, "X", Vec::new(), None, Some(SetId(9_999))),
            ErrorCode::NotFound,
            "Set 9999",
        ),
        (
            lib.pin(FaceId(9_999), "X", Vec::new(), Some(collection), None),
            ErrorCode::NotFound,
            "Face 9999",
        ),
    ];
    for (got, code, text) in cases {
        let err = got.expect_err(text);
        assert_eq!(err.code(), code, "{err:?}");
        assert!(err.message().contains(text), "{err:?} does not say {text}");
    }
    let count: u32 = lib
        .index
        .conn
        .query_row("SELECT count(*) FROM virtual_style", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 0);

    lib.pin(variable, "Wide", Vec::new(), Some(collection), None)
        .expect("pin");
    let err = lib
        .pin(
            variable,
            "WIDE",
            vec![value("wght", 700.0)],
            Some(collection),
            None,
        )
        .expect_err("same name in any case");
    assert_eq!(err.code(), ErrorCode::Conflict);
    assert_eq!(lib.in_collection().len(), 1);
}

#[test]
fn deleting_the_collection_or_set_deletes_its_virtual_styles() {
    let mut lib = Lib::new();
    let (variable, collection, set) = (lib.variable, lib.collection, lib.set);
    lib.pin(variable, "Wide", Vec::new(), Some(collection), None)
        .expect("pin");
    lib.pin(
        variable,
        "Black",
        vec![value("wght", 900.0)],
        None,
        Some(set),
    )
    .expect("pin");
    let other = lib
        .index
        .create_set(&CreateSetRequest {
            name: "Other".into(),
            faces: Vec::new(),
        })
        .expect("set")
        .id;
    lib.pin(
        variable,
        "Black",
        vec![value("wght", 900.0)],
        None,
        Some(other),
    )
    .expect("pin");

    lib.index.delete_collection(collection).expect("delete");
    lib.index.delete_set(set).expect("delete");

    let left: Vec<(String, Option<u32>)> = {
        let mut stmt = lib
            .index
            .conn
            .prepare("SELECT name, set_id FROM virtual_style")
            .expect("query");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("rows")
            .collect::<Result<_, _>>()
            .expect("rows")
    };
    assert_eq!(left, vec![("Black".to_owned(), Some(other.0))]);
    assert!(lib.index.face_detail(variable).is_ok(), "the face stays");
}
