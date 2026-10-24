//! Framework snippets (E20.5): a Tailwind `fontFamily` entry, CSS custom
//! properties or a Next.js `localFont` call for the families of some faces.
//! Built here, never in an adapter; nothing is written.
//!
//! - **Families**: faces are grouped by their typographic family (as in
//!   `fonts.css`, [`crate::web_css`]), families in request order. Within a
//!   family faces are listed lightest first, upright before italic.
//! - **Key**: the family in kebab case (`Open Sans` → `open-sans`): letters
//!   and digits lowercased, every other run one `-`; `font` when nothing is
//!   left. Two families with the same key get `-2`, `-3`, … in order.
//! - **Fallback**: the generic family of the first face with a design class:
//!   serif and slab are `serif`, mono `monospace`, script `cursive`, every
//!   other class and an unknown one `sans-serif`.
//! - **Next.js**: one `localFont` call per family, exported as `font` plus the
//!   family's ASCII letters and digits in Pascal case (`fontOpenSans`), so the
//!   name is always a JS identifier and never a reserved word. Each face is a
//!   `src` entry at `<fonts_path>/<file stem>.woff2` (the E20.1 name), with the
//!   `font-weight` and `font-style` of its `@font-face` block.
//! - **Escaping**: every snippet is ASCII. A JS string is single-quoted with
//!   `\` and `'` escaped and every control or non-ASCII character as
//!   `\uXXXX`; a CSS string or name escapes `"` and `\`, and a control or
//!   non-ASCII character is a hex escape followed by a space.

use std::collections::HashSet;

use fm_api::common::{Classification, Slant};
use fm_api::export::Framework;

use crate::web_css::{self, FaceStyle};

/// The fonts folder of a Next.js snippet when the request names none.
pub const DEFAULT_FONTS_PATH: &str = "./fonts";

/// A face of a snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct SnippetFace {
    /// Family, slant, weight and axes, as for `@font-face`.
    pub style: FaceStyle,
    /// The web font's file name without extension
    /// ([`crate::web_font::file_stem`]).
    pub file_stem: String,
    /// The design class, when known.
    pub classification: Option<Classification>,
}

/// One family of a snippet.
struct Family<'a> {
    name: &'a str,
    key: String,
    fallback: &'static str,
    faces: Vec<&'a SnippetFace>,
}

/// The snippet for `framework` over `faces`. `fonts_path` is where a Next.js
/// project keeps the web fonts; [`DEFAULT_FONTS_PATH`] when absent or blank.
pub fn snippet(framework: Framework, faces: &[SnippetFace], fonts_path: Option<&str>) -> String {
    let families = families(faces);
    match framework {
        Framework::Tailwind => tailwind(&families),
        Framework::CssVariables => css_variables(&families),
        Framework::NextLocalFont => next_local_font(&families, fonts_path),
    }
}

/// The generic CSS family that stands in for a face of `class`.
pub fn generic_fallback(class: Option<Classification>) -> &'static str {
    match class {
        Some(Classification::Serif | Classification::Slab) => "serif",
        Some(Classification::Mono) => "monospace",
        Some(Classification::Script) => "cursive",
        Some(Classification::Sans | Classification::Display | Classification::Symbol) | None => {
            "sans-serif"
        }
    }
}

/// `name` in kebab case: letters and digits lowercased, every other run of
/// characters one `-`, none at either end; `font` when nothing is left.
pub fn kebab(name: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.extend(c.to_lowercase());
        } else {
            gap = true;
        }
    }
    if out.is_empty() {
        "font".to_owned()
    } else {
        out
    }
}

/// `s` as a single-quoted JS string, ASCII only.
pub fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\\' | '\'' => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            c => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
        }
    }
    out.push('\'');
    out
}

/// `s` as a double-quoted CSS string, ASCII only.
pub fn css_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c => push_css_char(&mut out, c),
        }
    }
    out.push('"');
    out
}

/// `--font-<key>` as written in CSS, ASCII only. A key holds only letters,
/// digits and `-` ([`kebab`]), so only non-ASCII letters need escaping.
fn css_property(key: &str) -> String {
    let mut out = String::from("--font-");
    for c in key.chars() {
        push_css_char(&mut out, c);
    }
    out
}

