//! Integration tests for the PP20 decruncher: the magic sniff, truncated and
//! malformed input handling, and a real-media regression against a genuine
//! crunched Amiga module.
//!
//! The real-media test needs a file this repository never commits (the
//! no-media-in-the-repo rule) — see [`decrunches_a_genuine_pp20_module`]
//! below for how to run it.

use format198x_commodore_amiga_powerpacker::{DecodeError, MAGIC, decrunch, is_powerpacked};
use std::collections::BTreeMap;

#[test]
fn recognises_the_pp20_magic() {
    assert!(is_powerpacked(b"PP20\x09\x0a\x0c\x0d\x00\x00\x00\x00"));
    assert!(!is_powerpacked(b"FORM\x00\x00\x00\x00ILBM"));
    assert!(!is_powerpacked(b"PP"));
    assert!(!is_powerpacked(b""));
}

#[test]
fn rejects_truncated_input_without_panicking() {
    for len in 0..12usize {
        let bytes = vec![b'P'; len];
        assert!(decrunch(&bytes).is_err(), "length {len} must be rejected");
    }
}

#[test]
fn rejects_missing_magic() {
    let mut bytes = vec![0u8; 16];
    bytes[..4].copy_from_slice(b"FORM");
    assert_eq!(decrunch(&bytes), Err(DecodeError::BadMagic));
}

#[test]
fn rejects_out_of_range_offset_length_bytes() {
    // A header that passes the length and magic checks but claims an
    // offset-length byte of 255 must not turn into an oversized bit read.
    let mut bytes = vec![0u8; 16];
    bytes[..4].copy_from_slice(b"PP20");
    bytes[4] = 255;
    assert!(matches!(decrunch(&bytes), Err(DecodeError::Corrupt { .. })));
}

#[test]
fn rejects_oversized_initial_skip() {
    let mut bytes = vec![0u8; 16];
    bytes[..4].copy_from_slice(b"PP20");
    bytes[4..8].copy_from_slice(&[9, 9, 9, 9]);
    *bytes.last_mut().expect("non-empty") = 33; // skip_bits > 32
    assert!(matches!(decrunch(&bytes), Err(DecodeError::Corrupt { .. })));
}

/// Build a PP20 stream whose body encodes `reads` — a list of
/// `(value, bit_width)` pairs in the order [`decrunch`] will pull them, with
/// no initial bit-skip.
///
/// The bitstream is read backwards from the end of the body, LSB-first
/// within each byte, so read-order bit *k* lives in bit `k % 8` of body byte
/// `len - 1 - k / 8`. Encoding that here rather than hand-writing hex is what
/// makes a crafted stream readable at the call site.
fn crafted_stream(offset_lens: [u8; 4], dest_len: u32, reads: &[(u32, u32)]) -> Vec<u8> {
    let mut bits: Vec<u8> = Vec::new();
    for &(value, width) in reads {
        for i in (0..width).rev() {
            bits.push(((value >> i) & 1) as u8);
        }
    }
    let body_len = bits.len().div_ceil(8).max(1);
    let mut body = vec![0u8; body_len];
    for (k, bit) in bits.iter().enumerate() {
        if *bit == 1 {
            body[body_len - 1 - k / 8] |= 1u8 << (k % 8);
        }
    }

    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&offset_lens);
    out.extend_from_slice(&body);
    out.extend_from_slice(&[
        ((dest_len >> 16) & 0xFF) as u8,
        ((dest_len >> 8) & 0xFF) as u8,
        (dest_len & 0xFF) as u8,
        0, // initial bit-skip
    ]);
    out
}

/// The literal-run continuation code chains without limit — every `11` chunk
/// asks for three more bytes and another chunk — so a hostile body can drive
/// the accumulator past `u32::MAX` and panic on overflow. `dest_len` bounds
/// it: a literal run longer than the declared output can never succeed.
#[test]
fn a_literal_run_longer_than_the_declared_output_is_corrupt() {
    // Literal marker, then 2-bit run chunks of 3: run = 1 + 3 + 3 = 7
    // against a declared output of 4.
    let bytes = crafted_stream([9, 10, 12, 13], 4, &[(0, 1), (3, 2), (3, 2), (3, 2)]);
    assert_eq!(
        decrunch(&bytes),
        Err(DecodeError::Corrupt {
            what: "literal run length exceeds the declared output length"
        })
    );
}

