# format198x-commodore-c64-d81

Read and write **Commodore 1581 D81 disk images** in Rust.

A D81 is the flat run of 3,200 256-byte blocks the 1581 presents: 80 tracks of 40 sectors. The crate reads the directory and PRG file chains and reads and writes single sectors. The crate is dependency-free and returns typed errors for malformed input.

File-level writing (mastering a disk with files, as the D64 crate does) is not here yet; sector writes are.

## Provenance

Graduated from Emu198x's `format-commodore-c64-d81` crate (emu198x/emu198x@63fac19e77), whose history of 2 commits stays in that repository.

## Licence

GPL-2.0-or-later.
