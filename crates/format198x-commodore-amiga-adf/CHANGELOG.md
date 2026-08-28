# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Renamed on 2026-08-26.** This crate's 0.1.0-0.2.2 releases were published as
> `format-commodore-amiga-adf`, and a final version may still appear under that
> name pointing here. Every entry below that point was released under the
> old name, and its links point at tags that still carry it — they are left
> as they were so they keep resolving. The version numbering continues
> unbroken across the rename.

## [Unreleased]

## [0.3.1](https://github.com/format198x/format198x/compare/format198x-commodore-amiga-adf-v0.3.0...format198x-commodore-amiga-adf-v0.3.1) - 2026-08-28

### Other

- correct a capacity figure that mixed two kinds of kilobyte ([#28](https://github.com/format198x/format198x/pull/28))

## [0.3.0](https://github.com/format198x/format198x/compare/format198x-commodore-amiga-adf-v0.2.3...format198x-commodore-amiga-adf-v0.3.0) - 2026-08-27

The crate gains the layer beneath the filesystem, and high-density support at
both layers. Additive throughout: `Disk`, `Volume`, `master` and `master_fs`
keep their signatures, and every image that built before builds to the same
bytes ([#26](https://github.com/format198x/format198x/pull/26)).

### Added

- **A raw sector layer.** `Image` and `ImageMut` address an ADF's sectors
  directly — by cylinder/head/sector as a drive does, or by logical block as
  AmigaDOS does. `Geometry` describes the media (`DD`, `HD`). Useful for
  bootblock-only disks, copy-protected track data, and emulator floppy
  peripherals, none of which have a filesystem to read. `Image::track` returns
  one contiguous slice, so an MFM encoder can take it without copying.
- **High-density floppies**, at both layers. `Geometry::root_block` derives the
  root's position from Commodore's `rootblock.c` formula instead of the
  hardcoded 880 — 1760 on HD. Verified against images written by amitools'
  `xdftool`, not derived alone. Choose the media with `Volume::set_geometry`.
- **`DiskMut`** — change a disk that already exists. `write_file` (creating or
  replacing in place), `create_dir`, `delete`, `format`, `free_blocks`. Every
  operation maintains the bitmap, splices the directory hash chain, and
  recomputes the checksums it disturbs.
- **`Disk::check`** — an exhaustive verifier reporting every fault rather than
  the first, including whether the bitmap agrees with what is actually
  reachable. Returns a `Report` of `Problem`s; `verify` is unchanged beside it.
- **`identify_container`** is now public: name the container a file is in
  (IPF, DMS, zip, gzip) without opening it.
- `Error::OutOfBounds` and `Error::BadSectorLength` (the enum is
  `#[non_exhaustive]`).

### Fixed

- **`verify` called ordinary data disks corrupt.** It rejected any disk with a
  zero boot-checksum field — which is what AmigaDOS `Format` leaves until
  `Install` writes a bootstrap, and so what most data disks carry. It now
  accepts that state and checks everything else, including looking for the
  bootstrap in sector 0, where the ROM actually executes from.
- **A volume could fill only half its disk.** Blocks were allocated upward from
  the root and never below it, capping an 880K floppy near 432K. Allocation now
  goes through the bitmap, so a single file can reach about 865K of an 880K disk
  and 1733K of a 1760K HD one — 98% of the media rather than 49%.
- Out-of-range sector and block addresses return an error instead of indexing
  out of bounds.

### Changed

- `Volume::build` is expressed over `DiskMut`, so there is one implementation of
  placing a file on an Amiga disk rather than two. Output is byte-identical.
- `Error::DiskFull` and the wrong-size `Error::Corrupt` message no longer name
  double density specifically, now that both geometries are read.

## [0.2.3](https://github.com/format198x/format198x/compare/format198x-commodore-amiga-adf-v0.2.2...format198x-commodore-amiga-adf-v0.2.3) - 2026-08-26

### Fixed

- say which container an ADF image really is, and spec the layers beneath ([#20](https://github.com/format198x/format198x/pull/20))

## [0.2.2](https://github.com/format198x/format198x/compare/format-commodore-amiga-adf-v0.2.1...format-commodore-amiga-adf-v0.2.2) - 2026-08-26

### Other

- stop the ADF manifest describing shipped work as pending ([#9](https://github.com/format198x/format198x/pull/9))

## [0.2.1](https://github.com/format198x/format198x/compare/format-commodore-amiga-adf-v0.2.0...format-commodore-amiga-adf-v0.2.1) - 2026-07-10

### Other

- split the crate into focused modules
