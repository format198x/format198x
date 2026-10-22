# Decision: the Amiga ADF library's design and constraints

**Status:** Active, binding for `format198x-commodore-amiga-adf`.

**Date:** 2026-07-10. Moved here from Build198x on 2026-08-28, when this
workspace gained a decisions layer.

## Why the crate is written from scratch

The Rust ADF-write ecosystem stops short. `adflib`'s create is unimplemented,
`fstool` is a heavy multi-format dependency, and `gadf` — which does precisely
this job — is Go. A bounded from-scratch writer is a few hundred lines against a
fully documented format, and it is the only path that guarantees determinism.

Layout was taken as ground truth from a known-good disk and cross-checked against
ADFlib and gadf, per the evidence rule in [`../AGENTS.md`](../AGENTS.md): prefer a
round-trip against a real image over a restatement of a secondary source.

## Constraints that bind

**Deterministic output.** Dates are zeroed and images are byte-stable across
runs. `xdftool` stamps creation dates, so its output is not reproducible; a
committed `.adf` deliverable built by this crate is.

**Panic-free on malformed input.** Every block pointer is range-checked and every
chain loop-bounded, so a corrupt image yields `Error::Corrupt` rather than a
crash. A format crate is a parser for hostile bytes; treat every pointer in the
image as attacker-controlled.

**Protection bits are `0x00`.** The RWED bits are **active-low**, so `0x0d`
revokes read and the CLI cannot `LoadSeg` the file. KS1.3 never enforced this,
which is exactly why a wrong value can sit unnoticed and look cosmetic — the
disks boot. KS2.04 reports `file is read protected`. `0x00` is a normal
readable/executable file and makes OFS disks portable to KS2.0+ as well as
KS1.3.

**FFS floppies boot only on KS2.0+.** The 1.3 ROM's floppy filesystem is
OFS-only. FFS is a general-tool capability, not a default.

**Correct for any input within the disk shape.** Data-pointer overflow beyond a
header's 72 slots chains into `T_LIST` extension blocks, and a program too large
for the disk is a typed error rather than a corrupt image. Directory inserts
chain through `hash_chain` on a slot collision instead of clobbering, so any set
of names is correct. Header checksums are deferred until after all inserts,
because an insert can set a header's `hash_chain`.

**Validation is functional, not a byte-compare.** The bar is that a mastered
`.adf` boots in emu198x-amiga to the same verified screenshot. A structural
read-back is a useful secondary check. A byte-compare against `xdftool` is
meaningless because it stamps dates.

## Native bitmap slack

Accepted 2026-10-22 for issue #69. Read-side verification accepts nonzero whole
words after the allocation map. The final allocation word's padding bits must
remain clear; real-block ownership and the checksum over the entire bitmap
block remain checked. The deterministic writer continues to zero unused words.

This refines issue #7's requirement to validate unused tail bits: the check
covers padding in the last allocation word, not subsequent whole-word slack.
The reason is [independent native Easy AMOS Save evidence](https://github.com/lang198x/amos/blob/52909f1/corpus/editions/save-load/easy/reference-save.md):
FS-UAE and Emu198x produce the same complete bitmap, with correct real allocation
and checksum but nonzero whole words beyond the map. The filename difference
and scope limits are explicit in that report. Native media remain private;
regressions use synthetic bytes. Do not normalise original media to satisfy our
writer's canonical padding.

## The OFS structures a writer must emit

An 880K DD image is 1760 × 512 = 901,120 bytes.

- **Boot block** (sectors 0–1, 1024 B): `DOS\0` + the fixed 1.x boot code + boot
  checksum. The boot code is a constant blob — embed it, don't author it.
- **Root block** (sector 880): volume name, 72-slot hash table, bitmap
  pointer(s), dates, block checksum.
- **Bitmap block**: free/used sector map; one block suffices for DD.
- **Dir header**: like a file header, sec_type 2, with its own 72-slot table.
- **File headers**: name hashed into the parent's table, size, protection bits,
  data-block list, checksum.
- **OFS data blocks**: 24-byte header (type / header-key / seq / data-size /
  next / checksum) + up to 488 B of data, chained per file.

Plus the AmigaDOS filename hash and the OFS block checksum, both small and fully
specified.

**FFS (`DOS\1`)** shares the volume structure exactly. Its data blocks are raw
512-byte sectors with no per-block header or chain, navigated entirely by the
header and extension pointer tables.

## Scope

Out, each its own later scope: the International and Dir-Cache variants,
hard-disk (RDB) layouts, multi-disk sets, copy protection, and custom bootblocks
or trackloaders.

## Drift triggers

- **"Copy the protection bits from a working disk"** — a wrong value boots fine
  on KS1.3. Verify on KS2.04, where the bits are enforced.
- **"Byte-compare against xdftool to check correctness"** — it stamps dates.
  Boot it instead.
- **"Trust a block pointer from the image"** — range-check it. A corrupt image
  must not panic a consumer.
- **"Default to FFS because it is the better filesystem"** — it does not boot on
  KS1.3.
