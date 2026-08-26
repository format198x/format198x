//! Decoding: raw bytes into a [`Module`].
//!
//! Every offset this module reads is checked against the actual input
//! length before the read happens — a corrupt song length, an order byte
//! that pushes the pattern count past what the file holds, or a sample
//! length that overruns the file all become a typed [`DecodeError`], never
//! an out-of-bounds index or an arithmetic overflow panic.
//!
//! Every field is stored exactly as read — no trimming, no collapsing "no
//! loop" onto a single value, no normalising the magic — so [`crate::encode`]
//! can reproduce the original bytes exactly. See the crate documentation's
//! "Losslessness" section.

use crate::error::DecodeError;
use crate::{
    CHANNELS, MAGIC_OFFSET, MAGICS, Module, NUM_SAMPLES, Note, ORDER_TABLE_LEN, ROWS_PER_PATTERN,
    SAMPLE_NAME_LEN, Sample, TITLE_LEN,
};

const SAMPLE_HEADER_LEN: usize = 30;
const HEADERS_OFFSET: usize = TITLE_LEN;
const SONG_LENGTH_OFFSET: usize = HEADERS_OFFSET + NUM_SAMPLES * SAMPLE_HEADER_LEN; // 950
const RESTART_OFFSET: usize = SONG_LENGTH_OFFSET + 1; // 951
const ORDERS_OFFSET: usize = RESTART_OFFSET + 1; // 952
const PATTERN_LEN: usize = ROWS_PER_PATTERN * CHANNELS * 4; // 1024

/// One sample header's raw fields, ahead of knowing where its data lives in
/// the file (that depends on every earlier sample's length, so it is
/// resolved in a second pass).
struct SampleHeader {
    name_bytes: [u8; SAMPLE_NAME_LEN],
    data_len: usize,
    volume: u8,
    finetune_byte: u8,
    repeat_start_words: u16,
    repeat_length_words: u16,
}

