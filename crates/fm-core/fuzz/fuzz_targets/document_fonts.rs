//! Fuzzes `fm_core::document_fonts::read` (E32.1): any bytes, read as every
//! document format, must give a result, never a panic. The zip, XML, kiwi,
//! XMP/PDF and Photoshop readers all sit behind it.
//!
//! From `crates/fm-core`, seeded with the document fixtures, for 10 minutes:
//!
//! ```text
//! cargo +nightly fuzz run document_fonts fuzz/corpus/document_fonts ../../fixtures/documents -- -max_total_time=600 -max_len=262144
//! ```
#![no_main]

use std::io::Cursor;

use fm_api::workflows::DocumentFormat;
use libfuzzer_sys::fuzz_target;

const FORMATS: [DocumentFormat; 7] = [
    DocumentFormat::Docx,
    DocumentFormat::Pptx,
    DocumentFormat::Sketch,
    DocumentFormat::Fig,
    DocumentFormat::Indd,
    DocumentFormat::Ai,
    DocumentFormat::Psd,
];

fuzz_target!(|data: &[u8]| {
    for format in FORMATS {
        let _ = fm_core::document_fonts::read(format, &mut Cursor::new(data));
    }
});
