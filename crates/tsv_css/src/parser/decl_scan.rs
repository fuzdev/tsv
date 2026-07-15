// Boundary scan for a declaration's value.
//
// A declaration's value text is walked more than once: this scan finds where the value
// ends (and the few facts the declaration node needs), and `parser::value` then parses
// the same text byte-wise into the `CssValue` tree. This module makes the *first* walk a
// byte scan instead of a token walk — the value's bytes are overwhelmingly inert
// identifier/number content, and tokenizing them to find a `;` is paying a lexer to
// answer a question the raw bytes already answer.
//
// Two implementations of one contract:
//
//   - `scan_value_tokens` — the reference. Drives the real `Lexer` token by token, exactly
//     as the value loop always has. It is the definition of correct, it is what runs
//     whenever the byte scan declines, and in debug builds it re-derives every fact behind
//     the byte scan's back (see `scan_value` — the whole test suite is the proof).
//   - `scan_value_bytes` — the fast path. Returns `None` ("I decline") for anything it
//     cannot decide exactly, which hands the value to the reference walk. That is what
//     keeps lexer errors — an unterminated string, a bad escape, a stray backtick — the
//     reference's job alone: the byte scan never has to *reject*, only to recognize the
//     shapes it fully models.

use super::CssParser;
use crate::lexer::{IDENT_CONTINUE_LUT, Lexer, TokenKind, is_ascii_css_whitespace};
use tsv_lang::ParseError;

/// The token that closes a declaration's value. Exactly three can, so the scan reports
/// which one rather than a general `TokenKind`: `CssParser::seat_at_terminator` builds that
/// token instead of lexing it, and a narrow type is what stops it from ever building one of
/// the wrong width. A `TokenKind` there would need a catch-all arm covering ~30 variants
/// that cannot occur — and would silently mis-seat any that later did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TerminatorKind {
    Semicolon,
    RightBrace,
    Eof,
}

impl TerminatorKind {
    /// The lexer token this stands for, and its width in bytes: `;` and `}` are one byte,
    /// and the EOF token is zero-width at end-of-source.
    pub(super) fn token(self) -> (TokenKind, usize) {
        match self {
            Self::Semicolon => (TokenKind::Semicolon, 1),
            Self::RightBrace => (TokenKind::RightBrace, 1),
            Self::Eof => (TokenKind::Eof, 0),
        }
    }
}

/// Everything the declaration node needs from its value's text. Offsets are raw
/// `source` offsets (the caller shifts them into host coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ValueFacts {
    /// Where the value's terminator token begins: the `;` / `}` that closes it at depth
    /// zero, or end-of-source. The parser re-seats its lexer here.
    pub(super) terminator: usize,
    /// Which token that is. Both walks branch on it to stop, so the parser seats it
    /// directly rather than lexing the byte a second time.
    pub(super) terminator_kind: TerminatorKind,
    /// End of the value's span — the end of its last non-whitespace token, already rolled
    /// back past a trailing `!important` when there is one.
    pub(super) value_end: usize,
    /// End of the `important` identifier, when the value ends in `!important`.
    pub(super) important_end: Option<usize>,
    /// Whether a `/* */` comment appears anywhere in the value (at any nesting depth).
    /// Comment-only values are legal, so this also gates the empty-value error.
    pub(super) has_comment: bool,
    /// Whether the value holds no tokens at all besides whitespace, comments, and a
    /// trailing `!important`.
    pub(super) is_empty: bool,
}

