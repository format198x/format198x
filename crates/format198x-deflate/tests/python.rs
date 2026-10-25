//! Our encoder's output must inflate with the reference zlib.
//!
//! This test needs `python3`, whose `zlib` module wraps the reference
//! library, so it is ignored by default:
//!
//! ```sh
//! cargo test -p format198x-deflate --test python -- --ignored
//! ```

use format198x_deflate::{Level, deflate, gzip, zlib};
use std::path::PathBuf;
use std::process::Command;

const CHECK: &str = r#"
import gzip, sys, zlib, pathlib
d = pathlib.Path(sys.argv[1])
bad = 0
for plain in sorted(d.glob("*.plain")):
    want = plain.read_bytes()
    stem = plain.name[:-6]
    for f in sorted(d.glob(stem + ".*")):
        kind = f.suffix
        if kind == ".plain":
            continue
        data = f.read_bytes()
        if kind == ".deflate":
            got = zlib.decompress(data, -15)
        elif kind == ".zlib":
            got = zlib.decompress(data)
        else:
            got = gzip.decompress(data)
        if got != want:
            print("MISMATCH", f.name)
            bad += 1
print("checked", len(list(d.iterdir())))
sys.exit(1 if bad else 0)
"#;

fn inputs() -> Vec<(String, Vec<u8>)> {
    let vectors = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    let text = std::fs::read(vectors.join("text.txt")).expect("text vector");
    let mut noise = Vec::new();
    let mut x: u32 = 7;
    for _ in 0..70_000 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        noise.push((x >> 16) as u8);
    }
    let mut runs = Vec::new();
    for i in 0..200_000u32 {
        runs.push(if i % 1000 < 700 { 0 } else { (i % 13) as u8 });
    }
    vec![
        ("empty".to_owned(), Vec::new()),
        ("one".to_owned(), b"x".to_vec()),
        ("text".to_owned(), text),
        ("noise".to_owned(), noise),
        ("runs".to_owned(), runs),
    ]
}

#[test]
#[ignore = "needs python3"]
fn python_zlib_inflates_our_output() {
    let dir = std::env::temp_dir().join(format!("format198x-deflate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    for (name, data) in inputs() {
        std::fs::write(dir.join(format!("{name}.plain")), &data).expect("write");
        for level in 0..=9u8 {
            let l = Level::new(level);
            std::fs::write(
                dir.join(format!("{name}.l{level}.deflate")),
                deflate(&data, l),
            )
            .expect("write");
            std::fs::write(
                dir.join(format!("{name}.l{level}.zlib")),
                zlib::encode(&data, l),
            )
            .expect("write");
        }
        let header = gzip::Header {
            name: Some(format!("{name}.bin").into_bytes()),
            comment: Some(b"comment".to_vec()),
            extra: Some(b"AB\x01\x00z".to_vec()),
            header_crc: true,
            ..gzip::Header::default()
        };
        let gz = gzip::encode(&header, &data, Level::DEFAULT).expect("encodes");
        std::fs::write(dir.join(format!("{name}.l6.gz")), gz).expect("write");
    }
    let status = Command::new("python3")
        .arg("-I")
        .arg("-c")
        .arg(CHECK)
        .arg(&dir)
        .status()
        .expect("python3 runs");
    std::fs::remove_dir_all(&dir).ok();
    assert!(status.success(), "Python's zlib rejected our output");
}
