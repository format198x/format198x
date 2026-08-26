//! Amiga **PowerPacker (PP20)** decrunching.
//!
//! PowerPacker is the cruncher Amiga music disks overwhelmingly use to shrink
//! tracker modules and executables: of the first four modules found on a
//! real Gathering '92 music disk during this project's research, three were
//! PP20-crunched. A media player that cannot decrunch cannot read that disk,
//! so this crate exists to make real Amiga media readable — decode-only, no
//! compressor.
//!
//! # Layout
//!
//! ```text
//! offset 0     "PP20" magic (4 bytes)
//! offset 4     4 offset-length bytes, one per back-reference length class
//! offset 8     crunched data
//! offset N-4   trailer: top 3 bytes = decrunched length, low byte = initial
//!              bit-skip
//! ```
//!
//! # The bitstream runs backwards
//!
//! The part every naive port gets wrong: the compressor packs from the
//! *front* of the input, so the decruncher must read the crunched data
//! **backwards from the end**, one byte at a time, and write the decrunched
//! output **backwards from its end toward its start**. A back-reference
//! offset therefore points *forward* in the output buffer — toward bytes
//! already written, nearer the end — not backward the way a conventional
//! LZ77 decoder's window does.
//!
//! # Algorithm provenance
//!
//! The bit-level algorithm below is ported from libxmp's
//! `depackers/ppdepack.c` (`ppDecrunch`) — the implementation most tracker
//! players have shipped for two decades. Its own header records the lineage:
//! the decruncher is based on code by **Stuart Caie**, placed in the public
//! domain; the version libxmp carries came from **Heikki Orsila**'s
//! `amigadepack` 0.02; and **Claudio Matsuoka** modified it for xmp in
//! 08/2007, merging in the corrupt-file and data-detection checks from the
//! older depack sources (credited there to Don Adan, Dirk Stoecker and Georg
//! Hoermann) and again in 05/2013 to remove the decryption code.
//!
//! Matsuoka's 2007 merge is precisely the corruption detection this port
//! carries over, so that credit is load-bearing rather than ceremonial.
//!
//! This port keeps the control flow (the literal/match continuation codes,
//! the 4-way offset-length selector, the `x == 3` long-match escape)
//! exactly, but replaces every raw pointer read with a bounds-checked one: a
//! malformed offset-length byte, an initial bit-skip over 32, a run or match
//! length longer than the declared output, or a back-reference that would
//! land past the end of the output all return [`DecodeError::Corrupt`] or
//! [`DecodeError::Truncated`] rather than reading out of bounds.
//!
//! # Memory: the input does not bound the output
//!
//! A PP20 stream declares its decrunched length in a 3-byte trailer field,
//! so a 12-byte input can legitimately ask for a 16 MB output buffer, and
//! [`decrunch`] allocates it before reading a single bit of the body. That
//! is the format's own ceiling, not a bug — but a caller working to a
//! tighter budget should read the declared length itself (the top 3 bytes
//! of the last 4) and decline before calling, rather than expect
//! [`decrunch`] to refuse on its behalf.
//!
//! # Example
//!
//! ```
//! use format198x_commodore_amiga_powerpacker::is_powerpacked;
//!
//! assert!(is_powerpacked(b"PP20\x09\x0a\x0c\x0d\x00\x00\x00\x00"));
//! ```

mod error;
pub use error::DecodeError;

/// The 4-byte magic every PP20 stream starts with.
pub const MAGIC: [u8; 4] = *b"PP20";

/// The shortest a PP20 stream can be: 4-byte magic, 4-byte offset-length
/// table, 4-byte trailer, zero bytes of crunched data.
pub const MIN_LEN: usize = 12;

/// The largest bit-width [`decrunch`] will read in one go. Every width the
/// algorithm actually uses (the fixed 1/2/3/8/7-bit reads, the initial
/// bit-skip, and the per-class offset width) fits comfortably under this —
/// it exists purely to reject a hostile offset-length byte before it can
/// turn into an oversized read.
const MAX_BITS_PER_READ: u32 = 32;

