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
| [`format198x-commodore-amiga-adf`](crates/format198x-commodore-amiga-adf) | Amiga ADF floppy images (OFS/FFS) — read, write, verify | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-amiga-adf.svg)](https://crates.io/crates/format198x-commodore-amiga-adf) |
| [`format198x-commodore-amiga-ilbm`](crates/format198x-commodore-amiga-ilbm) | Amiga IFF/ILBM images — interleaved bitplanes, ByteRun1, CAMG flags | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-amiga-ilbm.svg)](https://crates.io/crates/format198x-commodore-amiga-ilbm) |
| [`format198x-commodore-amiga-mod`](crates/format198x-commodore-amiga-mod) | ProTracker MOD modules — 31 samples, patterns, order table | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-amiga-mod.svg)](https://crates.io/crates/format198x-commodore-amiga-mod) |
| [`format198x-commodore-amiga-powerpacker`](crates/format198x-commodore-amiga-powerpacker) | Amiga PowerPacker (PP20) crunched files — decrunch | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-amiga-powerpacker.svg)](https://crates.io/crates/format198x-commodore-amiga-powerpacker) |
| [`format198x-commodore-c64-koala`](crates/format198x-commodore-c64-koala) | C64 Koala Painter multicolour bitmaps — read, write | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-c64-koala.svg)](https://crates.io/crates/format198x-commodore-c64-koala) |
| [`format198x-commodore-c64-art-studio`](crates/format198x-commodore-c64-art-studio) | C64 OCP Art Studio hires bitmaps — read, write | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-c64-art-studio.svg)](https://crates.io/crates/format198x-commodore-c64-art-studio) |
| [`format198x-sinclair-zx-spectrum-scr`](crates/format198x-sinclair-zx-spectrum-scr) | ZX Spectrum SCR screen dumps — read, write | [![crates.io](https://img.shields.io/crates/v/format198x-sinclair-zx-spectrum-scr.svg)](https://crates.io/crates/format198x-sinclair-zx-spectrum-scr) |
| [`format198x-sinclair-zx-spectrum-tap`](crates/format198x-sinclair-zx-spectrum-tap) | ZX Spectrum TAP tape images — read, write, block parity | [![crates.io](https://img.shields.io/crates/v/format198x-sinclair-zx-spectrum-tap.svg)](https://crates.io/crates/format198x-sinclair-zx-spectrum-tap) |
| [`format198x-tangerine-oric-tap`](crates/format198x-tangerine-oric-tap) | Oric TAP tape images — headers, names, addresses, and concatenated files | [![crates.io](https://img.shields.io/crates/v/format198x-tangerine-oric-tap.svg)](https://crates.io/crates/format198x-tangerine-oric-tap) |
| [`format198x-tzx`](crates/format198x-tzx) | Shared TZX/CDT block stream — lossless decode and encode | [![crates.io](https://img.shields.io/crates/v/format198x-tzx.svg)](https://crates.io/crates/format198x-tzx) |

More formats (C64 D64, Amstrad DSK, and others) graduate here from their
originating projects as they earn a standalone consumer.

## Conventions

- **Org-prefixed, normally system-namespaced names** — `format198x-{manufacturer}-{system}-{format}`.
  The `format198x-` prefix says which org publishes the crate, because a registry
  entry has no folder to sit in; the rest is system-namespaced because retro
  extensions collide across machines (ADF, DSK, TAP).
  A standard genuinely shared byte-for-byte by multiple systems may instead
  use `format198x-{standard}`; TZX/CDT is the first such case.
- **Dependency-free** — `core`/`std` only; pure byte-layout code.
- **Deterministic** — the same inputs always produce identical bytes.
- **Panic-free reads** — malformed input yields a typed error, never a panic.

## Licence

GPL-2.0-or-later, throughout — consistent with the 198x emulator/tooling family
and the GPL retro-computing ecosystem these crates compose with.

[198x]: https://github.com/format198x