/// Bytes the byte scan can skip outright: everything that can move none of its state.
///
/// That is every ASCII byte except the ones it must inspect — the nesting and terminator
/// punctuation (`( ) { } [ ] ;`), the quote and comment introducers (`" ' /`), the `!` of
/// `!important`, and the `\` escape introducer (which it declines on). Whitespace is
/// skippable too: it moves no state, and the two places its position matters (the value's
/// end, and the roll-back before a `!`) are recovered afterwards by a trim.
///
/// Identifier letters, digits, `-`, `#`, `%`, `.` — the overwhelming bulk of a value's
/// text — all land here, so a whole `var(--fuz_color_a_5)` costs one L1 load per byte.
///
/// **Only the ASCII half is populated.** A byte `>= 0x80` is never skipped, so it reaches
/// the match's catch-all and the scan declines: a non-ASCII byte at content position may
/// be Unicode whitespace (NBSP), an identifier code point, or a lexer error, and the byte
/// scan does not model that fork. It is unreachable inside a string, comment, or url-token
/// (those are consumed opaquely by their own sub-scans), which is where non-ASCII text in
/// real CSS actually lives.
const SKIP: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 128 {
        let b = i as u8;
        t[i] = !value_scan_inspects(b) && (is_ascii_css_whitespace(b) || is_inert_content(b));
        i += 1;
    }
    t
};

/// The bytes `scan_value_bytes` has a match arm for. **This must stay in lockstep with that
/// match**: a byte named here but unhandled there is merely slow, but a byte handled there and
/// *missing* here is skipped by the table and its arm goes dead — a silent misparse. (The
/// debug oracle would catch it; this exists so it never gets that far.)
const fn value_scan_inspects(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'{' | b'}' | b'[' | b']' | b';' | b'"' | b'\'' | b'/' | b'!' | b'\\'
    )
}

/// An ASCII byte that lexes to a token none of these scans ever looks at: it can neither
/// nest, nor terminate, nor open a string/comment/url, nor be the `!` of `!important`.
///
/// Deliberately an allow-list, not a deny-list: an ASCII byte that is *neither* inert here
/// *nor* one of the inspected ones is a byte the lexer itself rejects (a control character,
/// a backtick), so leaving it out of the table routes it to the catch-all and the scan
/// declines — letting the reference walk raise the very error the lexer would.
///
/// `[` and `]` are absent because they are not inert *everywhere*: they nest in a value
/// (a custom property's `<declaration-value>` permits balanced `[]`), but the
/// rule-or-declaration scan ignores them, so it adds them to its own table.
const fn is_inert_content(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'_' | b'-'
                | b'.'
                | b'#'
                | b'%'
                | b'*'
                | b'&'
                | b'@'
                | b'='
                | b'^'
                | b'?'
                | b'~'
                | b'>'
                | b'<'
                | b'+'
                | b':'
                | b','
                | b'|'
                | b'$'
        )
}

/// Bytes the rule-or-declaration scan can skip. It asks a narrower question than the value
/// scan — does a `{` arrive before a `;`/`}` at paren depth zero? — so it inspects strictly
/// less: it tracks only parens, and `[`, `]` and `!` all join the inert set (the token walk
/// it fronts ignores them too). Same ASCII-half-only rule.
const SKIP_RULE: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 128 {
        let b = i as u8;
        t[i] = !rule_scan_inspects(b)
            && (is_ascii_css_whitespace(b)
                || is_inert_content(b)
                || matches!(b, b'[' | b']' | b'!'));
        i += 1;
    }
    t
};

/// The bytes `scan_rule_or_declaration_bytes` has a match arm for — strictly fewer than the
/// value scan's, since it tracks only parens. Same lockstep requirement as
/// [`value_scan_inspects`].
const fn rule_scan_inspects(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'{' | b'}' | b';' | b'"' | b'\'' | b'/' | b'\\'
    )
}

