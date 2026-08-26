# format-commodore-amiga-powerpacker

Decrunch Amiga **PowerPacker (PP20)** streams in Rust — the compressor Amiga
music disks overwhelmingly use for tracker modules and executables.
Dependency-free (`core`/`std` only), decode-only, and panic-free on malformed
input.

PowerPacker is not an edge case for Amiga media: of the first four modules
found on a real Gathering '92 music disk during this project's research,
three were PP20-crunched. A player that cannot decrunch cannot read that
disk.

## Decrunch a stream

```rust
use format_commodore_amiga_powerpacker::{decrunch, is_powerpacked};

let bytes = std::fs::read("Module.PP20")?;
if is_powerpacked(&bytes) {
    let original = decrunch(&bytes)?;
    std::fs::write("Module", &original)?;
}
```

## Notes

- **Decode-only.** There is no PP20 compressor here, and none is planned —
  this crate exists to read music disks, not to make new ones.
- **The bitstream runs backwards.** PowerPacker's crunched data is read from
  its *end* toward its *start*, and the decrunched output is written from
  its end toward its start too — the detail every naive port gets wrong. A
  back-reference offset points *forward* in the output, toward bytes already
  written near the end, not backward the way a conventional LZ77 window
  does.
- **Bounds-checked throughout.** This crate sits behind an FFI boundary in
  the wider Play198x player, where a panic is undefined behaviour. Every
  header field and bitstream read is range-checked against the input it
  actually has, and a violation returns [`DecodeError`] rather than
  indexing past a slice.
- **Algorithm provenance.** Ported from the public-domain PowerPacker
  decruncher by Stuart Caie, as carried into Heikki Orsila's `amigadepack`
  and from there into libxmp's `ppDecrunch` — the implementation most
  tracker players have shipped for two decades. See the module
  documentation (`src/lib.rs`) for the exact lineage and what changed in
  the port.

[`DecodeError`]: https://docs.rs/format-commodore-amiga-powerpacker/latest/format_commodore_amiga_powerpacker/enum.DecodeError.html

## Part of the 198x family

This crate lets [Play198x], the retro media player, read PowerPacker-crunched
modules straight off Amiga disk images — the common case for music disks,
not a special one.

## Licence

GPL-2.0-or-later. The 198x emulator/tooling family is copyleft throughout — it
composes freely with the GPL retro-computing ecosystem most Amiga tooling
lives in. If you build on this crate, your work inherits those terms.

[Play198x]: https://github.com/play198x/play198x
