//! Raw DEFLATE decoding (RFC 1951).

use crate::bits::BitReader;
use crate::huffman::{Decoder, Kind};
use crate::tables::{
    CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, END_OF_BLOCK, FIXED_DIST, FIXED_LITLEN, LENGTH_BASE,
    LENGTH_EXTRA,
};
use crate::{Error, Options};
use alloc::vec::Vec;

/// Decode a raw DEFLATE stream that fills `data` exactly.
///
/// # Errors
///
/// Returns an [`Error`] for a damaged or truncated stream, and
/// [`Error::TrailingBytes`] when bytes follow it.
///
/// ```
/// // A fixed-Huffman block holding "hi".
/// assert_eq!(format198x_deflate::inflate(&[0xCB, 0xC8, 0x04, 0x00])?, b"hi");
/// # Ok::<(), format198x_deflate::Error>(())
/// ```
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, Error> {
    inflate_with(data, &Options::DEFAULT)
}

/// Decode a raw DEFLATE stream under the given options.
///
/// # Errors
///
/// As [`inflate`], plus [`Error::OutputLimit`] past `options.max_output`.
/// Trailing bytes are an error unless `options.allow_trailing` is set.
pub fn inflate_with(data: &[u8], options: &Options) -> Result<Vec<u8>, Error> {
    let (out, used) = inflate_prefix(data, options.max_output)?;
    if used < data.len() && !options.allow_trailing {
        return Err(Error::TrailingBytes { offset: used });
    }
    Ok(out)
}

/// Decode the DEFLATE stream at the start of `data`, returning the output
/// and how many input bytes the stream used. A partly used last byte counts
/// as used. Bytes after the stream are not examined.
///
/// # Errors
///
/// As [`inflate`], plus [`Error::OutputLimit`] past `max_output` bytes.
pub fn inflate_prefix(data: &[u8], max_output: usize) -> Result<(Vec<u8>, usize), Error> {
    let mut reader = BitReader::new(data);
    let mut out = Vec::with_capacity(data.len().saturating_mul(3).min(max_output).min(1 << 24));
    let mut fixed: Option<(Decoder, Decoder)> = None;
    loop {
        let last = reader.bits(1)? == 1;
        match reader.bits(2)? {
            0 => stored(&mut reader, &mut out, max_output)?,
            1 => {
                if fixed.is_none() {
                    fixed = Some((
                        Decoder::new(&FIXED_LITLEN, Kind::Symbols)?,
                        Decoder::new(&FIXED_DIST, Kind::Symbols)?,
                    ));
                }
                if let Some((lit, dist)) = &fixed {
                    codes(&mut reader, &mut out, lit, dist, max_output)?;
                }
            }
            2 => {
                let (lit, dist) = dynamic_tables(&mut reader)?;
                codes(&mut reader, &mut out, &lit, &dist, max_output)?;
            }
            _ => return Err(Error::ReservedBlockType),
        }
        if last {
            break;
        }
    }
    Ok((out, reader.consumed()))
}

fn stored(reader: &mut BitReader<'_>, out: &mut Vec<u8>, max: usize) -> Result<(), Error> {
    reader.align();
    let len = reader.bits(16)? as u16;
    let nlen = reader.bits(16)? as u16;
    if len != !nlen {
        return Err(Error::StoredLengthMismatch { len, nlen });
    }
    let len = usize::from(len);
    if out.len() + len > max {
        return Err(Error::OutputLimit { limit: max });
    }
    reader.copy_bytes(len, out)
}

fn dynamic_tables(reader: &mut BitReader<'_>) -> Result<(Decoder, Decoder), Error> {
    let nlit = reader.bits(5)? as usize + 257;
    let ndist = reader.bits(5)? as usize + 1;
    let ncode = reader.bits(4)? as usize + 4;
    if nlit > 286 || ndist > 30 {
        return Err(Error::TooManyCodes);
    }
    let mut code_lengths = [0u8; 19];
    for &i in &CODE_LENGTH_ORDER[..ncode] {
        code_lengths[i] = reader.bits(3)? as u8;
    }
    let lencode = Decoder::new(&code_lengths, Kind::CodeLengths)?;
    let mut lengths = [0u8; 286 + 30];
    let total = nlit + ndist;
    let mut i = 0;
    while i < total {
        let sym = lencode.decode(reader)?;
        let (value, repeat) = match sym {
            0..=15 => (sym as u8, 1),
            16 => {
                let prev = *i
                    .checked_sub(1)
                    .and_then(|p| lengths.get(p))
                    .ok_or(Error::BadLengthRepeat)?;
                (prev, 3 + reader.bits(2)? as usize)
            }
            17 => (0, 3 + reader.bits(3)? as usize),
            _ => (0, 11 + reader.bits(7)? as usize),
        };
        if i + repeat > total {
            return Err(Error::BadLengthRepeat);
        }
        lengths[i..i + repeat].fill(value);
        i += repeat;
    }
    if lengths[usize::from(END_OF_BLOCK)] == 0 {
        return Err(Error::BadCodeLengths);
    }
    let lit = Decoder::new(&lengths[..nlit], Kind::Symbols)?;
    let dist = Decoder::new(&lengths[nlit..total], Kind::Symbols)?;
    Ok((lit, dist))
}

