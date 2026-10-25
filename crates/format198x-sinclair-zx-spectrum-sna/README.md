# format198x-sinclair-zx-spectrum-sna

Read and write **ZX Spectrum `.sna` snapshots** in Rust.

A `.sna` is a 27-byte register block and the 48K of RAM from `$4000`. A 48K
snapshot has no PC field: the PC is pushed onto the stack and `SP` points at
it. A 128K snapshot adds the PC, the `$7FFD` paging byte, a TR-DOS flag and
the remaining RAM banks. The crate keeps the file's own shape, so a snapshot
writes back byte for byte, and is dependency-free with typed errors.

```rust
use format198x_sinclair_zx_spectrum_sna::{RAM_LEN, Registers, Sna};

let registers = Registers { iff2: true, im: 1, sp: 0xFFFE, ..Registers::default() };
let snapshot = Sna::new_48k(registers, 0x8000, 7, vec![0; RAM_LEN])?;
let bytes = snapshot.encode()?;
assert_eq!(Sna::parse(&bytes)?.pc(), Some(0x8000));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Turning a snapshot into a running machine — the shared Spectrum state model,
popping the stacked PC, setting IFF1 from IFF2 — is the emulator's business.

## Provenance

Graduated from Emu198x's `format-sinclair-zx-spectrum-sna` crate
(emu198x/emu198x@1d70a0a216). Emu198x parsed straight into its shared
machine-state `Snapshot` type; this crate has its own file-shaped types
instead, and adds the writer.

## Licence

GPL-2.0-or-later.
