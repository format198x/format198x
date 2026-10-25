# format198x-sinclair-zx81-p

Read and write **Sinclair ZX81 `.p` tape images** in Rust.

A `.p` / `.p81` / `.81` file is the RAM the ZX81's `SAVE` writes, from `VERSN` at `$4009` up to `E_LINE`. Two thirds of preserved images carry slack past `E_LINE`, so the check is that `E_LINE` lands inside the image, not that it ends it. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_sinclair_zx81_p::Zx81Image;

let mut ram = vec![0u8; 0x40];
ram[0x0B..0x0D].copy_from_slice(&0x4029u16.to_le_bytes());
let image = Zx81Image::from_ram(&ram)?;
assert_eq!(image.program().len(), 0x20);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Provenance

Graduated from Emu198x's `format-sinclair-zx81-p` crate (emu198x/emu198x@1d70a0a216), whose history of 43 commits stays in that repository.

## Licence

GPL-2.0-or-later.
