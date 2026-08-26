# format198x-sinclair-zx-spectrum-tap

Decode and encode Sinclair ZX Spectrum **TAP** tape images in Rust — the block
stream a `.tap` file holds, exactly as the ROM loader would have read it off
tape. Dependency-free (`core`/`std` only), deterministic, and panic-free on
malformed input.

A TAP file stores the *data* of each block and nothing else: the leader tone,
the sync pulses and the bit timings are left for a loader to reconstruct from
the standard ROM rules. That is what makes it the simplest archival form of a
tape, and what it cannot represent — turbo loaders, custom pulse timings,
direct recordings, any metadata at all — is why `.tzx` exists.

## Read a tape

```rust
use format198x_sinclair_zx_spectrum_tap::{Header, HeaderKind, decode};

let blocks = decode(&std::fs::read("game.tap")?)?;
for block in &blocks {
    if block.is_header() {
        let header = Header::from_payload(&block.data)?;
        println!("{:?} {:?}, {} bytes", header.kind, header.name, header.length);
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Write one

```rust
use format198x_sinclair_zx_spectrum_tap::{Header, HeaderKind, TapBlock, encode};

let code = vec![0xF3, 0x76];                       // di / halt
let header = Header::new(HeaderKind::Code, "demo", code.len() as u16, 0x8000, 0x8000);
let tape = encode(&[header.block(), TapBlock::data(code)]);
std::fs::write("demo.tap", tape)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The parity byte is not yours to carry: it is the XOR of a block's flag and
payload, so `encode` computes it and `decode` needs nothing from you to check
it.

## The layout

| | |
|---|---|
| file | a bare sequence of blocks, no header and no magic number |
| block | little-endian `u16` length, then that many bytes |
| those bytes | one flag, the payload, one parity byte |
| flag | `$00` header, `$FF` data — the loader reads bit 7 |
| parity | XOR of the flag and every payload byte |
| header payload | 17 bytes: kind, 10-byte space-padded name, next block's length, two kind-dependent parameters |

Layout facts are authored from `syntheses/zx-spectrum/tape-loading-format.md`
(§ 2 and § 4), which cross-checks the Sinclair BASIC manual against fuse's
`libspectrum`. The tests hold a real tape written by SjASMPlus 1.21.0 and
require this crate to reproduce it byte for byte — a codec can round-trip its
own mistakes, so agreeing with itself is not the claim being made.

## Licence

GPL-2.0-or-later.
