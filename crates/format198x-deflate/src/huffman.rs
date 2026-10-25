//! Canonical Huffman codes: lookup tables for decoding, and length-limited
//! code construction for encoding.

use crate::Error;
use crate::bits::BitReader;
use alloc::collections::BinaryHeap;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Reverse;

/// The longest code DEFLATE allows.
pub const MAX_BITS: usize = 15;

/// Which code a table decodes, since the rules for incomplete codes differ.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The code-length code of a dynamic block: must be complete.
    CodeLengths,
    /// A literal/length or distance code: may be empty, or a single code of
    /// one bit, as zlib allows.
    Symbols,
}

/// A decoding table indexed by the next `bits` input bits, low bit first.
///
/// Each entry is `length << 9 | symbol`; zero marks a bit pattern that
/// begins no code.
pub struct Decoder {
    table: Vec<u16>,
    bits: u32,
}

impl Decoder {
    /// Build the table for a code given by its code lengths.
    pub fn new(lengths: &[u8], kind: Kind) -> Result<Self, Error> {
        let mut count = [0u16; MAX_BITS + 1];
        for &len in lengths {
            count[usize::from(len)] += 1;
        }
        count[0] = 0;
        let max = (1..=MAX_BITS).rev().find(|&l| count[l] != 0).unwrap_or(0);
        let mut left: i32 = 1;
        for &c in &count[1..] {
            left = (left << 1) - i32::from(c);
            if left < 0 {
                return Err(Error::BadCodeLengths);
            }
        }
        if left > 0 && max != 0 && (kind == Kind::CodeLengths || max != 1) {
            return Err(Error::BadCodeLengths);
        }
        if max == 0 && kind == Kind::CodeLengths {
            return Err(Error::BadCodeLengths);
        }
        let bits = max.max(1);
        let mut table = vec![0u16; 1 << bits];
        let mut next = [0u32; MAX_BITS + 2];
        let mut code = 0u32;
        for len in 1..=MAX_BITS {
            code = (code + u32::from(count[len - 1])) << 1;
            next[len] = code;
        }
        for (symbol, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let len = usize::from(len);
            let reversed = reverse(next[len], len as u32) as usize;
            next[len] += 1;
            let entry = ((len as u16) << 9) | symbol as u16;
            let mut i = reversed;
            while i < table.len() {
                table[i] = entry;
                i += 1 << len;
            }
        }
        Ok(Self {
            table,
            bits: bits as u32,
        })
    }

    /// Decode one symbol.
    #[inline]
    pub fn decode(&self, reader: &mut BitReader<'_>) -> Result<u16, Error> {
        let (buf, count) = reader.peek();
        let mask = (1u64 << self.bits) - 1;
        let entry = self.table[(buf & mask) as usize];
        let len = u32::from(entry >> 9);
        if len == 0 {
            return Err(if count < self.bits {
                Error::UnexpectedEof
            } else {
                Error::InvalidCode
            });
        }
        if len > count {
            return Err(Error::UnexpectedEof);
        }
        reader.consume(len);
        Ok(entry & 0x1FF)
    }
}

/// Reverse the low `len` bits of `code`.
pub fn reverse(code: u32, len: u32) -> u32 {
    if len == 0 {
        0
    } else {
        code.reverse_bits() >> (32 - len)
    }
}

/// Canonical codes for the given lengths, bit-reversed ready for writing.
pub fn codes(lengths: &[u8]) -> Vec<u16> {
    let mut count = [0u32; MAX_BITS + 1];
    for &len in lengths {
        count[usize::from(len)] += 1;
    }
    count[0] = 0;
    let mut next = [0u32; MAX_BITS + 1];
    let mut code = 0u32;
    for len in 1..=MAX_BITS {
        code = (code + count[len - 1]) << 1;
        next[len] = code;
    }
    lengths
        .iter()
        .map(|&len| {
            let len = usize::from(len);
            if len == 0 {
                return 0;
            }
            let c = next[len];
            next[len] += 1;
            reverse(c, len as u32) as u16
        })
        .collect()
}

