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
//
// The value byte-scan loop lives in `scan_value_core`, shared by two callers. The second
// is the rule/declaration disambiguation: a block child `identifier :` is told a
// declaration from a nested rule (`color: red;` vs `span:hover { }`) by walking its value
// to the first paren-depth-0 `{` (rule) or `;`/`}` (declaration) — the *same* bytes
// `parse_declaration` then walks again for its facts. `scan_rule_or_declaration` runs the
// shared loop with the verdict latch on (`WANT_VERDICT`), so that one walk answers the
// disambiguation *and*, for a declaration, produces the `ValueFacts` the parser stashes for
// `parse_declaration` to reuse — one walk where two separate byte scans per
// non-custom declaration would otherwise run. The same holds one step earlier: the walk's
// first phase locates the `:` and the value's first byte (`DeclarationHead`), which is all
// `parse_declaration` drove its lexer across the property→colon gap to learn, so on a
// stashed declaration the parser lexes nothing between the property and the terminator. A
// declaration with no stash — a custom property, which the disambiguation never sees, or a
// property whose disambiguation byte scan declined — runs the same head on its own.
// The verdict tracks paren depth only (the model of `scan_rule_or_declaration_tokens`): a
// `;` inside `[…]` really does end the disambiguation run, even though the value scan
// (which tracks `[]`/`{}` too) reads past it — the shared loop maintains both, and the two
// agree on the verdict because paren depth evolves identically in each.

use super::CssParser;
use crate::comments::{comment_end_checked, is_comment_start};
use crate::lexer::{
    IDENT_CONTINUE_LUT, Lexer, TokenKind, is_ascii_css_whitespace, string_end, url_arg_is_quoted,
    url_token_close,
};
use crate::parser::value::lists::ValueSeparator;
use crate::parser::value::scan::is_value_separator;
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
    /// Whether the value's first parenthesized group — the first `(` to raise the paren
    /// depth from zero — closes on the value's last byte (`value_end - 1`).
    ///
    /// Not one of the declaration node's facts: it is the value parser's, which asks the
    /// same question of a function leaf (`name(args)` is a function only when the name's
    /// `(` closes at the very end) by scanning the group a second time. Both walks here
    /// cross the group anyway and keep the paren depth, so they note where it first
    /// returns to zero. See `crate::parser::value::plain_function_parts` for where it is
    /// spent and what holds it to the scan it replaces.
    pub(super) group_closes_value: bool,
}

/// Bytes the byte scan can skip outright: everything that can move none of its state.
///
/// That is every ASCII byte except the ones it must inspect — the nesting and terminator
/// punctuation (`( ) { } [ ] ;`), the quote and comment introducers (`" ' /`), the `!` of
/// `!important`, the `\` escape introducer (which it declines on), and the comma and ASCII
/// whitespace the value's separator **class** turns on ([`is_value_separator`]). Those last
/// two move no state — the two places their position matters for the *extent* (the value's
/// end, and the roll-back before a `!`) are recovered afterwards by a trim — so they take a
/// short arm of their own rather than the state machine's.
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

/// How many bytes [`skip_run_end`] tests behind one bound check.
const SKIP_CHUNK: usize = 8;

/// The end of the run of [`SKIP`] bytes from `from` — `from` itself when none is one.
///
/// [`SKIP_CHUNK`] bytes at a time while that many are in bounds, each tested against the
/// table in turn with no bound check of its own, and byte by byte within a chunk of the end
/// of input. Every byte is still the table's to classify; only the bound check is shared
/// across the chunk.
#[inline]
fn skip_run_end(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(chunk) = bytes.get(i..i + SKIP_CHUNK) {
        if let Some(k) = chunk.iter().position(|&b| !SKIP[b as usize]) {
            return i + k;
        }
        i += SKIP_CHUNK;
    }
    while i < bytes.len() && SKIP[bytes[i] as usize] {
        i += 1;
    }
    i
}

/// The bytes `scan_value_bytes` has a match arm for. **This must stay in lockstep with that
/// match**: a byte named here but unhandled there is merely slow, but a byte handled there and
/// *missing* here is skipped by the table and its arm goes dead — a silent misparse. (The
/// debug oracle would catch it; this exists so it never gets that far.)
///
/// [`is_value_separator`] is here for the value's **class** rather than for its extent: a
/// comma and an ASCII whitespace move none of this scan's own state, and they are inspected
/// only so the scan can report what a top-level one of each would make of the value (see
/// `scan_value_core`'s separator arm, which is the "match arm" the lockstep rule means for
/// them). The vertical tab stays skipped — it is CSS whitespace to the lexer and value
/// content to the class, and `is_value_separator` is the side that decides here.
const fn value_scan_inspects(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'{' | b'}' | b'[' | b']' | b';' | b'"' | b'\'' | b'/' | b'!' | b'\\'
    ) || is_value_separator(b)
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

/// Where a declaration's head leaves its value: the `:` and whether a comment sat in the
/// property→colon gap, and the value's first byte. These are exactly the three things
/// `parse_declaration`'s token head (`lex_declaration_head`: `advance` past the property,
/// `skip_boundary_whitespace_and_comments`, `expect(:)`, `skip_whitespace`) establishes —
/// the gap's comments are dropped there, so whether there was one is all the gap owes the
/// declaration node. Offsets are raw `source` offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DeclarationHead {
    /// The `:` that ends the property→colon gap.
    pub(super) colon: usize,
    /// Whether a `/* */` comment sits in the property→colon gap.
    pub(super) gap_comment: bool,
    /// The value's first byte: past the `:` and the whitespace after it (a comment there is
    /// value content, so it is not stepped), or end-of-source.
    pub(super) value_start: usize,
}

/// A declaration located and measured from its bytes: its head and its value's facts,
/// together with the value's separator class (see [`scan_value_core`]).
#[derive(Debug, Clone, Copy)]
pub(super) struct ScannedDeclaration {
    pub(super) head: DeclarationHead,
    pub(super) facts: ValueFacts,
    pub(super) class: Option<ValueSeparator>,
}

/// What the fused disambiguation scan found: a nested rule, or a declaration together with
/// the head and value facts `parse_declaration` would otherwise re-derive.
enum Disambiguation {
    /// `identifier … { … }` — a nested rule; no value facts.
    Rule,
    /// `identifier : value ;` — a declaration.
    Declaration(ScannedDeclaration),
}

/// The declaration head from `from` (just past the property identifier), as bytes: to the
/// `:` across ASCII whitespace and comments, then across ASCII whitespace to the value's
/// first byte. `None` declines, on anything the token head reads differently or might
/// reject.
///
/// Its answer is the token head's on every input it accepts, which is what lets
/// `parse_declaration` skip that head outright (and what its debug oracle re-proves): a
/// terminated comment is one comment token, a `:` is always the one-byte colon, and a run of
/// ASCII whitespace ends where the lexer's whitespace token does — that token runs past the
/// run's last ASCII whitespace byte only onto a non-ASCII one (`<NEL>`, the one non-ASCII
/// code point it takes), and this scan declines wherever it stops on one. A non-ASCII byte
/// outside a comment declines wherever it stands; inside a gap comment it is stepped with the
/// comment.
/// In the gap it may be a boundary run (`color <NBSP>: red`), which `read_declaration` ends
/// the property at — a juncture `skip_boundary_whitespace` steps, and this scan does not
/// model. At the value's first byte it may be that `<NEL>`, which the lexer's whitespace
/// token would run on through, putting the value's start past where this scan stopped. An
/// unterminated comment declines too, leaving the lexer its error.
pub(super) fn scan_declaration_head_bytes(bytes: &[u8], from: usize) -> Option<DeclarationHead> {
    // To the `:`. Anything else declines to the token head rather than guess: for a custom
    // property it is the token head's own missing-colon error to raise.
    let (colon, gap_comment) = colon_after_trivia(bytes, from)?;
    head_after_colon(bytes, colon, gap_comment)
}

