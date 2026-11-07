# format198x-commodore-c128-d71

Read and write **Commodore 1571 D71 disk images** in Rust.

A D71 is a double-sided D64: 70 logical tracks of decoded 1541-zone sectors. The crate reads the directory and PRG file chains reads and writes single sectors, and masters fresh disks holding PRG files. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_commodore_c128_d71::{D71Program, extract_first_prg, master};
let program = D71Program { name: "HELLO".into(), data: vec![1, 8, 0], blocks: 0 };
let disk = master("MY DISK", "01", &[program])?;
assert_eq!(extract_first_prg(&disk)?.data, [1, 8, 0]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`master` creates a standard 349,696-byte double-sided DOS `2A` image, with
closed PRG entries in the supplied order. It includes every payload byte
(including the load address), recomputes block counts, and reserves tracks
18 and 53 from file data. Names use D64's restricted ASCII contract (up to
16 letters, digits or spaces); IDs are two ASCII letters or digits. The existing reader trims trailing
spaces from names.
It does not edit existing images or create REL files.

Layout authority: Commodore **1571 Users Guide**, DOS diskette format
appendix, including the separate side-two BAM on track 53. Independent
qualification uses VICE 3.10 c1541 in both directions, including validation
of clean and deliberately damaged allocation maps. Run it explicitly:
`cargo test -p format198x-commodore-c128-d71 --test c1541 -- --ignored`.
Set `C1541` if the executable is not on PATH.

## Provenance

Graduated from Emu198x's `format-commodore-c64-d71` crate (emu198x/emu198x@63fac19e77), whose history of 2 commits stays in that repository. It is named for the C128 rather than the C64 because the 1571 is the C128's drive and D71 images are C128 media; Emu198x filed it under the C64.

## Licence

GPL-2.0-or-later.