/// Code lengths for symbol frequencies, none longer than `limit`.
///
/// Unused symbols get length 0. When fewer than two symbols are used, two
/// get length 1, so the code is always complete and every decoder accepts
/// it. If the optimal code is too deep, frequencies are flattened and the
/// code rebuilt until it fits.
pub fn lengths(freqs: &[u32], limit: u8) -> Vec<u8> {
    let mut f: Vec<u32> = freqs.to_vec();
    let used = f.iter().filter(|&&x| x != 0).count();
    if used < 2 {
        let mut lens = vec![0u8; f.len()];
        let first = f.iter().position(|&x| x != 0).unwrap_or(0);
        let second = usize::from(first == 0);
        lens[first] = 1;
        if let Some(l) = lens.get_mut(second) {
            *l = 1;
        }
        return lens;
    }
    loop {
        let lens = unlimited_lengths(&f);
        if lens.iter().all(|&l| l <= limit) {
            return lens;
        }
        for x in &mut f {
            if *x != 0 {
                *x = (*x >> 1).max(1);
            }
        }
    }
}

/// Optimal code lengths, with no limit.
fn unlimited_lengths(freqs: &[u32]) -> Vec<u8> {
    // Nodes 0..n are leaves; later ones are internal, with a parent link.
    let n = freqs.len();
    let mut parent: Vec<usize> = vec![usize::MAX; n];
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = freqs
        .iter()
        .enumerate()
        .filter(|&(_, &f)| f != 0)
        .map(|(i, &f)| Reverse((u64::from(f), i)))
        .collect();
    while heap.len() > 1 {
        let (Some(Reverse((fa, a))), Some(Reverse((fb, b)))) = (heap.pop(), heap.pop()) else {
            break;
        };
        let node = parent.len();
        parent.push(usize::MAX);
        parent[a] = node;
        parent[b] = node;
        heap.push(Reverse((fa + fb, node)));
    }
    // Depth of each node: parents are always created after their children.
    let mut depth = vec![0u32; parent.len()];
    for i in (0..parent.len()).rev() {
        if let Some(&p) = parent.get(i).filter(|&&p| p != usize::MAX) {
            depth[i] = depth[p] + 1;
        }
    }
    (0..n)
        .map(|i| {
            if freqs[i] == 0 {
                0
            } else {
                depth[i].min(255) as u8
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kraft(lens: &[u8]) -> f64 {
        lens.iter()
            .filter(|&&l| l != 0)
            .map(|&l| 1.0 / f64::from(1u32 << l))
            .sum()
    }

    #[test]
    fn lengths_are_complete_and_limited() {
        let freqs: Vec<u32> = (0..288).map(|i| 1 << (i % 24)).collect();
        let lens = lengths(&freqs, 15);
        assert!(lens.iter().all(|&l| (1..=15).contains(&l)));
        assert!((kraft(&lens) - 1.0).abs() < 1e-9);
        let lens = lengths(
            &[5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1000],
            7,
        );
        assert!(lens.iter().all(|&l| l <= 7));
        assert!((kraft(&lens) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn one_or_no_symbol_still_makes_a_complete_code() {
        assert_eq!(lengths(&[0, 0, 7], 15), [1, 0, 1]);
        assert_eq!(lengths(&[0, 0, 0], 15), [1, 1, 0]);
        assert_eq!(lengths(&[3, 0], 15), [1, 1]);
    }

    #[test]
    fn decoder_rules() {
        assert!(Decoder::new(&[1, 1, 1], Kind::Symbols).is_err());
        assert!(Decoder::new(&[1, 2], Kind::Symbols).is_err());
        assert!(Decoder::new(&[1, 0], Kind::Symbols).is_ok());
        assert!(Decoder::new(&[1, 0], Kind::CodeLengths).is_err());
        assert!(Decoder::new(&[0, 0], Kind::Symbols).is_ok());
        assert!(Decoder::new(&[0, 0], Kind::CodeLengths).is_err());
    }
}
