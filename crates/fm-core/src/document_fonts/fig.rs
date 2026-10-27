//! Figma local copies (.fig): a zip holding `canvas.fig`, or `canvas.fig`
//! itself. That file is `fig-kiwi` (8 bytes), a version (u32), then
//! length-prefixed chunks: the kiwi schema and the kiwi-encoded document,
//! each raw-deflated. Newer files compress with zstd, which this module has
//! no decoder for: they are `Unsupported`.
//!
//! The schema is decoded, then the document is walked with it; every field
//! named `fontName` is a `FontName { family, style, postscript }`.

use std::io::{Read, Seek, SeekFrom};

use flate2::read::DeflateDecoder;

use super::zip::{MAX_ENTRY, Zip};
use super::{Bad, Fonts, Res, read_capped};

const ZSTD: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];
/// Kiwi definition kinds.
const ENUM: u8 = 0;
const STRUCT: u8 = 1;
const MESSAGE: u8 = 2;
/// The deepest nesting walked, and the most values decoded: a hostile file
/// cannot recurse the stack away or spin on zero-byte values.
const MAX_DEPTH: u32 = 64;
const MAX_STEPS: u64 = 50_000_000;

pub(super) fn read<R: Read + Seek>(src: &mut R, fonts: &mut Fonts) -> Res<()> {
    let mut magic = [0u8; 4];
    src.read_exact(&mut magic)?;
    src.seek(SeekFrom::Start(0))?;
    let canvas = if magic == *b"PK\x03\x04" {
        Zip::open(src)?
            .read("canvas.fig")?
            .ok_or_else(|| Bad::malformed("not a Figma file (no canvas.fig)"))?
    } else {
        read_capped(src, MAX_ENTRY, "canvas.fig")?
    };
    canvas_fonts(&canvas, fonts)
}

fn canvas_fonts(canvas: &[u8], fonts: &mut Fonts) -> Res<()> {
    if !canvas.starts_with(b"fig-") || canvas.len() < 12 {
        return Err(Bad::malformed("not a Figma canvas (no fig-kiwi header)"));
    }
    let mut chunks = Vec::new();
    let mut p = 12usize;
    while chunks.len() < 2 {
        let len = canvas
            .get(p..p + 4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| Bad::malformed("Figma chunk header ends early"))?;
        let body = canvas
            .get(p + 4..p + 4 + len as usize)
            .ok_or_else(|| Bad::malformed("Figma chunk ends early"))?;
        chunks.push(body);
        p += 4 + len as usize;
    }
    let [schema, data] = [chunks[0], chunks[1]].map(inflate);
    let schema = Schema::decode(&schema?)?;
    let data = data?;
    let root = schema
        .root()
        .ok_or_else(|| Bad::malformed("kiwi schema has no message"))?;
    let mut walk = Walk {
        schema: &schema,
        buf: Buf { b: &data, p: 0 },
        steps: 0,
        fonts,
    };
    walk.message(root, false, 0)
}

fn inflate(chunk: &[u8]) -> Res<Vec<u8>> {
    if chunk.starts_with(&ZSTD) {
        return Err(Bad::unsupported(
            "the Figma file is zstd-compressed, which Font Manager cannot read yet",
        ));
    }
    read_capped(&mut DeflateDecoder::new(chunk), MAX_ENTRY, "Figma chunk")
}

/// A byte cursor with kiwi's primitive readers.
struct Buf<'a> {
    b: &'a [u8],
    p: usize,
}

impl Buf<'_> {
    fn byte(&mut self) -> Res<u8> {
        let v = *self
            .b
            .get(self.p)
            .ok_or_else(|| Bad::malformed("kiwi data ends early"))?;
        self.p += 1;
        Ok(v)
    }

    fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.p)
    }

    fn var_uint(&mut self) -> Res<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            v |= u64::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(Bad::malformed("kiwi varint too long"))
    }

    fn var_int(&mut self) -> Res<i64> {
        let v = self.var_uint()?;
        // Zigzag: kiwi's `readVarInt`.
        Ok(if v & 1 == 0 {
            (v >> 1) as i64
        } else {
            !((v >> 1) as i64)
        })
    }

    /// Kiwi floats are one zero byte or four bytes.
    fn skip_float(&mut self) -> Res<()> {
        if self.byte()? != 0 {
            for _ in 0..3 {
                self.byte()?;
            }
        }
        Ok(())
    }

    fn string(&mut self) -> Res<String> {
        let rest = self.b.get(self.p..).unwrap_or_default();
        let end = rest
            .iter()
            .position(|&c| c == 0)
            .ok_or_else(|| Bad::malformed("kiwi string not terminated"))?;
        let s = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.p += end + 1;
        Ok(s)
    }
}

