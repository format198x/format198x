//! Parse and round-trip tests against a module built in code rather than
//! shipped as a file — no media may enter the repository.

use format_commodore_amiga_mod::{DecodeError, decode, encode};

/// One looped square-wave sample, one pattern, a C-2 on channel 0 at row 0.
fn synthetic_module() -> Vec<u8> {
    let sample: Vec<u8> = (0..64)
        .map(|i| if i < 32 { 100u8 } else { 156u8 })
        .collect();
    let mut out = b"SYNTH".to_vec();
    out.resize(20, 0);
    for i in 0..31 {
        let mut hdr = vec![0u8; 30];
        if i == 0 {
            hdr[..6].copy_from_slice(b"square");
            hdr[22..24].copy_from_slice(&((sample.len() / 2) as u16).to_be_bytes());
            hdr[25] = 64; // volume
            hdr[28..30].copy_from_slice(&((sample.len() / 2) as u16).to_be_bytes());
        }
        out.extend_from_slice(&hdr);
    }
    out.push(1); // song length
    out.push(0); // restart
    out.extend_from_slice(&[0u8; 128]); // order table
    out.extend_from_slice(b"M.K.");
    let mut pattern = vec![0u8; 64 * 4 * 4];
    let (period, smp) = (428u16, 1u8); // C-2, sample 1
    pattern[0] = (smp & 0xF0) | (period >> 8) as u8;
    pattern[1] = (period & 0xFF) as u8;
    pattern[2] = (smp & 0x0F) << 4;
    out.extend_from_slice(&pattern);
    out.extend_from_slice(&sample);
    out
}

#[test]
fn parses_a_synthetic_module() {
    let m = decode(&synthetic_module()).expect("decodes");
    assert_eq!(m.title(), "SYNTH");
    assert_eq!(m.orders().len(), 1);
    assert_eq!(m.patterns.len(), 1);
    assert_eq!(m.patterns[0].len(), 64);
    assert_eq!(m.patterns[0][0][0].period, 428);
    assert_eq!(m.patterns[0][0][0].sample, 1);
    let used: Vec<_> = m.samples.iter().filter(|s| !s.data.is_empty()).collect();
    assert_eq!(used.len(), 1);
    assert_eq!(used[0].name(), "square");
    assert_eq!(used[0].volume, 64);
    assert_eq!(used[0].data.len(), 64);
}

#[test]
fn round_trips_byte_for_byte() {
    let original = synthetic_module();
    let decoded = decode(&original).expect("decodes");
    let reencoded = encode(&decoded).expect("re-encodes");
    assert_eq!(reencoded, original, "round-trip changed bytes");
}

#[test]
fn malformed_input_never_panics() {
    for len in [0usize, 1, 20, 1079, 1083, 1084] {
        assert!(
            decode(&vec![0u8; len]).is_err(),
            "length {len} must be rejected"
        );
    }
}

// The tests above only ever feed all-zero buffers, which fail the BadMagic
// check at offset 1080 and return before touching the song length, the
// order table, the sample-length sums, or the pattern-count arithmetic —
// the code that has actually changed across this crate's revisions, and
// the highest-risk code in it. Everything below starts from a genuinely
// valid module (so it gets past BadMagic) and corrupts exactly one field,
// asserting the specific `DecodeError` variant that field's check must
// produce — not just `is_err()`, which would pass even if the wrong check
// fired.

/// Offset of sample `i`'s 30-byte header within [`synthetic_module`]'s
/// bytes (title is 20 bytes, so headers start at 20).
fn sample_header_offset(i: usize) -> usize {
    20 + i * 30
}

#[test]
fn song_length_over_128_is_corrupt() {
    let mut bytes = synthetic_module();
    bytes[950] = 200; // song length byte; legal range is 0..=128
    assert!(
        matches!(decode(&bytes), Err(DecodeError::Corrupt { .. })),
        "a song length past the 128-entry order table must be Corrupt"
    );
}

