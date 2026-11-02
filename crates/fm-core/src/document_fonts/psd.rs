//! Photoshop (.psd, and .psb with its 64-bit lengths). The header, colour
//! mode data and image resources are skipped; the layer records are walked
//! (also those in the `Lr16` / `Lr32` / `Layr` blocks of 16- and 32-bit
//! documents). A text layer has a `TySh` block whose engine data lists
//! fonts as `/FontSet [ << /Name (…) >> … ]` with PostScript names in UTF-16.
//! The `FontSet` also holds Photoshop's defaults (Myriad Pro, found on a real
//! file in E32.4), so the reader lists only the fonts the text runs use:
//! `/EngineDict /StyleRun /RunArray`, each run's `/StyleSheetData /Font <n>`
//! or the default style sheet's, `n` indexing the `FontSet`. Engine data it
//! cannot follow gives every `FontSet` name. Channel image data is seeked
//! over, never read.
//!
//! Block lengths are the data's own; the data is padded to 2 bytes in a
//! layer record (the specification) and to 4 in the global list after the
//! layer info (what Photoshop 2025 writes, found on a real file in E36.6).
//! The walk reads the length, then looks for the next signature at the
//! 2-byte boundary and, when none is there, at the 4-byte one.

use std::io::{Read, Seek, SeekFrom};

use super::{Bad, Fonts, Res, read_exact_capped};

/// The most bytes read for one `TySh` block.
const MAX_TYPE_TOOL: u64 = 16 << 20;
/// Photoshop's placeholder font, in every FontSet; not a font to install.
const INVISIBLE: &str = "AdobeInvisFont";
/// Blocks with an 8-byte length in a .psb.
const LONG_KEYS: [&[u8; 4]; 13] = [
    b"LMsk", b"Lr16", b"Lr32", b"Layr", b"Mt16", b"Mt32", b"Mtrn", b"Alph", b"FMsk", b"lnk2",
    b"FEid", b"FXid", b"PxSD",
];

struct Psd<'a, R> {
    src: &'a mut R,
    big: bool,
    len: u64,
}

