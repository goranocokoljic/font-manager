//! Word (.docx) and PowerPoint (.pptx): Office Open XML zip packages. Their
//! font names are family names.
//!
//! - Word: every `w:font` of `word/fontTable.xml`, and the explicit faces of
//!   every `w:rFonts` (`w:ascii`, `w:hAnsi`, `w:eastAsia`, `w:cs`) in the
//!   other `word/` parts (document, styles, numbering, headers, notes).
//! - PowerPoint: `a:latin`, `a:ea` and `a:cs` in slides, layouts, masters and
//!   notes. A `+mj-lt` / `+mn-ea` style placeholder is resolved to the major
//!   or minor font of the theme the part uses: slide → layout → master →
//!   theme, through the parts' relationships.

use std::collections::HashMap;
use std::io::{Read, Seek};

use super::xml::{self, Event, attr, local};
use super::zip::Zip;
use super::{Bad, Fonts, Res, xml_text};

/// An OLE compound file: what an encrypted (password) Office file is.
const OLE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

fn open<R: Read + Seek>(src: &mut R) -> Res<Zip<'_, R>> {
    let mut magic = [0u8; 8];
    let n = src.read(&mut magic)?;
    if n == 8 && magic == OLE {
        return Err(Bad::unsupported(
            "an OLE compound file: the Office document is password-protected",
        ));
    }
    Zip::open(src)
}

fn part<R: Read + Seek>(zip: &mut Zip<'_, R>, name: &str) -> Res<Option<String>> {
    zip.read(name)?.map(|b| xml_text(&b)).transpose()
}

pub(super) fn docx<R: Read + Seek>(src: &mut R, fonts: &mut Fonts) -> Res<()> {
    let mut zip = open(src)?;
    let names: Vec<String> = zip
        .names()
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("word/") && n.ends_with(".xml") && !n.starts_with("word/theme/")
        })
        .map(str::to_owned)
        .collect();
    if !names
        .iter()
        .any(|n| n.eq_ignore_ascii_case("word/document.xml"))
    {
        return Err(Bad::malformed("not a Word document (no word/document.xml)"));
    }
    for name in names {
        let Some(text) = part(&mut zip, &name)? else {
            continue;
        };
        if name.eq_ignore_ascii_case("word/fontTable.xml") {
            table_fonts(&text, fonts)?;
        } else {
            run_fonts(&text, fonts)?;
        }
    }
    Ok(())
}

fn table_fonts(text: &str, fonts: &mut Fonts) -> Res<()> {
    for ev in xml::parse(text)? {
        if let Event::Start { name, attrs, .. } = ev
            && local(name) == "font"
            && let Some(n) = attr(&attrs, "name")
        {
            fonts.family(n);
        }
    }
    Ok(())
}

fn run_fonts(text: &str, fonts: &mut Fonts) -> Res<()> {
    for ev in xml::parse(text)? {
        if let Event::Start { name, attrs, .. } = ev
            && local(name) == "rFonts"
        {
            for key in ["ascii", "hAnsi", "eastAsia", "cs"] {
                if let Some(n) = attr(&attrs, key) {
                    fonts.family(n);
                }
            }
        }
    }
    Ok(())
}

/// The folders whose parts carry text runs.
const PPT_PARTS: [&str; 5] = [
    "ppt/slides/",
    "ppt/slideLayouts/",
    "ppt/slideMasters/",
    "ppt/notesSlides/",
    "ppt/notesMasters/",
];

