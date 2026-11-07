# format198x-commodore-c64-d81

Read and write **Commodore 1581 D81 disk images** in Rust.

A D81 is the flat run of 3,200 256-byte blocks the 1581 presents: 80 tracks of 40 sectors. The crate reads the directory and PRG file chains reads and writes single sectors, and masters fresh disks holding PRG files. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_commodore_c64_d81::{D81Program, extract_first_prg, master};
let program = D81Program { name: "HELLO".into(), data: vec![1, 8, 0], blocks: 0 };
let disk = master("MY DISK", "01", &[program])?;
assert_eq!(extract_first_prg(&disk)?.data, [1, 8, 0]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`master` creates a standard 819,200-byte DOS `3D` image, with closed PRG
entries in the supplied order. It includes every payload byte (including
the load address), recomputes block counts, and reserves track 40 from
file data. Names use D64's restricted ASCII contract (up to 16 letters,
digits or spaces); IDs are two ASCII letters or digits. The existing reader trims trailing
spaces from names. It creates only
the root directory; it does not edit existing images, create REL files or
create partitions.

Layout authority: Commodore **1581 Users Guide**, appendix C, pp. 119–120:
header on track 40 sector 0, two BAM sectors and directory sectors 3–39.
Independent qualification uses VICE 3.10 c1541 in both directions, including
validation of clean and deliberately damaged allocation maps. Run it:
`cargo test -p format198x-commodore-c64-d81 --test c1541 -- --ignored`.
Set `C1541` if the executable is not on PATH.

## Provenance

Graduated from Emu198x's `format-commodore-c64-d81` crate (emu198x/emu198x@63fac19e77), whose history of 2 commits stays in that repository.

## Licence

GPL-2.0-or-later.
