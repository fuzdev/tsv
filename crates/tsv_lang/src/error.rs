// Error types for parsing

use std::fmt;
use std::fmt::Write as _;
use std::ops::Range;

use crate::depth::DepthLimit;
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
    /// [`Repr`] variant carries — at the size the `const` assert below pins.
    source_line: Box<str>,
    /// Characters from the excerpt's start to the error — what the caret is padded past.
    caret_column: u32,
    /// The error's position in the document's wire coordinates.
    point: WirePoint,
    /// The coordinates `point` was taken in, kept so the context can be re-taken over
    /// another text ([`Error::unfold`]).
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

    /// The **frame** under a located error's summary: the `{line}:{col}` header and the
    /// excerpted line, then the caret line beneath — two lines, joined by one `\n`, no
    /// terminator after. The message a located error renders is its summary, a `\n`, and
    /// this ([`ErrorContext::format_with_caret`]); a binding reports the two apart
    /// ([`ErrorReport`]).
    fn frame(&self) -> String {
        // The pad reproduces what the excerpt PRINTS ahead of the error, not how many
        // characters it holds. Two independent reasons a character count is wrong: a CJK
        // character occupies two columns, and a tab occupies however many the *terminal*
        // says — its stops are absolute, so no fixed width can stand in for one. So a tab
        // is echoed AS a tab (both lines then reach the same stop, whatever it is) and
        // everything else is padded by its display width.
        // Widened before the `+ 1`: a column can be `u32::MAX` (the cap admits a source of
        // exactly `u32::MAX` bytes, so an EOF error on its one line sits there), and a
        // `u32` add would overflow. The line prints as taken, saturated by the point walk.
        let header = format!("{}:{}", self.point.line, u64::from(self.point.column) + 1);
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
        format!("{header} {}\n{indent}^ here", self.source_line)
    }

    /// `message`, then a `\n`, then the [frame](ErrorContext::frame): the rendered form of a
    /// located error.
    fn format_with_caret(&self, message: &str) -> String {
        format!("{message}\n{}", self.frame())
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

/// The error payload, behind the `Box` in [`Error`]. Private: `Error` is
/// construction-only outside this module — nothing matches a variant or reads a field —
/// so the constructor functions below are the whole API, and keeping the enum private
/// makes them the only way in (which is what keeps `context` uniformly `None` at
/// construction, filled later by [`Error::with_context`]).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Repr {
    UnexpectedToken {
        expected: String,
        found: String,
        position: usize,
        context: Option<ErrorContext>,
    },
    UnexpectedEof {
        position: usize,
        context: Option<ErrorContext>,
    },
    InvalidSyntax {
        message: String,
        position: usize,
        context: Option<ErrorContext>,
    },
    InvalidExpression {
        found: String,
        position: usize,
        context: Option<ErrorContext>,
    },
    // Constructed only by `Error::too_deep`: a located syntax error like the four
    // above, kept apart so a caller can tell a resource limit from malformed source
    // (`Error::kind`).
    TooDeep {
        limit: DepthLimit,
        position: usize,
        context: Option<ErrorContext>,
    },
    // Constructed only by `ensure_source_fits`, which every public parse entry point
    // calls before touching the source — that guard is what makes `Token`/`Span`'s
    // `u32` offsets sound (see `tsv_ts/src/lexer/token.rs`).
    FileTooLarge {
        size: usize,
        max: usize,
    },
    // Constructed only by `Error::refusal`. Not a located error, though it has a
    // span: see that constructor. `span` is the refused bytes in the source the refusal
    // was built over ([`Error::span`]); `start` / `end` the same two ends in its wire
    // coordinates ([`Error::report`]).
    Refused {
        message: Box<str>,
        frame: Box<str>,
        span: Range<usize>,
        start: WirePoint,
        end: WirePoint,
    },
}

impl Repr {
    /// What went wrong, without where: the message's first part, ahead of the frame a
    /// located error renders under it — always ONE line. Every summary every surface prints or
    /// returns is built here, and several quote source text (a token, a Svelte attribute's
    /// raw text), so a line terminator in it is spelled out ([`escape_line_terminators`]).
    fn summary(&self) -> String {
        escape_line_terminators(self.raw_summary())
    }

    /// [`Repr::summary`] before its line terminators are spelled out.
    fn raw_summary(&self) -> String {
        match self {
            Repr::UnexpectedToken {
                expected, found, ..
            } => format!("Expected {expected}, found {found}"),
            Repr::UnexpectedEof { .. } => "Unexpected end of file".to_owned(),
            Repr::InvalidSyntax { message, .. } => message.clone(),
            Repr::InvalidExpression { found, .. } => {
                format!("Expected expression, found {found}")
            }
            Repr::TooDeep { limit, .. } => match limit {
                DepthLimit::Nesting => format!(
                    "Nesting exceeds tsv's depth limit of {} levels",
                    limit.value()
                ),
                // no number: the limit counts weighted units, not links, with the nesting
                // on the chain's path charged on top, so no count of links names it
                DepthLimit::Spine => "Chain of operators, member accesses, calls, assertions or \
                                      type suffixes exceeds tsv's length limit"
                    .to_string(),
            },
            Repr::FileTooLarge { size, max } => {
                format!("File too large: {size} bytes (maximum: {max} bytes / 4 GiB)")
            }
            Repr::Refused { message, .. } => message.to_string(),
        }
    }
}

