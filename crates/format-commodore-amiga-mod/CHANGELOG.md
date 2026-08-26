# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/format198x/format198x/compare/format-commodore-amiga-mod-v0.1.0...format-commodore-amiga-mod-v0.1.1) - 2026-08-26

### Other

- release ([#13](https://github.com/format198x/format198x/pull/13))

## [0.1.0](https://github.com/format198x/format198x/releases/tag/format-commodore-amiga-mod-v0.1.0) - 2026-08-26

### Added

- export the MOD layout constants and close the API gaps consumers need
- recognise Startrekker's FLT8 8-channel magic
- add format-commodore-amiga-mod

### Fixed

- *(mod)* read a module's own bytes to tell an extra pattern from a tail
- read MOD titles and sample names as Latin-1, not UTF-8
- name the real ambiguity when a MOD's size doesn't divide evenly
- stop trailing bytes being misread as MOD pattern data
- derive MOD pattern count from file size, not the order table
- the MOD Module struct must be lossless

### Other

- *(mod)* drop an unused dev-dependency and correct two stale notes
- *(mod)* make the corpus run measure the pattern rule, not just round-trip
- store Sample::data as raw bytes, add data_i8 for the signed view
- close the gaps the final review found in both crates
- make the MOD sample and row counts type-level invariants
- cover the pattern-count arithmetic malformed_input_never_panics missed
