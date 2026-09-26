//! Text-preserving conversion for editable listings. The ROM judges syntax;
//! unlike the analysis AST, this path never invents expressions or drops text.
use crate::error::ListingError;
use crate::list::{list_line, rom_leading_space};
use crate::tokens::KeywordRole;
use crate::{BasicProgram, rom_number, tokens::KEYWORDS};
use std::collections::BTreeMap;

/// One stored piece of a line body, with where it came from in the source.
///
/// Concatenating the `bytes` of a line's pieces gives the stored line body,
/// without its line number, length or closing `0x0D`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// What sort of text this piece is.
    pub kind: PieceKind,
    /// Bytes this piece stores (a token byte, text, or text + 0x0E + 5-byte float).
    pub bytes: Vec<u8>,
    /// 0-based byte offset of the piece within the line body (the text after
    /// the line number and its following spaces). Add
    /// [`LexLine::body_column`] for the offset within the source line.
    pub column: usize,
    /// The source text the piece came from. A keyword's text is as typed
    /// (any case, without the spaces the lexer absorbs around it).
    pub text: String,
}

/// What sort of text a [`Piece`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PieceKind {
    /// A keyword stored as one token byte, which this carries: 0xA5 (RND)
    /// to 0xFF (COPY), including the operators `<=` (0xC7), `>=` (0xC8)
    /// and `<>` (0xC9). [`crate::KEYWORD_NAMES`] gives its listed text.
    Keyword(u8),
    /// A variable or other name, stored as its characters, including any
    /// spaces inside it (`a 1` is the variable `a1`).
    Name,
    /// A number: its spelling as typed, including spaces inside it and any
    /// after it, then 0x0E and its hidden five-byte value. After a `BIN`
    /// with no digits, which the ROM gives the value 0, the spelling is only
    /// the spaces after `BIN`, and may be empty.
    Number,
    /// A string literal, including both quotation marks.
    Str,
    /// The text after REM, to the end of the line, stored as typed.
    Rem,
    /// One stored space.
    Space,
    /// Any other single character: punctuation, operators, and characters
    /// the ROM will judge, such as a stray `$`.
    Punct,
}