/// One line by default, the whole frame under the alternate flag (`{:#}`) — see
/// [`Error`]'s `Display`.
impl fmt::Display for Repr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let summary = self.summary();
        if f.alternate() {
            // The summary, then — for a located error with its context filled, and for a
            // refusal — a `\n` and the frame; a located error no context has been filled for
            // falls back to `at position N`.
            if let Repr::Refused { frame, .. } = self {
                return write!(f, "{summary}\n{frame}");
            }
            return match self.located() {
                Some((position, context)) => {
                    f.write_str(&format_error(&summary, *position, context.as_ref()))
                }
                None => f.write_str(&summary),
            };
        }
        // The point the frame's header prints, as `line:col`; a byte offset where no context
        // has been filled; nothing for the size cap, which has no position.
        let point = match self {
            Repr::Refused { start, .. } => Some(*start),
            _ => match self.located() {
                Some((_, Some(context))) => Some(context.point),
                Some((position, None)) => return write!(f, "{summary} (at byte {position})"),
                None => None,
            },
        };
        match point {
            Some(point) => write!(
                f,
                "{summary} ({}:{})",
                point.line,
                u64::from(point.column) + 1
            ),
            None => f.write_str(&summary),
        }
    }
}

impl ErrorReport {
    /// A refusal's report — what [`Error::refusal`] reports for the same arguments —
    /// built directly, for a caller whose refusal is no `Error` (the type strip's,
    /// `tsv_ts::UnsupportedKind::report`): `message` as the summary (its line terminators
    /// spelled out), the frame of `start` in `source` under `coordinates`, and the span
    /// `start..end`.
    #[doc(hidden)]
    #[must_use]
    pub fn refusal(
        message: &str,
        source: &str,
        start: usize,
        end: usize,
        coordinates: WireCoordinates,
    ) -> Self {
        let context = ErrorContext::from_source(source, start, coordinates);
        ErrorReport {
            summary: escape_line_terminators(message.to_owned()),
            frame: context.frame(),
            start: context.point,
            end: coordinates.point(source, end.max(start)),
            refused: true,
        }
    }
}

/// `summary` with every line terminator — LF, CR, U+2028, U+2029, the characters that end a
/// line in a terminal or in ECMAScript — spelled `<U+XXXX>` (the `U+` notation the codepoint
/// errors already use), so a summary quoting source text stays one line and no raw CR
/// overwrites a terminal line. The frame needs none of this: its excerpt is bounded by the
/// same terminators ([`ErrorContext::from_source`]).
fn escape_line_terminators(summary: String) -> String {
    const TERMINATORS: [char; 4] = ['\n', '\r', '\u{2028}', '\u{2029}'];
    if !summary.contains(TERMINATORS) {
        return summary;
    }
    let mut escaped = String::with_capacity(summary.len() + 8);
    for c in summary.chars() {
        if TERMINATORS.contains(&c) {
            // infallible: writing to a `String` never fails
            let _ = write!(escaped, "<U+{:04X}>", u32::from(c));
        } else {
            escaped.push(c);
        }
    }
    escaped
}

/// A pointed error as a binding reports it: the summary and the frame apart, and the span
/// in the document's wire coordinates ([`Error::report`]). The error's full message (its
/// `{:#}` form) is exactly `summary`, a `\n`, then `frame`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ErrorReport {
    /// What went wrong — the message's first part.
    pub summary: String,
    /// The `{line}:{col}` header with the excerpted line, a `\n`, then the caret line —
    /// what the message prints after the summary and a `\n`.
    pub frame: String,
    /// Where the error starts: the point the header prints.
    pub start: WirePoint,
    /// Where it ends — `start` itself for a syntax error, which knows a point and no span;
    /// a refusal's own end.
    pub end: WirePoint,
    /// A refusal of a source that parses ([`ErrorKind::Refusal`]), not a syntax error.
    pub refused: bool,
}

