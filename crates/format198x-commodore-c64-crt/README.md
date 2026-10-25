# format198x-commodore-c64-crt

Read and write **Commodore 64 CRT cartridge images** in Rust.

A `.crt` file is the standard C64 cartridge container used by VICE, CCS64 and Hoxs64: a 64-byte header with the hardware type and the reset state of the `EXROM`/`GAME` lines, then one `CHIP` packet per ROM or RAM bank. The crate reads and writes the bytes; how the PLA maps them is the machine's business. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_commodore_c64_crt::{CrtCartridge, CrtChip, CrtMode, encode, parse};

let cart = CrtCartridge {
    hardware_type: 0,
    exrom: true,
    game: false,
    name: "DEMO".to_owned(),
    chips: vec![CrtChip { chip_type: 0, bank: 0, load_address: 0x8000, data: vec![0; 0x2000] }],
};
let back = parse(&encode(&cart)?)?;
assert_eq!(back.mode(), CrtMode::Rom8k);
assert_eq!(back, cart);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-commodore-c64-crt` crate (emu198x/emu198x@63fac19e77), whose history of 2 commits stays in that repository.

## Licence

GPL-2.0-or-later.
