# format198x-commodore-c64-art-studio

Decode and encode Commodore 64 **OCP Art Studio** (`.art`) hires bitmaps in
Rust — the VIC-II standard-bitmap-mode format Art Studio saves natively.
Dependency-free (`core`/`std` only), deterministic, and panic-free on
malformed input.

It handles a 320×200, 1-bit-per-pixel bitmap with a per-cell colour pair
(screen RAM), the `$2000` load address, and the format's 7-byte trailing pad.

## Decode an image

```rust
use format198x_commodore_c64_art_studio::decode;

let bytes = std::fs::read("picture.art")?;
let image = decode(&bytes)?;
let color = image.color_index(0, 0); // Option<u8>, resolved 4-bit colour
```

## Encode an image

```rust
use format198x_commodore_c64_art_studio::{encode, ArtStudio};

let mut image = ArtStudio::blank();
image.screen_ram[0] = 0x16; // set-pixel colour red, clear-pixel colour black
let bytes = encode(&image)?; // canonical 9,009 bytes, $2000-prefixed, zero-padded
std::fs::write("picture.art", &bytes)?;
```

## Notes

- **Cell-major bitmap.** The bitmap uses the same VIC-II fetch order as
  Koala Painter: 8 consecutive bytes per 8×8 cell, 40 cells per cell row.
  [`bitmap_offset`] computes the byte offset for a pixel; use
  [`ArtStudio::pixel`] or [`ArtStudio::color_index`] rather than indexing the
  bitmap directly.
- **Hires colour resolution.** Each bitmap bit selects one of two colours
  per cell from screen RAM: a set bit resolves to the upper nybble, a clear
  bit to the lower nybble (standard bitmap mode, VIC-II synthesis § 5).
- **Trailing pad, accepted either way.** A canonical file is 9,009 bytes —
  load address, bitmap, screen RAM, then 7 filler bytes carrying no image
  data. [`decode`] accepts the file with or without that pad (9,002 to 9,009
  bytes) and ignores whatever the trailing bytes hold; [`encode`] always
  emits the canonical length, zero-filling the pad. This is covered by
  `decode_accepts_padless_and_canonical_lengths_and_ignores_the_pad` in the
  test suite.
- **Fixed size range, checked signature.** Anything outside 9,002–9,009 bytes,
  or a load address other than `$2000`, is a typed [`DecodeError`], never a
  panic.

## Part of the 198x family

This crate powers the Art Studio conversion step of [Build198x], the
build-tools pipeline for the [198x] retro-computing project.

## Licence

GPL-2.0-or-later. The 198x emulator/tooling family is copyleft throughout — it
composes freely with the GPL retro-computing ecosystem most C64 tooling lives
in. If you build on this crate, your work inherits those terms.

[198x]: https://github.com/build198x
[Build198x]: https://github.com/build198x/build198x
