//! Real SNES dumps write back byte for byte, and good ones keep their
//! checksum.
//!
//! Point `FORMAT198X_SNES_CORPUS_DIR` at a directory of `.sfc`, `.smc` or
//! `.bin` dumps; its subdirectories are searched too. Every file must parse
//! and write back unchanged. For every dump whose stored checksum already
//! matches its ROM, [`set_checksum`] must leave the ROM unchanged: the
//! checksums were set by the publishers' tools, not this crate.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use format198x_nintendo_snes_sfc::{compute_checksum, parse, set_checksum};

fn dumps(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read corpus") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            dumps(&path, found);
        } else if path.extension().is_some_and(|ext| {
            ["sfc", "smc", "bin"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        }) {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "FIXTURE: set FORMAT198X_SNES_CORPUS_DIR to a directory of SNES dumps"]
fn real_dumps_write_back_and_keep_their_checksums() {
    let Ok(dir) = env::var("FORMAT198X_SNES_CORPUS_DIR") else {
        panic!("set FORMAT198X_SNES_CORPUS_DIR");
    };
    let mut files = Vec::new();
    dumps(Path::new(&dir), &mut files);

    let mut checked = 0;
    let mut checksum_ok = 0;
    for path in files {
        let bytes = fs::read(&path).expect("read dump");
        let image = parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            image.to_bytes() == bytes,
            "{} changed on write",
            path.display()
        );
        checked += 1;

        if image.header.checksum_pair_valid()
            && compute_checksum(image.rom) == image.header.checksum
        {
            let mut rom = image.rom.to_vec();
            let set = set_checksum(&mut rom, image.header.map).expect("header fits");
            assert_eq!(set, image.header.checksum, "{}", path.display());
            assert!(
                rom == image.rom,
                "{} changed by set_checksum",
                path.display()
            );
            checksum_ok += 1;
        }
    }
    println!("checked={checked} checksum_ok={checksum_ok}");
    assert!(checked > 0, "corpus directory held no SNES dumps");
    assert!(
        checksum_ok > 0,
        "no dump had a matching checksum to check against"
    );
}
