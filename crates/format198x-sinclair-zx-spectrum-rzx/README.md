# format198x-sinclair-zx-spectrum-rzx

Read and write **ZX Spectrum RZX input recordings** in Rust.

RZX records every `IN` a Spectrum program makes, frame by frame, against a
starting snapshot, so a player can replay a session exactly. The crate keeps
every block in file order — creator, snapshot, input recording and any it does
not know — so files with several snapshots write back as read. It decodes and
encodes frame streams, including the "same as previous frame" marker. It is
dependency-free and returns typed errors.

```rust
use format198x_sinclair_zx_spectrum_rzx::{Block, Frame, InputBlock, Rzx, SnapshotBlock};

let frames = vec![Frame { fetch_count: 100, inputs: vec![0xBF] }];
let rzx = Rzx {
    major: 0,
    minor: 13,
    flags: 0,
    blocks: vec![
        Block::Snapshot(SnapshotBlock::embedded(*b"sna\0", vec![0; 49179])?),
        Block::Input(InputBlock::from_frames(0, &frames)?),
    ],
};
let back = Rzx::parse(&rzx.encode()?)?;
let Some(Block::Input(input)) = back.blocks.get(1) else { unreachable!() };
assert_eq!(input.frames()?, frames);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Snapshot data and frame streams may be zlib-compressed. Having no
dependencies, the crate keeps those bytes as stored and reports whether they
are compressed; inflate them yourself and pass a stream to `decode_frames`.

## Provenance

Graduated from Emu198x's `format-sinclair-zx-spectrum-rzx` crate
(emu198x/emu198x@63fac19e77).

## Licence

GPL-2.0-or-later.
