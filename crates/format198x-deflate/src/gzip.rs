//! The gzip wrapper (RFC 1952): a header with optional name, comment and
//! extra field, a DEFLATE stream, then the CRC-32 and length of the
//! decoded data.
//!
//! UEF tape images are usually gzipped whole.

use crate::{Error, Level, Options, crc32, deflate, inflate_prefix};
use alloc::vec::Vec;

/// The gzip signature.
pub const MAGIC: [u8; 2] = [0x1F, 0x8B];

const FTEXT: u8 = 0x01;
const FHCRC: u8 = 0x02;
const FEXTRA: u8 = 0x04;
const FNAME: u8 = 0x08;
const FCOMMENT: u8 = 0x10;
const FRESERVED: u8 = 0xE0;

/// A gzip member header. Every field is kept, so a member writes back with
/// the same header it was read with.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Header {
    /// `FTEXT`: the writer thought the data was text.
    pub text: bool,
    /// `MTIME`: modification time in Unix seconds, 0 when unknown.
    pub mtime: u32,
    /// `XFL`: 2 for maximum compression, 4 for fastest, otherwise 0.
    pub extra_flags: u8,
    /// `OS`: the writer's file system; 255 for unknown.
    pub os: u8,
    /// `FEXTRA`: the extra field's bytes.
    pub extra: Option<Vec<u8>>,
    /// `FNAME`: the original file name, without its NUL. ISO 8859-1 by
    /// the specification.
    pub name: Option<Vec<u8>>,
    /// `FCOMMENT`: a comment, without its NUL.
    pub comment: Option<Vec<u8>>,
    /// `FHCRC`: whether the header carries its own CRC-16.
    pub header_crc: bool,
}

impl Default for Header {
    /// No name, no comment, time 0, OS unknown.
    fn default() -> Self {
        Self {
            text: false,
            mtime: 0,
            extra_flags: 0,
            os: 255,
            extra: None,
            name: None,
            comment: None,
            header_crc: false,
        }
    }
}

/// A decoded gzip member: its header and data.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Member {
    /// The header.
    pub header: Header,
    /// The decoded data.
    pub data: Vec<u8>,
}

/// Whether `data` starts with the gzip signature.
#[must_use]
pub fn is_gzip(data: &[u8]) -> bool {
    data.starts_with(&MAGIC)
}

/// Decode a gzip file.
///
/// A file may hold several members one after another; their data is joined,
/// as `gunzip` does, and the first member's header is returned. Anything
/// after a member that starts with the gzip signature is read as another
/// member. Zero bytes after the last member are ignored, since some tools
/// pad with them.
///
/// # Errors
///
/// Returns [`Error::BadGzipMagic`], [`Error::UnsupportedMethod`],
/// [`Error::ReservedGzipFlags`] or [`Error::HeaderCrcMismatch`] for a bad
/// header, [`Error::Crc32Mismatch`] or [`Error::SizeMismatch`] when the
/// trailer fails, the errors of [`inflate`](crate::inflate), and
/// [`Error::TrailingBytes`] for other bytes after the last member.
pub fn decode(data: &[u8]) -> Result<Member, Error> {
    decode_with(data, &Options::DEFAULT)
}

/// Decode a gzip file under the given options.
///
/// # Errors
///
/// As [`decode`], plus [`Error::OutputLimit`] past `options.max_output`,
/// counted over all members. Trailing bytes are an error unless
/// `options.allow_trailing` is set.
pub fn decode_with(data: &[u8], options: &Options) -> Result<Member, Error> {
    let mut members = decode_members(data, options)?.into_iter();
    let Some(mut first) = members.next() else {
        return Err(Error::BadGzipMagic);
    };
    for member in members {
        first.data.extend_from_slice(&member.data);
    }
    Ok(first)
}

