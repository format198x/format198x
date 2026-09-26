# format198x-commodore-c64-bas

Tokenise numbered Commodore 64 BASIC V2 listings in Rust — plain text to a PRG
loading at `$0801`. Dependency-free (`core`/`std` only) and deterministic.

A C64 program in memory is a chain of lines, each prefixed by a pointer to the
next line and its line number, with keywords packed into single token bytes
and text converted to PETSCII. This crate reproduces that layout, including
the zero-pointer end marker the BASIC interpreter expects.

## Tokenise a listing

```rust
use format198x_commodore_c64_bas::tokenise;

let source = "10 PRINT \"HELLO, WORLD!\"\n20 GOTO 10\n";
let program = tokenise(source)?;
std::fs::write("hello.prg", &program.bytes)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Keywords are tokenised wherever they appear outside strings, `REM` and `DATA`
values, as the C64 does: `GOTO10` and `FORI=1TO10` tokenise, and `SCORE`
stores the `OR` token in the middle. Lines are stored in line-number order.
Blank lines are skipped; any other line must start with a line number.

## Errors

`tokenise`, `lex_line` and `listed_form` return a `ListingError` with the
1-based source line kept apart from the message, so a tool can print
`in.bas:LINE: message` directly. `list` returns the same type for a
truncated PRG, with `line` 0, since it reads bytes rather than source lines:

```rust
use format198x_commodore_c64_bas::tokenise;

let error = tokenise("10 PRINT 1\n10 PRINT 2\n").expect_err("duplicate line");
assert_eq!(error.line, 2);
assert_eq!(error.message, "line 10 appears twice; edit its existing line");
```

## List a program

```rust
use format198x_commodore_c64_bas::{list, list_line, listed_form, tokenise};

let program = tokenise("10 print \"hi\":goto10\n")?;
assert_eq!(list(&program.bytes)?, ["10 PRINT \"HI\":GOTO10"]);

// One stored line body, without its link, number or 0x00 terminator.
assert_eq!(list_line(20, &[0x99, b' ', b'1']), "20 PRINT 1");

// What LIST would print for each source line, with its 0-based source index.
assert_eq!(listed_form("\n10 print 1\n")?, [(1, "10 PRINT 1".to_string())]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`list` and `list_line` print stored bytes as the C64's LIST does: tokens
expand to keywords outside quotes, and everything else prints as stored.
PETSCII graphics and control codes are not rendered.

## Lex a line

`lex_line` splits one source line into its number and the pieces it stores,
each with its kind and its byte offset, for editors and diagnostics:

```rust
use format198x_commodore_c64_bas::{PieceKind, lex_line};

let line = lex_line("20 IFA=1THEN20")?;
assert_eq!(line.number, 20);
assert_eq!(line.pieces[0].kind, PieceKind::Keyword(0x8B)); // IF
assert_eq!(line.pieces[1].text, "A");
// Piece columns are within the body; add body_column for the source column.
assert_eq!(line.body_column + line.pieces[1].column, 5);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Licence

GPL-2.0-or-later.

Graduated from emu198x's `format-commodore-c64-bas` crate.
