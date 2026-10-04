// Error types for parsing

use std::fmt;

use thiserror::Error;

use crate::location::{WireCoordinates, WirePoint};

/// Where a located error sits: its point in the document's wire coordinates and the
/// source line excerpted under the message.
///
/// The two answer different questions and are bounded differently on purpose. The
/// **point** is the position a caller can act on — the `start` / `line` / `column` the
/// wire would give a node there, under the document's own line rule and BOM reading
/// ([`WireCoordinates::point`]) — and the message's `line:col` header prints it. The
/// **excerpt** is for a terminal: it is bounded by every ECMAScript terminator whatever
/// the language, since a raw `<CR>`, `<LS>` or `<PS>` printed mid-line overwrites or
/// garbles the text the caret points into, so on a Svelte or CSS line holding one the
/// excerpt is the stretch after it while the header still counts the whole `\n` line. Nor
/// does it echo a leading byte-order mark, in any language — the header alone says how the
/// language's wire counts one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErrorContext {
    /// The excerpted source line (no terminator). A `Box<str>` rather than a `String`, and
    /// the caret column a `u32`, to keep the context — and so the payload every located
    /// [`ParseErrorKind`] variant carries — at the size the `const` assert below pins.
    source_line: Box<str>,
    /// Characters from the excerpt's start to the error — what the caret is padded past.
    caret_column: u32,
    /// The error's position in the document's wire coordinates.
    point: WirePoint,
    /// The coordinates `point` was taken in, kept so the context can be re-taken over
    /// another text ([`ParseError::unfold`]).
    coordinates: WireCoordinates,
}

impl ErrorContext {
    /// The context of byte offset `position` in `source`, under `coordinates`.
    ///
    /// Total, like [`WireCoordinates::point`]: a position past the end is clamped to it and
    /// one inside a multibyte character — a lexer/parser error position on malformed
    /// multibyte input — is floored to that character's start, so every slice below is on
    /// a boundary. An empty source is one empty line.
    pub(crate) fn from_source(source: &str, position: usize, coordinates: WireCoordinates) -> Self {
        let mut position = position.min(source.len());
        while !source.is_char_boundary(position) {
            position -= 1;
        }

        // The excerpt's bounds, over the ECMAScript terminator class (`\n`, `\r`, `\r\n`,
        // `<LS>`, `<PS>`) rather than `\n` alone — one question, stated once, in `printing`
        // beside the class itself.
        let (mut line_start, line_end, _) = crate::printing::line_bounds_at(source, position);
        // A leading byte-order mark is never echoed, in any language: the excerpt is display
        // text, whatever the header's column makes of the mark. It is no terminator, so line
        // 1 always reaches past it.
        if line_start == 0 && source.starts_with('\u{FEFF}') {
            line_start = '\u{FEFF}'.len_utf8();
        }

        // Clamped to the line's own end, which `position` can exceed only by sitting inside
        // the terminator sequence that ends it, and to its start, which it falls short of
        // only at the dropped mark (floored to byte 0) — the caret then sits under the
        // excerpt's first character.
        // Saturating: every source a parse accepts is under the `u32` cap, so a column
        // past it would be one no parse could report.
        let caret_end = position.min(line_end).max(line_start);
        let caret_column = source[line_start..caret_end].chars().count();
        let caret_column = u32::try_from(caret_column).unwrap_or(u32::MAX);

        ErrorContext {
            source_line: source[line_start..line_end].into(),
            caret_column,
            point: coordinates.point(source, position),
            coordinates,
        }
    }