// 8 bytes, because the payload lives behind the `Box`. The size is the point: a
// `Result<T, E>` is sized by `max(T, E)`, so an inline `Repr` (96 bytes) makes every fallible
// function whose success payload is smaller than that return 96 bytes through memory on its
// hot `Ok` path — and the parsers are full of `Result<(), _>`, `Result<bool, _>`,
// `Result<usize, _>`. Boxing the payload once, here, shrinks the `Result` at every one of
// those call sites in all three language crates without a single signature mentioning a
// `Box`. The larger effect is on code size rather than the data path: the error half of each
// `Result` shrinks at every site, including the many whose `Result` size never changed
// because a fat AST node (`Statement`, `Expression`) already dominated it, so the hot parse
// loops pack into less instruction cache.
/// A parse error: what went wrong ([`Error::kind`]), where ([`Error::span`], UTF-8 byte
/// offsets), and the pointed report a binding renders ([`Error::report`]). One pointer
/// wide, so a `Result` carrying it costs a successful call almost nothing.
///
/// # Display
///
/// `{}` is **one line**: the summary, then the error's `line:col` (1-based line, 1-based
/// UTF-16 column, the point its frame's header prints) in parentheses —
/// `Unexpected end of file (3:7)`. A source over the size cap, which has no position, is
/// the summary alone. The alternate form `{:#}` is the whole frame: the
/// summary, a `\n`, the `line:col` header with the excerpted line, a `\n`, and the caret
/// line — what the `tsv` CLI prints (and `summary + "\n" + frame` of [`Error::report`]).
/// `Debug` prints the private payload's variant.
#[derive(Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct Error(Box<Payload>);

/// What an [`Error`] boxes: the error, and the one fact about it its message does
/// not carry.
#[derive(Clone, PartialEq, Eq)]
struct Payload {
    kind: Repr,
    /// The error is a **goal gate**: it fired on a construct one parse goal reads and the
    /// other refuses (`import` / `export` / `import.meta`, a top-level `for await`, or the
    /// operand of a top-level `await`, at `Script`), so the rejection says which grammar the
    /// source was written
    /// against rather than that it is broken. Beside the kind rather than on one variant,
    /// because the refusal it marks can take any kind's shape — the `await` operand's is
    /// whatever error the name reading hit there. Read by the format fallback's
    /// attribution ([`Error::is_goal_gated`]).
    goal_gated: bool,
}

// The whole point of the newtype — guard it. `Box` is non-null, so the niche also carries
// `Result<(), Error>` down to a bare pointer. `#[repr(transparent)]` makes both a layout
// guarantee rather than rustc's choice, which is why these two hold in every build.
const _: () = assert!(size_of::<Error>() == size_of::<*const ()>());
const _: () = assert!(size_of::<Result<()>>() == size_of::<*const ()>());
// The payload size the rationale above (and `with_context`'s move) is stated in, on a
// 64-bit target: an inline error would cost every small `Result` this many bytes. An exact
// rustc layout, so it is pinned in the test build only.
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(size_of::<Repr>() == 96);

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.kind, f)
    }
}

impl fmt::Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0.kind, f)
    }
}

/// What kind of failure an [`Error`] is ([`Error::kind`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The source is malformed at a point: [`Error::span`] is that point, as an empty
    /// range.
    Syntax,
    /// The source parses, but an operation cannot handle what it holds — a format that
    /// cannot print it without changing what it means, or — as the type
    /// strip's `tsv_ts::StripError::kind` reports — a construct type blanking cannot erase.
    /// [`Error::span`] is the refused bytes.
    Refusal,
    /// The source passed one of the parsers' depth limits — a resource bound rather than a
    /// grammar verdict. Located like a syntax error: [`Error::span`] is the point the limit
    /// tripped at.
    TooDeep,
    /// The source is longer than the `u32` offsets can index (over 4 GiB − 1 bytes). No
    /// position: [`Error::span`] is `None`.
    TooLarge,
}

impl std::error::Error for Error {}

// An error crosses threads and outlives the call that made it (a worker pool's results, a
// boxed `dyn Error`); pin that so a field added later cannot take it away unnoticed.
const _: () = {
    const fn assert_send_sync_static<T: Send + Sync + 'static>() {}
    assert_send_sync_static::<Error>();
    assert_send_sync_static::<ErrorReport>();
};

