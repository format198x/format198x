# Changelog

## [0.1.0] - 2026-11-04

### Added

- Parse and encode 5.25-inch WOZ 2 bitstreams without dependencies.
- Validate checksums, chunks, track allocations, maps and exact bit counts.
- Preserve opaque chunks, metadata, padding and shared quarter-track mappings
  when editing bits in an existing track allocation.
- Build canonical images and set write protection on an explicit memory copy.

Flux tracks and 3.5-inch playback are rejected. The DOS 3.3 System Master and
an independently published minimal WOZ2 fixture round-trip byte for byte.
