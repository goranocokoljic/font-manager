use super::*;

/// The cells of `csv`, read back as RFC 4180 says: a quoted cell may hold
/// commas, line breaks and doubled quotes; records end in CRLF. Panics on
/// anything else, so a test sees a malformed file.
pub(crate) fn parse(csv: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut cell = String::new();
    let mut chars = csv.chars().peekable();
    let mut at_start = true;
    while let Some(c) = chars.next() {
        match c {
            '"' if at_start => {
                loop {
                    match chars.next() {
                        Some('"') if chars.peek() == Some(&'"') => {
                            chars.next();
                            cell.push('"');
                        }
                        Some('"') => break,
                        Some(c) => cell.push(c),
                        None => panic!("unterminated quote in {csv:?}"),
                    }
                }
                assert!(
                    matches!(chars.peek(), Some(',' | '\r') | None),
                    "text after a closing quote in {csv:?}"
                );
                at_start = false;
            }
            '"' => panic!("a bare quote in an unquoted cell in {csv:?}"),
            ',' => {
                record.push(std::mem::take(&mut cell));
                at_start = true;
            }
            '\r' => {
                assert_eq!(chars.next(), Some('\n'), "a bare CR in {csv:?}");
                record.push(std::mem::take(&mut cell));
                records.push(std::mem::take(&mut record));
                at_start = true;
            }
            '\n' => panic!("a bare LF in an unquoted cell in {csv:?}"),
            c => {
                cell.push(c);
                at_start = false;
            }
        }
    }
    assert!(
        record.is_empty() && cell.is_empty(),
        "the last record does not end in CRLF: {csv:?}"
    );
    records
}

fn row(family: &str) -> InventoryRow {
    InventoryRow {
        family: family.into(),
        ..InventoryRow::default()
    }
}

#[test]
fn the_header_comes_first_and_every_row_has_every_column() {
    let full = InventoryRow {
        family: "Barlow".into(),
        foundry: Some("Tribby Type".into()),
        styles: 18,
        formats: vec![FontFormat::Ttf, FontFormat::Woff2],
        declared: Some(LicenseType::PersonalOnly),
        seats: Some(3),
        purchase_date: Some("2026-03-01".into()),
        receipt: Some("/Fonts/.fontlibrary/receipts/invoice.pdf".into()),
        open_license: Some(OpenLicense::Ofl),
        embedding: vec![Embedding::Installable, Embedding::Restricted],
        license_url: Some("https://openfontlicense.org".into()),
        source_folder: Some("/Fonts".into()),
    };
    let inventory = to_csv(&[full, row("Empty")]);
    assert_eq!(inventory.rows, 2);
    let records = parse(&inventory.csv);
    assert_eq!(records[0], HEADER);
    assert_eq!(
        records[1],
        [
            "Barlow",
            "Tribby Type",
            "18",
            "TTF; WOFF2",
            "Personal use only",
            "3",
            "2026-03-01",
            "/Fonts/.fontlibrary/receipts/invoice.pdf",
            "SIL Open Font License",
            "Installable; Restricted",
            "https://openfontlicense.org",
            "/Fonts",
        ]
    );
    assert_eq!(
        records[2],
        ["Empty", "", "0", "", "", "", "", "", "", "", "", ""]
    );
    assert_eq!(records.len(), 3);
}

#[test]
fn an_empty_inventory_is_the_header_alone() {
    let inventory = to_csv(&[]);
    assert_eq!(inventory.rows, 0);
    assert_eq!(parse(&inventory.csv), [HEADER]);
}

#[test]
fn commas_quotes_and_line_breaks_are_quoted_per_rfc_4180() {
    let tricky = [
        "Acme, Inc.",
        "The \"Pro\" cut",
        "two\nlines",
        "two\r\nlines",
        "lone\rcarriage",
        "\"",
        ",",
        "",
    ];
    let rows: Vec<InventoryRow> = tricky
        .iter()
        .map(|t| InventoryRow {
            foundry: Some((*t).into()),
            ..row("F")
        })
        .collect();
    let records = parse(&to_csv(&rows).csv);
    for (record, want) in records[1..].iter().zip(tricky) {
        assert_eq!(record[1], want);
    }
    assert_eq!(cell("Acme, Inc."), "\"Acme, Inc.\"");
    assert_eq!(cell("The \"Pro\" cut"), "\"The \"\"Pro\"\" cut\"");
    assert_eq!(cell("plain"), "plain");
}

#[test]
fn a_cell_a_spreadsheet_would_run_as_a_formula_is_prefixed() {
    for formula in [
        "=HYPERLINK(\"http://evil\",\"x\")",
        "+1+1",
        "-2+3",
        "@SUM(A1)",
        "\tTab",
        "\rCR",
    ] {
        let records = parse(&to_csv(&[row(formula)]).csv);
        assert_eq!(records[1][0], format!("'{formula}"), "{formula:?}");
    }
    // The formula characters anywhere else, and a quote, are left alone.
    for safe in ["A=B", "Acme+", "Semi-Bold", "me@foundry", "'quoted"] {
        assert_eq!(cell(safe), safe);
    }
    // A defused formula that also needs quotes is defused, then quoted.
    assert_eq!(cell("=1,2"), "\"'=1,2\"");
}

#[test]
fn save_writes_the_file_the_user_picked_and_reports_a_folder_that_is_not_there() {
    let dir = crate::test_support::TempDir::new("inventory-save");
    let path = dir.path().join("inventory, 2026.csv");
    std::fs::write(&path, "old").expect("old file");
    save(&path, "Family\r\n").expect("saved");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "Family\r\n");

    let missing = dir.path().join("no such folder").join("inventory.csv");
    let err = save(&missing, "x").expect_err("no folder");
    assert_eq!(err.code(), fm_api::ErrorCode::NotFound);
    assert_eq!(
        err.message(),
        "The license inventory could not be saved there."
    );
    assert!(err.detail().is_some_and(|d| d.contains("no such folder")));
}
