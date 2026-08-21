// Error types for parsing

use std::fmt;

use thiserror::Error;

/// Rich error context with source snippet and position
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    /// The source line containing the error
    pub source_line: String,
    /// Column position within the line (0-indexed)
    pub column: usize,
    /// Line number in the source (1-indexed)
    pub line_number: usize,
}

impl ErrorContext {
    /// Extract error context from source code at a given byte position
    ///
    /// Returns None if position is out of bounds or source is empty
    pub fn from_source(source: &str, position: usize) -> Option<Self> {
        if source.is_empty() || position > source.len() {
            return None;
        }

        // `position` is a byte offset that can land *inside* a multibyte char — a
        // lexer/parser error position on malformed multibyte input. Error
        // formatting must be total, so floor it to the nearest char boundary
        // before slicing (every slice below would otherwise panic).
        let mut position = position.min(source.len());
        while position > 0 && !source.is_char_boundary(position) {
            position -= 1;
        }

        // The line's bounds and number, over the ECMAScript terminator class (`\n`, `\r`,
        // `\r\n`, `<LS>`, `<PS>`) rather than `\n` alone — one question, stated once, in
        // `printing` beside the class itself.
        let (line_start, line_end, line_number) = crate::printing::line_bounds_at(source, position);

        // Extract the line
        let source_line = source[line_start..line_end].to_string();

        // Calculate column (CHARACTERS from line start to error position, matching the
        // character-based columns the wire AST reports — a byte count puts the caret past
        // its token on any line with a multi-byte character ahead of the error). Clamped to
        // the line's own end, which `position` can exceed only by sitting inside the
        // terminator sequence that ends it.
        let column = source[line_start..position.min(line_end)].chars().count();

        Some(ErrorContext {
            source_line,
            column,
            line_number,
        })
    }

    /// Format error context with caret pointer
    pub fn format_with_caret(&self, message: &str) -> String {
        // The pad reproduces what the excerpt PRINTS ahead of the error, not how many
        // characters it holds. Two independent reasons a character count is wrong: a CJK
        // character occupies two columns, and a tab occupies however many the *terminal*
        // says — its stops are absolute, so no fixed width can stand in for one. So a tab
        // is echoed AS a tab (both lines then reach the same stop, whatever it is) and
        // everything else is padded by its display width.
        let header = format!("{}:{}", self.line_number, self.column + 1);
        // Everything printed ahead of the excerpt: the whole `{line}:{col}` header plus its
        // one separating space. Measuring only `{line}:` left the caret short by the
        // column's own digits at every position past column 9.
        let mut indent = " ".repeat(header.chars().count() + 1);
        let prefix: String = self.source_line.chars().take(self.column).collect();
        for (i, segment) in prefix.split('\t').enumerate() {
            if i > 0 {
                indent.push('\t');
            }
            // Tab-free by construction, so the tab width passed here is unreachable.
            for _ in 0..crate::printing::visual_width(segment, crate::config::TAB_WIDTH) {
                indent.push(' ');
            }
        }
        format!("{message}\n{header} {}\n{indent}^ here", self.source_line,)
    }
}

/// Format error message with context (caret pointer) or position fallback
fn format_error(base_msg: &str, position: usize, context: Option<&ErrorContext>) -> String {
    if let Some(ctx) = context {
        ctx.format_with_caret(base_msg)
    } else {
        format!("{base_msg} at position {position}")
    }
}

/// The error payload, behind the `Box` in [`ParseError`]. Private: `ParseError` is
/// construction-only outside this module — nothing matches a variant or reads a field —
/// so the constructor functions below are the whole API, and keeping the enum private
/// makes them the only way in (which is what keeps `context` uniformly `None` at
/// construction, filled later by [`ParseError::with_context`]).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
enum ParseErrorKind {
    #[error("{}", format_error(&format!("Expected {expected}, found {found}"), *position, context.as_ref()))]
    UnexpectedToken {
        expected: String,
        found: String,
        position: usize,
        context: Option<ErrorContext>,
    },
    #[error("{}", format_error("Unexpected end of file", *position, context.as_ref()))]
    UnexpectedEof {
        position: usize,
        context: Option<ErrorContext>,
    },
    #[error("{}", format_error(message, *position, context.as_ref()))]
    InvalidSyntax {
        message: String,
        position: usize,
        context: Option<ErrorContext>,
    },
    #[error("{}", format_error(&format!("Expected expression, found {found}"), *position, context.as_ref()))]
    InvalidExpression {
        found: String,
        position: usize,
        context: Option<ErrorContext>,
    },
    // Constructed only by `ensure_source_fits`, which every public parse entry point
    // calls before touching the source — that guard is what makes `Token`/`Span`'s
    // `u32` offsets sound (see `tsv_ts/src/lexer/token.rs`).
    #[error("File too large: {size} bytes (maximum: {max} bytes / 4GB)")]
    FileTooLarge { size: usize, max: usize },
}

