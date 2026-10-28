# Strict DSK container view for Einstein #277

The user approved an additive, opt-in strict API and a published consumer update.
Keep existing parse/DiskImage behaviour and literals source-compatible.

1. Add container.rs: interleaved track slots, optional physical TIB metadata,
   sector CHRN/status/data, strict header/table/descriptor/payload bounds.
   Retain zero-length EDSK sectors and distinguish unformatted slots from
   present zero-sector track headers. Standard DSK derives sizes without
   clamping; no controller geometry or status policy enters this crate.
2. Share sector decoding internally, with legacy parse retaining its existing
   choices. Test legacy compatibility alongside strict opt-in differences.
3. Test independent binary standard/extended fixtures, displaced track metadata,
   malformed tables/counts/extents, large N, zero-length and unformatted tracks.
   Read format-author evidence in the umbrella source library before coding.
4. Run crate tests, workspace tests/Clippy, docs and fmt. Push a checked PR,
   rebase merge only after complete green CI. Publish through the existing
   per-crate release flow; verify registry bytes before Emu consumption.
5. Verify remote retention and clean temporary worktree/build caches.
