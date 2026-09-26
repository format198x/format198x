//! Text-preserving conversion for editable listings. The ROM judges syntax;
//! unlike the analysis AST, this path never invents expressions or drops text.
use crate::error::ListingError;
use crate::list::{list_line, rom_leading_space};
use crate::tokens::KeywordRole;
use crate::{BasicProgram, serialize::number_to_float5, tokens::KEYWORDS};
use std::collections::BTreeMap;

/// One stored piece of a line body, with where it came from in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub kind: PieceKind,
    /// Bytes this piece stores (a token byte, text, or text + 0x0E + 5-byte float).
    pub bytes: Vec<u8>,
    /// 0-based byte offset of the piece in the body text passed to `lex_line`.
    pub column: usize,
    /// The source text the piece came from.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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

/// Lex a single listing line into its line number and positioned pieces.
///
/// # Errors
/// Returns an error for unsupported characters, a malformed or missing line
/// number, an empty body, or a malformed number/string within the body. A
/// single line has no source context, so the error's `line` is 0;
/// [`tokenise_listing`] and [`listed_form`] fill it in.
pub fn lex_line(line: &str) -> Result<LexLine, ListingError> {
    let fail = |message: &str| ListingError::new(0, message);
    let trimmed_end = line.trim_end_matches('\r');
    let text = trimmed_end.trim();
    if !text.bytes().all(|b| (32..=126).contains(&b)) {
        return Err(fail(
            "use plain ASCII text; graphics and control codes are not supported here",
        ));
    }
    let end = text.bytes().take_while(u8::is_ascii_digit).count();
    let number: u16 = text[..end]
        .parse()
        .map_err(|_| fail("start with a line number from 1 to 9999"))?;
    if !(1..=9999).contains(&number) {
        return Err(fail("line number must be 1 to 9999"));
    }
    let after_number = &text[end..];
    let body = after_number.trim_start();
    if body.is_empty() {
        return Err(fail("remove an entire line to delete it"));
    }
    let leading_ws = trimmed_end.len() - trimmed_end.trim_start().len();
    let body_column = leading_ws + end + (after_number.len() - body.len());
    let pieces = lex_body(body).map_err(|message| fail(&message))?;
    Ok(LexLine {
        number,
        body_column,
        pieces,
    })
}

/// Convert a numbered ASCII listing to stored BASIC, ordered by line number.
///
/// Blank lines are skipped; every other line, including one starting `#`,
/// must start with a line number. Keyword names ending in a letter or `$`
/// need word boundaries; strings and REM text stay literal. Numeric spellings are retained with their hidden five-byte values appended.
/// This bounded editor route excludes DEF FN (which needs parameter markers).
///
/// # Errors
/// Returns an error for unsupported characters, malformed numbers/strings,
/// missing, duplicate or empty lines, unsupported DEF FN, or oversized output.
/// BASIC grammar is intentionally left to the ROM; tokenisation is not validation.
pub fn tokenise_listing(source: &str) -> Result<BasicProgram, ListingError> {
    if source.len() > 65536 {
        return Err(ListingError::new(0, "Source exceeds 64 KiB"));
    }
    let mut lines = BTreeMap::new();
    for (index, raw) in source.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        let context = |message: &str| ListingError::new(index + 1, message);
        let lexed = lex_line(raw).map_err(|e| context(&e.message))?;
        let mut bytes: Vec<u8> = lexed.pieces.into_iter().flat_map(|p| p.bytes).collect();
        bytes.push(13);
        let length = u16::try_from(bytes.len()).map_err(|_| context("line is too long"))?;
        let mut line = lexed.number.to_be_bytes().to_vec();
        line.extend_from_slice(&length.to_le_bytes());
        line.extend(bytes);
        if lines.insert(lexed.number, line).is_some() {
            return Err(context(&format!(
                "line {} appears twice; edit its existing line",
                lexed.number
            )));
        }
    }
    let bytes: Vec<u8> = lines.into_values().flatten().collect();
    if bytes.is_empty() {
        return Err(ListingError::new(
            0,
            "Enter at least one numbered BASIC line",
        ));
    }
    if bytes.len() > 0x9000 {
        return Err(ListingError::new(
            0,
            "Program exceeds this 48K editor's 36 KiB limit",
        ));
    }
    Ok(BasicProgram { bytes })
}

