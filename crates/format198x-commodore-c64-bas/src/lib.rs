//! Commodore 64 BASIC V2 tokeniser and lister.
//!
//! Converts plain-text `.bas` files with line numbers into tokenised PRG bytes
//! suitable for direct PRG import, and lists a PRG back out the way the C64's
//! own LIST command prints it.
//!
//! Keywords tokenise wherever the C64's cruncher would tokenise them, with
//! one difference: `?` is stored as the character, as petcat stores it,
//! where the C64 stores the PRINT token.

mod error;
mod list;
mod tokens;

use std::collections::BTreeMap;

pub use error::ListingError;
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
///
/// Concatenating the `bytes` of a line's pieces gives the stored line body,
/// without its link, line number or closing `0x00`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// What sort of text this piece is.
    pub kind: PieceKind,
    /// Bytes this piece stores (a token byte, or one or more PETSCII bytes).
    pub bytes: Vec<u8>,
    /// 0-based byte offset of the piece within the line body (the text after
    /// the line number and the one space that may follow it). Add
    /// [`LexLine::body_column`] for the offset within the source line.
    pub column: usize,
    /// The source text the piece came from, as typed (any case).
    pub text: String,
}

/// What sort of text a [`Piece`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PieceKind {
    /// A keyword stored as one token byte, which this carries: 0x80 (END)
    /// to 0xCB (GO), including the operators `+ - * / ^ > = <` (0xAA–0xB3).
    Keyword(u8),
    /// A run of letters, with any digits or `$` that follow, not matched as
    /// a keyword. Keywords match anywhere outside strings, REM and DATA
    /// values, so `SCORE` lexes as the name `SC`, the OR keyword and the
    /// name `E`.
    Name,
    /// A run of digits.
    Number,
    /// A string literal, including its quotation marks (the closing one may
    /// be missing at the end of a line, as the C64 allows).
    Str,
    /// The text after REM, to the end of the line, stored as typed.
    Rem,
    /// One stored space.
    Space,
    /// Any other single character, such as `:`, `,`, `(` or `.`.
    Punct,
}

/// A listing line split into number and pieces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexLine {
    /// The line number, 1 to 63999.
    pub number: u16,
    /// 0-based byte offset of the body within the source line.
    pub body_column: usize,
    /// The body's pieces, in source order.
    pub pieces: Vec<Piece>,
}

