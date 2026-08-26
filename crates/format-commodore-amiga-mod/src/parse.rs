//! Decoding: raw bytes into a [`Module`].
//!
//! Every offset this module reads is checked against the actual input
//! length before the read happens — a corrupt song length, an order byte
//! that pushes the pattern count past what the file holds, or a sample
//! length that overruns the file all become a typed [`DecodeError`], never
//! an out-of-bounds index or an arithmetic overflow panic.

use crate::error::DecodeError;
use crate::{MAGIC_OFFSET, MAGICS, Module, Note, Sample};

const TITLE_LEN: usize = 20;
const NUM_SAMPLES: usize = 31;
const SAMPLE_HEADER_LEN: usize = 30;
const SAMPLE_NAME_LEN: usize = 22;
const HEADERS_OFFSET: usize = TITLE_LEN;
const SONG_LENGTH_OFFSET: usize = HEADERS_OFFSET + NUM_SAMPLES * SAMPLE_HEADER_LEN; // 950
const RESTART_OFFSET: usize = SONG_LENGTH_OFFSET + 1; // 951
const ORDERS_OFFSET: usize = RESTART_OFFSET + 1; // 952
const ORDERS_LEN: usize = 128;
const ROWS_PER_PATTERN: usize = 64;
const CHANNELS: usize = 4;
const PATTERN_LEN: usize = ROWS_PER_PATTERN * CHANNELS * 4; // 1024

/// One sample header's fields, ahead of knowing where its data lives in the
/// file (that depends on every earlier sample's length, so it is resolved
/// in a second pass).
struct SampleHeader {
    name: String,
    data_len: usize,
    volume: u8,
    finetune: i8,
    loop_start: usize,
    loop_len: usize,
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

    let title = read_padded_string(&bytes[0..TITLE_LEN]);

    let mut headers = Vec::with_capacity(NUM_SAMPLES);
    for i in 0..NUM_SAMPLES {
        let start = HEADERS_OFFSET + i * SAMPLE_HEADER_LEN;
        let hdr = &bytes[start..start + SAMPLE_HEADER_LEN];

        let name = read_padded_string(&hdr[0..SAMPLE_NAME_LEN]);
        let length_words = u16::from_be_bytes([hdr[22], hdr[23]]);
        let finetune_raw = hdr[24] & 0x0F;
        // finetune_raw is 0..=15, so the cast to i8 is exact; the format's
        // finetune is a signed nibble (-8..=7), so 8..=15 folds back by 16.
        let finetune = if finetune_raw >= 8 {
            (finetune_raw as i8) - 16
        } else {
            finetune_raw as i8
        };
        let volume = hdr[25];
        let repeat_start_words = u16::from_be_bytes([hdr[26], hdr[27]]);
        let repeat_length_words = u16::from_be_bytes([hdr[28], hdr[29]]);

        headers.push(SampleHeader {
            name,
            data_len: usize::from(length_words) * 2,
            volume,
            finetune,
            loop_start: usize::from(repeat_start_words) * 2,
            loop_len: if repeat_length_words <= 1 {
                0
            } else {
                usize::from(repeat_length_words) * 2
            },
        });
    }

    let song_length = bytes[SONG_LENGTH_OFFSET];
    let _restart = bytes[RESTART_OFFSET]; // historically set to 127, ignored by ProTracker; see crate docs.
    if usize::from(song_length) > ORDERS_LEN {
        return Err(DecodeError::Corrupt {
            what: "song length exceeds the 128-entry order table",
        });
    }
    let orders = bytes[ORDERS_OFFSET..ORDERS_OFFSET + usize::from(song_length)].to_vec();

    let pattern_count = orders
        .iter()
        .copied()
        .max()
        .map_or(0usize, |m| usize::from(m) + 1);
    let patterns_offset = MAGIC_OFFSET + 4;
    let patterns_len = pattern_count
        .checked_mul(PATTERN_LEN)
        .ok_or(DecodeError::Corrupt {
            what: "pattern count overflowed while computing pattern data size",
        })?;
    let patterns_end = patterns_offset
        .checked_add(patterns_len)
        .ok_or(DecodeError::Corrupt {
            what: "pattern data extent overflowed",
        })?;
    if bytes.len() < patterns_end {
        return Err(DecodeError::Truncated {
            what: "pattern data",
        });
    }

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
            name: header.name,
            data,
            volume: header.volume,
            finetune: header.finetune,
            loop_start: header.loop_start,
            loop_len: header.loop_len,
        });
    }

    Ok(Module {
        title,
        samples,
        orders,
        patterns,
    })
}

/// Read a fixed-width, NUL-padded field as a string, trimmed at the first
/// NUL byte (or the field's full width, if there is none).
fn read_padded_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
