# format198x-memotech-mtx-mtx

Read and write **Memotech MTX `.mtx` tape images and `.RUN` programs** in Rust.

A `.mtx` file is what an MTX cassette carries: an `$FF` byte and 15-byte name (absent in a common headerless variant), the system-variables base, the system variables and the BASIC program from `$4000`. A quarter of files with this extension are something else, so the reader validates rather than trusts. `.RUN` programs — a load address, a length and the code — are covered too. The crate is dependency-free and returns typed errors for malformed input.

```rust
use format198x_memotech_mtx_mtx::RunProgram;

let run = RunProgram { code_base: 0x8000, code: vec![0xC3, 0x00, 0x80] };
assert_eq!(RunProgram::parse(&run.encode()?)?, run);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Layouts and provenance: `reference/by-system/memotech-mtx/memotech-mtx-tape-and-run-formats.md` in the 198x reference library. The `.RUN` layout is documented by MEMU's author but not checked against a real file.

## Provenance

Graduated from Emu198x's `format-memotech-mtx` crate (emu198x/emu198x@c7b3834297), whose history of 1 commit stays in that repository.

## Licence

GPL-2.0-or-later.
