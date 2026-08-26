# `format198x-commodore-amiga-mfm` — the codec between sectors and cells

**Status:** Proposed 2026-08-26. Not started.

**Goal:** One MFM codec for the family, both directions, DD and HD — so the
emulator can put sectors on a simulated disk and a reader can get sectors back
off a preserved one.

**Depends on:** [`2026-08-26-adf-raw-and-filesystem-layers.md`](2026-08-26-adf-raw-and-filesystem-layers.md)
for the sector-layer types this produces and consumes.

---

## Why

MFM is the encoding between what a disk *stores* and what a filesystem *reads*.
The family has half of it, in the wrong place, in one direction.

`Emu198x/emu198x/crates/peripheral-commodore-amiga-floppy/src/mfm.rs` is 395
lines that encode sectors into Amiga raw MFM so the emulated drive has something
to read. It is careful work — it documents the odd/even interleave, the two
`$4489` sync words, the block and data checksums, and the clock-bit fix-up pass.
It has no decoder, because an emulator writing a disk never needed one.

A decoder is what IPF support turns on, and it is the same tables run backwards.
Writing a second, independent one inside an IPF reader is how the family ends up
with two MFM implementations that disagree about clock bits at three in the
morning.

This is the graduation rule doing its job: the encoder moves to Format198x once
a second consumer exists, exactly as ADF and TAP did.

## Read, write and verify

Unlike the raw ADF layer, this layer **can** verify meaningfully — MFM sectors
carry their own checksums, which is precisely what decoded sectors throw away.

| | Provided by |
|---|---|
| **Read** | `decode_track`, `decode_sector` — cells to sectors |
| **Write** | `encode_track`, `encode_sector` — sectors to cells |
| **Verify** | header checksum at offset 48, data checksum at offset 56, sync words, clock-bit invariant |

```rust
pub struct Geometry { pub sectors_per_track: u8 }   // 11 DD, 22 HD

pub const SECTOR_MFM_BYTES: usize = 1088;
pub const fn track_mfm_bytes(g: Geometry) -> usize;

/// Sectors to cells. `track_num` is `cyl * 2 + head`.
pub fn encode_track(sectors: &[u8], track_num: u8, g: Geometry) -> Result<Vec<u8>, Error>;

/// Cells to sectors. Finds sync words rather than assuming alignment, because a
/// real capture does not start at a sector boundary.
pub fn decode_track(cells: &[u8], g: Geometry) -> Result<DecodedTrack, Error>;

pub struct DecodedTrack {
    /// One entry per sector found, indexed by the sector number in its header
    /// — not by the order it appeared, which on a real disk is arbitrary.
    pub sectors: [Option<Sector>; 22],
}

pub struct Sector { pub data: [u8; 512], pub header_ok: bool, pub data_ok: bool }
```

`header_ok` and `data_ok` are reported rather than enforced. A decoder that
refuses a sector with a bad checksum is useless for the job people actually have
— recovering what is left of a failing disk — so the caller decides.

## HD

`MFM_TRACK_BYTES` is currently `12_668`, hardcoded for 11 sectors. It becomes
`track_mfm_bytes(geometry)`. This is the same DD assumption the ADF crate
carries and it is fixed for the same reason and at the same time.

## The trap already documented, and why the decoder must tolerate it

The existing encoder carries a comment worth preserving verbatim into the new
crate: each sector is encoded independently, so the MFM invariant *"clock bit =
1 if and only if both adjacent data bits are 0"* breaks at a sector boundary
when the previous sector's last data bit is 1. Real trackdisk readers rely on
that invariant — KS 1.3 decodes sector 0 correctly and mangles the rest. vAmiga
fixes it with a `rectifyClockBit` pass and so does this code.

The consequence for the **decoder** is the part not yet written down: images
produced by tools that did *not* do that fix-up exist, and a decoder that treats
a clock-bit violation as corruption will reject real disks. Clock bits are
checked and **reported**, never used to reject a sector whose checksums pass.

## Provenance, stated because it is weak

The existing code is a port of vAmiga's `AmigaEncoder::encodeSector` and
`MFM::addClockBits`, cross-referenced against WinUAE's `disk.cpp`. Both are
**implementations, not references** — precedent under the family's
source-of-truth rule, never prose-fact authority.

No primary MFM source is in `reference/` today. This is recorded as wanted in
[`reference/by-topic/disk-formats/index.md`](../../../reference/by-topic/disk-formats/index.md),
and until one lands, every constant here traces to two emulators agreeing with
each other. That is worth something and it is not the same as being right.

## Testing

- **Round trip both ways.** `decode(encode(sectors)) == sectors` for random
  sector data, and `encode(decode(cells)) == cells` for cells this crate
  produced. The second is the weaker property and is expected to hold only for
  our own output — a real capture carries gap bytes we do not reproduce.
- **Differential against the existing encoder.** Before Emu198x switches, the
  new `encode_track` must produce byte-identical output to
  `peripheral-commodore-amiga-floppy`'s for the same input. This is a port, and
  a port that changes behaviour is a rewrite pretending otherwise.
- **Decode what we did not encode**, from a real ADF converted by another tool,
  as an `#[ignore]`d test reading a path from an environment variable. No media
  in the repository.
- **Clock-bit violations do not reject a good sector** — an explicit test, since
  it is the failure a careless decoder ships.

## What this is NOT

- **Not flux.** Cells in, cells out. Timing, weak bits and revolutions belong to
  the IPF layer above.
- **Not the drive.** Rotation, stepping and DMA stay in Emu198x's peripheral.
- **Not GCR, FM, or anything Commodore 8-bit.** A C64 disk codec is a different
  crate under the same naming rule.
