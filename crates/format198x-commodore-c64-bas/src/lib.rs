//! Commodore 64 BASIC V2 tokeniser and lister.
//!
//! Converts plain-text `.bas` files with line numbers into tokenised PRG bytes
//! suitable for direct PRG import, and lists a PRG back out the way the C64's
//! own LIST command prints it.

mod list;
mod tokens;

pub use list::{list, list_line, listed_form};
use tokens::KEYWORDS;

const BASIC_START: u16 = 0x0801;

/// A tokenised BASIC program in PRG format.
#[derive(Debug, Clone)]
pub struct BasicProgram {
    /// PRG bytes: load address, tokenised lines, end marker.
    pub bytes: Vec<u8>,
}

/// One stored piece of a line body, with where it came from in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub kind: PieceKind,
    /// Bytes this piece stores (a token byte, or one or more PETSCII bytes).
    pub bytes: Vec<u8>,
    /// 0-based byte offset of the piece in the body text passed to `lex_body`.
    pub column: usize,
    /// The source text the piece came from.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Keyword(u8),
    Name,
    Number,
    Str,
    Rem,
    Space,
    Punct,
}

/// A listing line split into number and pieces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexLine {
    pub number: u16,
    /// 0-based byte offset of the body within the source line.
    pub body_column: usize,
    pub pieces: Vec<Piece>,
}

/// Tokenises text BASIC source into C64 PRG format.
///
/// # Errors
///
/// Returns an error if any line number is missing or out of range.
pub fn tokenise(source: &str) -> Result<BasicProgram, String> {
    let mut lines: Vec<(u16, Vec<u8>)> = Vec::new();

    for (line_idx, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (line_num, body_start) = parse_line_number(line)
            .map_err(|message| format!("Line {}: {message}", line_idx + 1))?;
        let bytes: Vec<u8> = lex_body(&line[body_start..])
            .into_iter()
            .flat_map(|piece| piece.bytes)
            .collect();
        lines.push((line_num, bytes));
    }

    let mut output = vec![BASIC_START as u8, (BASIC_START >> 8) as u8];
    let mut addr = BASIC_START;

    for (line_num, content) in &lines {
        let line_size = 2 + 2 + content.len() + 1;
        let next_addr = addr + line_size as u16;
        output.push(next_addr as u8);
        output.push((next_addr >> 8) as u8);
        output.push(*line_num as u8);
        output.push((line_num >> 8) as u8);
        output.extend_from_slice(content);
        output.push(0x00);
        addr = next_addr;
    }

    output.push(0x00);
    output.push(0x00);

    Ok(BasicProgram { bytes: output })
}

/// Lex a single listing line into its line number and positioned pieces.
///
/// Only one space after the line number is treated as a separator; a second
/// space is stored as part of the body, so it lists back out too.
///
/// # Errors
/// Returns an error for a malformed or missing line number.
pub fn lex_line(line: &str) -> Result<LexLine, String> {
    let trimmed_end = line.trim_end();
    let text = trimmed_end.trim_start();
    let (number, body_start) = parse_line_number(text)?;
    let leading_ws = trimmed_end.len() - text.len();
    let body_column = leading_ws + body_start;
    let pieces = lex_body(&text[body_start..]);
    Ok(LexLine {
        number,
        body_column,
        pieces,
    })
}

/// Parses the line number at the start of `text` (already trimmed of
/// surrounding whitespace). Returns the number and the byte offset in `text`
/// where the body starts, after skipping exactly one space following the
/// digits — a second space is left for the body to store.
fn parse_line_number(text: &str) -> Result<(u16, usize), String> {
    let digit_end = text.bytes().take_while(u8::is_ascii_digit).count();
    if digit_end == 0 {
        return Err(format!("expected a line number, got: {text}"));
    }

    let num: u32 = text[..digit_end]
        .parse()
        .map_err(|err| format!("invalid line number: {err}"))?;
    if num == 0 || num > 63_999 {
        return Err(format!("line number {num} out of range (1-63999)"));
    }

    let body_start = if text[digit_end..].starts_with(' ') {
        digit_end + 1
    } else {
        digit_end
    };

    Ok((num as u16, body_start))
}

