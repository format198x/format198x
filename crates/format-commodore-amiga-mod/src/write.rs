//! Encoding: a [`Module`] into raw bytes — the exact inverse of
//! [`decode`](crate::decode).
//!
//! Two bytes ProTracker itself ignores are not carried by [`Module`] and so
//! cannot be reproduced: the restart byte at offset 951 (fixed here at `0`)
//! and the magic variant (always written as `M.K.`). See the crate
//! documentation's "What a round-trip cannot preserve" section.

use crate::Module;
use crate::error::EncodeError;

const TITLE_LEN: usize = 20;
const NUM_SAMPLES: usize = 31;
const SAMPLE_NAME_LEN: usize = 22;
const ROWS_PER_PATTERN: usize = 64;
const CHANNELS: usize = 4;
const RESTART_BYTE: u8 = 0;
const MAGIC: &[u8; 4] = b"M.K.";

/// Encode a [`Module`] as ProTracker MOD bytes.
///
/// # Errors
///
/// [`EncodeError::WrongSampleCount`] unless `module.samples.len() == 31`.
/// [`EncodeError::TooManyOrders`] if the order table has more than 128
/// entries. [`EncodeError::WrongPatternRows`] if a pattern is not exactly 64
/// rows. [`EncodeError::SampleDataInvalid`] or
/// [`EncodeError::LoopInvalid`] if a sample's data length, loop start, or
/// loop length is odd or too large for the header's 16-bit word fields.
/// [`EncodeError::NoteOutOfRange`] if a note's period exceeds 12 bits or its
/// effect exceeds 4 bits. [`EncodeError::PatternDataTooLarge`] if the
/// pattern count overflows while computing the pattern data size.
pub fn encode(module: &Module) -> Result<Vec<u8>, EncodeError> {
    if module.samples.len() != NUM_SAMPLES {
        return Err(EncodeError::WrongSampleCount {
            found: module.samples.len(),
        });
    }
    if module.orders.len() > 128 {
        return Err(EncodeError::TooManyOrders {
            found: module.orders.len(),
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

    let mut title_field = [0u8; TITLE_LEN];
    write_padded(&mut title_field, module.title.as_bytes());
    out.extend_from_slice(&title_field);

    for (index, sample) in module.samples.iter().enumerate() {
        let mut name_field = [0u8; SAMPLE_NAME_LEN];
        write_padded(&mut name_field, sample.name.as_bytes());
        out.extend_from_slice(&name_field);

        let length_words =
            to_word_count(sample.data.len()).ok_or(EncodeError::SampleDataInvalid { index })?;
        out.extend_from_slice(&length_words.to_be_bytes());

        // Finetune is a signed nibble; fold any value into 0..=15 by twos
        // complement on 4 bits so the write never panics on an
        // out-of-range value.
        let finetune_byte = (sample.finetune as i32).rem_euclid(16) as u8;
        out.push(finetune_byte);
        out.push(sample.volume);

        let repeat_start_words =
            to_word_count(sample.loop_start).ok_or(EncodeError::LoopInvalid { index })?;
        out.extend_from_slice(&repeat_start_words.to_be_bytes());

        // loop_len == 0 means "no loop"; ProTracker's own convention for
        // that is a repeat length of 0 or 1 words; a bare 0 is written back
        // (see the crate documentation — the original raw value of 0 vs. 1
        // is not recoverable once decoded).
        let repeat_length_words = if sample.loop_len == 0 {
            0
        } else {
            to_word_count(sample.loop_len).ok_or(EncodeError::LoopInvalid { index })?
        };
        out.extend_from_slice(&repeat_length_words.to_be_bytes());
    }

    out.push(module.orders.len() as u8);
    out.push(RESTART_BYTE);

    let mut order_table = [0u8; 128];
    order_table[..module.orders.len()].copy_from_slice(&module.orders);
    out.extend_from_slice(&order_table);

    out.extend_from_slice(MAGIC);

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

    Ok(out)
}

/// Write `src` into `field`, NUL-padding or truncating to fit exactly.
fn write_padded(field: &mut [u8], src: &[u8]) {
    let n = src.len().min(field.len());
    field[..n].copy_from_slice(&src[..n]);
}

/// Convert a byte length to a 16-bit word count, rejecting an odd length
/// (the format only stores whole words) or one too long to fit `u16`.
fn to_word_count(bytes: usize) -> Option<u16> {
    if !bytes.is_multiple_of(2) {
        return None;
    }
    u16::try_from(bytes / 2).ok()
}
