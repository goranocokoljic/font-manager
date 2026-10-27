use std::io::Write as _;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use fm_api::ErrorCode;

use super::*;

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(data).expect("deflate");
    z.finish().expect("deflate")
}

/// The forward PNG filter `kind` of `row`, `prior` the unfiltered row above.
fn filter(kind: u8, row: &[u8], prior: Option<&[u8]>, bpp: usize) -> Vec<u8> {
    let mut out = vec![kind];
    for x in 0..row.len() {
        let a = if x >= bpp { row[x - bpp] } else { 0 };
        let b = prior.map_or(0, |p| p[x]);
        let c = match prior {
            Some(p) if x >= bpp => p[x - bpp],
            _ => 0,
        };
        let predicted = match kind {
            0 => 0,
            1 => a,
            2 => b,
            3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
            _ => paeth(a, b, c),
        };
        out.push(row[x].wrapping_sub(predicted));
    }
    out
}

/// A PNG with the given header fields; row `y` uses filter `y % 5`.
fn png(width: u32, height: u32, depth: u8, colour: u8, interlace: u8, samples: &[u8]) -> Vec<u8> {
    let channels = match colour {
        0 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => 1,
    };
    let stride = width as usize * channels;
    let mut raw = Vec::new();
    for (y, row) in samples.chunks(stride.max(1)).enumerate() {
        let prior = y
            .checked_sub(1)
            .map(|p| &samples[p * stride..(p + 1) * stride]);
        raw.extend(filter((y % 5) as u8, row, prior, channels));
    }
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[depth, colour, 0, 0, interlace]);
    let mut out = SIGNATURE.to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"tEXt", b"Comment\0ignored");
    chunk(&mut out, b"IDAT", &zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Deterministic samples that exercise every filter's arithmetic.
fn samples(n: usize) -> Vec<u8> {
    (0..n)
        .map(|i| ((i * 37 + i / 7 * 11) % 256) as u8)
        .collect()
}

#[test]
fn reads_greyscale_with_every_filter_type() {
    let data = samples(9 * 10);
    let image = decode(&png(9, 10, 8, 0, 0, &data)).expect("decode");
    assert_eq!((image.width, image.height), (9, 10));
    assert_eq!(image.grey, data);
    assert_eq!(image.alpha, None);
}

#[test]
fn reads_greyscale_with_alpha() {
    let data = samples(5 * 6 * 2);
    let image = decode(&png(5, 6, 8, 4, 0, &data)).expect("decode");
    let grey: Vec<u8> = data.chunks(2).map(|p| p[0]).collect();
    let alpha: Vec<u8> = data.chunks(2).map(|p| p[1]).collect();
    assert_eq!(image.grey, grey);
    assert_eq!(image.alpha, Some(alpha));
}

#[test]
fn reads_rgb_as_luminance() {
    // Pure red, green, blue, white, black.
    let data = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0];
    let image = decode(&png(5, 1, 8, 2, 0, &data)).expect("decode");
    assert_eq!(image.grey, [76, 149, 29, 255, 0]);
    assert_eq!(image.alpha, None);
}

#[test]
fn reads_rgba_and_what_the_encoder_writes() {
    let data = samples(7 * 6 * 4);
    let image = decode(&png(7, 6, 8, 6, 0, &data)).expect("decode");
    let alpha: Vec<u8> = data.chunks(4).map(|p| p[3]).collect();
    assert_eq!(image.alpha, Some(alpha.clone()));
    // Stored deflate blocks, filter type 0 (preview/png.rs).
    let ours = decode(&super::super::png::encode_rgba(7, 6, &data)).expect("decode");
    assert_eq!(ours.grey, image.grey);
    assert_eq!(ours.alpha, Some(alpha));
}

#[test]
fn other_formats_are_a_plain_error() {
    for (what, bytes) in [
        ("jpeg", b"\xff\xd8\xff\xe0\0\x10JFIF".to_vec()),
        ("empty", Vec::new()),
        ("text", b"not an image at all".to_vec()),
    ] {
        let e = decode(&bytes).expect_err(what);
        assert_eq!(e.code(), ErrorCode::Unsupported, "{what}");
        assert_eq!(e.message(), "Only PNG images can be read.", "{what}");
    }
}

