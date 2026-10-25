# format198x-dragon-data-dragon-cas

Read and write **Dragon CAS cassette images** in Rust.

A Dragon CAS stores the byte stream the ROM's cassette routines see, not audio: `$55` leaders, a `$3C` sync byte, then type, length, payload and checksum per block. A strict parser and a tolerant one that records junk between blocks are both provided; timing reconstruction belongs to the machine. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_dragon_data_dragon_cas::{CasBlock, encode_cas, parse_cas};

let blocks = vec![CasBlock::new(0x01, vec![1, 2, 3], 128), CasBlock::new(0xFF, Vec::new(), 1)];
let image = parse_cas(&encode_cas(&blocks)?)?;
assert!(image.checksums_valid());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-dragon-cas` crate (emu198x/emu198x@63fac19e77), whose history of 7 commits stays in that repository. The name carries the manufacturer, Dragon Data, as well as the machine, as `{manufacturer}-{system}` requires; Emu198x's name had only the machine.

## Licence

GPL-2.0-or-later.
