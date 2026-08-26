# format-sinclair-zx-spectrum-scr

Decode and encode Sinclair ZX Spectrum **SCR** screen dumps in Rust — the
raw 6,912-byte display file the Spectrum's ULA reads directly from memory.
Dependency-free (`core`/`std` only), deterministic, and panic-free on
malformed input.

It handles the full display file: a 256×192, 1-bit-per-pixel bitmap plus a
24×32 attribute table (INK, PAPER, BRIGHT, FLASH per 8×8 cell), including the
ULA's interleaved bitmap byte order.

## Decode a screen

```rust
use format_sinclair_zx_spectrum_scr::decode;

let bytes = std::fs::read("screen.scr")?;
let screen = decode(&bytes)?;
let top_left_pixel_is_set = screen.pixel(0, 0); // Option<bool>
```

## Encode a screen

```rust
use format_sinclair_zx_spectrum_scr::{encode, Screen};

let mut screen = Screen::blank();
screen.attributes[0] = 0x47; // BRIGHT, white INK, black PAPER
let bytes = encode(&screen)?; // exactly 6,912 bytes
std::fs::write("screen.scr", &bytes)?;
```

## Notes

- **Interleaved bitmap.** The file does not store the bitmap in linear
  row-major order. The ULA's memory layout packs it into three 2 KB bands of
  64 pixel rows each, with the rows of one character cell 256 bytes apart —
  file offset `((y & 0xC0) << 5) | ((y & 0x07) << 8) | ((y & 0x38) << 2) | c`
  for pixel row `y`, byte column `c`. [`Screen`] stores the bitmap
  **de-interleaved**, in linear row-major order; `decode`/`encode` apply the
  permutation in each direction. It is a pure permutation, so round-tripping
  through [`Screen`] is lossless.
- **Linear attributes.** Unlike the bitmap, the 768-byte attribute table is
  linear — 24 rows × 32 cells, one byte per cell — starting immediately after
  the bitmap.
- **Fixed size.** An SCR file is exactly 6,912 bytes; anything else is a
  typed [`DecodeError::WrongLength`], never a panic.

## Part of the 198x family

This crate powers the Spectrum screen-conversion step of [Build198x], the
build-tools pipeline for the [198x] retro-computing project.

## Licence

GPL-2.0-or-later. The 198x emulator/tooling family is copyleft throughout — it
composes freely with the GPL retro-computing ecosystem most Spectrum tooling
lives in. If you build on this crate, your work inherits those terms.

[198x]: https://github.com/build198x
[Build198x]: https://github.com/build198x/build198x
