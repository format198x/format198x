//! Raw DEFLATE encoding (RFC 1951): LZ77 over hash chains, then the
//! shortest of stored, fixed-Huffman and dynamic-Huffman coding per block.

use crate::Level;
use crate::bits::BitWriter;
use crate::huffman::{codes, lengths};
use crate::tables::{
    CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, END_OF_BLOCK, FIXED_DIST, FIXED_LITLEN, LENGTH_BASE,
    LENGTH_EXTRA, dist_code, length_code,
};
use alloc::vec;
use alloc::vec::Vec;

/// The LZ77 window.
const WINDOW: usize = 32_768;
const WINDOW_MASK: usize = WINDOW - 1;
const HASH_BITS: u32 = 15;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
/// A three-byte match further back than this costs more than its literals.
const TOO_FAR: usize = 4096;
/// Tokens per block before the encoder starts a new one.
const BLOCK_TOKENS: usize = 16_384;
/// The most bytes a stored block can hold.
const STORED_MAX: usize = 65_535;

/// Per-level search effort: chain length, "nice" length, lazy matching.
const PARAMS: [(usize, usize, bool); 10] = [
    (0, 0, false),
    (4, 8, false),
    (8, 16, false),
    (16, 32, false),
    (16, 16, true),
    (32, 32, true),
    (128, 128, true),
    (256, 258, true),
    (1024, 258, true),
    (4096, 258, true),
];

/// Length code (0-based) for each match length 0 to 258.
const LEN_CODE: [u8; 259] = {
    let mut t = [0u8; 259];
    let mut len = MIN_MATCH;
    while len <= MAX_MATCH {
        t[len] = length_code(len) as u8;
        len += 1;
    }
    t
};

/// Distance codes for distances 1 to 256, by `distance - 1`.
const DIST_LO: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = dist_code(i + 1) as u8;
        i += 1;
    }
    t
};

/// Distance codes for distances 257 to 32768, by `(distance - 1) >> 7`.
const DIST_HI: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 2;
    while i < 256 {
        t[i] = dist_code((i << 7) + 1) as u8;
        i += 1;
    }
    t
};

fn dist_symbol(dist: usize) -> usize {
    if dist <= 256 {
        usize::from(DIST_LO[dist - 1])
    } else {
        usize::from(DIST_HI[(dist - 1) >> 7])
    }
}

/// A literal (`len == 0`, byte in `dist`) or a match.
#[derive(Clone, Copy)]
struct Token {
    len: u16,
    dist: u16,
}

/// Compress `data` as a raw DEFLATE stream.
///
/// ```
/// use format198x_deflate::{Level, deflate, inflate};
///
/// let data = b"a long, long, long, long, long line";
/// let packed = deflate(data, Level::BEST);
/// assert!(packed.len() < data.len());
/// assert_eq!(inflate(&packed)?, data);
/// # Ok::<(), format198x_deflate::Error>(())
/// ```
#[must_use]
pub fn deflate(data: &[u8], level: Level) -> Vec<u8> {
    let mut w = BitWriter::new(Vec::with_capacity(data.len() / 2 + 64));
    let (max_chain, nice, lazy) = PARAMS[usize::from(level.get())];
    if level.get() == 0 {
        write_stored(&mut w, data, true);
        return w.finish();
    }
    if data.is_empty() {
        // A final fixed-Huffman block holding only end-of-block.
        w.put(1, 1);
        w.put(1, 2);
        w.put(0, 7);
        return w.finish();
    }
    let mut m = Matcher::new(data, max_chain, nice);
    let mut tokens: Vec<Token> = Vec::with_capacity(BLOCK_TOKENS + 1);
    let mut block_start = 0;
    let mut pos = 0;
    let n = data.len();
    while pos < n {
        let (len, dist) = m.find(pos);
        m.insert(pos);
        if len >= MIN_MATCH {
            if lazy && len < nice && pos + 1 < n {
                let (next_len, _) = m.find(pos + 1);
                if next_len > len {
                    tokens.push(Token {
                        len: 0,
                        dist: u16::from(data[pos]),
                    });
                    pos += 1;
                    if tokens.len() >= BLOCK_TOKENS {
                        emit_block(&mut w, &tokens, &data[block_start..pos], false);
                        tokens.clear();
                        block_start = pos;
                    }
                    continue;
                }
            }
            tokens.push(Token {
                len: len as u16,
                dist: dist as u16,
            });
            for p in pos + 1..pos + len {
                m.insert(p);
            }
            pos += len;
        } else {
            tokens.push(Token {
                len: 0,
                dist: u16::from(data[pos]),
            });
            pos += 1;
        }
        if tokens.len() >= BLOCK_TOKENS && pos < n {
            emit_block(&mut w, &tokens, &data[block_start..pos], false);
            tokens.clear();
            block_start = pos;
        }
    }
    emit_block(&mut w, &tokens, &data[block_start..], true);
    w.finish()
}

