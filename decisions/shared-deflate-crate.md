# DEFLATE is a shared crate, and format crates may depend on it

**Status:** Active

**Decided:** 2026-10-25

## Decision

Format198x has its own DEFLATE crate, `format198x-deflate`: inflate and
deflate (RFC 1951) with the zlib (RFC 1950) and gzip (RFC 1952) wrappers.
Format crates whose files carry DEFLATE payloads depend on it, and on
nothing else. This is the one exception to "no dependencies": a format
crate may depend on another Format198x crate that holds a shared codec, but
never on a crate from outside the family.

The first dependents are:

| Crate | Payload |
|---|---|
| `format198x-acorn-uef` | the whole image, gzipped |
| `format198x-sinclair-zx-spectrum-szx` | `RAMP` pages, zlib |
| `format198x-sinclair-zx-spectrum-rzx` | snapshots and frame streams, zlib |

Steve decided this on 2026-10-25, closing the open question left by point 6
of [`emu198x-layouts-graduate-together.md`](emu198x-layouts-graduate-together.md).

## Why

**A crate that cannot read real files does not stand alone.** Most UEF files
in circulation are gzipped, and SZX and RZX writers compress by default.
Keeping compressed bytes "as stored" made every consumer bring its own
inflate, which is what Emu198x did with `flate2`. The point of graduating a
format is that a stranger can depend on one crate and read the files.

**One inflate, not three.** Each of these crates could have carried a
private inflate. That is three copies of the subtlest code in the workspace,
each needing its own hostile-input tests. A shared crate is tested once, as
hard as it deserves.

**Why not `flate2` or `miniz_oxide`.** Both are good. But the family's rule
is that a Format198x crate pulls nothing from outside, so a consumer's
dependency tree and licence review end at this workspace. DEFLATE is small
and fully specified, and an implementation tested against the reference
zlib costs less than the exception would.

**Why it lives here, though nobody graduated it.** A Format198x crate
normally arrives from the project that first needed it
([`formats-graduate-to-their-own-projects.md`](https://github.com/stevehill1981/198x/blob/main/decisions/formats-graduate-to-their-own-projects.md)).
DEFLATE never lived in a family project: Emu198x borrowed `flate2`. Its
first consumers are Format198x crates themselves, so this workspace is where
it is first needed.

## Naming

`format198x-deflate`, under the umbrella's
[crate-naming record](https://github.com/stevehill1981/198x/blob/main/decisions/crate-naming.md).
DEFLATE has no manufacturer or system: it is one byte grammar, read the same
way whichever machine's file holds it. That is the named-standard refinement
that gave `format198x-tzx` its name; the test is one parser with no branch
asking which system is reading, and DEFLATE passes it. The name is
`deflate`, not `zlib` or `gzip`, because the crate's core is the RFC 1951
stream and the other two are wrappers around it. The `format198x-` prefix
keeps it off the generic `deflate` name.

## Rules for the dependency

- **Version and path together.** Dependents write
  `format198x-deflate = { version = "0.1.0", path = "../format198x-deflate" }`,
  so the workspace builds from source and crates.io resolves the version.
- **The shared crate publishes first.** `cargo publish` for a dependent fails
  until `format198x-deflate` is on crates.io, so it leads any release that
  includes them.
- **Dependents re-export it** as `deflate`, so a caller can name
  `Options` and `Level` without adding the dependency themselves.
- **Compressed data is still kept as stored** where the format keeps it
  inside a chunk or block (SZX, RZX), and inflated on request. That keeps
  files writing back byte for byte. UEF's gzip layer wraps the whole file,
  so `Uef::parse` inflates it and `Uef::encode_gzipped` writes it again.

## Drift triggers

- A format crate depending on anything outside this workspace, for
  compression or anything else. Add the codec here, or go without.
- A second compression codec appearing inside a format crate. If two
  crates need it, it is a shared crate like this one.
- `format198x-deflate` growing a dependency, or losing `no_std`.
- Treating "a format crate may depend on a shared codec" as "format crates
  may depend on each other". A UEF crate depending on a TZX crate couples two
  formats; that is not this exception.
