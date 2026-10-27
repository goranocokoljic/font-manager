//! InDesign (.indd) and Illustrator (.ai).
//!
//! Both embed an XMP packet whose `xmpTPg:Fonts` bag lists the document's
//! fonts (`stFnt:fontName`, `stFnt:fontFamily`, `stFnt:fontFace`), written
//! as attributes or as child elements. An Illustrator file is also a PDF:
//! every `/BaseFont /Name` adds a PostScript name, both in plain objects and
//! inside Flate-compressed object streams (`/Type /ObjStm`). An encrypted
//! PDF (`/Encrypt`) is refused.
//!
//! An InDesign file is two 4 KB master pages, the database pages, then
//! contiguous objects (the XMP packet is one), each between a 32-byte header
//! and trailer with fixed GUIDs (Adobe XMP Toolkit, `InDesign_Handler`). The
//! master page with the higher sequence number gives the database page count;
//! a file shorter than its pages, or that ends inside an object, is truncated.
//!
//! The file is scanned in chunks for the markers; only bounded windows around
//! them are kept in memory.

use std::io::{Read, Seek, SeekFrom};

use flate2::read::ZlibDecoder;

use super::xml::{self, Event, attr, local};
use super::{Bad, Fonts, Res, read_capped, xml_text};

/// InDesign's master page GUID, the first 16 bytes of every .indd.
const INDD_GUID: [u8; 16] = [
    0x06, 0x06, 0xED, 0xF5, 0xD8, 0x1D, 0x46, 0xE5, 0xBD, 0x31, 0xEF, 0xE7, 0xFE, 0x74, 0xB7, 0x1D,
];
/// The GUIDs that open and close an InDesign contiguous object.
const INDD_OBJECT_HEADER: [u8; 16] = [
    0xDE, 0x39, 0x39, 0x79, 0x51, 0x88, 0x4B, 0x6C, 0x8E, 0x63, 0xEE, 0xF8, 0xEE, 0xE0, 0xDF, 0xD3,
];
const INDD_OBJECT_TRAILER: [u8; 16] = [
    0xFD, 0xCE, 0xDB, 0x70, 0xF7, 0x86, 0x4B, 0x4F, 0xA4, 0xD3, 0xC7, 0x28, 0xB3, 0x41, 0x71, 0x31,
];
const INDD_PAGE: u64 = 4096;
const XMP_OPEN: &[u8] = b"<x:xmpmeta";
const XMP_OPEN_OLD: &[u8] = b"<x:xapmeta";
const BASE_FONT: &[u8] = b"/BaseFont";
const OBJ_STM: &[u8] = b"/ObjStm";
const ENCRYPT: &[u8] = b"/Encrypt";
/// The most bytes kept for one XMP packet, one object stream (compressed and
/// inflated) and the matches of one scan.
const MAX_XMP: u64 = 16 << 20;
const MAX_STREAM: u64 = 16 << 20;
const MAX_INFLATED: u64 = 64 << 20;
const MAX_MATCHES: usize = 100_000;
const CHUNK: usize = 1 << 20;

pub(super) fn indd<R: Read + Seek>(src: &mut R, fonts: &mut Fonts) -> Res<()> {
    indd_structure(src)?;
    let hits = scan(src, &[XMP_OPEN, XMP_OPEN_OLD])?;
    // InDesign always stores its metadata: without it the document was cut
    // where an object ended.
    if hits.is_empty() {
        return Err(Bad::malformed(
            "the InDesign document has no XMP packet (truncated)",
        ));
    }
    for (_, at) in hits {
        xmp_at(src, at, fonts)?;
    }
    Ok(())
}

