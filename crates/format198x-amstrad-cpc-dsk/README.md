# format198x-amstrad-cpc-dsk

Dependency-free CPCEMU standard and extended DSK reading, with a bounded
standard 180 KiB Spectrum +3DOS disk writer. The container is shared by Amstrad
CPC and Spectrum +3 tools; the `plus3` module names the filesystem profile.

`parse` returns sectors and their CHRN, ST1/ST2 and payload bytes. It does not
implement a floppy controller. `plus3::basic` and `plus3::code` frame +3DOS files;
`plus3::master` writes uppercase 8.3 files in caller order, under CP/M user 0.
Files may span directory extents. Invalid names, duplicates and capacity overflow
are errors. No timestamps or host metadata enter the output.

```rust
use format198x_amstrad_cpc_dsk::plus3::{File, code, master};
let program = code(&[0xc9], 0x8000)?;
let image = master(&[File { name: "RETURN.BIN", bytes: &program }])?;
assert_eq!(image.len(), 194816); // 180 KiB sectors plus DSK headers
# Ok::<(), String>(())
```

The writer uses 40 tracks, one side, nine 512-byte sectors per track, one
reserved track and 1024-byte allocation blocks. Its first sector deliberately
does not have an executable bootstrap checksum. To use the +3 menu Loader,
include an autorunning BASIC file named `DISK`. EDSK writing, other geometries,
protection, writeback and general CP/M filesystem editing are outside this API.

Sources: CPCEMU documentation [§7.7.1](https://cpc-emu.org/docu_e.html), and
Amstrad's *ZX Spectrum +3 User Manual*, printed pp153, 203, 213–217. The former
defines the container; the latter defines the disk geometry and +3DOS framing.

The reader graduated from Emu198x's `format-amstrad-dsk` at
`1464b89faf354bfe098fda88ee6f9df05be3495c`. Its subtree history is retained in
this repository. Version 0.1.0 starts independent versioning; the old workspace
versions described emulator releases. See `decisions/amstrad-cpc-dsk.md`.