/// Whether the `identifier :` at `from` (an offset just past the identifier) opens a
/// **nested rule** rather than a declaration.
///
/// `color: red;` and `span:hover { }` are both `identifier` `:` — they are told apart only
/// by what terminates the run: a `{` (a rule) or a `;`/`}` (a declaration). Answering it
/// means walking the whole value, which is why it is a byte scan and not a token walk;
/// `None` declines, exactly as `scan_value_bytes` does, and for the same reasons.
///
/// Only paren depth is tracked, matching the token walk this fronts: a `;` inside a `[…]`
/// really does end the run here.
fn scan_rule_or_declaration_bytes(source: &str, from: usize) -> Option<bool> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = from;
    let mut paren: u32 = 0;

    loop {
        while i < len && SKIP_RULE[bytes[i] as usize] {
            i += 1;
        }
        if i >= len {
            return Some(false); // EOF — not a rule
        }
        match bytes[i] {
            b'{' if paren == 0 => return Some(true),
            b';' | b'}' if paren == 0 => return Some(false),
            b'{' | b';' | b'}' => i += 1,
            b')' => {
                paren = paren.saturating_sub(1);
                i += 1;
            }
            b'(' => match url_token_end(source, bytes, from, i) {
                Some(end) => i = end,
                None => {
                    paren += 1;
                    i += 1;
                }
            },
            b'"' | b'\'' => i = string_end(bytes, i)?,
            b'/' if bytes.get(i + 1) == Some(&b'*') => i = comment_end(bytes, i)?,
            b'/' => i += 1,
            _ => return None,
        }
    }
}

/// Byte scan first, the token walk on decline — and in debug the token walk runs behind a
/// successful byte scan and must agree, so the test suite proves the equivalence.
pub(super) fn scan_rule_or_declaration(
    parser: &CssParser<'_, '_>,
    from: usize,
) -> Result<bool, ParseError> {
    let source = parser.source();
    match scan_rule_or_declaration_bytes(source, from) {
        Some(is_rule) => {
            #[cfg(debug_assertions)]
            {
                // An `Err` here fails the assert too, and must: it would mean the byte scan
                // accepted input the lexer rejects.
                let expected = scan_rule_or_declaration_tokens(source, from);
                assert!(
                    expected.as_ref().is_ok_and(|expected| *expected == is_rule),
                    "rule-or-declaration byte scan disagreed with the token walk at {from}: \
                     scan said {is_rule}, walk said {expected:?}"
                );
            }
            Ok(is_rule)
        }
        None => scan_rule_or_declaration_tokens(source, from),
    }
}

/// The reference: the disambiguation as a token walk, on its own lexer.
///
/// ⚠️ Lexes a **slice** (`source[from..]`), not the whole source from an offset. The two are
/// interchangeable for the verdict — this walk reads only token *kinds* — but not for a lexer
/// **error**, whose position would then be absolute rather than slice-relative. A declaration
/// whose value holds a stray backtick reports that position, so the slice is load-bearing:
/// swapping it to a `seek` silently moves the caret in every such error.
fn scan_rule_or_declaration_tokens(source: &str, from: usize) -> Result<bool, ParseError> {
    let mut lexer = Lexer::new(&source[from..]);
    // `u32` so an unbalanced close saturates at zero — see `scan_value_tokens`.
    let mut paren: u32 = 0;
    loop {
        let token = lexer.next_token().map_err(|err| *err)?;
        match token.kind {
            TokenKind::LeftParen => paren += 1,
            TokenKind::RightParen => paren = paren.saturating_sub(1),
            TokenKind::LeftBrace if paren == 0 => return Ok(true),
            TokenKind::Semicolon | TokenKind::RightBrace if paren == 0 => return Ok(false),
            TokenKind::Eof => return Ok(false),
            _ => {}
        }
    }
}

/// The kind of the next significant token after `from`, when the bytes settle it.
///
/// Every block child that starts with an identifier asks this once, and asks it only to
/// learn whether a `:` follows the name — `color` is a property, `span` is a type selector,
/// and the colon is the whole difference. So a `:` is the one kind recognized here:
/// whitespace and comments are trivia and get skipped, and **everything else declines**
/// (`None`), including bytes whose token is perfectly obvious.
///
/// Declining on the negative is deliberate. It costs one short whitespace scan on the rarer
/// nested-rule child, and it buys the property the rest of this module rests on: the scan
/// never has to *reject*, so a lexer error stays `peek_past_whitespace`'s alone, reported at
/// its own position. Widening the accept set would mean re-deriving which bytes the lexer
/// can error on — the same bet `scan_value_bytes` declines to make.
fn peek_significant_kind_bytes(bytes: &[u8], from: usize) -> Option<TokenKind> {
    let len = bytes.len();
    let mut i = from;
    loop {
        while i < len && is_ascii_css_whitespace(bytes[i]) {
            i += 1;
        }
        match bytes.get(i)? {
            b':' => return Some(TokenKind::Colon),
            b'/' if bytes.get(i + 1) == Some(&b'*') => i = comment_end(bytes, i)?,
            _ => return None,
        }
    }
}