#[test]
fn other_kinds_of_png_are_unsupported() {
    let kinds = [
        ("16-bit", png(2, 2, 16, 0, 0, &[0; 8])),
        ("palette", png(2, 2, 8, 3, 0, &[0; 4])),
        ("interlaced", png(2, 2, 8, 0, 1, &[0; 4])),
    ];
    for (what, bytes) in kinds {
        let e = decode(&bytes).expect_err(what);
        assert_eq!(e.code(), ErrorCode::Unsupported, "{what}");
        assert!(e.message().contains("8-bit"), "{what}: {}", e.message());
    }
}

#[test]
fn an_image_over_4096_px_a_side_is_refused_before_it_is_inflated() {
    for (w, h) in [(4097, 1), (1, 4097)] {
        // The IDAT is far too short: the size is refused first.
        let mut bytes = SIGNATURE.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&u32::to_be_bytes(w));
        ihdr.extend_from_slice(&u32::to_be_bytes(h));
        ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
        chunk(&mut bytes, b"IHDR", &ihdr);
        chunk(&mut bytes, b"IDAT", &zlib(&[0]));
        chunk(&mut bytes, b"IEND", &[]);
        let e = decode(&bytes).expect_err("too large");
        assert_eq!(e.code(), ErrorCode::InvalidRequest);
        assert!(e.message().contains("4096"), "{}", e.message());
    }
    assert!(decode(&png(4096, 1, 8, 0, 0, &[255; 4096])).is_ok());
}

#[test]
fn a_corrupt_png_is_a_plain_error_at_every_truncation() {
    let good = png(6, 5, 8, 6, 0, &samples(6 * 5 * 4));
    for len in 8..good.len() {
        let e = decode(&good[..len]).expect_err("truncated");
        assert_eq!(e.code(), ErrorCode::Unsupported, "at {len}");
        assert_eq!(e.message(), UNREADABLE, "at {len}");
    }
    // Every single-bit flip after the signature: an error or (a flip in
    // an ancillary chunk's data) the same image; never a panic.
    let image = decode(&good).expect("decode");
    for at in 8..good.len() {
        for bit in 0..8 {
            let mut bad = good.clone();
            bad[at] ^= 1 << bit;
            if let Ok(other) = decode(&bad) {
                assert_eq!(other, image, "flip at {at}.{bit}");
            }
        }
    }
}

#[test]
fn too_much_image_data_is_corrupt() {
    let mut bytes = SIGNATURE.to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    chunk(&mut bytes, b"IHDR", &ihdr);
    chunk(&mut bytes, b"IDAT", &zlib(&[0; 1 << 20]));
    chunk(&mut bytes, b"IEND", &[]);
    let e = decode(&bytes).expect_err("too much");
    assert!(e.detail().is_some_and(|d| d.contains("needs 6")), "{e:?}");
}

#[test]
fn an_unknown_filter_type_is_corrupt() {
    let mut bytes = SIGNATURE.to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    chunk(&mut bytes, b"IHDR", &ihdr);
    chunk(&mut bytes, b"IDAT", &zlib(&[7, 0]));
    chunk(&mut bytes, b"IEND", &[]);
    assert_eq!(decode(&bytes).expect_err("filter").message(), UNREADABLE);
}

/// The fuzz seeds (`fuzz/seeds/png_decode`) are PNGs this decoder reads, so
/// the fuzzer starts from valid images.
#[test]
fn the_fuzz_seeds_decode() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/seeds/png_decode");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("seeds") {
        let path = entry.expect("entry").path();
        decode(&std::fs::read(&path).expect("seed")).unwrap_or_else(|e| panic!("{path:?}: {e:?}"));
        seen += 1;
    }
    assert!(seen >= 4);
    let text = std::fs::read(dir.join("grey-text.png")).expect("seed");
    let lines = crate::image_match::lines(
        &crate::image_match::coverage(&decode(&text).expect("decode")).expect("ink"),
    );
    assert_eq!(lines.iter().map(Vec::len).collect::<Vec<_>>(), [3]);
}
