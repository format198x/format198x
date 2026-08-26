//! Integration tests for the PP20 decruncher: the magic sniff, truncated and
//! malformed input handling, and a real-media regression against a genuine
//! crunched Amiga module.
//!
//! The real-media test needs a file this repository never commits (the
//! no-media-in-the-repo rule) — see [`decrunches_a_genuine_pp20_module`]
//! below for how to run it.

use format_commodore_amiga_powerpacker::{DecodeError, decrunch, is_powerpacked};

#[test]
fn recognises_the_pp20_magic() {
    assert!(is_powerpacked(b"PP20\x09\x0a\x0c\x0d\x00\x00\x00\x00"));
    assert!(!is_powerpacked(b"FORM\x00\x00\x00\x00ILBM"));
    assert!(!is_powerpacked(b"PP"));
    assert!(!is_powerpacked(b""));
}

#[test]
fn rejects_truncated_input_without_panicking() {
    for len in 0..12usize {
        let bytes = vec![b'P'; len];
        assert!(decrunch(&bytes).is_err(), "length {len} must be rejected");
    }
}

#[test]
fn rejects_missing_magic() {
    let mut bytes = vec![0u8; 16];
    bytes[..4].copy_from_slice(b"FORM");
    assert_eq!(decrunch(&bytes), Err(DecodeError::BadMagic));
}

#[test]
fn rejects_out_of_range_offset_length_bytes() {
    // A header that passes the length and magic checks but claims an
    // offset-length byte of 255 must not turn into an oversized bit read.
    let mut bytes = vec![0u8; 16];
    bytes[..4].copy_from_slice(b"PP20");
    bytes[4] = 255;
    assert!(matches!(decrunch(&bytes), Err(DecodeError::Corrupt { .. })));
}

#[test]
fn rejects_oversized_initial_skip() {
    let mut bytes = vec![0u8; 16];
    bytes[..4].copy_from_slice(b"PP20");
    bytes[4..8].copy_from_slice(&[9, 9, 9, 9]);
    *bytes.last_mut().expect("non-empty") = 33; // skip_bits > 32
    assert!(matches!(decrunch(&bytes), Err(DecodeError::Corrupt { .. })));
}

/// A wide sweep of small, structurally-plausible-but-arbitrary byte strings
/// must never panic, whatever `decrunch` decides to return.
#[test]
fn malformed_input_never_panics() {
    for len in 12..64usize {
        for fill in [0x00u8, 0xFF, 0x55, 0xAA] {
            let mut bytes = vec![fill; len];
            bytes[..4].copy_from_slice(b"PP20");
            let _ = decrunch(&bytes);
        }
    }
}

/// Decrunches a genuine PP20-crunched ProTracker module extracted from the
/// Gathering '92 music disk. Gated behind an environment variable and
/// `#[ignore]` because the disk image is real media and this repository
/// never commits media (reference by path only).
///
/// Run it with:
///
/// ```text
/// FORMAT198X_PP20_ADF_FIXTURE="/path/to/10 Best Tunes ... (Disk 1 of 3).adf" \
///     cargo test -p format-commodore-amiga-powerpacker -- --ignored
/// ```
#[test]
#[ignore = "needs a real Gathering '92 ADF on disk; set FORMAT198X_PP20_ADF_FIXTURE"]
fn decrunches_a_genuine_pp20_module() {
    let Ok(path) = std::env::var("FORMAT198X_PP20_ADF_FIXTURE") else {
        eprintln!("skipping: FORMAT198X_PP20_ADF_FIXTURE not set");
        return;
    };
    let img = std::fs::read(&path).expect("read the ADF fixture");
    let disk = format_commodore_amiga_adf::Disk::open(&img).expect("open the ADF image");

    for name in [
        "Ash-Vixen_Soulside Journey",
        "Gin-Carnage_Party Time",
        "Noteman-Virtual_The art of tun",
    ] {
        let packed = disk.read(name).unwrap_or_else(|_| panic!("read {name}"));
        assert!(
            is_powerpacked(&packed),
            "{name} does not start with the PP20 magic"
        );

        let decrunched = decrunch(&packed).unwrap_or_else(|e| panic!("decrunch {name}: {e}"));

        let magic = decrunched
            .get(1080..1084)
            .unwrap_or_else(|| panic!("{name}: decrunched output shorter than 1084 bytes"));
        let known_magics: [[u8; 4]; 4] = [*b"M.K.", *b"M!K!", *b"FLT4", *b"4CHN"];
        assert!(
            known_magics.iter().any(|m| m == magic),
            "{name}: unexpected magic at offset 1080: {magic:?}"
        );
    }
}
