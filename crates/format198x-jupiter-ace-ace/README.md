# format198x-jupiter-ace-ace

Read and write **Jupiter Ace `.ace` snapshots** in Rust.

An `.ace` file, as written by ACE32, has no header: it is a run-length-encoded stream (`ED count value`, `ED 00` to end) that decodes to RAM from `$2000`, with the Z80 registers in a block at `$2100`. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_jupiter_ace_ace::{Snapshot, decode, encode};

let ram = vec![0u8; 0x2000];
let stream = encode(&ram);
assert_eq!(decode(&stream)?, ram);
let snapshot = Snapshot::parse(&stream)?;
assert_eq!(snapshot.encode()?, stream);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Format and provenance: `reference/by-system/jupiter-ace/jupiter-ace-ace-snapshot-format.md` in the 198x reference library.

## Provenance

Graduated from Emu198x's `format-jupiter-ace-ace` crate (emu198x/emu198x@15e4c8210d), whose history of 1 commit stays in that repository.

## Licence

GPL-2.0-or-later.
