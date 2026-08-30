# format198x-tangerine-oric-tap

Read and write **Oric-1 and Oric Atmos TAP tape images** in Rust.

Oric TAP stores the bytes the ROM cassette routines receive, not sampled
audio: a run of `$16` leader bytes, `$24`, a nine-byte header, a NUL-terminated
name, and the file data. Multiple files may be concatenated on one virtual
tape. The crate is dependency-free and returns typed errors for malformed
input.

```rust
use format198x_tangerine_oric_tap::{FileKind, TapFile, decode, encode};

let file = TapFile::new(FileKind::MachineCode, false, 0x500, "demo", vec![0xa9, 0x41])?;
let image = encode(&[file])?;
let decoded = decode(&image)?;

assert_eq!(decoded[0].start_address(), 0x500);
assert_eq!(decoded[0].data, [0xa9, 0x41]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The codec models the ordinary BASIC and machine-code layout whose data length
is the inclusive end address minus the start address plus one. Atmos `STORE`
array records repurpose those header fields and are deliberately rejected
until their inconsistent ROM-produced lengths can be represented without
guessing.

## Evidence

The byte layout is cross-checked against the OSDK `Header` and `Tap2Dsk`
utilities and Oricutron's tape implementation. The machine-visible behaviour
and address/name semantics are documented by the *Oric Atmos Handbook* and
*The Oric-1 Companion* in the 198x reference library.

## Licence

GPL-2.0-or-later.