/// Checks the master pages and walks the contiguous objects to the end of
/// the file: a truncated document is an error, not an empty font list.
fn indd_structure<R: Read + Seek>(src: &mut R) -> Res<()> {
    let len = src.seek(SeekFrom::End(0))?;
    src.seek(SeekFrom::Start(0))?;
    let mut masters = [0u8; 2 * INDD_PAGE as usize];
    src.read_exact(&mut masters)?;
    let (first, second) = masters.split_at(INDD_PAGE as usize);
    let mut best: Option<(u64, u64)> = None; // (sequence, pages)
    for page in [first, second] {
        if page.get(..16) != Some(&INDD_GUID[..]) || page.get(16..24) != Some(&b"DOCUMENT"[..]) {
            continue;
        }
        // Master page fields are little-endian whatever the object streams use.
        let le = |at: usize, n: usize| {
            page.get(at..at + n)
                .map(|b| b.iter().rev().fold(0u64, |v, &x| v << 8 | u64::from(x)))
                .unwrap_or_default()
        };
        let (sequence, pages) = (le(264, 8), le(280, 4));
        if best.is_none_or(|(s, _)| sequence > s) {
            best = Some((sequence, pages));
        }
    }
    let (_, pages) =
        best.ok_or_else(|| Bad::malformed("not an InDesign document (no master page)"))?;
    let mut at = pages
        .checked_mul(INDD_PAGE)
        .filter(|end| *end >= 2 * INDD_PAGE)
        .ok_or_else(|| Bad::malformed("InDesign page count out of range"))?;
    if at > len {
        return Err(Bad::malformed("the document ends early (truncated)"));
    }
    while at < len {
        src.seek(SeekFrom::Start(at))?;
        let mut header = [0u8; 32];
        let got = read_up_to(src, &mut header)?;
        let n = got.min(16);
        if header[..n] != INDD_OBJECT_HEADER[..n] {
            // Not an object: padding after the last one, as the toolkit allows.
            break;
        }
        if got < 32 {
            return Err(Bad::malformed("the document ends early (truncated)"));
        }
        let stream = u64::from(u32::from_le_bytes([
            header[24], header[25], header[26], header[27],
        ]));
        let trailer_at = at + 32 + stream;
        if trailer_at + 32 > len {
            return Err(Bad::malformed("the document ends early (truncated)"));
        }
        src.seek(SeekFrom::Start(trailer_at))?;
        let mut trailer = [0u8; 16];
        src.read_exact(&mut trailer)?;
        if trailer != INDD_OBJECT_TRAILER {
            return Err(Bad::malformed("InDesign object trailer"));
        }
        at = trailer_at + 32;
    }
    Ok(())
}

pub(super) fn ai<R: Read + Seek>(src: &mut R, fonts: &mut Fonts) -> Res<()> {
    let mut head = [0u8; 10];
    let n = read_up_to(src, &mut head)?;
    let head = &head[..n];
    let pdf = head.starts_with(b"%PDF-");
    if !pdf && !head.starts_with(b"%!PS-Adobe") {
        return Err(Bad::malformed(
            "not an Illustrator document (no PDF or PostScript header)",
        ));
    }
    if pdf {
        // A complete PDF ends with `%%EOF` (an incremental save, too).
        let len = src.seek(SeekFrom::End(0))?;
        let tail_len = len.min(1024);
        src.seek(SeekFrom::Start(len - tail_len))?;
        let tail = read_capped(src, tail_len, "PDF tail")?;
        if !contains(&tail, b"%%EOF") {
            return Err(Bad::malformed("the PDF has no %%EOF (truncated)"));
        }
    }
    let hits = scan(src, &[XMP_OPEN, XMP_OPEN_OLD, BASE_FONT, OBJ_STM, ENCRYPT])?;
    if hits.iter().any(|(k, _)| *k == 4) {
        return Err(Bad::unsupported("the PDF is encrypted"));
    }
    for (kind, at) in hits {
        match kind {
            0 | 1 => xmp_at(src, at, fonts)?,
            2 => {
                src.seek(SeekFrom::Start(at))?;
                let mut window = Vec::new();
                (&mut *src).take(512).read_to_end(&mut window)?;
                base_fonts(&window, fonts);
            }
            _ => object_stream(src, at, fonts)?,
        }
    }
    Ok(())
}

