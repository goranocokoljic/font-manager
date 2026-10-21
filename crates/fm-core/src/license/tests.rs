use std::path::{Path, PathBuf};

use fm_api::common::{Embedding, Health};
use fm_api::library::OpenLicense;

use super::*;
use crate::parser;

#[test]
fn fs_type_decodes_by_the_opentype_rules() {
    let table: [(u16, Embedding); 16] = [
        (0x0000, Embedding::Installable),
        (0x0001, Embedding::Installable), // bit 0 is reserved
        (0x0002, Embedding::Restricted),
        (0x0004, Embedding::PreviewAndPrint),
        (0x0008, Embedding::Editable),
        // Several usage bits (pre-1.3 fonts): the least restrictive wins.
        (0x0006, Embedding::PreviewAndPrint),
        (0x000A, Embedding::Editable),
        (0x000C, Embedding::Editable),
        (0x000E, Embedding::Editable),
        // No-subsetting (bit 8) and bitmap-only (bit 9) leave the permission alone.
        (0x0100, Embedding::Installable),
        (0x0200, Embedding::Installable),
        (0x0102, Embedding::Restricted),
        (0x0204, Embedding::PreviewAndPrint),
        (0x0308, Embedding::Editable),
        (0x0302, Embedding::Restricted),
        (0xFFFF, Embedding::Editable),
    ];
    for (fs_type, expected) in table {
        assert_eq!(embedding(fs_type), expected, "fsType {fs_type:#06x}");
    }
}

#[test]
fn grants_in_the_description_are_recognized() {
    let cases: [(&str, OpenLicense); 8] = [
        (
            "This Font Software is licensed under the SIL Open Font License, Version 1.1.",
            OpenLicense::Ofl,
        ),
        (
            "Copyright 2011 Cyreal\r\rThis Font Software is licensed under the SIL Open Font License,\rVersion 1.1.",
            OpenLicense::Ofl,
        ),
        ("Released under the OFL.", OpenLicense::Ofl),
        (
            "Licensed under the Apache License, Version 2.0",
            OpenLicense::Apache,
        ),
        (
            "Licensed under the Ubuntu Font Licence 1.0.",
            OpenLicense::Ufl,
        ),
        (
            "Copyright (c) 2020 Someone. Licensed under the MIT License.",
            OpenLicense::Mit,
        ),
        (
            "Distributed under the terms of the MIT license; see LICENSE.",
            OpenLicense::Mit,
        ),
        (
            "Permission is hereby granted, free of charge, to any person obtaining a copy",
            OpenLicense::Mit,
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(open_license(Some(text), None), Some(expected), "{text}");
    }
}

#[test]
fn a_license_title_at_the_start_of_a_line_is_recognized() {
    let cases: [(&str, OpenLicense); 4] = [
        (
            "SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007",
            OpenLicense::Ofl,
        ),
        (
            "Copyright 2020 Someone\n\nApache License\nVersion 2.0, January 2004",
            OpenLicense::Apache,
        ),
        (
            "UBUNTU FONT LICENCE Version 1.0\n\nPREAMBLE",
            OpenLicense::Ufl,
        ),
        (
            "MIT License\n\nCopyright (c) 2020 Someone",
            OpenLicense::Mit,
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(open_license(Some(text), None), Some(expected), "{text}");
    }
}

#[test]
fn text_that_only_mentions_a_license_is_not_badged() {
    let mentions = [
        "Unlike our free fonts under the SIL Open Font License, this font is commercial.",
        "This font is not licensed under the SIL Open Font License.",
        "This font isn't released under the Apache License.",
        "Licensed under a commercial EULA. Ask about our OFL fonts.",
        "Our OFL FAQ lives at https://openfontlicense.org.",
        "Commercial license; see the MIT License FAQ for comparison.",
        "Licensed under the Submit Fonts EULA.",
        "All rights reserved.",
        "",
    ];
    for text in mentions {
        assert_eq!(open_license(Some(text), None), None, "{text}");
    }
}

#[test]
fn license_urls_are_recognized_only_when_they_are_the_license() {
    let cases: [(&str, Option<OpenLicense>); 12] = [
        ("http://scripts.sil.org/OFL", Some(OpenLicense::Ofl)),
        (" https://scripts.sil.org/OFL ", Some(OpenLicense::Ofl)),
        ("https://openfontlicense.org", Some(OpenLicense::Ofl)),
        (
            "http://scripts.sil.org/cms/scripts/page.php?site_id=nrsi&id=OFL",
            Some(OpenLicense::Ofl),
        ),
        (
            "http://www.apache.org/licenses/LICENSE-2.0",
            Some(OpenLicense::Apache),
        ),
        ("http://font.ubuntu.com/ufl/", Some(OpenLicense::Ufl)),
        (
            "https://opensource.org/licenses/MIT",
            Some(OpenLicense::Mit),
        ),
        ("http://scripts.sil.org/", None),
        ("https://www.apache.org/", None),
        ("https://example.com/eula", None),
        ("https://example.com/why-ofl", None),
        ("", None),
    ];
    for (url, expected) in cases {
        assert_eq!(open_license(None, Some(url)), expected, "{url}");
    }
}

#[test]
fn the_description_wins_over_the_url() {
    assert_eq!(
        open_license(
            Some("Licensed under the Apache License, Version 2.0"),
            Some("http://scripts.sil.org/OFL")
        ),
        Some(OpenLicense::Apache)
    );
    assert_eq!(
        open_license(
            Some("All rights reserved."),
            Some("http://scripts.sil.org/OFL")
        ),
        Some(OpenLicense::Ofl),
        "a description with no license leaves the URL to say it"
    );
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

/// The license `manifest.tsv` lists for a fixture.
fn manifest_license(spdx: &str) -> OpenLicense {
    match spdx {
        "OFL-1.1" => OpenLicense::Ofl,
        "Apache-2.0" => OpenLicense::Apache,
        "UFL-1.0" => OpenLicense::Ufl,
        "MIT" => OpenLicense::Mit,
        other => panic!("manifest license {other} has no OpenLicense"),
    }
}

#[test]
fn every_fixture_face_with_license_text_gets_its_manifest_license() {
    let manifest = std::fs::read_to_string(fixtures().join("manifest.tsv")).expect("manifest");
    let mut checked = 0;
    for line in manifest.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        let (spdx, path) = (cols[2], cols[4]);
        let parsed = parser::parse_file(&fixtures().join(path)).expect("read fixture");
        for face in parsed.faces {
            if matches!(face.health, Health::Broken(_)) {
                continue;
            }
            if face.license.is_none() && face.license_url.is_none() {
                continue;
            }
            assert_eq!(
                open_license(face.license.as_deref(), face.license_url.as_deref()),
                Some(manifest_license(spdx)),
                "{path}: {:?} / {:?}",
                face.license,
                face.license_url
            );
            checked += 1;
        }
    }
    assert!(
        checked > 150,
        "only {checked} fixture faces carry license text"
    );
}
