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

## Extended encoding for live-media export

Accepted 2026-10-16. Emu198x #45 is the concrete consumer: a modified +3 disk
must export to a new path and reopen without replacing it with stale source
bytes. The user approved shipping this format prerequisite separately before
runtime wiring. This extends the initial writer boundary above; it adds no
controller behaviour or new dependency.

`encode_extended(&DiskImage)` serializes exactly the represented sector model.
It preserves physical placement/order, CHRN/status and stored bytes/lengths,
including zero-length sectors and repeated IDs. Empty tracks map to unformatted
entries. Invalid dimensions, overflowing tables and oversized blocks reject;
the earlier frozen writer's silent sector truncation is not carried forward.
No archive code is ported. The format authors' Extended DSK Rev.5 specification
in the umbrella reference layer binds the byte layout.

The current model does not preserve original gap/filler, recording mode/rate,
creator or container padding. The encoder documents deterministic defaults
rather than changing the model or claiming byte-preserving dump rewriting.
Stored weak-sector copies remain bytes; emulator interpretation is separate.
Runtime export and fresh-machine reopening remain Emu-owned acceptance gates.