/// The largest value an offset-length table byte may hold. Genuine PP20
/// files use 9–13. The upper bound mirrors the range check libxmp's loader
/// applies (each byte's high nibble must be zero) rather than inventing a
/// new one; rejecting a width of zero as well is this crate's own addition,
/// because a zero-width offset read is meaningless and the reference
/// decruncher does not guard against it.
const MAX_OFFSET_BITS: u8 = 15;

/// Whether `bytes` looks like a PowerPacker (PP20) stream: long enough to
/// hold a header and trailer, and starting with the `"PP20"` magic.
///
/// This is a cheap sniff, not a validation — [`decrunch`] does the real
/// checking and can still fail on bytes this function accepts.
#[must_use]
pub fn is_powerpacked(bytes: &[u8]) -> bool {
    bytes.len() >= MIN_LEN && bytes[..4] == MAGIC
}

/// Decrunch a PowerPacker (PP20) stream, returning its original bytes.
///
/// # Errors
///
/// [`DecodeError::BadMagic`] when `bytes` does not start with `"PP20"`.
/// [`DecodeError::Truncated`] when `bytes` is shorter than [`MIN_LEN`], or
/// the backward bitstream runs out of source bytes before decompression
/// finishes. [`DecodeError::Corrupt`] when a header field or back-reference
/// is out of range for the format — never a panic, even on hostile input.
///
/// # Allocation
///
/// The output buffer is allocated up front at the length the stream's
/// trailer declares, which a 12-byte input can legitimately set as high as
/// 16 MB. See the crate documentation's memory section.
pub fn decrunch(bytes: &[u8]) -> Result<Vec<u8>, DecodeError> {
    if bytes.len() < MIN_LEN {
        return Err(DecodeError::Truncated {
            what: "PP20 header and trailer",
        });
    }
    if bytes[..4] != MAGIC {
        return Err(DecodeError::BadMagic);
    }

    let len = bytes.len();
    let offset_lens = &bytes[4..8];
    for &width in offset_lens {
        if width == 0 || width > MAX_OFFSET_BITS {
            return Err(DecodeError::Corrupt {
                what: "offset-length table entry out of range",
            });
        }
    }

    let trailer = &bytes[len - 4..];
    let dest_len =
        (usize::from(trailer[0]) << 16) | (usize::from(trailer[1]) << 8) | usize::from(trailer[2]);
    let skip_bits = trailer[3];
    if skip_bits as u32 > MAX_BITS_PER_READ {
        return Err(DecodeError::Corrupt {
            what: "initial bit-skip exceeds 32 bits",
        });
    }

    let body = &bytes[8..len - 4];
    let mut reader = BitReader::new(body);
    reader.read_bits(u32::from(skip_bits))?; // discard the padding bits

    let mut out = vec![0u8; dest_len];
    let mut written = 0usize;
    // `cursor` mirrors the reference decruncher's `out` pointer as an offset
    // from the start of the output buffer: it starts one past the last
    // index and decrements with every byte written, so the buffer fills
    // from its end toward its start.
    let mut cursor = dest_len;

    'outer: while written < dest_len {
        // A clear bit starts a literal run followed by a match; a set bit
        // is a match with no literal run in front of it.
        if reader.read_bits(1)? == 0 {
            let mut run = 1u32;
            loop {
                let chunk = reader.read_bits(2)?;
                // A literal run can never be longer than the output it has
                // to fit into, so `dest_len` bounds this accumulator. The
                // continuation code chains without limit, so without the
                // cap a hostile body grows `run` by 3 per 2 bits until it
                // overflows: a panic in debug and test builds, a silent
                // wrap in release.
                run = run.saturating_add(chunk);
                if run as usize > dest_len {
                    return Err(DecodeError::Corrupt {
                        what: "literal run length exceeds the declared output length",
                    });
                }
                if chunk != 3 {
                    break;
                }
            }
            for _ in 0..run {
                let byte = reader.read_bits(8)?;
                byte_out(&mut out, &mut cursor, byte as u8, &mut written)?;
            }
            if written == dest_len {
                break 'outer;
            }
        }

        let selector = reader.read_bits(2)?;
        let mut offset_bits = u32::from(offset_lens[selector as usize]);
        let mut length = selector + 2;
        let offset = if selector == 3 {
            if reader.read_bits(1)? == 0 {
                offset_bits = 7;
            }
            let offset = reader.read_bits(offset_bits)?;
            loop {
                let chunk = reader.read_bits(3)?;
                // Bounded by `dest_len` for the same reason `run` is: a
                // match longer than the declared output can never succeed,
                // and the `x == 7` continuation chains without limit.
                length = length.saturating_add(chunk);
                if length as usize > dest_len {
                    return Err(DecodeError::Corrupt {
                        what: "match length exceeds the declared output length",
                    });
                }
                if chunk != 7 {
                    break;
                }
            }
            offset
        } else {
            reader.read_bits(offset_bits)?
        } as usize;

        if cursor + offset >= dest_len {
            return Err(DecodeError::Corrupt {
                what: "back-reference offset overruns the output buffer",
            });
        }
        for _ in 0..length {
            let byte = out[cursor + offset];
            byte_out(&mut out, &mut cursor, byte, &mut written)?;
        }
    }

    Ok(out)
}

