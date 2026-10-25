//! Adler-32 (RFC 1950) and CRC-32 (RFC 1952, ISO 3309).

/// The largest prime below 2^16.
const ADLER_MOD: u32 = 65_521;
/// The most bytes that can be summed before `b` could overflow a `u32`.
const ADLER_NMAX: usize = 5_552;

/// The Adler-32 checksum of `data`, as zlib's trailer stores it.
///
/// ```
/// assert_eq!(format198x_deflate::adler32(b"Wikipedia"), 0x11E6_0398);
/// ```
#[must_use]
pub fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for chunk in data.chunks(ADLER_NMAX) {
        for &byte in chunk {
            a += u32::from(byte);
            b += a;
        }
        a %= ADLER_MOD;
        b %= ADLER_MOD;
    }
    (b << 16) | a
}

/// The reflected CRC-32 table for polynomial `0xEDB88320`.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

/// The CRC-32 of `data`, as gzip's trailer stores it.
///
/// ```
/// assert_eq!(format198x_deflate::crc32(b"123456789"), 0xCBF4_3926);
/// ```
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc = CRC_TABLE[usize::from((crc as u8) ^ byte)] ^ (crc >> 8);
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn known_values() {
        assert_eq!(adler32(b""), 1);
        assert_eq!(crc32(b""), 0);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }

    #[test]
    fn adler_survives_long_runs_of_ff() {
        // Long runs of 0xFF are the worst case for overflow between
        // reductions; zlib's value for 100,000 of them.
        let data = vec![0xFFu8; 100_000];
        let mut a: u64 = 1;
        let mut b: u64 = 0;
        for &x in &data {
            a = (a + u64::from(x)) % 65_521;
            b = (b + a) % 65_521;
        }
        assert_eq!(adler32(&data), ((b << 16) | a) as u32);
    }
}
