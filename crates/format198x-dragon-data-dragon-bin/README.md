# format198x-dragon-data-dragon-bin

Read and write **DragonDOS BIN program files** in Rust.

A DragonDOS `.BIN` is a nine-byte header — `$55`, file type, big-endian load address, length and execution address, `$AA` — followed by the machine code. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_dragon_data_dragon_bin::{DRAGON_BIN_MACHINE_CODE_TYPE, DragonBinImage, encode_dragon_bin, parse_dragon_bin};

let image = DragonBinImage {
    file_type: DRAGON_BIN_MACHINE_CODE_TYPE,
    load_address: 0x3000,
    exec_address: 0x3000,
    payload: vec![0x39],
};
assert_eq!(parse_dragon_bin(&encode_dragon_bin(&image)?)?, image);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-dragon-bin` crate (emu198x/emu198x@63fac19e77), whose history of 6 commits stays in that repository. The name carries the manufacturer, Dragon Data, as well as the machine, as `{manufacturer}-{system}` requires; Emu198x's name had only the machine.

## Licence

GPL-2.0-or-later.
