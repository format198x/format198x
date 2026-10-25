# format198x-sinclair-zx80-o

Read and write **Sinclair ZX80 `.o` tape images** in Rust.

A `.o` / `.80` file is exactly the RAM the ZX80's `SAVE` writes: `$4000` up to the end pointer it holds at offset `$0A`. That pointer is the format's only integrity check, and it is applied. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_sinclair_zx80_o::Zx80Image;

let mut ram = vec![0u8; 0x40];
ram[0x0A..0x0C].copy_from_slice(&0x4020u16.to_le_bytes());
let image = Zx80Image::from_ram(&ram)?;
assert_eq!(image.encode().len(), 0x20);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-sinclair-zx80-o` crate (emu198x/emu198x@7e4fb5b666), whose history of 2 commits stays in that repository.

## Licence

GPL-2.0-or-later.
