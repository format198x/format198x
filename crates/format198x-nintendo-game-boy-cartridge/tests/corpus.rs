//! Real Game Boy dumps parse, and good ones keep their checksums.
//!
//! Point `FORMAT198X_GB_CORPUS_DIR` at a directory of `.gb` or `.gbc` dumps;
//! its subdirectories are searched too. Every file must parse. For every
//! dump whose stored global checksum already matches, [`set_checksums`] must
//! leave the ROM unchanged: those checksums were written by the publishers'
//! tools, not this crate.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use format198x_nintendo_game_boy_cartridge::{CartridgeHeader, global_checksum, set_checksums};

fn dumps(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read corpus") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            dumps(&path, found);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gb") || ext.eq_ignore_ascii_case("gbc"))
        {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "FIXTURE: set FORMAT198X_GB_CORPUS_DIR to a directory of Game Boy dumps"]
fn real_dumps_parse_and_keep_their_checksums() {
    let Ok(dir) = env::var("FORMAT198X_GB_CORPUS_DIR") else {
        panic!("set FORMAT198X_GB_CORPUS_DIR");
    };
    let mut files = Vec::new();
    dumps(Path::new(&dir), &mut files);

    let mut checked = 0;
    let mut global_ok = 0;
    for path in files {
        let rom = fs::read(&path).expect("read dump");
        let header =
            CartridgeHeader::parse(&rom).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        checked += 1;
        if global_checksum(&rom) == header.global_checksum {
            let mut copy = rom.clone();
            set_checksums(&mut copy).expect("long enough");
            assert!(copy == rom, "{} changed by set_checksums", path.display());
            global_ok += 1;
        }
    }
    println!("checked={checked} global_ok={global_ok}");
    assert!(checked > 0, "corpus directory held no Game Boy dumps");
    assert!(
        global_ok > 0,
        "no dump had a matching global checksum to check against"
    );
}