/// A parse error — **8 bytes**, because the payload lives behind the `Box`.
///
/// The size is the point. A `Result<T, E>` is sized by `max(T, E)`, so an inline
/// [`ParseErrorKind`] (96 bytes) makes *every* fallible function whose success payload is
/// smaller than that return 96 bytes through memory on its hot `Ok` path — and the parsers
/// are full of `Result<(), _>`, `Result<bool, _>`, `Result<usize, _>`. Boxing the payload
/// once, here, shrinks the `Result` at every one of those call sites in all three language
/// crates without a single signature mentioning a `Box`.
///
/// The larger effect is on code size rather than the data path: the error half of each
/// `Result` shrinks at every site, including the many whose `Result` size never changed
/// because a fat AST node (`Statement`, `Expression`) already dominated it, so the hot
/// parse loops pack into less instruction cache.
///
/// `Display` and `Debug` forward to the inner kind, so rendered messages and debug output
/// are exactly what the enum produces.
#[derive(Clone, PartialEq, Eq)]
pub struct ParseError(Box<ParseErrorKind>);

// The whole point of the newtype — guard it. `Box` is non-null, so the niche also carries
// `Result<(), ParseError>` down to a bare pointer.
const _: () = assert!(size_of::<ParseError>() == size_of::<*const ()>());
const _: () = assert!(size_of::<Result<()>>() == size_of::<*const ()>());

impl fmt::Display for ParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for ParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ParseError {}

/// Result type alias for parsing operations
pub type Result<T> = std::result::Result<T, ParseError>;

/// Construct a lexer error. `#[cold]` / `#[inline(never)]` outlines the construction so it
/// never bloats the inlined token-scan fast path. Shared by all three language lexers
/// (`tsv_ts`, `tsv_css`, `tsv_svelte`).
#[cold]
#[inline(never)]
pub fn lex_err(message: impl Into<String>, position: usize) -> ParseError {
    ParseError::invalid_syntax(message.into(), position)
}

impl ParseError {
    #[inline]
    fn new(kind: ParseErrorKind) -> Self {
        ParseError(Box::new(kind))
    }

    /// A general parse error at `position`.
    pub fn invalid_syntax(message: String, position: usize) -> Self {
        ParseError::new(ParseErrorKind::InvalidSyntax {
            message,
            position,
            context: None,
        })
    }

    /// Found `found` where `expected` was required.
    pub fn unexpected_token(expected: String, found: String, position: usize) -> Self {
        ParseError::new(ParseErrorKind::UnexpectedToken {
            expected,
            found,
            position,
            context: None,
        })
    }

    /// Input ended while more was required.
    pub fn unexpected_eof(position: usize) -> Self {
        ParseError::new(ParseErrorKind::UnexpectedEof {
            position,
            context: None,
        })
    }

    /// Found `found` where an expression was required.
    pub fn invalid_expression(found: String, position: usize) -> Self {
        ParseError::new(ParseErrorKind::InvalidExpression {
            found,
            position,
            context: None,
        })
    }

    /// Source exceeds the 4 GB cap the `u32` span offsets assume.
    pub fn file_too_large(size: usize, max: usize) -> Self {
        ParseError::new(ParseErrorKind::FileTooLarge { size, max })
    }

    /// Reject a source longer than the `u32` span offsets can index (> 4 GiB − 1).
    /// Every public parse entry point calls this before touching the source; the
    /// lexers and `Span`/`Token` assume the cap holds rather than re-checking.
    pub fn ensure_source_fits(source: &str) -> Result<()> {
        const MAX: usize = u32::MAX as usize;
        if source.len() > MAX {
            return Err(ParseError::file_too_large(source.len(), MAX));
        }
        Ok(())
    }

    /// Lift a position out of a lexer's own coordinates into the document's.
    ///
    /// A [`lex_err`] position indexes the lexer's `source`, which is routinely a **slice**
    /// of the document the error is finally rendered against: a Svelte `<script>` /
    /// `<style>` island, the CSS declaration-value scan (`source[from..]`), the Svelte
    /// parser's own reseek after a jumped scan. [`ParseError::with_context`] is handed the
    /// whole document, so a slice-local position points at the wrong construct — an error
    /// on line 4 of a component rendered against line 1, out in the markup.
    ///
    /// Each lexer applies this **once**, at the entry point that PRODUCES the error; a
    /// wrapper that delegates to such an entry point must not re-apply it. A double shift
    /// runs the position past the end of the source, where `ErrorContext::from_source`
    /// returns `None` and the caret disappears entirely.
    ///
    /// The parser side needs none of this: its positions are already host coordinates
    /// (each parser's `current_pos` adds the same `base_offset`), which is why parser
    /// errors were always right and only lexer errors drifted.
    #[cold]
    #[inline(never)]
    pub fn shift_position(mut self, base_offset: usize) -> Self {
        match &mut *self.0 {
            ParseErrorKind::UnexpectedToken { position, .. }
            | ParseErrorKind::UnexpectedEof { position, .. }
            | ParseErrorKind::InvalidSyntax { position, .. }
            | ParseErrorKind::InvalidExpression { position, .. } => *position += base_offset,
            // No position to shift.
            ParseErrorKind::FileTooLarge { .. } => {}
        }
        self
    }

