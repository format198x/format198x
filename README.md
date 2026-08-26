# format198x

Retro disk- and media-**format** libraries for the [198x] family — a workspace
of small, dependency-free Rust crates that read and write the on-disk formats of
1970s–1990s computers.

Each crate is independent: its own version, its own crates.io release, no shared
lockstep. They exist in their own right — usable by any Rust tool or emulator,
not just the 198x projects.

## Crates

| Crate | Format | crates.io |
|-------|--------|-----------|
| [`format-commodore-amiga-adf`](crates/format-commodore-amiga-adf) | Amiga ADF floppy images (OFS/FFS) — read, write, verify | [![crates.io](https://img.shields.io/crates/v/format-commodore-amiga-adf.svg)](https://crates.io/crates/format-commodore-amiga-adf) |
| [`format-commodore-amiga-ilbm`](crates/format-commodore-amiga-ilbm) | Amiga IFF/ILBM images — interleaved bitplanes, ByteRun1, CAMG flags | [![crates.io](https://img.shields.io/crates/v/format-commodore-amiga-ilbm.svg)](https://crates.io/crates/format-commodore-amiga-ilbm) |
| [`format-commodore-amiga-mod`](crates/format-commodore-amiga-mod) | ProTracker MOD modules — 31 samples, patterns, order table | [![crates.io](https://img.shields.io/crates/v/format-commodore-amiga-mod.svg)](https://crates.io/crates/format-commodore-amiga-mod) |
| [`format-commodore-amiga-powerpacker`](crates/format-commodore-amiga-powerpacker) | Amiga PowerPacker (PP20) crunched files — decrunch | [![crates.io](https://img.shields.io/crates/v/format-commodore-amiga-powerpacker.svg)](https://crates.io/crates/format-commodore-amiga-powerpacker) |
| [`format-commodore-c64-koala`](crates/format-commodore-c64-koala) | C64 Koala Painter multicolour bitmaps — read, write | [![crates.io](https://img.shields.io/crates/v/format-commodore-c64-koala.svg)](https://crates.io/crates/format-commodore-c64-koala) |
| [`format-commodore-c64-art-studio`](crates/format-commodore-c64-art-studio) | C64 OCP Art Studio hires bitmaps — read, write | [![crates.io](https://img.shields.io/crates/v/format-commodore-c64-art-studio.svg)](https://crates.io/crates/format-commodore-c64-art-studio) |
| [`format-sinclair-zx-spectrum-scr`](crates/format-sinclair-zx-spectrum-scr) | ZX Spectrum SCR screen dumps — read, write | [![crates.io](https://img.shields.io/crates/v/format-sinclair-zx-spectrum-scr.svg)](https://crates.io/crates/format-sinclair-zx-spectrum-scr) |

More formats (C64 D64, Spectrum TAP, and others) graduate here from their
originating projects as they earn a standalone consumer.

## Conventions

- **System-namespaced names** — `format-{manufacturer}-{system}-{format}`,
  because retro extensions collide across machines (ADF, DSK, TAP).
- **Dependency-free** — `core`/`std` only; pure byte-layout code.
- **Deterministic** — the same inputs always produce identical bytes.
- **Panic-free reads** — malformed input yields a typed error, never a panic.

## Licence

GPL-2.0-or-later, throughout — consistent with the 198x emulator/tooling family
and the GPL retro-computing ecosystem these crates compose with.

[198x]: https://github.com/format198x
