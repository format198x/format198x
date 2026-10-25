# format198x-commodore-c64-d64

Read and write **Commodore 1541 D64 disk images** in Rust.

A D64 is the 683 decoded 256-byte sectors of a 35-track 1541 disk, optionally followed by an error-info table. The crate reads the BAM and directory, follows file chains, reads and writes single sectors, and masters a freshly formatted disk holding PRG files. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_commodore_c64_d64::{D64Program, extract_first_prg, master};

let program = D64Program { name: "HELLO".to_owned(), data: vec![0x01, 0x08, 0x00], blocks: 0 };
let disk = master("MY DISK", "01", &[program])?;
assert_eq!(extract_first_prg(&disk)?.data, [0x01, 0x08, 0x00]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-commodore-c64-d64` crate (emu198x/emu198x@63fac19e77), whose history of 7 commits stays in that repository.

## Licence

GPL-2.0-or-later.