/// The declaration head's second half: from the `:` at `colon` (the property→colon gap
/// already walked, `gap_comment` its answer) across ASCII whitespace to the value's first
/// byte. `None` where [`scan_declaration_head_bytes`] declines past the colon: a non-ASCII
/// value start.
#[inline]
fn head_after_colon(bytes: &[u8], colon: usize, gap_comment: bool) -> Option<DeclarationHead> {
    let len = bytes.len();
    // Whitespace only after the `:` — a comment here opens the value.
    let mut i = colon + 1;
    while i < len && is_ascii_css_whitespace(bytes[i]) {
        i += 1;
    }
    if bytes.get(i).is_some_and(|b| !b.is_ascii()) {
        return None;
    }
    Some(DeclarationHead {
        colon,
        gap_comment,
        value_start: i,
    })
}

/// Disambiguate `identifier :` between a nested rule and a declaration, and — for a
/// declaration — collect its `ValueFacts` in the same walk.
///
/// Phase one, the declaration head, is the caller's: [`identifier_child_is_rule`] has walked
/// the property→colon gap to learn that a `:` follows the name, and the head that walk ends in
/// is taken on from its `:` ([`head_after_colon`]) rather than walked again. Phase two is the
/// shared value loop with the verdict latch on: it stops at the first paren-depth-0 `{` (a
/// rule) and otherwise runs the value to its terminator, so the one walk answers both
/// questions. `None` declines — exactly as the value byte scan does, for the same reasons —
/// and hands the verdict back to `scan_rule_or_declaration_tokens`.
fn scan_rule_or_declaration_and_value_bytes(
    source: &str,
    head: DeclarationHead,
) -> Option<Disambiguation> {
    match scan_value_core::<true>(source, head.value_start)? {
        ValueScanOutcome::Rule => Some(Disambiguation::Rule),
        ValueScanOutcome::Value(facts, class) => {
            Some(Disambiguation::Declaration(ScannedDeclaration {
                head,
                facts,
                class,
            }))
        }
    }
}

/// Byte scan first, the token walk on decline — and in debug the token walk (and, for a
/// declaration, the value token walk) run behind a successful byte scan and must agree, so
/// the test suite proves the equivalence. A declaration's head and facts are stashed on the
/// parser for `parse_declaration` to take instead of lexing its head and re-scanning its
/// value; a rule (or a decline) clears the stash.
///
/// `from` is just past the identifier, and `colon` / `gap_comment` are where and how the
/// gap walk from there found the `:`. The byte head's second half runs here
/// ([`head_after_colon`]); its decline declines the byte scan outright.
fn scan_rule_or_declaration(
    parser: &CssParser<'_, '_>,
    from: usize,
    colon: usize,
    gap_comment: bool,
) -> Result<bool, ParseError> {
    let source = parser.source();
    let head = head_after_colon(source.as_bytes(), colon, gap_comment);
    match head.and_then(|head| scan_rule_or_declaration_and_value_bytes(source, head)) {
        Some(outcome) => {
            let is_rule = matches!(outcome, Disambiguation::Rule);
            #[cfg(debug_assertions)]
            {
                // An `Err` here fails the assert too, and must: it would mean the byte scan
                // accepted input the lexer rejects.
                let expected = scan_rule_or_declaration_tokens(source, parser.base_offset(), from);
                assert!(
                    expected.as_ref().is_ok_and(|expected| *expected == is_rule),
                    "rule-or-declaration byte scan disagreed with the token walk at {from}: \
                     scan said {is_rule}, walk said {expected:?}"
                );
                if let Disambiguation::Declaration(scanned) = &outcome {
                    let value_start = scanned.head.value_start;
                    let expected_facts =
                        scan_value_tokens(source, parser.base_offset(), value_start);
                    assert!(
                        expected_facts
                            .as_ref()
                            .is_ok_and(|expected| *expected == scanned.facts),
                        "fused value scan disagreed with the token walk at {value_start}: \
                         scan said {:?}, walk said {expected_facts:?}",
                        scanned.facts
                    );
                }
            }
            parser.stash_declaration(match outcome {
                Disambiguation::Declaration(scanned) => Some((from, scanned)),
                Disambiguation::Rule => None,
            });
            Ok(is_rule)
        }
        None => rule_or_declaration_by_tokens(parser, from),
    }
}

/// The rule-or-declaration verdict where the byte scan declined: the token walk, with the
/// stash cleared, so `parse_declaration` lexes its head and scans the value itself.
fn rule_or_declaration_by_tokens(
    parser: &CssParser<'_, '_>,
    from: usize,
) -> Result<bool, ParseError> {
    parser.stash_declaration(None);
    scan_rule_or_declaration_tokens(parser.source(), parser.base_offset(), from)
}

