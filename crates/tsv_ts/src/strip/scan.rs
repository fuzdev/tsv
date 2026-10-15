//! The strip's source scans: the bytes the AST holds no position for — a modifier keyword,
//! a `?` / `!` mark, a paren the parser stripped, the token after an assertion — located
//! through `tsv_lang::source_scan`, trivia-aware and bounded by the node at hand.

use tsv_lang::is_js_whitespace;
use tsv_lang::printing::has_newline_between;
use tsv_lang::source_scan::{
    TriviaProfile, find_char_skipping_comments, find_keyword, rfind_char_skipping_comments,
    rfind_keyword, skip_trivia_run, whole_word_at,
};

/// The source a walk reads, with the scans it asks of it.
#[derive(Clone, Copy)]
pub(super) struct Source<'s> {
    pub text: &'s str,
    pub bytes: &'s [u8],
}

impl<'s> Source<'s> {
    pub fn new(text: &'s str) -> Self {
        Self {
            text,
            bytes: text.as_bytes(),
        }
    }

    /// The byte before offset `end` — a node's last byte — is a `;`.
    pub fn ends_with_semicolon(self, end: u32) -> bool {
        end > 0 && self.bytes.get(end as usize - 1) == Some(&b';')
    }

    /// Whether an ECMAScript line terminator (LF, CR, U+2028, U+2029) lies in
    /// `[start, end)`, comments included.
    pub fn line_break_between(self, start: u32, end: u32) -> bool {
        has_newline_between(self.text, start, end)
    }

    /// The offset of the first significant byte at or after `from`: past whitespace, line
    /// terminators and comments. `self.text.len()` at the end of the source.
    pub fn next_significant(self, from: u32) -> u32 {
        skip_trivia_run(
            self.text,
            from as usize,
            TriviaProfile::COMMENTS,
            is_js_whitespace,
        ) as u32
    }

    /// How the first significant token at or after `from` would continue an expression left
    /// open on an earlier line, among the tokens a statement or a class member can open
    /// with; `None` when it would not (`++` / `--`, before which ASI ends the line, or
    /// anything else). A statement opening with `<` is a generic arrow (`<T>(x: T) => x`),
    /// whose type parameters blank to leave its `(` the line's first token, so it continues
    /// as a call's `(` does.
    pub fn continuation(self, from: u32) -> Option<Continuation> {
        let at = self.next_significant(from) as usize;
        match self.bytes.get(at)? {
            b'(' | b'[' | b'`' | b'<' => Some(Continuation::Postfix),
            b'/' | b'*' => Some(Continuation::Operator),
            &sign @ (b'+' | b'-') => {
                (self.bytes.get(at + 1) != Some(&sign)).then_some(Continuation::Operator)
            }
            b'i' => (whole_word_at(self.bytes, at, b"in")
                || whole_word_at(self.bytes, at, b"instanceof"))
            .then_some(Continuation::Operator),
            _ => None,
        }
    }

    /// The first `target` in `[start, end)` outside a comment.
    pub fn find_char(self, start: u32, end: u32, target: u8) -> Option<u32> {
        find_char_skipping_comments(self.bytes, start as usize, end as usize, target)
            .map(|at| at as u32)
    }

    /// The last `target` in `[start, end)` outside a comment.
    pub fn rfind_char(self, start: u32, end: u32, target: u8) -> Option<u32> {
        rfind_char_skipping_comments(self.bytes, start as usize, end as usize, target)
            .map(|at| at as u32)
    }

    /// The first whole-word `keyword` in `[start, end)` outside a comment or string.
    pub fn find_keyword(self, start: u32, end: u32, keyword: &[u8]) -> Option<u32> {
        find_keyword(
            self.bytes,
            start as usize,
            end as usize,
            keyword,
            TriviaProfile::JS,
        )
        .map(|at| at as u32)
    }

    /// The last whole-word `keyword` in `[start, end)` outside a comment or string.
    pub fn rfind_keyword(self, start: u32, end: u32, keyword: &[u8]) -> Option<u32> {
        rfind_keyword(
            self.bytes,
            start as usize,
            end as usize,
            keyword,
            TriviaProfile::JS,
        )
        .map(|at| at as u32)
    }

    /// The end of a `,` that directly follows offset `end` (past trivia), else `end` — the
    /// extent of a blanked list element that takes its trailing comma with it.
    pub fn end_with_trailing_comma(self, end: u32) -> u32 {
        let next = self.next_significant(end);
        if self.bytes.get(next as usize) == Some(&b',') {
            next + 1
        } else {
            end
        }
    }