impl<R: Read + Seek> Psd<'_, R> {
    fn bytes<const N: usize>(&mut self) -> Res<[u8; N]> {
        let mut b = [0u8; N];
        self.src.read_exact(&mut b)?;
        Ok(b)
    }

    fn u16(&mut self) -> Res<u16> {
        Ok(u16::from_be_bytes(self.bytes()?))
    }

    fn u32(&mut self) -> Res<u32> {
        Ok(u32::from_be_bytes(self.bytes()?))
    }

    /// A section length: 4 bytes, or 8 in a .psb when `long`.
    fn length(&mut self, long: bool) -> Res<u64> {
        if self.big && long {
            Ok(u64::from_be_bytes(self.bytes()?))
        } else {
            Ok(u64::from(self.u32()?))
        }
    }

    fn pos(&mut self) -> Res<u64> {
        Ok(self.src.stream_position()?)
    }

    /// The end of a block of `len` bytes starting here, inside the file.
    fn end(&mut self, len: u64) -> Res<u64> {
        let end = self
            .pos()?
            .checked_add(len)
            .ok_or_else(|| Bad::malformed("Photoshop length overflows"))?;
        if end > self.len {
            return Err(Bad::malformed("the document ends early (truncated)"));
        }
        Ok(end)
    }

    fn skip_block(&mut self) -> Res<()> {
        let len = u64::from(self.u32()?);
        let end = self.end(len)?;
        self.src.seek(SeekFrom::Start(end))?;
        Ok(())
    }

    /// The layer count and records from here to `end`.
    fn layer_info(&mut self, end: u64, fonts: &mut Fonts) -> Res<()> {
        if self.pos()? >= end {
            return Ok(());
        }
        let count = i16::from_be_bytes(self.bytes()?).unsigned_abs();
        for _ in 0..count {
            self.layer_record(fonts)?;
        }
        Ok(())
    }

    fn layer_record(&mut self, fonts: &mut Fonts) -> Res<()> {
        self.bytes::<16>()?; // bounds
        let channels = self.u16()?;
        for _ in 0..channels {
            self.u16()?; // channel id
            self.length(true)?;
        }
        if &self.bytes::<4>()? != b"8BIM" {
            return Err(Bad::malformed("Photoshop layer record signature"));
        }
        self.bytes::<8>()?; // blend mode, opacity, clipping, flags, filler
        let extra = u64::from(self.u32()?);
        let extra_end = self.end(extra)?;
        self.skip_block()?; // layer mask
        self.skip_block()?; // blending ranges
        let name_len = u64::from(self.bytes::<1>()?[0]);
        // A Pascal string padded to a multiple of 4, its length byte included.
        let padded = (name_len + 1).div_ceil(4) * 4 - 1;
        let at = self.end(padded)?;
        self.src.seek(SeekFrom::Start(at))?;
        self.tagged_blocks(extra_end, false, fonts)?;
        self.src.seek(SeekFrom::Start(extra_end))?;
        Ok(())
    }

    /// Additional layer information blocks up to `end`. Only the global
    /// blocks (`global`) may hold more layers, so nesting stays one deep.
    fn tagged_blocks(&mut self, end: u64, global: bool, fonts: &mut Fonts) -> Res<()> {
        while self.pos()? + 12 <= end {
            let sig = self.bytes::<4>()?;
            if &sig != b"8BIM" && &sig != b"8B64" {
                return Err(Bad::malformed("Photoshop block signature"));
            }
            let key = self.bytes::<4>()?;
            let len = self.length(LONG_KEYS.contains(&&key))?;
            let block_end = self.end(len)?;
            if block_end > end {
                return Err(Bad::malformed("Photoshop block runs past its section"));
            }
            match &key {
                b"TySh" => {
                    let data = read_exact_capped(self.src, len, MAX_TYPE_TOOL, "TySh block")?;
                    text_fonts(&data, fonts);
                }
                b"Lr16" | b"Lr32" | b"Layr" if global => self.layer_info(block_end, fonts)?,
                _ => {}
            }
            let next = self.next_block(block_end, len, end)?;
            self.src.seek(SeekFrom::Start(next))?;
        }
        Ok(())
    }

    /// Where the block after one of `len` bytes ending at `block_end`
    /// starts: the data padded to 2 bytes, or to 4 when a signature sits
    /// there and not at the 2-byte boundary (Photoshop pads the global
    /// blocks to 4). `end` when the list is over either way.
    fn next_block(&mut self, block_end: u64, len: u64, end: u64) -> Res<u64> {
        let padded = |n: u64| block_end - len + len.div_ceil(n) * n;
        let (two, four) = (padded(2).min(end), padded(4).min(end));
        if two == four || two + 12 > end || self.signature_at(two)? {
            return Ok(two);
        }
        if four + 12 <= end && self.signature_at(four)? {
            return Ok(four);
        }
        Ok(two)
    }

    /// Whether a block signature is at `at`; the position is left alone.
    fn signature_at(&mut self, at: u64) -> Res<bool> {
        let here = self.pos()?;
        self.src.seek(SeekFrom::Start(at))?;
        let sig = self.bytes::<4>()?;
        self.src.seek(SeekFrom::Start(here))?;
        Ok(&sig == b"8BIM" || &sig == b"8B64")
    }
}

pub(super) fn read<R: Read + Seek>(src: &mut R, fonts: &mut Fonts) -> Res<()> {
    let len = src.seek(SeekFrom::End(0))?;
    src.seek(SeekFrom::Start(0))?;
    let mut p = Psd {
        src,
        big: false,
        len,
    };
    if &p.bytes::<4>()? != b"8BPS" {
        return Err(Bad::malformed("not a Photoshop document (no 8BPS)"));
    }
    p.big = match p.u16()? {
        1 => false,
        2 => true,
        v => return Err(Bad::malformed(format!("Photoshop version {v}"))),
    };
    p.bytes::<6>()?; // reserved
    let channels = u64::from(p.u16()?);
    let height = u64::from(p.u32()?);
    let width = u64::from(p.u32()?);
    let depth = u64::from(p.u16()?);
    p.u16()?; // colour mode
    p.skip_block()?; // colour mode data
    p.skip_block()?; // image resources
    let section = p.length(true)?;
    let section_end = p.end(section)?;
    // The merged image data follows: its compression method, then raw rows
    // or (RLE) one byte count per row and channel.
    p.src.seek(SeekFrom::Start(section_end))?;
    let rows = channels.saturating_mul(height);
    let least = match p.u16()? {
        0 => rows.saturating_mul((width.saturating_mul(depth)).div_ceil(8)),
        1 => rows.saturating_mul(if p.big { 4 } else { 2 }),
        _ => 0,
    };
    if section_end.saturating_add(2).saturating_add(least) > len {
        return Err(Bad::malformed("the document ends early (truncated)"));
    }
    p.src.seek(SeekFrom::Start(section_end - section))?;
    if section == 0 {
        return Ok(());
    }
    let info = p.length(true)?;
    let info_end = p.end(info)?;
    if info_end > section_end {
        return Err(Bad::malformed("Photoshop layer info runs past its section"));
    }
    p.layer_info(info_end, fonts)?;
    p.src.seek(SeekFrom::Start(info_end))?;
    if p.pos()? + 4 <= section_end {
        p.skip_block()?; // global layer mask
        p.tagged_blocks(section_end, true, fonts)?;
    }
    Ok(())
}

