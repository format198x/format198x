# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-sinclair-zx80-o` at
  emu198x/emu198x@7e4fb5b666, with no `serde` or `thiserror`.
- Decode and validate `.o` / `.80` images, build one from a RAM dump, and encode it.
- The cassette pulse train (`to_pulses`) from Emu198x's crate stays in Emu198x, as machine behaviour.

Independent versions start at 0.1.0.