    /// Add source context to an error
    ///
    /// Call this to enrich errors with source snippets for better debugging.
    /// Example:
    /// ```ignore
    /// let err = ParseError::unexpected_token(expected, found, position);
    /// let rich_err = err.with_context(source);
    /// ```
    pub fn with_context(mut self, source: &str) -> Self {
        // Filling in place rather than rebuilding the variant: the payload is already
        // boxed, so this is a write through the pointer instead of a 96-byte move.
        let (position, slot) = match &mut *self.0 {
            ParseErrorKind::UnexpectedToken {
                position, context, ..
            }
            | ParseErrorKind::UnexpectedEof { position, context }
            | ParseErrorKind::InvalidSyntax {
                position, context, ..
            }
            | ParseErrorKind::InvalidExpression {
                position, context, ..
            } => (*position, context),
            // FileTooLarge doesn't need context
            ParseErrorKind::FileTooLarge { .. } => return self,
        };
        *slot = ErrorContext::from_source(source, position);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Display` forwards to the boxed kind, so the rendered message must be exactly
    /// what the enum's `#[error(...)]` attributes produce — both the bare
    /// `at position N` fallback and the caret form `with_context` fills in. The
    /// error-message fixtures and `input_invalid_*` gate this at the product level;
    /// this pins it beside the forwarding impl.
    #[test]
    fn test_display_renders_through_the_newtype() {
        let source = "let x =\ny";

        let e = ParseError::invalid_syntax("bad token".to_string(), 4);
        assert_eq!(e.to_string(), "bad token at position 4");
        assert_eq!(
            e.with_context(source).to_string(),
            "bad token\n1:5 let x =\n        ^ here"
        );

        let e = ParseError::unexpected_token("';'".to_string(), "'y'".to_string(), 8);
        assert_eq!(e.to_string(), "Expected ';', found 'y' at position 8");
        assert_eq!(
            e.with_context(source).to_string(),
            "Expected ';', found 'y'\n2:1 y\n    ^ here"
        );

        assert_eq!(
            ParseError::unexpected_eof(9).to_string(),
            "Unexpected end of file at position 9"
        );
        assert_eq!(
            ParseError::invalid_expression("'='".to_string(), 6).to_string(),
            "Expected expression, found '=' at position 6"
        );
        assert_eq!(
            ParseError::file_too_large(5, 4).to_string(),
            "File too large: 5 bytes (maximum: 4 bytes / 4GB)"
        );

        // `with_context` on the context-free variant is a no-op, not a panic.
        assert_eq!(
            ParseError::file_too_large(5, 4)
                .with_context(source)
                .to_string(),
            "File too large: 5 bytes (maximum: 4 bytes / 4GB)"
        );

        // `Debug` forwards too, so it prints the kind — not a `ParseError(..)` wrapper.
        assert!(
            format!("{:?}", ParseError::unexpected_eof(9)).starts_with("UnexpectedEof"),
            "Debug must forward to the kind"
        );
    }

    /// The excerpt is bounded by the ECMAScript terminator class, not by `\n` alone. On a
    /// lone-`<CR>` source the `\n`-only reading made the whole file one line, so the line
    /// number was 1 whatever the position and the excerpt carried raw `<CR>`s — which a
    /// terminal renders by overwriting, hiding the very text the caret points into.
    #[test]
    fn context_bounds_the_line_on_every_terminator() {
        let cr = "let a = 1;\rlet b = 2;\r";
        let ctx = ErrorContext::from_source(cr, 15).expect("context");
        assert_eq!(ctx.source_line, "let b = 2;");
        assert_eq!(ctx.line_number, 2);
        assert_eq!(ctx.column, 4);

        // `<CR><LF>` is ONE terminator, so the second line is still line 2.
        let crlf = "let a = 1;\r\nlet b = 2;\r\n";
        let ctx = ErrorContext::from_source(crlf, 16).expect("context");
        assert_eq!(ctx.source_line, "let b = 2;");
        assert_eq!(ctx.line_number, 2);
        assert_eq!(ctx.column, 4);

        // `<LS>` and `<PS>` terminate a line for ECMAScript too.
        let ls = "let a = 1;\u{2028}let b = 2;";
        let ctx = ErrorContext::from_source(ls, 17).expect("context");
        assert_eq!(ctx.source_line, "let b = 2;");
        assert_eq!(ctx.line_number, 2);
        assert_eq!(ctx.column, 4);
    }

    /// A position INSIDE a terminator sequence — the byte between a `<CR>` and its `<LF>`
    /// — belongs to the line that sequence ends, and the excerpt stops before the `<CR>`
    /// rather than carrying it.
    #[test]
    fn context_of_a_position_inside_a_terminator_sequence() {
        let crlf = "let a = 1;\r\nlet b = 2;\r\n";
        let ctx = ErrorContext::from_source(crlf, 11).expect("context");
        assert_eq!(ctx.source_line, "let a = 1;");
        assert_eq!(ctx.line_number, 1);
        assert_eq!(ctx.column, 10);
    }

    /// The column counts CHARACTERS and the caret pads by DISPLAY width — the two differ
    /// from a byte count in opposite directions, and a byte count gets both wrong.
    #[test]
    fn caret_lands_under_its_token_past_a_multibyte_prefix() {
        // `à` is 2 bytes, 1 character, 1 column: a byte column would report 12 and pad one
        // space too far.
        let src = "const à = ;";
        let ctx = ErrorContext::from_source(src, src.find(';').expect("semi")).expect("context");
        assert_eq!(ctx.column, 10);
        assert_eq!(
            ctx.format_with_caret("bad"),
            "bad\n1:11 const à = ;\n               ^ here"
        );

        // A tab is 1 character and however many columns the terminal's stops give it, so
        // the pad echoes the tab itself rather than guessing a width for it.
        let src = "\tconst a = ;";
        let ctx = ErrorContext::from_source(src, src.find(';').expect("semi")).expect("context");
        assert_eq!(ctx.column, 11);
        assert_eq!(
            ctx.format_with_caret("bad"),
            "bad\n1:12 \tconst a = ;\n     \t          ^ here"
        );
    }

    #[test]
    fn test_error_context_at_eof_no_newline() {
        // Position at EOF, source doesn't end with newline
        let source = "hello";
        let ctx = ErrorContext::from_source(source, 5).unwrap();
        assert_eq!(ctx.source_line, "hello");
        assert_eq!(ctx.column, 5);
        assert_eq!(ctx.line_number, 1);
    }

    #[test]
    fn test_error_context_at_eof_with_newline() {
        // Position at EOF, source ends with newline
        let source = "hello\n";
        let ctx = ErrorContext::from_source(source, 6).unwrap();
        assert_eq!(ctx.source_line, ""); // Empty line after newline
        assert_eq!(ctx.column, 0);
        assert_eq!(ctx.line_number, 2);
    }

    #[test]
    fn test_error_context_middle_of_line() {
        let source = "abc\ndef\nghi";
        let ctx = ErrorContext::from_source(source, 5).unwrap(); // 'e' in "def"
        assert_eq!(ctx.source_line, "def");
        assert_eq!(ctx.column, 1);
        assert_eq!(ctx.line_number, 2);
    }

    #[test]
    fn test_error_context_start_of_file() {
        let source = "hello";
        let ctx = ErrorContext::from_source(source, 0).unwrap();
        assert_eq!(ctx.source_line, "hello");
        assert_eq!(ctx.column, 0);
        assert_eq!(ctx.line_number, 1);
    }

    #[test]
    fn test_error_context_position_inside_multibyte_char() {
        // A byte offset landing *inside* a multibyte char must not panic — it's
        // floored to the char boundary. `名` is 3 bytes (starts at byte 4).
        let source = "abc 名 def";
        for pos in 4..=6 {
            let ctx = ErrorContext::from_source(source, pos)
                .expect("in-bounds position yields a context");
            assert_eq!(ctx.source_line, source);
            assert_eq!(ctx.line_number, 1);
            // Floored to the char boundary at byte 4 (the start of `名`) — which is
            // character 4 as well, everything before it being ASCII.
            assert_eq!(ctx.column, 4);
        }
        // A boundary just past the multibyte char is kept as-is: byte 7, character 5.
        let ctx = ErrorContext::from_source(source, 7).unwrap();
        assert_eq!(ctx.column, 5);
    }

    #[test]
    fn test_error_context_empty_source() {
        assert!(ErrorContext::from_source("", 0).is_none());
    }

    #[test]
    fn test_error_context_position_out_of_bounds() {
        assert!(ErrorContext::from_source("hello", 10).is_none());
    }
}
