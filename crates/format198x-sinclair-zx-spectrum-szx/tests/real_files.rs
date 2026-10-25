//! Read the real SZX files the Emu198x reader was written for.
//!
//! The unit tests build their own snapshots, so they prove the crate
//! self-consistent and nothing more. These read files written by
//! Spectaculator and SpecEmu, from the `zx-spectrum-tests` corpus, which is
//! not redistributable and so is not in this repository:
//!
//! ```sh
//! export EMU198X_ZX_SPECTRUM_TESTS_DIR=$PWD/zx-spectrum-tests
//! cargo test -p format198x-sinclair-zx-spectrum-szx --test real_files -- --ignored
//! ```
//!
//! Every file must also write back byte for byte, which is what a lossless
//! container promises.

use format198x_sinclair_zx_spectrum_szx::Szx;
use std::path::PathBuf;

const TESTS_DIR_ENV: &str = "EMU198X_ZX_SPECTRUM_TESTS_DIR";

fn tests_dir() -> Option<PathBuf> {
    std::env::var_os(TESTS_DIR_ENV).map(PathBuf::from)
}

/// The 128K timing suite: a 128K machine, eight distinct RAM banks, paging
/// locked in `$7FFD`.
#[test]
#[ignore = "FIXTURE: needs EMU198X_ZX_SPECTRUM_TESTS_DIR"]
fn reads_the_128k_timing_suite() {
    let Some(dir) = tests_dir() else {
        eprintln!("skipped: set {TESTS_DIR_ENV}");
        return;
    };
    let path = dir.join(
        "ZX Spectrum Timing Tests - 128K v1.0 (2015-03-30)(Butler, Richard; Butler, Tim)[!].szx",
    );
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let szx = Szx::parse(&bytes).expect("the 128K timing suite should parse");
    assert_eq!(szx.encode().expect("encodes"), bytes);
    assert_eq!(szx.machine_id, 2, "ZXSTMID_128K");
    let regs = szx.z80_registers().expect("Z80R").expect("present");
    assert!(regs.sp >= 0x4000 && regs.im <= 2);
    let spcr = szx.spectrum_registers().expect("SPCR").expect("present");
    assert_eq!(spcr.port_7ffd & 0x20, 0x20, "paging locked");
    let pages = szx.ram_pages().expect("RAMP");
    let mut banks: Vec<u8> = pages.iter().map(|p| p.bank).collect();
    banks.sort_unstable();
    assert_eq!(banks, (0..8).collect::<Vec<u8>>());
    let distinct = pages
        .iter()
        .map(|p| p.data)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    assert!(distinct > 1, "the banks must not all be one page");
}

/// Every `.szx` in the corpus parses and writes back unchanged.
#[test]
#[ignore = "FIXTURE: needs EMU198X_ZX_SPECTRUM_TESTS_DIR"]
fn reads_every_szx_in_the_corpus() {
    let Some(dir) = tests_dir() else {
        eprintln!("skipped: set {TESTS_DIR_ENV}");
        return;
    };
    let mut seen = 0usize;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("corpus directory should be readable") {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("szx") {
            continue;
        }
        seen += 1;
        let bytes = std::fs::read(&path).expect("readable");
        match Szx::parse(&bytes) {
            Ok(szx) if szx.encode().ok().as_deref() == Some(&bytes[..]) => {}
            Ok(_) => failures.push(format!("{}: did not write back", path.display())),
            Err(e) => failures.push(format!("{}: {e}", path.display())),
        }
    }
    assert!(seen > 0, "no .szx files found under {}", dir.display());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