/// Convert a text listing to the lines LIST would print for it, in source
/// order, alongside each line's 0-based source line index. Blank source lines
/// are skipped, matching [`tokenise_listing`].
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
        let bytes: Vec<u8> = lexed.pieces.into_iter().flat_map(|p| p.bytes).collect();
        out.push((index, list_line(lexed.number, &bytes)));
    }
    Ok(out)
}

fn lex_body(source: &str) -> Result<Vec<Piece>, String> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0;
    let mut binary = false;
    let mut statement_start = true;
    let mut printing = false;
    let mut variable = false;
    while pos < bytes.len() {
        let ch = bytes[pos];
        if variable && ch.is_ascii_alphabetic() {
            let start = pos;
            while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'$') {
                pos += 1;
            }
            out.push(Piece {
                kind: PieceKind::Name,
                bytes: bytes[start..pos].to_vec(),
                column: start,
                text: source[start..pos].to_string(),
            });
            variable = false;
            statement_start = false;
        } else if ch == b'"' {
            let start = pos;
            pos += 1;
            while pos < bytes.len() && bytes[pos] != b'"' {
                pos += 1;
            }
            if pos == bytes.len() {
                return Err("missing closing quotation mark".into());
            }
            pos += 1;
            out.push(Piece {
                kind: PieceKind::Str,
                bytes: bytes[start..pos].to_vec(),
                column: start,
                text: source[start..pos].to_string(),
            });
        } else if ch == b' ' {
            out.push(Piece {
                kind: PieceKind::Space,
                bytes: vec![ch],
                column: pos,
                text: " ".to_string(),
            });
            pos += 1;
        } else if let Some((operator, token)) = [("<=", 0xC7), (">=", 0xC8), ("<>", 0xC9)]
            .iter()
            .find(|(operator, _)| source[pos..].starts_with(operator))
        {
            out.push(Piece {
                kind: PieceKind::Keyword(*token),
                bytes: vec![*token],
                column: pos,
                text: source[pos..pos + operator.len()].to_string(),
            });
            pos += operator.len();
        } else if let Some(&(keyword, token, _)) = KEYWORDS.iter().find(|(keyword, _, role)| {
            let allowed = match role {
                KeywordRole::Statement | KeywordRole::RestOfLine => statement_start,
                KeywordRole::PrintItem => statement_start || printing,
                _ => true,
            };
            if !allowed {
                return false;
            }
            let word = keyword.trim_end();
            let rest = &bytes[pos..];
            // A word boundary only matters when the keyword ends in a letter
            // or `$`: `OPEN #` and `CLOSE #` end in `#`, so the stream number
            // may follow directly (`OPEN #4`), as LIST prints it.
            let needs_boundary = word
                .bytes()
                .last()
                .is_some_and(|b| b.is_ascii_alphabetic() || b == b'$');
            rest.len() >= word.len()
                && rest[..word.len()].eq_ignore_ascii_case(word.as_bytes())
                && (!needs_boundary
                    || rest
                        .get(word.len())
                        .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'$'))
        }) {
            if token == 0xCE {
                return Err("DEF FN is not supported by this editor yet".into());
            }
            let start = pos;
            let word_len = keyword.trim_end().len();
            // The ROM prints a leading space before this keyword itself when
            // listing (PO-SEARCH); a single source space serving that purpose
            // is redundant, so drop it rather than store it twice over. Two
            // or more source spaces are real content (the ROM only ever adds
            // one), so only pop when the run immediately before the keyword
            // is exactly one space long.
            let last_is_space = matches!(out.last(), Some(p) if p.kind == PieceKind::Space);
            let run_is_one = out.len() < 2 || out[out.len() - 2].kind != PieceKind::Space;
            if rom_leading_space(token) && last_is_space && run_is_one {
                out.pop();
            }
            out.push(Piece {
                kind: PieceKind::Keyword(token),
                bytes: vec![token],
                column: start,
                text: source[start..start + word_len].to_string(),
            });
            if statement_start {
                printing = token == 0xF5 || token == 0xEE || token == 0xE0;
            }
            statement_start = token == 0xCB;
            variable = matches!(token, 0xF1 | 0xEB | 0xF3 | 0xE9 | 0xE3);
            pos += word_len;
            // A source space right after a keyword is dropped, except after
            // RND, INKEY$ and PI: those three take no arguments and get no
            // ROM trailing space on LIST either (`list::rom_trailing_space`
            // excludes them for the same reason), so a source space after
            // one is real content, not display padding the ROM will restore.
            // OPEN # and CLOSE # absorb one source space too, so `OPEN # 4`
            // stores the same bytes as `OPEN #4`, which is how LIST prints it
            // (no ROM trailing space after `#`) and which lexes back to the
            // same bytes.
            if !matches!(token, 0xA5..=0xA7) && bytes.get(pos) == Some(&b' ') {
                pos += 1;
            }
            if token == 0xEA {
                out.push(Piece {
                    kind: PieceKind::Rem,
                    bytes: bytes[pos..].to_vec(),
                    column: pos,
                    text: source[pos..].to_string(),
                });
                break;
            }
            binary = token == 0xC4;
        } else if ch.is_ascii_alphabetic() {
            let start = pos;
            while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'$') {
                pos += 1;
            }
            out.push(Piece {
                kind: PieceKind::Name,
                bytes: bytes[start..pos].to_vec(),
                column: start,
                text: source[start..pos].to_string(),
            });
            binary = false;
        } else if ch.is_ascii_digit()
            || (ch == b'.' && bytes.get(pos + 1).is_some_and(u8::is_ascii_digit))
        {
            let start = pos;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            if !binary && bytes.get(pos) == Some(&b'.') {
                pos += 1;
                while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
            }
            if !binary
                && bytes
                    .get(pos)
                    .is_some_and(|b| b.eq_ignore_ascii_case(&b'e'))
            {
                pos += 1;
                if bytes.get(pos).is_some_and(|b| *b == b'+' || *b == b'-') {
                    pos += 1;
                }
                let digits = pos;
                while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
                if pos == digits {
                    return Err("an exponent needs digits after E".into());
                }
            }
            let spelling = &source[start..pos];
            let value = if binary {
                u16::from_str_radix(spelling, 2)
                    .map(f64::from)
                    .map_err(|_| "BIN needs up to 16 binary digits")?
            } else {
                spelling.parse::<f64>().map_err(|_| "invalid number")?
            };
            if !value.is_finite()
                || value >= 2.0_f64.powi(127)
                || (value != 0.0 && value < 2.0_f64.powi(-128))
            {
                return Err("number is outside the Spectrum's range".into());
            }
            let mut number_bytes = spelling.as_bytes().to_vec();
            number_bytes.push(14);
            number_bytes.extend_from_slice(&number_to_float5(value));
            out.push(Piece {
                kind: PieceKind::Number,
                bytes: number_bytes,
                column: start,
                text: spelling.to_string(),
            });
            binary = false;
        } else {
            if ch == b':' {
                statement_start = true;
                printing = false;
            }
            // Retain punctuation and mistakes, so the ROM sees what was entered.
            out.push(Piece {
                kind: PieceKind::Punct,
                bytes: vec![ch],
                column: pos,
                text: (ch as char).to_string(),
            });
            pos += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_text_and_unknown_input_are_not_rewritten() {
        let result = tokenise_listing("10 PRINT \"GO TO 12\": REM PRINT 99").expect("listing");
        // No space before REM: Task 5 stops storing it, since the ROM supplies
        // it on LIST (REM starts with a letter, so `rom_leading_space` holds).
        assert_eq!(&result.bytes[4..], b"\xF5\"GO TO 12\":\xEAPRINT 99\r");
        assert!(result.bytes.windows(8).any(|w| w == b"GO TO 12"));
        assert!(result.bytes.windows(8).any(|w| w == b"PRINT 99"));
        assert!(!result.bytes.contains(&14));
        assert_eq!(
            &tokenise_listing("10 PRONT 2").expect("retain typo").bytes[4..9],
            b"PRONT"
        );
    }
    #[test]
    fn statement_words_can_be_variable_names() {
        let result = tokenise_listing("10 LET cat=2\n20 PRINT cat").expect("listing");
        assert!(!result.bytes.contains(&0xCF));
        assert_eq!(result.bytes.windows(3).filter(|w| *w == b"cat").count(), 2);
    }

    #[test]
    fn numbers_order_and_comparisons() {
        let result = tokenise_listing("20 IF x<>2 THEN PRINT x\n10 LET x=1.5").expect("listing");
        assert_eq!(&result.bytes[..2], &[0, 10]);
        assert!(result.bytes.contains(&0xC9));
        assert!(
            result
                .bytes
                .windows(9)
                .any(|w| w == [b'1', b'.', b'5', 14, 129, 64, 0, 0, 0])
        );
        assert!(tokenise_listing("10 PRINT 1\n10 PRINT 2").is_err());
        assert!(tokenise_listing("PRINT 1").is_err());
        assert!(tokenise_listing("10 PRINT \"oops").is_err());
        assert!(tokenise_listing("10 PRINT 1e999").is_err());
        assert!(tokenise_listing("10 PRINT BIN 102").is_err());
    }

    #[test]
    fn lex_line_reports_kinds_and_columns() {
        // No space before THEN or REM: Task 5 stops storing those, and this test must hold either side of it.
        let line = lex_line("  20 IF a=1THEN PRINT \"x\":REM hi").expect("lex");
        assert_eq!(line.number, 20);
        assert_eq!(line.body_column, 5);
        let kinds: Vec<PieceKind> = line.pieces.iter().map(|p| p.kind).collect();
        assert_eq!(
            kinds,
            vec![
                PieceKind::Keyword(0xFA),
                PieceKind::Name,
                PieceKind::Punct,
                PieceKind::Number,
                PieceKind::Keyword(0xCB),
                PieceKind::Keyword(0xF5),
                PieceKind::Str,
                PieceKind::Punct,
                PieceKind::Keyword(0xEA),
                PieceKind::Rem,
            ]
        );
        let name = &line.pieces[1];
        assert_eq!((name.column, name.text.as_str()), (3, "a"));
    }

    #[test]
    fn pieces_concatenate_to_the_stored_bytes() {
        for src in [
            "10 PRINT \"GO TO 12\": REM PRINT 99",
            "20 IF x<>2 THEN PRINT x",
            "10 LET cat=2",
        ] {
            let lexed: Vec<u8> = lex_line(src)
                .expect("lex")
                .pieces
                .into_iter()
                .flat_map(|p| p.bytes)
                .collect();
            let stored = tokenise_listing(src).expect("tokenise").bytes;
            assert_eq!(&stored[4..stored.len() - 1], lexed.as_slice(), "{src}");
        }
    }

    #[test]
    fn errors_carry_the_source_line_separately_from_the_message() {
        let source = "10 PRINT 1\n\n30 PRINT \"x";
        let expected = ListingError {
            line: 3,
            message: "missing closing quotation mark".into(),
        };
        assert_eq!(tokenise_listing(source).err(), Some(expected.clone()));
        assert_eq!(listed_form(source).err(), Some(expected));
        let alone = lex_line("30 PRINT \"x").expect_err("error");
        assert_eq!(alone.line, 0);
        let empty = tokenise_listing("\n").expect_err("error");
        assert_eq!(
            (empty.line, empty.to_string().as_str()),
            (0, "Enter at least one numbered BASIC line")
        );
    }

    #[test]
    fn blank_lines_are_skipped_and_comment_lines_are_errors() {
        assert!(tokenise_listing("\n  \n10 CLS\n").is_ok());
        for source in ["# comment\n10 CLS", "10 CLS\n# comment"] {
            let line = if source.starts_with('#') { 1 } else { 2 };
            assert_eq!(tokenise_listing(source).expect_err(source).line, line);
            assert_eq!(listed_form(source).expect_err(source).line, line);
        }
    }

    #[test]
    fn crlf_and_trailing_blank_lines_tokenise_like_lf() {
        let lf = tokenise_listing("10 PRINT 1\n20 GO TO 10\n")
            .expect("lf")
            .bytes;
        let crlf = tokenise_listing("10 PRINT 1\r\n20 GO TO 10\r\n\r\n")
            .expect("crlf")
            .bytes;
        assert_eq!(lf, crlf);
    }

    #[test]
    fn a_space_the_rom_supplies_before_a_keyword_is_not_stored() {
        let spaced = tokenise_listing("10 IF a=1 THEN STOP")
            .expect("spaced")
            .bytes;
        let tight = tokenise_listing("10 IF a=1THEN STOP").expect("tight").bytes;
        assert_eq!(spaced, tight);
    }

    #[test]
    fn spaces_the_rom_does_not_supply_are_kept() {
        // `<=` gets no leading space from the ROM, and strings/REM are content.
        let b = tokenise_listing("10 IF a <= b THEN PRINT \"a  THEN\": REM  x")
            .expect("t")
            .bytes;
        assert!(
            b.windows(2).any(|w| w == [b' ', 0xC7]),
            "space before <= kept"
        );
        assert!(b.windows(7).any(|w| w == b"a  THEN"), "string untouched");
    }

    #[test]
    fn listed_form_round_trips_canonical_lines() {
        let src =
            "  10 PRINT CHR$ (147)\n  20 IF a=1 THEN GO TO 20\n  30 PRINT \"a = b\";INKEY$;RND";
        let listed = listed_form(src).expect("listed");
        assert_eq!(listed.len(), src.lines().count());
        for ((_, got), want) in listed.iter().zip(src.lines()) {
            assert_eq!(got.trim_end(), want);
        }
    }

    #[test]
    fn a_space_after_a_no_trailing_space_keyword_is_kept() {
        // RND, INKEY$ and PI get no automatic trailing space from the ROM, so a
        // source space after them must be stored and must list back unchanged.
        let with_space = tokenise_listing("10 PRINT RND * 4").expect("spaced").bytes;
        let tight = tokenise_listing("10 PRINT RND*4").expect("tight").bytes;
        assert_ne!(with_space, tight);
        assert!(
            !tight.windows(2).any(|w| w == [0xA5, b' ']),
            "no space stored after RND when source has none"
        );
        assert!(
            with_space.windows(2).any(|w| w == [0xA5, b' ']),
            "space stored after RND when source has one"
        );
        let listed = listed_form("10 PRINT RND * 4").expect("listed");
        assert_eq!(listed[0].1.trim_end(), "  10 PRINT RND * 4");
    }

    #[test]
    fn open_and_close_take_a_stream_number_directly_after_the_hash() {
        for (src, token) in [("10 OPEN #4,\"s\"", 0xD3), ("10 CLOSE #4", 0xD4)] {
            let bytes = tokenise_listing(src).expect("tokenises").bytes;
            assert_eq!(bytes[4], token, "{src}");
            assert_eq!(bytes[5], b'4', "{src}");
        }
        let spaced = tokenise_listing("10 OPEN # 4,\"s\"").expect("spaced").bytes;
        let tight = tokenise_listing("10 OPEN #4,\"s\"").expect("tight").bytes;
        assert_eq!(spaced, tight);
        assert_eq!(
            listed_form("10 OPEN # 4,\"s\"").expect("listed")[0].1,
            "  10 OPEN #4,\"s\""
        );
    }

    /// The listed form of each line, re-joined as a listing.
    fn relisted(source: &str) -> String {
        listed_form(source)
            .expect("listed")
            .into_iter()
            .map(|(_, line)| line + "\n")
            .collect()
    }

    #[test]
    fn listed_form_is_idempotent() {
        let cases = include_str!("../tests/fixtures/rom-list/cases.bas");
        let tricky = [
            "10 OPEN # 4,\"s\":CLOSE # 4",
            "10 OPEN #4:CLOSE #4",
            "10 IF a  THEN STOP",
            "10 PRINT 1:  REM  x  ",
            "10 PRINT RND * 4;INKEY$ ;PI ",
            "10 IF a=1THEN GO TO 20",
            "10 SAVE \"x\"LINE 10",
            "10 LLIST : STOP",
            "10 PRINT a<=b ; a >= b;a <> b",
            "10 FOR i=1TO 9STEP 2",
            "10 PRINT \"  THEN  \"",
            "10 PRINT 1e3;.5;BIN 101",
        ];
        for source in cases.lines().chain(tricky) {
            let once = relisted(source);
            assert_eq!(relisted(&once), once, "{source}");
        }
    }

    #[test]
    fn two_spaces_before_a_keyword_round_trip_unchanged() {
        // The ROM only ever adds ONE leading space, so a genuine two-space
        // gap before a keyword must survive as two stored spaces, not one.
        let src = "  10 IF a  THEN STOP\n  20 PRINT 1:  REM x";
        let listed = listed_form(src).expect("listed");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].1.trim_end(), "  10 IF a  THEN STOP");
        assert_eq!(listed[1].1.trim_end(), "  20 PRINT 1:  REM x");
    }
}
