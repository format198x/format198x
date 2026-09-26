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

## Licence

GPL-2.0-or-later.

Graduated from emu198x's `format-commodore-c64-bas` crate.
