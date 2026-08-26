//! Parse and round-trip tests against a module built in code rather than
//! shipped as a file — no media may enter the repository.

use format_commodore_amiga_mod::{decode, encode};

/// One looped square-wave sample, one pattern, a C-2 on channel 0 at row 0.
fn synthetic_module() -> Vec<u8> {
    let sample: Vec<u8> = (0..64)
        .map(|i| if i < 32 { 100u8 } else { 156u8 })
        .collect();
    let mut out = b"SYNTH".to_vec();
    out.resize(20, 0);
    for i in 0..31 {
        let mut hdr = vec![0u8; 30];
        if i == 0 {
            hdr[..6].copy_from_slice(b"square");
            hdr[22..24].copy_from_slice(&((sample.len() / 2) as u16).to_be_bytes());
            hdr[25] = 64; // volume
            hdr[28..30].copy_from_slice(&((sample.len() / 2) as u16).to_be_bytes());
        }
        out.extend_from_slice(&hdr);
    }
    out.push(1); // song length
    out.push(0); // restart
    out.extend_from_slice(&[0u8; 128]); // order table
    out.extend_from_slice(b"M.K.");
    let mut pattern = vec![0u8; 64 * 4 * 4];
    let (period, smp) = (428u16, 1u8); // C-2, sample 1
    pattern[0] = (smp & 0xF0) | (period >> 8) as u8;
    pattern[1] = (period & 0xFF) as u8;
    pattern[2] = (smp & 0x0F) << 4;
    out.extend_from_slice(&pattern);
    out.extend_from_slice(&sample);
    out
}

#[test]
fn parses_a_synthetic_module() {
    let m = decode(&synthetic_module()).expect("decodes");
    assert_eq!(m.title(), "SYNTH");
    assert_eq!(m.orders().len(), 1);
    assert_eq!(m.patterns.len(), 1);
    assert_eq!(m.patterns[0].len(), 64);
    assert_eq!(m.patterns[0][0][0].period, 428);
    assert_eq!(m.patterns[0][0][0].sample, 1);
    let used: Vec<_> = m.samples.iter().filter(|s| !s.data.is_empty()).collect();
    assert_eq!(used.len(), 1);
    assert_eq!(used[0].name(), "square");
    assert_eq!(used[0].volume, 64);
    assert_eq!(used[0].data.len(), 64);
}

#[test]
fn round_trips_byte_for_byte() {
    let original = synthetic_module();
    let decoded = decode(&original).expect("decodes");
    let reencoded = encode(&decoded).expect("re-encodes");
    assert_eq!(reencoded, original, "round-trip changed bytes");
}

#[test]
fn malformed_input_never_panics() {
    for len in [0usize, 1, 20, 1079, 1083, 1084] {
        assert!(
            decode(&vec![0u8; len]).is_err(),
            "length {len} must be rejected"
        );
    }
}
