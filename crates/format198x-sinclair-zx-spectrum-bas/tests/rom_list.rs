use format198x_sinclair_zx_spectrum_bas::{list, tokenise_listing};

#[test]
fn lister_matches_the_rom_for_every_captured_case() {
    let source = include_str!("fixtures/rom-list/cases.bas");
    let expected: Vec<&str> = include_str!("fixtures/rom-list/cases.list")
        .lines()
        .collect();
    let program = tokenise_listing(source).expect("cases tokenise");
    let listed = list(&program.bytes).expect("list");
    assert_eq!(listed.len(), expected.len());
    for (got, want) in listed.iter().zip(&expected) {
        assert_eq!(got.trim_end(), *want, "ROM lists `{want}`");
    }
}