/// Hash chains over the whole input.
struct Matcher<'a> {
    data: &'a [u8],
    /// Most recent position + 1 for each hash; 0 for none.
    head: Vec<usize>,
    /// Previous position + 1 with the same hash, by position in the window.
    prev: Vec<usize>,
    max_chain: usize,
    nice: usize,
}

impl<'a> Matcher<'a> {
    fn new(data: &'a [u8], max_chain: usize, nice: usize) -> Self {
        Self {
            data,
            head: vec![0; 1 << HASH_BITS],
            prev: vec![0; WINDOW],
            max_chain,
            nice,
        }
    }

    fn hash(&self, pos: usize) -> usize {
        let d = self.data;
        let v = u32::from(d[pos]) | u32::from(d[pos + 1]) << 8 | u32::from(d[pos + 2]) << 16;
        (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    }

    fn insert(&mut self, pos: usize) {
        if pos + MIN_MATCH <= self.data.len() {
            let h = self.hash(pos);
            self.prev[pos & WINDOW_MASK] = self.head[h];
            self.head[h] = pos + 1;
        }
    }

    /// The longest earlier match for `pos`, as `(length, distance)`, or
    /// length 0 when there is none worth taking.
    fn find(&self, pos: usize) -> (usize, usize) {
        let d = self.data;
        if pos + MIN_MATCH > d.len() {
            return (0, 0);
        }
        let max_len = (d.len() - pos).min(MAX_MATCH);
        let mut best_len = MIN_MATCH - 1;
        let mut best_dist = 0;
        let mut cand = self.head[self.hash(pos)];
        let mut chain = self.max_chain;
        while cand != 0 && chain > 0 {
            let c = cand - 1;
            let dist = pos - c;
            if dist > WINDOW {
                break;
            }
            if d[c + best_len] == d[pos + best_len] {
                let len = match_len(d, c, pos, max_len);
                if len > best_len {
                    best_len = len;
                    best_dist = dist;
                    if len >= self.nice || len == max_len {
                        break;
                    }
                }
            }
            let next = self.prev[c & WINDOW_MASK];
            if next == 0 || next > c {
                break;
            }
            cand = next;
            chain -= 1;
        }
        if best_len < MIN_MATCH || (best_len == MIN_MATCH && best_dist > TOO_FAR) {
            (0, 0)
        } else {
            (best_len, best_dist)
        }
    }
}

/// How many bytes at `a` and `b` agree, up to `max`; `a < b`.
fn match_len(d: &[u8], a: usize, b: usize, max: usize) -> usize {
    let mut len = 0;
    while len + 8 <= max {
        let mut x = [0u8; 8];
        let mut y = [0u8; 8];
        x.copy_from_slice(&d[a + len..a + len + 8]);
        y.copy_from_slice(&d[b + len..b + len + 8]);
        let diff = u64::from_le_bytes(x) ^ u64::from_le_bytes(y);
        if diff != 0 {
            return len + (diff.trailing_zeros() / 8) as usize;
        }
        len += 8;
    }
    while len < max && d[a + len] == d[b + len] {
        len += 1;
    }
    len
}

/// Write `raw` as stored blocks, the last one final if `last`.
fn write_stored(w: &mut BitWriter, raw: &[u8], last: bool) {
    let mut chunks = raw.chunks(STORED_MAX).peekable();
    if raw.is_empty() {
        w.put(u32::from(last), 1);
        w.put(0, 2);
        w.align();
        w.put(0, 16);
        w.put(0xFFFF, 16);
        return;
    }
    while let Some(chunk) = chunks.next() {
        let final_chunk = last && chunks.peek().is_none();
        w.put(u32::from(final_chunk), 1);
        w.put(0, 2);
        w.align();
        let len = chunk.len() as u32;
        w.put(len, 16);
        w.put(!len & 0xFFFF, 16);
        w.out.extend_from_slice(chunk);
    }
}

/// Bits that stored blocks for `len` bytes would take from bit position
/// `at`.
fn stored_cost(at: u64, len: usize) -> u64 {
    let chunks = len.div_ceil(STORED_MAX).max(1) as u64;
    let first_pad = (8 - (at + 3) % 8) % 8;
    let later_pad = 5 * (chunks - 1);
    chunks * (3 + 32) + first_pad + later_pad + 8 * len as u64
}

/// Run-length code a sequence of code lengths with symbols 16, 17 and 18.
fn rle(lens: &[u8]) -> Vec<(u8, u8)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lens.len() {
        let l = lens[i];
        let mut run = 1;
        while i + run < lens.len() && lens[i + run] == l {
            run += 1;
        }
        let mut r = run;
        if l == 0 {
            while r >= 11 {
                let k = r.min(138);
                out.push((18, (k - 11) as u8));
                r -= k;
            }
            if r >= 3 {
                out.push((17, (r - 3) as u8));
                r = 0;
            }
        } else {
            out.push((l, 0));
            r -= 1;
            while r >= 3 {
                let k = r.min(6);
                out.push((16, (k - 3) as u8));
                r -= k;
            }
        }
        for _ in 0..r {
            out.push((l, 0));
        }
        i += run;
    }
    out
}

