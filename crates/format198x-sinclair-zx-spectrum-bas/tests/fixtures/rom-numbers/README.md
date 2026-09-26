# Hidden number forms computed by the 48K ROM

`numbers.tsv` holds, for each of 1620 number spellings, the five-byte form
the genuine Spectrum 48K ROM computes for it: spelling, a tab, ten hex
digits. `tests/rom_numbers.rs` tokenises `PRINT <spelling>` and checks the
hidden number the crate stores against it. Never edit `numbers.tsv` by hand:
change the list in `capture.mjs` and capture again.

## Capture

| | |
|---|---|
| ROM | `48.rom`, SHA1 `5ea7c2b824672e914525d1d5c419d71b84a426a2` (the script refuses any other) |
| Emulator | `@emu198x/zx-spectrum` 0.4.0, headless |
| Captured | 2026-09-26 |

```bash
EMU198X_ZX_SPECTRUM_PKG='<path to the @emu198x/zx-spectrum package>' \
SPECTRUM_48K_ROM='<path to the 48K ROM>' \
node crates/format198x-sinclair-zx-spectrum-bas/tests/fixtures/rom-numbers/capture.mjs
```

The spellings are fixed edge cases plus a seeded pseudo-random spread:
integers up to 65535 and beyond, decimals with up to nine fraction digits,
fractions with a leading point or trailing zero, and exponents in `E`, `e`,
`E+` and `E-` forms down to underflow. Numbers the calculator cannot hold
stop the program with report 6, so they are left out here; the
[`../rom-editor`](../rom-editor/README.md) capture shows the ROM refusing
them.

In batches of 300, the script installs a program that `READ`s each spelling
as a string and stores `VAL s$` in an array, runs it, and reads the array
from the variables area. `VAL` checks its string's syntax before evaluating
it, and on that pass S-DECIMAL (`268D`) calls DEC-TO-FP (`2C9B`), just as
when a line is entered, so the stored value is the form the editor would
insert. The ROM-editor capture confirms it for the spellings both share
(`.5`, `.25`, `0.1`, `1.5E-3` and others). The program itself is installed
with the emulator's `runBasic`; only its `DATA` strings matter here.
