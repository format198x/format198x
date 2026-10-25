# format198x-deflate

**DEFLATE** ([RFC 1951]) with its **zlib** ([RFC 1950]) and **gzip**
([RFC 1952]) wrappers, in Rust, with no dependencies.

Many retro formats compress their payloads with DEFLATE. UEF tapes are
usually gzipped whole, and SZX and RZX snapshots hold zlib streams. This crate
decodes and encodes all three layers, so the Format198x crates read and write
real files without leaving the family. It is `no_std` (it needs only
`alloc`), and it never panics on bad input.

```rust
use format198x_deflate::{Error, Level, Options, gzip, inflate, deflate, zlib};

// Raw DEFLATE.
let packed = deflate(b"BBC Micro, BBC Micro, BBC Micro", Level::BEST);
assert_eq!(inflate(&packed)?, b"BBC Micro, BBC Micro, BBC Micro");

// zlib, with a cap on how much a hostile stream may expand.
let page = zlib::encode(&[0u8; 16384], Level::DEFAULT);
let opts = Options::default().max_output(16384);
assert_eq!(zlib::decode_with(&page, &opts)?.len(), 16384);
let tight = Options::default().max_output(1000);
assert_eq!(zlib::decode_with(&page, &tight), Err(Error::OutputLimit { limit: 1000 }));

// gzip, keeping every header field.
let header = gzip::Header { name: Some(b"elite.uef".to_vec()), ..Default::default() };
let file = gzip::encode(&header, b"UEF File!\0", Level::DEFAULT)?;
let member = gzip::decode(&file)?;
assert_eq!(member.header.name.as_deref(), Some(&b"elite.uef"[..]));
# Ok::<(), Error>(())
```

## What it supports

**Decoding.** Stored, fixed-Huffman and dynamic-Huffman blocks. zlib headers
are checked (method, window size, check bits) and the Adler-32 trailer is
verified; a preset dictionary is reported as unsupported. gzip headers keep
`FTEXT`, `MTIME`, `XFL`, `OS`, `FEXTRA`, `FNAME`, `FCOMMENT` and `FHCRC`, the
header CRC-16 is checked, and the CRC-32 and `ISIZE` trailer is verified.
Several gzip members in one file are joined, as `gunzip` does. Every failure
is a typed `Error`.

**Encoding.** Levels 0 to 9. Level 0 writes stored blocks; higher levels find
repeats with hash chains over the full 32K window, with lazy matching from
level 4. Each block is written as whichever of stored, fixed-Huffman and
dynamic-Huffman coding is shortest, so incompressible data grows by only a
few bytes. Output is deterministic. zlib and gzip wrappers write their
headers and trailers; a gzip header writes back exactly as it was read.

**Safety.** `Options::max_output` caps the decoded size. Truncated streams,
garbage, bad code tables and matches that reach before the start all return
errors. Bytes after a stream are an error unless `Options::allow_trailing` is
set. Tests feed tens of thousands of random and mutated streams to every
decoder.

## Correctness

The tests decode streams written by the reference zlib (through CPython;
`tests/vectors/generate.py` regenerates them) and check the results against
zlib's own lengths and checksums. An ignored test does the reverse: it writes
this crate's output at every level and has Python's `zlib` and `gzip`
modules decode it.

```sh
cargo test -p format198x-deflate --test python -- --ignored
```

## Performance

Measured on one core of the development machine, release build, against
CPython's zlib 1.3 on the same files:

| Input | Level | Ratio (zlib) | Encode MB/s (zlib) | Decode MB/s (zlib) |
|---|---|---|---|---|
| 717 KB of Markdown | 1 | 41.0% (43.3%) | 58 (73) | 210 (180) |
| | 6 | 36.8% (36.8%) | 15 (20) | 250 (190) |
| | 9 | 36.7% (36.8%) | 13 (16) | 220 (200) |
| 293 KB of compiled Rust | 1 | 26.5% (28.4%) | 58 (93) | 250 (230) |
| | 6 | 22.9% (23.1%) | 21 (35) | 340 (290) |
| | 9 | 22.4% (22.4%) | 3 (4) | 450 (340) |

Compression matches zlib's; encoding is somewhat slower and decoding about the
same. Retro payloads are kilobytes, so either is instant in practice.

## Licence

GPL-2.0-or-later.

[RFC 1950]: https://www.rfc-editor.org/rfc/rfc1950
[RFC 1951]: https://www.rfc-editor.org/rfc/rfc1951
[RFC 1952]: https://www.rfc-editor.org/rfc/rfc1952
