//! LIST as the C64's own tokeniser stores it back out: each stored byte is
//! either a token (0x80–0xCB) printed as its keyword text, or a PETSCII byte
//! printed as its character. There is no ROM leading/trailing space logic to
//! model here — unlike the Spectrum, the C64 stores exactly what LIST prints.

use crate::tokens::KEYWORDS;
use crate::{ListingError, lex_line};

/// Keyword text for a stored token byte, the inverse of [`KEYWORDS`]. Covers
/// the token range 0x80 (END) to 0xCB (GO) that the tokeniser produces.
fn keyword_text(code: u8) -> Option<&'static str> {
    KEYWORDS
        .iter()
        .find(|&&(_, token)| token == code)
        .map(|&(text, _)| text)
}

/// Converts a stored PETSCII byte back to the ASCII character LIST prints,
/// inverting the tokeniser's `ascii_to_petscii` for the range it maps. That
/// range only raises `a`-`z` to `A`-`Z`, so every other stored byte —
/// including the letters it already raised — is already its own listed form.
fn petscii_to_ascii(byte: u8) -> u8 {
    byte
}

/// One line as LIST prints it: the line number, a space, then `body` with
/// tokens expanded to keyword text. `body` excludes the line's own `0x00`
/// terminator.
pub fn list_line(number: u16, body: &[u8]) -> String {
    let mut out = format!("{number} ");
    for &byte in body {
        if let Some(text) = keyword_text(byte) {
            out.push_str(text);
        } else {
            out.push(char::from(petscii_to_ascii(byte)));
        }
    }
    out
}

/// Every line of a PRG, as LIST prints them. `prg` is the whole PRG,
/// including its 2-byte load address.
///
/// # Errors
/// Returns an error if the bytes end partway through a line's link, header,
/// or body.
pub fn list(prg: &[u8]) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let mut at = 2; // skip the 2-byte load address
    loop {
        let link = prg
            .get(at..at + 2)
            .ok_or("program ends inside a line's next-line link")?;
        if link == [0x00, 0x00] {
            break;
        }
        let header = prg
            .get(at + 2..at + 4)
            .ok_or("program ends inside a line header")?;
        let number = u16::from_le_bytes([header[0], header[1]]);
        let body_start = at + 4;
        let body_end = prg
            .get(body_start..)
            .and_then(|rest| rest.iter().position(|&b| b == 0x00))
            .map(|offset| body_start + offset)
            .ok_or("program ends inside a line body")?;
        lines.push(list_line(number, &prg[body_start..body_end]));
        at = body_end + 1;
    }
    Ok(lines)
}

/// Convert a text listing to the lines LIST would print for it, in source
/// order, alongside each line's 0-based source line index. Blank source
/// lines are skipped and any other line must start with a line number,
/// matching [`crate::tokenise`].
///
/// # Errors
/// Returns an error under the same conditions as [`lex_line`].
pub fn listed_form(source: &str) -> Result<Vec<(usize, String)>, ListingError> {
    let mut out = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        let lexed = lex_line(raw).map_err(|e| ListingError::new(index + 1, e.message))?;
        let bytes: Vec<u8> = lexed
            .pieces
            .into_iter()
            .flat_map(|piece| piece.bytes)
            .collect();
        out.push((index, list_line(lexed.number, &bytes)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PieceKind, tokenise};

    #[test]
    fn c64_lists_stored_characters_without_adding_spaces() {
        let prg = tokenise("10 PRINT CHR$(147)\n20 FORI=1TO10:NEXT")
            .expect("t")
            .bytes;
        assert_eq!(
            list(&prg).expect("list"),
            vec!["10 PRINT CHR$(147)", "20 FORI=1TO10:NEXT"]
        );
    }

    #[test]
    fn lowercase_source_lists_uppercase_and_round_trips_uppercase() {
        let prg = tokenise("10 print \"hi\"").expect("t").bytes;
        assert_eq!(list(&prg).expect("list"), vec!["10 PRINT \"HI\""]);
        let upper = tokenise("10 PRINT \"HI\"").expect("t").bytes;
        assert_eq!(prg, upper);
    }

    #[test]
    fn c64_listed_form_flags_extra_space_after_the_number() {
        let listed = listed_form("10  PRINT 1").expect("listed");
        assert_eq!(listed[0].1, "10  PRINT 1"); // the second space is stored, so it lists
        assert_eq!(listed_form("10 PRINT 1").expect("l")[0].1, "10 PRINT 1");
    }

    #[test]
    fn keyword_text_covers_the_token_range() {
        assert_eq!(keyword_text(0x80), Some("END"));
        assert_eq!(keyword_text(0x99), Some("PRINT"));
        assert_eq!(keyword_text(0xCB), Some("GO"));
        assert_eq!(keyword_text(0x00), None);
    }

    #[test]
    fn truncated_programs_are_errors() {
        let prg = tokenise("10 PRINT 1").expect("tokenise").bytes;
        assert!(list(&prg[..3]).is_err());
        assert!(list(&prg[..prg.len() - 1]).is_err());
    }

    #[test]
    fn listed_form_round_trips_a_multi_line_listing() {
        let src = "10 PRINT \"A\"\n20 GOTO 10";
        let listed = listed_form(src).expect("listed");
        assert_eq!(listed.len(), 2);
        for ((_, got), want) in listed.iter().zip(src.lines()) {
            assert_eq!(got, want);
        }
    }

    #[test]
    fn listed_form_skips_blank_lines_and_rejects_comment_lines() {
        let listed = listed_form("\n\n10 END\n").expect("listed");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0], (2, "10 END".to_string()));
        assert_eq!(
            listed_form("# comment\n10 END").expect_err("comment").line,
            1
        );
    }

    #[test]
    fn lex_line_pieces_know_their_kind() {
        // Sanity check that `PieceKind` values are reachable from here too,
        // for a later task treating both dialects uniformly.
        let line = lex_line("10 PRINT 1").expect("lex");
        assert_eq!(line.pieces[0].kind, PieceKind::Keyword(0x99));
    }
}
