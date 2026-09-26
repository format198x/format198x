//! The typed failure for converting a text listing.

/// Why a text listing could not be tokenised, lexed or listed.
///
/// `line` is the 1-based line of the source text the problem is on, or 0
/// when it is not about one line (an empty or oversized program), or when
/// the function had no source context (a single line passed to `lex_line`).
/// `message` says what is wrong without repeating the line number, so a
/// caller can print `file.bas:LINE: message` without parsing text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingError {
    /// 1-based source line, or 0 when the error is not line-specific.
    pub line: usize,
    /// What is wrong, without the line number.
    pub message: String,
}

impl ListingError {
    /// An error on 1-based source line `line` (0 when not line-specific).
    pub(crate) fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl core::fmt::Display for ListingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.line == 0 {
            f.write_str(&self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for ListingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_prefixes_the_line_only_when_there_is_one() {
        assert_eq!(ListingError::new(3, "oops").to_string(), "line 3: oops");
        assert_eq!(ListingError::new(0, "oops").to_string(), "oops");
    }
}