/// Tokenises text BASIC source into C64 PRG format.
///
/// Blank lines are skipped; every other line must start with a line number.
/// Lines are stored in line-number order whatever order the source gives
/// them in, as the C64 inserts each typed line into place.
///
/// # Errors
///
/// Returns an error if any line number is missing, out of range or used
/// twice, a line holds a character outside printable ASCII, or the program
/// would run past the top of the 64K address space.
pub fn tokenise(source: &str) -> Result<BasicProgram, ListingError> {
    let mut lines: BTreeMap<u16, Vec<u8>> = BTreeMap::new();

    for (line_idx, raw_line) in source.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }

        let lexed = lex_line(raw_line).map_err(|e| ListingError::new(line_idx + 1, e.message))?;
        let bytes: Vec<u8> = lexed
            .pieces
            .into_iter()
            .flat_map(|piece| piece.bytes)
            .collect();
        if lines.insert(lexed.number, bytes).is_some() {
            return Err(ListingError::new(
                line_idx + 1,
                format!(
                    "line {} appears twice; edit its existing line",
                    lexed.number
                ),
            ));
        }
    }

    let mut output = vec![BASIC_START as u8, (BASIC_START >> 8) as u8];
    let mut addr = BASIC_START;

    for (line_num, content) in &lines {
        let line_size = 2 + 2 + content.len() + 1;
        let next_addr = u16::try_from(line_size)
            .ok()
            .and_then(|size| addr.checked_add(size))
            .ok_or_else(|| ListingError::new(0, "program too large for the C64's memory"))?;
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
/// Returns an error for a malformed or missing line number, or a character
/// outside printable ASCII (the lexer converts ASCII to PETSCII, and has no
/// mapping for anything else). A single line has no source context, so the
/// error's `line` is 0; [`tokenise`] and [`listed_form`] fill it in.
pub fn lex_line(line: &str) -> Result<LexLine, ListingError> {
    let trimmed_end = line.trim_end();
    let text = trimmed_end.trim_start();
    if !text.bytes().all(|b| (32..=126).contains(&b)) {
        return Err(ListingError::new(
            0,
            "use plain ASCII text; graphics and control codes are not supported here",
        ));
    }
    let (number, body_start) =
        parse_line_number(text).map_err(|message| ListingError::new(0, message))?;
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

/// Lex a line body into positioned pieces. Concatenating the pieces' bytes
/// gives the stored line body, which `tokenise` relies on.
fn lex_body(source: &str) -> Vec<Piece> {
    let bytes = source.as_bytes();
    let mut out: Vec<Piece> = Vec::new();
    let mut pos = 0usize;
    // After DATA, the cruncher stores text literally up to the next `:`
    // outside quotes, so DATA values are never tokenised. In the ROM
    // (CRUNCH, $A57C) the DATA flag lives in $0F and only a `:` clears it; a
    // quoted value inside DATA leaves it set.
    let mut in_data = false;

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

        if ch == b':' {
            in_data = false;
        }

        if let Some((token, keyword_len)) = match_keyword(&bytes[pos..]).filter(|_| !in_data) {
            let start = pos;
            out.push(Piece {
                kind: PieceKind::Keyword(token),
                bytes: vec![token],
                column: start,
                text: source[start..start + keyword_len].to_string(),
            });
            pos += keyword_len;
            in_data = token == 0x83;
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

/// The keyword starting at `text`, if any. Like the C64's own cruncher, this
/// matches anywhere, with no word boundary: `GOTO10` and `FORI=1TO10`
/// tokenise, and `SCORE` stores `S`, `C`, the `OR` token and `E`. `?` is
/// the PRINT token, as the cruncher substitutes it ($A59C). The caller keeps
/// strings, REM text and DATA values away from here, and the ROM does the
/// same: its DATA test ($A598) comes before the `?` substitution, so a `?`
/// in a DATA value stays a character.
fn match_keyword(text: &[u8]) -> Option<(u8, usize)> {
    if text.first() == Some(&b'?') {
        return Some((0x99, 1));
    }
    KEYWORDS
        .iter()
        .find(|(keyword, _)| {
            text.len() >= keyword.len()
                && text[..keyword.len()].eq_ignore_ascii_case(keyword.as_bytes())
        })
        .map(|&(keyword, token)| (token, keyword.len()))
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

    /// The stored body of a one-line program: after the 2-byte load address,
    /// link and line number, up to the line's 0x00 terminator.
    fn body(src: &str) -> Vec<u8> {
        let bytes = tokenise(src).expect("should tokenise").bytes;
        bytes[6..bytes.len() - 3].to_vec()
    }

    #[test]
    fn keywords_match_anywhere_like_the_c64() {
        // Expected bytes are petcat 3's (`petcat -w2`, lowercased input),
        // which tokenises these as the C64's own cruncher does.
        assert_eq!(body("10 GOTO10"), [0x89, b'1', b'0']);
        assert_eq!(
            body("20 FORI=1TO10"),
            [0x81, b'I', 0xB2, b'1', 0xA4, b'1', b'0']
        );
        assert_eq!(
            body("30 IFA=1THEN20"),
            [0x8B, b'A', 0xB2, b'1', 0xA7, b'2', b'0']
        );
        assert_eq!(body("40 SCORE=1"), [b'S', b'C', 0xB0, b'E', 0xB2, b'1']);
        assert_eq!(
            body("70 LET PRINTER=1"),
            [0x88, b' ', 0x99, b'E', b'R', 0xB2, b'1']
        );
    }

    #[test]
    fn question_mark_is_the_print_token_as_the_c64_stores_it() {
        // The ROM's cruncher substitutes PRINT for `?` ($A59C); petcat
        // stores 0x3F instead, and the ROM is followed here.
        assert_eq!(body("10 ?A"), [0x99, b'A']);
        assert_eq!(
            body("10 ?\"?\":REM ?"),
            [&[0x99, b'"', b'?', b'"', b':', 0x8F][..], b" ?"].concat()
        );
        // After DATA the ROM stores everything literally up to a colon,
        // `?` included, then tokenises it again.
        assert_eq!(
            body("10 DATA ?:?"),
            [&[0x83][..], b" ?:", &[0x99][..]].concat()
        );
        let line = lex_line("10 ?A").expect("lex");
        assert_eq!(line.pieces[0].kind, PieceKind::Keyword(0x99));
        assert_eq!(line.pieces[0].text, "?");
        let prg = tokenise("10 ?A\n20 PRINT\"?\"").expect("t").bytes;
        assert_eq!(list(&prg).expect("list"), ["10 PRINTA", "20 PRINT\"?\""]);
    }

    #[test]
    fn data_values_are_stored_literally_up_to_a_colon() {
        // petcat 3 gives the same bytes for this line.
        assert_eq!(
            body("50 DATA TOAST,ORANGE:PRINTER"),
            [&[0x83][..], b" TOAST,ORANGE:", &[0x99, b'E', b'R'][..]].concat()
        );
        // A colon inside a quoted DATA value does not end the DATA.
        assert_eq!(
            body("60 DATA \"A:PRINT\",1:PRINT"),
            [&[0x83][..], b" \"A:PRINT\",1:", &[0x99][..]].concat()
        );
    }

    #[test]
    fn lowercase_converted_to_petscii() {
        let prog = tokenise("10 PRINT \"hello\"").expect("should tokenise");
        let quote_pos = prog.bytes.iter().position(|&b| b == b'"').expect("quote");
        assert_eq!(&prog.bytes[quote_pos + 1..quote_pos + 6], b"HELLO");
    }

    #[test]
    fn blank_lines_are_skipped_and_comment_lines_are_errors() {
        let prog = tokenise("\n  \n10 END\n").expect("should tokenise");
        assert_eq!(prog.bytes[4], 10);
        assert_eq!(prog.bytes[5], 0);
        let error = tokenise("# comment\n10 END").expect_err("comment line");
        assert_eq!(error.line, 1);
    }

    #[test]
    fn errors_carry_the_source_line_separately_from_the_message() {
        let error = tokenise("10 END\n\n64000 END").expect_err("error");
        assert_eq!(error.line, 3);
        assert_eq!(error.message, "line number 64000 out of range (1-63999)");
        assert_eq!(
            error.to_string(),
            "line 3: line number 64000 out of range (1-63999)"
        );
        assert_eq!(listed_form("10 END\n\n64000 END").err(), Some(error));
        assert_eq!(lex_line("PRINT").expect_err("error").line, 0);
    }

    #[test]
    fn characters_outside_printable_ascii_are_errors() {
        // `é` is two UTF-8 bytes, which used to be stored raw and list back
        // as the LEN and STEP tokens.
        for src in ["10 PRINT \"CAFé\"", "10 PRINT 1\t:END", "10 REM \u{1}"] {
            let error = tokenise(src).expect_err(src);
            assert_eq!(error.line, 1, "{src}");
            assert!(error.message.contains("plain ASCII"), "{src}");
        }
        assert!(listed_form("10 PRINT \"é\"").is_err());
        assert!(lex_line("10 PRINT \"é\"").is_err());
    }

    #[test]
    fn lines_are_stored_in_number_order_and_duplicates_are_errors() {
        let sorted = tokenise("10 PRINT 1\n20 PRINT 2").expect("sorted").bytes;
        let shuffled = tokenise("20 PRINT 2\n10 PRINT 1").expect("shuffled").bytes;
        assert_eq!(sorted, shuffled);
        let error = tokenise("10 PRINT 1\n20 END\n10 PRINT 2").expect_err("duplicate");
        assert_eq!(error.line, 3);
        assert_eq!(
            error.message,
            "line 10 appears twice; edit its existing line"
        );
    }

    #[test]
    fn a_program_past_the_top_of_memory_is_an_error_not_a_panic() {
        // 3,000 lines of ~25 bytes each run past $FFFF from $0801.
        let source: String = (1..=3000)
            .map(|n| format!("{n} PRINT \"XXXXXXXXXXXXXXXXXX\"\n"))
            .collect();
        let error = tokenise(&source).expect_err("too large");
        assert_eq!(
            (error.line, error.message.as_str()),
            (0, "program too large for the C64's memory")
        );
    }

    #[test]
    fn line_number_validation() {
        assert!(tokenise("0 PRINT \"BAD\"").is_err());
        assert!(tokenise("64000 PRINT \"BAD\"").is_err());
        assert!(tokenise("PRINT \"BAD\"").is_err());
    }

    #[test]
    fn pieces_concatenate_to_the_stored_bytes() {
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
