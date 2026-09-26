use format198x_sinclair_zx_spectrum_bas::tokenise_listing;

/// Each number's hidden five bytes, as the genuine ROM computed them
/// (tests/fixtures/rom-numbers), against what the tokeniser stores.
#[test]
fn hidden_numbers_are_what_the_rom_computes() {
    let mut failures = Vec::new();
    let cases = include_str!("fixtures/rom-numbers/numbers.tsv");
    for case in cases.lines() {
        let (spelling, want) = case.split_once('\t').expect("tab");
        let line = format!("10 PRINT {spelling}");
        let stored = tokenise_listing(&line).expect("tokenises").bytes;
        // Line header (4), PRINT, the spelling, 0x0E, five bytes, 0x0D.
        let hidden = &stored[stored.len() - 6..stored.len() - 1];
        let got: String = hidden.iter().map(|b| format!("{b:02x}")).collect();
        if got != want {
            failures.push(format!("{spelling}: ROM {want}, crate {got}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.lines().count(),
        failures.join("\n")
    );
}