/// Result type alias for parsing operations
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Construct a lexer error. `#[cold]` / `#[inline(never)]` outlines the construction so it
    /// never bloats the inlined token-scan fast path. Shared by all three language lexers
    /// (`tsv_ts`, `tsv_css`, `tsv_svelte`).
    #[doc(hidden)]
    #[cold]
    #[inline(never)]
    pub fn lex(message: impl Into<String>, position: usize) -> Error {
        Error::invalid_syntax(message.into(), position)
    }

    #[inline]
    fn new(kind: Repr) -> Self {
        Error(Box::new(Payload {
            kind,
            goal_gated: false,
        }))
    }

    /// A general parse error at `position`.
    #[doc(hidden)]
    pub fn invalid_syntax(message: String, position: usize) -> Self {
        Error::new(Repr::InvalidSyntax {
            message,
            position,
            context: None,
        })
    }

    /// A general parse error at `position` that is a **goal gate** — a construct one
    /// parse goal admits and the other refuses, so the error is evidence about the
    /// source's grammar rather than about a mistake in it. See
    /// [`Error::is_goal_gated`].
    #[doc(hidden)]
    pub fn goal_gate(message: String, position: usize) -> Self {
        Error::invalid_syntax(message, position).into_goal_gate()
    }

    /// This error, marked a goal gate — for a refusal no construct-specific constructor
    /// builds: whatever error a name reading of `await` hit at the operand a module would
    /// have read (`tsv_ts`'s `Parser::parse`). Message and position are untouched.
    #[doc(hidden)]
    #[must_use]
    pub fn into_goal_gate(mut self) -> Self {
        self.0.goal_gated = true;
        self
    }

    /// Whether this error is a goal gate ([`Error::goal_gate`]).
    ///
    /// The format fallback holds two errors for one source when both goals reject it,
    /// and a *script* attempt that died on a goal gate has proved the source a module —
    /// it holds an `import` / `export` / `import.meta`, or a top-level `await` with an
    /// operand — whatever position either error reached, so the module attempt's error is
    /// the source's own.
    #[doc(hidden)]
    pub fn is_goal_gated(&self) -> bool {
        self.0.goal_gated
    }

    /// Found `found` where `expected` was required.
    #[doc(hidden)]
    pub fn unexpected_token(expected: String, found: String, position: usize) -> Self {
        Error::new(Repr::UnexpectedToken {
            expected,
            found,
            position,
            context: None,
        })
    }

    /// Input ended while more was required.
    #[doc(hidden)]
    pub fn unexpected_eof(position: usize) -> Self {
        Error::new(Repr::UnexpectedEof {
            position,
            context: None,
        })
    }

    /// Found `found` where an expression was required.
    #[doc(hidden)]
    pub fn invalid_expression(found: String, position: usize) -> Self {
        Error::new(Repr::InvalidExpression {
            found,
            position,
            context: None,
        })
    }

    /// The byte offset the error is reported at, in the coordinates the error was
    /// built in (host coordinates for a parser error; see [`Error::shift_position`]
    /// for the lexer case). `None` for the kinds that are not syntax errors, a source over
    /// the `u32` cap and an [`Error::refusal`]. A caller holding two errors for one source
    /// reads this to say which got further — the format fallback's module-vs-script choice.
    #[doc(hidden)]
    pub fn position(&self) -> Option<usize> {
        self.0.kind.located().map(|(position, _)| *position)
    }

    /// What kind of failure this is: a syntax error, a refusal of a source that parses, a
    /// depth limit, or a source over the size cap ([`ErrorKind`]).
    ///
    /// The surfaces split on it: the CLI prints `Parse error:` for [`ErrorKind::Syntax`]
    /// and [`ErrorKind::TooDeep`] (both located, so both carry a frame) and `Error:` for the
    /// rest, and a binding reads the refusal line from [`ErrorReport::refused`]. A
    /// whole-document parse entry point fills every located error's context, so on its
    /// errors `Syntax | TooDeep` is exactly `wire_point().is_some()`.
    #[inline]
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match &self.0.kind {
            Repr::UnexpectedToken { .. }
            | Repr::UnexpectedEof { .. }
            | Repr::InvalidSyntax { .. }
            | Repr::InvalidExpression { .. } => ErrorKind::Syntax,
            Repr::TooDeep { .. } => ErrorKind::TooDeep,
            Repr::FileTooLarge { .. } => ErrorKind::TooLarge,
            Repr::Refused { .. } => ErrorKind::Refusal,
        }
    }

    /// The error's byte range in the source the caller handed over — UTF-8 byte offsets,
    /// not the UTF-16 wire coordinates [`Error::report`] gives. A syntax error and a depth
    /// limit know a point, so their range is empty (`p..p`); a refusal's is the bytes it
    /// refuses; a source over the size cap has none (`None`).
    ///
    /// The offsets index the text the caller passed: a format entry point's error is
    /// mapped back through its line-terminator fold, so it points into the unfolded bytes,
    /// and an error inside an embedded `<script>` / `<style>` or template expression is
    /// absolute in the host document. A leading byte-order mark is counted, whatever a
    /// language's wire does with it — these are bytes.
    #[must_use]
    pub fn span(&self) -> Option<Range<usize>> {
        if let Repr::Refused { span, .. } = &self.0.kind {
            return Some(span.clone());
        }
        self.0
            .kind
            .located()
            .map(|(position, _)| *position..*position)
    }

    /// The [`Error::too_deep`] payload, built here where the kind is visible.
    pub(crate) fn depth_limit(limit: DepthLimit, position: usize) -> Self {
        Error::new(Repr::TooDeep {
            limit,
            position,
            context: None,
        })
    }

    /// Source exceeds the 4 GiB − 1 cap the `u32` span offsets assume.
    fn file_too_large(size: usize, max: usize) -> Self {
        Error::new(Repr::FileTooLarge { size, max })
    }

    /// A **refusal**: `source` parses, but an operation cannot handle what it holds — a
    /// format that cannot print it without changing what it means (a Svelte in-tag `//`
    /// comment holding a lone `<CR>`, which the format path's line-terminator fold would end
    /// early). It covers the bytes `start..end`. The type strip's refusals take the same
    /// report shape without an `Error` ([`ErrorReport::refusal`], via
    /// `tsv_ts::UnsupportedKind::report`).
    ///
    /// The message is rendered here, with the `line:col` header and excerpt of `start` in
    /// `source` under `coordinates` that a located error prints, so it reads like every other
    /// error, and the span is kept ([`Error::report`]). But it is **not a syntax error** —
    /// no [`Error::position`], no [`Error::wire_point`], [`Error::kind`]
    /// [`ErrorKind::Refusal`]: the source is not malformed (`parse` accepts it),
    /// so every surface reports it apart from a parse failure — the CLI's `Error:` prefix, the
    /// npm facade's `kind: 'refused'`.
    ///
    /// `source` must be the caller's own text: neither `Error::shift_position` nor the
    /// line-terminator fold's mapping moves a refusal.
    #[doc(hidden)]
    #[cold]
    #[inline(never)]
    pub fn refusal(
        message: &str,
        source: &str,
        start: usize,
        end: usize,
        coordinates: WireCoordinates,
    ) -> Self {
        let report = ErrorReport::refusal(message, source, start, end, coordinates);
        Error::new(Repr::Refused {
            message: report.summary.into(),
            frame: report.frame.into(),
            // the same ends the report's points are taken at: `end` never short of `start`
            span: start..end.max(start),
            start: report.start,
            end: report.end,
        })
    }

    /// The error as a binding reports it — summary, frame and span — for every error with a
    /// point: a syntax error or a depth limit (its span the point alone: `end == start`)
    /// and a refusal (its own span, `refused` set). `None` for a source
    /// over the size cap ([`ErrorKind::TooLarge`]).
    pub fn report(&self) -> Option<ErrorReport> {
        if let Repr::Refused {
            frame, start, end, ..
        } = &self.0.kind
        {
            return Some(ErrorReport {
                summary: self.0.kind.summary(),
                frame: frame.to_string(),
                start: *start,
                end: *end,
                refused: true,
            });
        }
        let (_, context) = self.0.kind.located()?;
        let context = context.as_ref()?;
        Some(ErrorReport {
            summary: self.0.kind.summary(),
            frame: context.frame(),
            start: context.point,
            end: context.point,
            refused: false,
        })
    }

    /// Reject a source longer than the `u32` span offsets can index (> 4 GiB − 1).
    /// Every public parse entry point calls this before touching the source; the
    /// lexers and `Span`/`Token` assume the cap holds rather than re-checking.
    #[doc(hidden)]
    pub fn ensure_source_fits(source: &str) -> Result<()> {
        // `try_from` rather than `len > u32::MAX as usize`: on a 32-bit target (wasm32)
        // `usize` is `u32`, the comparison is vacuous, and clippy rejects it as such.
        if u32::try_from(source.len()).is_err() {
            return Err(Error::file_too_large(source.len(), u32::MAX as usize));
        }
        Ok(())
    }

    /// Lift a position out of a lexer's own coordinates into the document's.
    ///
    /// An `Error::lex` position indexes the lexer's `source`, which for the TypeScript and
    /// CSS lexers is routinely a **slice** of the document the error is finally rendered
    /// against: a Svelte `<script>` / `<style>` island or template expression, the CSS
    /// declaration-value scan (`source[from..]`). [`Error::with_context`] is handed the
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
    #[doc(hidden)]
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
    /// prints and [`Error::wire_point`] reports, and the line excerpted under it. Each
    /// language crate's public parse entry points call this with that language's
    /// coordinates (`tsv_ts::WIRE_COORDINATES` and its siblings). The embedded entry points
    /// `tsv_svelte` uses for `<script>` / `<style>` bodies do not: their source is a slice of
    /// a host document, so their errors leave context-free and the host
    /// (`tsv_svelte::__private::parse`) fills them over the whole document.
    ///
    /// ```ignore
    /// parser::parse(source).map_err(|e| e.with_context(source, WIRE_COORDINATES))
    /// ```
    #[doc(hidden)]
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
    /// `None` for a source over the `u32` cap ([`ErrorKind::TooLarge`]) and for a refusal,
    /// whose span is [`Error::report`]'s.
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

