# format198x-acorn-adfs

Read and write **Acorn ADFS old-map floppy images** (`.adf`, `.adl`) in Rust.

ADFS shipped on the Acorn Electron's Plus 3, the BBC Master and the BBC Micro
with a 1770 disc interface. Its first discs use the "old map": a two-sector
free-space map, then a five-sector root directory. The S, M and L formats are
40 or 80 tracks of sixteen 256-byte sectors, on one side or two. An `.adl`
image interleaves its two sides track by track. The crate is dependency-free
and returns typed errors.

It works out an image's format, pads a trimmed image to full size, computes
the map checksums, writes a blank formatted disc and reads the root
directory.

```rust
use format198x_acorn_adfs::{blank, detect, root_directory, Format};

let disc = blank(Format::L, "GAMES");
assert_eq!(disc.len(), 655_360);
assert_eq!(detect(&disc)?, Format::L);

let root = root_directory(&disc)?;
assert_eq!((root.name.as_str(), root.title.as_str()), ("$", "GAMES"));
assert!(root.entries.is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Adding files to a directory is not covered yet. An image is its sectors, so
`blank` and the image bytes are the write side for now.

An ignored test checks the map checksums and root directories of real discs
that ADFS wrote.

## Provenance

Graduated from Emu198x's `format-acorn-adfs` crate
(emu198x/emu198x@31e7e1dc2), whose history of 1 commit stays in that
repository.

## Licence

GPL-2.0-or-later.
