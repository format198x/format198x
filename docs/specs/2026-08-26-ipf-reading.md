# `format198x-commodore-amiga-ipf` — reading preserved disk images

**Status:** Proposed 2026-08-26. Not started. **Read-only by design.**

**Goal:** Read IPF images as sectors where the disk's tracks are standard, and
say clearly when they are not — so the Amiga library stops being cracked
releases only.

**Depends on:** [`2026-08-26-amiga-mfm-codec.md`](2026-08-26-amiga-mfm-codec.md)
for the decoder, and
[`2026-08-26-adf-raw-and-filesystem-layers.md`](2026-08-26-adf-raw-and-filesystem-layers.md)
for the sector layer this feeds.

---

## What this buys

`198x/docs/software-library-coverage.md` records the state plainly: Amiga
commercial releases are filed under `Games/SPS` as IPF and unsupported
(`emu198x#105`), and the plain `[ADF]` tree is largely **cracked** releases. So
today the family's Amiga catalogue is the scene's output rather than the
publishers'. IPF is what changes that.

## The limit, stated first because it decides the scope

**A protected disk is often exactly why an image is IPF, and protection is
exactly what sectors erase.**

IPF preserves flux-level truth: track timing, deliberately weak bits,
non-standard sector counts, long tracks, and layouts that are not sectors at
all. Decoding to 512-byte sectors throws all of that away. So this crate reads
the images whose tracks are conventional, and cannot help with the ones whose
protection motivated preserving them at that level in the first place.

That is not a defect to fix later. Sectors are the wrong abstraction for a
protected track, and an emulator wanting those needs flux at the drive head, not
a sector array. The scope is therefore **"read standard-format IPF tracks as
sectors"**, and the crate must say so in its own documentation — because
"Play198x supports IPF" would set an expectation against the SPS catalogue that
this cannot meet.

```rust
pub enum Track {
    /// Decoded to conventional sectors.
    Sectors(DecodedTrack),
    /// Well-formed, but not representable as sectors. Says why.
    NotSectors { reason: &'static str },
}
```

Reporting `NotSectors { reason }` is the whole difference between a tool that is
honest and one that silently hands back a corrupted disk.

## Provenance — the reason this needs unusual care

**The IPF format is open and undocumented at the same time.** From the source
acquired at
[`reference/by-topic/disk-formats/`](../../../reference/by-topic/disk-formats/index.md),
section 1.2, Jean Louis-Guerin:

> the IPF format is now open but unfortunately completely undocumented

SPS released the decoder under the MAME licence after long debate, and never
published a specification. The document the family now holds is his
reconstruction from several sources, and he warns it "inevitably must contain
errors and therefore must be used with caution."

Three consequences bind this work:

1. **Every field offset is a hypothesis.** Confirm each against real images
   before relying on it. The sidecar carries this as a `provenance_warning` so a
   reader citing the file cannot miss it.
2. **Implement from the documentation, not from their code.** Format198x is
   GPL-2.0-or-later and its crates are dependency-free; the released SPS decoder
   carries its own terms. An independent implementation from a published
   description avoids the question entirely, and is what the family does anyway —
   author from sources, cite upward.
3. **The released decoder is a test oracle, not an input.** Comparing our output
   against it on real images is exactly the differential technique already used
   against libxmp for MOD, and it is how the hypotheses in point 1 get settled.

## Shape

```rust
pub fn is_ipf(bytes: &[u8]) -> bool;                      // "CAPS" record at 0

pub struct Ipf<'a> { /* … */ }

impl<'a> Ipf<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<Self, Error>;
    pub fn info(&self) -> &Info;                          // platform, release, revision
    pub fn track(&self, cyl: u16, head: u8) -> Result<Track, Error>;
    pub fn geometry(&self) -> Option<Geometry>;           // None when non-standard
    pub fn verify(&self) -> Report;                       // every record's CRC
}
```

Records to parse, per the documentation: `CAPS`, `INFO`, `IMGE`, `DATA` with its
block descriptors and data areas, and `CTEI`/`CTEX` for CT RAW.

## Read, write and verify

| | Position |
|---|---|
| **Read** | Yes — the point of the crate. |
| **Write** | **No, deliberately.** |
| **Verify** | Yes — every record carries a CRC, and this layer can check them all. |

**Not writing IPF is a decision, not a gap.** The released SPS library is
read-only too; images are produced by SPS from original media with hardware the
family does not have. An IPF this crate wrote would be a *claim of preservation
provenance* that nothing behind it supports. Reissuing a modified disk as an IPF
would be a small act of forgery in a format whose entire purpose is
trustworthiness.

That is the sharpest line in this spec and it should not be softened later
because writing turned out to be easy.

## Testing

- **Synthetic images built in code**, record by record, for the parser.
- **Differential against the released decoder** on real images — `#[ignore]`d,
  path from an environment variable. This is what turns documented hypotheses
  into confirmed layout.
- **A protected image must report `NotSectors`, not a plausible-looking disk.**
  The test that matters most, and the one a passing parser will not give you for
  free.
- **No media in the repository, ever.**

## Sequencing

1. Record parsing and `verify` — no MFM needed, and it is independently useful:
   identifying and validating an IPF is worth having on its own.
2. Track decode to sectors, once the MFM decoder exists.
3. Wire into `play198x-core`'s container layer, so an IPF opens like an ADF.

## What this is NOT

- **Not an IPF writer.** See above.
- **Not flux emulation.** Weak bits and timing are read and reported, not
  simulated. What an emulated drive does with them is Emu198x's business.
- **Not KryoFlux STREAM, SCP, or CT RAW.** `CTEI`/`CTEX` records are parsed
  because they appear inside IPF files; the standalone formats are separate
  work.
- **Not a preservation tool.** The family reads what SPS produced. It does not
  produce it.