/// The fonts the text runs of one `TySh` block use; every `FontSet` name
/// when the runs cannot be followed, so a malformed block loses none.
pub(super) fn text_fonts(data: &[u8], fonts: &mut Fonts) {
    match engine_data(data).and_then(run_fonts) {
        Some(names) => {
            for name in names.iter().filter(|n| *n != INVISIBLE) {
                fonts.postscript(name);
            }
        }
        None => font_set(data, fonts),
    }
}

/// The engine data of a `TySh` block: the `EngineData` item of its text
/// descriptor, a `tdta` of a 4-byte length and the raw text.
fn engine_data(data: &[u8]) -> Option<&[u8]> {
    const KEY: &[u8] = b"EngineDatatdta";
    let at = data.windows(KEY.len()).position(|w| w == KEY)? + KEY.len();
    let len = u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?);
    data.get(at + 4..)?.get(..usize::try_from(len).ok()?)
}

/// The `FontSet` names the runs use, or `None` when the engine data has no
/// run array, a run without a font, or an index outside the `FontSet`.
fn run_fonts(data: &[u8]) -> Option<Vec<String>> {
    let root = Engine::parse(data)?;
    let resources = root.get("ResourceDict")?;
    let set: Vec<String> = resources
        .get("FontSet")?
        .array()?
        .iter()
        .map(|f| f.get("Name").and_then(Value::string).map(decode))
        .collect::<Option<_>>()?;
    let sheets = resources.get("StyleSheetSet").and_then(Value::array);
    // `TheNormalStyleSheet` is the index of the default sheet; the first
    // entry when it is absent.
    let normal = resources
        .get("TheNormalStyleSheet")
        .and_then(Value::index)
        .unwrap_or(0);
    let default = sheets
        .and_then(|s| s.get(normal))
        .and_then(|s| s.get("StyleSheetData"))
        .and_then(|d| d.get("Font"));
    let runs = root
        .get("EngineDict")?
        .get("StyleRun")?
        .get("RunArray")?
        .array()?;
    if runs.is_empty() {
        return None;
    }
    let mut names = Vec::new();
    for run in runs {
        let own = run
            .get("StyleSheet")
            .and_then(|s| s.get("StyleSheetData"))
            .and_then(|d| d.get("Font"));
        let name = set.get(own.or(default)?.index()?)?;
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    Some(names)
}

/// A value of engine data, Photoshop's PostScript-like text.
#[derive(Debug)]
enum Value {
    Dict(Vec<(Vec<u8>, Value)>),
    Array(Vec<Value>),
    String(Vec<u8>),
    /// A name, number or keyword, as written.
    Atom(Vec<u8>),
}

impl Value {
    fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Dict(entries) => entries
                .iter()
                .find(|(k, _)| k == key.as_bytes())
                .map(|(_, v)| v),
            _ => None,
        }
    }

    fn array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    fn string(&self) -> Option<&[u8]> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// A non-negative integer.
    fn index(&self) -> Option<usize> {
        match self {
            Value::Atom(a) => std::str::from_utf8(a).ok()?.parse().ok(),
            _ => None,
        }
    }
}

/// The most nested dictionaries and arrays followed in engine data.
const MAX_DEPTH: u32 = 64;

struct Engine<'a> {
    data: &'a [u8],
    p: usize,
}

