//! LIST as the 48K ROM prints it: PO-TOKENS/PO-SEARCH (0C10–0C54) for token
//! spacing, PO-CHAR (0B6A) for the leading-space flag, OUT-NUM-2 for the
//! four-column line number. Screen wrapping is not modelled.

use crate::ListingError;

/// Keyword text in ROM table order, codes 0xA5 (RND) to 0xFF (COPY): the
/// entry for token `t` is `KEYWORD_NAMES[usize::from(t - 0xA5)]`.
pub const KEYWORD_NAMES: [&str; 91] = [
    "RND",
    "INKEY$",
    "PI",
    "FN",
    "POINT",
    "SCREEN$",
    "ATTR",
    "AT",
    "TAB",
    "VAL$",
    "CODE",
    "VAL",
    "LEN",
    "SIN",
    "COS",
    "TAN",
    "ASN",
    "ACS",
    "ATN",
    "LN",
    "EXP",
    "INT",
    "SQR",
    "SGN",
    "ABS",
    "PEEK",
    "IN",
    "USR",
    "STR$",
    "CHR$",
    "NOT",
    "BIN",
    "OR",
    "AND",
    "<=",
    ">=",
    "<>",
    "LINE",
    "THEN",
    "TO",
    "STEP",
    "DEF FN",
    "CAT",
    "FORMAT",
    "MOVE",
    "ERASE",
    "OPEN #",
    "CLOSE #",
    "MERGE",
    "VERIFY",
    "BEEP",
    "CIRCLE",
    "INK",
    "PAPER",
    "FLASH",
    "BRIGHT",
    "INVERSE",
    "OVER",
    "OUT",
    "LPRINT",
    "LLIST",
    "STOP",
    "READ",
    "DATA",
    "RESTORE",
    "NEW",
    "BORDER",
    "CONTINUE",
    "DIM",
    "REM",
    "FOR",
    "GO TO",
    "GO SUB",
    "INPUT",
    "LOAD",
    "LIST",
    "LET",
    "PAUSE",
    "NEXT",
    "POKE",
    "PRINT",
    "PLOT",
    "RUN",
    "SAVE",
    "RANDOMIZE",
    "IF",
    "CLS",
    "DRAW",
    "CLEAR",
    "RETURN",
    "COPY",
];

/// The first token code, RND.
const FIRST_TOKEN: u8 = 0xA5;

/// Whether the ROM prints a space before `token` when the previous
/// character was not a space. PO-SEARCH returns carry (no space) for the
/// first 32 entries, RND to BIN, and for text that does not start with a
/// letter (`<=`, `>=`, `<>`).
pub(crate) fn rom_leading_space(token: u8) -> bool {
    let index = usize::from(token.saturating_sub(FIRST_TOKEN));
    token >= FIRST_TOKEN
        && index >= 0x20
        && KEYWORD_NAMES[index]
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic())
}

/// Whether the ROM prints a space after `token`. PO-TOKENS adds one when the
/// text ends in `$` or a letter, except for RND, INKEY$ and PI.
fn rom_trailing_space(token: u8) -> bool {
    let index = usize::from(token.saturating_sub(FIRST_TOKEN));
    token >= FIRST_TOKEN
        && index >= 3
        && KEYWORD_NAMES[index]
            .bytes()
            .last()
            .is_some_and(|b| b == b'$' || b >= b'A')
}

/// One line as LIST prints it, without screen wrapping. `body` runs up to but
/// excluding the line's `0x0D`.
///
/// Output is text, not a screen rendering. Token bytes (0xA5 and up) print
/// as keywords with the ROM's spacing, and each number's hidden five-byte
/// value is skipped; every other byte prints as the Unicode character with
/// the same code. That is right for printable ASCII (0x20–0x7E) except 0x60,
/// which the Spectrum shows as `£`. The rest is not rendered: 0x7F (`©`),
/// block graphics (0x80–0x8F), user-defined graphics (0x90–0xA4), and the
/// colour and position control codes 0x10–0x17 with their parameter bytes
/// all come out as raw code points. [`crate::tokenise_listing`] accepts only printable
/// ASCII, so its output never holds them.
pub fn list_line(number: u16, body: &[u8]) -> String {
    let mut out = format!("{number:>4}");
    // FLAGS bit 0 after OUT-LINE: reset, so a leading space is allowed.
    let mut suppress = false;
    let mut i = 0;
    while i < body.len() {
        let b = body[i];
        if b == 0x0E {
            i += 6; // hidden five-byte number
            continue;
        }
        if b >= FIRST_TOKEN {
            if rom_leading_space(b) && !suppress {
                out.push(' ');
            }
            out.push_str(KEYWORD_NAMES[usize::from(b - FIRST_TOKEN)]);
            // The trailing space goes through PO-CHAR, which sets bit 0;
            // any other last character resets it.
            suppress = rom_trailing_space(b);
            if suppress {
                out.push(' ');
            }
        } else {
            out.push(char::from(b));
            suppress = b == b' ';
        }
        i += 1;
    }
    out
}

