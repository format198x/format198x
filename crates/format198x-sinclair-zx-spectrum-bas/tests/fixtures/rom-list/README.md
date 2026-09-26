# LIST output captured from the 48K ROM

`cases.list` is what the genuine Spectrum 48K ROM prints when it lists
`cases.bas`, one listed line per source line, with screen wrapping undone and
trailing spaces trimmed. `tests/rom_list.rs` checks the crate's `list` against
it, so the ROM, not our reading of its disassembly, decides the spacing.
Never edit `cases.list` by hand: change `cases.bas` and capture again.

## Capture

| | |
|---|---|
| ROM | `48.rom`, SHA1 `5ea7c2b824672e914525d1d5c419d71b84a426a2` (the script refuses any other) |
| Emulator | `@emu198x/zx-spectrum` 0.4.0, headless |
| Captured | 2026-09-26 |

```bash
EMU198X_ZX_SPECTRUM_PKG=/Users/stevehill/Projects/198x/Code198x/website-bpi/node_modules/@emu198x/zx-spectrum \
SPECTRUM_48K_ROM=$HOME/.emu198x/roms/sinclair-zx-spectrum-48k/48.rom \
node crates/format198x-sinclair-zx-spectrum-bas/tests/fixtures/rom-list/capture.mjs
```

The script boots for 3000 ms, installs `cases.bas` with `runBasic` (line 1 is
`STOP`, so nothing else runs), then for each line types `CLS` and a `LIST`,
reads `screen.text.lines`, and joins that line's rows up to the next line's
number or a blank row. Keys are held 100 ms and released for 200 ms.

It lists from one past the previous line number rather than `LIST n`: `LIST n`
sets the edit line to `n`, and the ROM then prints the `>` cursor in place of
the space after the number. Listing from a number with no line still starts at
line `n` but marks nothing.

`runBasic` uses the emulator's own tokeniser. On the capture date the program
it installed (PROG to VARS) was byte-identical to `tokenise_listing(cases.bas)`,
so the ROM listed the same bytes the test lists.

## Cases

- Every token from `0xA5` (RND) to `0xFF` (COPY) except `0xCE` (DEF FN), which
  `tokenise_listing` does not accept: functions after `PRINT`, operator words
  between values, statements after `:`.
- Line numbers 1, 10, 100, 1000 and 9999; strings holding keywords; `REM`
  with doubled spaces and on its own.
- Lines 2010–2400: forty lines from the Code198x Spectrum BASIC samples
  (`code-samples/sinclair-zx-spectrum/basic/*/unit-*/*.bas`), renumbered.
