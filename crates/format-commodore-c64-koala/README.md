# format-commodore-c64-koala

Decode and encode Commodore 64 **Koala Painter** (`.koa`) multicolour
bitmaps in Rust — the VIC-II multicolour-bitmap-mode format Koala Painter
saves natively. Dependency-free (`core`/`std` only), deterministic, and
panic-free on malformed input.

It handles the full 10,003-byte file: the `$6000` load address, an
8,000-byte cell-major bitmap, 1,000 bytes of screen RAM, 1,000 bytes of
colour RAM, and a trailing background-colour byte.

## Decode an image

```rust
use format_commodore_c64_koala::decode;

let bytes = std::fs::read("picture.koa")?;
let image = decode(&bytes)?;
let color = image.color_index(0, 0); // Option<u8>, resolved 4-bit colour
```

## Encode an image

```rust
use format_commodore_c64_koala::{encode, Koala};

let mut image = Koala::blank();
image.background = 0x06; // blue
let bytes = encode(&image)?; // exactly 10,003 bytes, $6000-prefixed
std::fs::write("picture.koa", &bytes)?;
```

## Notes

- **Cell-major bitmap.** The bitmap is stored exactly as the VIC-II fetches
  it, not row-major: 8 consecutive bytes per 8×8 cell, 40 cells per cell
  row. [`bitmap_offset`] computes the byte offset for a pixel; use
  [`Koala::bit_pair`] or [`Koala::color_index`] rather than indexing the
  bitmap directly.
- **Multicolour bit pairs.** Each bitmap byte holds four double-wide pixels
  (bits 7–6 leftmost). The bit-pair colour source table — `%00` background,
  `%01`/`%10` screen RAM nybbles, `%11` colour RAM nybble — is reference-backed
  from the VIC-II synthesis; the container layout (load address, section
  order, file length) is Koala Painter's well-documented native save format.
- **Verbatim colour RAM.** Only the low nybble of each colour RAM byte is
  significant on hardware; this codec preserves the upper nybble byte for
  byte rather than masking it, so any file round-trips losslessly.
- **Fixed size, checked signature.** A Koala file is exactly 10,003 bytes and
  always declares the `$6000` load address; anything else is a typed
  [`DecodeError`], never a panic.

## Part of the 198x family

This crate powers the Koala Painter conversion step of [Build198x], the
build-tools pipeline for the [198x] retro-computing project.

## Licence

GPL-2.0-or-later. The 198x emulator/tooling family is copyleft throughout — it
composes freely with the GPL retro-computing ecosystem most C64 tooling lives
in. If you build on this crate, your work inherits those terms.

[198x]: https://github.com/build198x
[Build198x]: https://github.com/build198x/build198x
