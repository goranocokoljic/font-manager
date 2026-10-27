//! A read-only zip reader: the central directory (zip64 included), then one
//! entry at a time, stored or deflated (`flate2`). Office, Sketch and Figma
//! documents are zip containers; nothing else of the format is needed.

use std::io::{Read, Seek, SeekFrom};

use flate2::read::DeflateDecoder;

use super::{Bad, Res, read_capped, read_exact_capped};

/// The most an entry may inflate to; a larger one is refused (zip bomb).
pub(super) const MAX_ENTRY: u64 = 256 << 20;
/// The most the central directory may take.
const MAX_DIRECTORY: u64 = 64 << 20;
/// End of central directory record, its zip64 locator and record.
const EOCD: u32 = 0x0605_4b50;
const EOCD64_LOCATOR: u32 = 0x0706_4b50;
const EOCD64: u32 = 0x0606_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;

#[derive(Debug)]
struct Entry {
    name: String,
    flags: u16,
    method: u16,
    compressed: u64,
    size: u64,
    offset: u64,
}

/// An opened archive.
#[derive(Debug)]
pub(super) struct Zip<'a, R> {
    src: &'a mut R,
    entries: Vec<Entry>,
}

fn u16_at(b: &[u8], at: usize) -> Res<u16> {
    match b.get(at..at + 2) {
        Some(&[x, y]) => Ok(u16::from_le_bytes([x, y])),
        _ => Err(Bad::malformed("zip record ends early")),
    }
}

fn u32_at(b: &[u8], at: usize) -> Res<u32> {
    match b.get(at..at + 4) {
        Some(&[a, b2, c, d]) => Ok(u32::from_le_bytes([a, b2, c, d])),
        _ => Err(Bad::malformed("zip record ends early")),
    }
}

fn u64_at(b: &[u8], at: usize) -> Res<u64> {
    let lo = u64::from(u32_at(b, at)?);
    let hi = u64::from(u32_at(b, at + 4)?);
    Ok(lo | hi << 32)
}

impl<'a, R: Read + Seek> Zip<'a, R> {
    /// Reads the central directory. Not a zip → malformed.
    pub(super) fn open(src: &'a mut R) -> Res<Self> {
        let len = src.seek(SeekFrom::End(0))?;
        // The EOCD is 22 bytes plus a comment of at most 65,535.
        let tail_len = len.min(22 + 0xFFFF);
        src.seek(SeekFrom::Start(len - tail_len))?;
        let tail = read_exact_capped(src, tail_len, tail_len, "zip tail")?;
        let at = (0..tail.len().saturating_sub(21))
            .rev()
            .find(|&i| u32_at(&tail, i).is_ok_and(|s| s == EOCD))
            .ok_or_else(|| Bad::malformed("not a zip archive (no end of central directory)"))?;
        let eocd_pos = len - tail_len + at as u64;
        let mut count = u64::from(u16_at(&tail, at + 10)?);
        let mut dir_size = u64::from(u32_at(&tail, at + 12)?);
        let mut dir_offset = u64::from(u32_at(&tail, at + 16)?);
        if count == 0xFFFF || dir_size == 0xFFFF_FFFF || dir_offset == 0xFFFF_FFFF {
            let loc = at
                .checked_sub(20)
                .ok_or_else(|| Bad::malformed("zip64 locator missing"))?;
            if u32_at(&tail, loc)? != EOCD64_LOCATOR {
                return Err(Bad::malformed("zip64 locator missing"));
            }
            let record = u64_at(&tail, loc + 8)?;
            if record >= eocd_pos {
                return Err(Bad::malformed("zip64 record out of range"));
            }
            src.seek(SeekFrom::Start(record))?;
            let r = read_exact_capped(src, 56, 56, "zip64 record")?;
            if u32_at(&r, 0)? != EOCD64 {
                return Err(Bad::malformed("zip64 record signature"));
            }
            count = u64_at(&r, 32)?;
            dir_size = u64_at(&r, 40)?;
            dir_offset = u64_at(&r, 48)?;
        }
        if dir_offset
            .checked_add(dir_size)
            .is_none_or(|end| end > eocd_pos)
        {
            return Err(Bad::malformed("central directory out of range"));
        }
        src.seek(SeekFrom::Start(dir_offset))?;
        let dir = read_exact_capped(src, dir_size, MAX_DIRECTORY, "central directory")?;
        // A record takes at least 46 bytes, so the count cannot exceed that.
        if count > dir_size / 46 {
            return Err(Bad::malformed("central directory count out of range"));
        }
        let mut entries = Vec::new();
        let mut p = 0usize;
        for _ in 0..count {
            if u32_at(&dir, p)? != CENTRAL {
                return Err(Bad::malformed("central directory record signature"));
            }
            let flags = u16_at(&dir, p + 8)?;
            let method = u16_at(&dir, p + 10)?;
            let mut compressed = u64::from(u32_at(&dir, p + 20)?);
            let mut size = u64::from(u32_at(&dir, p + 24)?);
            let name_len = usize::from(u16_at(&dir, p + 28)?);
            let extra_len = usize::from(u16_at(&dir, p + 30)?);
            let comment_len = usize::from(u16_at(&dir, p + 32)?);
            let mut offset = u64::from(u32_at(&dir, p + 42)?);
            let name = dir
                .get(p + 46..p + 46 + name_len)
                .ok_or_else(|| Bad::malformed("zip entry name ends early"))?;
            let extra = dir
                .get(p + 46 + name_len..p + 46 + name_len + extra_len)
                .ok_or_else(|| Bad::malformed("zip extra field ends early"))?;
            zip64_extra(extra, &mut size, &mut compressed, &mut offset)?;
            entries.push(Entry {
                name: String::from_utf8_lossy(name).into_owned(),
                flags,
                method,
                compressed,
                size,
                offset,
            });
            p += 46 + name_len + extra_len + comment_len;
        }
        Ok(Self { src, entries })
    }

