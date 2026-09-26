# format198x-sinclair-zx-spectrum-bas

Tokenise numbered ZX Spectrum BASIC listings in Rust — plain text to the bytes
the ROM stores in program memory. Dependency-free (`core`/`std` only) and
deterministic.

A Spectrum program in memory is not the text you typed: each keyword is a
single token byte, and every number carries a hidden five-byte floating-point
value after its printed digits. This crate reproduces that layout so the
output can be poked straight into RAM or written to a tape or disk image.

## Tokenise a listing

```rust
use format198x_sinclair_zx_spectrum_bas::tokenise_listing;

let source = "10 PRINT \"Hello, world!\"\n20 GO TO 10\n";
let program = tokenise_listing(source)?;
std::fs::write("hello.bin", &program.bytes)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`tokenise_listing` preserves the source text exactly — strings, `REM`
comments, and even typos are carried through unchanged, because the ROM
judges syntax, not this crate. `tokenise` and `parse` take the same numbered
listing through an AST instead, for callers that want to inspect or verify
the program's structure before serialising it.

## List a program

```rust
use format198x_sinclair_zx_spectrum_bas::{list, tokenise_listing};

let program = tokenise_listing("10 PRINT CHR$(147)\n")?;
assert_eq!(list(&program.bytes)?, ["  10 PRINT CHR$ (147)"]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`list` prints stored bytes as the 48K ROM's LIST does — four-column line
numbers and the ROM's own spacing around keywords — without screen wrapping,
stopping where the variables area starts. Its tests compare against LIST
output captured from the genuine ROM. `list_line` prints one stored line
body, and `listed_form` gives what LIST would print for each line of a source
listing, with its 0-based source index:

```rust
use format198x_sinclair_zx_spectrum_bas::{list_line, listed_form};

// One stored line body, without its number, length or 0x0D.
assert_eq!(list_line(20, &[0xE2]), "  20 STOP ");

// The ROM prints a space after a keyword that ends a line; trim if comparing.
let listed = listed_form("\n10 IF a=1 THEN STOP\n")?;
assert_eq!(listed[0].0, 1);
assert_eq!(listed[0].1.trim_end(), "  10 IF a=1 THEN STOP");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Lex a line

`lex_line` splits one source line into its number and the pieces it stores,
each with its kind and its byte offset, for editors and diagnostics:

```rust
use format198x_sinclair_zx_spectrum_bas::{PieceKind, lex_line};

let line = lex_line("20 PRINT a")?;
assert_eq!(line.number, 20);
assert_eq!(line.pieces[0].kind, PieceKind::Keyword(0xF5)); // PRINT
assert_eq!(line.pieces[1].text, "a");
// Piece columns are within the body; add body_column for the source column.
assert_eq!(line.body_column + line.pieces[1].column, 9);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Errors

`tokenise_listing`, `lex_line` and `listed_form` return a `ListingError` with
the 1-based source line kept apart from the message, so a tool can print
`in.bas:LINE: message` directly:

```rust
use format198x_sinclair_zx_spectrum_bas::tokenise_listing;

let error = tokenise_listing("10 PRINT 1\n20 PRINT \"oops\n").expect_err("unclosed string");
assert_eq!(error.line, 2);
assert_eq!(error.message, "missing closing quotation mark");
```

`tokenise` and `parse`, the analysis route, return plain `String` messages.

## Licence

GPL-2.0-or-later.

Graduated from emu198x's `format-sinclair-zx-spectrum-bas` crate.