/// Reads from `src` until the end of the first of `markers` (the marker
/// included), the end of the file or `max` bytes; `true` when a marker ended it.
fn read_until(src: &mut impl Read, markers: &[&[u8]], max: u64) -> Res<(Vec<u8>, bool)> {
    let longest = markers.iter().map(|m| m.len()).max().unwrap_or(0);
    let mut buf = Vec::new();
    let mut chunk = vec![0u8; 64 << 10];
    while (buf.len() as u64) < max {
        let n = src.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        let from = buf.len().saturating_sub(longest);
        buf.extend_from_slice(&chunk[..n]);
        let end = markers
            .iter()
            .filter_map(|m| find(&buf[from..], m).map(|i| from + i + m.len()))
            .min();
        if let Some(end) = end {
            buf.truncate(end);
            return Ok((buf, true));
        }
    }
    Ok((buf, false))
}

fn read_up_to(src: &mut impl Read, buf: &mut [u8]) -> Res<usize> {
    let mut n = 0;
    while n < buf.len() {
        let got = src.read(&mut buf[n..])?;
        if got == 0 {
            break;
        }
        n += got;
    }
    Ok(n)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    find(hay, needle).is_some()
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Every offset of every needle in the file, by needle index, in file order.
fn scan<R: Read + Seek>(src: &mut R, needles: &[&[u8]]) -> Res<Vec<(usize, u64)>> {
    let keep = needles.iter().map(|n| n.len()).max().unwrap_or(1) - 1;
    src.seek(SeekFrom::Start(0))?;
    let mut hits = Vec::new();
    let mut buf: Vec<u8> = Vec::with_capacity(CHUNK + keep);
    let mut base = 0u64; // file offset of buf[0]
    let mut chunk = vec![0u8; CHUNK];
    loop {
        let n = src.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        for (i, w) in buf.windows(keep + 1).enumerate() {
            for (k, needle) in needles.iter().enumerate() {
                if w.starts_with(needle) {
                    hits.push((k, base + i as u64));
                }
            }
        }
        // Keep the last `keep` bytes so a needle across chunks is found once:
        // a window starts at most at len - keep - 1, the next pass at len - keep.
        let cut = buf.len().saturating_sub(keep);
        base += cut as u64;
        buf.drain(..cut);
        if hits.len() > MAX_MATCHES {
            return Err(Bad::malformed("too many font markers"));
        }
    }
    // Needles shorter than the window may sit in the last `keep` bytes.
    for i in 0..buf.len() {
        for (k, needle) in needles.iter().enumerate() {
            if buf[i..].starts_with(needle) {
                hits.push((k, base + i as u64));
            }
        }
    }
    hits.sort_by_key(|(_, at)| *at);
    Ok(hits)
}

/// The XMP packet starting at `at`, up to its closing tag.
fn xmp_at<R: Read + Seek>(src: &mut R, at: u64, fonts: &mut Fonts) -> Res<()> {
    src.seek(SeekFrom::Start(at))?;
    let (packet, closed) = read_until(src, &[b"</x:xmpmeta>", b"</x:xapmeta>"], MAX_XMP)?;
    if !closed {
        return Err(Bad::malformed("XMP packet is not closed (truncated)"));
    }
    xmp_fonts(&xml_text(&packet)?, fonts)
}

/// The fonts of the `xmpTPg:Fonts` bag of an XMP packet.
pub(super) fn xmp_fonts(text: &str, fonts: &mut Fonts) -> Res<()> {
    let mut in_fonts = 0u32;
    // The `rdf:li` being read: its parts, and the element whose text is next.
    let mut item: Option<[Option<String>; 3]> = None;
    let mut field: Option<usize> = None;
    let slot = |n: &str| match n {
        "fontName" => Some(0),
        "fontFamily" => Some(1),
        "fontFace" => Some(2),
        _ => None,
    };
    for ev in xml::parse(text)? {
        match ev {
            Event::Start { name, attrs, empty } => {
                let l = local(name);
                if l == "Fonts" && !empty {
                    in_fonts += 1;
                } else if in_fonts > 0 && l == "li" {
                    let mut parts: [Option<String>; 3] = Default::default();
                    for key in ["fontName", "fontFamily", "fontFace"] {
                        if let (Some(i), Some(v)) = (slot(key), attr(&attrs, key)) {
                            parts[i] = Some(v.to_owned());
                        }
                    }
                    if empty {
                        add(fonts, &parts);
                    } else {
                        item = Some(parts);
                    }
                } else if item.is_some() && !empty {
                    field = slot(l);
                }
            }
            Event::Text(t) => {
                if let (Some(i), Some(parts)) = (field, item.as_mut()) {
                    parts[i].get_or_insert_with(String::new).push_str(&t);
                }
            }
            Event::End { name } => match local(name) {
                "Fonts" => in_fonts = in_fonts.saturating_sub(1),
                "li" if in_fonts > 0 => {
                    if let Some(parts) = item.take() {
                        add(fonts, &parts);
                    }
                    field = None;
                }
                _ => field = None,
            },
        }
    }
    Ok(())
}

fn add(fonts: &mut Fonts, [ps, family, face]: &[Option<String>; 3]) {
    fonts.add(ps.as_deref(), family.as_deref(), face.as_deref());
}

/// The names after every `/BaseFont` in `bytes`.
fn base_fonts(bytes: &[u8], fonts: &mut Fonts) {
    let mut rest = bytes;
    while let Some(i) = find(rest, BASE_FONT) {
        rest = &rest[i + BASE_FONT.len()..];
        let r = rest
            .iter()
            .position(|c| !is_pdf_space(*c))
            .map_or(&[][..], |s| &rest[s..]);
        if let Some(name) = r.strip_prefix(b"/") {
            let end = name
                .iter()
                .position(|c| is_pdf_delimiter(*c))
                .unwrap_or(name.len());
            let name = pdf_name(&name[..end]);
            fonts.postscript(strip_subset(&name));
        }
    }
}

fn is_pdf_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | 0)
}

