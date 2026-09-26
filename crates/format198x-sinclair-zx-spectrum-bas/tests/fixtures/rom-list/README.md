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
EMU198X_ZX_SPECTRUM_PKG='<path to the @emu198x/zx-spectrum package>' \
SPECTRUM_48K_ROM='<path to the 48K ROM>' \
node crates/format198x-sinclair-zx-spectrum-bas/tests/fixtures/rom-list/capture.mjs
```

The script refuses a package whose version differs from the one in the table
above, so a capture with a different emulator build cannot silently replace
this one; update both together.

The script boots for 3000 ms, installs `cases.bas` with `runBasic` (line 1 is
`STOP`, so nothing else runs), then for each line types `CLS` and a `LIST`,
reads `screen.text.lines`, and joins that line's rows up to the next line's
number or a blank row. Keys are held 100 ms and released for 200 ms.

It lists from one past the previous line number rather than `LIST n`: `LIST n`
sets the edit line to `n`, and the ROM then prints the `>` cursor in place of
the space after the number. Listing from a number with no line still starts at
line `n` but marks nothing.

`runBasic` uses the emulator's own tokeniser, a separate implementation from
this crate's `tokenise_listing`. The two no longer store identical bytes for
every case: `tokenise_listing` now drops a single source space before a
keyword such as `THEN` or `OR` (the ROM reprints it on LIST — see
`fn rom_leading_space`), while `runBasic` still stores that space. The
captured listings agree regardless, because a *stored* space suppresses the
ROM's own leading-space insertion exactly as a dropped one does: either way,
LIST prints the same text. That equivalence is what makes this fixture a
valid oracle for `list()` despite the two tokenisers disagreeing — but it does
not hold universally: see the RND/INKEY$/PI note below for a divergence where
it fails.

## Cases

- Every token from `0xA5` (RND) to `0xFF` (COPY) except `0xCE` (DEF FN), which
  `tokenise_listing` does not accept: functions after `PRINT`, operator words
  between values, statements after `:`.
- Line numbers 1, 10, 100, 1000 and 9999; strings holding keywords; `REM`
  with doubled spaces and on its own.
- Lines 430–460: the trailing space after `LLIST`/`RETURN` mid-line, and
  `THEN`/`LINE` typed directly after a value with no source space.
- Lines 470–490: spaces inside numbers (`1.5 E3`, `. 5`, `BIN 1 0 1`) and
  names (`a 1`), and after numbers. This checks only how they list; what the
  ROM stores for them, hidden numbers included, is checked by
  [`../rom-editor`](../rom-editor/README.md), which types lines into the
  ROM's own editor.
- Lines 2010–2400: forty lines from the Code198x Spectrum BASIC samples
  (`code-samples/sinclair-zx-spectrum/basic/*/unit-*/*.bas`), renumbered.

## A source space after RND, INKEY$, PI is not exercised here

`tokenise_listing` keeps a source space typed after RND, INKEY$ or PI (they
take no arguments and get no ROM-side spacing on LIST either), but `runBasic`
below is "the emulator's own tokeniser" (a direct-to-RAM installer, not a
keystroke-accurate simulation of the ROM's line editor) and always absorbs
that space, matching this crate's *old* behaviour. Capturing a case like
`PRINT RND * 4` here would therefore assert the old, wrong behaviour, not the
genuine ROM's. Lines 2030, 2120 and 2370 (copied from Code198x samples that
naturally write `RND * n`) have that one space removed for this reason; the
fix itself is covered by unit tests in `src/listing.rs` instead, which check
`tokenise_listing`'s stored bytes directly rather than round-tripping through
`runBasic`.
