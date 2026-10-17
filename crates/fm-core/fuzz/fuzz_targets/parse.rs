//! Fuzzes `fm_core::parser::parse`: any bytes must give a result, never a panic.
//!
//! From `crates/fm-core`, seeded with the fixture corpus, for 10 minutes:
//!
//! ```text
//! cargo +nightly fuzz run parse fuzz/corpus/parse ../../fixtures/fonts -- -max_total_time=600 -max_len=1048576
//! ```
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = fm_core::parser::parse(data);
});
