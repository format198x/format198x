# format198x-apple-ii-woz

Dependency-free reader and writer for WOZ 2 single-sided 5.25-inch bitstream
images. Originated by Emu198x's Apple II Disk II work; the format lives here under
the all-Emu198x-layouts graduation decision.

`Woz::parse` validates the checksum (unless zero), chunk bounds, quarter-track
map, track allocations, bit counts and overlaps. Flux and 3.5-inch images fail
explicitly. No disk controller, sector codec, filesystem access or ROM is included.

`track` exposes MSB-first bytes and the exact bit count. `track_mut` permits
in-memory edits without resizing. `to_bytes` preserves unknown chunks, padding
and unmodified source bytes; an existing nonzero checksum is recomputed.
`from_tracks` builds canonical block-aligned images with a recorded CRC.

Specification: [Applesauce WOZ 2.1](https://applesaucefdc.com/woz/reference2/).
Primary-library reading note: `198x/reference/by-system/apple-ii/woz-2-bitstream-format.md`.
