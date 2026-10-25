# format198x-sinclair-zx-spectrum-z80

Read and write **ZX Spectrum `.z80` snapshots** in Rust.

Versions 1, 2 and 3 are covered, with the `ED ED` run-length compression they
share. A snapshot keeps the registers, border, flags, the version-specific
extension bytes and the decompressed 16K pages, so it writes back to an
equivalent file. The hardware mode is read with the published table for the
snapshot's version. The crate is dependency-free and returns typed errors.

```rust
use format198x_sinclair_zx_spectrum_z80::{Machine, PAGE_LEN, Registers, Version, Z80Snapshot};

let snapshot = Z80Snapshot {
    version: Version::V1,
    registers: Registers { pc: 0x8000, sp: 0xFF00, im: 1, ..Registers::default() },
    border: 7,
    samrom: false,
    emulator_flags: 0,
    extension: Vec::new(),
    pages: [8, 4, 5].iter().map(|&n| (n, vec![0; PAGE_LEN])).collect(),
};
let bytes = snapshot.encode(true)?;
let back = Z80Snapshot::parse(&bytes)?;
assert_eq!(back.machine(), Machine::Spectrum48K);
assert_eq!(back, snapshot);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-sinclair-zx-spectrum-z80` crate
(emu198x/emu198x@1d70a0a216). Emu198x parsed straight into its shared
machine-state `Snapshot` type and mapped hardware modes to its own models;
this crate keeps the file's own fields, adds the writer and follows the
published hardware table.

## Licence

GPL-2.0-or-later.