    /// Format error context with caret pointer
    fn format_with_caret(&self, message: &str) -> String {
        // The pad reproduces what the excerpt PRINTS ahead of the error, not how many
        // characters it holds. Two independent reasons a character count is wrong: a CJK
        // character occupies two columns, and a tab occupies however many the *terminal*
        // says — its stops are absolute, so no fixed width can stand in for one. So a tab
        // is echoed AS a tab (both lines then reach the same stop, whatever it is) and
        // everything else is padded by its display width.
        let header = format!("{}:{}", self.point.line, self.point.column + 1);
        // Everything printed ahead of the excerpt: the whole `{line}:{col}` header plus its
        // one separating space. Measuring only `{line}:` left the caret short by the
        // column's own digits at every position past column 9.
        let mut indent = " ".repeat(header.chars().count() + 1);
        let prefix: String = self
            .source_line
            .chars()
            .take(self.caret_column as usize)
            .collect();
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
pub struct ParseError(Box<Payload>);

/// What a [`ParseError`] boxes: the error, and the one fact about it its message does
/// not carry.
#[derive(Clone, PartialEq, Eq)]
struct Payload {
    kind: ParseErrorKind,
    /// The error is a **goal gate**: it fired on a construct one parse goal reads and the
    /// other refuses (`import` / `export` / `import.meta`, a top-level `for await`, or the
    /// operand of a top-level `await`, at `Script`), so the rejection says which grammar the
    /// source was written
    /// against rather than that it is broken. Beside the kind rather than on one variant,
    /// because the refusal it marks can take any kind's shape — the `await` operand's is
    /// whatever error the name reading hit there. Read by the format fallback's
    /// attribution ([`ParseError::is_goal_gated`]).
    goal_gated: bool,
}

// The whole point of the newtype — guard it. `Box` is non-null, so the niche also carries
// `Result<(), ParseError>` down to a bare pointer.
const _: () = assert!(size_of::<ParseError>() == size_of::<*const ()>());
const _: () = assert!(size_of::<Result<()>>() == size_of::<*const ()>());
// The payload size the rationale above (and `with_context`'s move) is stated in, on a
// 64-bit target: an inline error would cost every small `Result` this many bytes.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<ParseErrorKind>() == 96);

impl fmt::Display for ParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.kind.fmt(f)
    }
}