/// Lex a line body into positioned pieces. Byte-for-byte, concatenating the
/// pieces' bytes reproduces what the original per-character tokeniser wrote,
/// including its keyword-matching quirks (e.g. a word ending in a two-letter
/// keyword such as `TO` at the very end of a line still tokenises as that
/// keyword) — `tokenise` relies on this to stay byte-identical.
fn lex_body(source: &str) -> Vec<Piece> {
    let bytes = source.as_bytes();
    let mut out: Vec<Piece> = Vec::new();
    let mut pos = 0usize;

    while pos < bytes.len() {
        let ch = bytes[pos];

        if ch == b'"' {
            let start = pos;
            pos += 1;
            while pos < bytes.len() && bytes[pos] != b'"' {
                pos += 1;
            }
            if pos < bytes.len() {
                pos += 1; // include the closing quote
            }
            out.push(Piece {
                kind: PieceKind::Str,
                bytes: bytes[start..pos]
                    .iter()
                    .copied()
                    .map(ascii_to_petscii)
                    .collect(),
                column: start,
                text: source[start..pos].to_string(),
            });
            continue;
        }

        if let Some((token, keyword_len)) = match_keyword(&bytes[pos..]) {
            let start = pos;
            out.push(Piece {
                kind: PieceKind::Keyword(token),
                bytes: vec![token],
                column: start,
                text: source[start..start + keyword_len].to_string(),
            });
            pos += keyword_len;
            if token == 0x8F {
                // REM: the rest of the line is stored literally.
                out.push(Piece {
                    kind: PieceKind::Rem,
                    bytes: bytes[pos..].iter().copied().map(ascii_to_petscii).collect(),
                    column: pos,
                    text: source[pos..].to_string(),
                });
                break;
            }
            continue;
        }

        // Continue a Name run onto a digit or `$` (e.g. `A1`, `A$`), but only
        // when it is genuinely contiguous with a Name piece just written.
        let continues_name = matches!(
            out.last(),
            Some(piece) if piece.kind == PieceKind::Name && piece.column + piece.bytes.len() == pos
        ) && (ch.is_ascii_alphanumeric() || ch == b'$');
        let kind = if continues_name {
            PieceKind::Name
        } else if ch.is_ascii_digit() {
            PieceKind::Number
        } else if ch.is_ascii_alphabetic() {
            PieceKind::Name
        } else if ch == b' ' {
            PieceKind::Space
        } else {
            PieceKind::Punct
        };

        let byte = ascii_to_petscii(ch);
        let extends_previous = matches!(kind, PieceKind::Name | PieceKind::Number)
            && matches!(
                out.last(),
                Some(piece) if piece.kind == kind && piece.column + piece.bytes.len() == pos
            );
        if extends_previous {
            let piece = out
                .last_mut()
                .expect("extends_previous is only true when out.last() matched above");
            piece.bytes.push(byte);
            piece.text.push(char::from(ch));
        } else {
            out.push(Piece {
                kind,
                bytes: vec![byte],
                column: pos,
                text: char::from(ch).to_string(),
            });
        }
        pos += 1;
    }

    out
}

fn match_keyword(text: &[u8]) -> Option<(u8, usize)> {
    for &(keyword, token) in KEYWORDS {
        if text.len() >= keyword.len()
            && text[..keyword.len()].eq_ignore_ascii_case(keyword.as_bytes())
        {
            let last_kw = keyword.as_bytes()[keyword.len() - 1];
            if (last_kw.is_ascii_alphabetic() || last_kw == b'$')
                && let Some(&next) = text.get(keyword.len())
                && (next.is_ascii_alphanumeric() || next == b'$')
            {
                continue;
            }

            return Some((token, keyword.len()));
        }
    }

    None
}