/// Decode every member of a gzip file separately.
///
/// # Errors
///
/// As [`decode_with`].
pub fn decode_members(data: &[u8], options: &Options) -> Result<Vec<Member>, Error> {
    let mut members = Vec::new();
    let mut offset = 0;
    let mut total = 0usize;
    loop {
        let (member, used) =
            decode_member(&data[offset..], options.max_output - total).map_err(|e| match e {
                Error::OutputLimit { .. } => Error::OutputLimit {
                    limit: options.max_output,
                },
                e => e,
            })?;
        total += member.data.len();
        members.push(member);
        offset += used;
        let rest = &data[offset..];
        if is_gzip(rest) {
            continue;
        }
        if rest.is_empty() || options.allow_trailing || rest.iter().all(|&b| b == 0) {
            return Ok(members);
        }
        return Err(Error::TrailingBytes { offset });
    }
}

/// A cursor over a header, failing with [`Error::UnexpectedEof`].
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::UnexpectedEof)?;
        let bytes = self.data.get(self.pos..end).ok_or(Error::UnexpectedEof)?;
        self.pos = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, Error> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn zstring(&mut self) -> Result<Vec<u8>, Error> {
        let rest = self.data.get(self.pos..).unwrap_or(&[]);
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::UnexpectedEof)?;
        let s = rest[..len].to_vec();
        self.pos += len + 1;
        Ok(s)
    }
}

fn decode_member(data: &[u8], max_output: usize) -> Result<(Member, usize), Error> {
    let mut c = Cursor { data, pos: 0 };
    if c.take(2).map_err(|_| Error::BadGzipMagic)? != MAGIC {
        return Err(Error::BadGzipMagic);
    }
    let method = c.u8()?;
    if method != 8 {
        return Err(Error::UnsupportedMethod(method));
    }
    let flags = c.u8()?;
    if flags & FRESERVED != 0 {
        return Err(Error::ReservedGzipFlags(flags));
    }
    let mtime = c.u32()?;
    let extra_flags = c.u8()?;
    let os = c.u8()?;
    let extra = if flags & FEXTRA != 0 {
        let len = usize::from(c.u16()?);
        Some(c.take(len)?.to_vec())
    } else {
        None
    };
    let name = (flags & FNAME != 0).then(|| c.zstring()).transpose()?;
    let comment = (flags & FCOMMENT != 0).then(|| c.zstring()).transpose()?;
    let header_crc = flags & FHCRC != 0;
    if header_crc {
        let actual = crc32(&data[..c.pos]) as u16;
        let expected = c.u16()?;
        if expected != actual {
            return Err(Error::HeaderCrcMismatch { expected, actual });
        }
    }
    let (out, used) = inflate_prefix(&data[c.pos..], max_output)?;
    c.pos += used;
    let expected = c.u32()?;
    let size = c.u32()?;
    let actual = crc32(&out);
    if expected != actual {
        return Err(Error::Crc32Mismatch { expected, actual });
    }
    let actual_size = out.len() as u32;
    if size != actual_size {
        return Err(Error::SizeMismatch {
            expected: size,
            actual: actual_size,
        });
    }
    let header = Header {
        text: flags & FTEXT != 0,
        mtime,
        extra_flags,
        os,
        extra,
        name,
        comment,
        header_crc,
    };
    Ok((Member { header, data: out }, c.pos))
}

