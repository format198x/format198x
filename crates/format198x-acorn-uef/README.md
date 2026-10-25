# format198x-acorn-uef

Read and write **Acorn UEF tape images** in Rust.

UEF is the chunked container used for BBC Micro, Acorn Electron and Acorn Atom
tapes: a `UEF File!` magic, a version, then id-and-length chunks. The crate
reads and writes the container losslessly, keeping chunks it does not know, and
reads and builds the common tape chunks (data blocks, carrier tones, gaps,
base-frequency and baud-rate changes). It is dependency-free and returns typed
errors.

```rust
use format198x_acorn_uef::{Chunk, TapeChunk, Uef};

let mut tape = Uef::new(10, 0);
tape.chunks = vec![Chunk::baud_rate(300), Chunk::carrier(2400), Chunk::implicit_data(b"*")];
let bytes = tape.encode()?;
let back = Uef::parse(&bytes)?;
assert_eq!(back.chunks[1].tape()?, Some(TapeChunk::Carrier { cycles: 2400 }));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Two things are deliberately left to the caller. UEF files are often gzipped as
a whole; that wrapper is gzip rather than UEF, so `Uef::parse` reports
`UefError::Gzipped` and the caller inflates. Turning chunks into a cassette
waveform at a machine's clock is the emulator's business.

## Provenance

The container half of Emu198x's `format-acorn-uef` crate
(emu198x/emu198x@1d70a0a216), whose chunk handling was ported from MAME's
`uef_cas.cpp`. The waveform synthesis, the gzip layer and the Atom `SAVE`
writer stay in Emu198x as an adapter over this crate.

## Licence

GPL-2.0-or-later.
