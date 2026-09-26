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
numbers and the ROM's own spacing around keywords — without screen wrapping.
Its tests compare against LIST output captured from the genuine ROM.

## Licence

GPL-2.0-or-later.

Graduated from emu198x's `format-sinclair-zx-spectrum-bas` crate.