/// A listing line split into number and pieces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexLine {
    /// The line number, 1 to 9999.
    pub number: u16,
    /// 0-based byte offset of the body within the source line.
    pub body_column: usize,
    /// The body's pieces, in source order.
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
    if end == 0 {
        return Err(fail("start with a line number from 1 to 9999"));
    }
    // Any numeral outside 1 to 9999 gets the same message, whether or not
    // it fits in a u16.
    let number = text[..end]
        .parse::<u16>()
        .ok()
        .filter(|n| (1..=9999).contains(n))
        .ok_or_else(|| fail("line number must be 1 to 9999"))?;
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
/// need word boundaries; strings and REM text stay literal. Numeric
/// spellings are retained with their hidden five-byte values appended, after
/// any spaces that follow the number, where the ROM's editor puts them.
/// Spaces inside numbers and names are read as the ROM reads them: `1.5 E3`
/// is one number, `a 1` the variable `a1`. This bounded editor route
/// excludes DEF FN (which needs parameter markers).
///
/// # Errors
/// Returns an error for unsupported characters, malformed numbers/strings
/// (including a space in a number's whole part, as in `1 000`),
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
/// Each line is exactly what [`crate::list_line`] prints, including the
/// space the ROM prints after a keyword that ends the line (`  10 STOP `).
/// Callers comparing against source text usually want to trim it.
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
    let mut items = Items::None;
    let mut variable = false;
    while pos < bytes.len() {
        let ch = bytes[pos];
        if variable && ch.is_ascii_alphabetic() {
            let start = pos;
            pos = name_end(source, pos, statement_start, items);
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
        } else if let Some((keyword, token)) = keyword_at(source, pos, statement_start, items) {
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
                items = Items::after(token);
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
            if binary
                && !bytes
                    .get(skip_spaces(bytes, pos))
                    .is_some_and(u8::is_ascii_digit)
            {
                // BIN with no digits: BIN-END (2CB3) stacks zero, and S-BIN,
                // which is S-DECIMAL, inserts it as for any number.
                let (piece, end) = number_piece(
                    source,
                    pos,
                    pos,
                    rom_number::bin_to_fp(0),
                    statement_start,
                    items,
                );
                out.push(piece);
                pos = end;
                binary = false;
            }
        } else if ch.is_ascii_alphabetic() {
            let start = pos;
            pos = name_end(source, pos, statement_start, items);
            out.push(Piece {
                kind: PieceKind::Name,
                bytes: bytes[start..pos].to_vec(),
                column: start,
                text: source[start..pos].to_string(),
            });
            binary = false;
        } else if ch.is_ascii_digit()
            || (ch == b'.'
                && bytes
                    .get(skip_spaces(bytes, pos + 1))
                    .is_some_and(u8::is_ascii_digit))
        {
            let start = pos;
            pos = number_end(source, pos, binary, statement_start, items)?;
            let digits: String = source[start..pos].chars().filter(|c| *c != ' ').collect();
            // The hidden form is what the ROM's own arithmetic makes of the
            // digits (see `rom_number`), not the correctly rounded value.
            let hidden = if binary {
                u16::from_str_radix(&digits, 2)
                    .map(rom_number::bin_to_fp)
                    .map_err(|_| "BIN needs up to 16 binary digits")?
            } else {
                rom_number::dec_to_fp(&digits)
                    .map_err(|_| "number is outside the Spectrum's range")?
            };
            let (piece, end) = number_piece(source, start, pos, hidden, statement_start, items);
            out.push(piece);
            pos = end;
            binary = false;
        } else {
            if ch == b':' {
                statement_start = true;
                items = Items::None;
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

/// Which embedded items the current statement's keyword admits, beyond
/// ordinary expression keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Items {
    /// None: PRINT's items and colour items are ordinary names here.
    None,
    /// PRINT, LPRINT and INPUT: print items (AT, TAB) and colour items.
    Print,
    /// PLOT, DRAW and CIRCLE, whose syntax class CLASS-09 (1CBE) lets colour
    /// items (INK to OVER, tokens 0xD9–0xDE, CO-TEMP-3 at 21F2) precede the
    /// coordinates: `PLOT INK 2; OVER 1;x,y`.
    Colour,
}

impl Items {
    /// The items admitted by a statement that starts with `token`.
    fn after(token: u8) -> Self {
        match token {
            0xF5 | 0xEE | 0xE0 => Self::Print,
            0xF6 | 0xFC | 0xD8 => Self::Colour,
            _ => Self::None,
        }
    }

    /// Whether a PrintItem keyword with this token is admitted.
    fn allows(self, token: u8) -> bool {
        match self {
            Self::None => false,
            Self::Print => true,
            Self::Colour => (0xD9..=0xDE).contains(&token),
        }
    }
}

/// A Number piece for the spelling from `start` to `end`, taking in the
/// spaces after it, and where the piece ends. S-DECIMAL (268D) collects the
/// character after a number with GET-CHAR, which skips spaces, and opens the
/// room for the hidden form there, so spaces after a number are stored
/// before its 0x0E. A single space before a keyword the ROM spaces itself on
/// LIST is left for the keyword branch to drop.
fn number_piece(
    source: &str,
    start: usize,
    end: usize,
    hidden: [u8; 5],
    statement_start: bool,
    items: Items,
) -> (Piece, usize) {
    let after = skip_spaces(source.as_bytes(), end);
    let dropped_before_keyword = after == end + 1
        && keyword_at(source, after, statement_start, items)
            .is_some_and(|(_, token)| rom_leading_space(token));
    let end = if dropped_before_keyword { end } else { after };
    let spelling = &source[start..end];
    let mut bytes = spelling.as_bytes().to_vec();
    bytes.push(14);
    bytes.extend_from_slice(&hidden);
    let piece = Piece {
        kind: PieceKind::Number,
        bytes,
        column: start,
        text: spelling.to_string(),
    };
    (piece, end)
}

/// The first position at or after `pos` that is not a space.
fn skip_spaces(bytes: &[u8], pos: usize) -> usize {
    pos + bytes
        .get(pos..)
        .map_or(0, |rest| rest.iter().take_while(|b| **b == b' ').count())
}

/// The keyword this lexer tokenises at `pos`, given the statement context.
fn keyword_at(
    source: &str,
    pos: usize,
    statement_start: bool,
    items: Items,
) -> Option<(&'static str, u8)> {
    let rest = source.as_bytes().get(pos..)?;
    KEYWORDS
        .iter()
        .find(|(keyword, token, role)| {
            let allowed = match role {
                KeywordRole::Statement | KeywordRole::RestOfLine => statement_start,
                KeywordRole::PrintItem => statement_start || items.allows(*token),
                _ => true,
            };
            if !allowed {
                return false;
            }
            let word = keyword.trim_end();
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
        })
        .map(|&(keyword, token, _)| (keyword, token))
}

/// Where a variable name starting at `pos` ends. LOOK-VARS (28B2) reads a
/// name with NEXT-CHAR (V-CHAR, 28D4), which skips spaces, so `a 1` and
/// `a b` are the names `a1` and `ab`: the name runs on across spaces to a
/// following letter or
/// digit, unless that letter starts a keyword (typed on a Spectrum, it would
/// be a token, not letters). A name ending in `$` is a string name and stops.
fn name_end(source: &str, mut pos: usize, statement_start: bool, items: Items) -> usize {
    let bytes = source.as_bytes();
    loop {
        while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'$') {
            pos += 1;
        }
        if bytes[pos - 1] == b'$' {
            return pos;
        }
        let next = skip_spaces(bytes, pos);
        let continues = bytes.get(next).is_some_and(|b| {
            b.is_ascii_digit()
                || (b.is_ascii_alphabetic()
                    && keyword_at(source, next, statement_start, items).is_none())
        });
        if next == pos || !continues {
            return pos;
        }
        pos = next;
    }
}

/// Digits from `pos`, running on across spaces to a further digit when
/// `spaced`: the ROM's digit loops that step with NEXT-CHAR skip spaces, but
/// INT-TO-FP (2D3B) steps with CH-ADD+1 (0074), which does not.
fn digits_end(bytes: &[u8], mut pos: usize, spaced: bool) -> usize {
    loop {
        while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        let next = skip_spaces(bytes, pos);
        if !spaced || next == pos || !bytes.get(next).is_some_and(u8::is_ascii_digit) {
            return pos;
        }
        pos = next;
    }
}

/// Where a number starting at `pos` ends, before any spaces that follow it,
/// following DEC-TO-FP (2C9B):
///
/// - BIN digits (BIN-DIGIT, 2CA2) step with NEXT-CHAR, so spaces between
///   them are skipped: `BIN 1 0 1` is 5.
/// - Integer and exponent digits (INT-TO-FP, 2D3B) step with CH-ADD+1 and
///   stop at a space: `1 000` is the number 1 followed by `000`, which the
///   ROM's syntax check rejects, so a digit or point after a number is an
///   error rather than a second number.
/// - After the point, NEXT-CHAR and GET-CHAR (DECIMAL, 2CCB; NXT-DGT-1, 2CDA)
///   skip spaces: `1. 5`, `1.5 5` and `. 5` are all one number, and so is the
///   `E` part of `1.5 E3`.
/// - E-FORMAT (2CEB) tests the character INT-TO-FP stopped at, so without a
///   point the `E` must follow the digits directly: `1 E3` is not 1000.
/// - After `E`, NEXT-CHAR skips spaces up to the sign and the first exponent
///   digit (SIGN-FLAG, 2CF2); the exponent's digits go through INT-TO-FP.
fn number_end(
    source: &str,
    pos: usize,
    binary: bool,
    statement_start: bool,
    items: Items,
) -> Result<usize, String> {
    let bytes = source.as_bytes();
    if binary {
        return Ok(digits_end(bytes, pos, true));
    }
    let mut pos = digits_end(bytes, pos, false);
    let point = bytes.get(pos) == Some(&b'.');
    if point {
        pos += 1;
        let next = skip_spaces(bytes, pos);
        if bytes.get(next).is_some_and(u8::is_ascii_digit) {
            pos = digits_end(bytes, next, true);
        }
    }
    let e_at = if point { skip_spaces(bytes, pos) } else { pos };
    // An `E` that starts a keyword (`EXP`) would be a token on a Spectrum.
    if bytes
        .get(e_at)
        .is_some_and(|b| b.eq_ignore_ascii_case(&b'e'))
        && keyword_at(source, e_at, statement_start, items).is_none()
    {
        pos = skip_spaces(bytes, e_at + 1);
        if bytes.get(pos).is_some_and(|b| *b == b'+' || *b == b'-') {
            pos = skip_spaces(bytes, pos + 1);
        }
        let digits = pos;
        pos = digits_end(bytes, pos, false);
        if pos == digits {
            return Err("an exponent needs digits after E".into());
        }
    }
    // Whatever follows ends the number; a digit or point there starts
    // another, which the ROM's syntax check rejects (`1 000`, `1E3 3`).
    if bytes
        .get(skip_spaces(bytes, pos))
        .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
    {
        return Err("remove the space inside this number: the ROM ends a number's whole part or exponent at a space".into());
    }
    Ok(pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_out_of_range_line_number_gets_the_same_message() {
        for numeral in ["0", "10000", "65535", "70000", "99999999999"] {
            let error = lex_line(&format!("{numeral} PRINT 1")).expect_err(numeral);
            assert_eq!(error.message, "line number must be 1 to 9999", "{numeral}");
        }
        let error = lex_line("PRINT 1").expect_err("no line number");
        assert_eq!(error.message, "start with a line number from 1 to 9999");
    }
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
            "10 PRINT 1.5 E3;. 5;BIN 1 0 1 ;7  ;1 :PRINT 2",
            "10 PRINT BIN ;BIN  ;BIN",
            "10 LET a 1=2: PRINT a1;a b",
            "10 IF a=1  THEN STOP",
            "10 PLOT INK 7; OVER 1;x,y: DRAW PAPER 1;3,4: CIRCLE BRIGHT 1;9,9,5",
        ];
        for source in cases.lines().chain(tricky) {
            let once = relisted(source);
            assert_eq!(relisted(&once), once, "{source}");
        }
    }

    /// The pieces of one line's body, as (kind, text) pairs.
    fn pieces(line: &str) -> Vec<(PieceKind, String)> {
        lex_line(line)
            .expect("lex")
            .pieces
            .into_iter()
            .map(|p| (p.kind, p.text))
            .collect()
    }

    /// A number piece's stored bytes: its text, 0x0E and the value's form.
    fn number(text: &str, hidden: [u8; 5]) -> Vec<u8> {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(14);
        bytes.extend_from_slice(&hidden);
        bytes
    }

    /// A small integer's hidden form.
    fn int(n: u8) -> [u8; 5] {
        [0, 0, n, 0, 0]
    }

    #[test]
    fn spaces_after_the_point_stay_inside_one_number() {
        // DEC-TO-FP reads a number's fraction and E part with NEXT-CHAR,
        // which skips spaces, so each of these is one number with one hidden
        // value, stored with its spaces as typed.
        // The hidden forms are what the ROM stored for each, typed in.
        for (src, text, hidden) in [
            ("10 PRINT 1.5 E3", "1.5 E3", [0x8B, 0x3B, 0x80, 0, 0]),
            ("10 PRINT 1. 5", "1. 5", [0x81, 0x40, 0, 0, 0]),
            ("10 PRINT 1.5 5", "1.5 5", [0x81, 0x46, 0x66, 0x66, 0x66]),
            ("10 PRINT . 5", ". 5", [0x7F, 0x7F, 0xFF, 0xFF, 0xFF]),
            ("10 PRINT 12.5 E 2", "12.5 E 2", [0x8B, 0x1C, 0x40, 0, 0]),
            (
                "10 PRINT 1.5E- 3",
                "1.5E- 3",
                [0x77, 0x44, 0x9B, 0xA5, 0xE3],
            ),
            ("10 PRINT 1.5 e + 3", "1.5 e + 3", [0x8B, 0x3B, 0x80, 0, 0]),
            ("10 PRINT BIN 1 0 1", "1 0 1", int(5)),
        ] {
            let line = lex_line(src).expect(src);
            let numbers: Vec<&Piece> = line
                .pieces
                .iter()
                .filter(|p| p.kind == PieceKind::Number)
                .collect();
            assert_eq!(numbers.len(), 1, "{src}");
            assert_eq!(numbers[0].text, text, "{src}");
            assert_eq!(numbers[0].bytes, number(text, hidden), "{src}");
        }
    }

    #[test]
    fn a_space_in_a_whole_part_or_exponent_ends_the_number() {
        // INT-TO-FP steps with CH-ADD+1, which does not skip spaces; the ROM
        // then rejects the digits that follow, and so does the tokeniser.
        for src in [
            "10 PRINT 1 000",
            "10 PRINT 1 .5",
            "10 PRINT 1E3 3",
            "10 PRINT 1.5 .5",
        ] {
            assert!(tokenise_listing(src).is_err(), "{src}");
            assert!(listed_form(src).is_err(), "{src}");
        }
        // An E after a space is not an exponent without a point before it.
        assert_eq!(
            pieces("10 PRINT 1 E3"),
            vec![
                (PieceKind::Keyword(0xF5), "PRINT".to_string()),
                (PieceKind::Number, "1 ".to_string()),
                (PieceKind::Name, "E3".to_string()),
            ]
        );
    }

    #[test]
    fn spaces_after_a_number_are_stored_before_its_hidden_value() {
        // S-DECIMAL opens the room for the hidden form at GET-CHAR, past the
        // spaces after the number.
        let bytes = tokenise_listing("10 PRINT 1 :PRINT 7  ;2")
            .expect("t")
            .bytes;
        let mut want = vec![0xF5];
        want.extend(number("1 ", int(1)));
        want.extend([b':', 0xF5]);
        want.extend(number("7  ", int(7)));
        want.push(b';');
        want.extend(number("2", int(2)));
        want.push(13);
        assert_eq!(&bytes[4..], want.as_slice());
        // A single space before THEN is still the one LIST supplies, so it
        // is dropped; a second one is stored, before the hidden value.
        let one = tokenise_listing("10 IF a=1 THEN STOP").expect("t").bytes;
        assert!(one.windows(7).any(|w| w == number("1", int(1)).as_slice()));
        assert!(one.windows(2).all(|w| w != [b' ', 0xCB]));
        let two = tokenise_listing("10 IF a=1  THEN STOP").expect("t").bytes;
        assert!(
            two.windows(9)
                .any(|w| w == number("1  ", int(1)).as_slice())
        );
    }

    #[test]
    fn a_bare_bin_stores_a_hidden_zero() {
        // Typed in, `PRINT BIN;2` is stored C4 0E 00 00 00 00 00 ; ...
        let bytes = tokenise_listing("10 PRINT BIN ;2").expect("t").bytes;
        let mut want = vec![0xF5, 0xC4];
        want.extend(number("", int(0)));
        want.push(b';');
        want.extend(number("2", int(2)));
        want.push(13);
        assert_eq!(&bytes[4..], want.as_slice());
        assert_eq!(
            pieces("10 PRINT BIN")[2],
            (PieceKind::Number, String::new())
        );
        assert_eq!(
            listed_form("10 PRINT BIN ;2").expect("l")[0].1,
            "  10 PRINT BIN ;2"
        );
    }

    #[test]
    fn a_name_runs_on_across_spaces_to_a_letter_or_digit() {
        // LOOK-VARS reads a name with NEXT-CHAR, so `a 1` is the variable a1.
        assert_eq!(
            pieces("10 LET a 1=2: PRINT a1;a b"),
            vec![
                (PieceKind::Keyword(0xF1), "LET".to_string()),
                (PieceKind::Name, "a 1".to_string()),
                (PieceKind::Punct, "=".to_string()),
                (PieceKind::Number, "2".to_string()),
                (PieceKind::Punct, ":".to_string()),
                (PieceKind::Keyword(0xF5), "PRINT".to_string()),
                (PieceKind::Name, "a1".to_string()),
                (PieceKind::Punct, ";".to_string()),
                (PieceKind::Name, "a b".to_string()),
            ]
        );
        // A keyword ends the name; a string name and an array's bracket
        // are untouched.
        assert_eq!(
            pieces("10 IF a AND b THEN PRINT a$ ;x (1)")
                .into_iter()
                .filter(|(kind, _)| *kind == PieceKind::Name)
                .map(|(_, text)| text)
                .collect::<Vec<_>>(),
            ["a", "b", "a$", "x"]
        );
        let tight = tokenise_listing("10 FOR i=1 TO n STEP 2").expect("t").bytes;
        assert!(tight.contains(&0xCC) && tight.contains(&0xCD));
    }

    #[test]
    fn colour_items_after_plot_draw_and_circle_are_keywords() {
        // CLASS-09 (1CBE) lets INK to OVER precede PLOT, DRAW and CIRCLE's
        // coordinates, and the ROM's editor stores them as tokens.
        for (src, tokens) in [
            ("10 PLOT INK 7; OVER 1;x,y", vec![0xF6, 0xD9, 0xDE]),
            ("10 DRAW PAPER 6; BRIGHT 1;3,0", vec![0xFC, 0xDA, 0xDC]),
            ("10 CIRCLE FLASH 0; INVERSE 1;9,9,5", vec![0xD8, 0xDB, 0xDD]),
        ] {
            let kinds: Vec<u8> = lex_line(src)
                .expect(src)
                .pieces
                .iter()
                .filter_map(|p| match p.kind {
                    PieceKind::Keyword(token) => Some(token),
                    _ => None,
                })
                .collect();
            assert_eq!(kinds, tokens, "{src}");
        }
        // Only colour items: AT and TAB are PRINT items, not PLOT's.
        assert_eq!(
            pieces("10 PLOT at,1")[1],
            (PieceKind::Name, "at".to_string())
        );
        // A later statement without colour items does not inherit them.
        assert_eq!(
            pieces("10 PLOT 1,2: LET ink=3")[6],
            (PieceKind::Name, "ink".to_string())
        );
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
