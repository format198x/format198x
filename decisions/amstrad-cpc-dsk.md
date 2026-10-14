# CPCEMU DSK reading and the standard +3DOS writer

Accepted 2026-10-14. The Spectrum source-to-disk qualification consumer opens
the graduation gate: Build198x writes media that Emu198x already reads.

The existing Emu198x reader moves here with its subtree history. Its format
model contains sector metadata and bytes, with no controller state or serde
dependency. Emu keeps a compatibility adapter into its existing controller
types so this move does not change save-state layout.

The first writer is deliberately bounded to the +3 manual's type-0 180 KiB
geometry. It creates user-0 files with +3DOS headers, CP/M extents and allocation
blocks in a standard DSK. It does not execute a boot sector. Build198x owns
composition and BASIC tokenisation; this crate owns all disk byte layouts.

The source specification is the original reader's module documentation plus
CPCEMU documentation §7.7.1 and the Amstrad +3 manual pp213–217. Hardware facts
remain in the umbrella reference layer. Emulator loading decisions remain in
Emu because their controller and ROM behaviour is not a format contract.

Preserve existing standard/extended reads, including CHRN, status flags and
zero-length EDSK sectors. Validate writer output using a real ROM and an
independent reader/runtime, not solely this crate's parser. Do not add FDC
behaviour, arbitrary geometry or EDSK writing without a concrete consumer.
