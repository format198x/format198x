//! Garbage in, an error out: no input makes the decoders panic, run long
//! or allocate without bound.
//!
//! The inputs come from a fixed-seed generator, so a failure reproduces.

use format198x_deflate::{Level, Options, deflate, gzip, inflate_with, zlib};

/// xorshift64*: small, fast and deterministic.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }
}

const LIMIT: usize = 1 << 20;

fn decode_all(data: &[u8]) {
    let opts = Options::default().max_output(LIMIT);
    for out in [
        inflate_with(data, &opts),
        zlib::decode_with(data, &opts),
        gzip::decode_with(data, &opts).map(|m| m.data),
    ]
    .into_iter()
    .flatten()
    {
        assert!(out.len() <= LIMIT);
    }
}

#[test]
fn random_bytes_never_panic() {
    let mut rng = Rng(0x0DEF_1A7E);
    for _ in 0..20_000 {
        let n = rng.below(300);
        let data = rng.bytes(n);
        decode_all(&data);
    }
}

#[test]
fn random_bytes_behind_valid_headers_never_panic() {
    let mut rng = Rng(0x5EED);
    for _ in 0..10_000 {
        let n = rng.below(200);
        let mut z = vec![0x78, 0x9C];
        z.extend(rng.bytes(n));
        decode_all(&z);
        let mut g = vec![0x1F, 0x8B, 8, (rng.next() as u8) & 0x1F, 0, 0, 0, 0, 0, 3];
        g.extend(rng.bytes(n));
        decode_all(&g);
        // A dynamic-block header with random tables behind it.
        let mut d = vec![0x05 | ((rng.next() as u8) & 0xF8)];
        d.extend(rng.bytes(n));
        decode_all(&d);
    }
}

#[test]
fn mutated_valid_streams_never_panic() {
    let mut rng = Rng(0xF00D);
    let mut source = Vec::new();
    for i in 0..6000u32 {
        source.push((i % 37) as u8 ^ (i / 300) as u8);
    }
    for level in [1, 6, 9] {
        let packed = zlib::encode(&source, Level::new(level));
        let gz =
            gzip::encode(&gzip::Header::default(), &source, Level::new(level)).expect("encodes");
        for base in [&packed, &gz] {
            for _ in 0..3000 {
                let mut data = base.clone();
                for _ in 0..1 + rng.below(4) {
                    let at = rng.below(data.len());
                    data[at] ^= 1 << rng.below(8);
                }
                if rng.below(4) == 0 {
                    let cut = rng.below(data.len());
                    data.truncate(cut);
                }
                decode_all(&data);
            }
        }
    }
}

#[test]
fn random_data_round_trips_at_every_level() {
    let mut rng = Rng(0xC0FFEE);
    for round in 0..60 {
        let n = rng.below(5000);
        // Mix noise with runs and repeats so every token kind appears.
        let mut data = Vec::with_capacity(n);
        while data.len() < n {
            match rng.below(3) {
                0 => {
                    let k = 1 + rng.below(20);
                    data.extend(rng.bytes(k));
                }
                1 => {
                    let b = rng.next() as u8;
                    data.extend(std::iter::repeat_n(b, 1 + rng.below(300)));
                }
                _ if !data.is_empty() => {
                    let start = rng.below(data.len());
                    let len = 1 + rng.below(data.len() - start);
                    data.extend_from_within(start..start + len);
                }
                _ => {}
            }
        }
        let level = Level::new((round % 10) as u8);
        let packed = deflate(&data, level);
        let opts = Options::default();
        assert_eq!(inflate_with(&packed, &opts).expect("round trip"), data);
        assert_eq!(
            zlib::decode(&zlib::encode(&data, level)).expect("zlib"),
            data
        );
    }
}

#[test]
fn deflate_bombs_respect_the_limit() {
    let bomb = zlib::encode(&vec![0u8; 4 << 20], Level::BEST);
    assert!(bomb.len() < 8 << 10);
    let opts = Options::default().max_output(LIMIT);
    assert!(zlib::decode_with(&bomb, &opts).is_err());
}