impl fmt::Debug for ParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.kind.fmt(f)
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
        ParseError(Box::new(Payload {
            kind,
            goal_gated: false,
        }))
    }

    /// A general parse error at `position`.
    pub fn invalid_syntax(message: String, position: usize) -> Self {
        ParseError::new(ParseErrorKind::InvalidSyntax {
            message,
            position,
            context: None,
        })
    }

    /// A general parse error at `position` that is a **goal gate** — a construct one
    /// parse goal admits and the other refuses, so the error is evidence about the
    /// source's grammar rather than about a mistake in it. See
    /// [`ParseError::is_goal_gated`].
    pub fn goal_gate(message: String, position: usize) -> Self {
        ParseError::invalid_syntax(message, position).into_goal_gate()
    }

    /// This error, marked a goal gate — for a refusal no construct-specific constructor
    /// builds: whatever error a name reading of `await` hit at the operand a module would
    /// have read (`tsv_ts`'s `Parser::parse`). Message and position are untouched.
    #[must_use]
    pub fn into_goal_gate(mut self) -> Self {
        self.0.goal_gated = true;
        self
    }

    /// Whether this error is a goal gate ([`ParseError::goal_gate`]).
    ///
    /// The format fallback holds two errors for one source when both goals reject it,
    /// and a *script* attempt that died on a goal gate has proved the source a module —
    /// it holds an `import` / `export` / `import.meta`, or a top-level `await` with an
    /// operand — whatever position either error reached, so the module attempt's error is
    /// the source's own.
    pub fn is_goal_gated(&self) -> bool {
        self.0.goal_gated
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

    /// The byte offset the error is reported at, in the coordinates the error was
    /// built in (host coordinates for a parser error; see [`ParseError::shift_position`]
    /// for the lexer case). `None` for the one positionless kind, a source over the
    /// `u32` cap. A caller holding two errors for one source reads this to say which got
    /// further — the format fallback's module-vs-script choice.
    pub fn position(&self) -> Option<usize> {
        self.0.kind.located().map(|(position, _)| *position)
    }

    /// Source exceeds the 4 GB cap the `u32` span offsets assume.
    fn file_too_large(size: usize, max: usize) -> Self {
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
    /// A [`lex_err`] position indexes the lexer's `source`, which for the TypeScript and
    /// CSS lexers is routinely a **slice** of the document the error is finally rendered
    /// against: a Svelte `<script>` / `<style>` island or template expression, the CSS
    /// declaration-value scan (`source[from..]`). [`ParseError::with_context`] is handed the
    /// whole document, so a slice-local position points at the wrong construct — an error
    /// on line 4 of a component rendered against line 1, out in the markup.
    ///
    /// A position is lifted **once per coordinate frame it is lifted across** — the
    /// escape decoder's digit-relative position to its backslash, the backslash to the
    /// literal's content, the content to the lexer's source, the lexer's source to the
    /// document (the lexer's `base_offset`) — each lift applied by the code that owns that
    /// frame, and never again by a wrapper delegating to it. A frame lifted twice runs the
    /// position past the end of the source, where the context clamps it to the end of the
    /// document, so the caret lands there instead of on the construct.
    ///
    /// The parser side needs none of this: its positions are already host coordinates (the
    /// TypeScript and CSS parsers' `current_pos` adds the same `base_offset`; the Svelte
    /// parser's tokens are document offsets). Nor does the Svelte lexer: it always scans
    /// the whole document, resuming at a document offset after a jumped scan, so its
    /// positions are document offsets by construction.
    #[cold]
    #[inline(never)]
    pub fn shift_position(mut self, base_offset: usize) -> Self {
        // A positionless error has nothing to shift.
        if let Some((position, context)) = self.0.kind.located_mut() {
            debug_assert!(
                context.is_none(),
                "shift_position after with_context leaves the context at the unshifted point"
            );
            *position += base_offset;
        }
        self
    }

    /// Fill in where the error sits in `source` — the document every position of this
    /// error indexes — under the document's `coordinates`: the point its message header
    /// prints and [`ParseError::wire_point`] reports, and the line excerpted under it. Each
    /// language crate's public parse entry points call this with that language's
    /// coordinates (`tsv_ts::WIRE_COORDINATES` and its siblings). The embedded entry points
    /// (`tsv_ts::parse_embedded` and its siblings, `tsv_css::parse_embedded`) do not: their
    /// source is a slice of a host document, so their errors leave context-free and the
    /// host (`tsv_svelte::parse`) fills them over the whole document.
    ///
    /// ```ignore
    /// parser::parse(source).map_err(|e| e.with_context(source, WIRE_COORDINATES))
    /// ```
    #[must_use]
    pub fn with_context(mut self, source: &str, coordinates: WireCoordinates) -> Self {
        // Filling in place rather than rebuilding the variant: the payload is already
        // boxed, so this is a write through the pointer instead of a 96-byte move.
        // A positionless error has no line to excerpt.
        if let Some((position, slot)) = self.0.kind.located_mut() {
            // One fill per error: a second would overwrite the first with whatever text it
            // was handed. Re-taking a context deliberately is `unfold`'s job, not this one's.
            debug_assert!(
                slot.is_none(),
                "with_context on an error that already has one"
            );
            *slot = Some(ErrorContext::from_source(source, *position, coordinates));
        }
        self
    }

    /// Where the error sits, in the wire coordinates of the document it was parsed from:
    /// the `start` offset a wire node there would carry, and the `loc` point
    /// (`line` 1-based, `column` 0-based UTF-16 units) the document's line rule gives it —
    /// the numbers its message header prints, as `line:column+1`.
    ///
    /// `None` for the positionless kind (a source over the `u32` cap) and for an error no
    /// [`ParseError::with_context`] has filled — which only the embedded entry points
    /// return, ahead of their host's fill; no whole-document parse entry point does.
    pub fn wire_point(&self) -> Option<WirePoint> {
        self.0
            .kind
            .located()
            .and_then(|(_, context)| context.as_ref())
            .map(|context| context.point)
    }

    /// Lift an error raised by a parse of the CR-folded text
    /// ([`crate::printing::normalize_carriage_returns`]) back onto `original`, the text the
    /// caller handed over: the position moves to the original byte it was folded from
    /// ([`crate::printing::unfold_position`]), and the context is re-taken there, in the
    /// coordinates it was first taken in. Reached only through
    /// [`crate::printing::FoldedSource::parse_with`], which holds both texts.
    pub(crate) fn unfold(mut self, original: &str) -> Self {
        if let Some((position, slot)) = self.0.kind.located_mut() {
            *position = crate::printing::unfold_position(original, *position);
            if let Some(context) = slot {
                *context = ErrorContext::from_source(original, *position, context.coordinates);
            }
        }
        self
    }
}

impl ParseErrorKind {
    /// The position and context slots of a located error, `None` for the one
    /// positionless kind (`FileTooLarge`). Which variants carry a position is stated here
    /// and in [`ParseErrorKind::located_mut`] — one match cannot lend both a shared and
    /// an exclusive borrow — so `position`, `shift_position` and `with_context` restate
    /// no variant list of their own, and a new variant is classified in these two.
    fn located(&self) -> Option<(&usize, &Option<ErrorContext>)> {
        match self {
            ParseErrorKind::UnexpectedToken {
                position, context, ..
            }
            | ParseErrorKind::UnexpectedEof { position, context }
            | ParseErrorKind::InvalidSyntax {
                position, context, ..
            }
            | ParseErrorKind::InvalidExpression {
                position, context, ..
            } => Some((position, context)),
            ParseErrorKind::FileTooLarge { .. } => None,
        }
    }

    /// [`ParseErrorKind::located`], mutably — the same variant list, kept in step.
    fn located_mut(&mut self) -> Option<(&mut usize, &mut Option<ErrorContext>)> {
        match self {
            ParseErrorKind::UnexpectedToken {
                position, context, ..
            }
            | ParseErrorKind::UnexpectedEof { position, context }
            | ParseErrorKind::InvalidSyntax {
                position, context, ..
            }
            | ParseErrorKind::InvalidExpression {
                position, context, ..
            } => Some((position, context)),
            ParseErrorKind::FileTooLarge { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::{LeadingBom, LineRule};

    /// A TypeScript document's coordinates: ECMAScript lines, a BOM counted.
    const ECMA: WireCoordinates = WireCoordinates {
        lines: LineRule::Ecmascript,
        bom: LeadingBom::Counted,
    };
    /// A Svelte or CSS document's coordinates: `\n` lines, a BOM elided.
    const LF: WireCoordinates = WireCoordinates {
        lines: LineRule::Lf,
        bom: LeadingBom::Elided,
    };

    fn point(start: u32, line: u32, column: u32) -> WirePoint {
        WirePoint {
            start,
            line,
            column,
        }
    }

    /// The `line:col` header of a rendered caret-form message.
    fn header(rendered: &str) -> &str {
        let located = rendered.lines().nth(1).expect("a located line");
        located.split_once(' ').map_or(located, |(head, _)| head)
    }

    /// The goal-gate mark rides the payload through the two rewrites a located error
    /// takes on its way out of an embedded parse — the offset shift and the context
    /// fill — and every other constructor leaves it unset.
    #[test]
    fn test_goal_gate_mark_survives_shift_and_context() {
        let e = ParseError::goal_gate("'export' is only allowed in a module".to_string(), 0)
            .shift_position(3)
            .with_context("x; export {};", ECMA);
        assert!(e.is_goal_gated());
        assert_eq!(e.position(), Some(3));
        assert!(!ParseError::invalid_syntax("x".to_string(), 0).is_goal_gated());
        assert!(!ParseError::unexpected_eof(0).is_goal_gated());
    }

    /// `into_goal_gate` marks an error of any kind — the refusal a name `await` hits at
    /// its operand is whatever the name reading raised there — and changes nothing the
    /// error renders or reports.
    #[test]
    fn test_into_goal_gate_keeps_the_kind_and_its_message() {
        let plain = ParseError::unexpected_token("';'".to_string(), "identifier".to_string(), 6)
            .with_context("await x;", ECMA);
        let gated = plain.clone().into_goal_gate();
        assert!(!plain.is_goal_gated());
        assert!(gated.is_goal_gated());
        assert_eq!(gated.to_string(), plain.to_string());
        assert_eq!(gated.position(), Some(6));
        assert_eq!(gated.wire_point(), plain.wire_point());
    }

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
            e.with_context(source, ECMA).to_string(),
            "bad token\n1:5 let x =\n        ^ here"
        );

        let e = ParseError::unexpected_token("';'".to_string(), "'y'".to_string(), 8);
        assert_eq!(e.to_string(), "Expected ';', found 'y' at position 8");
        assert_eq!(
            e.with_context(source, ECMA).to_string(),
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

        // `with_context` on the context-free variant is a no-op, not a panic, and it has
        // no point to report.
        let too_large = ParseError::file_too_large(5, 4).with_context(source, ECMA);
        assert_eq!(
            too_large.to_string(),
            "File too large: 5 bytes (maximum: 4 bytes / 4GB)"
        );
        assert_eq!(too_large.wire_point(), None);
        // Nor does a located error no context has been filled for.
        assert_eq!(ParseError::unexpected_eof(9).wire_point(), None);

        // `Debug` forwards too, so it prints the kind — not a `ParseError(..)` wrapper.
        assert!(
            format!("{:?}", ParseError::unexpected_eof(9)).starts_with("UnexpectedEof"),
            "Debug must forward to the kind"
        );
    }

    /// The excerpt is bounded by the ECMAScript terminator class, not by `\n` alone, in
    /// EVERY language's coordinates. On a lone-`<CR>` source the `\n`-only reading made the
    /// whole file one excerpt, carrying raw `<CR>`s — which a terminal renders by
    /// overwriting, hiding the very text the caret points into. The header's line is the
    /// coordinates' own: the ECMAScript count for a TypeScript document, `\n` alone for a
    /// Svelte or CSS one.
    #[test]
    fn context_bounds_the_line_on_every_terminator() {
        let cr = "let a = 1;\rlet b = 2;\r";
        for coordinates in [ECMA, LF] {
            let ctx = ErrorContext::from_source(cr, 15, coordinates);
            assert_eq!(&*ctx.source_line, "let b = 2;");
            assert_eq!(ctx.caret_column, 4);
        }
        assert_eq!(
            ErrorContext::from_source(cr, 15, ECMA).point,
            point(15, 2, 4)
        );
        assert_eq!(
            ErrorContext::from_source(cr, 15, LF).point,
            point(15, 1, 15)
        );

        // `<CR><LF>` is ONE terminator, so the second line is still line 2 — and `\n`
        // alone counts it once too.
        let crlf = "let a = 1;\r\nlet b = 2;\r\n";
        for coordinates in [ECMA, LF] {
            let ctx = ErrorContext::from_source(crlf, 16, coordinates);
            assert_eq!(&*ctx.source_line, "let b = 2;");
            assert_eq!(ctx.caret_column, 4);
            assert_eq!(ctx.point, point(16, 2, 4));
        }

        // `<LS>` and `<PS>` terminate a line for ECMAScript, and are ordinary characters
        // (one UTF-16 unit each) on a `\n` line.
        let ls = "let a = 1;\u{2028}let b = 2;";
        for coordinates in [ECMA, LF] {
            let ctx = ErrorContext::from_source(ls, 17, coordinates);
            assert_eq!(&*ctx.source_line, "let b = 2;");
            assert_eq!(ctx.caret_column, 4);
        }
        assert_eq!(
            ErrorContext::from_source(ls, 17, ECMA).point,
            point(15, 2, 4)
        );
        assert_eq!(
            ErrorContext::from_source(ls, 17, LF).point,
            point(15, 1, 15)
        );
    }

    /// The rendered excerpt never carries a raw terminator or a leading BOM, and the header
    /// is the point's `line:column+1` — whatever the coordinates and wherever the error sits.
    #[test]
    fn header_is_the_point_and_the_excerpt_holds_no_terminator() {
        let sources = [
            "a\rb\r\nc\u{2028}d\u{2029}e\nf",
            "\u{FEFF}x𝒜y\r\nz",
            "",
            "\r\n",
        ];
        for source in sources {
            for coordinates in [ECMA, LF] {
                for position in 0..=source.len() + 2 {
                    let e = ParseError::invalid_syntax("bad".to_string(), position)
                        .with_context(source, coordinates);
                    let p = e.wire_point().expect("a located, filled error has a point");
                    let rendered = e.to_string();
                    assert_eq!(header(&rendered), format!("{}:{}", p.line, p.column + 1));
                    let excerpt = rendered.lines().nth(1).expect("located line");
                    assert!(
                        !excerpt.contains(['\r', '\u{2028}', '\u{2029}']),
                        "{source:?} @ {position}: {rendered:?}"
                    );
                    assert!(
                        !excerpt.contains('\u{FEFF}'),
                        "{source:?} @ {position}: {rendered:?}"
                    );
                    assert_eq!(rendered.split('\n').count(), 3, "{rendered:?}");
                }
            }
        }
    }

    /// `WireCoordinates::point` across both line rules and both BOM readings.
    #[test]
    fn wire_point_across_rules_and_bom_readings() {
        // An astral character is two UTF-16 units: `start` and the column both step 2.
        let astral = "a𝒜b";
        assert_eq!(ECMA.point(astral, 5), point(3, 1, 3));
        assert_eq!(LF.point(astral, 5), point(3, 1, 3));
        // A position INSIDE a multibyte character floors to its start.
        for inside in 2..=4 {
            assert_eq!(ECMA.point(astral, inside), point(1, 1, 1));
        }
        // Past the end clamps to the end.
        assert_eq!(ECMA.point(astral, 99), point(4, 1, 4));
        assert_eq!(ECMA.point("", 3), point(0, 1, 0));

        // A leading BOM: counted, it is one unit at position 0 (acorn); elided, it occupies
        // no position, so everything after it — line 1's columns included — is one lower.
        let bom = "\u{FEFF}ab\ncd";
        assert_eq!(ECMA.point(bom, 0), point(0, 1, 0));
        assert_eq!(LF.point(bom, 0), point(0, 1, 0));
        // Inside the BOM's three bytes floors to 0.
        assert_eq!(LF.point(bom, 2), point(0, 1, 0));
        assert_eq!(ECMA.point(bom, 4), point(2, 1, 2));
        assert_eq!(LF.point(bom, 4), point(1, 1, 1));
        assert_eq!(ECMA.point(bom, 7), point(5, 2, 1));
        assert_eq!(LF.point(bom, 7), point(4, 2, 1));
        // A U+FEFF anywhere but byte 0 is an ordinary character under both readings.
        assert_eq!(LF.point("a\u{FEFF}b", 4), point(2, 1, 2));

        // Inside `<CR><LF>` — the byte between the two — the CR has not ended the line
        // (CRLF is one terminator, ended by its LF), so the point is the CR's line, one
        // column past it. `\n` alone reads the CR as an ordinary character: the same point.
        let crlf = "ab\r\ncd";
        assert_eq!(ECMA.point(crlf, 3), point(3, 1, 3));
        assert_eq!(LF.point(crlf, 3), point(3, 1, 3));
        assert_eq!(ECMA.point(crlf, 4), point(4, 2, 0));
        assert_eq!(LF.point(crlf, 4), point(4, 2, 0));

        // A lone CR, U+2028 and U+2029 break an ECMAScript line only.
        for terminator in ["\r", "\u{2028}", "\u{2029}"] {
            let source = format!("ab{terminator}cd");
            let at = source.len() - 1;
            assert_eq!(ECMA.point(&source, at), point(4, 2, 1), "{terminator:?}");
            assert_eq!(LF.point(&source, at), point(4, 1, 4), "{terminator:?}");
        }
    }

    /// `unfold_position` maps a byte offset in the CR-folded text back to the byte the fold
    /// produced it from: a CRLF's single folded LF maps to the CR that opens the pair, a
    /// lone CR's LF to that CR, and every later byte past the dropped LFs.
    #[test]
    fn unfold_position_maps_folded_offsets_to_the_original() {
        use crate::printing::{normalize_carriage_returns, unfold_position};

        // No CR: the identity.
        for p in 0..=6 {
            assert_eq!(unfold_position("ab\ncdé", p), p);
        }
        // CRLF: folded "a\nb\nc" from "a\r\nb\r\nc".
        let original = "a\r\nb\r\nc";
        assert_eq!(normalize_carriage_returns(original).text(), "a\nb\nc");
        let expected = [0, 1, 3, 4, 6, 7];
        for (folded, original_pos) in expected.into_iter().enumerate() {
            assert_eq!(unfold_position(original, folded), original_pos, "{folded}");
        }
        // Lone CR: the fold changes the byte, not the length.
        for p in 0..=5 {
            assert_eq!(unfold_position("a\rb\rc", p), p);
        }
        // Mixed, with a multibyte character ahead: folded "é\nx\ny" from "é\r\nx\ry".
        let original = "é\r\nx\ry";
        assert_eq!(normalize_carriage_returns(original).text(), "é\nx\ny");
        let expected = [0, 1, 2, 4, 5, 6, 7];
        for (folded, original_pos) in expected.into_iter().enumerate() {
            assert_eq!(unfold_position(original, folded), original_pos, "{folded}");
        }
    }

    /// An error raised over the folded text, unfolded, is the error a parse of the original
    /// would report at the same character: position, point and message.
    #[test]
    fn unfold_error_retakes_the_context_over_the_original() {
        let original = "a;\r\nb;\r\n  @";
        let folded = crate::printing::normalize_carriage_returns(original);
        let at_folded = folded.text().find('@').expect("@");
        let e = ParseError::invalid_syntax("bad".to_string(), at_folded)
            .with_context(folded.text(), ECMA);
        let unfolded = folded.unfold_error(e);
        let at_original = original.find('@').expect("@");
        let direct =
            ParseError::invalid_syntax("bad".to_string(), at_original).with_context(original, ECMA);
        assert_eq!(unfolded.position(), Some(at_original));
        assert_eq!(unfolded.wire_point(), direct.wire_point());
        assert_eq!(unfolded.to_string(), direct.to_string());
        assert_eq!(unfolded.wire_point(), Some(point(10, 3, 2)));

        // A text the fold left alone is handed back untouched.
        let plain = "a;\n@";
        let folded = crate::printing::normalize_carriage_returns(plain);
        let e = ParseError::invalid_syntax("bad".to_string(), 3).with_context(plain, LF);
        assert_eq!(folded.unfold_error(e.clone()), e);
    }

    /// A position INSIDE a terminator sequence — the byte between a `<CR>` and its `<LF>`
    /// — belongs to the line that sequence ends, and the excerpt stops before the `<CR>`
    /// rather than carrying it.
    #[test]
    fn context_of_a_position_inside_a_terminator_sequence() {
        let crlf = "let a = 1;\r\nlet b = 2;\r\n";
        let ctx = ErrorContext::from_source(crlf, 11, ECMA);
        assert_eq!(&*ctx.source_line, "let a = 1;");
        assert_eq!(ctx.caret_column, 10);
        assert_eq!(ctx.point, point(11, 1, 11));
    }

    /// The caret pads by DISPLAY width; the header's column counts UTF-16 units — the
    /// two differ from a byte count in opposite directions, and a byte count gets both
    /// wrong.
    #[test]
    fn caret_lands_under_its_token_past_a_multibyte_prefix() {
        // `à` is 2 bytes, 1 UTF-16 unit, 1 column: a byte column would report 12 and pad one
        // space too far.
        let src = "const à = ;";
        let ctx = ErrorContext::from_source(src, src.find(';').expect("semi"), ECMA);
        assert_eq!(ctx.point.column, 10);
        assert_eq!(
            ctx.format_with_caret("bad"),
            "bad\n1:11 const à = ;\n               ^ here"
        );

        // `𝒜` is 4 bytes, 1 character, 1 display column and 2 UTF-16 units: the header
        // counts the units (the wire's column), the caret pads the one column it prints.
        let src = "x𝒜 = ;";
        let ctx = ErrorContext::from_source(src, src.find(';').expect("semi"), ECMA);
        assert_eq!(ctx.point.column, 6);
        assert_eq!(
            ctx.format_with_caret("bad"),
            "bad\n1:7 x𝒜 = ;\n         ^ here"
        );

        // A tab is 1 character and however many columns the terminal's stops give it, so
        // the pad echoes the tab itself rather than guessing a width for it.
        let src = "\tconst a = ;";
        let ctx = ErrorContext::from_source(src, src.find(';').expect("semi"), ECMA);
        assert_eq!(ctx.point.column, 11);
        assert_eq!(
            ctx.format_with_caret("bad"),
            "bad\n1:12 \tconst a = ;\n     \t          ^ here"
        );

        // An elided BOM moves line 1's header column, never the caret: the excerpt leaves
        // it out (`leading_bom_is_not_echoed`).
        let src = "\u{FEFF}a = ;";
        let ctx = ErrorContext::from_source(src, src.find(';').expect("semi"), LF);
        assert_eq!(ctx.point, point(4, 1, 4));
        assert_eq!(
            ctx.format_with_caret("bad"),
            "bad\n1:5 a = ;\n        ^ here"
        );
    }

    /// The excerpt is display text, so it never shows a leading BOM, whatever the language;
    /// the header keeps the wire's reading of it — counted on line 1 for TypeScript, elided
    /// for Svelte and CSS. Only the byte-0 mark goes: a content U+FEFF stays, and so does
    /// every line past the first.
    #[test]
    fn leading_bom_is_not_echoed() {
        let src = "\u{FEFF}a = ;";
        let semi = src.find(';').expect("semi");
        // Counted: the header's column is one higher, the excerpt and caret the same.
        let ctx = ErrorContext::from_source(src, semi, ECMA);
        assert_eq!(&*ctx.source_line, "a = ;");
        assert_eq!(ctx.caret_column, 4);
        assert_eq!(ctx.point, point(5, 1, 5));
        assert_eq!(
            ctx.format_with_caret("bad"),
            "bad\n1:6 a = ;\n        ^ here"
        );
        let ctx = ErrorContext::from_source(src, semi, LF);
        assert_eq!(&*ctx.source_line, "a = ;");
        assert_eq!(ctx.caret_column, 4);

        // At the BOM itself, and inside its three bytes (floored to it): the caret is under
        // the excerpt's first character.
        for position in 0..=2 {
            for coordinates in [ECMA, LF] {
                let ctx = ErrorContext::from_source(src, position, coordinates);
                assert_eq!(&*ctx.source_line, "a = ;");
                assert_eq!(ctx.caret_column, 0);
                assert_eq!(ctx.format_with_caret("bad"), "bad\n1:1 a = ;\n    ^ here");
            }
        }
        // Just past it: the excerpt's first character, the header counting the BOM or not.
        assert_eq!(
            ErrorContext::from_source(src, 3, ECMA).format_with_caret("bad"),
            "bad\n1:2 a = ;\n    ^ here"
        );
        assert_eq!(
            ErrorContext::from_source(src, 3, LF).format_with_caret("bad"),
            "bad\n1:1 a = ;\n    ^ here"
        );

        // A BOM-only source is one empty line, at either end of the mark.
        for position in [0, 3] {
            let ctx = ErrorContext::from_source("\u{FEFF}", position, ECMA);
            assert_eq!(&*ctx.source_line, "");
            assert_eq!(ctx.caret_column, 0);
            let ctx = ErrorContext::from_source("\u{FEFF}", position, LF);
            assert_eq!(ctx.format_with_caret("bad"), "bad\n1:1 \n    ^ here");
        }

        // A content U+FEFF later on line 1 is echoed: only the byte-0 mark is metadata.
        let src = "\u{FEFF}a\u{FEFF}b = ;";
        let ctx = ErrorContext::from_source(src, src.find(';').expect("semi"), LF);
        assert_eq!(&*ctx.source_line, "a\u{FEFF}b = ;");
        assert_eq!(ctx.caret_column, 6);
        // ...and a U+FEFF that is not at byte 0 is never a mark.
        let src = " \u{FEFF}a = ;";
        let ctx = ErrorContext::from_source(src, src.find(';').expect("semi"), LF);
        assert_eq!(&*ctx.source_line, " \u{FEFF}a = ;");

        // A line-2 error is untouched.
        let src = "\u{FEFF}a;\nb = ;";
        for coordinates in [ECMA, LF] {
            let ctx = ErrorContext::from_source(src, src.rfind(';').expect("semi"), coordinates);
            assert_eq!(&*ctx.source_line, "b = ;");
            assert_eq!(ctx.caret_column, 4);
        }
    }

    #[test]
    fn test_error_context_at_eof_no_newline() {
        // Position at EOF, source doesn't end with newline
        let ctx = ErrorContext::from_source("hello", 5, ECMA);
        assert_eq!(&*ctx.source_line, "hello");
        assert_eq!(ctx.caret_column, 5);
        assert_eq!(ctx.point, point(5, 1, 5));
    }

    #[test]
    fn test_error_context_at_eof_with_newline() {
        // Position at EOF, source ends with newline
        let ctx = ErrorContext::from_source("hello\n", 6, ECMA);
        assert_eq!(&*ctx.source_line, ""); // Empty line after newline
        assert_eq!(ctx.caret_column, 0);
        assert_eq!(ctx.point, point(6, 2, 0));
    }

    #[test]
    fn test_error_context_middle_of_line() {
        let ctx = ErrorContext::from_source("abc\ndef\nghi", 5, ECMA); // 'e' in "def"
        assert_eq!(&*ctx.source_line, "def");
        assert_eq!(ctx.caret_column, 1);
        assert_eq!(ctx.point, point(5, 2, 1));
    }

    #[test]
    fn test_error_context_start_of_file() {
        let ctx = ErrorContext::from_source("hello", 0, ECMA);
        assert_eq!(&*ctx.source_line, "hello");
        assert_eq!(ctx.caret_column, 0);
        assert_eq!(ctx.point, point(0, 1, 0));
    }

    #[test]
    fn test_error_context_position_inside_multibyte_char() {
        // A byte offset landing *inside* a multibyte char must not panic — it's
        // floored to the char boundary. `名` is 3 bytes (starts at byte 4).
        let source = "abc 名 def";
        for pos in 4..=6 {
            let ctx = ErrorContext::from_source(source, pos, ECMA);
            assert_eq!(&*ctx.source_line, source);
            // Floored to the char boundary at byte 4 (the start of `名`) — which is
            // character 4 as well, everything before it being ASCII.
            assert_eq!(ctx.caret_column, 4);
            assert_eq!(ctx.point, point(4, 1, 4));
        }
        // A boundary just past the multibyte char is kept as-is: byte 7, character 5.
        let ctx = ErrorContext::from_source(source, 7, ECMA);
        assert_eq!(ctx.caret_column, 5);
        assert_eq!(ctx.point, point(5, 1, 5));
    }

    /// An empty source is one empty line, and its one position is the start of it.
    #[test]
    fn test_error_context_empty_source() {
        let ctx = ErrorContext::from_source("", 0, LF);
        assert_eq!(&*ctx.source_line, "");
        assert_eq!(ctx.point, point(0, 1, 0));
    }

    /// A position past the end — a bug upstream (a double `shift_position`) — clamps to
    /// the end rather than dropping the caret.
    #[test]
    fn test_error_context_position_out_of_bounds() {
        let ctx = ErrorContext::from_source("hello", 10, ECMA);
        assert_eq!(&*ctx.source_line, "hello");
        assert_eq!(ctx.caret_column, 5);
        assert_eq!(ctx.point, point(5, 1, 5));
    }
}
