# Finish moving Emu198x's format layouts

The umbrella decision `emu198x-format-layouts-move-to-format198x.md` moves
every Emu198x `format-*` layout here. #75 moved 25 and Emu198x now uses them.
Nine pure layouts remain in Emu198x at emu198x/emu198x@32c46c10f: two that
#75 held back (NES, Game Boy) and seven created on 2026-10-25, after #75's
audit was taken.

Steve's rulings on 2026-11-07: work in this order, and the Neo Geo ROM-set
loader is a format. Its crate takes and returns MAME-named `(name, bytes)`
pairs. Unzipping stays in Emu198x's adapter; a family zip crate would need
its own decision.

Every crate follows `decisions/emu198x-layouts-graduate-together.md`: one
import commit naming the source commit, no dependencies, a writer for every
reader with round-trip tests, a CHANGELOG (release-plz needs it on first
publish), a README with a doctested example, and a row in `README.md`.

## PR A: Nintendo layouts

| Emu198x crate | Format198x crate | Work beyond the import |
|---|---|---|
| `format-nintendo-nes-ines` | `format198x-nintendo-nes-ines` | Writer: header (iNES and NES 2.0, plain and exponent sizes), trainer, PRG, CHR. |
| `format-nintendo-famicom-fds` | `format198x-nintendo-famicom-fds` | None; it reads and writes already. |
| `format-nintendo-snes-sfc` | `format198x-nintendo-snes-sfc` | Writer: add or strip the copier header, and set the checksum pair. |
| `format-nintendo-game-boy-cartridge` | `format198x-nintendo-game-boy-cartridge` | Drop `serde` and `thiserror`. Writer: header fields, header checksum and global checksum. |

## PR B: disk and microdrive images

| Emu198x crate | Format198x crate | Work beyond the import |
|---|---|---|
| `format-atari-st-st` | `format198x-atari-st-st` | Check the writer covers every geometry the reader accepts. |
| `format-atari-st-msa` | `format198x-atari-st-msa` | Writer exists; round-trip it against `.st`. |
| `format-acorn-adfs` | `format198x-acorn-adfs` | Sector image read and write. Writing catalogue entries is a follow-up, as for D71 and D81. |
| `format-sinclair-zx-spectrum-mdr` | `format198x-sinclair-zx-spectrum-mdr` | None; it reads and writes already. |

## PR C: Neo Geo ROM set

`format-snk-neo-geo-romset` becomes `format198x-snk-neo-geo-romset`. Drop
`zip` and `thiserror`; the zip entry point stays in Emu198x. Add the writer:
regions back to MAME-named files (P-ROM words swapped back, C-ROM pairs
split). Record the ruling as a point in
`decisions/emu198x-layouts-graduate-together.md`.

## After each PR merges

1. Merge the release PR and check each new crate's 0.1.0 is on crates.io.
   The new-crate rate limit may return 429; wait it out and use the
   reconcile dispatch, never a limit request.
2. File one emu198x issue to switch Emu198x onto the published crates. That
   work belongs to the Emu198x session.
3. Add the outcome table to emu198x#1993.

## Then the #75 follow-ups

- PR D: file-level writers for D71 and D81, matching D64's `master`.
- PR E: typed block-field views in `format198x-tzx`, so Emu198x's
  `format-tzx` stops decoding fields by hand.

## Checks for every PR

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace`, each judged by exit status.
- `cargo publish --dry-run -p <crate>` for each new crate.
- Tests the Emu198x crate had still pass. Where an independent real file
  exists, a round trip through it, not only through this crate's own writer.
- CI green by `gh pr checks --json name,bucket`, then rebase-merge.
- Remove the worktree once its branch is merged.
