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
| [`format198x-acorn-atom-atm`](crates/format198x-acorn-atom-atm) | Acorn Atom `.atm` program images — read, write | Initial release |
| [`format198x-amstrad-cpc-dsk`](crates/format198x-amstrad-cpc-dsk) | CPCEMU DSK/EDSK reading and standard Spectrum +3DOS disk writing | Initial release |
| [`format198x-atari-8bit-atr`](crates/format198x-atari-8bit-atr) | Atari 8-bit ATR disk images — all boot-sector layouts, sector access, read, write | Initial release |
| [`format198x-atari-8bit-xex`](crates/format198x-atari-8bit-xex) | Atari 8-bit XEX segmented executables — ordered, zero-copy parsing | [![crates.io](https://img.shields.io/crates/v/format198x-atari-8bit-xex.svg)](https://crates.io/crates/format198x-atari-8bit-xex) |
| [`format198x-commodore-amiga-adf`](crates/format198x-commodore-amiga-adf) | Amiga ADF floppy images (OFS/FFS) — read, write, verify | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-amiga-adf.svg)](https://crates.io/crates/format198x-commodore-amiga-adf) |
| [`format198x-commodore-amiga-ilbm`](crates/format198x-commodore-amiga-ilbm) | Amiga IFF/ILBM images — interleaved bitplanes, ByteRun1, CAMG flags | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-amiga-ilbm.svg)](https://crates.io/crates/format198x-commodore-amiga-ilbm) |
| [`format198x-commodore-amiga-mod`](crates/format198x-commodore-amiga-mod) | ProTracker MOD modules — 31 samples, patterns, order table | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-amiga-mod.svg)](https://crates.io/crates/format198x-commodore-amiga-mod) |
| [`format198x-commodore-amiga-powerpacker`](crates/format198x-commodore-amiga-powerpacker) | Amiga PowerPacker (PP20) crunched files — decrunch | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-amiga-powerpacker.svg)](https://crates.io/crates/format198x-commodore-amiga-powerpacker) |
| [`format198x-commodore-c64-art-studio`](crates/format198x-commodore-c64-art-studio) | C64 OCP Art Studio hires bitmaps — read, write | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-c64-art-studio.svg)](https://crates.io/crates/format198x-commodore-c64-art-studio) |
| [`format198x-commodore-c64-bas`](crates/format198x-commodore-c64-bas) | C64 BASIC V2 listings — tokenise to a PRG, list as the C64 does | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-c64-bas.svg)](https://crates.io/crates/format198x-commodore-c64-bas) |
| [`format198x-commodore-c64-crt`](crates/format198x-commodore-c64-crt) | C64 CRT cartridge images — header, `EXROM`/`GAME`, CHIP packets, read, write | Initial release |
| [`format198x-commodore-c64-d64`](crates/format198x-commodore-c64-d64) | Commodore 1541 D64 disk images — directory, sectors, mastering with PRG files | Initial release |
| [`format198x-commodore-c64-d71`](crates/format198x-commodore-c64-d71) | Commodore 1571 D71 disk images — directory, sector read and write | Initial release |
| [`format198x-commodore-c64-d81`](crates/format198x-commodore-c64-d81) | Commodore 1581 D81 disk images — directory, sector read and write | Initial release |
| [`format198x-commodore-c64-g64`](crates/format198x-commodore-c64-g64) | Commodore 1541 G64 raw-GCR disk images — read, write | Initial release |
| [`format198x-commodore-c64-koala`](crates/format198x-commodore-c64-koala) | C64 Koala Painter multicolour bitmaps — read, write | [![crates.io](https://img.shields.io/crates/v/format198x-commodore-c64-koala.svg)](https://crates.io/crates/format198x-commodore-c64-koala) |
| [`format198x-commodore-c64-p00`](crates/format198x-commodore-c64-p00) | Commodore PC64 P00 containers — read, write | Initial release |
| [`format198x-commodore-c64-prg`](crates/format198x-commodore-c64-prg) | Commodore PRG program files — read, write | Initial release |
| [`format198x-commodore-c64-t64`](crates/format198x-commodore-c64-t64) | C64 T64 tape containers — read, write | Initial release |
| [`format198x-commodore-c64-tap`](crates/format198x-commodore-c64-tap) | Commodore TAP datasette pulse images — read, write | Initial release |
| [`format198x-commodore-vic-20-crt`](crates/format198x-commodore-vic-20-crt) | VIC-20 CRT and raw cartridge images — read, write | Initial release |
| [`format198x-dragon-bin`](crates/format198x-dragon-bin) | DragonDOS BIN machine-code files — read, write | Initial release |
| [`format198x-dragon-cas`](crates/format198x-dragon-cas) | Dragon CAS cassette images — blocks, checksums, read, write | Initial release |
| [`format198x-dragon-disk`](crates/format198x-dragon-disk) | Dragon VDK disk images — read, write | Initial release |
| [`format198x-dragon-pak`](crates/format198x-dragon-pak) | Dragon cartridge ROMs and PC-Dragon PAK snapshots — read, write | Initial release |
| [`format198x-jupiter-ace-ace`](crates/format198x-jupiter-ace-ace) | Jupiter Ace `.ace` snapshots — RLE decode and encode | Initial release |
| [`format198x-memotech-mtx-mtx`](crates/format198x-memotech-mtx-mtx) | Memotech MTX `.mtx` tape images and `.RUN` programs — read, write | Initial release |
| [`format198x-sinclair-zx-spectrum-bas`](crates/format198x-sinclair-zx-spectrum-bas) | ZX Spectrum BASIC listings — tokenise to stored bytes, list as the 48K ROM does | [![crates.io](https://img.shields.io/crates/v/format198x-sinclair-zx-spectrum-bas.svg)](https://crates.io/crates/format198x-sinclair-zx-spectrum-bas) |
| [`format198x-sinclair-zx-spectrum-scr`](crates/format198x-sinclair-zx-spectrum-scr) | ZX Spectrum SCR screen dumps — read, write | [![crates.io](https://img.shields.io/crates/v/format198x-sinclair-zx-spectrum-scr.svg)](https://crates.io/crates/format198x-sinclair-zx-spectrum-scr) |
| [`format198x-sinclair-zx-spectrum-sna`](crates/format198x-sinclair-zx-spectrum-sna) | ZX Spectrum `.sna` snapshots — 48K and 128K, read, write | Initial release |
| [`format198x-sinclair-zx-spectrum-tap`](crates/format198x-sinclair-zx-spectrum-tap) | ZX Spectrum TAP tape images — read, write, block parity | [![crates.io](https://img.shields.io/crates/v/format198x-sinclair-zx-spectrum-tap.svg)](https://crates.io/crates/format198x-sinclair-zx-spectrum-tap) |
| [`format198x-sinclair-zx-spectrum-z80`](crates/format198x-sinclair-zx-spectrum-z80) | ZX Spectrum `.z80` snapshots — versions 1 to 3, ED ED compression, read, write | Initial release |
| [`format198x-sinclair-zx80-o`](crates/format198x-sinclair-zx80-o) | Sinclair ZX80 `.o` tape images — read, write | Initial release |
| [`format198x-sinclair-zx81-p`](crates/format198x-sinclair-zx81-p) | Sinclair ZX81 `.p` tape images — read, write | Initial release |
| [`format198x-spectravideo-svi-cas`](crates/format198x-spectravideo-svi-cas) | Spectravideo SVI CAS cassette images — marker-delimited blocks | [![crates.io](https://img.shields.io/crates/v/format198x-spectravideo-svi-cas.svg)](https://crates.io/crates/format198x-spectravideo-svi-cas) |
| [`format198x-tangerine-oric-tap`](crates/format198x-tangerine-oric-tap) | Oric TAP tape images — headers, names, addresses, and concatenated files | [![crates.io](https://img.shields.io/crates/v/format198x-tangerine-oric-tap.svg)](https://crates.io/crates/format198x-tangerine-oric-tap) |
| [`format198x-tzx`](crates/format198x-tzx) | Shared TZX/CDT block stream — lossless decode and encode | [![crates.io](https://img.shields.io/crates/v/format198x-tzx.svg)](https://crates.io/crates/format198x-tzx) |

More formats graduate here from their originating projects as they earn a
standalone consumer. Emu198x's format layouts all move here regardless; see
[`decisions/emu198x-layouts-graduate-together.md`](decisions/emu198x-layouts-graduate-together.md).

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
