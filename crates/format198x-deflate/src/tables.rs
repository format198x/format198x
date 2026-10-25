//! The fixed tables of RFC 1951, section 3.2.5 and 3.2.7.

/// Base match length for length codes 257 to 285.
pub const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];

/// Extra bits for length codes 257 to 285.
pub const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// Base distance for distance codes 0 to 29.
pub const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

/// Extra bits for distance codes 0 to 29.
pub const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// The order in which a dynamic block stores the code-length code lengths.
pub const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// End-of-block symbol.
pub const END_OF_BLOCK: u16 = 256;

/// Literal/length code lengths of the fixed Huffman code.
pub const FIXED_LITLEN: [u8; 288] = {
    let mut lens = [0u8; 288];
    let mut i = 0;
    while i < 288 {
        lens[i] = if i < 144 {
            8
        } else if i < 256 {
            9
        } else if i < 280 {
            7
        } else {
            8
        };
        i += 1;
    }
    lens
};

/// Distance code lengths of the fixed Huffman code. Codes 30 and 31 take
/// part in the code but are invalid in a stream.
pub const FIXED_DIST: [u8; 32] = [5; 32];

/// The length code (0-based, add 257) for a match length of 3 to 258.
pub const fn length_code(len: usize) -> usize {
    let mut code = 28;
    while code > 0 && LENGTH_BASE[code] as usize > len {
        code -= 1;
    }
    code
}

/// The distance code for a distance of 1 to 32768.
pub const fn dist_code(dist: usize) -> usize {
    let mut code = 29;
    while code > 0 && DIST_BASE[code] as usize > dist {
        code -= 1;
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_cover_their_ranges() {
        for len in 3..=258usize {
            let c = length_code(len);
            let base = usize::from(LENGTH_BASE[c]);
            assert!(len >= base && len - base < (1 << LENGTH_EXTRA[c]) || len == 258);
        }
        assert_eq!(length_code(258), 28);
        assert_eq!(length_code(257), 27);
        for dist in 1..=32_768usize {
            let c = dist_code(dist);
            let base = usize::from(DIST_BASE[c]);
            assert!(dist >= base && dist - base < (1 << DIST_EXTRA[c]));
        }
    }
}
