# format198x-spectravideo-svi-cas

Read and write **Spectravideo SVI-318 and SVI-328 CAS cassette images** in Rust.

SVI CAS stores decoded tape bytes. Each block begins with sixteen `$55` bytes
and a `$7f` sync byte. The crate preserves block payloads losslessly, rejects
missing or empty blocks with typed errors, and has no dependencies.

```rust
use format198x_spectravideo_svi_cas::{CasImage, decode, encode};

let bytes = encode(&CasImage::new(vec![vec![0x42, 0x43]])?)?;
let image = decode(&bytes)?;
assert_eq!(image.blocks()[0], [0x42, 0x43]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Evidence

The layout is cross-checked against MAME's `svi_cas.cpp`, including its exact
seventeen-byte block marker and concatenated-block behaviour.

## Licence

GPL-2.0-or-later.