impl Repr {
    /// The position and context slots of a located error, `None` for the kinds that are not
    /// syntax errors (`FileTooLarge`; `Refused`, whose point is fixed at construction).
    /// Which variants carry a position is stated here and in [`Repr::located_mut`]
    /// — one match cannot lend both a shared and an exclusive borrow — so `position`,
    /// `shift_position` and `with_context` restate no variant list of their own, and a new
    /// variant is classified in these two.
    fn located(&self) -> Option<(&usize, &Option<ErrorContext>)> {
        match self {
            Repr::UnexpectedToken {
                position, context, ..
            }
            | Repr::UnexpectedEof { position, context }
            | Repr::InvalidSyntax {
                position, context, ..
            }
            | Repr::InvalidExpression {
                position, context, ..
            }
            | Repr::TooDeep {
                position, context, ..
            } => Some((position, context)),
            Repr::FileTooLarge { .. } | Repr::Refused { .. } => None,
        }
    }

    /// [`Repr::located`], mutably — the same variant list, kept in step.
    fn located_mut(&mut self) -> Option<(&mut usize, &mut Option<ErrorContext>)> {
        match self {
            Repr::UnexpectedToken {
                position, context, ..
            }
            | Repr::UnexpectedEof { position, context }
            | Repr::InvalidSyntax {
                position, context, ..
            }
            | Repr::InvalidExpression {
                position, context, ..
            }
            | Repr::TooDeep {
                position, context, ..
            } => Some((position, context)),
            Repr::FileTooLarge { .. } | Repr::Refused { .. } => None,
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

    /// A column at the `u32` cap prints its 1-based form without overflowing: the cap
    /// admits a source of exactly `u32::MAX` bytes, so an EOF error on its single line
    /// sits at column `u32::MAX`.
    #[test]
    fn test_header_column_at_the_u32_cap() {
        let ctx = ErrorContext {
            source_line: "x".into(),
            caret_column: 1,
            point: point(u32::MAX, 1, u32::MAX),
            coordinates: ECMA,
        };
        let rendered = ctx.format_with_caret("Unexpected end of input");
        assert_eq!(header(&rendered), "1:4294967296", "{rendered:?}");
    }

    /// The goal-gate mark rides the payload through the two rewrites a located error
    /// takes on its way out of an embedded parse — the offset shift and the context
    /// fill — and every other constructor leaves it unset.
    #[test]
    fn test_goal_gate_mark_survives_shift_and_context() {
        let e = Error::goal_gate("'export' is only allowed in a module".to_string(), 0)
            .shift_position(3)
            .with_context("x; export {};", ECMA);
        assert!(e.is_goal_gated());
        assert_eq!(e.position(), Some(3));
        assert!(!Error::invalid_syntax("x".to_string(), 0).is_goal_gated());
        assert!(!Error::unexpected_eof(0).is_goal_gated());
    }

    /// `into_goal_gate` marks an error of any kind — the refusal a name `await` hits at
    /// its operand is whatever the name reading raised there — and changes nothing the
    /// error renders or reports.
    #[test]
    fn test_into_goal_gate_keeps_the_kind_and_its_message() {
        let plain = Error::unexpected_token("';'".to_string(), "identifier".to_string(), 6)
            .with_context("await x;", ECMA);
        let gated = plain.clone().into_goal_gate();
        assert!(!plain.is_goal_gated());
        assert!(gated.is_goal_gated());
        assert_eq!(gated.to_string(), plain.to_string());
        assert_eq!(gated.position(), Some(6));
        assert_eq!(gated.wire_point(), plain.wire_point());
    }

    /// `{:#}` forwards to the boxed kind, so the rendered message must be exactly what the
    /// payload's arms produce — both the bare `at position N` fallback and the caret form
    /// `with_context` fills in. The error-message fixtures and `input_invalid_*` gate this
    /// at the product level (the CLI prints `{:#}`); this pins it beside the forwarding impl.
    #[test]
    fn test_alternate_display_renders_the_frame() {
        let source = "let x =\ny";

        let e = Error::invalid_syntax("bad token".to_string(), 4);
        assert_eq!(format!("{e:#}"), "bad token at position 4");
        assert_eq!(
            format!("{:#}", e.with_context(source, ECMA)),
            "bad token\n1:5 let x =\n        ^ here"
        );

        let e = Error::unexpected_token("';'".to_string(), "'y'".to_string(), 8);
        assert_eq!(format!("{e:#}"), "Expected ';', found 'y' at position 8");
        assert_eq!(
            format!("{:#}", e.with_context(source, ECMA)),
            "Expected ';', found 'y'\n2:1 y\n    ^ here"
        );

        assert_eq!(
            format!("{:#}", Error::unexpected_eof(9)),
            "Unexpected end of file at position 9"
        );
        assert_eq!(
            format!("{:#}", Error::invalid_expression("'='".to_string(), 6)),
            "Expected expression, found '=' at position 6"
        );
        assert_eq!(
            format!("{:#}", Error::file_too_large(5, 4)),
            "File too large: 5 bytes (maximum: 4 bytes / 4 GiB)"
        );

        // `with_context` on the context-free variant is a no-op, not a panic, and it has
        // no point to report.
        let too_large = Error::file_too_large(5, 4).with_context(source, ECMA);
        assert_eq!(
            format!("{too_large:#}"),
            "File too large: 5 bytes (maximum: 4 bytes / 4 GiB)"
        );
        assert_eq!(too_large.wire_point(), None);
        // Nor does a located error no context has been filled for.
        assert_eq!(Error::unexpected_eof(9).wire_point(), None);

        // `Debug` prints the kind — not an `Error(..)` wrapper.
        assert!(
            format!("{:?}", Error::unexpected_eof(9)).starts_with("UnexpectedEof"),
            "Debug must forward to the kind"
        );
    }

    /// `{}` is one line: the summary and the frame header's `line:col`, a byte offset for a
    /// located error no context has been filled for, the summary alone without a position.
    #[test]
    fn test_display_is_one_line() {
        let source = "let x =\ny";
        let e = Error::invalid_syntax("bad token".to_string(), 4);
        assert_eq!(e.to_string(), "bad token (at byte 4)");
        assert_eq!(e.with_context(source, ECMA).to_string(), "bad token (1:5)");
        let e = Error::unexpected_token("';'".to_string(), "'y'".to_string(), 8)
            .with_context(source, ECMA);
        assert_eq!(e.to_string(), "Expected ';', found 'y' (2:1)");
        // the header's numbers, and the summary's first line, exactly
        let alternate = format!("{e:#}");
        let summary = alternate.lines().next().unwrap();
        assert_eq!(e.to_string(), format!("{summary} ({})", header(&alternate)));
        assert_eq!(
            Error::file_too_large(5, 4).to_string(),
            "File too large: 5 bytes (maximum: 4 bytes / 4 GiB)"
        );
        let refusal = Error::refusal("no", "a\nbc", 3, 4, LF);
        assert_eq!(refusal.to_string(), "no (2:2)");
        // a column at the `u32` cap prints whole, as the header does
        let at_cap = Error::new(Repr::UnexpectedEof {
            position: 0,
            context: Some(ErrorContext {
                source_line: "x".into(),
                caret_column: 1,
                point: point(u32::MAX, 1, u32::MAX),
                coordinates: ECMA,
            }),
        });
        assert_eq!(at_cap.to_string(), "Unexpected end of file (1:4294967296)");
    }

    /// `kind()` names each payload; `span()` is the point as an empty range for a located
    /// error, the refused bytes for a refusal, nothing for the size cap.
    #[test]
    fn test_kind_and_span() {
        let syntax = Error::invalid_syntax("bad".to_string(), 4).with_context("let x =\ny", ECMA);
        assert_eq!(syntax.kind(), ErrorKind::Syntax);
        assert_eq!(syntax.span(), Some(4..4));
        for e in [
            Error::unexpected_token("a".to_string(), "b".to_string(), 2),
            Error::unexpected_eof(2),
            Error::invalid_expression("b".to_string(), 2),
            Error::goal_gate("g".to_string(), 2),
        ] {
            assert_eq!(e.kind(), ErrorKind::Syntax, "{e:?}");
            // unfilled, the position is already the caller's
            assert_eq!(e.span(), Some(2..2), "{e:?}");
        }
        let deep = Error::depth_limit(DepthLimit::Nesting, 7);
        assert_eq!(deep.kind(), ErrorKind::TooDeep);
        assert_eq!(deep.span(), Some(7..7));
        let refusal = Error::refusal("no", "ab\ncd", 3, 5, LF);
        assert_eq!(refusal.kind(), ErrorKind::Refusal);
        assert_eq!(refusal.span(), Some(3..5));
        // an end short of the start is the start, as the report's points take it
        assert_eq!(Error::refusal("no", "abcd", 3, 1, LF).span(), Some(3..3));
        let too_large = Error::file_too_large(5, 4);
        assert_eq!(too_large.kind(), ErrorKind::TooLarge);
        assert_eq!(too_large.span(), None);
        // the goal-gate mark and a shift move nothing but what they say
        let shifted = Error::invalid_syntax("bad".to_string(), 2).shift_position(10);
        assert_eq!(shifted.span(), Some(12..12));
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
                    let e = Error::invalid_syntax("bad".to_string(), position)
                        .with_context(source, coordinates);
                    let p = e.wire_point().expect("a located, filled error has a point");
                    let rendered = format!("{e:#}");
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

    /// A refusal renders the located error's `line:col` header and excerpt and reports its
    /// span, but carries no position a binding could report as a syntax error's point — and
    /// an unfold, which re-takes a located error's context, leaves its already-caller-side
    /// text alone.
    #[test]
    fn a_refusal_renders_its_point_but_is_no_syntax_error() {
        let source = "ab\nc\rd";
        // The `<CR>` itself, as the Svelte format entry's refusal points: the header counts
        // `\n` lines, the excerpt stops at the `<CR>`.
        let refusal = Error::refusal("no", source, 4, 5, LF);
        assert_eq!(format!("{refusal:#}"), "no\n2:2 c\n     ^ here");
        assert_eq!(refusal.position(), None);
        assert_eq!(refusal.wire_point(), None);
        assert_eq!(refusal.kind(), ErrorKind::Refusal);
        assert_eq!(refusal.span(), Some(4..5));
        assert_eq!(
            refusal.report(),
            Some(ErrorReport {
                summary: "no".to_owned(),
                frame: "2:2 c\n     ^ here".to_owned(),
                start: point(4, 2, 1),
                end: point(5, 2, 2),
                refused: true,
            })
        );
        assert_eq!(
            format!("{:#}", refusal.clone().unfold(source)),
            format!("{refusal:#}")
        );
    }

    /// A summary quoting source text never carries a raw line terminator: LF, CR, U+2028 and
    /// U+2029 are spelled `<U+XXXX>`, in the message and the report alike, for a syntax
    /// error and a refusal — so the summary stays one line and the message stays
    /// `summary + "\n" + frame`.
    #[test]
    fn a_summary_spells_out_its_line_terminators() {
        let source = "x\ny";
        for (raw, spelled) in [
            ("a\nb", "a<U+000A>b"),
            ("a\rb", "a<U+000D>b"),
            ("a\r\nb", "a<U+000D><U+000A>b"),
            ("a\u{2028}b", "a<U+2028>b"),
            ("a\u{2029}b", "a<U+2029>b"),
            ("plain", "plain"),
        ] {
            let e = Error::invalid_syntax(format!("bad '{raw}'"), 2).with_context(source, ECMA);
            let report = e.report().expect("a filled syntax error reports");
            assert_eq!(report.summary, format!("bad '{spelled}'"));
            assert_eq!(
                format!("{e:#}"),
                format!("{}\n{}", report.summary, report.frame)
            );
            let e = Error::unexpected_token("';'".to_string(), format!("'{raw}'"), 2)
                .with_context(source, ECMA);
            assert_eq!(
                e.report().expect("reports").summary,
                format!("Expected ';', found '{spelled}'")
            );
            let refusal = Error::refusal(raw, source, 2, 3, LF);
            let report = refusal.report().expect("a refusal reports");
            assert_eq!(report.summary, spelled);
            assert_eq!(
                format!("{refusal:#}"),
                format!("{}\n{}", report.summary, report.frame)
            );
        }
    }

    /// A filled syntax error reports its summary and frame apart — the two its message
    /// joins with one `\n` — over a one-point span; an unfilled one and the size cap report
    /// nothing.
    #[test]
    fn a_syntax_error_reports_its_summary_frame_and_point() {
        let source = "let x =\ny";
        let e = Error::unexpected_token("';'".to_string(), "'y'".to_string(), 8);
        assert_eq!(e.report(), None);
        let e = e.with_context(source, ECMA);
        let report = e.report().expect("a filled syntax error reports");
        assert_eq!(report.summary, "Expected ';', found 'y'");
        assert_eq!(report.frame, "2:1 y\n    ^ here");
        assert_eq!(report.start, point(8, 2, 0));
        assert_eq!(report.end, report.start);
        assert!(!report.refused);
        assert_eq!(
            format!("{e:#}"),
            format!("{}\n{}", report.summary, report.frame)
        );
        assert_eq!(Error::file_too_large(5, 4).report(), None);
    }

    /// An error raised over the folded text, unfolded, is the error a parse of the original
    /// would report at the same character: position, point and message.
    #[test]
    fn unfold_error_retakes_the_context_over_the_original() {
        let original = "a;\r\nb;\r\n  @";
        let folded = crate::printing::normalize_carriage_returns(original);
        let at_folded = folded.text().find('@').expect("@");
        let e =
            Error::invalid_syntax("bad".to_string(), at_folded).with_context(folded.text(), ECMA);
        let unfolded = folded.unfold_error(e);
        let at_original = original.find('@').expect("@");
        let direct =
            Error::invalid_syntax("bad".to_string(), at_original).with_context(original, ECMA);
        assert_eq!(unfolded.position(), Some(at_original));
        assert_eq!(unfolded.wire_point(), direct.wire_point());
        assert_eq!(format!("{unfolded:#}"), format!("{direct:#}"));
        assert_eq!(unfolded.wire_point(), Some(point(10, 3, 2)));

        // A text the fold left alone is handed back untouched.
        let plain = "a;\n@";
        let folded = crate::printing::normalize_carriage_returns(plain);
        let e = Error::invalid_syntax("bad".to_string(), 3).with_context(plain, LF);
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