/// A dynamic block's header, ready to cost and write.
struct DynamicHeader {
    nlit: usize,
    ndist: usize,
    ncode: usize,
    cl_lens: Vec<u8>,
    cl_codes: Vec<u16>,
    rle: Vec<(u8, u8)>,
}

impl DynamicHeader {
    fn new(lit_lens: &[u8], dist_lens: &[u8]) -> Self {
        let nlit = 257.max(lit_lens.iter().rposition(|&l| l != 0).map_or(0, |p| p + 1));
        let ndist = 1.max(dist_lens.iter().rposition(|&l| l != 0).map_or(0, |p| p + 1));
        let mut all = lit_lens[..nlit].to_vec();
        all.extend_from_slice(&dist_lens[..ndist]);
        let rle = rle(&all);
        let mut freqs = [0u32; 19];
        for &(sym, _) in &rle {
            freqs[usize::from(sym)] += 1;
        }
        let cl_lens = lengths(&freqs, 7);
        let cl_codes = codes(&cl_lens);
        let ncode = 4.max(
            CODE_LENGTH_ORDER
                .iter()
                .rposition(|&i| cl_lens[i] != 0)
                .map_or(0, |p| p + 1),
        );
        Self {
            nlit,
            ndist,
            ncode,
            cl_lens,
            cl_codes,
            rle,
        }
    }

    fn cost(&self) -> u64 {
        let mut bits = 5 + 5 + 4 + 3 * self.ncode as u64;
        for &(sym, _) in &self.rle {
            bits += u64::from(self.cl_lens[usize::from(sym)]);
            bits += match sym {
                16 => 2,
                17 => 3,
                18 => 7,
                _ => 0,
            };
        }
        bits
    }

    fn write(&self, w: &mut BitWriter) {
        w.put((self.nlit - 257) as u32, 5);
        w.put((self.ndist - 1) as u32, 5);
        w.put((self.ncode - 4) as u32, 4);
        for &i in &CODE_LENGTH_ORDER[..self.ncode] {
            w.put(u32::from(self.cl_lens[i]), 3);
        }
        for &(sym, extra) in &self.rle {
            let s = usize::from(sym);
            w.put(u32::from(self.cl_codes[s]), u32::from(self.cl_lens[s]));
            match sym {
                16 => w.put(u32::from(extra), 2),
                17 => w.put(u32::from(extra), 3),
                18 => w.put(u32::from(extra), 7),
                _ => {}
            }
        }
    }
}

/// Bits the tokens take under the given code lengths, end-of-block included.
fn token_cost(tokens: &[Token], lit_lens: &[u8], dist_lens: &[u8]) -> u64 {
    let mut bits = u64::from(lit_lens[usize::from(END_OF_BLOCK)]);
    for t in tokens {
        if t.len == 0 {
            bits += u64::from(lit_lens[usize::from(t.dist)]);
        } else {
            let lc = usize::from(LEN_CODE[usize::from(t.len)]);
            let dc = dist_symbol(usize::from(t.dist));
            bits += u64::from(lit_lens[257 + lc]) + u64::from(LENGTH_EXTRA[lc]);
            bits += u64::from(dist_lens[dc]) + u64::from(DIST_EXTRA[dc]);
        }
    }
    bits
}

fn write_tokens(
    w: &mut BitWriter,
    tokens: &[Token],
    lit_lens: &[u8],
    dist_lens: &[u8],
    lit_codes: &[u16],
    dist_codes: &[u16],
) {
    for t in tokens {
        if t.len == 0 {
            let s = usize::from(t.dist);
            w.put(u32::from(lit_codes[s]), u32::from(lit_lens[s]));
        } else {
            let len = usize::from(t.len);
            let dist = usize::from(t.dist);
            let lc = usize::from(LEN_CODE[len]);
            let s = 257 + lc;
            w.put(u32::from(lit_codes[s]), u32::from(lit_lens[s]));
            w.put(
                (len - usize::from(LENGTH_BASE[lc])) as u32,
                u32::from(LENGTH_EXTRA[lc]),
            );
            let dc = dist_symbol(dist);
            w.put(u32::from(dist_codes[dc]), u32::from(dist_lens[dc]));
            w.put(
                (dist - usize::from(DIST_BASE[dc])) as u32,
                u32::from(DIST_EXTRA[dc]),
            );
        }
    }
    let eob = usize::from(END_OF_BLOCK);
    w.put(u32::from(lit_codes[eob]), u32::from(lit_lens[eob]));
}