/// The same unbounded chaining in the `x == 3` long-match escape, where each
/// `111` chunk adds seven to the match length and asks for another chunk.
#[test]
fn a_match_longer_than_the_declared_output_is_corrupt() {
    // Match marker, selector 3 (the long-match escape), the bit that keeps
    // the table's own offset width, a 13-bit offset, then 3-bit length
    // chunks of 7: length = 5 + 7 + 7 = 19 against a declared output of 16.
    let bytes = crafted_stream(
        [9, 10, 12, 13],
        16,
        &[(1, 1), (3, 2), (1, 1), (0, 13), (7, 3), (7, 3), (7, 3)],
    );
    assert_eq!(
        decrunch(&bytes),
        Err(DecodeError::Corrupt {
            what: "match length exceeds the declared output length"
        })
    );
}

/// A wide sweep of small, structurally-plausible-but-arbitrary byte strings
/// must never panic, whatever `decrunch` decides to return.
///
/// The sweep varies the header fields *independently* of the body, because
/// an earlier version of this test did not: it filled the whole buffer with
/// one byte, so every fill also landed in the offset-length table at
/// `bytes[4..8]`. All 208 inputs failed that table's range check in the
/// first few lines of `decrunch` and returned the identical error; not one
/// reached the bitstream reader, the back-reference bound, or the
/// decompression loop. It read as coverage and was worth nothing.
///
/// The tallies are what keep it honest, and they have to be specific to be
/// worth anything. Counting `Corrupt` in bulk is not: most of this sweep's
/// corrupts come from the offset-length table and the bit-skip range
/// check, both of which fire in the first dozen lines of `decrunch`, so
/// `corrupt > 0` would still hold if the sweep regressed all the way back
/// to the header-only version above. So the tally is kept per message, and
/// every check inside the decompression loop has to be reached by name.
///
/// The successes needed the same treatment. Every non-empty success used to
/// be exactly one byte, from the most degenerate traversal the loop has —
/// one literal, then stop — which says next to nothing about the loop.
/// `0x0B` is in the fill set because it is a fill that drives the
/// literal-run accumulator and the back-reference copy far enough to
/// produce five bytes, and the test now insists some success does more
/// than one.
#[test]
fn malformed_input_never_panics() {
    // Four offset-length tables: three legal (the range check allows 1..=15;
    // genuine files use 9..=13) and one with a zero entry, so the table
    // check is still exercised without being the only thing exercised.
    const TABLES: [[u8; 4]; 4] = [[9, 10, 12, 13], [9, 9, 9, 9], [8, 10, 11, 15], [0, 9, 9, 9]];

    // Every `Corrupt` message the decompression loop itself can return.
    // Reaching each one by name is what proves the sweep exercises the
    // loop rather than the header checks in front of it.
    const LOOP_INTERNAL: [&str; 4] = [
        "literal run length exceeds the declared output length",
        "match length exceeds the declared output length",
        "back-reference offset overruns the output buffer",
        "decrunched output overflowed its declared length",
    ];

    let mut decrunched = 0usize;
    let mut longest_output = 0usize;
    let mut truncated = 0usize;
    let mut bad_magic = 0usize;
    let mut corrupts: BTreeMap<&'static str, usize> = BTreeMap::new();

    for magic in [b"PP20", b"PP11"] {
        for table in TABLES {
            for fill in [0x00u8, 0x0B, 0x0F, 0x55, 0xAA, 0xFF] {
                for len in 12..64usize {
                    for dest_len in [0u32, 1, 2, 5, 64, 1024] {
                        for skip in [0u8, 3, 7, 33] {
                            let mut bytes = vec![fill; len];
                            bytes[..4].copy_from_slice(magic);
                            bytes[4..8].copy_from_slice(&table);
                            let trailer = len - 4;
                            bytes[trailer] = ((dest_len >> 16) & 0xFF) as u8;
                            bytes[trailer + 1] = ((dest_len >> 8) & 0xFF) as u8;
                            bytes[trailer + 2] = (dest_len & 0xFF) as u8;
                            bytes[trailer + 3] = skip;

                            match decrunch(&bytes) {
                                Ok(out) => {
                                    assert_eq!(
                                        out.len(),
                                        dest_len as usize,
                                        "a successful decrunch must produce exactly the declared length"
                                    );
                                    if !out.is_empty() {
                                        decrunched += 1;
                                        longest_output = longest_output.max(out.len());
                                    }
                                }
                                Err(DecodeError::Truncated { .. }) => truncated += 1,
                                Err(DecodeError::Corrupt { what }) => {
                                    *corrupts.entry(what).or_default() += 1;
                                }
                                Err(DecodeError::BadMagic) => bad_magic += 1,
                            }
                        }
                    }
                }
            }
        }
    }

    for (what, count) in &corrupts {
        eprintln!("{count:6} corrupt: {what}");
    }
    eprintln!("{decrunched} non-empty successes, longest {longest_output} bytes");

    // A zero declared length succeeds without entering the loop at all, so
    // only non-empty output proves the sweep got that far — and a one-byte
    // output proves only the shortest path through it.
    assert!(
        decrunched > 0,
        "the sweep never reached the decompression loop: no input produced non-empty output"
    );
    assert!(
        longest_output > 1,
        "every success was a single byte: the sweep only takes the one-literal-then-stop path"
    );
    for what in LOOP_INTERNAL {
        assert!(
            corrupts.contains_key(what),
            "no input reached the decompression loop's {what:?} check; the sweep is bouncing off an earlier one"
        );
    }
    assert!(truncated > 0, "no input exhausted the bitstream");
    assert!(bad_magic > 0, "no input was rejected on its magic");
}

