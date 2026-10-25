# format198x-commodore-c64-tap

Read and write **Commodore TAP datasette pulse images** in Rust.

A Commodore TAP (`C64-TAPE-RAW` or `C16-TAPE-RAW`) stores the datasette's pulse lengths, in short one-byte or extended three-byte form. The crate reads them as durations in machine cycles and writes them back. The crate is dependency-free and returns typed errors for malformed input.

## Provenance

Graduated from Emu198x's `format-commodore-c64-tap` crate (emu198x/emu198x@63fac19e77), whose history of 7 commits stays in that repository.

## Licence

GPL-2.0-or-later.
