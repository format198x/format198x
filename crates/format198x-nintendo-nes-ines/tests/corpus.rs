//! Real NES files write back byte for byte.
//!
//! Point `FORMAT198X_NES_CORPUS_DIR` at a directory of `.nes` files; its
//! subdirectories are searched too. Every file that starts with the iNES
//! magic must parse, and writing the parsed image must reproduce it exactly.
//! The files were written by other tools, so this checks the reader and
//! writer against layouts this crate did not produce.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use format198x_nintendo_nes_ines::{MAGIC, parse};

fn nes_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read corpus") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            nes_files(&path, found);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("nes"))
        {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "FIXTURE: set FORMAT198X_NES_CORPUS_DIR to a directory of .nes files"]
fn real_files_write_back_unchanged() {
    let Ok(dir) = env::var("FORMAT198X_NES_CORPUS_DIR") else {
        panic!("set FORMAT198X_NES_CORPUS_DIR");
    };
    let mut files = Vec::new();
    nes_files(Path::new(&dir), &mut files);

    let mut checked = 0;
    let mut nes_2_0 = 0;
    let mut not_ines = 0;
    for path in files {
        let bytes = fs::read(&path).expect("read image");
        if !bytes.starts_with(&MAGIC) {
            not_ines += 1;
            continue;
        }
        let image = parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let written = image
            .to_bytes()
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(written == bytes, "{} changed on write", path.display());
        checked += 1;
        nes_2_0 += usize::from(image.header.is_nes_2_0);
    }
    println!("checked={checked} nes_2_0={nes_2_0} not_ines={not_ines}");
    assert!(checked > 0, "corpus directory held no iNES files");
}
