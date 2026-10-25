# format198x-commodore-vic-20-crt

Read and write **Commodore VIC-20 cartridge images** in Rust.

VICE `.crt` files carry a 64-byte `VIC20 CARTRIDGE` header and address-tagged `CHIP` packets. Raw 2, 4 and 8 KiB images are read as the ordinary BLK5 cartridge at `$A000`. Generic, statically mapped cartridges are supported. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_commodore_vic_20_crt::{Cartridge, CartridgeBlock, encode, parse};

let cart = Cartridge {
    hardware_type: 0,
    name: "DEMO".to_owned(),
    blocks: vec![CartridgeBlock { load_address: 0xA000, data: vec![0; 0x2000] }],
};
assert_eq!(parse(&encode(&cart)?)?, cart);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-commodore-vic-20-crt` crate (emu198x/emu198x@0169259ca4), whose history of 1 commit stays in that repository.

## Licence

GPL-2.0-or-later.
