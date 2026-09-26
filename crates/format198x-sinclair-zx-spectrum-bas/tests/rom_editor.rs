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

/// A hidden five-byte number's value: the small-integer form (first byte 0)
/// or the full floating-point form.
fn value(float5: &[u8]) -> f64 {
    if float5[0] == 0 {
        let magnitude = f64::from(u16::from_le_bytes([float5[2], float5[3]]));
        return if float5[1] == 0 {
            magnitude
        } else {
            magnitude - 65536.0
        };
    }
    let mantissa = u32::from_be_bytes([float5[1] | 0x80, float5[2], float5[3], float5[4]]);
    let sign = if float5[1] & 0x80 == 0 { 1.0 } else { -1.0 };
    sign * f64::from(mantissa) * 2f64.powi(i32::from(float5[0]) - 160)
}

/// A line's bytes with each hidden number replaced by its value, so lines
/// compare by where their numbers are and what they are worth.
fn split_hidden(line: &[u8]) -> (Vec<u8>, Vec<f64>) {
    let (mut text, mut values) = (Vec::new(), Vec::new());
    let mut at = 0;
    while at < line.len() {
        text.push(line[at]);
        if line[at] == 0x0E && at >= 4 {
            values.push(value(&line[at + 1..at + 6]));
            at += 6;
        } else {
            at += 1;
        }
    }
    (text, values)
}

#[test]
fn tokeniser_stores_what_the_roms_editor_stores() {
    let source = include_str!("fixtures/rom-editor/cases.bas");
    let lines: Vec<&str> = source.lines().collect();
    let stored = rom_stored();
    assert_eq!(lines.len(), stored.len());
    for (line, want) in lines.iter().zip(&stored) {
        let got = tokenise_listing(line).expect("tokenises").bytes;
        // Every stored character, and a hidden number after the same ones
        // (after any spaces that follow a number), holding the same value.
        // The five bytes themselves can differ: the ROM computes `1.5 E3`
        // with its calculator and stores 1500 in the full floating-point
        // form, where this crate stores the small-integer form.
        assert_eq!(split_hidden(&got), split_hidden(want), "`{line}`");
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
