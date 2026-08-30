# Oric TAP belongs in Format198x

**Status:** Active

**Decided:** 2026-08-30

## Decision

The Oric TAP byte-stream codec is published as
`format198x-tangerine-oric-tap`. Emu198x consumes it but does not own a private
copy.

The crate covers the file-format boundary: leaders, sync, the ROM's nine-byte
header, name, address-sized contents, concatenation, and deterministic writing.
It does not turn those bytes into cassette edges, drive VIA CB1, model the
motor, or choose playback timing; those are machine behaviours and remain in
Emu198x.

Atmos `STORE` array records are initially rejected. Their header fields do not
carry a reliable ordinary-file byte length because of ROM bugs, so accepting
them by scanning for a plausible next leader would make preservation depend on
a guess. They can be added once the representation preserves that ambiguity.

## Why now

Emu198x issue #338 is the first consumer. Adding the codec directly to the
emulator would immediately put a reusable historical media contract inside one
consumer, contrary to the umbrella graduation rule.

## Evidence

- The OSDK `Header` and `Tap2Dsk` sources independently write/read the same
  leader, sync, nine-byte header, NUL name, and inclusive address-sized body.
- Oricutron `tape.c` consumes the same byte stream and reconstructs the serial
  waveform at the machine boundary.
- The *Oric Atmos Handbook* and *The Oric-1 Companion* in `reference/by-system/oric/`
  document names, BASIC/memory-block loading, start/end addresses, autorun, and
  fast/slow cassette behaviour.
