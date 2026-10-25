# format198x-acorn-atom-atm

Read and write **Acorn Atom `.atm` program images** in Rust.

An `.atm` file (the Wouter Ras format used by Atomulator and others) is a 22-byte header — a 16-byte name and little-endian load, execution and length words — followed by the program body. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_acorn_atom_atm::{AtmImage, encode, parse};

let image = AtmImage {
    filename: "DEMO".to_owned(),
    load_address: 0x2900,
    exec_address: 0x2900,
    payload: vec![0xA9, 0x42, 0x60],
};
let bytes = encode(&image)?;
assert_eq!(parse(&bytes)?, image);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-acorn-atom-atm` crate (emu198x/emu198x@63fac19e77), whose history of 2 commits stays in that repository.

## Licence

GPL-2.0-or-later.
