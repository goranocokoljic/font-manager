//! Sketch (.sketch): a zip with one JSON file per page (`pages/<id>.json`).
//! A text layer's runs and its text style carry
//! `MSAttributedStringFontAttribute.attributes.name`, the PostScript name.

use std::io::{Read, Seek};

use serde_json::Value;

use super::zip::Zip;
use super::{Bad, Fonts, Res};

const FONT_ATTRIBUTE: &str = "MSAttributedStringFontAttribute";

pub(super) fn read<R: Read + Seek>(src: &mut R, fonts: &mut Fonts) -> Res<()> {
    let mut zip = Zip::open(src)?;
    let pages: Vec<String> = zip
        .names()
        .filter(|n| {
            n.strip_prefix("pages/")
                .is_some_and(|f| f.ends_with(".json") && !f.contains('/'))
        })
        .map(str::to_owned)
        .collect();
    if zip.read("document.json")?.is_none() {
        return Err(Bad::malformed("not a Sketch document (no document.json)"));
    }
    for page in pages {
        let Some(bytes) = zip.read(&page)? else {
            continue;
        };
        let json: Value =
            serde_json::from_slice(&bytes).map_err(|e| Bad::malformed(format!("{page}: {e}")))?;
        collect(&json, fonts);
    }
    Ok(())
}

/// Walks the tree without recursion (serde_json already bounds the depth).
fn collect(root: &Value, fonts: &mut Fonts) {
    let mut stack = vec![root];
    while let Some(v) = stack.pop() {
        match v {
            Value::Object(map) => {
                if let Some(name) = map
                    .get(FONT_ATTRIBUTE)
                    .and_then(|f| f.get("attributes"))
                    .and_then(|a| a.get("name"))
                    .and_then(Value::as_str)
                {
                    fonts.postscript(name);
                }
                stack.extend(map.values());
            }
            Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
}