/// The reference: the disambiguation as a token walk, on its own lexer.
///
/// Lexes a **slice** (`source[from..]`) rather than seeking into the whole source: the two
/// are interchangeable for the verdict, since this walk reads only token *kinds*, and for a
/// lexer **error** too — the lexer carries the slice's own offset (`base_offset + from`),
/// so a declaration whose value holds a stray backtick reports the same document position
/// from either shape.
fn scan_rule_or_declaration_tokens(
    source: &str,
    base_offset: usize,
    from: usize,
) -> Result<bool, ParseError> {
    let mut lexer = Lexer::at_offset(&source[from..], base_offset + from);
    // `u32` so an unbalanced close saturates at zero — see `scan_value_tokens`.
    let mut paren: u32 = 0;
    loop {
        let token = lexer.next_token()?;
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

/// Whether the identifier-led block child at the current token — a name that is not a
/// custom property's — opens a nested rule rather than a declaration.
///
/// `color` is a property and `span` a type selector, and whether a `:` follows the name is
/// the whole difference. The property→colon gap is walked once ([`colon_after_trivia`]), and
/// the declaration head goes on from the `:` it finds ([`head_after_colon`]). With a `:` —
/// nearly every block child — the child is still ambiguous (`color: red;` against
/// `span:hover { }`), and [`scan_rule_or_declaration`] settles it by walking on to the first
/// paren-depth-0 `{` (a rule) or `;`/`}` (a declaration).
///
/// Only the `:` is recognized from the bytes: whitespace and comments are trivia and get
/// skipped, and **everything else declines**, including bytes whose token is perfectly
/// obvious. Declining on the negative is deliberate. It costs one short whitespace scan on
/// the rarer nested-rule child, and it buys the property the rest of this module rests on:
/// the scan never has to *reject*, so a lexer error stays
/// [`peek_past_boundary_whitespace`](CssParser::peek_past_boundary_whitespace)'s alone,
/// reported at its own position. Widening the accept set would mean re-deriving which bytes
/// the lexer can error on — the same bet `scan_value_bytes` declines to make.
///
/// The whitespace the walk skips is ASCII, and that is a second decline, not a class: a
/// boundary run (`a { color <NBSP>: red }`) is a non-ASCII byte to it, so it declines to the
/// token lookahead, which steps the run — `read_declaration` ends the property at JS `\s`
/// and `allow_whitespace()`s to the colon, so the gap is a juncture like any other. The
/// lookahead must be the boundary-aware one because the skip it predicts is: the declaration's
/// token head (`lex_declaration_head`) steps such a gap with
/// `skip_boundary_whitespace_and_comments`, and a narrower lookahead would read the run as the
/// identifier where the `:` is due, classify the child a nested rule, and reject the document.
/// A `:` found past a gap the bytes declined is a head the bytes cannot locate either, so the
/// scan behind it runs on the token walk.
///
/// In debug the token lookahead runs behind a `:` the bytes found and must agree, so the
/// test suite proves the equivalence.
pub(super) fn identifier_child_is_rule(parser: &CssParser<'_, '_>) -> Result<bool, ParseError> {
    let bytes = parser.source().as_bytes();
    let from = parser.current_end();
    match colon_after_trivia(bytes, from) {
        Some((colon, gap_comment)) => {
            #[cfg(debug_assertions)]
            {
                // An `Err` here fails the assert too, and must: it would mean the byte walk
                // accepted a lookahead the lexer rejects.
                let expected = parser.peek_past_boundary_whitespace();
                assert!(
                    expected
                        .as_ref()
                        .is_ok_and(|expected| *expected == TokenKind::Colon),
                    "property→colon byte walk disagreed with the token lookahead at {from}: \
                     walk found a `:`, lookahead said {expected:?}"
                );
            }
            scan_rule_or_declaration(parser, from, colon, gap_comment)
        }
        None => match parser.peek_past_boundary_whitespace()? {
            // The gap walk declined, so the byte head — which starts with the same walk —
            // would too.
            TokenKind::Colon => rule_or_declaration_by_tokens(parser, from),
            // `{` (`div {`), a selector token (`a.b`, `a#b`, `a[b]`), or anything else: a
            // nested rule
            _ => Ok(true),
        },
    }
}

/// The `:` that is the first significant byte at or past `from` — ASCII whitespace and
/// comments are trivia — as its offset and whether a comment stood in the gap. `None` at any
/// other byte, a non-ASCII one included, and at an unterminated comment. The gap walk behind
/// [`scan_declaration_head_bytes`] and [`identifier_child_is_rule`].
#[inline]
fn colon_after_trivia(bytes: &[u8], from: usize) -> Option<(usize, bool)> {
    let len = bytes.len();
    let mut i = from;
    let mut gap_comment = false;
    loop {
        while i < len && is_ascii_css_whitespace(bytes[i]) {
            i += 1;
        }
        match bytes.get(i)? {
            b':' => return Some((i, gap_comment)),
            b'/' if is_comment_start(bytes, i) => {
                gap_comment = true;
                i = comment_end_checked(bytes, i)?;
            }
            _ => return None,
        }
    }
}

/// Scan a declaration's value from `value_start` (the start of its first token — the
/// caller has already consumed the `:` and the whitespace after it).
///
/// Byte scan first, reference token walk on decline. In debug builds the reference *also*
/// runs behind a successful byte scan and must agree fact for fact, so every value in
/// every fixture and every unit test re-proves the equivalence.
///
/// The second element is the value's separator **class**, which is not one of the facts and
/// has a different oracle: the reference walk never classifies, so a declined value gets
/// `None` and the value parser derives its own as it always did. See `scan_value_core`.
pub(super) fn scan_value(
    parser: &CssParser<'_, '_>,
    value_start: usize,
) -> Result<(ValueFacts, Option<ValueSeparator>), ParseError> {
    let source = parser.source();
    match scan_value_bytes(source, value_start) {
        Some((facts, class)) => {
            #[cfg(debug_assertions)]
            {
                // An `Err` here fails the assert too, and must: it would mean the byte scan
                // accepted a value the lexer rejects, silently dropping a parse error.
                let expected = scan_value_tokens(source, parser.base_offset(), value_start);
                assert!(
                    expected.as_ref().is_ok_and(|expected| *expected == facts),
                    "declaration value byte scan disagreed with the token walk at \
                     {value_start}: scan said {facts:?}, walk said {expected:?}"
                );
            }
            Ok((facts, class))
        }
        // The token walk answers for the value's extent only — it never classifies — so a
        // declined value simply reaches the value parser's own fused pass, as it always did.
        None => scan_value_tokens(source, parser.base_offset(), value_start).map(|f| (f, None)),
    }
}

/// The fast path. `None` = "I decline" — hand the value to `scan_value_tokens`.
fn scan_value_bytes(
    source: &str,
    value_start: usize,
) -> Option<(ValueFacts, Option<ValueSeparator>)> {
    // `WANT_VERDICT == false` compiles out the verdict latch, the only path that yields
    // `Rule`, so only `Value` can arrive; a `Rule` (which cannot) safely declines.
    match scan_value_core::<false>(source, value_start) {
        Some(ValueScanOutcome::Value(facts, class)) => Some((facts, class)),
        _ => None,
    }
}

/// What [`scan_value_core`] found: a declaration value's facts, or — only when the verdict
/// latch is enabled — that the run is a nested rule.
#[cfg_attr(test, derive(Debug, PartialEq))]
enum ValueScanOutcome {
    /// A paren-depth-0 `{` arrived before any paren-depth-0 `;`/`}` — a nested rule. Only
    /// produced when `WANT_VERDICT` is set (the disambiguation caller); the plain value scan
    /// never asks the question and never sees it.
    Rule,
    /// The value ran to its terminator; here are its facts, and what
    /// [`ValueParser::fast_scan`](crate::parser::value::parser::ValueParser) would make of
    /// the value's own text (`None` when this scan cannot say — see `scan_value_core`).
    Value(ValueFacts, Option<ValueSeparator>),
}

/// The value byte-scan loop, from `value_start` (the value's first byte). Shared by the
/// plain value scan (`WANT_VERDICT == false`) and the rule/declaration disambiguation
/// (`WANT_VERDICT == true`).
///
/// With the latch on, the first paren-depth-0 `{` returns `Rule` and the first paren-depth-0
/// `;`/`}` fixes the verdict as a declaration — the paren-only model of
/// `scan_rule_or_declaration_tokens` — while the loop keeps tracking `[]`/`{}` for the *value*
/// terminator (which a `;`/`}` inside them does not end). The two models share the one `paren`
/// counter and it evolves identically in each, so the fused verdict is exactly what a standalone
/// paren-only walk would return; the facts are exactly what the plain scan returns. `None`
/// declines, for the reasons in the module docs.
fn scan_value_core<const WANT_VERDICT: bool>(
    source: &str,
    value_start: usize,
) -> Option<ValueScanOutcome> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = value_start;

    // The commonest value is one inert run straight to its terminator (`red;`, `0}`,
    // `1.5rem;` — three in five on real stylesheets). Nothing in it is a byte the state
    // machine below inspects, so every fact is settled where the run stops: no comment, no
    // `!important`, no separator, and — the run's last byte being no whitespace (the vertical
    // tab is the one skipped byte that is) — a value that ends there and is not empty. The run
    // is the loop's own first skip, so any other value carries on from where it stopped with
    // no state to recover: a skipped byte moves none.
    i = skip_run_end(bytes, i);
    if i > value_start && !is_ascii_css_whitespace(bytes[i - 1]) {
        let terminator_kind = match bytes.get(i) {
            None => Some(TerminatorKind::Eof),
            Some(b';') => Some(TerminatorKind::Semicolon),
            Some(b'}') => Some(TerminatorKind::RightBrace),
            Some(_) => None,
        };
        if let Some(terminator_kind) = terminator_kind {
            return Some(ValueScanOutcome::Value(
                ValueFacts {
                    terminator: i,
                    terminator_kind,
                    value_end: i,
                    important_end: None,
                    has_comment: false,
                    is_empty: false,
                    group_closes_value: false,
                },
                Some(ValueSeparator::None),
            ));
        }
    }

    // `u32` so an unbalanced close saturates at zero rather than going negative and
    // disabling the depth-zero terminator tests — the same rule, for the same reason, as
    // the token walk it replaces.
    let mut paren: u32 = 0;
    let mut brace: u32 = 0;
    let mut bracket: u32 = 0;
    let mut has_comment = false;
    // The value's separator CLASS, in the two positions it is decided from: the first
    // paren-depth-0 comma and the first paren-depth-0 ASCII whitespace, as
    // `ValueParser::fast_scan` would see them. `usize::MAX` is "none seen" — the walk runs
    // past the value proper (over the trailing whitespace and any `!important`, neither of
    // which is the value's own), so both are filtered against `value_end` below, and the
    // FIRST is the one that survives that filter whenever any does.
    let mut top_comma = usize::MAX;
    let mut top_ws = usize::MAX;
    // A construct whose interior this scan steps over WHOLE but `fast_scan` walks byte by
    // byte, so the two cannot be shown to agree: the class is not stated for it. Today that
    // is the url-token alone (a string is opaque to both, and a comment is `has_comment`).
    let mut class_unstated = false;
    // The *last* `!` at content position, at any depth. `!important` must be the value's
    // final two tokens, so only the last `!` can possibly open it; a `!` nested in parens
    // (or followed by anything but `important`) is rejected by the forward check below.
    let mut last_bang: Option<usize> = None;
    // Whether the paren-only verdict has settled on "declaration" (a `;`/`}` at paren depth
    // zero). Latched once, then a later paren-depth-0 `{` must not be misread as a rule.
    let mut verdict_is_decl = false;
    // The `)` that first brings the paren depth back to zero — the close of the value's
    // first parenthesized group. `usize::MAX` is "none yet".
    let mut first_group_close = usize::MAX;

    let (terminator, terminator_kind) = loop {
        // Most of these runs are empty — the loop comes back here after every inspected
        // byte, and inspected bytes come in clusters (`), `, `(--`) — so the first byte is
        // tested alone, ahead of the chunked walk's bound check.
        if i < len && SKIP[bytes[i] as usize] {
            i = skip_run_end(bytes, i + 1);
        }
        if i >= len {
            break (len, TerminatorKind::Eof);
        }
        let b = bytes[i];
        // A separator moves none of this scan's state — it is inspected only for the value's
        // class — so it takes its own short arm ahead of the depth computation, the verdict
        // latch and the match, none of which it could reach anyway. `paren == 0` (not
        // `at_top`) is deliberately `fast_scan`'s notion of top level: that scanner treats
        // `[]` and `{}` as ordinary content and nests on parens alone.
        if is_value_separator(b) {
            if paren == 0 {
                if b == b',' {
                    top_comma = top_comma.min(i);
                } else {
                    top_ws = top_ws.min(i);
                }
            }
            i += 1;
            continue;
        }
        // One dispatch on the byte. The verdict latch (the paren-only model of
        // `scan_rule_or_declaration_tokens`: the first paren-depth-0 structural byte decides
        // rule vs declaration) and the all-depths-zero terminator test are asked inside the
        // three arms that can spend them — `;`, `}` and `{` — rather than ahead of the
        // dispatch for every inspected byte, most of which are parens and quotes.
        match b {
            b';' | b'}' => {
                // A `;`/`}` at paren depth zero fixes a declaration, and the loop reads on
                // for the value terminator (`[]`/`{}` may push it further).
                if WANT_VERDICT && paren == 0 {
                    verdict_is_decl = true;
                }
                if paren == 0 && brace == 0 && bracket == 0 {
                    let kind = if b == b';' {
                        TerminatorKind::Semicolon
                    } else {
                        TerminatorKind::RightBrace
                    };
                    break (i, kind);
                }
                if b == b'}' {
                    brace = brace.saturating_sub(1);
                }
                i += 1;
            }
            b'{' => {
                // A `{` at paren depth zero ahead of any such `;`/`}` is a rule.
                if WANT_VERDICT && !verdict_is_decl && paren == 0 {
                    return Some(ValueScanOutcome::Rule);
                }
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
                if paren == 1 {
                    first_group_close = first_group_close.min(i);
                }
                paren = paren.saturating_sub(1);
                i += 1;
            }
            // A `url(…)` is ONE opaque token (css-syntax §4.3.6), so its parens are content,
            // not nesting — an interior `;` or `)` must not be seen. Any other `(` nests.
            b'(' => match paren_open_kind(source, bytes, value_start, i) {
                ParenOpen::UrlToken { end } => {
                    // Opaque here, walked there: a url-token's interior can hold a `(`, a
                    // quote or a `/*`, each of which moves `fast_scan`'s state and none of
                    // which this scan sees. Rare enough (0.3% of real declaration values)
                    // that declining the class outright beats modelling the interior.
                    class_unstated = true;
                    i = end;
                }
                ParenOpen::Nesting => {
                    paren += 1;
                    i += 1;
                }
                ParenOpen::UnterminatedUrl => return None,
            },
            // A string the lexer would reject (unterminated / trailing `\`) declines.
            b'"' | b'\'' => i = string_end(bytes, i).ok()?,
            b'/' if is_comment_start(bytes, i) => {
                has_comment = true;
                i = comment_end_checked(bytes, i)?;
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
        let name_start = crate::comments::skip_trivia_forward(bytes, bang + 1, terminator);
        let mut name_end = name_start;
        while name_end < terminator && IDENT_CONTINUE_LUT[bytes[name_end] as usize] {
            name_end += 1;
        }
        if !source[name_start..name_end].eq_ignore_ascii_case("important") {
            return None;
        }
        if crate::comments::skip_trivia_forward(bytes, name_end, terminator) < terminator {
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
    let is_empty = crate::comments::skip_trivia_forward(bytes, value_start, span_end) >= span_end;

    // The class the value parser would have derived for itself, over `[value_start,
    // value_end)` — the text `parse_value_from_source` hands its `ValueParser`. A separator
    // at or past `value_end` sits in the trailing whitespace or the `!important` and is not
    // the value's own (`color: red !important` is a leaf, not a whitespace list). A comment
    // is unstated because `fast_scan` refuses a commented value outright and defers to the
    // comment-aware two-pass path, which classifies differently.
    let class = if has_comment || class_unstated {
        None
    } else if top_comma < value_end {
        Some(ValueSeparator::Comma)
    } else if top_ws < value_end {
        Some(ValueSeparator::Whitespace)
    } else {
        Some(ValueSeparator::None)
    };

    Some(ValueScanOutcome::Value(
        ValueFacts {
            terminator,
            terminator_kind,
            value_end,
            important_end,
            has_comment,
            is_empty,
            // `first_group_close` is `usize::MAX` for no group, which no `value_end` reaches.
            group_closes_value: first_group_close.wrapping_add(1) == value_end,
        },
        class,
    ))
}

/// What a content-position `(` opens, as the value byte scan must treat it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParenOpen {
    /// An ordinary nesting paren: a function-token's `(`, or a bare one.
    Nesting,
    /// An opaque url-token (css-syntax-3 §4.3.6) that closes; `end` is one past its
    /// matching unescaped `)`.
    UrlToken { end: usize },
    /// A url-token that nothing closes. The scan declines on it — see
    /// [`paren_open_kind`].
    UnterminatedUrl,
}

/// What the `(` at `open` opens: a url-token when it closes a `url` **identifier token**
/// with an unquoted argument, and an ordinary nesting paren otherwise.
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
///
/// A url-token with no matching unescaped `)` is [`ParenOpen::UnterminatedUrl`], and the
/// caller must decline on it rather than read it as either of the other two. The lexer
/// models no `<bad-url-token>`: whatever css-syntax-3 would make of the interior, the lexer
/// takes the token to end-of-source, so it can *end in whitespace* — and `value_end`'s
/// trailing-whitespace trim assumes the value's last token does not, which is why the trim
/// is exact everywhere else. Read as a nesting paren instead, the url's interior is
/// scanned as content and the trim stops short of the token's real end (`url( ⏎` at
/// end-of-source: the scan's `value_end` sits after the `(`, the walk's after the
/// newline). A plain `url(` at end-of-source reaches it, and so does any unclosed `url(`
/// in a Svelte `<style>` whose `</style>` sits on its own line, since the island then ends
/// in that newline. The parse fails regardless (the url swallows the block's `}`), so
/// declining costs nothing and leaves the error to the reference walk.
fn paren_open_kind(source: &str, bytes: &[u8], value_start: usize, open: usize) -> ParenOpen {
    if open < 3 || open - 3 < value_start {
        return ParenOpen::Nesting;
    }
    if !bytes[open - 3..open].eq_ignore_ascii_case(b"url") {
        return ParenOpen::Nesting;
    }
    // `open - 3 == value_start` needs no check: the byte before the value's first token is
    // the `:` or the whitespace after it, neither of which continues an identifier.
    if open - 3 > value_start {
        let prev = bytes[open - 4];
        if IDENT_CONTINUE_LUT[prev as usize] || prev == b'$' {
            return ParenOpen::Nesting;
        }
    }
    // A quoted argument makes it a function-token (`url("…")` lexes as ident + `(` +
    // string), not a url-token — the lexer's own fork, shared.
    if url_arg_is_quoted(source, open + 1) {
        return ParenOpen::Nesting;
    }
    // Opaque to the matching unescaped `)`, via the lexer's own scan.
    match url_token_close(bytes, open + 1) {
        Some(end) => ParenOpen::UrlToken { end },
        None => ParenOpen::UnterminatedUrl,
    }
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
/// byte scan declines still takes — so a lexer error, and its exact position, is raised
/// here rather than by the byte scan.
///
/// Seeks into the **whole source**, unlike [`scan_rule_or_declaration_tokens`], which lexes
/// a slice. The asymmetry is shape only: this walk stands in for the *parser* driving
/// its own lexer, the rule walk for a temp lexer over a slice. Both report a lexer
/// error at the same document position, since each lexer carries the offset of what it
/// scans.
fn scan_value_tokens(
    source: &str,
    base_offset: usize,
    value_start: usize,
) -> Result<ValueFacts, ParseError> {
    let mut lexer = Lexer::at_offset(source, base_offset);
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
    // The `)` token that first brings the paren depth back to zero (`usize::MAX`: none).
    let mut first_group_close = usize::MAX;

    // The terminator token's own start — where the parser re-seats its lexer. At EOF the
    // token is zero-width at end-of-source, so the same field serves both exits.
    let (terminator, terminator_kind) = loop {
        let token = lexer.next_token()?;
        let decoded = lexer.decoded_str();
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
            TokenKind::RightParen => {
                if paren == 1 {
                    first_group_close = first_group_close.min(token.start as usize);
                }
                paren = paren.saturating_sub(1);
            }
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
                let text = decoded.unwrap_or_else(|| &source[start..end]);
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
        group_closes_value: first_group_close.wrapping_add(1) == value_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`skip_run_end`] walks a chunk at a time and then a byte at a time; either way it must
    /// stop where a byte-at-a-time walk of [`SKIP`] does. Runs of every length up to three
    /// chunks, each skipped byte class at one position per length, from offsets 0 and 3,
    /// stopped by every byte (and by end of input) and followed by input of every length.
    #[test]
    fn skip_runs_end_where_the_table_ends() {
        let skipped: Vec<u8> = (0..=255u8).filter(|&b| SKIP[usize::from(b)]).collect();
        let reference = |bytes: &[u8], mut i: usize| {
            while i < bytes.len() && SKIP[usize::from(bytes[i])] {
                i += 1;
            }
            i
        };
        let mut graded = 0;
        for len in 0..=3 * SKIP_CHUNK {
            for (k, &c) in skipped.iter().enumerate() {
                let mut run = vec![b'a'; len];
                if len > 0 {
                    run[k % len] = c;
                }
                for stop in (0..=255u8).map(Some).chain([None]) {
                    for tail in [0, 1, SKIP_CHUNK - 1, SKIP_CHUNK] {
                        for from in [0, 3] {
                            let mut bytes = vec![b';'; from];
                            bytes.extend_from_slice(&run);
                            if let Some(stop) = stop {
                                bytes.push(stop);
                                bytes.extend(core::iter::repeat_n(b'a', tail));
                            }
                            assert_eq!(
                                skip_run_end(&bytes, from),
                                reference(&bytes, from),
                                "{bytes:?} from {from}"
                            );
                            graded += 1;
                        }
                    }
                }
            }
        }
        assert!(graded > 100_000, "{graded}");
    }

    /// [`scan_value_core`] with the verdict latch and the all-depths-zero test computed
    /// ahead of the dispatch for every inspected byte, and `;` / `}` each split across a
    /// guarded and an unguarded arm — the spelling the single dispatch is graded against.
    /// Everything outside those arms is the scan's own text.
    fn scan_value_core_model<const WANT_VERDICT: bool>(
        source: &str,
        value_start: usize,
    ) -> Option<ValueScanOutcome> {
        let bytes = source.as_bytes();
        let len = bytes.len();
        let mut i = value_start;

        i = skip_run_end(bytes, i);
        if i > value_start && !is_ascii_css_whitespace(bytes[i - 1]) {
            let terminator_kind = match bytes.get(i) {
                None => Some(TerminatorKind::Eof),
                Some(b';') => Some(TerminatorKind::Semicolon),
                Some(b'}') => Some(TerminatorKind::RightBrace),
                Some(_) => None,
            };
            if let Some(terminator_kind) = terminator_kind {
                return Some(ValueScanOutcome::Value(
                    ValueFacts {
                        terminator: i,
                        terminator_kind,
                        value_end: i,
                        important_end: None,
                        has_comment: false,
                        is_empty: false,
                        group_closes_value: false,
                    },
                    Some(ValueSeparator::None),
                ));
            }
        }

        let mut paren: u32 = 0;
        let mut brace: u32 = 0;
        let mut bracket: u32 = 0;
        let mut has_comment = false;
        let mut top_comma = usize::MAX;
        let mut top_ws = usize::MAX;
        let mut class_unstated = false;
        let mut last_bang: Option<usize> = None;
        let mut verdict_is_decl = false;
        let mut first_group_close = usize::MAX;

        let (terminator, terminator_kind) = loop {
            if i < len && SKIP[bytes[i] as usize] {
                i = skip_run_end(bytes, i + 1);
            }
            if i >= len {
                break (len, TerminatorKind::Eof);
            }
            let b = bytes[i];
            if is_value_separator(b) {
                if paren == 0 {
                    if b == b',' {
                        top_comma = top_comma.min(i);
                    } else {
                        top_ws = top_ws.min(i);
                    }
                }
                i += 1;
                continue;
            }
            // Where the model differs from the live scan: the depth test and the verdict
            // latch run ahead of the dispatch, for every inspected byte, and `;` / `}`
            // each take a guarded and an unguarded arm below.
            let at_top = paren == 0 && brace == 0 && bracket == 0;
            if WANT_VERDICT && !verdict_is_decl && paren == 0 {
                match b {
                    b'{' => return Some(ValueScanOutcome::Rule),
                    b';' | b'}' => verdict_is_decl = true,
                    _ => {}
                }
            }
            match b {
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
                    if paren == 1 {
                        first_group_close = first_group_close.min(i);
                    }
                    paren = paren.saturating_sub(1);
                    i += 1;
                }
                b'(' => match paren_open_kind(source, bytes, value_start, i) {
                    ParenOpen::UrlToken { end } => {
                        class_unstated = true;
                        i = end;
                    }
                    ParenOpen::Nesting => {
                        paren += 1;
                        i += 1;
                    }
                    ParenOpen::UnterminatedUrl => return None,
                },
                b'"' | b'\'' => i = string_end(bytes, i).ok()?,
                b'/' if is_comment_start(bytes, i) => {
                    has_comment = true;
                    i = comment_end_checked(bytes, i)?;
                }
                b'/' => i += 1,
                b'!' => {
                    last_bang = Some(i);
                    i += 1;
                }
                _ => return None,
            }
        };

        let important = last_bang.and_then(|bang| {
            let name_start = crate::comments::skip_trivia_forward(bytes, bang + 1, terminator);
            let mut name_end = name_start;
            while name_end < terminator && IDENT_CONTINUE_LUT[bytes[name_end] as usize] {
                name_end += 1;
            }
            if !source[name_start..name_end].eq_ignore_ascii_case("important") {
                return None;
            }
            if crate::comments::skip_trivia_forward(bytes, name_end, terminator) < terminator {
                return None;
            }
            Some((bang, name_end))
        });

        let (span_end, important_end) = match important {
            Some((bang, name_end)) => (bang, Some(name_end)),
            None => (terminator, None),
        };
        let value_end = trim_end(bytes, value_start, span_end);

        let is_empty =
            crate::comments::skip_trivia_forward(bytes, value_start, span_end) >= span_end;

        let class = if has_comment || class_unstated {
            None
        } else if top_comma < value_end {
            Some(ValueSeparator::Comma)
        } else if top_ws < value_end {
            Some(ValueSeparator::Whitespace)
        } else {
            Some(ValueSeparator::None)
        };

        Some(ValueScanOutcome::Value(
            ValueFacts {
                terminator,
                terminator_kind,
                value_end,
                important_end,
                has_comment,
                is_empty,
                group_closes_value: first_group_close.wrapping_add(1) == value_end,
            },
            class,
        ))
    }

    /// The single-dispatch scan against [`scan_value_core_model`], with the verdict latch
    /// off and on, over an alphabet of everything either spelling acts on: all three
    /// nesting pairs, both terminators, both separators, both quotes, the comment
    /// delimiters, the escape introducer, `!important`, a url-token opener, a custom
    /// property name, and a line break. Every string of up to four members, then a seeded
    /// sample of longer ones — long enough to nest, close out of order, leave a construct
    /// open at end of input, and put a terminator inside each of them.
    #[test]
    fn single_dispatch_matches_the_split_arm_scan() {
        const ALPHABET: [&str; 22] = [
            "(",
            ")",
            "[",
            "]",
            "{",
            "}",
            ";",
            ",",
            " ",
            "\"",
            "'",
            "/",
            "*",
            "\\",
            "!",
            "a",
            "5",
            ":",
            "\n",
            "url(",
            "important",
            "--x",
        ];
        fn grade(source: &str) {
            assert_eq!(
                scan_value_core::<false>(source, 0),
                scan_value_core_model::<false>(source, 0),
                "plain scan, {source:?}"
            );
            assert_eq!(
                scan_value_core::<true>(source, 0),
                scan_value_core_model::<true>(source, 0),
                "verdict scan, {source:?}"
            );
        }

        let mut source = String::new();
        let mut graded = 0u32;
        for len in 0..=4u32 {
            for case in 0..ALPHABET.len().pow(len) {
                source.clear();
                let mut rest = case;
                for _ in 0..len {
                    source.push_str(ALPHABET[rest % ALPHABET.len()]);
                    rest /= ALPHABET.len();
                }
                grade(&source);
                graded += 1;
            }
        }

        // xorshift64, fixed seed: the same sample every run.
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..200_000 {
            source.clear();
            let len = 5 + next() % 12;
            for _ in 0..len {
                source.push_str(ALPHABET[(next() % ALPHABET.len() as u64) as usize]);
            }
            grade(&source);
            graded += 1;
        }
        assert!(graded > 400_000, "{graded}");
    }

    /// [`ValueFacts::group_closes_value`] against the scan it stands in for, in both
    /// directions, over every value of up to four members of an alphabet holding what
    /// either scan acts on (and a seeded sample of longer ones): wherever the boundary
    /// scan states a leaf class and the value parser would ask a plain-name function's
    /// paren scan, the fact is `true` exactly when `matching_close_paren` closes the
    /// group on the value's last byte. Each value is then parsed whole, which runs the
    /// debug oracles on the path that spends the fact.
    #[test]
    fn group_close_agrees_with_the_function_paren_scan() {
        use crate::parser::value::plain_function_paren;
        use crate::parser::value::scan::matching_close_paren;

        const ALPHABET: [&str; 21] = [
            "(",
            ")",
            "[",
            "]",
            "{",
            "}",
            ";",
            ",",
            " ",
            "\"",
            "'",
            "/",
            "*",
            "\\",
            "!",
            "a",
            "-",
            "5",
            "url(",
            "f(",
            "important",
        ];
        const HEAD: &str = "a{b:";
        let mut arena = bumpalo::Bump::new();
        let mut spent = 0u32;
        let mut grade = |value: &str| {
            let source = format!("{HEAD}{value}");
            if let Some((facts, Some(ValueSeparator::None))) = scan_value_bytes(&source, HEAD.len())
            {
                let text = &source[HEAD.len()..facts.value_end];
                if let Some(open) = plain_function_paren(text.as_bytes()) {
                    let closes = matching_close_paren(text, open) == Some(text.len() - 1);
                    assert_eq!(facts.group_closes_value, closes, "for {source:?}");
                    spent += u32::from(closes);
                }
            }
            arena.reset();
            // An `Err` is a rejected stylesheet, which is most of these; the parse is run
            // for the assertions inside it.
            let _ = crate::parse(&source, &arena);
        };

        let mut value = String::new();
        for len in 0..=4u32 {
            for case in 0..ALPHABET.len().pow(len) {
                value.clear();
                let mut rest = case;
                for _ in 0..len {
                    value.push_str(ALPHABET[rest % ALPHABET.len()]);
                    rest /= ALPHABET.len();
                }
                grade(&value);
            }
        }
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..100_000 {
            value.clear();
            let len = 5 + next() % 8;
            for _ in 0..len {
                value.push_str(ALPHABET[(next() % ALPHABET.len() as u64) as usize]);
            }
            grade(&value);
        }
        assert!(spent > 500, "{spent}");
    }

    /// The value's separator class, for the declaration `source` holds.
    ///
    /// `Err(())` is the scan declining outright (an escape, a non-ASCII byte at content
    /// position, a bad string) — a different answer from `Ok(None)`, which is the scan
    /// running to the end and refusing to state the class.
    fn class_of(source: &str) -> Result<Option<ValueSeparator>, ()> {
        let bytes = source.as_bytes();
        let colon = source.find(':').expect("test source needs a `:`");
        let mut value_start = colon + 1;
        while is_ascii_css_whitespace(bytes[value_start]) {
            value_start += 1;
        }
        scan_value_bytes(source, value_start)
            .map(|(_, class)| class)
            .ok_or(())
    }

    /// The class is a claim about the value's OWN text — `[value_start, value_end)` — while
    /// the scan runs on past it, over the trailing whitespace and any `!important`. A
    /// separator out there belongs to neither, so these are the cases the `< value_end`
    /// filter exists for: `red !important` is a leaf, not a two-element whitespace list.
    #[test]
    fn separators_past_the_value_are_not_the_values_own() {
        assert_eq!(
            class_of("a{color: red !important;}"),
            Ok(Some(ValueSeparator::None))
        );
        assert_eq!(class_of("a{color: red ;}"), Ok(Some(ValueSeparator::None)));
        assert_eq!(class_of("a{color: red}"), Ok(Some(ValueSeparator::None)));
        assert_eq!(
            class_of("a{margin: 0 auto !important;}"),
            Ok(Some(ValueSeparator::Whitespace))
        );
        assert_eq!(
            class_of("a{font-family: a, b !important;}"),
            Ok(Some(ValueSeparator::Comma))
        );
    }

    /// "Top level" is the value parser's, which nests on parens and quotes ALONE — `[]` and
    /// `{}` are ordinary content there, even though this scan tracks them for the value's
    /// terminator.
    #[test]
    fn top_level_is_parens_and_quotes_only() {
        assert_eq!(
            class_of("a{width: calc(1px + 2px);}"),
            Ok(Some(ValueSeparator::None))
        );
        assert_eq!(
            class_of("a{transform: translate(1px, 2px);}"),
            Ok(Some(ValueSeparator::None))
        );
        assert_eq!(
            class_of("a{content: 'a b';}"),
            Ok(Some(ValueSeparator::None))
        );
        assert_eq!(
            class_of("a{content: 'a,b';}"),
            Ok(Some(ValueSeparator::None))
        );
        assert_eq!(
            class_of("a{--x: [a b];}"),
            Ok(Some(ValueSeparator::Whitespace))
        );
        assert_eq!(
            class_of("a{grid-template: 'a a' 1fr;}"),
            Ok(Some(ValueSeparator::Whitespace))
        );
        // A comma anywhere at top level wins over whitespace, wherever each sits.
        assert_eq!(
            class_of("a{font: a b, c;}"),
            Ok(Some(ValueSeparator::Comma))
        );
    }

    /// The two constructs whose interior this scan steps over WHOLE but the value parser
    /// walks byte by byte: the class is withheld rather than guessed.
    #[test]
    fn opaque_interiors_withhold_the_class() {
        assert_eq!(class_of("a{background: url(a.png);}"), Ok(None));
        assert_eq!(class_of("a{color: /* c */ red;}"), Ok(None));
        assert_eq!(class_of("a{color: red /* c */;}"), Ok(None));
        // A QUOTED url argument is a function token, not a url-token — nothing is hidden.
        assert_eq!(
            class_of("a{background: url('a.png') no-repeat;}"),
            Ok(Some(ValueSeparator::Whitespace))
        );
    }

    /// A declining scan states nothing at all, which is not the same as stating "no
    /// separator" — the value parser must fall back to its own pass, not to a leaf.
    #[test]
    fn a_declining_scan_is_not_a_leaf_verdict() {
        assert_eq!(class_of("a{content: a\\ b;}"), Err(()));
        assert_eq!(class_of("a{font-family: \u{e9}x;}"), Err(()));
    }

    /// A value that is one inert run to its terminator is settled where the run stops, and
    /// must read exactly as the token walk does — including the run that ends in a vertical
    /// tab, the one byte the run skips that is whitespace, whose value ends before it.
    #[test]
    fn a_single_run_value_agrees_with_the_token_walk() {
        for source in [
            "a{color:red;}",
            "a{color:red}",
            "a{color:red",
            "a{width:1.5rem;}",
            "a{--x:a\u{b};}",
            "a{--x:a\u{b}}",
        ] {
            let value_start = source.find(':').expect("test source needs a `:`") + 1;
            let scanned = scan_value_bytes(source, value_start).map(|(facts, _)| facts);
            let walked = scan_value_tokens(source, 0, value_start).ok();
            assert_eq!(scanned, walked, "{source:?}");
        }
        assert_eq!(class_of("a{color:red;}"), Ok(Some(ValueSeparator::None)));
        assert_eq!(class_of("a{--x:a\u{b};}"), Ok(Some(ValueSeparator::None)));
    }

    /// The byte head of the declaration whose property is `property`, as
    /// `(colon, gap_comment, value_start)`.
    fn head_of(source: &str, property: &str) -> Option<(usize, bool, usize)> {
        let from = source
            .find(property)
            .expect("test source holds the property")
            + property.len();
        scan_declaration_head_bytes(source.as_bytes(), from)
            .map(|head| (head.colon, head.gap_comment, head.value_start))
    }

    /// The head the token walk would reach: the `:` across whitespace and comments, then the
    /// value's first byte across whitespace alone — a comment after the `:` is value content.
    #[test]
    fn declaration_head_locates_the_colon_and_the_value() {
        assert_eq!(head_of("a{color: red}", "color"), Some((7, false, 9)));
        assert_eq!(head_of("a{color:red}", "color"), Some((7, false, 8)));
        assert_eq!(head_of("a{color\t:\n red}", "color"), Some((8, false, 11)));
        assert_eq!(head_of("a{--x: 1}", "--x"), Some((5, false, 7)));
        // A value that is only a comment, or nothing at all, starts where the token walk's
        // does: on the comment, the terminator, or end-of-source.
        assert_eq!(head_of("a{color:/* c */red}", "color"), Some((7, false, 8)));
        assert_eq!(head_of("a{color: ;}", "color"), Some((7, false, 9)));
        assert_eq!(head_of("a{--x:", "--x"), Some((5, false, 6)));
    }

    /// A comment in the property→colon gap is dropped by the token head, which records only
    /// that there was one — so that is all the byte head reports too, however it is spaced.
    #[test]
    fn declaration_head_reports_a_gap_comment() {
        assert_eq!(
            head_of("a{color/* c */: red}", "color"),
            Some((14, true, 16))
        );
        assert_eq!(
            head_of("a{color /* c */ : red}", "color"),
            Some((16, true, 18))
        );
        assert_eq!(
            head_of("a{color /* a */ /* b */: red}", "color"),
            Some((23, true, 25))
        );
        assert_eq!(head_of("a{--x/* c */: 1}", "--x"), Some((12, true, 14)));
    }

    /// Anything the token head reads differently, or might reject, declines: a boundary run
    /// in the gap (a juncture `skip_boundary_whitespace` steps), a `<NEL>` where the value
    /// starts (the lexer's whitespace token runs on through it), an unterminated comment (the
    /// lexer's error), and a gap that holds anything but trivia. So, conservatively, does any
    /// other non-ASCII byte where the value starts, although the token head would start the
    /// value on it too: the scan does not tell the C1 whitespace from the rest.
    #[test]
    fn declaration_head_declines_what_the_token_head_reads_differently() {
        assert_eq!(head_of("a{color \u{a0}: red}", "color"), None);
        assert_eq!(head_of("a{color /* c */\u{a0}: red}", "color"), None);
        assert_eq!(head_of("a{color: \u{85}red}", "color"), None);
        assert_eq!(head_of("a{--x:\u{85}1}", "--x"), None);
        assert_eq!(head_of("a{color /* c : red}", "color"), None);
        assert_eq!(head_of("a{color red}", "color"), None);
        // conservative: the token head starts this value on the `é` too
        assert_eq!(head_of("a{content: \u{e9}}", "content"), None);
    }

    /// The block-child disambiguation over `source`, whose first token is the identifier:
    /// the verdict (`true` = a nested rule), and whether it left a declaration stashed for
    /// that identifier — which it does exactly where the byte walks located the declaration.
    fn identifier_child(source: &str) -> (bool, bool) {
        let arena = bumpalo::Bump::new();
        let parser = CssParser::new(source, 0, &arena).expect("test source lexes");
        assert_eq!(parser.current_kind(), TokenKind::Identifier);
        let is_rule = identifier_child_is_rule(&parser).expect("test source disambiguates");
        let stashed = parser.take_declaration(parser.current_end()).is_some();
        (is_rule, stashed)
    }

    /// A `:` found across the property→colon gap is the head the rule-or-declaration scan
    /// goes on from, so the gap is walked once; a gap the bytes decline reaches the token
    /// lookahead, and a `:` it finds still settles the child, on the token walk alone.
    #[test]
    fn identifier_child_walks_the_gap_once() {
        // declarations, located from the bytes and stashed
        assert_eq!(identifier_child("color: red;"), (false, true));
        assert_eq!(identifier_child("color:red}"), (false, true));
        assert_eq!(
            identifier_child("color\t/* a */ /* b */\n: red"),
            (false, true)
        );
        assert_eq!(identifier_child("filter:blur(5px);"), (false, true));
        // nested rules: a `:` the value walk finds a `{` past, and no `:` at all
        assert_eq!(identifier_child("span:hover { }"), (true, false));
        assert_eq!(
            identifier_child("span:not(:last-child)::after { }"),
            (true, false)
        );
        assert_eq!(identifier_child("div { }"), (true, false));
        assert_eq!(identifier_child("a.b { }"), (true, false));
        assert_eq!(identifier_child("a /* c */ [b] { }"), (true, false));
        // a boundary run in the gap declines the byte walk; the token lookahead steps it to
        // the `:`, and the declaration is left for `parse_declaration`'s token head
        assert_eq!(identifier_child("color \u{a0}: red;"), (false, false));
        assert_eq!(
            identifier_child("color /* c */\u{a0}: red;"),
            (false, false)
        );
        // a non-ASCII value start declines the byte head past a `:` the gap walk found
        assert_eq!(identifier_child("color: \u{85}red;"), (false, false));
        assert_eq!(identifier_child("content: \u{e9};"), (false, false));
    }

    /// Both byte scans over the declaration `source` holds, graded exactly as the debug
    /// oracles in [`scan_value`] and [`scan_rule_or_declaration`] grade them: a scan that
    /// answers must agree with the token walk fact for fact (an `Err` walk fails too — the
    /// scan accepted what the lexer rejects). Returns whether each scan declined, as
    /// `(plain, fused)`, for the caller to pin.
    fn value_scans_agree_with_the_token_walk(source: &str) -> (bool, bool) {
        let bytes = source.as_bytes();
        let colon = source.find(':').expect("test source needs a `:`");
        let mut value_start = colon + 1;
        while value_start < bytes.len() && is_ascii_css_whitespace(bytes[value_start]) {
            value_start += 1;
        }
        let walked = scan_value_tokens(source, 0, value_start).ok();

        let plain = scan_value_bytes(source, value_start).map(|(facts, _)| facts);
        if let Some(facts) = plain {
            assert_eq!(
                Some(facts),
                walked,
                "plain value scan disagreed: {source:?}"
            );
        }

        // The fused walk takes the head the disambiguation's gap walk ends in; the head scan
        // steps to the `:` itself, so the colon's own offset serves as the property's end.
        let fused = scan_declaration_head_bytes(bytes, colon)
            .and_then(|head| scan_rule_or_declaration_and_value_bytes(source, head));
        match &fused {
            Some(Disambiguation::Declaration(scanned)) => {
                assert_eq!(
                    scan_rule_or_declaration_tokens(source, 0, colon).ok(),
                    Some(false),
                    "fused verdict disagreed: {source:?}"
                );
                assert_eq!(
                    Some(scanned.facts),
                    walked,
                    "fused value scan disagreed: {source:?}"
                );
            }
            Some(Disambiguation::Rule) => panic!("a declaration read as a rule: {source:?}"),
            None => {}
        }
        (plain.is_none(), fused.is_none())
    }

    /// A `url(` with no matching unescaped `)` declines both scans, whatever follows it.
    ///
    /// tsv's lexer models no `<bad-url-token>` (css-syntax-3 §4.3.6's recovery): it takes an
    /// unclosed url-token to end-of-source, so "runs to end-of-source" here is the LEXER's
    /// model, not a claim that each spelling is a spec url-token — `url(x;` is a
    /// `<bad-url-token>` to the spec, with the same extent. Running to end-of-source, the
    /// token can end in whitespace, which the byte scan's `value_end` trim cannot model.
    /// The decline is the tri-state [`ParenOpen::UnterminatedUrl`], not a whitespace test:
    /// `url(x` and `url(x;` end in no whitespace and must decline too, as must a url after
    /// another member (`x url( `), a url inside a function, and both the fused (property)
    /// and plain (custom property) paths.
    ///
    /// The three escaped-close spellings (`url(x\)`, whose `\)` leaves nothing to close
    /// the url) are regression guards, not reproducers: were their `(` misread as nesting,
    /// the scan would still decline at the `\` the loop reaches next.
    #[test]
    fn an_unterminated_url_token_declines_both_scans() {
        for source in [
            "a{color: url( \n",
            "a{color: url( ",
            "a{color: URL( \n",
            "a{color: url(x \n",
            "a{color: url(x",
            "a{color: url(x;",
            "a{color: x url( \n",
            "a{--x: url( \n",
            "a{b: f(url( \n",
            "a{b: f(url(x \n",
            // regression guards: the `\` arm declines these even without the tri-state
            "a{color: url(x\\)",
            "a{color: url(x\\) ",
            "a{color: url(x\\)\n",
        ] {
            assert_eq!(
                value_scans_agree_with_the_token_walk(source),
                (true, true),
                "{source:?}"
            );
        }
    }

    /// The tri-state itself, at the `(` of each spelling.
    #[test]
    fn paren_open_kind_tells_the_three_apart() {
        let kind = |source: &str| {
            let value_start = source.find(':').expect("test source needs a `:`") + 2;
            let open = source.rfind('(').expect("test source needs a `(`");
            paren_open_kind(source, source.as_bytes(), value_start, open)
        };
        assert_eq!(kind("a{b: url(x)}"), ParenOpen::UrlToken { end: 11 });
        assert_eq!(kind("a{b: url(x\\))}"), ParenOpen::UrlToken { end: 13 });
        assert_eq!(kind("a{b: url(x"), ParenOpen::UnterminatedUrl);
        assert_eq!(kind("a{b: url(x\\)"), ParenOpen::UnterminatedUrl);
        assert_eq!(kind("a{b: f(x"), ParenOpen::Nesting);
        assert_eq!(kind("a{b: url('x"), ParenOpen::Nesting);
        assert_eq!(kind("a{b: 5url(x"), ParenOpen::Nesting);
        assert_eq!(kind("a{b: blurl(x"), ParenOpen::Nesting);
    }

    /// The controls: a terminated url-token is stepped over whole and answered by the
    /// byte scan, and an unterminated ordinary `(` is plain nesting that the scan answers
    /// too (its last token, the `(`, ends on a non-whitespace byte, so the trim is exact).
    #[test]
    fn a_terminated_url_and_an_unterminated_plain_paren_are_answered() {
        for source in [
            "a{color: url(x)}",
            "a{color: url(x) }",
            "a{color: url( x )\n",
            "a{b: f( \n",
            "a{b: f(x \n",
        ] {
            assert_eq!(
                value_scans_agree_with_the_token_walk(source),
                (false, false),
                "{source:?}"
            );
        }
    }

    /// End to end, through the parser that runs the debug oracles: an unterminated
    /// url-token is the parse error the token walk reports (the url swallows the rest of
    /// the block, so its `}` never arrives), never an oracle panic.
    #[test]
    fn an_unterminated_url_token_is_the_token_walks_parse_error() {
        for source in [
            "a{color: url( \n",
            "a{color: url(x \n",
            "a{--x: url( \n",
            "a{color: url(x\\) \n",
            "a{color: url(x;",
            "a{color: x url( \n",
            "a{b: f(url( \n",
            "@media x{a{color: url( \n",
            "\n\ta {\n\t\tcolor: url(x;\n\t}\n",
        ] {
            let arena = bumpalo::Bump::new();
            let err = crate::parse(source, &arena).expect_err(source);
            assert!(
                err.to_string().contains("Expected '}'"),
                "{source:?}: {err}"
            );
        }
        let arena = bumpalo::Bump::new();
        assert!(crate::parse("a{color: url(x)}", &arena).is_ok());
    }
}