/// Write one block of tokens covering `raw`, in its cheapest coding.
fn emit_block(w: &mut BitWriter, tokens: &[Token], raw: &[u8], last: bool) {
    let mut lit_freq = [0u32; 286];
    let mut dist_freq = [0u32; 30];
    lit_freq[usize::from(END_OF_BLOCK)] = 1;
    for t in tokens {
        if t.len == 0 {
            lit_freq[usize::from(t.dist)] += 1;
        } else {
            lit_freq[257 + usize::from(LEN_CODE[usize::from(t.len)])] += 1;
            dist_freq[dist_symbol(usize::from(t.dist))] += 1;
        }
    }
    let lit_lens = lengths(&lit_freq, 15);
    let dist_lens = lengths(&dist_freq, 15);
    let header = DynamicHeader::new(&lit_lens, &dist_lens);

    let fixed = 3 + token_cost(tokens, &FIXED_LITLEN, &FIXED_DIST);
    let dynamic = 3 + header.cost() + token_cost(tokens, &lit_lens, &dist_lens);
    let stored = stored_cost(w.bit_len(), raw.len());

    if stored < fixed && stored < dynamic {
        write_stored(w, raw, last);
    } else if fixed <= dynamic {
        w.put(u32::from(last), 1);
        w.put(1, 2);
        let lit_codes = codes(&FIXED_LITLEN);
        let dist_codes = codes(&FIXED_DIST);
        write_tokens(
            w,
            tokens,
            &FIXED_LITLEN,
            &FIXED_DIST,
            &lit_codes,
            &dist_codes,
        );
    } else {
        w.put(u32::from(last), 1);
        w.put(2, 2);
        header.write(w);
        let lit_codes = codes(&lit_lens);
        let dist_codes = codes(&dist_lens);
        write_tokens(w, tokens, &lit_lens, &dist_lens, &lit_codes, &dist_codes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflate;

    #[test]
    fn distance_symbols_match_the_table() {
        for dist in 1..=WINDOW {
            assert_eq!(dist_symbol(dist), dist_code(dist), "distance {dist}");
        }
        for (len, &code) in LEN_CODE.iter().enumerate().skip(MIN_MATCH) {
            assert_eq!(usize::from(code), length_code(len));
        }
    }

    #[test]
    fn rle_uses_every_repeat_code() {
        let mut lens = vec![8u8; 10];
        lens.extend_from_slice(&[0; 150]);
        lens.extend_from_slice(&[0; 5]);
        lens.extend_from_slice(&[7, 7]);
        lens.extend_from_slice(&[0; 4]);
        lens.push(1);
        let coded = rle(&lens);
        let syms: Vec<u8> = coded.iter().map(|&(s, _)| s).collect();
        assert!(syms.contains(&16) && syms.contains(&17) && syms.contains(&18));
        let mut back = Vec::new();
        for &(s, e) in &coded {
            match s {
                16 => {
                    let p = *back.last().expect("previous");
                    back.extend(core::iter::repeat_n(p, 3 + usize::from(e)));
                }
                17 => back.extend(core::iter::repeat_n(0, 3 + usize::from(e))),
                18 => back.extend(core::iter::repeat_n(0, 11 + usize::from(e))),
                l => back.push(l),
            }
        }
        assert_eq!(back, lens);
    }

    #[test]
    fn every_level_round_trips() {
        let mut data = Vec::new();
        for i in 0..40_000u32 {
            data.push((i * i % 251) as u8);
            if i % 7 == 0 {
                data.extend_from_slice(b"repeat me ");
            }
        }
        for level in 0..=9 {
            let packed = deflate(&data, Level::new(level));
            assert_eq!(inflate(&packed).as_deref(), Ok(&data[..]), "level {level}");
        }
    }

    #[test]
    fn stored_blocks_split_at_64k() {
        let data = vec![0x5Au8; 140_000];
        let packed = deflate(&data, Level::NONE);
        assert_eq!(packed.len(), data.len() + 3 * 5);
        assert_eq!(inflate(&packed), Ok(data));
        assert_eq!(deflate(&[], Level::NONE), [1, 0, 0, 0xFF, 0xFF]);
        assert_eq!(deflate(&[], Level::DEFAULT), [3, 0]);
    }

    #[test]
    fn incompressible_data_falls_back_to_stored() {
        let mut x: u32 = 0x1234_5678;
        let data: Vec<u8> = (0..50_000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        let packed = deflate(&data, Level::BEST);
        assert!(packed.len() <= data.len() + 5 * 4);
        assert_eq!(inflate(&packed), Ok(data));
    }
}
