# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
