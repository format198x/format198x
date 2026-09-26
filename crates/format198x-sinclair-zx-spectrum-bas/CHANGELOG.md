# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/format198x/format198x/compare/format198x-sinclair-zx-spectrum-bas-v0.1.1...format198x-sinclair-zx-spectrum-bas-v0.1.2) - 2026-09-26

### Fixed

- store a hidden zero after a bare BIN, as the ROM does
- compute hidden numbers with the ROM's own arithmetic
- tokenise colour items after PLOT, DRAW and CIRCLE

## [0.1.1](https://github.com/format198x/format198x/compare/format198x-sinclair-zx-spectrum-bas-v0.1.0...format198x-sinclair-zx-spectrum-bas-v0.1.1) - 2026-09-26

### Fixed

- **Spaces inside numbers and names now read as the 48K ROM reads them**
  ([#49](https://github.com/format198x/format198x/pull/49)). The ROM skips a
  space after a decimal point and around an exponent's `E`, so `1. 5` and
  `1.5 E3` are single numbers, but it refuses a space among a number's
  whole-part digits: `1 000`, `1 .5` and `1E3 3` are now errors rather than
  two numbers. A numeric variable name continues across spaces, so `a 1` is
  the variable `a1`, stored with its space. Spaces after a number are stored
  before its hidden five-byte value, where the ROM puts them. Each case is
  checked against what the genuine ROM's editor stores, and against the
  output of a tape built from this crate's bytes and run on it.

## [0.1.0](https://github.com/format198x/format198x/releases/tag/format198x-sinclair-zx-spectrum-bas-v0.1.0) - 2026-09-26

### Added

- Dependency-free tokenising of numbered ZX Spectrum BASIC listings into the
  bytes the 48K ROM stores, keeping strings, `REM` text, typos and number
  spellings as typed, with each number's hidden five-byte value.
- `list` and `list_line`, which print a stored program as the 48K ROM's LIST
  does, checked against LIST output captured from the genuine ROM, and
  `listed_form`, which gives what LIST would print for a source listing.
- `lex_line`, which splits a source line into positioned, classified pieces.
- `ListingError`, a typed error carrying the 1-based source line apart from
  its message, returned by every listing function including `list`.
- `tokenise` and `parse`, an analysis route from a listing through an AST.
