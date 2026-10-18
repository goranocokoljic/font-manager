//! A minimal PNG encoder for rendered previews.
//!
//! The spec's library table names no PNG or deflate crate, so this writes the
//! format by hand: 8-bit RGBA, no interlace, filter type 0, and a zlib stream
//! of stored (uncompressed) deflate blocks. Every decoder reads it; the cost is
//! size (about four bytes per pixel), which is fine for thumbnail-sized images.

/// The largest payload of one stored deflate block.
const MAX_STORED_BLOCK: usize = 0xFFFF;

/// Encodes straight (not premultiplied) RGBA pixels, row by row, as a PNG.
/// `rgba.len()` must be `width * height * 4`; the caller guarantees it.
pub(crate) fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row = width as usize * 4;
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgba.chunks_exact(row.max(1)).take(height as usize) {
        raw.push(0); // filter type None
        raw.extend_from_slice(line);
    }

    let mut out = Vec::with_capacity(raw.len() + raw.len() / MAX_STORED_BLOCK * 5 + 64);
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // Bit depth 8, colour type 6 (RGBA), deflate, adaptive filtering, no interlace.
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    // Chunk data never exceeds a few hundred MB: the canvas is capped.
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// A zlib stream (RFC 1950) holding `data` in stored deflate blocks (RFC 1951).
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / MAX_STORED_BLOCK * 5 + 11);
    // CMF: deflate, 32 KiB window. FLG: no dictionary, check bits so that
    // (CMF * 256 + FLG) % 31 == 0.
    out.extend_from_slice(&[0x78, 0x01]);
    let mut blocks = data.chunks(MAX_STORED_BLOCK).peekable();
    if blocks.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(block) = blocks.next() {
        let last = u8::from(blocks.peek().is_none());
        let len = block.len() as u16;
        out.push(last);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut n = 0;
        while n < 256 {
            let mut c = n as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 == 1 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
                k += 1;
            }
            table[n] = c;
            n += 1;
        }
        table
    };
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let (mut a, mut b) = (1u32, 0u32);
    // 5,552 bytes is the most that can be summed before `b` could overflow.
    for block in data.chunks(5552) {
        for &x in block {
            a += u32::from(x);
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A PNG read back by a test: its header fields and the RGBA pixels.
    #[derive(Debug)]
    pub(crate) struct Decoded {
        pub width: u32,
        pub height: u32,
        pub rgba: Vec<u8>,
    }

    fn be32(b: &[u8]) -> u32 {
        u32::from_be_bytes([b[0], b[1], b[2], b[3]])
    }

    /// Reads a PNG this encoder wrote, checking every CRC, the zlib header,
    /// the stored blocks and the Adler-32. Panics on anything else.
    pub(crate) fn decode(png: &[u8]) -> Decoded {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "signature");
        let mut at = 8;
        let (mut width, mut height, mut idat) = (0, 0, Vec::new());
        let mut kinds = Vec::new();
        while at < png.len() {
            let len = be32(&png[at..]) as usize;
            let kind = &png[at + 4..at + 8];
            let data = &png[at + 8..at + 8 + len];
            let crc = be32(&png[at + 8 + len..]);
            assert_eq!(crc, crc32(&png[at + 4..at + 8 + len]), "crc of {kind:?}");
            kinds.push(kind.to_vec());
            match kind {
                b"IHDR" => {
                    width = be32(data);
                    height = be32(&data[4..]);
                    assert_eq!(&data[8..], &[8, 6, 0, 0, 0]);
                }
                b"IDAT" => idat.extend_from_slice(data),
                _ => {}
            }
            at += 12 + len;
        }
        assert_eq!(
            kinds,
            vec![b"IHDR".to_vec(), b"IDAT".to_vec(), b"IEND".to_vec()]
        );
        assert_eq!((u32::from(idat[0]) * 256 + u32::from(idat[1])) % 31, 0);
        let mut raw = Vec::new();
        let mut p = 2;
        loop {
            let last = idat[p] & 1;
            assert_eq!(idat[p] >> 1, 0, "stored block type");
            let len = usize::from(u16::from_le_bytes([idat[p + 1], idat[p + 2]]));
            let nlen = u16::from_le_bytes([idat[p + 3], idat[p + 4]]);
            assert_eq!(!nlen as usize, len);
            raw.extend_from_slice(&idat[p + 5..p + 5 + len]);
            p += 5 + len;
            if last == 1 {
                break;
            }
        }
        assert_eq!(be32(&idat[p..]), adler32(&raw), "adler32");
        let row = width as usize * 4;
        let mut rgba = Vec::new();
        for line in raw.chunks_exact(row + 1) {
            assert_eq!(line[0], 0, "filter type");
            rgba.extend_from_slice(&line[1..]);
        }
        assert_eq!(rgba.len(), row * height as usize);
        Decoded {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn checksums_match_the_reference_values() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(&[]), 1);
    }

    #[test]
    fn round_trips_pixels() {
        let rgba: Vec<u8> = (0..3 * 2 * 4).map(|i| i as u8 * 7).collect();
        let png = encode_rgba(3, 2, &rgba);
        let back = decode(&png);
        assert_eq!((back.width, back.height), (3, 2));
        assert_eq!(back.rgba, rgba);
    }

    #[test]
    fn splits_large_images_into_several_stored_blocks() {
        // 200 x 200 x 4 + 200 filter bytes = 160,200 bytes: three blocks.
        let rgba: Vec<u8> = (0..200 * 200 * 4).map(|i| (i % 251) as u8).collect();
        let back = decode(&encode_rgba(200, 200, &rgba));
        assert_eq!(back.rgba, rgba);
    }
}