/// Byte scan first, `peek_past_whitespace` on decline — and in debug the token lookahead
/// runs behind a successful byte scan and must agree, so the test suite proves the
/// equivalence.
pub(super) fn peek_significant_kind(parser: &CssParser<'_, '_>) -> Result<TokenKind, ParseError> {
    match peek_significant_kind_bytes(parser.source().as_bytes(), parser.current_end) {
        Some(kind) => {
            #[cfg(debug_assertions)]
            {
                // An `Err` here fails the assert too, and must: it would mean the byte scan
                // accepted a lookahead the lexer rejects.
                let expected = parser.peek_past_whitespace();
                assert!(
                    expected.as_ref().is_ok_and(|expected| *expected == kind),
                    "significant-kind byte scan disagreed with the token lookahead at {}: \
                     scan said {kind:?}, lookahead said {expected:?}",
                    parser.current_end
                );
            }
            Ok(kind)
        }
        None => parser.peek_past_whitespace(),
    }
}

/// Scan a declaration's value from `value_start` (the start of its first token — the
/// caller has already consumed the `:` and the whitespace after it).
///
/// Byte scan first, reference token walk on decline. In debug builds the reference *also*
/// runs behind a successful byte scan and must agree fact for fact, so every value in
/// every fixture and every unit test re-proves the equivalence.
pub(super) fn scan_value(
    parser: &CssParser<'_, '_>,
    value_start: usize,
) -> Result<ValueFacts, ParseError> {
    let source = parser.source();
    match scan_value_bytes(source, value_start) {
        Some(facts) => {
            #[cfg(debug_assertions)]
            {
                // An `Err` here fails the assert too, and must: it would mean the byte scan
                // accepted a value the lexer rejects, silently dropping a parse error.
                let expected = scan_value_tokens(source, value_start);
                assert!(
                    expected.as_ref().is_ok_and(|expected| *expected == facts),
                    "declaration value byte scan disagreed with the token walk at \
                     {value_start}: scan said {facts:?}, walk said {expected:?}"
                );
            }
            Ok(facts)
        }
        None => scan_value_tokens(source, value_start),
    }
}