/// Decode a ProTracker MOD module from raw bytes.
///
/// # Errors
///
/// [`DecodeError::Truncated`] when `bytes` is shorter than the fixed 1084-byte
/// header, or shorter than the pattern or sample data the header declares.
/// [`DecodeError::BadMagic`] when no recognised magic sits at
/// [`MAGIC_OFFSET`]. [`DecodeError::UnsupportedChannelCount`] for a 6- or
/// 8-channel module (`6CHN`/`8CHN`/`FLT8`) — see the crate documentation's Scope
/// section. [`DecodeError::Corrupt`] when a header field is out of range for
/// the format (a song length longer than the order table can hold) or an
/// offset computation would overflow — never a panic, even on hostile input.
pub fn decode(bytes: &[u8]) -> Result<Module, DecodeError> {
    if bytes.len() < MAGIC_OFFSET + 4 {
        return Err(DecodeError::Truncated {
            what: "MOD header (title, sample headers, order table, magic)",
        });
    }

    let mut magic = [0u8; 4];
    magic.copy_from_slice(&bytes[MAGIC_OFFSET..MAGIC_OFFSET + 4]);
    if !MAGICS.iter().any(|m| **m == magic) {
        return Err(DecodeError::BadMagic);
    }
    if &magic == b"6CHN" || &magic == b"8CHN" || &magic == b"FLT8" {
        return Err(DecodeError::UnsupportedChannelCount { magic });
    }

    let mut title_bytes = [0u8; TITLE_LEN];
    title_bytes.copy_from_slice(&bytes[0..TITLE_LEN]);

    // Every one of these 31 headers sits inside the fixed 1084-byte header
    // whose presence was checked above, so reading them cannot fail.
    let headers: [SampleHeader; NUM_SAMPLES] = core::array::from_fn(|i| {
        let start = HEADERS_OFFSET + i * SAMPLE_HEADER_LEN;
        let hdr = &bytes[start..start + SAMPLE_HEADER_LEN];

        let mut name_bytes = [0u8; SAMPLE_NAME_LEN];
        name_bytes.copy_from_slice(&hdr[0..SAMPLE_NAME_LEN]);

        SampleHeader {
            name_bytes,
            data_len: usize::from(u16::from_be_bytes([hdr[22], hdr[23]])) * 2,
            volume: hdr[25],
            finetune_byte: hdr[24],
            repeat_start_words: u16::from_be_bytes([hdr[26], hdr[27]]),
            repeat_length_words: u16::from_be_bytes([hdr[28], hdr[29]]),
        }
    });

    let song_length = bytes[SONG_LENGTH_OFFSET];
    let restart = bytes[RESTART_OFFSET];
    if usize::from(song_length) > ORDER_TABLE_LEN {
        return Err(DecodeError::Corrupt {
            what: "song length exceeds the 128-entry order table",
        });
    }
    let mut order_table = [0u8; ORDER_TABLE_LEN];
    order_table.copy_from_slice(&bytes[ORDERS_OFFSET..ORDERS_OFFSET + ORDER_TABLE_LEN]);

    // The pattern count is NOT reliably derivable from the order table —
    // neither the played prefix (`order_table[..song_length]`) nor even the
    // whole 128-entry table. Some real files store pattern data referenced
    // only by order-table slots *beyond* the song length ("hidden" patterns
    // that never play but are still physically present), which the
    // played-prefix rule silently drops. But the unplayed tail of the order
    // table is also where real files routinely leave non-zero leftover
    // garbage that does NOT correspond to any stored pattern — taking
    // `max()` over the whole table over-counts on those files and reads
    // past the end of the file (confirmed: one real file's garbage byte
    // implied 233 patterns when only 9 were physically present). Nothing in
    // the order table itself distinguishes a genuine hidden-pattern index
    // from garbage.
    //
    // The reliable derivation uses the file's own arithmetic instead: the
    // format's three regions (1084-byte header, patterns, then all 31
    // samples' PCM data concatenated) are contiguous and exhaustive, so the
    // pattern data's size is exactly `bytes.len() - 1084 - total sample
    // bytes` — independent of what the order table claims. Verified exact
    // (evenly divisible by 1024) on every file across two independent
    // real-media corpora, including both the hidden-pattern files and the
    // garbage-tail files.
    let patterns_offset = MAGIC_OFFSET + 4;
    let total_sample_bytes: usize = headers.iter().map(|h| h.data_len).sum();
    let available_for_patterns = bytes
        .len()
        .checked_sub(patterns_offset)
        .and_then(|v| v.checked_sub(total_sample_bytes))
        .ok_or(DecodeError::Truncated {
            what: "pattern and sample data",
        })?;
    if !available_for_patterns.is_multiple_of(PATTERN_LEN) {
        // Name the ambiguity rather than the pattern region: the leftover
        // could as easily be a truncated final sample as a bad pattern
        // count, and a player logging this should not be pointed at the
        // wrong part of the file.
        return Err(DecodeError::Corrupt {
            what: "the file size does not divide into whole patterns plus the declared sample data — the file is truncated or has trailing bytes",
        });
    }

    // The size rule alone assumes the file is exactly header + patterns +
    // samples, with nothing after the last sample. Real modules break that
    // assumption often: ones ripped out of an executable, padded to a block
    // boundary, or stored inside a larger container all carry surplus bytes
    // at the end. The size rule reads that surplus as extra patterns, which
    // shifts every sample's PCM forward into the junk — and because the
    // misparse is self-consistent, `encode(decode(x)) == x` still holds and
    // no round-trip test can catch it.
    //
    // The order table gives a free upper bound to cross-check against: no
    // file stores a pattern that nothing in the 128-entry table can even
    // name, so the largest index in the table plus one caps the count. When
    // the size rule wants more patterns than that, the surplus is not
    // pattern data; clamp the count and keep the surplus verbatim in
    // `Module::trailing` so a re-encode is still byte-identical.
    //
    // The reverse case — the size rule wanting fewer patterns than the
    // table's largest index implies — is the hidden-pattern/garbage-tail
    // situation above, where the table over-counts and the size rule is
    // right. Taking the smaller of the two handles both.
    let table_max = usize::from(order_table.iter().copied().max().unwrap_or(0)) + 1;
    let pattern_count = (available_for_patterns / PATTERN_LEN).min(table_max);
    let patterns_end = patterns_offset + pattern_count * PATTERN_LEN;

    // `patterns_end` is bounded by the file length above, so every cell
    // index below is inside the input.
    let mut patterns = Vec::with_capacity(pattern_count);
    for p in 0..pattern_count {
        let pattern_start = patterns_offset + p * PATTERN_LEN;
        let pattern: [[Note; CHANNELS]; ROWS_PER_PATTERN] = core::array::from_fn(|r| {
            let row_start = pattern_start + r * CHANNELS * 4;
            core::array::from_fn(|c| {
                let cell = row_start + c * 4;
                let b0 = bytes[cell];
                let b1 = bytes[cell + 1];
                let b2 = bytes[cell + 2];
                let b3 = bytes[cell + 3];
                Note {
                    sample: (b0 & 0xF0) | (b2 >> 4),
                    period: (u16::from(b0 & 0x0F) << 8) | u16::from(b1),
                    effect: b2 & 0x0F,
                    param: b3,
                }
            })
        });
        patterns.push(pattern);
    }

    // Resolve where each sample's PCM lives before building any of them,
    // so a length that overruns the file is a typed error rather than a
    // half-built array.
    let mut ranges = [(0usize, 0usize); NUM_SAMPLES];
    let mut cursor = patterns_end;
    for (range, header) in ranges.iter_mut().zip(headers.iter()) {
        let end = cursor
            .checked_add(header.data_len)
            .ok_or(DecodeError::Corrupt {
                what: "sample data extent overflowed",
            })?;
        if bytes.len() < end {
            return Err(DecodeError::Truncated {
                what: "sample data",
            });
        }
        *range = (cursor, end);
        cursor = end;
    }
    let samples: [Sample; NUM_SAMPLES] = core::array::from_fn(|i| {
        let (start, end) = ranges[i];
        Sample {
            name_bytes: headers[i].name_bytes,
            data: bytes[start..end].to_vec(),
            volume: headers[i].volume,
            finetune_byte: headers[i].finetune_byte,
            repeat_start_words: headers[i].repeat_start_words,
            repeat_length_words: headers[i].repeat_length_words,
        }
    });

    let trailing = bytes[cursor..].to_vec();

    Ok(Module {
        title_bytes,
        samples,
        song_length,
        order_table,
        restart,
        magic,
        patterns,
        trailing,
    })
}