/// Decrunches a genuine PP20-crunched ProTracker module extracted from the
/// Gathering '92 music disk. Gated behind an environment variable and
/// `#[ignore]` because the disk image is real media and this repository
/// never commits media (reference by path only).
///
/// Run it with:
///
/// ```text
/// FORMAT198X_PP20_ADF_FIXTURE="/path/to/10 Best Tunes ... (Disk 1 of 3).adf" \
///     cargo test -p format198x-commodore-amiga-powerpacker -- --ignored
/// ```
#[test]
#[ignore = "needs a real Gathering '92 ADF on disk; set FORMAT198X_PP20_ADF_FIXTURE"]
fn decrunches_a_genuine_pp20_module() {
    let Ok(path) = std::env::var("FORMAT198X_PP20_ADF_FIXTURE") else {
        eprintln!("skipping: FORMAT198X_PP20_ADF_FIXTURE not set");
        return;
    };
    let img = std::fs::read(&path).expect("read the ADF fixture");
    let disk = format198x_commodore_amiga_adf::Disk::open(&img).expect("open the ADF image");

    for name in [
        "Ash-Vixen_Soulside Journey",
        "Gin-Carnage_Party Time",
        "Noteman-Virtual_The art of tun",
    ] {
        let packed = disk.read(name).unwrap_or_else(|_| panic!("read {name}"));
        assert!(
            is_powerpacked(&packed),
            "{name} does not start with the PP20 magic"
        );

        let decrunched = decrunch(&packed).unwrap_or_else(|e| panic!("decrunch {name}: {e}"));

        let magic = decrunched
            .get(1080..1084)
            .unwrap_or_else(|| panic!("{name}: decrunched output shorter than 1084 bytes"));
        let known_magics: [[u8; 4]; 4] = [*b"M.K.", *b"M!K!", *b"FLT4", *b"4CHN"];
        assert!(
            known_magics.iter().any(|m| m == magic),
            "{name}: unexpected magic at offset 1080: {magic:?}"
        );
    }
}
