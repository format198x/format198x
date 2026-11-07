# format198x-sinclair-zx-spectrum-mdr

Read and write **ZX Spectrum Microdrive cartridge images** (`.mdr`) in Rust.

A Microdrive cartridge is an endless loop of tape holding up to 254 sectors.
An `.mdr` file stores them in tape order, 543 bytes each: a sector header, a
record header, 512 bytes of data and a data checksum. A write-protect byte
follows. The preambles and gaps the drive writes are left out. The crate is
dependency-free and returns typed errors.

```rust
use format198x_sinclair_zx_spectrum_mdr::{MdrImage, SECTOR_LEN};

let cartridge = MdrImage::blank(180);
let file = cartridge.to_bytes();
assert_eq!(file.len(), 180 * SECTOR_LEN + 1);

let image = MdrImage::parse(&file)?;
assert_eq!(image.sector_count(), 180);
assert!(!image.write_protected);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`to_bytes` always writes the write-protect byte, as `0` or `1`. A file that
lacked it, or stored another non-zero value, is written back in that form.

The layout, length rules and write-protect sense follow libspectrum's
`microdrive.c`, which is what Fuse reads and writes. The checksum follows the
Interface 1 ROM. An ignored test checks real cartridges against both.

## Provenance

Graduated from Emu198x's `format-sinclair-zx-spectrum-mdr` crate
(emu198x/emu198x@663fbacd5), whose history of 1 commit stays in that
repository.

## Licence

GPL-2.0-or-later.