    /// Every entry name, in directory order.
    pub(super) fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.name.as_str())
    }

    /// The inflated bytes of `name` (a leading `/` and the case are ignored);
    /// `None` when the archive has no such entry.
    pub(super) fn read(&mut self, name: &str) -> Res<Option<Vec<u8>>> {
        let want = name.trim_start_matches('/');
        let Some(e) = self
            .entries
            .iter()
            .find(|e| e.name.trim_start_matches('/').eq_ignore_ascii_case(want))
        else {
            return Ok(None);
        };
        if e.flags & 1 != 0 {
            return Err(Bad::unsupported(format!(
                "zip entry {} is encrypted",
                e.name
            )));
        }
        self.src.seek(SeekFrom::Start(e.offset))?;
        let head = read_exact_capped(self.src, 30, 30, "zip local header")?;
        if u32_at(&head, 0)? != LOCAL {
            return Err(Bad::malformed(format!("zip local header of {}", e.name)));
        }
        let skip = i64::from(u16_at(&head, 26)?) + i64::from(u16_at(&head, 28)?);
        self.src.seek(SeekFrom::Current(skip))?;
        let what = format!("zip entry {}", e.name);
        let data = match e.method {
            0 => read_exact_capped(self.src, e.compressed, MAX_ENTRY, &what)?,
            8 => {
                let raw = (&mut *self.src).take(e.compressed);
                read_capped(&mut DeflateDecoder::new(raw), MAX_ENTRY, &what)?
            }
            m => {
                return Err(Bad::unsupported(format!(
                    "{what} uses compression method {m}"
                )));
            }
        };
        if data.len() as u64 != e.size {
            return Err(Bad::malformed(format!(
                "{what} inflated to {} bytes, the directory says {}",
                data.len(),
                e.size
            )));
        }
        Ok(Some(data))
    }
}

/// Replaces the 32-bit sizes and offset that are all ones by the 64-bit
/// values of the zip64 extra field (id 1), in the order the format gives.
fn zip64_extra(extra: &[u8], size: &mut u64, compressed: &mut u64, offset: &mut u64) -> Res<()> {
    let mut p = 0;
    while p + 4 <= extra.len() {
        let id = u16_at(extra, p)?;
        let len = usize::from(u16_at(extra, p + 2)?);
        let body = extra
            .get(p + 4..p + 4 + len)
            .ok_or_else(|| Bad::malformed("zip extra field ends early"))?;
        if id == 1 {
            let mut q = 0;
            for v in [size, compressed, offset] {
                if *v == 0xFFFF_FFFF {
                    *v = u64_at(body, q)?;
                    q += 8;
                }
            }
            return Ok(());
        }
        p += 4 + len;
    }
    Ok(())
}
