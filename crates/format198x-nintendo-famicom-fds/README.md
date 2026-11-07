# format198x-nintendo-famicom-fds

Read and write **Famicom Disk System `.fds` images** in Rust.

An `.fds` file is each disk side's data, 65,500 bytes a side, back to back,
sometimes after a 16-byte header the fwNES emulator introduced. A side holds a
sequence of blocks: disk info, the file count, then a header and a data block
for each file. The file keeps only the block contents. The gaps, start marks
and CRCs the drive reads are left out, and putting them back is the
emulator's job. The crate is dependency-free and returns typed errors for
malformed input.

```rust
use format198x_nintendo_famicom_fds::{parse, write, BlockKind, SIDE_LEN};

let mut side = vec![0u8; SIDE_LEN];
side[0] = 1; // a disk info block
side[1..15].copy_from_slice(b"*NINTENDO-HVC*");

let file = write(&[&side], true)?;
let image = parse(&file)?;
assert!(image.has_header);

let scan = image.sides[0].scan();
assert_eq!(scan.blocks[0].kind, BlockKind::DiskInfo);
assert!(scan.blocks[0].disk_info().is_some_and(|info| info.signature_ok));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Layout: the NESdev wiki's "FDS file format" and "FDS disk format" pages.

## Provenance

Graduated from Emu198x's `format-nintendo-famicom-fds` crate
(emu198x/emu198x@6ecd5455b), whose history of 1 commit stays in that
repository.

## Licence

GPL-2.0-or-later.