#[derive(Debug)]
struct Field {
    name: String,
    ty: i64,
    array: bool,
    id: u64,
}

#[derive(Debug)]
struct Def {
    name: String,
    kind: u8,
    fields: Vec<Field>,
}

#[derive(Debug)]
struct Schema(Vec<Def>);

impl Schema {
    fn decode(bytes: &[u8]) -> Res<Self> {
        let mut b = Buf { b: bytes, p: 0 };
        let count = b.var_uint()?;
        let mut defs = Vec::new();
        for _ in 0..count {
            let name = b.string()?;
            let kind = b.byte()?;
            if kind > MESSAGE {
                return Err(Bad::malformed("kiwi definition kind"));
            }
            let n = b.var_uint()?;
            let mut fields = Vec::new();
            for _ in 0..n {
                // Each field takes at least four bytes: a bound on `n`.
                if b.remaining() < 4 {
                    return Err(Bad::malformed("kiwi schema ends early"));
                }
                let name = b.string()?;
                let ty = b.var_int()?;
                let array = b.byte()? & 1 == 1;
                let id = b.var_uint()?;
                fields.push(Field {
                    name,
                    ty,
                    array,
                    id,
                });
            }
            defs.push(Def { name, kind, fields });
        }
        for d in &defs {
            for f in &d.fields {
                if f.ty >= 0 && f.ty as u64 >= defs.len() as u64 || f.ty < -8 {
                    return Err(Bad::malformed("kiwi field type out of range"));
                }
            }
        }
        Ok(Self(defs))
    }

    /// Figma's root is `Message`; otherwise the first message.
    fn root(&self) -> Option<usize> {
        let msgs = || self.0.iter().enumerate().filter(|(_, d)| d.kind == MESSAGE);
        msgs()
            .find(|(_, d)| d.name == "Message")
            .or_else(|| msgs().next())
            .map(|(i, _)| i)
    }
}

/// The font name being collected (inside a `fontName` value).
#[derive(Debug, Default)]
struct FontName {
    family: Option<String>,
    style: Option<String>,
    postscript: Option<String>,
}

struct Walk<'a, 'f> {
    schema: &'a Schema,
    buf: Buf<'a>,
    steps: u64,
    fonts: &'f mut Fonts,
}

impl Walk<'_, '_> {
    fn step(&mut self) -> Res<()> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(Bad::malformed("kiwi data too large to walk"));
        }
        Ok(())
    }

    /// Decodes a value of definition `def`; `font` when it is a `fontName`.
    fn message(&mut self, def: usize, font: bool, depth: u32) -> Res<()> {
        if depth > MAX_DEPTH {
            return Err(Bad::malformed("kiwi data nested too deep"));
        }
        let schema = self.schema;
        let d = schema
            .0
            .get(def)
            .ok_or_else(|| Bad::malformed("kiwi definition"))?;
        let mut name = FontName::default();
        match d.kind {
            ENUM => {
                self.buf.var_uint()?;
            }
            STRUCT => {
                for f in &d.fields {
                    self.field(f, font, &mut name, depth)?;
                }
            }
            _ => loop {
                let id = self.buf.var_uint()?;
                if id == 0 {
                    break;
                }
                let f =
                    d.fields.iter().find(|f| f.id == id).ok_or_else(|| {
                        Bad::malformed(format!("kiwi field {id} not in {}", d.name))
                    })?;
                self.field(f, font, &mut name, depth)?;
            },
        }
        if font {
            self.fonts.add(
                name.postscript.as_deref(),
                name.family.as_deref(),
                name.style.as_deref(),
            );
        }
        Ok(())
    }

    fn field(&mut self, f: &Field, font: bool, name: &mut FontName, depth: u32) -> Res<()> {
        let count = if f.array {
            let n = self.buf.var_uint()?;
            if n > self.buf.remaining() as u64 {
                return Err(Bad::malformed("kiwi array longer than the data"));
            }
            n
        } else {
            1
        };
        for _ in 0..count {
            self.step()?;
            match f.ty {
                -1 | -2 => {
                    self.buf.byte()?;
                }
                -3 | -4 | -7 | -8 => {
                    self.buf.var_uint()?;
                }
                -5 => self.buf.skip_float()?,
                -6 => {
                    let s = self.buf.string()?;
                    if font && !f.array {
                        match f.name.as_str() {
                            "family" => name.family = Some(s),
                            "style" => name.style = Some(s),
                            "postscript" => name.postscript = Some(s),
                            _ => {}
                        }
                    }
                }
                t => {
                    let def = usize::try_from(t)
                        .map_err(|_| Bad::malformed("kiwi field type out of range"))?;
                    self.message(def, f.name == "fontName", depth + 1)?;
                }
            }
        }
        Ok(())
    }
}
