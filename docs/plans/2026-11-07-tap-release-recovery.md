# Release the TAP parity correction

Release-plz run 42382256214 stopped while attempting MDR's first publication
with a Trusted Publishing token. MDR was subsequently published manually;
rerunning the failed jobs succeeded and produced TAP release PR #89.

1. Author the 0.2.0 entry in
   `crates/format198x-sinclair-zx-spectrum-tap/CHANGELOG.md`, including the new
   public field and the migration for literal constructors and tape playback.
2. Run the TAP tests and formatting; wait for every current PR check, including
   the native release-PR checks. Rebase-merge the checked release PR.
3. Verify the registry index and downloaded 0.2.0 artifact expose stored parity
   and preserve a damaged block. Notify the emulator owner only after this
   succeeds. Inspect/remove this task's worktree once commits are retained.
