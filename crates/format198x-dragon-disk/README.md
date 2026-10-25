# format198x-dragon-disk

Read and write **Dragon VDK disk images** in Rust.

A VDK is a small `dk` header followed by raw 256-byte DragonDOS sectors in track order. The crate is dependency-free and returns typed errors for malformed input.

## Provenance

Graduated from Emu198x's `format-dragon-disk` crate (emu198x/emu198x@63fac19e77), whose history of 9 commits stays in that repository.

## Licence

GPL-2.0-or-later.
