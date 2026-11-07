# format198x-atari-st-msa

Read and write **Atari ST `.msa` floppy images** (Magic Shadow Archiver) in
Rust.

An MSA file is a ten-byte header giving the sectors per track, the sides and
the first and last track, then one block per track and side. A block is
either the raw track or a run-length encoding of it, where `$E5` introduces a
run. `parse` expands an archive to raw sectors in `.st` order, and `write`
compresses them back. The crate is dependency-free and returns typed errors.

```rust
use format198x_atari_st_msa::{parse, write, SECTOR_SIZE};

let sectors = vec![0u8; 80 * 2 * 9 * SECTOR_SIZE]; // a blank 720K disk
let archive = write(9, 2, &sectors)?;
assert!(archive.len() < sectors.len());

let image = parse(&archive)?;
assert_eq!((image.sectors_per_track, image.sides, image.tracks()), (9, 2, 80));
assert_eq!(image.data, sectors);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Archives written by Hatari's `hmsa` expand correctly here, and `hmsa`
expands this crate's archives correctly. An ignored test checks both
directions over real disk images.

## Provenance

Graduated from Emu198x's `format-atari-st-msa` crate
(emu198x/emu198x@adbc5752f), whose history of 1 commit stays in that
repository.

## Licence

GPL-2.0-or-later.
