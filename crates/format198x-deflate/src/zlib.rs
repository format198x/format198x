//! The zlib wrapper (RFC 1950): a two-byte header, a DEFLATE stream and a
//! big-endian Adler-32 of the decoded data.
//!
//! SZX snapshot pages and RZX recordings store their payloads this way.

use crate::{Error, Level, Options, adler32, deflate, inflate_prefix};
use alloc::vec::Vec;

/// Decode a zlib stream that fills `data` exactly.
///
/// # Errors
///
/// Returns [`Error::BadZlibHeader`], [`Error::UnsupportedMethod`] or
/// [`Error::PresetDictionary`] for a header this crate cannot use,
/// [`Error::Adler32Mismatch`] when the checksum fails, the errors of
/// [`inflate`](crate::inflate), and [`Error::TrailingBytes`] when bytes
/// follow the stream.
pub fn decode(data: &[u8]) -> Result<Vec<u8>, Error> {
    decode_with(data, &Options::DEFAULT)
}

/// Decode a zlib stream under the given options.
///
/// # Errors
///
/// As [`decode`], plus [`Error::OutputLimit`] past `options.max_output`.
/// Trailing bytes are an error unless `options.allow_trailing` is set.
pub fn decode_with(data: &[u8], options: &Options) -> Result<Vec<u8>, Error> {
    let [cmf, flg, body @ ..] = data else {
        return Err(Error::UnexpectedEof);
    };
    if (u16::from(*cmf) << 8 | u16::from(*flg)) % 31 != 0 {
        return Err(Error::BadZlibHeader);
    }
    if cmf & 0x0F != 8 {
        return Err(Error::UnsupportedMethod(cmf & 0x0F));
    }
    if cmf >> 4 > 7 {
        return Err(Error::BadZlibHeader);
    }
    if flg & 0x20 != 0 {
        return Err(Error::PresetDictionary);
    }
    let (out, used) = inflate_prefix(body, options.max_output)?;
    let trailer = body.get(used..used + 4).ok_or(Error::UnexpectedEof)?;
    let expected = u32::from_be_bytes([trailer[0], trailer[1], trailer[2], trailer[3]]);
    let actual = adler32(&out);
    if expected != actual {
        return Err(Error::Adler32Mismatch { expected, actual });
    }
    let end = 2 + used + 4;
    if end < data.len() && !options.allow_trailing {
        return Err(Error::TrailingBytes { offset: end });
    }
    Ok(out)
}

/// Compress `data` as a zlib stream.
///
/// The header's level field follows zlib's convention, so level 6 starts
/// `78 9C`, level 9 `78 DA` and levels 0 and 1 `78 01`.
#[must_use]
pub fn encode(data: &[u8], level: Level) -> Vec<u8> {
    let flevel: u8 = match level.get() {
        0 | 1 => 0,
        2..=5 => 1,
        6 => 2,
        _ => 3,
    };
    let cmf: u8 = 0x78;
    let mut flg = flevel << 6;
    flg += 31 - ((u16::from(cmf) << 8 | u16::from(flg)) % 31) as u8 % 31;
    let mut out = Vec::with_capacity(data.len() / 2 + 16);
    out.push(cmf);
    out.push(flg);
    out.extend_from_slice(&deflate(data, level));
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn headers_match_zlib() {
        assert_eq!(&encode(b"", Level::DEFAULT)[..2], [0x78, 0x9C]);
        assert_eq!(&encode(b"", Level::BEST)[..2], [0x78, 0xDA]);
        assert_eq!(&encode(b"", Level::FASTEST)[..2], [0x78, 0x01]);
        assert_eq!(&encode(b"", Level::new(3))[..2], [0x78, 0x5E]);
        // Python: zlib.compress(b"") == 78 9C 03 00 00 00 00 01.
        assert_eq!(
            encode(b"", Level::DEFAULT),
            [0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]
        );
    }

    #[test]
    fn header_and_trailer_errors() {
        assert_eq!(decode(&[0x78]), Err(Error::UnexpectedEof));
        assert_eq!(decode(&[0x78, 0x9D, 3, 0]), Err(Error::BadZlibHeader));
        // CM 7, with check bits fixed up.
        assert_eq!(decode(&[0x77, 0x85]), Err(Error::UnsupportedMethod(7)));
        // CINFO 8: a 64K window.
        assert_eq!(decode(&[0x88, 0x98]), Err(Error::BadZlibHeader));
        assert_eq!(decode(&[0x78, 0xBB]), Err(Error::PresetDictionary));
        let mut good = encode(b"hello", Level::DEFAULT);
        assert_eq!(decode(&good), Ok(b"hello".to_vec()));
        good.push(0);
        assert_eq!(
            decode(&good),
            Err(Error::TrailingBytes {
                offset: good.len() - 1
            })
        );
        let lenient = Options::default().allow_trailing(true);
        assert_eq!(decode_with(&good, &lenient), Ok(b"hello".to_vec()));
        good.pop();
        let n = good.len();
        good[n - 1] ^= 1;
        assert!(matches!(decode(&good), Err(Error::Adler32Mismatch { .. })));
        good.truncate(n - 2);
        assert_eq!(decode(&good), Err(Error::UnexpectedEof));
        let _ = vec![0u8; 0];
    }
}
