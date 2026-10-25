# format198x-commodore-c64-p00

Read and write **Commodore PC64 P00 containers** in Rust.

A P00 (and its `.S00`/`.U00`/`.R00` siblings) is a 26-byte `C64File` header with the original 16-byte CBM name and REL record size, then the file body — for a PRG, the load address and data. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_commodore_c64_p00::{P00File, encode, parse};

let file = P00File { name: "GAME".to_owned(), record_size: 0, data: vec![0x01, 0x08, 0x60] };
assert_eq!(parse(&encode(&file)?)?, file);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-commodore-c64-p00` crate (emu198x/emu198x@63fac19e77), whose history of 2 commits stays in that repository.

## Licence

GPL-2.0-or-later.
