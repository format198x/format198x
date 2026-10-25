# format198x-sinclair-zx-spectrum-szx

Read and write **ZX Spectrum `.szx` (ZX-State) snapshots** in Rust.

SZX is Spectaculator's chunked snapshot format: a `ZXST` header with a
machine id, then four-character chunks in any order. The crate keeps every
chunk as stored, so a snapshot writes back byte for byte, and reads and builds
the chunks that hold a machine's core state: `Z80R`, `SPCR`, `RAMP` and
`AY`. It is dependency-free and returns typed errors.

```rust
use format198x_sinclair_zx_spectrum_szx::{Chunk, PAGE_LEN, Szx, Z80Registers};

let mut szx = Szx::new(1, 4, 1, 0); // version 1.4, 48K
szx.chunks.push(Chunk::from_z80_registers(&Z80Registers { pc: 0x8000, ..Default::default() }));
szx.chunks.push(Chunk::uncompressed_ram_page(5, &[0; PAGE_LEN])?);
let back = Szx::parse(&szx.encode()?)?;
assert_eq!(back.z80_registers()?.map(|r| r.pc), Some(0x8000));
assert_eq!(back.ram_pages()?[0].bank, 5);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`RAMP` pages may be zlib-compressed. Having no dependencies, the crate keeps
them as stored and reports whether they are compressed; inflate them yourself.
Pages it writes are uncompressed, which the format allows.

## Provenance

Graduated from Emu198x's `format-sinclair-zx-spectrum-szx` crate
(emu198x/emu198x@1d70a0a216). Emu198x parsed straight into its shared
machine-state `Snapshot` type and inflated pages with `flate2`; both stay in
Emu198x as an adapter over this crate.

## Licence

GPL-2.0-or-later.
