# Encode live sector images as EDSK

Consumer: Emu198x #45 needs explicit export of a modified +3 disk. The user
approved shipping the format prerequisite independently before runtime wiring.
No dependencies, model/schema changes, FDC behaviour or host writeback API.

1. Extend umbrella `reference/by-system/amstrad-cpc/cpcemu-dsk-container.md`
   from the format authors' Extended DSK Rev.5 specification. Inspect Arnold,
   MAME and the frozen earlier writer for precedent; copy no archived code.
2. Add `src/encode.rs` and re-export `encode_extended(&DiskImage)` from
   `src/lib.rs`. Validate geometry, exact model dimensions, the 204-entry
   table, 29-sector header and 255-page track bounds before encoding.
   Preserve physical order, CHRN, ST1/ST2 and exact stored lengths/data;
   empty tracks become zero table entries. Document unrepresented metadata.
3. Add `tests/extended.rs`: independently authored byte expectations, mixed
   lengths/status/CHRN/duplicate IDs, double-side interleaving, empty tracks,
   valid limits and deliberate invalid models. Round-trip a normal +3 image.
4. Add a small conversion example for real-input verification, writing only
   to a newly created output. Run an independent reader/runtime on converted
   MIT qualification media; do not count this as live runtime export/reopen.
5. Update the crate README and DSK decision. Run affected tests, doc tests,
   workspace formatting/Clippy/tests and package verification. Push a reviewed
   PR, rebase merge after CI, author release notes and ship a compatible patch
   through the existing release workflow. Verify registry/tag identities.
6. Link exact results and remaining runtime work from Emu #45 and umbrella
   progress without advancing the seven-milestone qualification claim.
   Preserve evidence and remote commits; remove both task worktrees/caches.
