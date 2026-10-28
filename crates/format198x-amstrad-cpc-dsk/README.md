# format198x-amstrad-cpc-dsk

Dependency-free CPCEMU standard and extended DSK reading, with a bounded
standard 180 KiB Spectrum +3DOS disk writer and a sector-model EDSK encoder. The container is shared by Amstrad
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
include an autorunning BASIC file named `DISK`. Other filesystem geometries,
writeback and general CP/M filesystem editing are outside the `plus3` API.

`encode_extended(&DiskImage)` writes the represented sectors as EDSK. It
preserves physical placement/order, duplicate IDs, CHRN, ST1/ST2 and exact
stored lengths/data, including zero-length sectors. Empty tracks become
unformatted entries. Invalid dimensions and layouts exceeding the fixed disk
table, sector table or track-length fields return errors instead of truncating.

This is not a byte-preserving dump rewrite. The model does not retain creator,
gap/filler, recording mode/rate or padding. The encoder supplies deterministic
defaults; stored weak-sector copies remain uninterpreted bytes. Runtime export,
controller accuracy and protection behaviour need separate verification.

Convert an existing image without overwriting any existing output path:

```sh
cargo run -p format198x-amstrad-cpc-dsk --example encode_extended -- input.dsk new.edsk
```

Sources: CPCEMU documentation [§7.7.1](https://cpc-emu.org/docu_e.html), and
Amstrad's *ZX Spectrum +3 User Manual*, printed pp153, 203, 213–217. The former
defines the container; the latter defines the disk geometry and +3DOS framing.
Extended output follows the format authors' [Extended DSK Rev.5 definition](https://cpctech.cpcwiki.de/docs/extdsk.html),
summarised in the umbrella reference `by-system/amstrad-cpc/cpcemu-dsk-container.md`.

The reader graduated from Emu198x's `format-amstrad-dsk` at
`1464b89faf354bfe098fda88ee6f9df05be3495c`. Its subtree history is retained in
this repository. Version 0.1.0 starts independent versioning; the old workspace
versions described emulator releases. See `decisions/amstrad-cpc-dsk.md`.

`parse_container` is an opt-in checked view for adapters that need the physical
Track Information Block metadata as well as sector records. It retains
interleaved container slots and TIB track/side, size code, rate/mode, gap and
filler. Descriptor tables must fit the 256-byte TIB, and all sector payloads
must fit the declared track. Standard size codes are checked without clamping;
extended sectors keep their exact stored lengths, including zero.

An absent or entirely zeroed track header becomes an empty slot; a present
zero-sector header remains identifiable. Address marks and status flags remain
data, rather than being flattened into controller geometry. Callers reject
layouts their controllers cannot represent. Existing `parse` compatibility
behaviour and `DiskImage` struct literals are unchanged. Neither API retains
all original padding or creator bytes for byte-identical dump rewriting.
