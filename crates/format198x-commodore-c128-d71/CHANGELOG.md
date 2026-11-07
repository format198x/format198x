# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-11-07

### Added

- Create a fresh 1571 disk image with `master(disk_name, disk_id, files)`.
  Files become closed PRG entries in the supplied order, retaining their full
  payloads and recomputing directory block counts. The writer produces the
  double-sided BAM for standard D71 images.
- Handle disk and directory capacity with typed `D71WriteError` failures:
  up to 1,328 data blocks and 144 file entries. Names follow D64's restricted
  ASCII contract; IDs are two ASCII letters or digits.

It masters a fresh image; REL files and editing an existing image are outside this API.
Existing directory/file readers and sector writes remain compatible. This is
an additive patch release; existing `0.1.0` Cargo requirements accept it.

## [0.1.0] - 2026-10-27

### Added

- Dependency-free crate graduated from Emu198x's `format-commodore-c64-d71` at
  emu198x/emu198x@63fac19e77, with no `serde` or `thiserror`.
- Named `format198x-commodore-c128-d71`, for the machine whose drive the
  1571 is, rather than following Emu198x's `c64` segment.
- Read the directory and PRG file chains of D71 images, and read and write single sectors.
