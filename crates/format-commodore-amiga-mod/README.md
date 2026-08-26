# format-commodore-amiga-mod

Parse and write Amiga **ProTracker MOD** modules in Rust: 31 sample slots,
pattern data, and the order table. Dependency-free (`core`/`std` only),
bidirectional (`decode`/`encode`), and panic-free on malformed input.

**This crate parses and writes only. It does not play modules.** There is no
mixer, no tick loop, and no effect processing here — playback lives in
`play198x-core`, because tick scheduling and effect dispatch are playback
semantics rather than file layout. `Note` carries the raw effect number and
parameter byte exactly as the file stores them; interpreting what effect `4`
(vibrato) or `9` (sample offset) *means* during playback is the player's job,
not this crate's. If you find yourself reaching for this crate to schedule
ticks or mix samples, that work belongs in `play198x-core` instead.

## Decode and encode

```rust
use format_commodore_amiga_mod::{decode, encode, is_module};

let bytes = std::fs::read("tune.mod")?;
if is_module(&bytes) {
    let module = decode(&bytes)?;
    println!("{} — {} samples, {} patterns", module.title, module.samples.len(), module.patterns.len());

    let rebuilt = encode(&module)?;
    // `rebuilt` matches `bytes` byte-for-byte for the common case — see
    // "What a round-trip cannot preserve" below for the two exceptions.
}
```

## Identification

Identify a module by the magic at offset 1080 (`is_module`), never by file
extension — `.mod` is a convention, not a promise, and other trackers
(Startrekker, for one) reused this exact byte layout under other extensions.
`is_module` is a cheap sniff, not a validation: `decode` does the real
checking and can still fail (or reject the channel count) on bytes the sniff
accepts.

## Scope: 4-channel modules only

`Note`'s pattern rows are fixed at 4 channels, matching the classic `M.K.`,
`M!K!`, `FLT4`, and `4CHN` formats. A 6- or 8-channel module (`6CHN`, `8CHN`)
is recognised by `is_module` but rejected by `decode` with
`DecodeError::UnsupportedChannelCount` — this crate cannot represent a wider
pattern row without corrupting it, so it says so rather than misparsing.

## What a round-trip cannot preserve

`Module`'s fields hold the *meaningful* content of a file (a title trimmed
at its terminator, an order table trimmed to the song length, a loop flag
rather than a raw repeat length), not its raw bytes byte-for-byte. Verified
against 17 real Amiga music-disk modules (see the task report): every one
diverged from `encode(decode(bytes))` only in these ways — never in pattern
data, sample PCM, or any other header field:

- **The restart byte** (offset 951) — "historically set to 127, but can be
  safely ignored" per the community format documentation. `encode` always
  writes `0`.
- **The magic variant** when it isn't `M.K.` — `encode` always writes
  `M.K.`, even if the original used `M!K!`, `FLT4`, or `4CHN`.
- **Bytes trailing a name or the title past its first NUL.** Real files
  routinely leave non-zero leftover bytes there; `decode` trims at the
  first NUL (so `title`/`name` hold a clean string), and `encode`
  zero-pads instead of restoring the leftovers.
- **Order-table bytes past the song length.** `orders` holds only the used
  prefix; `encode` zero-pads the rest of the 128-entry table.
- **A loop length of exactly one word.** Both `0` and `1` words mean "no
  loop" and decode to `loop_len = 0`; `encode` always writes `0` back.
- **A finetune byte's unused upper nibble** — discarded on decode, always
  written back as zero.

Every other byte round-trips exactly: every pattern cell, every sample's
PCM data, every sample length/volume/loop-start, the song length, and the
magic when it is `M.K.`.

## Bounds-checked against hostile input

This crate sits behind an FFI boundary in the wider Play198x player, where a
panic is undefined behaviour. `decode` treats every length it reads from the
file as untrusted: a song length longer than the 128-entry order table, an
order byte that pushes the implied pattern count past what the file actually
holds, or a sample length that overruns the file, all become a typed
`DecodeError` rather than an out-of-bounds index or an arithmetic overflow
panic. `encode` takes a well-typed `Module` rather than raw bytes, but still
rejects a shape the format cannot hold (the wrong sample count, a pattern
that isn't 64 rows, a note field too wide for its nibble) with a typed
`EncodeError` rather than silently truncating it.

## Playback semantics, and why they're not here

The tick-scheduling and effect-dispatch rules ProTracker actually plays
by — including a place where the widely-cited community MOD specification
is wrong about the vibrato rate — are documented separately in
[`protracker-playback-reference.md`][ref], distilled from the ProTracker
2.3B replayer source. That document is about what a player does with the
`effect`/`param` bytes this crate hands back; it has no bearing on this
crate's byte layout, and this crate implements none of it.

[ref]: https://github.com/198x/reference/blob/main/by-topic/music-formats/protracker-playback-reference.md

## Part of the 198x family

This crate lets [Play198x], the retro media player, read and write
ProTracker modules straight off Amiga disk images.

## Licence

GPL-2.0-or-later. The 198x emulator/tooling family is copyleft throughout — it
composes freely with the GPL retro-computing ecosystem most Amiga tooling
lives in. If you build on this crate, your work inherits those terms.

[Play198x]: https://github.com/play198x/play198x