/// The fast path. `None` = "I decline" — hand the value to `scan_value_tokens`.
fn scan_value_bytes(source: &str, value_start: usize) -> Option<ValueFacts> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = value_start;

    // `u32` so an unbalanced close saturates at zero rather than going negative and
    // disabling the depth-zero terminator tests — the same rule, for the same reason, as
    // the token walk it replaces.
    let mut paren: u32 = 0;
    let mut brace: u32 = 0;
    let mut bracket: u32 = 0;
    let mut has_comment = false;
    // The *last* `!` at content position, at any depth. `!important` must be the value's
    // final two tokens, so only the last `!` can possibly open it; a `!` nested in parens
    // (or followed by anything but `important`) is rejected by the forward check below.
    let mut last_bang: Option<usize> = None;

    let (terminator, terminator_kind) = loop {
        while i < len && SKIP[bytes[i] as usize] {
            i += 1;
        }
        if i >= len {
            break (len, TerminatorKind::Eof);
        }
        let at_top = paren == 0 && brace == 0 && bracket == 0;
        match bytes[i] {
            b';' if at_top => break (i, TerminatorKind::Semicolon),
            b'}' if at_top => break (i, TerminatorKind::RightBrace),
            b';' => i += 1,
            b'}' => {
                brace = brace.saturating_sub(1);
                i += 1;
            }
            b'{' => {
                brace += 1;
                i += 1;
            }
            b'[' => {
                bracket += 1;
                i += 1;
            }
            b']' => {
                bracket = bracket.saturating_sub(1);
                i += 1;
            }
            b')' => {
                paren = paren.saturating_sub(1);
                i += 1;
            }
            // A `url(…)` is ONE opaque token (css-syntax §4.3.6), so its parens are content,
            // not nesting — an interior `;` or `)` must not be seen. Any other `(` nests.
            b'(' => match url_token_end(source, bytes, value_start, i) {
                Some(end) => i = end,
                None => {
                    paren += 1;
                    i += 1;
                }
            },
            b'"' | b'\'' => i = string_end(bytes, i)?,
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                has_comment = true;
                i = comment_end(bytes, i)?;
            }
            // A lone `/` is an ordinary token (`font: 1rem/1.5 sans`).
            b'/' => i += 1,
            b'!' => {
                last_bang = Some(i);
                i += 1;
            }
            // `\` (an escape — the identifier it opens decodes, which the byte scan does not
            // model), a non-ASCII byte, and every ASCII byte the lexer rejects.
            _ => return None,
        }
    };

    // `!important` is the value's last two tokens: the `!`, then — across nothing but
    // whitespace and comments — an identifier spelling `important`, and then nothing but
    // whitespace and comments to the terminator.
    let important = last_bang.and_then(|bang| {
        let name_start = skip_trivia(bytes, bang + 1, terminator);
        let mut name_end = name_start;
        while name_end < terminator && IDENT_CONTINUE_LUT[bytes[name_end] as usize] {
            name_end += 1;
        }
        if !source[name_start..name_end].eq_ignore_ascii_case("important") {
            return None;
        }
        if skip_trivia(bytes, name_end, terminator) < terminator {
            return None;
        }
        Some((bang, name_end))
    });

    // The value's span ends at its last non-whitespace token — which is a trailing trim,
    // since every token but whitespace ends on a non-whitespace byte. A `!important` rolls
    // it back to just before the `!`, keeping any comment that sat between.
    let (span_end, important_end) = match important {
        Some((bang, name_end)) => (bang, Some(name_end)),
        None => (terminator, None),
    };
    let value_end = trim_end(bytes, value_start, span_end);

    // Empty = no tokens besides whitespace, comments, and the `!important`.
    let is_empty = skip_trivia(bytes, value_start, span_end) >= span_end;

    Some(ValueFacts {
        terminator,
        terminator_kind,
        value_end,
        important_end,
        has_comment,
        is_empty,
    })
}

/// End of the string opened at `open` (past its closing quote), or `None` when the lexer
/// would reject it — unterminated, or a trailing backslash at end-of-source.
///
/// Mirrors `lexer::strings::read_string`: the quote and `\` are ASCII, so a multi-byte
/// char's continuation bytes (all `>= 0x80`) match neither and the run passes over them.
fn string_end(bytes: &[u8], open: usize) -> Option<usize> {
    let quote = bytes[open];
    let len = bytes.len();
    let mut p = open + 1;
    loop {
        while p < len && bytes[p] != quote && bytes[p] != b'\\' {
            p += 1;
        }
        if p >= len {
            return None; // unterminated
        }
        if bytes[p] == quote {
            return Some(p + 1);
        }
        if p + 1 >= len {
            return None; // backslash at end of source
        }
        p += 2;
    }
}

/// End of the comment opened at `open` (past its `*/`), or `None` when it is unterminated.
/// Mirrors `lexer::comments::read_comment`.
fn comment_end(bytes: &[u8], open: usize) -> Option<usize> {
    let len = bytes.len();
    let mut p = open + 2;
    loop {
        while p < len && bytes[p] != b'*' {
            p += 1;
        }
        if p >= len {
            return None; // unterminated
        }
        if bytes.get(p + 1) == Some(&b'/') {
            return Some(p + 2);
        }
        p += 1;
    }
}

