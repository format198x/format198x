# format198x-commodore-c64-t64

Read and write **Commodore 64 T64 tape containers** in Rust.

A T64 is a file container, not datasette pulses: a 64-byte header, a directory of 32-byte entries with load addresses and offsets, then the payloads. Entries with a wrong or zero end address, common in real files, have their length recovered as VICE does. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_commodore_c64_t64::{T64Program, encode, extract_programs};

let program = T64Program { name: "HELLO".to_owned(), start_address: 0x0801, end_address: 0x0803, data: vec![1, 2] };
let image = encode("MY TAPE", std::slice::from_ref(&program))?;
assert_eq!(extract_programs(&image)?, vec![program]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-commodore-c64-t64` crate (emu198x/emu198x@63fac19e77), whose history of 7 commits stays in that repository.

## Licence

GPL-2.0-or-later.