/// Every line of a stored program, as LIST prints them. `program` holds, per
/// line, the number (big-endian), the length (little-endian), the body and
/// `0x0D`, as [`crate::tokenise_listing`] returns.
///
/// Listing stops, as the ROM's does (OUT-LINE1, 0x1865), at the end of the
/// bytes or at a line whose number's high byte is 0x40 or more: that byte
/// starts the variables area, so `program` may run on into VARS and the
/// `0x80` end marker, as in a memory dump or a tape's program block.
///
/// # Errors
/// Returns a [`ListingError`] if the bytes end partway through a line. It
/// reads bytes, not source text, so the error's `line` is 0.
pub fn list(program: &[u8]) -> Result<Vec<String>, ListingError> {
    let mut lines = Vec::new();
    let mut at = 0;
    while program.get(at).is_some_and(|&high| high < 0x40) {
        let header = program
            .get(at..at + 4)
            .ok_or_else(|| ListingError::new(0, "program ends inside a line header"))?;
        let number = u16::from_be_bytes([header[0], header[1]]);
        let length = usize::from(u16::from_le_bytes([header[2], header[3]]));
        let body = program
            .get(at + 4..at + 4 + length)
            .ok_or_else(|| ListingError::new(0, "program ends inside a line"))?;
        let body = body.strip_suffix(&[0x0D]).unwrap_or(body);
        lines.push(list_line(number, body));
        at += 4 + length;
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise_listing;

    fn listed(source: &str) -> Vec<String> {
        let program = tokenise_listing(source).expect("tokenises");
        list(&program.bytes).expect("lists")
    }

    #[test]
    fn functions_after_print_take_no_leading_space() {
        assert_eq!(listed("10 PRINT a;INKEY$;RND"), ["  10 PRINT a;INKEY$;RND"]);
    }

    #[test]
    fn statement_keywords_keep_their_trailing_space() {
        assert_eq!(listed("10 IF x<=y THEN STOP"), ["  10 IF x<=y THEN STOP "]);
    }

    #[test]
    fn rem_text_keeps_its_spaces() {
        assert_eq!(listed("9999 REM  two  spaces"), ["9999 REM  two  spaces"]);
    }

    #[test]
    fn keyword_names_follow_the_rom_table() {
        assert_eq!(KEYWORD_NAMES[0xF5 - 0xA5], "PRINT");
        assert_eq!(KEYWORD_NAMES[0xCB - 0xA5], "THEN");
        assert_eq!(KEYWORD_NAMES[0xEC - 0xA5], "GO TO");
    }

    #[test]
    fn listing_stops_at_the_variables_area() {
        let mut bytes = tokenise_listing("10 PRINT 1\n20 STOP")
            .expect("tokenises")
            .bytes;
        // A numeric variable `a` holding 5 (0x61 + five-byte integer form),
        // then the 0x80 end of the variables area.
        bytes.extend_from_slice(&[0x61, 0x00, 0x00, 0x05, 0x00, 0x00, 0x80]);
        assert_eq!(list(&bytes).expect("lists"), ["  10 PRINT 1", "  20 STOP "]);
        // The high byte alone decides: 0x80 straight after the program also ends it.
        assert_eq!(list(&[0x80]).expect("lists"), Vec::<String>::new());
    }

    #[test]
    fn lexer_keywords_agree_with_the_listing_names() {
        // The lexer's table (with its matching spaces) and the ROM-order
        // listing table are written separately; every token the lexer can
        // store must list back as the text it was matched from.
        for &(keyword, token, _) in crate::tokens::KEYWORDS {
            assert_eq!(
                KEYWORD_NAMES[usize::from(token - FIRST_TOKEN)],
                keyword.trim_end(),
                "token {token:#04X}"
            );
        }
    }

    #[test]
    fn truncated_programs_are_errors() {
        let program = tokenise_listing("10 PRINT 1").expect("tokenises");
        let bytes = &program.bytes;
        let header = list(&bytes[..3]).expect_err("header");
        assert_eq!(
            header,
            ListingError::new(0, "program ends inside a line header")
        );
        let body = list(&bytes[..bytes.len() - 1]).expect_err("body");
        assert_eq!(body.to_string(), "program ends inside a line");
    }
}
