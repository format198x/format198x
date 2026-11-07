//! Real `.fds` files write back byte for byte.
//!
//! Point `FORMAT198X_FDS_CORPUS_DIR` at a directory of `.fds` files; its
//! subdirectories are searched too. Every file must parse, every side must
//! start with a disk info block carrying Nintendo's signature, and writing
//! the sides back must reproduce the file exactly.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use format198x_nintendo_famicom_fds::{BlockKind, parse, write};

fn fds_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read corpus") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            fds_files(&path, found);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("fds"))
        {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "FIXTURE: set FORMAT198X_FDS_CORPUS_DIR to a directory of .fds files"]
fn real_files_write_back_unchanged() {
    let Ok(dir) = env::var("FORMAT198X_FDS_CORPUS_DIR") else {
        panic!("set FORMAT198X_FDS_CORPUS_DIR");
    };
    let mut files = Vec::new();
    fds_files(Path::new(&dir), &mut files);

    let mut checked = 0;
    let mut with_header = 0;
    for path in files {
        let bytes = fs::read(&path).expect("read image");
        let image = parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for (index, side) in image.sides.iter().enumerate() {
            let scan = side.scan();
            let info = scan
                .blocks
                .first()
                .filter(|block| block.kind == BlockKind::DiskInfo)
                .and_then(|block| block.disk_info())
                .unwrap_or_else(|| panic!("{} side {index}: no disk info", path.display()));
            assert!(info.signature_ok, "{} side {index}", path.display());
        }
        let sides: Vec<&[u8]> = image.sides.iter().map(|side| side.bytes).collect();
        let written = write(&sides, image.has_header).expect("write");
        assert!(written == bytes, "{} changed on write", path.display());
        checked += 1;
        with_header += usize::from(image.has_header);
    }
    println!("checked={checked} with_header={with_header}");
    assert!(checked > 0, "corpus directory held no .fds files");
}
