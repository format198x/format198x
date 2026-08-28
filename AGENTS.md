# Format198x

> Read [`PRINCIPLES.md`](PRINCIPLES.md) first. [`MANIFESTO.md`](MANIFESTO.md) is why the project exists.

The 198x family's standalone retro disk- and media-**format** crates: small,
dependency-free Rust libraries that read and write the on-disk formats of
1970s–1990s computers. See [`../../AGENTS.md`](../../AGENTS.md) for umbrella
context and cross-project rules.

## What this is

A workspace of independent crates, each with its own version and its own
crates.io release — no shared lockstep. They exist in their own right: any Rust
tool or emulator can depend on one without taking on the rest of the family.

User-facing overview is in [`README.md`](README.md).

## Where a format comes from

A format crate is not started here. It graduates here from the project that
first needed it, once it has a standalone consumer — see
[`../../decisions/formats-graduate-to-their-own-projects.md`](../../decisions/formats-graduate-to-their-own-projects.md).
Before adding a crate, check that the format has left its originating project
for a reason that decision recognises.

## Naming

`format198x-{manufacturer}-{system}-{format}`. The org prefix is mandatory: a
registry entry has no folder to sit in, so the name is the only place its
provenance can live. See
[`../../decisions/crate-naming.md`](../../decisions/crate-naming.md).

## Evidence

A format crate encodes a claim about how bytes are laid out on real media. That
claim is answerable to the prose layers — [`../../reference/`](../../reference/)
and [`../../syntheses/`](../../syntheses/) — and to the artefacts themselves.
Cite upward when the layout is not obvious, and prefer a round-trip test against
a real image over a restatement of a secondary source.

## Releasing

Published crates follow
[`../../decisions/releasing-published-crates.md`](../../decisions/releasing-published-crates.md).
