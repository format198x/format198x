# format198x-atari-8bit-atr

Read and write **Atari 8-bit ATR disk images** in Rust.

An ATR is a 16-byte header followed by a sector-by-sector floppy dump. The sector count comes from the file length, because 40% of preserved images store the header's size field in the wrong unit, and all three double-density boot-sector layouts (logical, physical, padded) are read and written back as found. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_atari_8bit_atr::AtrImage;

let mut disk = AtrImage::blank(128, 720)?;
disk.write_sector(1, &[0xAA; 128])?;
let bytes = disk.encode();
assert_eq!(AtrImage::parse(&bytes)?.sector(1), Some(&[0xAA; 128][..]));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-atari-8bit-atr` crate (emu198x/emu198x@1d70a0a216), whose history of 29 commits stays in that repository.

## Licence

GPL-2.0-or-later.
