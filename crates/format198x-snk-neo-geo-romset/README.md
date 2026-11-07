# Neo Geo cartridge ROM files

Read MAME-named ROM files into P, S, M, ADPCM-A, ADPCM-B and interleaved C regions, and write those regions back to named files. No outside dependencies; callers handle ZIP archives.

```rust
use format198x_snk_neo_geo_romset::{from_files, to_files};

let set = from_files([
    ("201-p1.p1".to_owned(), vec![0x34, 0x12]),
    ("201-c1.c1".to_owned(), vec![1, 3]),
    ("201-c2.c2".to_owned(), vec![2, 4]),
])?;
assert_eq!(set.p, [0x12, 0x34]);
assert_eq!(set.c, [1, 2, 3, 4]);
assert_eq!(from_files(to_files(&set)?)?, set);
# Ok::<(), format198x_snk_neo_geo_romset::RomSetError>(())
```

P words swap into big-endian order. C pairs interleave the odd-numbered chip into even byte positions. V names separate sample regions (`v11` and `v21`); a one-digit V name belongs to ADPCM-A. Other board wiring and encrypted sets are not inferred.

A single 2 MiB P1 with a complete SNK header only in its upper half has its halves exchanged. The header recognition follows SNK's 1989 Hardware Specification, System Program p.6; the load/continue ordering follows MAME's `neogeo.cpp` Metal Slug set. This preserves Emu198x's existing interpretation.

The writer preserves region bytes, rather than the original filenames, chip boundaries or ignored files. It writes canonical P/S/M/V names and one C pair. A 2 MiB P region uses two 1 MiB files, preventing single-P1 layout recognition from changing an arbitrary region on reread. An empty P region and an odd-length C region return typed errors. The reader ignores unrecognised files and retains only the first S and M file in numeric order, as the originating reader did.

Graduated from Emu198x's `format-snk-neo-geo-romset`; ZIP and machine mapping remain with the caller. Optional corpus verification takes `FORMAT198X_NEOGEO_CORPUS_DIR`, a directory containing one subdirectory per extracted set:

```sh
FORMAT198X_NEOGEO_CORPUS_DIR=/path/to/sets cargo test -p format198x-snk-neo-geo-romset --test corpus -- --ignored --nocapture
```
