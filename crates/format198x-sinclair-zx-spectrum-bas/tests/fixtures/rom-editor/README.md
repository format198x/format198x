# Lines typed into the 48K ROM's own editor

`cases.hex` is what the genuine Spectrum 48K ROM stores when each line of
`cases.bas` is typed into its line editor key by key and entered, one line of
hex per program line (number, length, body, `0x0D`). `refused.bas` holds lines
the ROM will not store: typing one leaves the program empty. `run.txt` is the
screen after `RUN`. `tests/rom_editor.rs` checks `tokenise_listing` against
the first two, so the ROM, not our reading of its disassembly, decides where
spaces and hidden numbers go. Never edit these files by hand: change the
lines in `capture.mjs` and capture again.

## Capture

| | |
|---|---|
| ROM | `48.rom`, SHA1 `5ea7c2b824672e914525d1d5c419d71b84a426a2` (the script refuses any other) |
| Emulator | `@emu198x/zx-spectrum` 0.4.0, headless |
| Captured | 2026-09-26 |

```bash
EMU198X_ZX_SPECTRUM_PKG='<path to the @emu198x/zx-spectrum package>' \
SPECTRUM_48K_ROM='<path to the 48K ROM>' \
node crates/format198x-sinclair-zx-spectrum-bas/tests/fixtures/rom-editor/capture.mjs
```

Run it from the workspace root; it needs `cargo` for the last step.

The script boots for 3000 ms and types each line of `PROGRAM` as a Spectrum
owner would: keywords with their keys in K or E mode, everything else
character by character, keys held 100 ms and released for 200 ms. It reads
the program from `PROG` to `VARS`, types `RUN`, and records the screen. Each
line of `REFUSED` is typed on a fresh machine and must leave the program
empty.

Then it runs this crate's bytes on the same ROM: it tokenises `cases.bas`
with `cargo run --example tokenise_listing`, wraps the bytes in a TAP that
auto-runs from line 5, loads it with the package's `load` and `autoload`, and
fails unless the screen matches `run.txt`. The hidden numbers are what `PRINT`
prints, so this checks their values by running them, not by reading them.

## What the test compares

Every stored byte, hidden numbers included. The ROM computes a number's
hidden form with its own calculator as it reads the digits, so `.5` is
stored as `7F 7F FF FF FF` (just under a half) and `1.5 E3` as 1500 in the
full floating-point form; the crate repeats that arithmetic. The TAP check
compares what `PRINT` shows, which rounds those differences away, so it
confirms the program runs as typed rather than the hidden bytes themselves.
[`../rom-numbers`](../rom-numbers/README.md) checks many more numbers.

## Where the ROM skips spaces in a number

DEC-TO-FP (`2C9B`) reads the whole part with INT-TO-FP (`2D3B`), which steps
with CH-ADD+1 (`0074`) and stops at a space, so `1 000` is 1 followed by
`000` and the line is refused. After a decimal point it steps with NEXT-CHAR
and GET-CHAR (`0020`, `0018`), which skip spaces (SKIP-OVER, `007D`), so
`1. 5`, `1.5 5`, `1.5 E3` and `1.5E- 3` are each one number. BIN's digits
(BIN-DIGIT, `2CA2`) are read with NEXT-CHAR too. S-DECIMAL (`268D`) then
opens the room for the hidden number at GET-CHAR, past any spaces after the
number, so those spaces are stored before the `0x0E`. LOOK-VARS (`28B2`)
reads a name with NEXT-CHAR (V-CHAR, `28D4`) and matches it ignoring spaces
(V-SPACES, `2913`), so `a 1` is the variable `a1`.
