# format198x-atari-st-st

Read and write **Atari ST `.st` floppy images** in Rust.

A `.st` file is a raw dump of a floppy's 512-byte sectors, track by track,
alternating sides. It has no header, so the geometry has to be worked out. The
crate uses the boot sector's BIOS parameter block only when it describes
exactly the image's sectors. Otherwise it goes by the image size: games and
demos often carry a boot sector that does not match their disk. The crate is
dependency-free and returns typed errors.

Nothing is dropped. Bytes past the last whole track are kept, and
`StImage::to_bytes` writes the file back unchanged.

```rust
use format198x_atari_st_st::{blank, parse, Geometry};

let disk = blank(Geometry { tracks: 82, sides: 2, sectors_per_track: 10 });
let file = disk.to_bytes()?;

let image = parse(&file)?;
assert_eq!(image.geometry, disk.geometry);
assert_eq!(image.to_bytes()?, file);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The crate reads and writes the image only. Turning it into a disk a floppy
controller can read belongs to the emulator.

## Provenance

Graduated from Emu198x's `format-atari-st-st` crate
(emu198x/emu198x@adbc5752f), whose history of 1 commit stays in that
repository.

## Licence

GPL-2.0-or-later.
