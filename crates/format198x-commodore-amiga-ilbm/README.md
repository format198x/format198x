# format198x-commodore-amiga-ilbm

Decode and encode Commodore Amiga **IFF/ILBM** images in Rust — the
EA-IFF-85 `FORM ILBM` chunk container Amiga paint programs (DPaint and
others) save natively. Dependency-free (`core`/`std` only), deterministic,
and panic-free on malformed input.

It handles interleaved bitplane images in chunky indexed form: the `BMHD`
header, an optional `CMAP` palette, the `CAMG` viewmode longword, and a
`BODY` of per-plane scanlines, either raw or ByteRun1 (PackBits) packed.

## Decode an image

```rust
use format198x_commodore_amiga_ilbm::decode;

let bytes = std::fs::read("picture.iff")?;
let image = decode(&bytes)?;
let pixel_index = image.pixels[0]; // chunky indexed, row-major
```

## Encode an image

```rust
use format198x_commodore_amiga_ilbm::{encode, Compression, Ilbm};

let image = Ilbm {
    width: 320,
    height: 200,
    n_planes: 5,
    palette: vec![[0, 0, 0], [0xFF, 0xFF, 0xFF]],
    pixels: vec![0; 320 * 200],
    camg: 0,
    x_aspect: 10, // lores PAL
    y_aspect: 11,
};
let bytes = encode(&image, Compression::ByteRun1)?; // FORM ILBM byte stream
std::fs::write("picture.iff", &bytes)?;
```

Unlike the other 198x format crates, [`encode`] takes a
[`Compression`] argument: the caller chooses raw or ByteRun1 scanlines per
call, rather than the format carrying only one option.

## Notes

- **Chunked container, not a fixed dump.** An IFF file is a tree of 4-byte-ID,
  big-endian-length chunks padded to even length. `FORM` is the outermost
  chunk; its first four payload bytes are the form type (`ILBM` here). This
  codec's errors reflect that: there is no `DecodeError::WrongLength` because
  there is no single fixed file length to check — see [`DecodeError`].
- **Chunky, not planar, at the API boundary.** [`Ilbm::pixels`] holds one
  index per pixel, row-major. `encode`/`decode` do the interleave into (and
  out of) per-plane scanlines, word-padded per the ILBM row format —
  [`row_bytes`] computes a scanline's padded length for a given width.
- **ByteRun1 (PackBits).** `compression = 1` packs each plane scanline
  independently: a non-negative control byte copies `n + 1` literal bytes, a
  negative one repeats the next byte `1 - n` times. This crate's packer only
  emits a run for 3+ repeated bytes (the spec's documented break-even point)
  and never emits the `-128` no-op, but any conforming unpacker's output
  decodes correctly regardless of its packing choices.
- **CAMG carried, not interpreted.** The viewmode longword round-trips
  verbatim; [`CAMG_LACE`] and [`CAMG_HIRES`] name the two bits this crate
  documents, but any other bits are preserved too.
- **Tolerant decode, strict on corruption.** Unknown chunks are skipped, a
  missing `CMAP` yields an empty palette, `masking = 1` mask scanlines are
  skipped, and bytes after the FORM's declared end are ignored. Bad magic,
  truncated chunks, out-of-range `nPlanes`/compression/masking, oversized
  dimensions, and ByteRun1 runs that overrun their scanline are all typed
  [`DecodeError`]s, never panics.
- **netpbm cross-checks.** Beyond the golden-fixture and round-trip tests,
  `tests/netpbm.rs` checks this codec against the `netpbm` package's
  `ppmtoilbm`/`ilbmtoppm` in both directions. Those tests are `#[ignore]`d —
  validation-tier, not part of the default suite — and skip gracefully when
  the tools aren't on `PATH`. Run them with
  `cargo test -p format198x-commodore-amiga-ilbm -- --ignored`.

## Part of the 198x family

This crate powers the Amiga image-conversion step of [Build198x], the
build-tools pipeline for the [198x] retro-computing project.

## Licence

GPL-2.0-or-later. The 198x emulator/tooling family is copyleft throughout — it
composes freely with the GPL retro-computing ecosystem most Amiga tooling
lives in. If you build on this crate, your work inherits those terms.

[198x]: https://github.com/build198x
[Build198x]: https://github.com/build198x/build198x
