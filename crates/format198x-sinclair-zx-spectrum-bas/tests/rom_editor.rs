use format198x_sinclair_zx_spectrum_bas::{list, tokenise_listing};

/// Each line's bytes as the genuine ROM's editor stored them when the line
/// was typed in, hidden numbers included (tests/fixtures/rom-editor).
fn rom_stored() -> Vec<Vec<u8>> {
    include_str!("fixtures/rom-editor/cases.hex")
        .lines()
        .map(|line| {
            (0..line.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&line[at..at + 2], 16).expect("hex"))
                .collect()
        })
        .collect()
}

#[test]
fn tokeniser_stores_what_the_roms_editor_stores() {
    let source = include_str!("fixtures/rom-editor/cases.bas");
    let lines: Vec<&str> = source.lines().collect();
    let stored = rom_stored();
    assert_eq!(lines.len(), stored.len());
    for (line, want) in lines.iter().zip(&stored) {
        let got = tokenise_listing(line).expect("tokenises").bytes;
        assert_eq!(&got, want, "`{line}`");
    }
    let program: Vec<u8> = stored.concat();
    let trimmed = |bytes: &[u8]| -> Vec<String> {
        list(bytes)
            .expect("list")
            .iter()
            .map(|l| l.trim_end().to_string())
            .collect()
    };
    assert_eq!(
        trimmed(&tokenise_listing(source).expect("tokenises").bytes),
        trimmed(&program)
    );
}

#[test]
fn tokeniser_refuses_what_the_roms_editor_refuses() {
    for line in include_str!("fixtures/rom-editor/refused.bas").lines() {
        assert!(
            tokenise_listing(line).is_err(),
            "`{line}` should not tokenise"
        );
    }
}
