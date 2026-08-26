//! Parse and round-trip tests against a module built in code rather than
//! shipped as a file — no media may enter the repository.

use format_commodore_amiga_mod::{DecodeError, EncodeError, decode, encode, is_module};

/// One looped square-wave sample, one pattern, a C-2 on channel 0 at row 0.
fn synthetic_module() -> Vec<u8> {
    module_with_sample(&known_good_sample_0())
}

/// [`synthetic_module`] with sample 0's PCM replaced, so a test can choose
/// bytes that land in the region the pattern count decides the extent of.
fn module_with_sample(sample: &[u8]) -> Vec<u8> {
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
    out.extend_from_slice(sample);
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
fn data_i8_yields_the_signed_interpretation_of_the_stored_bytes() {
    // The synthetic sample's second half is stored as the raw byte 156
    // (above 0x7F, so its signed interpretation actually differs): as
    // signed 8-bit PCM that is -100, not 156.
    let m = decode(&synthetic_module()).expect("decodes");
    let sample = &m.samples[0];
    assert_eq!(sample.data[32], 156, "stored byte is unsigned 156");
    let signed: Vec<i8> = sample.data_i8().collect();
    assert_eq!(signed[32], -100, "data_i8 must reinterpret 156 as -100");
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

/// Startrekker's 8-channel magic, the counterpart to its `FLT4`. It must
/// reach the channel-count rejection like `8CHN` does, not fall through to
/// `BadMagic` — a content-sniffing player has to be able to identify the
/// file before it can say it cannot play it.
#[test]
fn startrekker_flt8_is_unsupported_rather_than_unrecognised() {
    let mut bytes = synthetic_module();
    bytes[1080..1084].copy_from_slice(b"FLT8");
    assert!(is_module(&bytes), "FLT8 must be identified as a module");
    assert_eq!(
        decode(&bytes),
        Err(DecodeError::UnsupportedChannelCount { magic: *b"FLT8" })
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

/// A 1024-byte block whose 256 cells all read as plausible pattern data:
/// row 0, channel 0 plays sample 2 at period 340, and every other cell is
/// an empty note. Deliberately different from [`synthetic_module`]'s own
/// pattern, so a test can prove *which* block a decoded pattern came from.
fn pattern_like_block() -> Vec<u8> {
    let mut block = vec![0u8; 1024];
    let (period, smp) = (340u16, 2u8);
    block[0] = (smp & 0xF0) | (period >> 8) as u8;
    block[1] = (period & 0xFF) as u8;
    block[2] = (smp & 0x0F) << 4;
    block
}

/// [`synthetic_module`] with `block` spliced in between its one real
/// pattern and its sample data — so the file physically holds two patterns
/// and no trailing bytes at all. The order table is all zeros, so nothing
/// in it names the second pattern: this is exactly the file whose pattern
/// count size alone cannot settle.
fn module_with_second_pattern(block: &[u8]) -> Vec<u8> {
    let mut bytes = synthetic_module();
    let after_first_pattern = 1084 + 1024;
    bytes.splice(
        after_first_pattern..after_first_pattern,
        block.iter().copied(),
    );
    bytes
}

/// Sample 0's PCM as [`synthetic_module`] stores it: 32 bytes of 100 then
/// 32 of 156. Every test below compares against this rather than against a
/// pattern count, because a misclassified block shifts where the PCM is
/// read from while leaving the count self-consistent and the round-trip
/// green.
fn known_good_sample_0() -> Vec<u8> {
    (0..64)
        .map(|i| if i < 32 { 100u8 } else { 156u8 })
        .collect()
}

#[test]
fn a_garbage_tail_is_kept_out_of_the_pattern_count() {
    // A module ripped out of an executable or padded out to a whole number
    // of 1024-byte units carries surplus bytes after its last sample. The
    // file-size rule alone reads a 1024-byte tail as a second pattern and
    // then takes sample 0's PCM from the junk — a misparse that is
    // self-consistent, so a round-trip assertion alone can never catch it.
    //
    // Note what this fixture does and does not cover: its order table is
    // all zeros and it holds one real pattern, so the table's largest index
    // plus one happens to equal the true pattern count. It says nothing
    // about a file that stores a pattern no order-table entry names — see
    // `an_unreferenced_pattern_is_kept_rather_than_clamped_away` for that
    // case, which the opposite mistake corrupts just as quietly.
    let mut bytes = synthetic_module();
    bytes.extend(std::iter::repeat_n(0xAAu8, 1024));

    let m = decode(&bytes).expect("decodes");
    assert_eq!(m.patterns.len(), 1, "the tail is not a second pattern");
    assert_eq!(m.trailing, vec![0xAAu8; 1024], "the tail is kept verbatim");
    assert_eq!(
        m.samples[0].data,
        known_good_sample_0(),
        "sample PCM must still come from the sample region, not the tail"
    );
    assert_eq!(
        encode(&m).expect("re-encodes"),
        bytes,
        "a module with a tail must still round-trip byte-for-byte"
    );
}

#[test]
fn an_unreferenced_pattern_is_kept_rather_than_clamped_away() {
    // The mirror image of the test above, and the bug that clamping to the
    // order table's largest index introduces. This file holds two patterns
    // and no tail, but its all-zero order table names only the first.
    // Clamping the count to what the table names moves sample 0's PCM read
    // back into the second pattern's bytes — silently wrong audio, and
    // `encode(decode(x)) == x` still holds, so only the PCM itself shows it.
    let bytes = module_with_second_pattern(&pattern_like_block());

    let m = decode(&bytes).expect("decodes");
    assert_eq!(
        m.samples[0].data,
        known_good_sample_0(),
        "sample PCM must come from the sample region, not from pattern 1"
    );
    assert_eq!(
        m.samples[0].data[0], 100,
        "the first PCM byte is the square wave's high level, not a pattern byte"
    );
    assert!(
        m.trailing.is_empty(),
        "the second pattern is pattern data, not a tail"
    );
    assert_eq!(m.patterns.len(), 2, "both stored patterns are decoded");
    assert_eq!(
        (m.patterns[1][0][0].period, m.patterns[1][0][0].sample),
        (340, 2),
        "pattern 1 must be the block that was spliced in, decoded as notes"
    );
    assert_eq!(
        encode(&m).expect("re-encodes"),
        bytes,
        "a module with an unreferenced pattern must round-trip byte-for-byte"
    );
}

#[test]
fn an_all_zero_unreferenced_pattern_is_kept() {
    // An all-zero block is byte-for-byte a legal empty pattern — 256 cells
    // of "no note" — so a silent unreferenced pattern is kept, and sample 0
    // still reads from the sample region.
    let bytes = module_with_second_pattern(&vec![0u8; 1024]);

    let m = decode(&bytes).expect("decodes");
    assert_eq!(
        m.samples[0].data,
        known_good_sample_0(),
        "sample PCM must come from the sample region, not from the silent pattern"
    );
    assert!(m.trailing.is_empty(), "a silent pattern is not a tail");
    assert_eq!(m.patterns.len(), 2, "the silent pattern is decoded too");
    assert!(
        m.patterns[1]
            .iter()
            .flatten()
            .all(|n| n.period == 0 && n.sample == 0 && n.effect == 0 && n.param == 0),
        "pattern 1 decodes as 64 rows of empty notes"
    );
    assert_eq!(
        encode(&m).expect("re-encodes"),
        bytes,
        "the module must still round-trip byte-for-byte"
    );
}

#[test]
fn an_all_zero_tail_after_real_sample_data_is_still_a_tail() {
    // Zero padding is the commonest tail there is, and it does not defeat
    // the rule the way an all-zero *disputed block* would: the block under
    // dispute starts where the samples do, not where the padding does, so
    // it is sample 0's own PCM that gets inspected — and 100 as a cell byte
    // means sample number 102, which no module has.
    let mut bytes = synthetic_module();
    bytes.extend(std::iter::repeat_n(0u8, 1024));

    let m = decode(&bytes).expect("decodes");
    assert_eq!(
        m.samples[0].data,
        known_good_sample_0(),
        "sample PCM must still come from the sample region"
    );
    assert_eq!(m.patterns.len(), 1, "the padding is not a second pattern");
    assert_eq!(m.trailing, vec![0u8; 1024], "the padding is kept verbatim");
    assert_eq!(encode(&m).expect("re-encodes"), bytes);
}

#[test]
fn an_all_zero_block_ahead_of_the_pcm_is_the_rules_one_blind_spot() {
    // The honest limit, pinned here so it cannot change unnoticed. An
    // all-zero 1024-byte block is byte-for-byte a legal empty pattern, so
    // when the disputed block really is the start of a sample whose PCM
    // opens with 1024 zero bytes, no parser can tell it from a pattern.
    // This file has one pattern, a 2048-byte sample that starts with
    // silence, and a 1024-byte tail; the rule reads the silence as a second
    // pattern and sample 0's PCM then runs 1024 bytes late, straight into
    // the tail.
    //
    // This is not a regression — it is what the file-size rule did before
    // any cross-check existed. Fixing it would need information the bytes
    // do not carry.
    let mut sample = vec![0u8; 1024];
    sample.extend(std::iter::repeat_n(100u8, 1024));
    let mut bytes = module_with_sample(&sample);
    bytes.extend(std::iter::repeat_n(0xAAu8, 1024));

    let m = decode(&bytes).expect("decodes");
    assert_eq!(
        m.patterns.len(),
        2,
        "the silent lead-in is read as a pattern"
    );
    assert!(
        m.trailing.is_empty(),
        "so nothing is left over for the tail"
    );
    assert_ne!(
        m.samples[0].data, sample,
        "sample 0's PCM is not the module's real sample data"
    );
    assert_eq!(
        &m.samples[0].data[..1024],
        &vec![100u8; 1024][..],
        "it starts 1024 bytes late, at the sample's second half"
    );
    assert_eq!(
        &m.samples[0].data[1024..],
        &vec![0xAAu8; 1024][..],
        "and runs on into the tail"
    );
    assert_eq!(
        encode(&m).expect("re-encodes"),
        bytes,
        "the misparse is still byte-identical on re-encode, which is exactly \
         why these tests assert the PCM rather than the round-trip"
    );
}

#[test]
fn latin1_text_survives_instead_of_vanishing() {
    // Amiga text is ISO-8859-1, and MOD titles and sample names carry
    // accented letters and box-drawing bytes routinely. Decoding them as
    // UTF-8 threw the whole string away on one high byte, which reaches a
    // metadata panel as a blank title.
    let mut bytes = synthetic_module();
    bytes[5] = 0xE9; // 'é' in ISO-8859-1, an invalid UTF-8 byte on its own
    let start = sample_header_offset(0);
    bytes[start + 6] = 0xFC; // 'ü'

    let m = decode(&bytes).expect("decodes");
    assert_eq!(m.title(), "SYNTHé");
    assert_eq!(m.samples[0].name(), "squareü");
    assert_eq!(
        encode(&m).expect("re-encodes"),
        bytes,
        "the raw bytes still round-trip"
    );
}

#[test]
fn encode_rejects_what_decode_rejects() {
    // A hand-built module could carry a magic or a song length `decode`
    // refuses. `encode` used to write those out happily, producing bytes
    // that failed this crate's own `decode`.
    let good = decode(&synthetic_module()).expect("decodes");

    let mut wide = good.clone();
    wide.magic = *b"8CHN";
    assert_eq!(
        encode(&wide),
        Err(EncodeError::UnsupportedMagic { magic: *b"8CHN" })
    );

    let mut unknown = good.clone();
    unknown.magic = *b"XXXX";
    assert_eq!(
        encode(&unknown),
        Err(EncodeError::UnsupportedMagic { magic: *b"XXXX" })
    );

    let mut long_song = good.clone();
    long_song.song_length = 200;
    assert_eq!(
        encode(&long_song),
        Err(EncodeError::SongLengthOutOfRange { found: 200 })
    );

    // The boundary stays legal: 128 is the whole order table, which decode
    // accepts.
    let mut full_song = good;
    full_song.song_length = 128;
    let bytes = encode(&full_song).expect("128 positions is the whole table");
    assert!(decode(&bytes).is_ok(), "encode's output must decode again");
}

#[test]
fn channel_count_comes_from_the_magic() {
    let m = decode(&synthetic_module()).expect("decodes");
    assert_eq!(m.channels(), 4);

    let mut wide = m;
    wide.magic = *b"FLT8";
    assert_eq!(wide.channels(), 8, "Startrekker's FLT8 is 8-channel");
    wide.magic = *b"6CHN";
    assert_eq!(wide.channels(), 6);
}