fn codes(
    reader: &mut BitReader<'_>,
    out: &mut Vec<u8>,
    lit: &Decoder,
    dist: &Decoder,
    max: usize,
) -> Result<(), Error> {
    loop {
        let sym = lit.decode(reader)?;
        if sym < 256 {
            if out.len() >= max {
                return Err(Error::OutputLimit { limit: max });
            }
            out.push(sym as u8);
            continue;
        }
        if sym == END_OF_BLOCK {
            return Ok(());
        }
        let index = usize::from(sym - 257);
        let (Some(&base), Some(&extra)) = (LENGTH_BASE.get(index), LENGTH_EXTRA.get(index)) else {
            return Err(Error::InvalidCode);
        };
        let len = usize::from(base) + reader.bits(u32::from(extra))? as usize;
        let dsym = usize::from(dist.decode(reader)?);
        let (Some(&dbase), Some(&dextra)) = (DIST_BASE.get(dsym), DIST_EXTRA.get(dsym)) else {
            return Err(Error::InvalidCode);
        };
        let distance = usize::from(dbase) + reader.bits(u32::from(dextra))? as usize;
        if distance > out.len() {
            return Err(Error::DistanceTooFar {
                distance,
                available: out.len(),
            });
        }
        if out.len() + len > max {
            return Err(Error::OutputLimit { limit: max });
        }
        let start = out.len() - distance;
        if distance >= len {
            out.extend_from_within(start..start + len);
        } else {
            // The match overlaps what it writes: copy in distance-sized
            // runs, each of which is already complete.
            let mut left = len;
            while left > 0 {
                let n = left.min(out.len() - start);
                out.extend_from_within(start..start + n);
                left -= n;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn empty_and_tiny_streams() {
        // Python: zlib.compressobj(wbits=-15) of b"" is 03 00.
        assert_eq!(inflate(&[0x03, 0x00]), Ok(vec![]));
        // A final stored block holding "abc".
        assert_eq!(
            inflate(&[0x01, 3, 0, 0xFC, 0xFF, b'a', b'b', b'c']),
            Ok(b"abc".to_vec())
        );
    }

    #[test]
    fn overlapping_matches_repeat_runs() {
        // "a" then a length-10, distance-1 match: Python's output for b"a" * 11.
        let packed = [0x4B, 0x4C, 0x84, 0x03, 0x00];
        assert_eq!(inflate(&packed), Ok(vec![b'a'; 11]));
    }

    #[test]
    fn structural_errors() {
        assert_eq!(inflate(&[]), Err(Error::UnexpectedEof));
        assert_eq!(inflate(&[0x07]), Err(Error::ReservedBlockType));
        assert_eq!(
            inflate(&[0x01, 3, 0, 0, 0]),
            Err(Error::StoredLengthMismatch { len: 3, nlen: 0 })
        );
        assert_eq!(
            inflate(&[0x01, 3, 0, 0xFC, 0xFF, b'a']),
            Err(Error::UnexpectedEof)
        );
        assert_eq!(
            inflate(&[0x03, 0x00, 0x00]),
            Err(Error::TrailingBytes { offset: 2 })
        );
        let o = Options::default().allow_trailing(true);
        assert_eq!(inflate_with(&[0x03, 0x00, 0x00], &o), Ok(vec![]));
    }

    #[test]
    fn the_output_limit_holds_for_every_block_type() {
        let o = Options::default().max_output(10);
        assert_eq!(
            inflate_with(&[0x4B, 0x4C, 0x84, 0x03, 0x00], &o),
            Err(Error::OutputLimit { limit: 10 })
        );
        let o = Options::default().max_output(2);
        assert_eq!(
            inflate_with(&[0x01, 3, 0, 0xFC, 0xFF, b'a', b'b', b'c'], &o),
            Err(Error::OutputLimit { limit: 2 })
        );
        let o = Options::default().max_output(0);
        assert_eq!(
            inflate_with(&[0xCB, 0xC8, 0x04, 0x00], &o),
            Err(Error::OutputLimit { limit: 0 })
        );
    }
}
