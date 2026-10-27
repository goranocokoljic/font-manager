//! Fuzzes the PNG decoder of find-by-image (E33.1): any bytes, read as a
//! PNG and then searched for glyphs, must give a result, never a panic.
//!
//! From `crates/fm-core`, seeded with the committed PNGs, for 10 minutes:
//!
//! ```text
//! cargo +nightly fuzz run png_decode fuzz/corpus/png_decode fuzz/seeds/png_decode -- -max_total_time=600 -max_len=65536
//! ```
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = fm_core::preview::png_decode::decode(data);
    let _ = fm_core::image_match::fingerprint_png(data);
});
