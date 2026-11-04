# WOZ 2 bitstream images

Approved 4 November 2026 for the Apple II Disk II milestone. The emulator
originated the need; the pure layout graduates here under the umbrella's
all-Emu198x-layouts rule, rather than being duplicated inside a machine.

Keep file layout separate from rotation, MC3470 approximation and controller
logic. This first release supports WOZ2, 5.25-inch bitstream tracks only; flux
and 3.5-inch media return typed unsupported errors. Retain the whole allocation
so unknown chunks, padding and metadata survive. Never guess that padded byte
length is a track's bit length. No dependencies or firmware.

Tests must cover CRC, malformed allocations, shared maps, non-byte-aligned
tracks and byte-exact round trips. No writes to filesystem paths are provided.
