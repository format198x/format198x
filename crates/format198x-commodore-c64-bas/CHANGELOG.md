# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/format198x/format198x/compare/format198x-commodore-c64-bas-v0.1.0...format198x-commodore-c64-bas-v0.1.1) - 2026-09-26

### Fixed

- **A program too big for BASIC memory is now an error**
  ([#53](https://github.com/format198x/format198x/pull/53)). BASIC memory on
  a stock C64 runs from `$0801` to `$9FFF`, 38,911 bytes with the end marker,
  because the BASIC ROM sits at `$A000`. A larger program was written without
  complaint and could not be LISTed or RUN; it is now refused with its size
  and the limit.
- **Line 0 is accepted, and every out-of-range line number gets one message**
  ([#55](https://github.com/format198x/format198x/pull/55)). BASIC V2 takes 0
  to 63999: typed into the C64 (VICE x64sc), lines 0 and 63999 are stored,
  listed and run, and 64000 and above give `?SYNTAX ERROR`. The crate refused
  line 0 and gave different messages for a numeral that was too big and one
  too long to parse; it now reports `line number N out of range (0-63999)`.

## [0.1.0](https://github.com/format198x/format198x/releases/tag/format198x-commodore-c64-bas-v0.1.0) - 2026-09-26

### Added

- Dependency-free tokenising of numbered Commodore 64 BASIC V2 listings into
  a PRG loading at `$0801`, byte-identical to petcat for every Code198x C64
  sample: keywords tokenise wherever the C64 would, `REM` text and `DATA`
  values stay literal, and lines are stored in number order.
- `?` is stored as the PRINT token, as the C64's own tokeniser stores it.
  The C64 ROM is the authority here; petcat stores `?` as the character.
- `list` and `list_line`, which print a stored program as the C64's LIST
  does, and `listed_form`, which gives what LIST would print for a source
  listing.
- `lex_line`, which splits a source line into positioned, classified pieces.
- `ListingError`, a typed error carrying the 1-based source line apart from
  its message, returned by every listing function including `list`.