#[test]
fn sample_length_overrunning_the_file_is_truncated() {
    let mut bytes = synthetic_module();
    // Inflate sample 0's declared length far past what the file actually
    // holds (the file still only has the original 64 bytes of PCM data).
    let start = sample_header_offset(0);
    bytes[start + 22..start + 24].copy_from_slice(&0xFFFFu16.to_be_bytes());
    assert!(
        matches!(decode(&bytes), Err(DecodeError::Truncated { .. })),
        "a declared sample length past the end of the file must be Truncated, not underflow"
    );
}

#[test]
fn non_1024_aligned_remainder_is_corrupt() {
    let mut bytes = synthetic_module();
    bytes.push(0); // one stray byte breaks pattern-data's 1024-byte alignment
    assert!(
        matches!(decode(&bytes), Err(DecodeError::Corrupt { .. })),
        "a remainder that isn't a whole number of 1024-byte patterns must be Corrupt, not rounded"
    );
}

#[test]
fn file_truncated_mid_pattern_is_truncated() {
    let bytes = synthetic_module();
    // Cut inside the single 1024-byte pattern (which starts at 1084), deep
    // enough that even sample 0's declared 64 bytes of PCM no longer fit.
    // Magic and every header are untouched.
    let truncated = &bytes[..1100];
    assert!(
        matches!(decode(truncated), Err(DecodeError::Truncated { .. })),
        "a file that ends mid-pattern must be Truncated"
    );
}

#[test]
fn absurd_sample_lengths_fail_fast_without_overflow_or_huge_allocation() {
    let mut bytes = synthetic_module();
    // Every one of the 31 sample slots claims the maximum possible length
    // (0xFFFF words = 131070 bytes each, ~4MB total) while the file itself
    // stays tiny. This must return a typed error immediately rather than
    // overflow the running sum, wrap around in the bounds arithmetic, or
    // attempt to allocate anywhere near 4MB for data the file doesn't have.
    for i in 0..31 {
        let start = sample_header_offset(i);
        bytes[start + 22..start + 24].copy_from_slice(&0xFFFFu16.to_be_bytes());
    }
    assert!(
        matches!(decode(&bytes), Err(DecodeError::Truncated { .. })),
        "absurd declared sample lengths must be rejected, not overflow or allocate"
    );
}

#[test]
fn six_channel_magic_is_unsupported() {
    let mut bytes = synthetic_module();
    bytes[1080..1084].copy_from_slice(b"6CHN");
    assert_eq!(
        decode(&bytes),
        Err(DecodeError::UnsupportedChannelCount { magic: *b"6CHN" })
    );
}

#[test]
fn eight_channel_magic_is_unsupported() {
    let mut bytes = synthetic_module();
    bytes[1080..1084].copy_from_slice(b"8CHN");
    assert_eq!(
        decode(&bytes),
        Err(DecodeError::UnsupportedChannelCount { magic: *b"8CHN" })
    );
}

#[test]
fn a_module_with_no_tail_has_no_trailing_bytes() {
    let m = decode(&synthetic_module()).expect("decodes");
    assert!(
        m.trailing.is_empty(),
        "a file that ends where its sample data ends has nothing trailing"
    );
}

#[test]
fn a_garbage_tail_is_kept_out_of_the_pattern_count() {
    // A module ripped out of an executable or padded to a block boundary
    // carries surplus bytes after its last sample. The file-size rule alone
    // reads a 1024-byte tail as a second pattern and then takes sample 0's
    // PCM from the junk — a misparse that is self-consistent, so a
    // round-trip assertion alone can never catch it.
    let mut bytes = synthetic_module();
    let original_sample: Vec<i8> = decode(&bytes).expect("decodes").samples[0].data.clone();
    bytes.extend(std::iter::repeat_n(0xAAu8, 1024));

    let m = decode(&bytes).expect("decodes");
    assert_eq!(m.patterns.len(), 1, "the tail is not a second pattern");
    assert_eq!(m.trailing, vec![0xAAu8; 1024], "the tail is kept verbatim");
    assert_eq!(
        m.samples[0].data, original_sample,
        "sample PCM must still come from the sample region, not the tail"
    );
    assert_eq!(
        encode(&m).expect("re-encodes"),
        bytes,
        "a module with a tail must still round-trip byte-for-byte"
    );
}