/// If the `(` at `open` closes a `url` **identifier token**, the end of the opaque
/// url-token it opens; `None` for an ordinary nesting paren.
///
/// The subtlety is that `url` only opens a url-token when it is a *token start*. `5url(`
/// lexes as one `Dimension` (`url` is the unit) followed by a plain `(`, so its interior
/// `;` really does terminate the declaration; `blurl(` is one identifier; `$url(` is the
/// identifier `$url`. All three are decided by the single byte *before* the `url`: if it
/// can continue an identifier (or is the `$` that can open one), the `url` is not a token
/// start. That is why the value's numbers never need tokenizing.
///
/// The three bytes before a content-position `(` can never be the tail of a region this
/// scan skipped — a string ends in a quote, a comment in `*/`, a url-token in `)`, and none
/// of those spell `url` — so reading them raw is sound.
fn url_token_end(source: &str, bytes: &[u8], value_start: usize, open: usize) -> Option<usize> {
    if open < 3 || open - 3 < value_start {
        return None;
    }
    if !bytes[open - 3..open].eq_ignore_ascii_case(b"url") {
        return None;
    }
    // `open - 3 == value_start` needs no check: the byte before the value's first token is
    // the `:` or the whitespace after it, neither of which continues an identifier.
    if open - 3 > value_start {
        let prev = bytes[open - 4];
        if IDENT_CONTINUE_LUT[prev as usize] || prev == b'$' {
            return None;
        }
    }
    // A quoted argument makes it a function-token (`url("…")` lexes as ident + `(` +
    // string), not a url-token. `char::is_whitespace`, matching the lexer.
    let mut i = open + 1;
    while let Some(ch) = source[i..].chars().next() {
        if ch.is_whitespace() {
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    if matches!(source[i..].chars().next(), Some('"' | '\'')) {
        return None;
    }
    // Opaque to the matching unescaped `)`, or end-of-source (an unterminated url-token is
    // taken as-is — the lexer does not model bad-url recovery either).
    let len = bytes.len();
    let mut j = open + 1;
    loop {
        while j < len && bytes[j] != b'\\' && bytes[j] != b')' {
            j += 1;
        }
        if j >= len {
            break;
        }
        if bytes[j] == b')' {
            j += 1;
            break;
        }
        j += 1;
        if j < len {
            j += 1;
        }
    }
    Some(j)
}

/// First offset in `[from, to)` that is neither whitespace nor inside a comment, or `to`.
/// Every comment in the range is known terminated — the scan already walked it.
fn skip_trivia(bytes: &[u8], from: usize, to: usize) -> usize {
    let mut i = from;
    while i < to {
        if is_ascii_css_whitespace(bytes[i]) {
            i += 1;
        } else if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            match comment_end(bytes, i) {
                Some(end) => i = end,
                None => return to,
            }
        } else {
            return i;
        }
    }
    to
}

/// `to`, walked back over trailing whitespace (never below `from`).
fn trim_end(bytes: &[u8], from: usize, to: usize) -> usize {
    let mut e = to;
    while e > from && is_ascii_css_whitespace(bytes[e - 1]) {
        e -= 1;
    }
    e
}

/// The reference: the value loop as a token walk, on its own lexer.
///
/// This is the contract `scan_value_bytes` must reproduce, and the path every value the
/// byte scan declines still takes — so lexer errors (and their exact positions) stay
/// exactly where they were.
///
/// ⚠️ Seeks into the **whole source**, unlike [`scan_rule_or_declaration_tokens`], which lexes
/// a slice. The asymmetry is not an oversight: this walk stands in for one the *parser* used
/// to drive on its own lexer, so its error positions are source-absolute, while the rule walk
/// stands in for a temp lexer over a slice, whose error positions are slice-relative. Each
/// must keep the coordinates its caller already reports.
fn scan_value_tokens(source: &str, value_start: usize) -> Result<ValueFacts, ParseError> {
    let mut lexer = Lexer::new(source);
    lexer.seek(value_start);

    let mut has_comment = false;
    let mut value_end = value_start;
    // The value text is re-extracted verbatim from source, so this walk never materializes
    // the tokens — it needs only (a) whether any token exists and (b) enough about the last
    // two to strip a trailing `!important`. A rolling two-token window does both.
    let mut part_count: usize = 0;
    let mut last_is_bang = false;
    let mut last_is_important = false;
    let mut last_ends: (usize, usize) = (0, 0);
    let mut prev_is_bang = false;
    let mut prev_ends: (usize, usize) = (0, 0);
    let mut paren: u32 = 0;
    let mut brace: u32 = 0;
    let mut bracket: u32 = 0;

    // The terminator token's own start — where the parser re-seats its lexer. At EOF the
    // token is zero-width at end-of-source, so the same field serves both exits.
    let (terminator, terminator_kind) = loop {
        let token = lexer.next_token().map_err(|err| *err)?;
        let decoded = lexer.take_decoded();
        if token.kind == TokenKind::Eof {
            break (token.start as usize, TerminatorKind::Eof);
        }
        if paren == 0 && brace == 0 && bracket == 0 {
            match token.kind {
                TokenKind::Semicolon => break (token.start as usize, TerminatorKind::Semicolon),
                TokenKind::RightBrace => break (token.start as usize, TerminatorKind::RightBrace),
                _ => {}
            }
        }

        match token.kind {
            TokenKind::LeftParen => paren += 1,
            TokenKind::RightParen => paren = paren.saturating_sub(1),
            TokenKind::LeftBrace => brace += 1,
            TokenKind::RightBrace => brace = brace.saturating_sub(1),
            TokenKind::LeftBracket => bracket += 1,
            TokenKind::RightBracket => bracket = bracket.saturating_sub(1),
            _ => {}
        }

        let (start, end) = (token.start as usize, token.end as usize);
        let (is_bang, is_important) = match token.kind {
            // An identifier can't be `!`; it can be `important` (case-insensitive), and an
            // escaped spelling counts — hence the decoded value.
            TokenKind::Identifier => {
                let text = decoded
                    .as_deref()
                    .map_or(&source[start..end], |s| s.as_str());
                (false, text.eq_ignore_ascii_case("important"))
            }
            // A quoted string / number / percentage / dimension is never `!` or `important`.
            TokenKind::String { .. }
            | TokenKind::Number
            | TokenKind::Percentage
            | TokenKind::Dimension { .. } => (false, false),
            TokenKind::Whitespace => continue,
            TokenKind::Comment => {
                // Not a token of the value, but it does extend the declaration's span.
                has_comment = true;
                value_end = end;
                continue;
            }
            TokenKind::Bang => (true, false),
            _ => {
                let text = &source[start..end];
                (text == "!", text.eq_ignore_ascii_case("important"))
            }
        };

        prev_is_bang = last_is_bang;
        prev_ends = last_ends;
        last_is_bang = is_bang;
        last_is_important = is_important;
        last_ends = (value_end, end);
        part_count += 1;
        value_end = end;
    };

    // `!important`: the second-to-last token is `!` and the last is `important`.
    let important_matched = part_count >= 2 && prev_is_bang && last_is_important;
    let important_end = if important_matched {
        // The `important` token's own end — a trailing comment may have pushed `value_end`
        // past it. The value span rolls back to just before the `!` was scanned, which
        // keeps any comment sitting between the value and the `!`.
        let end_with_important = last_ends.1;
        value_end = prev_ends.0;
        Some(end_with_important)
    } else {
        None
    };

    // Every token is non-empty, so "no tokens remain after the optional `!important`
    // strip" is a count check.
    let is_empty = if important_matched {
        part_count - 2 == 0
    } else {
        part_count == 0
    };

    Ok(ValueFacts {
        terminator,
        terminator_kind,
        value_end,
        important_end,
        has_comment,
        is_empty,
    })
}