/// Compress `data` as a one-member gzip file with the given header.
///
/// # Errors
///
/// Returns [`Error::NulInHeaderField`] when the name or comment contains a
/// NUL byte, and [`Error::ExtraFieldTooLong`] for an extra field over
/// 65535 bytes.
pub fn encode(header: &Header, data: &[u8], level: Level) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(data.len() / 2 + 32);
    out.extend_from_slice(&MAGIC);
    out.push(8);
    let mut flags = 0;
    if header.text {
        flags |= FTEXT;
    }
    if header.header_crc {
        flags |= FHCRC;
    }
    if header.extra.is_some() {
        flags |= FEXTRA;
    }
    if header.name.is_some() {
        flags |= FNAME;
    }
    if header.comment.is_some() {
        flags |= FCOMMENT;
    }
    out.push(flags);
    out.extend_from_slice(&header.mtime.to_le_bytes());
    out.push(header.extra_flags);
    out.push(header.os);
    if let Some(extra) = &header.extra {
        let len = u16::try_from(extra.len()).map_err(|_| Error::ExtraFieldTooLong)?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(extra);
    }
    for field in [&header.name, &header.comment].into_iter().flatten() {
        if field.contains(&0) {
            return Err(Error::NulInHeaderField);
        }
        out.extend_from_slice(field);
        out.push(0);
    }
    if header.header_crc {
        let crc = crc32(&out) as u16;
        out.extend_from_slice(&crc.to_le_bytes());
    }
    out.extend_from_slice(&deflate(data, level));
    out.extend_from_slice(&crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn full_header() -> Header {
        Header {
            text: true,
            mtime: 0x1234_5678,
            extra_flags: 2,
            os: 3,
            extra: Some(b"AP\x02\x00hi".to_vec()),
            name: Some(b"tape.uef".to_vec()),
            comment: Some(b"a comment".to_vec()),
            header_crc: true,
        }
    }

    #[test]
    fn every_header_field_round_trips() {
        let bytes = encode(&full_header(), b"payload", Level::BEST).expect("encodes");
        assert_eq!(bytes[3], 0x1F);
        let member = decode(&bytes).expect("decodes");
        assert_eq!(member.header, full_header());
        assert_eq!(member.data, b"payload");
    }

    #[test]
    fn members_join_and_zero_padding_is_ignored() {
        let mut bytes = encode(&Header::default(), b"one ", Level::DEFAULT).expect("encodes");
        bytes.extend(encode(&full_header(), b"two", Level::NONE).expect("encodes"));
        bytes.extend_from_slice(&[0, 0, 0]);
        let member = decode(&bytes).expect("decodes");
        assert_eq!(member.data, b"one two");
        assert_eq!(member.header, Header::default());
        let members = decode_members(&bytes, &Options::DEFAULT).expect("decodes");
        assert_eq!(members.len(), 2);
        let limited = Options::default().max_output(5);
        assert_eq!(
            decode_with(&bytes, &limited),
            Err(Error::OutputLimit { limit: 5 })
        );
    }

    #[test]
    fn header_and_trailer_errors() {
        assert_eq!(decode(&[]), Err(Error::BadGzipMagic));
        assert_eq!(decode(b"PK\x03\x04"), Err(Error::BadGzipMagic));
        assert_eq!(decode(&[0x1F, 0x8B, 7]), Err(Error::UnsupportedMethod(7)));
        assert_eq!(
            decode(&[0x1F, 0x8B, 8, 0x20]),
            Err(Error::ReservedGzipFlags(0x20))
        );
        assert_eq!(decode(&[0x1F, 0x8B, 8, 0]), Err(Error::UnexpectedEof));
        assert_eq!(
            decode(&[0x1F, 0x8B, 8, FNAME, 0, 0, 0, 0, 0, 255, b'a']),
            Err(Error::UnexpectedEof)
        );
        let good = encode(&full_header(), b"payload", Level::DEFAULT).expect("encodes");
        let mut bad = good.clone();
        bad[4] ^= 1;
        assert!(matches!(decode(&bad), Err(Error::HeaderCrcMismatch { .. })));
        let n = good.len();
        let mut bad = good.clone();
        bad[n - 8] ^= 1;
        assert!(matches!(decode(&bad), Err(Error::Crc32Mismatch { .. })));
        let mut bad = good.clone();
        bad[n - 4] ^= 1;
        assert_eq!(
            decode(&bad),
            Err(Error::SizeMismatch {
                expected: 6,
                actual: 7
            })
        );
        let mut bad = good.clone();
        bad.extend_from_slice(b"junk");
        assert_eq!(decode(&bad), Err(Error::TrailingBytes { offset: n }));
        let lenient = Options::default().allow_trailing(true);
        assert_eq!(
            decode_with(&bad, &lenient).map(|m| m.data),
            Ok(b"payload".to_vec())
        );
        assert!(is_gzip(&good));
    }

    #[test]
    fn encode_rejects_unwritable_headers() {
        let h = Header {
            name: Some(b"a\0b".to_vec()),
            ..Header::default()
        };
        assert_eq!(
            encode(&h, b"", Level::DEFAULT),
            Err(Error::NulInHeaderField)
        );
        let h = Header {
            extra: Some(vec![0; 70_000]),
            ..Header::default()
        };
        assert_eq!(
            encode(&h, b"", Level::DEFAULT),
            Err(Error::ExtraFieldTooLong)
        );
    }
}
