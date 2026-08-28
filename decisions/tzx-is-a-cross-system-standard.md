# TZX is named as a cross-system standard

**Status:** Active

**Decided:** 2026-08-28

## Decision

The dependency-free block codec shared byte-for-byte by ZX Spectrum TZX and
Amstrad CPC CDT is published as `format198x-tzx`.

System-facing playback adapters remain system-namespaced. They consume the
neutral block codec and own clock conversion and machine behaviour; the codec
does not depend on either system.

## Why this is not an ambiguous extension

Spectrum TAP and Commodore TAP are unrelated formats which happen to share an
extension, so their crates must name their systems. TZX and CDT are the
opposite case: CDT deliberately uses the TZX grammar, including pulse lengths
expressed against TZX's 3.5 MHz reference clock. Two independent codecs would
encode the same contract twice and drift.

The standard name remains under the mandatory `format198x-` registry prefix,
so it does not claim the generic `tzx` crate name.

## Boundary

This crate owns file framing, typed block boundaries, and lossless encoding.
It does not emit a machine-facing pulse stream. If it ever needs to ask which
machine is reading, the boundary has drifted and the interpretation belongs in
the system adapter instead.
