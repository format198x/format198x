# format198x-nintendo-nes-ines

Read and write **NES iNES and NES 2.0 ROM files** in Rust.

An iNES file is a 16-byte header, an optional 512-byte trainer, then the PRG
ROM and CHR ROM back to back. The header names the mapper board, the
nametable mirroring and whether the board has a battery, and says how much ROM
follows. NES 2.0 keeps the layout and widens the mapper number and the ROM
sizes. The crate is dependency-free and returns typed errors for malformed
input.

A parsed file keeps its header bytes and anything after the CHR ROM, so it
writes back byte for byte. `encode` writes a new file from its parts.

```rust
use format198x_nintendo_nes_ines::{encode, parse, HeaderMirroring, NewImage};

let prg = vec![0xEA; 32 * 1024];
let chr = vec![0x00; 8 * 1024];
let file = encode(&NewImage {
    mapper_number: 0,
    mirroring: HeaderMirroring::Vertical,
    has_battery: false,
    nes_2_0: false,
    trainer: None,
    prg_rom: &prg,
    chr_rom: &chr,
})?;

let image = parse(&file)?;
assert_eq!(image.prg_rom.len(), 32 * 1024);
assert_eq!(image.to_bytes()?, file);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The crate reads and writes the file only. Which board a mapper number means,
and how that board behaves, belongs to the emulator.

Layouts: the format author's specification at
<https://fms.komkon.org/EMUL8/NES.html>, and
`reference/by-system/nintendo-nes/2026-nes2-rom-size-observations.md` in the
198x reference library for NES 2.0 sizes.

## Provenance

Graduated from Emu198x's `format-nintendo-nes-ines` crate
(emu198x/emu198x@456ae57b7), whose history of 31 commits stays in that
repository.

## Licence

GPL-2.0-or-later.