/// Write one byte to the position just before `cursor`, then move `cursor`
/// back onto it — the backward-filling counterpart of a forward `push`.
/// Bounds-checked: `cursor` reaching zero before `written` reaches the
/// output length means the stream is corrupt, not a buffer overrun.
fn byte_out(
    out: &mut [u8],
    cursor: &mut usize,
    byte: u8,
    written: &mut usize,
) -> Result<(), DecodeError> {
    if *cursor == 0 {
        return Err(DecodeError::Corrupt {
            what: "decrunched output overflowed its declared length",
        });
    }
    *cursor -= 1;
    out[*cursor] = byte;
    *written += 1;
    Ok(())
}

/// Reads the PP20 bitstream from its end toward its start, LSB-first within
/// each byte, matching the reference decruncher bit for bit.
///
/// `pos` is the index of the next unread byte in `body`; a read pulls
/// `body[pos - 1]` and decrements `pos`, so `pos == 0` means the source is
/// exhausted — checked before every pull, never indexed past.
struct BitReader<'a> {
    body: &'a [u8],
    pos: usize,
    bit_buffer: u64,
    bits_left: u32,
}

impl<'a> BitReader<'a> {
    fn new(body: &'a [u8]) -> Self {
        Self {
            body,
            pos: body.len(),
            bit_buffer: 0,
            bits_left: 0,
        }
    }

    /// Read `n` bits (`n <= MAX_BITS_PER_READ`) as an integer, most
    /// significant of the `n` first. Pulls whole bytes from the tail of
    /// `body` into a 64-bit buffer as needed — wide enough that the largest
    /// width this crate ever requests can never overflow it.
    fn read_bits(&mut self, n: u32) -> Result<u32, DecodeError> {
        if n == 0 {
            return Ok(0);
        }
        if n > MAX_BITS_PER_READ {
            return Err(DecodeError::Corrupt {
                what: "bitstream read width out of range",
            });
        }
        while self.bits_left < n {
            if self.pos == 0 {
                return Err(DecodeError::Truncated {
                    what: "PP20 bitstream",
                });
            }
            self.pos -= 1;
            self.bit_buffer |= u64::from(self.body[self.pos]) << self.bits_left;
            self.bits_left += 8;
        }
        self.bits_left -= n;
        let mut value: u32 = 0;
        for _ in 0..n {
            value = (value << 1) | (self.bit_buffer & 1) as u32;
            self.bit_buffer >>= 1;
        }
        Ok(value)
    }
}
