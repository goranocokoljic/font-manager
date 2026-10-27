//! A minimal PNG decoder for find-by-image (E33.1), the reading half of
//! [`super::png`].
//!
//! The spec's library table names no image crate, so this reads the format
//! by hand over `flate2`: 8-bit greyscale, greyscale with alpha, RGB and RGBA,
//! not interlaced; every filter type. Palette images, other bit depths and
//! interlaced images are [`fm_api::ErrorCode::Unsupported`], as is anything
//! that is not a PNG; an image over [`MAX_SIDE_PX`] on a side is
//! [`fm_api::ErrorCode::InvalidRequest`]. Every length is checked before it
//! is used and the inflated data is bounded by the size the header declares,
//! so a corrupt or hostile file is an error, never a panic or a huge
//! allocation.

use std::io::Read;

use flate2::read::ZlibDecoder;

use super::png::crc32;
use crate::{FmError, FmResult};

/// The widest and tallest image that is decoded.
pub const MAX_SIDE_PX: u32 = 4096;

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// The message of an image that is damaged or not what its header says.
const UNREADABLE: &str = "This image could not be read.";

/// A decoded image, row by row from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The luminance of every pixel (`0` black, `255` white), from
    /// `(299 R + 587 G + 114 B) / 1000` for a colour image.
    pub grey: Vec<u8>,
    /// The alpha of every pixel, when the image has an alpha channel.
    pub alpha: Option<Vec<u8>>,
}

fn corrupt(detail: impl Into<String>) -> FmError {
    FmError::unsupported(UNREADABLE).with_detail(detail)
}

fn unsupported_kind(detail: impl Into<String>) -> FmError {
    FmError::unsupported(
        "This kind of PNG cannot be read; save it as an 8-bit grey or colour PNG without interlacing.",
    )
    .with_detail(detail)
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// Decodes the PNG `data`; see the module docs for what is read.
pub fn decode(data: &[u8]) -> FmResult<Pixels> {
    if data.len() < SIGNATURE.len() || &data[..8] != SIGNATURE {
        return Err(
            FmError::unsupported("Only PNG images can be read.").with_detail("no PNG signature")
        );
    }
    let mut at = 8;
    let mut header: Option<(u32, u32, usize)> = None;
    let mut idat = Vec::new();
    let mut ended = false;
    while at < data.len() {
        let rest = &data[at..];
        if rest.len() < 12 {
            return Err(corrupt(format!("truncated chunk at byte {at}")));
        }
        let len = be32(rest) as usize;
        if len > rest.len() - 12 {
            return Err(corrupt(format!("chunk at byte {at} runs past the end")));
        }
        let kind = &rest[4..8];
        let body = &rest[8..8 + len];
        if be32(&rest[8 + len..]) != crc32(&rest[4..8 + len]) {
            return Err(corrupt(format!("bad CRC in the chunk at byte {at}")));
        }
        at += 12 + len;
        match kind {
            b"IHDR" => header = Some(read_header(body)?),
            _ if header.is_none() => return Err(corrupt("the first chunk is not IHDR")),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => {
                ended = true;
                break;
            }
            // A suggested palette of a colour image; nothing to read.
            b"PLTE" => {}
            _ if kind[0].is_ascii_uppercase() => {
                return Err(unsupported_kind(format!(
                    "critical chunk {}",
                    String::from_utf8_lossy(kind)
                )));
            }
            _ => {}
        }
    }
    let (width, height, channels) = header.ok_or_else(|| corrupt("no IHDR chunk"))?;
    if !ended {
        return Err(corrupt("no IEND chunk: the file is truncated"));
    }
    let stride = width as usize * channels;
    let expected = height as usize * (stride + 1);
    let mut raw = Vec::with_capacity(expected);
    ZlibDecoder::new(&idat[..])
        .take(expected as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|e| corrupt(format!("image data: {e}")))?;
    if raw.len() != expected {
        return Err(corrupt(format!(
            "image data is {} bytes, the header needs {expected}",
            raw.len()
        )));
    }
    let pixels = unfilter(&raw, stride, channels)?;
    Ok(split(width, height, channels, &pixels))
}

/// Width, height and channels from the IHDR body.
fn read_header(body: &[u8]) -> FmResult<(u32, u32, usize)> {
    if body.len() != 13 {
        return Err(corrupt("IHDR is not 13 bytes"));
    }
    let (width, height) = (be32(body), be32(&body[4..]));
    let (depth, colour, compression, filter, interlace) =
        (body[8], body[9], body[10], body[11], body[12]);
    if width == 0 || height == 0 {
        return Err(corrupt("the image has no pixels"));
    }
    if compression != 0 || filter != 0 {
        return Err(corrupt("unknown compression or filter method"));
    }
    let channels = match colour {
        0 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        3 => return Err(unsupported_kind("palette image")),
        _ => return Err(corrupt(format!("colour type {colour}"))),
    };
    if depth != 8 {
        return Err(unsupported_kind(format!("bit depth {depth}")));
    }
    if interlace != 0 {
        return Err(unsupported_kind("interlaced image"));
    }
    if width > MAX_SIDE_PX || height > MAX_SIDE_PX {
        return Err(FmError::invalid(format!(
            "The image is larger than {MAX_SIDE_PX} px on a side; crop it to the text."
        ))
        .with_detail(format!("{width} x {height} px")));
    }
    Ok((width, height, channels))
}

/// Undoes the per-row filters of `raw` (one filter byte, then `stride`
/// bytes per row).
fn unfilter(raw: &[u8], stride: usize, bpp: usize) -> FmResult<Vec<u8>> {
    let rows = raw.len() / (stride + 1);
    let mut out = vec![0u8; rows * stride];
    for y in 0..rows {
        let line = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let (filter, src) = (line[0], &line[1..]);
        let (done, rest) = out.split_at_mut(y * stride);
        let prior = done
            .get(done.len().saturating_sub(stride)..)
            .filter(|_| y > 0);
        let cur = &mut rest[..stride];
        for x in 0..stride {
            let a = if x >= bpp { cur[x - bpp] } else { 0 };
            let b = prior.map_or(0, |p| p[x]);
            let c = match prior {
                Some(p) if x >= bpp => p[x - bpp],
                _ => 0,
            };
            let predicted = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                4 => paeth(a, b, c),
                _ => return Err(corrupt(format!("filter type {filter} in row {y}"))),
            };
            cur[x] = src[x].wrapping_add(predicted);
        }
    }
    Ok(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i16::from(a), i16::from(b), i16::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Interleaved samples to luminance and alpha planes.
fn split(width: u32, height: u32, channels: usize, pixels: &[u8]) -> Pixels {
    let luma = |p: &[u8]| -> u8 {
        if channels >= 3 {
            let sum = 299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2]);
            (sum / 1000) as u8
        } else {
            p[0]
        }
    };
    let px = pixels.chunks_exact(channels);
    let grey = px.clone().map(luma).collect();
    let alpha = matches!(channels, 2 | 4).then(|| px.map(|p| p[channels - 1]).collect());
    Pixels {
        width,
        height,
        grey,
        alpha,
    }
}

#[cfg(test)]
mod tests;
