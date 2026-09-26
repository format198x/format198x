# The Spectrum and C64 BASIC tokenisers belong in Format198x

**Status:** Active

**Decided:** 2026-09-26

## Decision

The ZX Spectrum and Commodore 64 BASIC tokenisers are published as
`format198x-sinclair-zx-spectrum-bas` and `format198x-commodore-c64-bas`.
Emu198x consumes them but does not own a private copy.

Both crates cover the text-listing boundary: a numbered text listing in, the
bytes the machine stores in program memory out — and, as that work lands,
back out again as the machine's own `LIST` would show it. They are
tokenisers and listers, not a BASIC parser or language toolchain; a
grammar-level parser for the language is a separate, planned project, not
part of Format198x's remit.

Each crate graduated unchanged: same public interface, same tests, same
behaviour, only the crate name and workspace changed.

## Why now

Build198x needs a `basic` verb — writing runnable BASIC tapes and PRGs from
text listings — making it a consumer that is not the producer. Per
[`../../../decisions/formats-graduate-to-their-own-projects.md`](../../../decisions/formats-graduate-to-their-own-projects.md),
a format crate graduates from the project that first needed it once
something outside that project consumes it; adding a second private copy in
Build198x, or reaching into Emu198x's workspace, would both put a reusable
file-format contract inside one consumer.

Emu198x still carries its own `format-sinclair-zx-spectrum-bas` and
`format-commodore-c64-bas` crates until it switches over to the published
ones and removes the local copies. That switch is tracked by the Emu198x
issue "Use the published format198x BASIC tokenisers", filed in Task 13 of
this plan.

## Evidence

- Both source crates already had no `emu198x`-workspace dependencies and no
  cross-crate `crate::` references outside themselves, so graduating them
  unchanged carries no hidden coupling.
- The carried test suites — 28 Spectrum tests, 10 C64 tests — pass unmodified
  in the new workspace, and `cargo clippy --all-targets -- -D warnings` is
  clean under this workspace's lints.
