//! Real `.mdr` cartridges write back byte for byte.
//!
//! Point `FORMAT198X_MDR_CORPUS_DIR` at a directory of `.mdr` files. Every
//! file must parse and write back unchanged, and every formatted sector's
//! header checksum must match the one the Interface 1 ROM wrote.

use std::env;
use std::fs;
use std::path::PathBuf;

use format198x_sinclair_zx_spectrum_mdr::MdrImage;

#[test]
#[ignore = "FIXTURE: set FORMAT198X_MDR_CORPUS_DIR to a directory of .mdr files"]
fn real_cartridges_write_back_unchanged() {
    let Ok(dir) = env::var("FORMAT198X_MDR_CORPUS_DIR") else {
        panic!("set FORMAT198X_MDR_CORPUS_DIR");
    };
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("read corpus")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("mdr"))
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "corpus directory held no .mdr files");

    let mut formatted = 0;
    for path in &files {
        let bytes = fs::read(path).expect("read cartridge");
        let image = MdrImage::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            image.to_bytes() == bytes,
            "{} changed on write",
            path.display()
        );
        for index in 0..image.sector_count() {
            let header = image.sector_header(index).expect("in range");
            if header.flag & 1 == 1 && header.number != 0 {
                assert!(
                    header.checksum_ok,
                    "{} sector {index}: header checksum",
                    path.display()
                );
                formatted += 1;
            }
        }
    }
    println!("checked={} formatted_sectors={formatted}", files.len());
    assert!(
        formatted > 0,
        "no formatted sectors to check the checksum against"
    );
}
