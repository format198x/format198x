# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Renamed on 2026-08-26.** This crate was published as `format-commodore-amiga-powerpacker` up to
> and including 0.1.1. Every entry below that point was released under the
> old name, and its links point at tags that still carry it — they are left
> as they were so they keep resolving. The version numbering continues
> unbroken across the rename.

## [Unreleased]

## [0.1.1](https://github.com/format198x/format198x/compare/format-commodore-amiga-powerpacker-v0.1.0...format-commodore-amiga-powerpacker-v0.1.1) - 2026-08-26

### Other

- release ([#13](https://github.com/format198x/format198x/pull/13))

## [0.1.0](https://github.com/format198x/format198x/releases/tag/format-commodore-amiga-powerpacker-v0.1.0) - 2026-08-26

### Added

- add format-commodore-amiga-powerpacker

### Fixed

- bound PowerPacker run and match lengths by the declared output

### Other

- *(powerpacker)* make the sweep's guards as strong as the sweep itself
- close the gaps the final review found in both crates
- credit Claudio Matsuoka for the PowerPacker corruption checks
- make the PP20 malformed-input sweep reach the decompression loop