fn ascii_to_petscii(ch: u8) -> u8 {
    match ch {
        b'a'..=b'z' => ch - 0x20,
        _ => ch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenise_simple_print() {
        let prog = tokenise("10 PRINT \"HELLO\"").expect("should tokenise");
        assert_eq!(prog.bytes[0], 0x01);
        assert_eq!(prog.bytes[1], 0x08);
        assert_eq!(prog.bytes[6], 0x99);
    }

    #[test]
    fn tokenise_goto() {
        let prog = tokenise("20 GOTO 10").expect("should tokenise");
        assert_eq!(prog.bytes[6], 0x89);
    }

    #[test]
    fn next_line_pointers() {
        let prog = tokenise("10 PRINT \"A\"\n20 GOTO 10").expect("should tokenise");
        let next_ptr = u16::from(prog.bytes[2]) | (u16::from(prog.bytes[3]) << 8);
        assert_eq!(next_ptr, 0x080B);
    }

    #[test]
    fn program_ends_with_zero_marker() {
        let prog = tokenise("10 END").expect("should tokenise");
        let len = prog.bytes.len();
        assert_eq!(prog.bytes[len - 2], 0x00);
        assert_eq!(prog.bytes[len - 1], 0x00);
    }

    #[test]
    fn rem_preserves_content() {
        let prog = tokenise("10 REM PRINT IS NOT TOKENISED").expect("should tokenise");
        assert_eq!(prog.bytes[6], 0x8F);
        let after_rem = &prog.bytes[7..prog.bytes.len() - 3];
        assert!(!after_rem.contains(&0x99));
    }

    #[test]
    fn string_preserves_keywords() {
        let prog = tokenise("10 PRINT \"GOTO\"").expect("should tokenise");
        let quote_pos = prog.bytes.iter().position(|&b| b == b'"').expect("quote");
        assert_eq!(&prog.bytes[quote_pos + 1..quote_pos + 5], b"GOTO");
    }

    #[test]
    fn keyword_not_matched_as_prefix() {
        let prog = tokenise("10 LET PRINTER=1").expect("should tokenise");
        let after_header = &prog.bytes[6..];
        assert!(after_header.contains(&0x88));
        assert!(!after_header.contains(&0x99));
    }

    #[test]
    fn lowercase_converted_to_petscii() {
        let prog = tokenise("10 PRINT \"hello\"").expect("should tokenise");
        let quote_pos = prog.bytes.iter().position(|&b| b == b'"').expect("quote");
        assert_eq!(&prog.bytes[quote_pos + 1..quote_pos + 6], b"HELLO");
    }

    #[test]
    fn skip_blank_and_comment_lines() {
        let prog = tokenise("# comment\n\n10 END\n").expect("should tokenise");
        assert_eq!(prog.bytes[4], 10);
        assert_eq!(prog.bytes[5], 0);
    }

    #[test]
    fn line_number_validation() {
        assert!(tokenise("0 PRINT \"BAD\"").is_err());
        assert!(tokenise("64000 PRINT \"BAD\"").is_err());
        assert!(tokenise("PRINT \"BAD\"").is_err());
    }

    #[test]
    fn pieces_concatenate_to_the_stored_bytes() {
        // Includes the mid-identifier keyword quirk that `tokenise` already
        // had: "AUTO" followed by nothing alphanumeric tokenises its trailing
        // "TO" as the TO keyword, because `match_keyword` only checks the
        // character immediately after the match, not whether it is at a word
        // start. `lex_body` must reproduce that, not smooth it over.
        for src in [
            "10 PRINT \"HELLO\"",
            "20 GOTO 10",
            "10 REM PRINT IS NOT TOKENISED",
            "10 LET PRINTER=1",
            "10 AUTO = 5",
            "10 LET A1=A$+B$",
        ] {
            let body_start = src.find(' ').expect("space after line number") + 1;
            let lexed: Vec<u8> = lex_body(&src[body_start..])
                .into_iter()
                .flat_map(|piece| piece.bytes)
                .collect();
            let full = tokenise(src).expect("tokenise");
            assert_eq!(
                &full.bytes[6..full.bytes.len() - 3],
                lexed.as_slice(),
                "{src}"
            );
        }
    }

    #[test]
    fn lex_line_reports_kinds_and_columns() {
        let line = lex_line("  20 IF A1=1 THEN PRINT \"X\":REM hi").expect("lex");
        assert_eq!(line.number, 20);
        assert_eq!(line.body_column, 5);
        let kinds: Vec<PieceKind> = line.pieces.iter().map(|piece| piece.kind).collect();
        assert_eq!(
            kinds,
            vec![
                PieceKind::Keyword(0x8B), // IF
                PieceKind::Space,
                PieceKind::Name,          // A1
                PieceKind::Keyword(0xB2), // =
                PieceKind::Number,        // 1
                PieceKind::Space,
                PieceKind::Keyword(0xA7), // THEN
                PieceKind::Space,
                PieceKind::Keyword(0x99), // PRINT
                PieceKind::Space,
                PieceKind::Str,           // "X"
                PieceKind::Punct,         // :
                PieceKind::Keyword(0x8F), // REM
                PieceKind::Rem,
            ]
        );
        let name = &line.pieces[2];
        assert_eq!((name.column, name.text.as_str()), (3, "A1"));
    }
}