pub(super) fn pptx<R: Read + Seek>(src: &mut R, fonts: &mut Fonts) -> Res<()> {
    let mut zip = open(src)?;
    if zip.read("ppt/presentation.xml")?.is_none() {
        return Err(Bad::malformed(
            "not a PowerPoint document (no ppt/presentation.xml)",
        ));
    }
    let parts: Vec<String> = zip
        .names()
        .filter(|n| {
            let lower = n.to_ascii_lowercase();
            lower.ends_with(".xml")
                && PPT_PARTS.iter().any(|dir| {
                    lower
                        .strip_prefix(&dir.to_ascii_lowercase())
                        .is_some_and(|f| !f.contains('/'))
                })
        })
        .map(str::to_owned)
        .collect();
    let mut themes: HashMap<String, Theme> = HashMap::new();
    for name in parts {
        let Some(text) = part(&mut zip, &name)? else {
            continue;
        };
        let mut theme: Option<Option<String>> = None;
        for ev in xml::parse(&text)? {
            let Event::Start {
                name: tag, attrs, ..
            } = ev
            else {
                continue;
            };
            if !matches!(local(tag), "latin" | "ea" | "cs") {
                continue;
            }
            let Some(face) = attr(&attrs, "typeface") else {
                continue;
            };
            let Some(placeholder) = face.strip_prefix('+') else {
                fonts.family(face);
                continue;
            };
            // `+mj-lt`: the theme's major Latin font, and so on.
            let theme_path = match &theme {
                Some(t) => t.clone(),
                None => {
                    let t = theme_of(&mut zip, &name, 0)?;
                    theme = Some(t.clone());
                    t
                }
            };
            let Some(theme_path) = theme_path else {
                continue;
            };
            if !themes.contains_key(&theme_path) {
                let t = match part(&mut zip, &theme_path)? {
                    Some(text) => Theme::parse(&text)?,
                    None => Theme::default(),
                };
                themes.insert(theme_path.clone(), t);
            }
            if let Some(f) = themes.get(&theme_path).and_then(|t| t.get(placeholder)) {
                fonts.family(f);
            }
        }
    }
    Ok(())
}

/// The major and minor fonts of a theme, by placeholder (`mj-lt`, `mn-ea`).
#[derive(Debug, Default)]
struct Theme(HashMap<String, String>);

impl Theme {
    fn parse(text: &str) -> Res<Self> {
        let mut map = HashMap::new();
        let mut group: Option<&str> = None;
        for ev in xml::parse(text)? {
            match ev {
                Event::Start { name, attrs, empty } => match (local(name), group) {
                    ("majorFont", _) if !empty => group = Some("mj"),
                    ("minorFont", _) if !empty => group = Some("mn"),
                    (slot @ ("latin" | "ea" | "cs"), Some(g)) => {
                        let slot = if slot == "latin" { "lt" } else { slot };
                        if let Some(face) = attr(&attrs, "typeface") {
                            map.entry(format!("{g}-{slot}"))
                                .or_insert_with(|| face.to_owned());
                        }
                    }
                    _ => {}
                },
                Event::End { name } if matches!(local(name), "majorFont" | "minorFont") => {
                    group = None;
                }
                _ => {}
            }
        }
        Ok(Self(map))
    }

    fn get(&self, placeholder: &str) -> Option<&str> {
        self.0.get(placeholder).map(String::as_str)
    }
}

/// The theme part `part` uses: its own theme relationship, or that of the
/// layout or master it relates to (at most three steps up).
fn theme_of<R: Read + Seek>(zip: &mut Zip<'_, R>, part: &str, depth: u8) -> Res<Option<String>> {
    if depth > 3 {
        return Ok(None);
    }
    let rels = relationships(zip, part)?;
    if let Some((_, t)) = rels.iter().find(|(ty, _)| ty == "theme") {
        return Ok(Some(t.clone()));
    }
    for up in ["slideLayout", "slideMaster", "notesMaster"] {
        if let Some((_, t)) = rels.iter().find(|(ty, _)| ty == up) {
            return theme_of(zip, t, depth + 1);
        }
    }
    Ok(None)
}

/// The internal relationships of `part`: (last segment of the type, the
/// target as a package path).
fn relationships<R: Read + Seek>(zip: &mut Zip<'_, R>, part: &str) -> Res<Vec<(String, String)>> {
    let (dir, file) = part.rsplit_once('/').unwrap_or(("", part));
    let rels_path = if dir.is_empty() {
        format!("_rels/{file}.rels")
    } else {
        format!("{dir}/_rels/{file}.rels")
    };
    let Some(text) = self::part(zip, &rels_path)? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for ev in xml::parse(&text)? {
        let Event::Start { name, attrs, .. } = ev else {
            continue;
        };
        if local(name) != "Relationship" || attr(&attrs, "TargetMode") == Some("External") {
            continue;
        }
        let (Some(ty), Some(target)) = (attr(&attrs, "Type"), attr(&attrs, "Target")) else {
            continue;
        };
        let ty = ty.rsplit('/').next().unwrap_or(ty).to_owned();
        out.push((ty, resolve(dir, target)));
    }
    Ok(out)
}

/// `target` relative to the folder `dir` (or absolute from the package root),
/// with `.` and `..` segments applied.
fn resolve(dir: &str, target: &str) -> String {
    let mut segs: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        dir.split('/').filter(|s| !s.is_empty()).collect()
    };
    for s in target.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    segs.join("/")
}
