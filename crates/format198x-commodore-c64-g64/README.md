# format198x-commodore-c64-g64

Read and write **Commodore 1541 G64 raw-GCR disk images** in Rust.

A G64 stores each half-track's raw GCR bitstream with its speed zone, so copy-protected disks a D64 cannot represent survive intact. The crate is dependency-free and returns typed errors for malformed input.

## Provenance

Graduated from Emu198x's `format-commodore-c64-g64` crate (emu198x/emu198x@63fac19e77), whose history of 5 commits stays in that repository.

## Licence

GPL-2.0-or-later.
