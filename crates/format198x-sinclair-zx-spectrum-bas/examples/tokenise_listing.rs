//! Tokenise a numbered listing and write the stored program bytes to stdout,
//! as `tokenise_listing` returns them: each line's number, length, body and
//! `0x0D`, with no tape header or variables area.
//!
//! ```text
//! cargo run --example tokenise_listing -- <listing.bas> > program.bin
//! ```
//!
//! `tests/fixtures/rom-editor/capture.mjs` uses it to load this crate's bytes
//! into the emulator and run them on the genuine ROM.

use format198x_sinclair_zx_spectrum_bas::tokenise_listing;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: tokenise_listing <listing.bas>")?;
    let source = std::fs::read_to_string(path)?;
    let program = tokenise_listing(&source)?;
    std::io::stdout().write_all(&program.bytes)?;
    Ok(())
}
