# format-commodore-amiga-mod

Parse and write Amiga **ProTracker MOD** modules in Rust: 31 sample slots,
pattern data, and the order table. Dependency-free (`core`/`std` only),
bidirectional (`decode`/`encode`), lossless (`encode(decode(bytes)) ==
bytes`, verified against 45 real Amiga music-disk modules across two
independent corpora — see the task report), and panic-free on malformed
input.

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
    println!("{} — {} samples, {} patterns", module.title(), module.samples.len(), module.patterns.len());

    let rebuilt = encode(&module)?;
    assert_eq!(rebuilt, bytes); // lossless: every byte round-trips
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

## Hidden patterns

The pattern count is **not** reliably derivable from the order table at
all — the widely-cited community MOD spec glosses over this entirely. Some
real files store pattern data referenced only by order-table slots past the
song length ("hidden" patterns that never play but are still physically
present); but other real files leave non-zero leftover garbage in that same
unplayed tail that does *not* correspond to a stored pattern. Nothing in
the table tells the two cases apart — one real file's garbage byte implied
233 patterns where only 9 were physically present.

`decode` instead derives the pattern count from the file's own arithmetic:
header (1084 bytes), patterns, and all 31 samples' PCM data are contiguous
and exhaustive, so pattern data is exactly `file length - 1084 - total
sample bytes`, independent of the order table. Verified exact on every file
across two independent real-media corpora, including both the
hidden-pattern files and the garbage-tail files. `encode` always writes
back exactly `patterns.len()` patterns, so this data round-trips like
everything else.

## Lossless: raw fields plus ergonomic accessors

An editor (Studio198x's tracker, the reason this matters) that opens a
module, changes one note, and saves it must not silently degrade every byte
it didn't touch. So `Module` and `Sample` store the file's raw bytes and
words directly — `title_bytes`, `name_bytes`, `order_table`, `restart`,
`magic`, `finetune_byte`, `repeat_start_words`, `repeat_length_words` —
rather than a value derived from them. Nothing is thrown away: a decoded
module's entire byte content survives, including bytes ProTracker itself
never reads (a name's leftover bytes past its NUL, order-table padding past
the song length, a finetune byte's unused upper nibble, the specific
"no loop" encoding a sample used, the restart byte, the exact magic
variant).

Raw byte arrays aren't pleasant to work with directly, so every field with
a more useful shape also has an accessor: `title()`/`name()` return the
trimmed, readable `&str`; `orders()` returns the order table's played
prefix; `finetune()` returns the signed nibble value; `loop_start()`,
`loop_len()`, and `is_looped()` give the loop points in bytes. Read through
the accessors; write through the raw fields (or leave them as `decode` set
them) so nothing is lost on the way back out.

This was verified, not assumed: an earlier version of this crate trimmed
and normalised these fields, and re-encoding a corpus of real Amiga
music-disk modules came back 0/17 byte-identical — every divergence traced
to exactly these fields. Storing them raw instead brought that to 17/17,
and a second, independent 28-module corpus (which additionally exercised
the hidden-pattern rule above) came back 28/28.

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