/// `c`, or its hex escape and a space when it is a control or non-ASCII
/// character.
fn push_css_char(out: &mut String, c: char) {
    if c.is_ascii() && !c.is_ascii_control() {
        out.push(c);
    } else {
        out.push_str(&format!("\\{:X} ", u32::from(c)));
    }
}

/// The families of `faces` in request order, each with a unique key, its
/// fallback and its faces lightest first, upright before italic.
fn families(faces: &[SnippetFace]) -> Vec<Family<'_>> {
    let mut out: Vec<Family<'_>> = Vec::new();
    for face in faces {
        match out.iter_mut().find(|f| f.name == face.style.family) {
            Some(f) => f.faces.push(face),
            None => out.push(Family {
                name: &face.style.family,
                key: String::new(),
                fallback: "",
                faces: vec![face],
            }),
        }
    }
    let mut keys = HashSet::new();
    for family in &mut out {
        family.faces.sort_by(|a, b| {
            lightest(&a.style)
                .total_cmp(&lightest(&b.style))
                .then(slant_rank(a.style.slant).cmp(&slant_rank(b.style.slant)))
        });
        family.key = unique(kebab(family.name), &mut keys, "-");
        family.fallback = generic_fallback(family.faces.iter().find_map(|f| f.classification));
    }
    out
}

/// The lightest weight of `style`: its `wght` minimum, else its weight class.
fn lightest(style: &FaceStyle) -> f32 {
    style
        .axes
        .iter()
        .find(|a| a.tag == "wght" && a.min.is_finite() && a.max.is_finite())
        .map_or(f32::from(style.weight), |a| a.min.min(a.max))
}

fn slant_rank(slant: Slant) -> u8 {
    match slant {
        Slant::Upright => 0,
        Slant::Italic => 1,
        Slant::Oblique => 2,
    }
}

/// `name`, or `name<sep>2`, `name<sep>3`, … when it is taken; marked taken.
fn unique(name: String, taken: &mut HashSet<String>, sep: &str) -> String {
    let mut candidate = name.clone();
    let mut n = 2;
    while !taken.insert(candidate.clone()) {
        candidate = format!("{name}{sep}{n}");
        n += 1;
    }
    candidate
}

fn tailwind(families: &[Family<'_>]) -> String {
    let mut out = String::from(
        "// tailwind.config.js\nexport default {\n  theme: {\n    extend: {\n      fontFamily: {\n",
    );
    for f in families {
        out.push_str(&format!(
            "        {}: [{}, {}],\n",
            js_string(&f.key),
            js_string(&css_string(f.name)),
            js_string(f.fallback),
        ));
    }
    out.push_str("      },\n    },\n  },\n};\n");
    out
}

fn css_variables(families: &[Family<'_>]) -> String {
    let mut out = String::from(":root {\n");
    for f in families {
        out.push_str(&format!(
            "  {}: {}, {};\n",
            css_property(&f.key),
            css_string(f.name),
            f.fallback,
        ));
    }
    out.push_str("}\n");
    out
}

fn next_local_font(families: &[Family<'_>], fonts_path: Option<&str>) -> String {
    let base = match fonts_path.map(str::trim) {
        Some(p) if !p.is_empty() => p.trim_end_matches(['/', '\\']),
        _ => DEFAULT_FONTS_PATH,
    };
    let mut names = HashSet::new();
    let mut out = String::from("import localFont from 'next/font/local';\n");
    for f in families {
        let name = unique(identifier(f.name), &mut names, "");
        out.push_str(&format!("\nexport const {name} = localFont({{\n  src: [\n"));
        for face in &f.faces {
            out.push_str(&format!(
                "    {{ path: {}, weight: {}, style: {} }},\n",
                js_string(&format!("{base}/{}.woff2", face.file_stem)),
                js_string(&web_css::font_weight(&face.style)),
                js_string(&web_css::font_style(&face.style)),
            ));
        }
        out.push_str(&format!(
            "  ],\n  variable: {},\n  display: 'swap',\n}});\n",
            js_string(&css_name(&f.key)),
        ));
    }
    out
}

/// The custom property `--font-<key>` as a plain string, for JS.
fn css_name(key: &str) -> String {
    format!("--font-{key}")
}

/// `font` and the ASCII words of `family` in Pascal case: `fontOpenSans`.
fn identifier(family: &str) -> String {
    let mut out = String::from("font");
    for word in family
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

#[cfg(test)]
mod tests;
