//! Round-trip and reference tests for the TAP codec.
//!
//! The reference fixture is a real tape, not one this crate wrote:
//! `sjasmplus-code.tap` is SjASMPlus 1.21.0's own `SAVETAP … CODE` output for
//! four bytes at `$8000` named `name`. Encoding the same tape has to reproduce
//! it byte for byte, which is a stronger claim than any round trip — a codec
//! can round-trip its own mistakes.

use format198x_sinclair_zx_spectrum_tap::{
    DATA_FLAG, DecodeError, Header, HeaderKind, TapBlock, decode, encode,
};

const REFERENCE: &[u8] = include_bytes!("fixtures/sjasmplus-code.tap");

#[test]
fn encoding_reproduces_a_real_tape() {
    let header = Header::new(HeaderKind::Code, "name", 4, 0x8000, 0x8000);
    let tape = encode(&[header.block(), TapBlock::data(vec![1, 2, 3, 4])]);
    assert_eq!(tape, REFERENCE);
}

#[test]
fn a_real_tape_reads_back_as_what_wrote_it() {
    let blocks = decode(REFERENCE).expect("a real tape decodes");
    assert_eq!(blocks.len(), 2);
    assert!(blocks[0].is_header());
    assert_eq!(blocks[1].flag, DATA_FLAG);
    assert_eq!(blocks[1].data, vec![1, 2, 3, 4]);

    let header = Header::from_payload(&blocks[0].data).expect("a header block holds a header");
    assert_eq!(header.kind, HeaderKind::Code);
    assert_eq!(header.name, "name");
    assert_eq!(header.length, 4);
    assert_eq!(header.param1, 0x8000);
    assert_eq!(header.param2, 0x8000);
}

/// The parity is the XOR of the flag and the payload, so a block of one byte
/// XORs to that byte and a header's covers all seventeen.
#[test]
fn parity_covers_the_flag_and_the_payload() {
    let block = TapBlock::data(vec![0x01, 0x08]);
    assert_eq!(block.parity(), 0xFF ^ 0x01 ^ 0x08);
    assert_eq!(
        encode(&[block]),
        vec![0x04, 0x00, 0xFF, 0x01, 0x08, 0xFF ^ 0x01 ^ 0x08]
    );
}

#[test]
fn encode_then_decode_round_trips() {
    let blocks = vec![
        Header::new(HeaderKind::Program, "loader", 10, 10, 10).block(),
        TapBlock::data((0..10).collect()),
        Header::new(HeaderKind::Code, "toolongtotruncate", 4, 0x8000, 0x8000).block(),
        TapBlock::data(vec![0xDE, 0xAD, 0xBE, 0xEF]),
    ];
    let decoded = decode(&encode(&blocks)).expect("round trip");
    assert_eq!(decoded.len(), blocks.len());
    for (read, built) in decoded.iter().zip(&blocks) {
        assert_eq!((read.flag, &read.data), (built.flag, &built.data));
        // A decoded block carries the parity it was read with; a built one
        // leaves it to `encode`. Both agree on what is on tape.
        assert_eq!(read.stored_parity, Some(built.parity()));
        assert_eq!(read.tape_bytes(), built.tape_bytes());
        assert!(read.parity_ok());
    }
}

/// A real tape decodes and encodes back to the same bytes.
#[test]
fn a_real_tape_survives_decode_and_encode() {
    let blocks = decode(REFERENCE).expect("a real tape decodes");
    assert!(blocks.iter().all(TapBlock::parity_ok));
    assert_eq!(encode(&blocks), REFERENCE);
}

/// A block whose payload no longer matches its parity is what a damaged tape
/// is. The ROM refuses it with "R Tape loading error", so decoding must not
/// quietly repair it: the stored parity survives decode and encode, and
/// `tape_bytes` gives a player the block exactly as damaged.
#[test]
fn a_damaged_block_stays_damaged() {
    let mut damaged = REFERENCE.to_vec();
    let last_payload_byte = damaged.len() - 2;
    damaged[last_payload_byte] ^= 0x01;

    let blocks = decode(&damaged).expect("a damaged block still decodes");
    assert!(blocks[0].parity_ok(), "the header block is untouched");
    assert!(!blocks[1].parity_ok());
    assert_eq!(
        blocks[1].stored_parity,
        Some(REFERENCE[REFERENCE.len() - 1])
    );
    assert_ne!(blocks[1].written_parity(), blocks[1].parity());
    assert_eq!(encode(&blocks), damaged);

    let on_tape = blocks[1].tape_bytes();
    assert_eq!(
        on_tape.as_slice(),
        &damaged[damaged.len() - on_tape.len()..]
    );
}

/// A block built rather than read has no stored parity, so `encode` writes the
/// correct one, and `stored_parity` set by hand overrides it.
#[test]
fn a_built_block_gets_the_correct_parity_unless_told_otherwise() {
    let mut block = TapBlock::new(DATA_FLAG, vec![1, 2, 3, 4]);
    assert_eq!(block.stored_parity, None);
    assert!(block.parity_ok());
    assert_eq!(encode(&[block.clone()]).last(), Some(&block.parity()));

    block.stored_parity = Some(block.parity() ^ 0xFF);
    assert!(!block.parity_ok());
    assert_eq!(
        encode(&[block.clone()]).last(),
        Some(&(block.parity() ^ 0xFF))
    );
}

/// Ten bytes is what the field holds: a shorter name is space-padded, a longer
/// one is cut to fit, and reading either back trims the padding.
#[test]
fn a_name_is_ten_bytes_however_long_it_was() {
    let short = Header::new(HeaderKind::Code, "ab", 0, 0, 0).block();
    assert_eq!(&short.data[1..11], b"ab        ");
    let long = Header::new(HeaderKind::Code, "abcdefghijklmno", 0, 0, 0).block();
    assert_eq!(&long.data[1..11], b"abcdefghij");
    assert_eq!(
        Header::from_payload(&short.data).expect("header").name,
        "ab"
    );
}

#[test]
fn a_truncated_block_is_refused() {
    let err = decode(&[0x05, 0x00, 0xFF, 0x01, 0x02]).expect_err("truncated");
    assert_eq!(
        err,
        DecodeError::Truncated {
            at: 0,
            claimed: 5,
            available: 3
        }
    );
}

#[test]
fn a_stray_byte_after_the_last_block_is_refused() {
    let mut tape = encode(&[TapBlock::data(vec![1])]);
    tape.push(0x00);
    assert!(matches!(
        decode(&tape).expect_err("odd byte"),
        DecodeError::TrailingByte { .. }
    ));
}

#[test]
fn a_header_payload_has_to_be_a_header() {
    assert!(matches!(
        Header::from_payload(&[0; 3]).expect_err("too short"),
        DecodeError::HeaderLength { found: 3 }
    ));
    let mut payload = [0u8; 17];
    payload[0] = 9;
    assert!(matches!(
        Header::from_payload(&payload).expect_err("no such kind"),
        DecodeError::HeaderKind { found: 9 }
    ));
}