    /// The precedence of the binary operator starting at the first significant byte at or
    /// after `from`, on ts-blank-space's scale (`**` 15 down to `??` 4); `None` when no
    /// binary operator starts there (an assignment operator, `=>`, a `?` / `?.`, `++` / `--`,
    /// a punctuator, or anything else). The lexing is longest-match, so `>>` is the shift
    /// operator and `>=` the comparison.
    pub fn binary_operator_precedence(self, from: u32) -> Option<u8> {
        let at = self.next_significant(from) as usize;
        let byte = |offset: usize| self.bytes.get(at + offset).copied();
        match byte(0)? {
            b'*' => match (byte(1), byte(2)) {
                (Some(b'*'), Some(b'=')) | (Some(b'='), _) => None,
                (Some(b'*'), _) => Some(15),
                _ => Some(14),
            },
            b'/' | b'%' => (byte(1) != Some(b'=')).then_some(14),
            sign @ (b'+' | b'-') => (byte(1) != Some(b'=') && byte(1) != Some(sign)).then_some(13),
            b'<' => match (byte(1), byte(2)) {
                (Some(b'<'), Some(b'=')) => None,
                (Some(b'<'), _) => Some(12),
                _ => Some(11),
            },
            b'>' => match (byte(1), byte(2), byte(3)) {
                (Some(b'>'), Some(b'>'), Some(b'=')) | (Some(b'>'), Some(b'='), _) => None,
                (Some(b'>'), _, _) => Some(12),
                _ => Some(11),
            },
            b'=' | b'!' => (byte(1) == Some(b'=')).then_some(10),
            b'&' => match (byte(1), byte(2)) {
                (Some(b'&'), Some(b'=')) | (Some(b'='), _) => None,
                (Some(b'&'), _) => Some(6),
                _ => Some(9),
            },
            b'^' => (byte(1) != Some(b'=')).then_some(8),
            b'|' => match (byte(1), byte(2)) {
                (Some(b'|'), Some(b'=')) | (Some(b'='), _) => None,
                (Some(b'|'), _) => Some(5),
                _ => Some(7),
            },
            b'?' => match (byte(1), byte(2)) {
                (Some(b'?'), Some(b'=')) => None,
                (Some(b'?'), _) => Some(4),
                _ => None,
            },
            b'i' => (whole_word_at(self.bytes, at, b"instanceof")
                || whole_word_at(self.bytes, at, b"in"))
            .then_some(11),
            _ => None,
        }
    }
}

/// How a next line's first token continues an expression ([`Source::continuation`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Continuation {
    /// A call's `(`, a member access's `[`, a tagged template's `` ` `` — and a generic
    /// arrow's `<`, whose `(` is what is left once its type parameters blank.
    Postfix,
    /// A binary operator: `+`, `-`, `/`, `*`, `in`, `instanceof`.
    Operator,
}

/// The precedence of `**` on [`Source::binary_operator_precedence`]'s scale.
pub(super) const EXPONENT_PRECEDENCE: u8 = 15;

#[cfg(test)]
mod tests {
    use super::Source;

    fn precedence(text: &str) -> Option<u8> {
        Source::new(text).binary_operator_precedence(0)
    }

    #[test]
    fn lexes_binary_operators_longest_match() {
        for (text, expected) in [
            ("**", Some(15)),
            ("*", Some(14)),
            ("/ c", Some(14)),
            ("%", Some(14)),
            ("+", Some(13)),
            ("-", Some(13)),
            ("<<", Some(12)),
            (">>", Some(12)),
            (">>>", Some(12)),
            ("<", Some(11)),
            ("<=", Some(11)),
            (">", Some(11)),
            (">=", Some(11)),
            ("instanceof", Some(11)),
            ("in x", Some(11)),
            ("==", Some(10)),
            ("!==", Some(10)),
            ("&", Some(9)),
            ("^", Some(8)),
            ("|", Some(7)),
            ("&&", Some(6)),
            ("||", Some(5)),
            ("??", Some(4)),
        ] {
            assert_eq!(precedence(text), expected, "{text}");
        }
    }

    #[test]
    fn rejects_everything_else() {
        for text in [
            "**=", "*=", "/=", "%=", "+=", "-=", "++", "--", "<<=", ">>=", ">>>=", "=", "=>", "&=",
            "&&=", "|=", "||=", "^=", "??=", "?", "?.", "!", ")", ";", "inner", "index", "",
        ] {
            assert_eq!(precedence(text), None, "{text}");
        }
    }

    #[test]
    fn continuation_tokens() {
        use super::Continuation::{Operator, Postfix};
        for (text, expected) in [
            ("(x)", Some(Postfix)),
            ("[k] = 1", Some(Postfix)),
            ("`t`", Some(Postfix)),
            ("<T>(x) => x", Some(Postfix)),
            ("/re/", Some(Operator)),
            ("*gen() {}", Some(Operator)),
            ("+x", Some(Operator)),
            ("- x", Some(Operator)),
            ("in = 1", Some(Operator)),
            ("instanceof()", Some(Operator)),
            ("++x", None),
            ("--x", None),
            ("inner", None),
            ("x", None),
            ("{}", None),
            ("", None),
        ] {
            assert_eq!(Source::new(text).continuation(0), expected, "{text}");
        }
    }

    #[test]
    fn skips_trivia_before_the_operator() {
        assert_eq!(precedence(" /* c */\n// d\n / e"), Some(14));
    }
}
