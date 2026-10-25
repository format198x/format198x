# format198x-acorn-uef

Read and write **Acorn UEF tape images** in Rust.

UEF is the chunked container used for BBC Micro, Acorn Electron and Acorn Atom
tapes: a `UEF File!` magic, a version, then id-and-length chunks. The crate
reads and writes the container losslessly, keeping chunks it does not know, and
reads and builds the common tape chunks (data blocks, carrier tones, gaps,
base-frequency and baud-rate changes). Gzipped images, the usual way UEF files
are distributed, are read and written too. Its one dependency is the family's
own `format198x-deflate`, and it returns typed errors.

```rust
use format198x_acorn_uef::{Chunk, TapeChunk, Uef};

let mut tape = Uef::new(10, 0);
tape.chunks = vec![Chunk::baud_rate(300), Chunk::carrier(2400), Chunk::implicit_data(b"*")];
let bytes = tape.encode()?;
let back = Uef::parse(&bytes)?;
assert_eq!(back.chunks[1].tape()?, Some(TapeChunk::Carrier { cycles: 2400 }));

// Gzipped, as most UEF files are: `parse` inflates it.
let gz = tape.encode_gzipped(format198x_acorn_uef::deflate::Level::BEST)?;
assert_eq!(Uef::parse(&gz)?, tape);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Uef::parse_with` takes `deflate::Options`, to cap how large a gzipped image
may grow. Turning chunks into a cassette waveform at a machine's clock is the
emulator's business.

## Provenance

The container half of Emu198x's `format-acorn-uef` crate
(emu198x/emu198x@1d70a0a216), whose chunk handling was ported from MAME's
`uef_cas.cpp`. The waveform synthesis and the Atom `SAVE` writer stay in
Emu198x as an adapter over this crate. Emu198x inflated with `flate2`; this
crate uses `format198x-deflate`.

## Licence

GPL-2.0-or-later.
