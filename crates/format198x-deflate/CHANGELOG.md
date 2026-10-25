# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free, `no_std` DEFLATE (RFC 1951): inflate of stored, fixed and
  dynamic Huffman blocks, and a deflate encoder from level 0 to 9 that picks
  the shortest block coding.
- zlib (RFC 1950) and gzip (RFC 1952) wrappers, with Adler-32, CRC-32,
  `ISIZE` and header CRC-16 verification, every gzip header field kept, and
  multi-member gzip files.
- `Options` for an output-size limit and for allowing trailing bytes.
- Known-good vectors from the reference zlib, and random and mutated input
  tests.
