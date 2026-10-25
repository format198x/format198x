//! Least-significant-bit-first bit reading and writing, as DEFLATE packs
//! its fields.

use crate::Error;
use alloc::vec::Vec;

/// Reads bits from a byte slice, low bit first.
pub struct BitReader<'a> {
    data: &'a [u8],
    /// Next byte of `data` to load into `buf`.
    pos: usize,
    /// Loaded bits, the next one in bit 0.
    buf: u64,
    /// How many bits of `buf` are loaded.
    count: u32,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            buf: 0,
            count: 0,
        }
    }

    /// Load whole bytes until the buffer holds at least 57 bits or the input
    /// runs out.
    #[inline]
    pub fn refill(&mut self) {
        while self.count <= 56 {
            let Some(&byte) = self.data.get(self.pos) else {
                break;
            };
            self.buf |= u64::from(byte) << self.count;
            self.pos += 1;
            self.count += 8;
        }
    }

    /// The loaded bits, without consuming them, and how many there are.
    #[inline]
    pub fn peek(&mut self) -> (u64, u32) {
        if self.count < 16 {
            self.refill();
        }
        (self.buf, self.count)
    }

    /// Drop `n` bits; the caller has checked that they are loaded.
    #[inline]
    pub fn consume(&mut self, n: u32) {
        self.buf >>= n;
        self.count -= n;
    }

    /// Read an `n`-bit field, `n` at most 16.
    #[inline]
    pub fn bits(&mut self, n: u32) -> Result<u32, Error> {
        if self.count < n {
            self.refill();
            if self.count < n {
                return Err(Error::UnexpectedEof);
            }
        }
        let value = (self.buf & ((1u64 << n) - 1)) as u32;
        self.consume(n);
        Ok(value)
    }

    /// Skip to the next byte boundary.
    pub fn align(&mut self) {
        let partial = self.count % 8;
        self.consume(partial);
    }

    /// Copy `len` whole bytes to `out`; the reader must be byte-aligned.
    pub fn copy_bytes(&mut self, len: usize, out: &mut Vec<u8>) -> Result<(), Error> {
        let mut left = len;
        while left > 0 && self.count >= 8 {
            out.push(self.buf as u8);
            self.consume(8);
            left -= 1;
        }
        let end = self.pos.checked_add(left).ok_or(Error::UnexpectedEof)?;
        let bytes = self.data.get(self.pos..end).ok_or(Error::UnexpectedEof)?;
        out.extend_from_slice(bytes);
        self.pos = end;
        Ok(())
    }

    /// Bytes of input used so far. A partly read byte counts as used.
    pub fn consumed(&self) -> usize {
        self.pos - (self.count / 8) as usize
    }
}

/// Writes bits to a byte vector, low bit first.
pub struct BitWriter {
    pub out: Vec<u8>,
    buf: u64,
    count: u32,
}

impl BitWriter {
    pub fn new(out: Vec<u8>) -> Self {
        Self {
            out,
            buf: 0,
            count: 0,
        }
    }

    /// Write the low `n` bits of `value`, `n` at most 32.
    #[inline]
    pub fn put(&mut self, value: u32, n: u32) {
        self.buf |= u64::from(value) << self.count;
        self.count += n;
        while self.count >= 8 {
            self.out.push(self.buf as u8);
            self.buf >>= 8;
            self.count -= 8;
        }
    }

    /// Pad with zero bits to a byte boundary.
    pub fn align(&mut self) {
        if self.count > 0 {
            self.out.push(self.buf as u8);
            self.buf = 0;
            self.count = 0;
        }
    }

    /// Bits written so far, counting the ones still buffered.
    pub fn bit_len(&self) -> u64 {
        self.out.len() as u64 * 8 + u64::from(self.count)
    }

    /// Finish, padding the last byte.
    pub fn finish(mut self) -> Vec<u8> {
        self.align();
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn writer_and_reader_agree() {
        let mut w = BitWriter::new(Vec::new());
        w.put(0b101, 3);
        w.put(0xABCD, 16);
        w.put(1, 1);
        assert_eq!(w.bit_len(), 20);
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        assert_eq!(r.bits(3), Ok(0b101));
        assert_eq!(r.bits(16), Ok(0xABCD));
        assert_eq!(r.bits(1), Ok(1));
        assert_eq!(r.consumed(), 3);
        r.align();
        assert_eq!(r.bits(1), Err(Error::UnexpectedEof));
    }

    #[test]
    fn copy_bytes_drains_the_buffer_first() {
        let data = [0xFFu8, 1, 2, 3, 4];
        let mut r = BitReader::new(&data);
        assert_eq!(r.bits(8), Ok(0xFF));
        let mut out = vec![];
        r.copy_bytes(3, &mut out).expect("enough");
        assert_eq!(out, [1, 2, 3]);
        assert_eq!(r.consumed(), 4);
        assert_eq!(r.copy_bytes(2, &mut out), Err(Error::UnexpectedEof));
    }
}
