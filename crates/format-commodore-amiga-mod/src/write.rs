//! Encoding: a [`Module`] into raw bytes — the exact inverse of
//! [`decode`](crate::decode).
//!
//! Every field [`Module`]/[`Sample`] carries is written back exactly as
//! stored — the restart byte, the magic variant, the full order table, the
//! raw name/title bytes, the raw finetune byte, the raw loop words — so
//! `encode(decode(bytes)) == bytes` for every 4-channel module (verified
//! against 17 real Amiga music-disk modules; see the task report). The only
//! values this module computes rather than copies are the ones the header
//! doesn't store directly: each sample's length in words (from
//! `data.len()`) and the pattern bytes (from `patterns`).

use crate::Module;
use crate::error::EncodeError;

const NUM_SAMPLES: usize = 31;
const ROWS_PER_PATTERN: usize = 64;
const CHANNELS: usize = 4;

/// Encode a [`Module`] as ProTracker MOD bytes.
///
/// # Errors
///
/// [`EncodeError::WrongSampleCount`] unless `module.samples.len() == 31`.
/// [`EncodeError::WrongPatternRows`] if a pattern is not exactly 64 rows.
/// [`EncodeError::SampleDataInvalid`] if a sample's data length is odd or
/// too large for the header's 16-bit word field. [`EncodeError::NoteOutOfRange`]
/// if a note's period exceeds 12 bits or its effect exceeds 4 bits.
/// [`EncodeError::PatternDataTooLarge`] if the pattern count overflows while
/// computing the pattern data size.
pub fn encode(module: &Module) -> Result<Vec<u8>, EncodeError> {
    if module.samples.len() != NUM_SAMPLES {
        return Err(EncodeError::WrongSampleCount {
            found: module.samples.len(),
        });
    }
    for (p, pattern) in module.patterns.iter().enumerate() {
        if pattern.len() != ROWS_PER_PATTERN {
            return Err(EncodeError::WrongPatternRows {
                pattern: p,
                found: pattern.len(),
            });
        }
    }
    let pattern_data_len = module
        .patterns
        .len()
        .checked_mul(ROWS_PER_PATTERN * CHANNELS * 4)
        .ok_or(EncodeError::PatternDataTooLarge)?;

    let mut out = Vec::with_capacity(pattern_data_len);

    out.extend_from_slice(&module.title_bytes);

    for (index, sample) in module.samples.iter().enumerate() {
        out.extend_from_slice(&sample.name_bytes);

        let length_words =
            to_word_count(sample.data.len()).ok_or(EncodeError::SampleDataInvalid { index })?;
        out.extend_from_slice(&length_words.to_be_bytes());

        out.push(sample.finetune_byte);
        out.push(sample.volume);
        out.extend_from_slice(&sample.repeat_start_words.to_be_bytes());
        out.extend_from_slice(&sample.repeat_length_words.to_be_bytes());
    }

    out.push(module.song_length);
    out.push(module.restart);
    out.extend_from_slice(&module.order_table);
    out.extend_from_slice(&module.magic);

    for (p, pattern) in module.patterns.iter().enumerate() {
        for (r, row) in pattern.iter().enumerate() {
            for (c, note) in row.iter().enumerate() {
                if note.period > 0x0FFF || note.effect > 0x0F {
                    return Err(EncodeError::NoteOutOfRange {
                        pattern: p,
                        row: r,
                        channel: c,
                    });
                }
                let period_hi = ((note.period >> 8) as u8) & 0x0F;
                let b0 = (note.sample & 0xF0) | period_hi;
                let b1 = (note.period & 0xFF) as u8;
                let b2 = ((note.sample & 0x0F) << 4) | note.effect;
                let b3 = note.param;
                out.extend_from_slice(&[b0, b1, b2, b3]);
            }
        }
    }

    for sample in &module.samples {
        out.extend(sample.data.iter().map(|&b| b as u8));
    }

    out.extend_from_slice(&module.trailing);

    Ok(out)
}

/// Convert a byte length to a 16-bit word count, rejecting an odd length
/// (the format only stores whole words) or one too long to fit `u16`.
fn to_word_count(bytes: usize) -> Option<u16> {
    if !bytes.is_multiple_of(2) {
        return None;
    }
    u16::try_from(bytes / 2).ok()
}