impl Engine<'_> {
    /// The top-level dictionary, or `None` when the data is not well formed.
    fn parse(data: &[u8]) -> Option<Value> {
        let mut e = Engine { data, p: 0 };
        let root = e.value(0)?;
        matches!(root, Value::Dict(_)).then_some(root)
    }

    fn skip_space(&mut self) {
        while self.data.get(self.p).is_some_and(u8::is_ascii_whitespace) {
            self.p += 1;
        }
    }

    fn at(&self, s: &[u8]) -> bool {
        self.data[self.p.min(self.data.len())..].starts_with(s)
    }

    fn value(&mut self, depth: u32) -> Option<Value> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.skip_space();
        if self.at(b"<<") {
            self.p += 2;
            let mut entries = Vec::new();
            loop {
                self.skip_space();
                if self.at(b">>") {
                    self.p += 2;
                    return Some(Value::Dict(entries));
                }
                let Value::Atom(key) = self.value(depth + 1)? else {
                    return None;
                };
                let key = key.strip_prefix(b"/")?.to_vec();
                let value = self.value(depth + 1)?;
                entries.push((key, value));
            }
        }
        match *self.data.get(self.p)? {
            b'[' => {
                self.p += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    if self.at(b"]") {
                        self.p += 1;
                        return Some(Value::Array(items));
                    }
                    items.push(self.value(depth + 1)?);
                }
            }
            b'(' => {
                // An unterminated string runs to the end of the data, so
                // its dictionary never closes and the parse fails.
                let (s, next) = ps_string(self.data, self.p + 1);
                self.p = next;
                Some(Value::String(s))
            }
            b']' | b'>' | b')' => None,
            _ => {
                let start = self.p;
                self.p += 1;
                while let Some(&c) = self.data.get(self.p) {
                    if c.is_ascii_whitespace() || b"[]()<>/".contains(&c) {
                        break;
                    }
                    self.p += 1;
                }
                Some(Value::Atom(self.data[start..self.p].to_vec()))
            }
        }
    }
}

/// The names of every `/FontSet` array in engine data (the resource
/// dictionary and the document resources each carry one).
fn font_set(data: &[u8], fonts: &mut Fonts) {
    for (at, _) in data
        .windows(8)
        .enumerate()
        .filter(|(_, w)| *w == b"/FontSet")
    {
        font_array(data, at + 8, fonts);
    }
}

/// The `/Name` strings of the array that starts after `p`.
fn font_array(data: &[u8], mut p: usize, fonts: &mut Fonts) {
    let mut depth = 0u32;
    let mut started = false;
    let mut last_key_is_name = false;
    while let Some(&c) = data.get(p) {
        match c {
            b'[' => {
                depth += 1;
                started = true;
                p += 1;
            }
            b']' => {
                depth = depth.saturating_sub(1);
                p += 1;
                if started && depth == 0 {
                    return;
                }
            }
            b'(' => {
                let (s, next) = ps_string(data, p + 1);
                if last_key_is_name {
                    let name = decode(&s);
                    if name != INVISIBLE {
                        fonts.postscript(&name);
                    }
                }
                last_key_is_name = false;
                p = next;
            }
            b'/' => {
                let end = data[p + 1..]
                    .iter()
                    .position(|c| !c.is_ascii_alphanumeric())
                    .map_or(data.len(), |e| p + 1 + e);
                last_key_is_name = &data[p + 1..end] == b"Name";
                p = end;
            }
            _ => p += 1,
        }
    }
}

/// A PostScript string body from `p` (after `(`) to its unescaped `)`, with
/// `\` escapes applied; returns the bytes and the position after `)`.
fn ps_string(data: &[u8], mut p: usize) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut nest = 0u32;
    while let Some(&c) = data.get(p) {
        p += 1;
        match c {
            b'\\' => {
                if let Some(&e) = data.get(p) {
                    p += 1;
                    out.push(match e {
                        b'n' => b'\n',
                        b'r' => b'\r',
                        b't' => b'\t',
                        e => e,
                    });
                }
            }
            b'(' => {
                nest += 1;
                out.push(c);
            }
            b')' if nest == 0 => return (out, p),
            b')' => {
                nest -= 1;
                out.push(c);
            }
            c => out.push(c),
        }
    }
    (out, p)
}

/// UTF-16BE after a byte-order mark, else Latin-1.
fn decode(s: &[u8]) -> String {
    match s {
        [0xFE, 0xFF, rest @ ..] => {
            let units = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_be_bytes(c));
            char::decode_utf16(units)
                .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect()
        }
        _ => s.iter().map(|&b| char::from(b)).collect(),
    }
}
