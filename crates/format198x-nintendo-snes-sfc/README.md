# format198x-nintendo-snes-sfc

Read and write **Super NES and Super Famicom cartridge images** (`.sfc`,
`.smc`) in Rust.

A cartridge image is the ROM in the order the board wires it, sometimes
behind a 512-byte header a copier added. Inside the ROM, a 64-byte internal
header names the game, the board's map mode and chips, the ROM and RAM sizes,
the region, a checksum and the 65C816's vectors. Nothing in the file says
where that header is, so the parser scores the LoROM, HiROM and ExHiROM
locations and takes the best. The crate is dependency-free and returns typed
errors for malformed input.

A parsed image writes back byte for byte, copier header included.
`set_checksum` makes a changed ROM's checksum pair match it again.

```rust
use format198x_nintendo_snes_sfc::{parse, set_checksum, MapMode};

let mut rom = vec![0u8; 0x40000];
let header = MapMode::LoRom.header_offset();
rom[header..header + 21].copy_from_slice(b"FORMAT198X EXAMPLE   ");
rom[header + 0x15] = 0x20; // LoROM
rom[header + 0x3C..header + 0x3E].copy_from_slice(&0x8000u16.to_le_bytes());
let checksum = set_checksum(&mut rom, MapMode::LoRom)?;

let image = parse(&rom)?;
assert_eq!(image.header.map, MapMode::LoRom);
assert_eq!(image.header.title, "FORMAT198X EXAMPLE");
assert_eq!(image.header.checksum, checksum);
assert_eq!(image.to_bytes(), rom);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The crate reads and writes the file only. How a board decodes the address
bus, and what its RAM and coprocessors do, belongs to the emulator.

Layout: `reference/by-system/nintendo-snes/` in the 198x reference library.

## Provenance

Graduated from Emu198x's `format-nintendo-snes-sfc` crate
(emu198x/emu198x@84b5031a2), whose history of 1 commit stays in that
repository.

## Licence

GPL-2.0-or-later.
