# format198x-commodore-c64-prg

Read and write **Commodore PRG program files** in Rust.

A PRG is a two-byte little-endian load address followed by the bytes that load there. Loading it into a machine — the KERNAL end pointer, relinking BASIC at `$0801` — is machine behaviour and is not here. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_commodore_c64_prg::PrgFile;

let prg = PrgFile::parse(&[0x00, 0xC0, 0xA9, 0x00])?;
assert_eq!(prg.load_address, 0xC000);
assert_eq!(prg.encode(), [0x00, 0xC0, 0xA9, 0x00]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-commodore-c64-prg` crate (emu198x/emu198x@b5f8fe59ce), whose history of 6 commits stays in that repository.

## Licence

GPL-2.0-or-later.