fn is_pdf_delimiter(c: u8) -> bool {
    is_pdf_space(c) || b"()<>[]{}/%".contains(&c)
}

/// A PDF name with its `#xx` escapes decoded.
fn pdf_name(raw: &[u8]) -> String {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let hex = raw
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (raw[i], hex) {
            (b'#', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `ABCDEF+Minion-Regular` → `Minion-Regular`: the tag a PDF writer puts in
/// front of a subset font is not part of the font's name.
fn strip_subset(name: &str) -> &str {
    match name.split_once('+') {
        Some((tag, rest)) if tag.len() == 6 && tag.bytes().all(|b| b.is_ascii_uppercase()) => rest,
        _ => name,
    }
}

/// The object stream whose dictionary names `/ObjStm` at `at`: its data up
/// to `endstream`, inflated, then searched for `/BaseFont`. A stream that is
/// not Flate-encoded is skipped.
fn object_stream<R: Read + Seek>(src: &mut R, at: u64, fonts: &mut Fonts) -> Res<()> {
    src.seek(SeekFrom::Start(at))?;
    let (window, _) = read_until(src, &[b"endstream"], MAX_STREAM)?;
    let Some(s) = find(&window[..window.len().min(4096)], b"stream") else {
        return Ok(());
    };
    let mut start = s + b"stream".len();
    if window.get(start) == Some(&b'\r') {
        start += 1;
    }
    if window.get(start) == Some(&b'\n') {
        start += 1;
    }
    let data = window.get(start..).unwrap_or_default();
    let end = find(data, b"endstream").unwrap_or(data.len());
    if let Ok(inflated) = read_capped(
        &mut ZlibDecoder::new(&data[..end]),
        MAX_INFLATED,
        "object stream",
    ) {
        base_fonts(&inflated, fonts);
    }
    Ok(())
}
