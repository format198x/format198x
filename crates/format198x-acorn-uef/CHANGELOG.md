# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free UEF container, split from Emu198x's `format-acorn-uef` at
  emu198x/emu198x@1d70a0a216: lossless chunk reading and writing, typed
  views and constructors for the common tape chunks.
- Gzipped images are reported as `UefError::Gzipped` rather than inflated.
- UEF floats are read as IEEE 754 singles with `f32::from_le_bytes`, so
  zero and subnormal values read correctly; Emu198x's decoder assumed a
  normal number.

Independent versions start at 0.1.0. Emu198x's crate never kept a changelog.
