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
    MAGIC_OFFSET, MAGICS, Module, Note, ORDER_TABLE_LEN, SAMPLE_NAME_LEN, Sample, TITLE_LEN,
};

const NUM_SAMPLES: usize = 31;
const SAMPLE_HEADER_LEN: usize = 30;
const HEADERS_OFFSET: usize = TITLE_LEN;
const SONG_LENGTH_OFFSET: usize = HEADERS_OFFSET + NUM_SAMPLES * SAMPLE_HEADER_LEN; // 950
const RESTART_OFFSET: usize = SONG_LENGTH_OFFSET + 1; // 951
const ORDERS_OFFSET: usize = RESTART_OFFSET + 1; // 952
const ROWS_PER_PATTERN: usize = 64;
const CHANNELS: usize = 4;
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
/// 8-channel module (`6CHN`/`8CHN`) — see the crate documentation's Scope
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
    if &magic == b"6CHN" || &magic == b"8CHN" {
        return Err(DecodeError::UnsupportedChannelCount { magic });
    }

    let mut title_bytes = [0u8; TITLE_LEN];
    title_bytes.copy_from_slice(&bytes[0..TITLE_LEN]);

    let mut headers = Vec::with_capacity(NUM_SAMPLES);
    for i in 0..NUM_SAMPLES {
        let start = HEADERS_OFFSET + i * SAMPLE_HEADER_LEN;
        let hdr = &bytes[start..start + SAMPLE_HEADER_LEN];

        let mut name_bytes = [0u8; SAMPLE_NAME_LEN];
        name_bytes.copy_from_slice(&hdr[0..SAMPLE_NAME_LEN]);
        let length_words = u16::from_be_bytes([hdr[22], hdr[23]]);
        let finetune_byte = hdr[24];
        let volume = hdr[25];
        let repeat_start_words = u16::from_be_bytes([hdr[26], hdr[27]]);
        let repeat_length_words = u16::from_be_bytes([hdr[28], hdr[29]]);

        headers.push(SampleHeader {
            name_bytes,
            data_len: usize::from(length_words) * 2,
            volume,
            finetune_byte,
            repeat_start_words,
            repeat_length_words,
        });
    }

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
        return Err(DecodeError::Corrupt {
            what: "pattern data is not a whole number of 1024-byte patterns",
        });
    }
    let pattern_count = available_for_patterns / PATTERN_LEN;
    let patterns_end = patterns_offset + available_for_patterns;

    let mut patterns = Vec::with_capacity(pattern_count);
    for p in 0..pattern_count {
        let pattern_start = patterns_offset + p * PATTERN_LEN;
        let mut rows = Vec::with_capacity(ROWS_PER_PATTERN);
        for r in 0..ROWS_PER_PATTERN {
            let row_start = pattern_start + r * CHANNELS * 4;
            let mut notes = [Note::default(); CHANNELS];
            for (c, note) in notes.iter_mut().enumerate() {
                let cell = row_start + c * 4;
                let b0 = bytes[cell];
                let b1 = bytes[cell + 1];
                let b2 = bytes[cell + 2];
                let b3 = bytes[cell + 3];
                *note = Note {
                    sample: (b0 & 0xF0) | (b2 >> 4),
                    period: (u16::from(b0 & 0x0F) << 8) | u16::from(b1),
                    effect: b2 & 0x0F,
                    param: b3,
                };
            }
            rows.push(notes);
        }
        patterns.push(rows);
    }

    let mut cursor = patterns_end;
    let mut samples = Vec::with_capacity(NUM_SAMPLES);
    for header in headers {
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
        let data: Vec<i8> = bytes[cursor..end].iter().map(|&b| b as i8).collect();
        cursor = end;
        samples.push(Sample {
            name_bytes: header.name_bytes,
            data,
            volume: header.volume,
            finetune_byte: header.finetune_byte,
            repeat_start_words: header.repeat_start_words,
            repeat_length_words: header.repeat_length_words,
        });
    }

    Ok(Module {
        title_bytes,
        samples,
        song_length,
        order_table,
        restart,
        magic,
        patterns,
    })
}
