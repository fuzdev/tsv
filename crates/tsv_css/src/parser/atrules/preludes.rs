use super::{CssParser, is_boolean_operator_keyword};
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::parser::selectors::{parse_complex_selector_list, parse_forgiving_selector_list};
use tsv_lang::{ParseError, Span};

/// Whether the current token is a CSS `<function-token>` — an identifier
/// immediately followed by `(` with no intervening whitespace (`url(`, `layer(`,
/// `supports(`, `selector(`). The lexer emits the name and `(` as separate tokens
/// (only an unquoted `url(...)` is one opaque `Url` token), so the
/// function-vs-plain-ident distinction is recovered here by peeking the source byte
/// after the identifier.
///
/// This is the whole of css-values-4 §"Functional Notations" on the point — any
/// character between the name and the `(`, whitespace included, leaves an ordinary
/// identifier followed by a parenthesis — and every reader of that rule in this file
/// asks it here, so a condition part, a container name and a value function cannot
/// disagree about what a function is.
fn is_function_token(parser: &CssParser<'_, '_>) -> bool {
    parser.check(TokenKind::Identifier) && {
        let end_pos = parser.current_end;
        parser.source.get(end_pos..=end_pos) == Some("(")
    }
}

/// Is the current token the `selector` of a `selector(` function token — the
/// condition grammar's one function whose argument is a selector rather than a
/// declaration?
///
/// The name is matched ASCII case-insensitively, css-values-4's other half of the
/// same section: "like keywords, function names are ASCII case-insensitive".
fn at_selector_function(parser: &CssParser<'_, '_>) -> bool {
    is_function_token(parser) && parser.current_identifier().eq_ignore_ascii_case("selector")
}

/// Read a `selector()` argument as the selector it is, with the parser positioned
/// on the argument's first token (just past the `(`).
///
/// On success the parser is seated on the closing `)`. Otherwise it is rewound to
/// `arg_start` — every token and every comment registration undone — and the
/// caller consumes the argument as opaque text: `<supports-in-parens>` falls
/// through to `<general-enclosed>` (css-conditional-3), a `<function-token>`
/// whose contents the grammar leaves undefined, so there is nothing to normalize
/// and a UA evaluates it as false.
///
/// Landing anywhere but the closing `)` counts as failure even when the parse
/// returned `Ok`: `parse_complex_selector` stops at the first token it cannot
/// continue with rather than erroring, so `Ok` alone would also accept a selector
/// followed by garbage.
fn parse_selector_argument<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    arg_start: usize,
    comments_len: usize,
) -> Result<Option<&'arena [ComplexSelector<'arena>]>, ParseError> {
    if let Ok(list) = parse_complex_selector_list(parser)
        && parser.check(TokenKind::RightParen)
    {
        return Ok(Some(list.selectors));
    }
    parser.rewind_to(arg_start, comments_len)?;
    Ok(None)
}

/// Must a spacing rule emit its own separator before the current token, or does the
/// part buffer already end in one?
///
/// The two rules that pad a token — the boolean operator's space and the comment's —
/// both ask this, so they can never stack into a double space with each other or with
/// the rules that ran before them (a source whitespace run, the space after a value
/// `:`, the space after a comment). `trailing_spaces` is the buffer's count of
/// *programmatic* trailing spaces, so it answers that directly; the one separated
/// position it can't see is an opening paren, which wants no separator at all
/// (`(not (…))` and `(/* c */ a: b)` keep the paren tight, matching prettier). `None`
/// is the part's first token, which is always that `(`.
fn needs_separator_before(prev: Option<TokenKind>, trailing_spaces: usize) -> bool {
    trailing_spaces == 0 && !matches!(prev, None | Some(TokenKind::LeftParen))
}

/// Skip a gap between condition parts, registering its comments and widening `end`
/// past the last of them.
///
/// The query's own gaps — before a part, and after a connector (`(a) /* c */ and
/// /* d */ (b)`) — are printer-reconstructed, so their comments must be *registered*
/// rather than emitted (a dropping skip loses them silently). That is
/// `skip_whitespace_registering_comments`'s job everywhere else in this file; the one
/// thing it can't do is report where the last comment ended, and the query's span has
/// to cover it — a trailing `(a) /* c */` gap is inside the prelude the printer
/// re-emits from. Hence the loop, in one place rather than at each gap.
///
/// `run` collects the gap's comments as it goes — the query cannot yet know whether
/// a part will follow to bind them, and one that never does keeps them (see
/// [`ConditionQuery::trailing_operators`]). A part that does binds them, and the query
/// loop clears the buffer.
fn skip_gap_registering_comments(
    parser: &mut CssParser<'_, '_>,
    end: &mut usize,
    run: &mut OperatorRun,
) -> Result<(), ParseError> {
    parser.skip_whitespace()?;
    while parser.check(TokenKind::Comment) {
        parser.register_current_comment();
        run.push_comment(parser.current_value());
        *end = parser.base_offset() + parser.current_end;
        parser.advance()?;
        parser.skip_whitespace()?;
    }
    Ok(())
}

/// What a condition query has consumed since its last part — the operators, whether a
/// part will bind them or not, and the gap comments collected beside them.
///
/// The facts move together (a part binds all of them at once), so they are one value:
/// keeping them as parallel locals is how a site comes to update some of them.
///
/// The buffer answers **two** questions off one walk, which is why they share it: the
/// operator run a following part binds ([`Self::connector_run`], which becomes that
/// part's `connector_run`) and the one no part ever comes for ([`Self::finish`], which
/// becomes the query's `trailing_operators`). They differ only in their bounds — the
/// bound run stops at the first and last *operator*, so the comments on either side of
/// it stay the printer's to claim, while the unbound one is the whole gap, there being
/// no part left to claim anything. Named for the material rather than for either answer:
/// whether the run is bound is a question you ask it, not a fact about the buffer.
struct OperatorRun {
    /// The words so far, a single space between them whatever the author wrote — the run's
    /// spacing is the grammar's, exactly as a condition part's is (`parse_condition_part`
    /// builds `part_buf` the same way). Each word is pushed **verbatim** from source, so an
    /// operator keeps its case (`AND` stays `AND`) and a comment its interior.
    text: String,
    /// The bound run's bounds, set once an operator lands. `None` while the buffer holds
    /// only comments — a gap that ends the query holding just those
    /// (`@container b /* c */ {`) is the ordinary trailing-comment claim's, and they stay
    /// registered for it, so only an operator turns the buffer into a run.
    ///
    /// One `Option` rather than three fields because the three are meaningless apart: an
    /// end with no start, or a registry length belonging to an operator that never landed,
    /// are states this buffer must not be able to hold.
    operators: Option<OperatorBounds>,
    /// The comment registry's length when the run began, to truncate back to if it does
    /// end up claiming its comments.
    comments_len: usize,
}

/// Where a bound run sits in [`OperatorRun::text`], and what the comment registry owes it.
struct OperatorBounds {
    /// Where the run's first operator starts.
    start: usize,
    /// Where its last operator ends. Everything between the two is run text — the later
    /// operators and the comments among them.
    end: usize,
    /// The registry's length when the **first** operator landed, to truncate back to when a
    /// later one proves the comments since then are inside the run's own text.
    comments_len: usize,
}

impl OperatorRun {
    /// A fresh run, with the comment registry standing at `comments_len`.
    fn new(comments_len: usize) -> Self {
        Self {
            text: String::new(),
            operators: None,
            comments_len,
        }
    }

    /// Collect a gap comment — not yet a run, since a part may still bind it.
    fn push_comment(&mut self, comment: &str) {
        self.push_word(comment);
    }

    /// Collect an operator, which is what makes the buffer a run.
    ///
    /// `comments_len` is the registry's length now. The return is the length to truncate
    /// it back to, `Some` only for an operator that is **not** the run's first: the
    /// comments registered since that first one sit *between* two operators, so the run's
    /// own text carries them and their registration has to go, or the printer's gap sweep
    /// would print each a second time.
    ///
    /// `#[must_use]` is the pairing's enforcement — a caller that pushes an operator and
    /// drops the answer leaves a comment for two emitters to print, and the double print is
    /// invisible to every fixed-point gate.
    #[must_use]
    fn push_operator(&mut self, operator: &str, comments_len: usize) -> Option<usize> {
        // Read before the push: `None` here is exactly "this is the run's first operator".
        let unregister_to = self.operators.as_ref().map(|bounds| bounds.comments_len);
        let start = self.push_word(operator);
        match &mut self.operators {
            Some(bounds) => bounds.end = self.text.len(),
            None => {
                self.operators = Some(OperatorBounds {
                    start,
                    end: self.text.len(),
                    comments_len,
                });
            }
        }
        unregister_to
    }

    /// Push a word and report where it landed in `text`.
    fn push_word(&mut self, word: &str) -> usize {
        if !self.text.is_empty() {
            self.text.push(' ');
        }
        let start = self.text.len();
        self.text.push_str(word);
        start
    }

    /// The operator run a part would bind — first operator through last, the comments
    /// between them included — or `None` when nothing but comments has been collected.
    ///
    /// Bounded at the operators on **both** ends, unlike [`Self::finish`]: a comment
    /// before the first (`(a: b) /* c */ and (c: d)`) or after the last (`and /* c */
    /// (c: d)`) is one the printer's own gap sweep claims onto its authored side of the
    /// separator, so the run must not swallow it.
    fn connector_run(&self) -> Option<&str> {
        self.operators
            .as_ref()
            .map(|bounds| &self.text[bounds.start..bounds.end])
    }

    /// A part bound everything collected so far: its connector run rides the part and the
    /// gap comments outside that run stay registered for the printer's separator, so the
    /// run restarts here — in place, keeping the buffer's allocation across a many-part
    /// query.
    fn bind(&mut self, comments_len: usize) {
        self.text.clear();
        self.operators = None;
        self.comments_len = comments_len;
    }

    /// The run's text and the comment-registry length to truncate back to, or `None` when
    /// the query bound everything it consumed — every well-formed condition.
    ///
    /// The whole buffer, comments on both ends included: no part is coming to claim them,
    /// so the run is the only carrier they have.
    fn finish(&self) -> Option<(&str, usize)> {
        self.operators
            .as_ref()
            .map(|_| (self.text.as_str(), self.comments_len))
    }
}

/// Can a boolean operator (`and`/`or`/`not`) begin at this point in a condition?
///
/// Only where the grammar can start one: at the beginning of a condition, or
/// against a parenthesis — `not (…)`, `(…) and (…)`. Everywhere else an
/// identifier spelled `and` is just an identifier: a value (`(font-family: and)`),
/// a class (`selector(.and)`), a pseudo-class name (`selector(div:not(.a))`).
/// Spacing one of those as an operator does not merely look wrong, it rewrites
/// the declaration or the selector.
///
/// `prev` is the previous **significant** token — whitespace and comments are
/// transparent, since neither can turn an identifier into an operator.
fn boolean_operator_position(prev: Option<TokenKind>, in_selector_args: bool) -> bool {
    !in_selector_args
        && matches!(
            prev,
            None | Some(TokenKind::LeftParen) | Some(TokenKind::RightParen)
        )
}

/// Which of prettier's two prelude readers a condition prelude is read by — the axis
/// that decides whether its value text is regrouped or kept as the author wrote it.
///
/// `parser-postcss.js` hands `@supports` (and the `supports()` of an `@import`) to
/// `parseValue`, which regroups the value into its own nodes and prints its own
/// separators, so a comma there is a separator and takes prettier's `, `.
/// `@container` is on **neither** of prettier's lists: its params stay raw, so a comma
/// between its parens is the author's byte and moving it would be a rewrite.
///
/// The axis `value_normalization::ValueReader` already names for the printer's
/// number/hex rules, stated here for the parser's separator rules.
/// ⚠️ The variants name **prettier's** reader, not tsv's output. `Raw` bounds the comma
/// rule and nothing else: tsv still applies its own boolean-operator and value-colon
/// spacing to a `@container` prelude, a deliberate divergence with its own catalog entry
/// (`container_spacing_prettier_divergence` — ◆spec_violation, the grammar requires the
/// space `and(` omits). Don't read `Raw` as "tsv emits this verbatim".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConditionReader {
    /// `@supports`, and `@import`'s `supports()` — read by `parseValue`.
    Value,
    /// `@container` — read by neither of prettier's readers, so its prelude text is
    /// prettier's verbatim.
    Raw,
}

/// Parse a condition query — `(prop: val)` parts connected by `and`/`or`, with
/// an optional leading `not` and function-style `selector(...)` conditions.
///
/// This *is* the entire `@supports` prelude (`<supports-condition>`), and
/// `@container` reuses it verbatim for its `<container-query>` — the two grammars
/// are identical; `@container` only adds an optional `<container-name>` preamble
/// before calling this. The returned span starts at the parser's current
/// position (the first condition token); `parse_container_prelude` widens the
/// start to cover the name.
///
/// Examples:
/// - `(display: grid)` - single condition
/// - `(display: grid) and (flex: 1)` - conjunction
/// - `not (color: red)` - negation
/// - `(a) and (b) or (c)` - mixed (parsed left-to-right)
pub(super) fn parse_condition_query<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    reader: ConditionReader,
) -> Result<(ConditionQuery<'arena>, Span), ParseError> {
    let start = parser.base_offset() + parser.current_start;
    let mut parts = parser.bvec();
    let mut current_connector: Option<ConditionConnector> = None;
    let mut end_pos = start;
    // What the loop has consumed since the last part — the connector run a part binds
    // (`ConditionPart::connector_run`, whose source text and case the printer preserves),
    // or, if none ever comes, `ConditionQuery::trailing_operators`.
    let mut run = OperatorRun::new(parser.comments.len());

    while !parser.at_prelude_end() {
        // The gap before a part (`(a) /* comment */ and (b)`)
        skip_gap_registering_comments(parser, &mut end_pos, &mut run)?;

        // Check for `and`/`or` connector. CSS grammar keywords are ASCII
        // case-insensitive (CSS Syntax 3), so `AND`/`Or` connect like `and`; the
        // enum normalizes for logic but the run's source text is kept in `connector_run`
        // and preserved by the printer (matching prettier).
        if parser.check(TokenKind::Identifier) {
            let ident = parser.current_identifier();

            let connector = if ident.eq_ignore_ascii_case("and") {
                Some(ConditionConnector::And)
            } else if ident.eq_ignore_ascii_case("or") {
                Some(ConditionConnector::Or)
            } else {
                None
            };

            if let Some(conn) = connector {
                // This is a connector between parts. `current_connector` is the run's
                // LAST operator — the one whose kind the printer splits the gap's
                // comments around — while the run itself keeps every word.
                current_connector = Some(conn);
                if let Some(comments_len) =
                    run.push_operator(parser.current_value(), parser.comments.len())
                {
                    parser.comments.truncate(comments_len);
                }
                // Widen over the keyword itself, so a connector that never finds its
                // part still lies inside the query's span (a part that does follow
                // widens past it anyway).
                end_pos = parser.base_offset() + parser.current_end;
                parser.advance()?;
                // The gap after a connector (`and /* comment */ (b)`)
                skip_gap_registering_comments(parser, &mut end_pos, &mut run)?;
                continue;
            }
        }

        // The run this part binds, allocated once here rather than per operator.
        let connector_run: Option<&'arena str> =
            run.connector_run().map(|text| parser.alloc_str_in(text));
        match parse_condition_part(
            parser,
            current_connector.take(),
            connector_run,
            end_pos,
            reader,
        )? {
            ConditionPartOutcome::Parsed { part, end, .. } => {
                end_pos = end;
                run.bind(parser.comments.len());
                if let Some(part) = part {
                    parts.push(part);
                }
            }
            // Not a valid condition part, so the query ends here; the caller reports
            // whatever follows. A `not` prefix the part reader consumed before giving up
            // comes back with it — the reader took those bytes, so it owes them.
            ConditionPartOutcome::NotAPart { consumed, end } => {
                if let Some(consumed) = consumed {
                    // No part is coming, so `finish` truncates the registry past this
                    // anyway; honored all the same, so one rule holds at every call.
                    if let Some(comments_len) = run.push_operator(consumed, parser.comments.len()) {
                        parser.comments.truncate(comments_len);
                    }
                }
                end_pos = end;
                break;
            }
        }
    }

    // Whatever is left is a run of operators with no operand. Its comments were
    // registered on the way in, when a part might still have bound them; they ride the
    // run's own text now, so drop the registrations or both would print it — the pair
    // `CssParser::rewind_to` keeps together, for the same reason.
    let trailing_operators = run.finish().map(|(text, comments_len)| {
        parser.comments.truncate(comments_len);
        parser.alloc_str_in(text)
    });

    let span = Span {
        start: start as u32,
        end: end_pos as u32,
    };

    Ok((
        ConditionQuery {
            parts: parts.into_bump_slice(),
            trailing_operators,
        },
        span,
    ))
}

/// What one `parse_condition_part` call produced.
enum ConditionPartOutcome<'arena> {
    /// A parsed part — `None` when its content came out empty, which records nothing
    /// but has still consumed tokens. `end` is the widened end position; `closed` is
    /// false only when the content ran to end-of-input without the part's own `)`.
    Parsed {
        part: Option<ConditionPart<'arena>>,
        end: usize,
        closed: bool,
    },
    /// The current token can't start a condition part. `consumed` is the `not` prefix
    /// the reader had already taken (with any comments after it) — the one thing it
    /// consumes before it can tell, and content the query owes back rather than drops;
    /// `None` on the ordinary path, where nothing was consumed. `end` is the widened end.
    NotAPart {
        consumed: Option<&'arena str>,
        end: usize,
    },
}

/// Parse one condition part — an optional leading `not`, an optional function name,
/// then the parenthesized content, whose tokens are re-emitted with the spacing the
/// grammar calls for (the space after a value `:`, the spacing around a boolean
/// operator), except where the grammar says they are a selector.
///
/// `end_pos` is the query's end position so far, widened by the part and handed back
/// in the outcome.
fn parse_condition_part<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    connector: Option<ConditionConnector>,
    connector_run: Option<&'arena str>,
    mut end_pos: usize,
    reader: ConditionReader,
) -> Result<ConditionPartOutcome<'arena>, ParseError> {
    let part_start = parser.span_pos(parser.current_start);
    // One growable buffer instead of a `Vec<String>` of per-token / per-space pieces
    // joined at the end (mirrors `parse_raw_prelude_content` and
    // `parse_declaration`): tokens `push_str` straight in, separators are a single
    // `push(' ')`. `trailing_spaces` counts the trailing programmatic spaces (the
    // collapse unit) so `truncate` strips exactly those, never a token's own
    // escape-terminator space — the old `Vec` "last part is `\" \"`" test, exactly.
    let mut part_buf = String::new();
    // The part's content. Everything lands in `part_buf` and flushes to one `Text`
    // segment, except a `selector()` argument, which is parsed as a selector and
    // rides its own segment for the printer to hand to the selector printer.
    let mut segments = parser.bvec();
    let mut trailing_spaces: usize = 0;
    let mut paren_depth: usize = 0;
    // `Some(depth)` while inside a `selector()` argument the selector parse
    // declined — a `<general-enclosed>`, where the declaration rules below (the
    // value colon's spacing, boolean operators) must not fire.
    let mut general_enclosed_selector: Option<usize> = None;

    // Check for leading `not` (ASCII case-insensitive). Its source case is kept
    // (pushed verbatim), preserved by the printer like the `and`/`or` connectors.
    if parser.check(TokenKind::Identifier) {
        let ident = parser.current_identifier();
        if ident.eq_ignore_ascii_case("not") {
            part_buf.push_str(parser.current_value());
            trailing_spaces = 0;
            // Widen over the keyword before consuming it: a `not` whose operand never
            // arrives is handed back to the query with this end (`NotAPart`), and the
            // query's span has to reach it.
            end_pos = parser.base_offset() + parser.current_end;
            parser.advance()?;
            parser.skip_whitespace()?;
            // Include comments after `not` in content (e.g., `not /* comment */ (...)`)
            // These go in the part buffer rather than being registered, since they're
            // inside the condition part's span
            while parser.check(TokenKind::Comment) {
                part_buf.push(' ');
                part_buf.push_str(parser.current_value());
                trailing_spaces = 0;
                end_pos = parser.base_offset() + parser.current_end;
                parser.advance()?;
                parser.skip_whitespace()?;
            }
            part_buf.push(' ');
            trailing_spaces += 1;
        }
    }

    // Check for function-style condition like `selector(:has(...))`: an
    // identifier directly followed by `(`. The name is serialized verbatim from
    // source (escapes preserved) — only `and`/`or`/`not` keyword matches decode.
    let mut at_selector_args = false;
    if is_function_token(parser) {
        at_selector_args = at_selector_function(parser);
        part_buf.push_str(parser.current_value());
        trailing_spaces = 0;
        parser.advance()?;
        // Continue to parse the parenthesized part below
    }

    // Now parse the parenthesized condition
    if !parser.check(TokenKind::LeftParen) {
        // Not a valid @supports part, so the query ends here. Only the `not` prefix
        // above can have consumed anything by now (a function token is an identifier
        // glued to its `(`, so that branch always finds one); it goes back to the query
        // as the operator-with-no-operand it is, rather than being dropped with the
        // buffer.
        let consumed = (!part_buf.is_empty()).then(|| {
            parser
                .alloc_str_in(part_buf.trim_end_matches(crate::whitespace::is_boundary_whitespace))
        });
        return Ok(ConditionPartOutcome::NotAPart {
            consumed,
            end: end_pos,
        });
    }

    // Whether the part's own closing `)` was reached. False means the content ran
    // to end-of-input unterminated, which only the `supports()` caller cares about
    // (a condition prelude is bounded by the `{`/`;` its at-rule needs anyway).
    let mut closed = false;

    // Parse until we close all parens and hit whitespace/and/or/brace
    // Track state for whitespace normalization
    let mut prev_token_kind: Option<TokenKind> = None;
    // The previous token that can decide a spacing rule — whitespace and comments
    // are transparent to both readers (the value colon's space, and whether an
    // `and`/`or`/`not` sits where the grammar can start a boolean operator).
    let mut last_significant_kind: Option<TokenKind> = None;

    while !parser.check(TokenKind::Eof) {
        let opening_selector_args = at_selector_args;
        at_selector_args = false;

        // A `selector()` argument is a selector, not a declaration: parse it as
        // one so the printer can hand it to the selector printer, and the
        // declaration rules below never see its tokens. An argument that declines
        // to parse is a `<general-enclosed>` and falls through to the loop, which
        // consumes it as opaque text.
        if opening_selector_args && parser.check(TokenKind::LeftParen) {
            part_buf.push('(');
            trailing_spaces = 0;
            parser.advance()?; // consume '('
            let arg_start = parser.current_start;
            let comments_len = parser.comments.len();
            if let Some(selectors) = parse_selector_argument(parser, arg_start, comments_len)? {
                if !part_buf.is_empty() {
                    segments.push(ConditionSegment::Text(parser.alloc_str_in(&part_buf)));
                    part_buf.clear();
                }
                segments.push(ConditionSegment::Selectors(selectors));
                // Seated on the closing `)`; the `(` it matches was consumed above,
                // so the two cancel and `paren_depth` never saw either.
                part_buf.push(')');
                end_pos = parser.base_offset() + parser.current_end;
                parser.advance()?;
                prev_token_kind = Some(TokenKind::RightParen);
                last_significant_kind = Some(TokenKind::RightParen);
                if paren_depth == 0 {
                    // The part *was* the `selector()` call, and its `)` just closed
                    // it (`@supports selector(a)`, no wrapping paren).
                    closed = true;
                    break;
                }
                continue;
            }
            // Rewound to the argument's first token: the `(` stays consumed, so
            // account for it and mark the region the declaration rules skip.
            paren_depth += 1;
            general_enclosed_selector.get_or_insert(paren_depth);
            prev_token_kind = Some(TokenKind::LeftParen);
            last_significant_kind = Some(TokenKind::LeftParen);
            continue;
        }

        // Track paren depth
        if parser.check(TokenKind::LeftParen) {
            paren_depth += 1;
        } else if parser.check(TokenKind::RightParen) {
            if paren_depth == 0 {
                break;
            }
            paren_depth -= 1;
            if general_enclosed_selector == Some(paren_depth + 1) {
                general_enclosed_selector = None;
            }
        }

        // Check for end of part (at top level)
        if paren_depth == 0 && parser.check(TokenKind::RightParen) {
            // Include the closing paren (loop ends here, so no counter reset needed)
            part_buf.push(')');
            end_pos = parser.base_offset() + parser.current_end;
            parser.advance()?;
            closed = true;
            break;
        }

        // Handle whitespace normalization
        if parser.check(TokenKind::Whitespace) {
            // A whitespace run right after a value colon (`(a: )`, empty value) is
            // the prettier-mandated single space after `:` — keep it before `)`
            // rather than dropping it, or `(a: )` would collapse to `(a:)` while
            // `(a:)` gains the space (the colon-space rule below), an F1 oscillation.
            // In `@supports`/`@container` a `:` is always a value colon.
            let after_value_colon = matches!(prev_token_kind, Some(TokenKind::Colon));
            let skip_whitespace = matches!(prev_token_kind, Some(TokenKind::LeftParen))
                || (matches!(parser.peek_kind(), Ok(TokenKind::RightParen)) && !after_value_colon);

            parser.advance()?;

            if skip_whitespace {
                continue;
            }
            part_buf.push(' ');
            trailing_spaces += 1;
            prev_token_kind = Some(TokenKind::Whitespace);
            continue;
        }

        let is_comment = matches!(parser.current_kind, TokenKind::Comment);

        // Check if this is a boolean operator (and/or/not) inside nested parens.
        // Match on the decoded value, not the verbatim source slice, so an escaped
        // operator still spaces correctly — and only where the grammar can start
        // one, so an identifier that merely spells `and` keeps its own spacing.
        let is_bool_op = matches!(&parser.current_kind, TokenKind::Identifier)
            && is_boolean_operator_keyword(parser.current_identifier())
            && boolean_operator_position(
                last_significant_kind,
                general_enclosed_selector.is_some(),
            );

        // The next token opens a `selector()` argument, handled at the top of the
        // loop once its `(` is current.
        at_selector_args = at_selector_function(parser);

        // Two token kinds carry their own leading separator: a boolean operator, and a
        // comment (padded on **both** sides — the after side is handled once the token
        // is emitted, below; emitting only one leaves `(display: grid/* c */)`
        // half-spaced). They ask for it in ONE place, because two emission sites each
        // seeing only its own reason to pad is exactly how the doubled `/* c */  and`
        // arose. The comment's bound is `pads_comment`, which **both** of its sides
        // read for the same reason: inside a `<general-enclosed>` `selector()` argument
        // the tokens are opaque by grammar, so it is bounded like the value colon's rule
        // below — a space inserted inside a compound would turn
        // `selector([a=1.5]/* c */.b)` into a descendant `[a=1.5] .b`, and a bound only
        // one side honors inserts that space from the other. The operator's matching
        // bound rides inside `is_bool_op`, which its after-space reads too.
        let pads_comment = is_comment && general_enclosed_selector.is_none();
        let pads_before = is_bool_op || pads_comment;
        if pads_before && needs_separator_before(prev_token_kind, trailing_spaces) {
            part_buf.push(' ');
            trailing_spaces += 1;
        }

        // A comma is a SEPARATOR on the value reader, which regroups the value into its
        // own nodes and prints its own `, ` — so its left side is stripped below and its
        // right side padded further down, the same pair the value colon takes. Bounded
        // twice: `@container` is on neither of prettier's reader lists (its params are
        // verbatim, comma included), and inside a `<general-enclosed>` `selector()`
        // argument the comma separates a selector list the selector printer owns.
        let pads_comma = matches!(parser.current_kind, TokenKind::Comma)
            && reader == ConditionReader::Value
            && general_enclosed_selector.is_none();

        // Remove trailing whitespace before a value ':' or a separator ',' — only the
        // counted programmatic spaces, never a token's own escape-terminator space. (The
        // counter is reset by the token emission just below, which always runs next.)
        // Inside a `<general-enclosed>` `selector()` argument the colon opens a
        // pseudo-class instead, where a preceding space is a descendant combinator:
        // `selector(div :hover)` is not `selector(div:hover)`. One statement of which
        // tokens strip, the `raw.rs` sibling's shape.
        let strips_leading_space = pads_comma
            || (matches!(parser.current_kind, TokenKind::Colon)
                && general_enclosed_selector.is_none());
        if strips_leading_space {
            part_buf.truncate(part_buf.len() - trailing_spaces);
        }

        // Emit the token verbatim from source: identifiers serialize their raw slice so
        // escapes survive (`\@foo` stays `\@foo`), a string keeps its surrounding quotes,
        // and a comment is included verbatim.
        match &parser.current_kind {
            TokenKind::String { quote } => {
                let content = &parser.source()[parser.current_start + 1..parser.current_end - 1];
                part_buf.push(*quote);
                part_buf.push_str(content);
                part_buf.push(*quote);
            }
            _ => part_buf.push_str(parser.current_value()),
        }
        trailing_spaces = 0;
        let current_kind = parser.current_kind;
        end_pos = parser.base_offset() + parser.current_end;
        parser.advance()?;

        // Add space after boolean operators — a separator to the operand the grammar
        // binds to their right (`and <supports-in-parens>`), so a `)` closing the group
        // means there is nothing to separate from and the space would strand before it
        // (`((a: b) and )`). The comment's own pad below reads the same `)`, and the
        // text path states the rule once more (`Printer::build_and_or_wrap_doc`).
        if is_bool_op
            && !parser.check(TokenKind::Whitespace)
            && !parser.check(TokenKind::RightParen)
        {
            part_buf.push(' ');
            trailing_spaces += 1;
        }

        // Add space after comment if followed by non-whitespace
        // (e.g., `/* comment */ grid` needs space before `grid`)
        if pads_comment
            && !parser.check(TokenKind::Whitespace)
            && !parser.check(TokenKind::RightParen)
        {
            part_buf.push(' ');
            trailing_spaces += 1;
        }

        // Add space after a value-reader comma — `pads_comma`, whose left-side strip
        // above is the same rule read from the other side.
        if pads_comma && !parser.check(TokenKind::Whitespace) {
            part_buf.push(' ');
            trailing_spaces += 1;
        }

        // Add space after ':' for property:value pairs — a value colon only. The
        // colon of a pseudo-class inside a `<general-enclosed>` argument binds the
        // name to it (`selector(div:hover)`), and spacing it makes a selector that
        // no longer parses.
        if !parser.check(TokenKind::Whitespace)
            && matches!(current_kind, TokenKind::Colon)
            && general_enclosed_selector.is_none()
            && matches!(
                last_significant_kind,
                Some(TokenKind::Identifier)
                    | Some(TokenKind::Number)
                    | Some(TokenKind::Dimension { .. })
                    | Some(TokenKind::Percentage)
            )
        {
            part_buf.push(' ');
            trailing_spaces += 1;
        }

        prev_token_kind = Some(current_kind);
        if !matches!(current_kind, TokenKind::Whitespace | TokenKind::Comment) {
            last_significant_kind = Some(current_kind);
        }
    }

    // Build the part. The buffer's outer whitespace is trimmed as one whole:
    // only the first and last segments can carry any, and both
    // are `Text` whenever a selector segment is present — the function name opens
    // the part and its `)` closes it.
    //
    // ⚠️ The trim's class is `is_boundary_whitespace`, **not** `str::trim`'s Unicode
    // `White_Space` — which excludes `U+FEFF`, where JS `\s` includes it. This gap is also
    // the one `build_condition_query_doc` restores a boundary run into, and the two have to
    // agree on where the part's text begins or the character belongs to both: a
    // `@container name <ZWNBSP>(a: b)` kept it inside the segment AND beside it, doubling the
    // run on every pass. Same lesson as the declaration's property→colon trim
    // (`trim_property_part`) — a printer-facing trim beside a boundary claim owes the claim's
    // class.
    if !part_buf.is_empty() {
        segments.push(ConditionSegment::Text(parser.alloc_str_in(&part_buf)));
    }
    if let Some(ConditionSegment::Text(first)) = segments.first_mut() {
        *first = first.trim_start_matches(crate::whitespace::is_boundary_whitespace);
    }
    if let Some(ConditionSegment::Text(last)) = segments.last_mut() {
        *last = last.trim_end_matches(crate::whitespace::is_boundary_whitespace);
    }
    let is_empty = segments
        .iter()
        .all(|segment| matches!(segment, ConditionSegment::Text(text) if text.is_empty()));
    let part = (!is_empty).then(|| ConditionPart {
        connector,
        connector_run,
        segments: segments.into_bump_slice(),
        span: Span {
            start: part_start,
            end: end_pos as u32,
        },
    });

    Ok(ConditionPartOutcome::Parsed {
        part,
        end: end_pos,
        closed,
    })
}

/// Parse the argument of an `@import` prelude's `supports()` as the condition it is,
/// with the parser seated on the function's `(`.
///
/// `supports( <supports-condition> | <declaration> )` (css-cascade-5 §"Conditional
/// import rules") is the same grammar `@supports` takes, so the argument is read by
/// the same reader and printed by the same printer — one condition, one form,
/// wherever it appears. Seating on the `(` rather than past it is what makes that
/// work: **the function's own parentheses are the condition part's**, so the bare
/// `<declaration>` alternative (`supports(display: grid)`) reuses the
/// parenthesized-part grammar unchanged instead of needing a second reader.
///
/// Exactly one part can come of that (the parens wrap the whole argument), so this
/// never runs the query loop — which is also what bounds it to the function:
/// whatever follows the `)` is the caller's (`supports(a) screen`).
///
/// Returns with the parser just past the matching `)`, or `None` when the argument is
/// not a parenthesized part or never closed — the caller reports the unterminated
/// function.
pub(super) fn parse_supports_function_condition<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<Option<(ConditionQuery<'arena>, Span)>, ParseError> {
    let start = parser.base_offset() + parser.current_start;
    let ConditionPartOutcome::Parsed {
        part,
        end,
        closed: true,
    } = parse_condition_part(parser, None, None, start, ConditionReader::Value)?
    else {
        return Ok(None);
    };

    let mut parts = parser.bvec();
    if let Some(part) = part {
        parts.push(part);
    }
    Ok(Some((
        ConditionQuery {
            // The query loop never runs here, so nothing can go unbound: the one part is
            // the function's parenthesized argument, and an operator inside it is the
            // part's own text.
            parts: parts.into_bump_slice(),
            trailing_operators: None,
        },
        Span {
            start: start as u32,
            end: end as u32,
        },
    )))
}

/// Parse @container prelude into structured condition parts with optional name
///
/// CSS Syntax: `@container [<container-name>]? <container-query>`
/// where container-query is similar to @supports: `(prop: val)` parts connected by `and`/`or`
///
/// Examples:
/// - `(min-width: 100px)` - no name, single condition
/// - `(min-width: 100px) and (max-width: 200px)` - no name, conjunction
/// - `sidebar (min-width: 100px)` - named container
/// - `sidebar (min-width: 100px) and (max-width: 200px)` - named container with conjunction
pub(super) fn parse_container_prelude<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<(Option<&'arena str>, ConditionQuery<'arena>, Span), ParseError> {
    let start = parser.span_pos(parser.current_start);

    // Check for optional container name: an identifier before the first '(' that
    // isn't a `not`/`and`/`or` keyword or a function call (`style(...)`, no space
    // before `(`). The keyword/function exclusion decodes + a structural `(`
    // lookahead; the stored name is serialized verbatim from source so escapes
    // survive (`\@named` stays `\@named`).
    let container_name = if parser.check(TokenKind::Identifier)
        && !is_function_token(parser)
        && !is_boolean_operator_keyword(parser.current_identifier())
    {
        // Copy into the arena only on the path that stores the name as a node.
        let name = parser.alloc_str_in(parser.current_value());
        parser.advance()?;
        parser.skip_whitespace()?;
        Some(name)
    } else {
        None
    };

    // Now parse the condition (same grammar as @supports).
    let (condition, cond_span) = parse_condition_query(parser, ConditionReader::Raw)?;

    // The prelude span keeps the pre-name `start` and takes the condition's end,
    // so a named `@container foo (…)` covers the name while an unnamed one matches
    // `parse_condition_query` exactly.
    let span = Span {
        start,
        end: cond_span.end,
    };

    Ok((container_name, condition, span))
}

/// Parse one `@scope` clause — `(<forgiving-selector-list>)` — with the in-paren
/// leading/trailing gap comments registered (the printer re-emits them from the AST via
/// `comments_to_emit_in_range`, the same wrapping `:is()` args use). Assumes the current token
/// is `(`. The list is **forgiving** (css-cascade-6 makes `<scope-start>`/`<scope-end>`
/// `<forgiving-selector-list>`s, the same production `:is()`/`:where()` use), so an empty
/// or invalid list — `@scope ()`, `@scope (.a, , .b)`, `@scope (.)` — parses (each is
/// kept verbatim), matching parseCss (which captures the prelude raw) and prettier.
/// `what` names the clause for the unterminated-`)` error.
fn parse_scope_clause<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    what: &str,
) -> Result<ScopeClause<'arena>, ParseError> {
    let paren_start = parser.span_pos(parser.current_start);
    parser.advance()?; // consume '('
    parser.skip_whitespace_registering_comments()?; // leading comment
    let list = parse_forgiving_selector_list(parser)?;
    parser.skip_whitespace_registering_comments()?; // trailing comment
    if !parser.check(TokenKind::RightParen) {
        return Err(parser.error_expected_after("')'", what));
    }
    let paren_end = parser.span_pos(parser.current_end);
    parser.advance()?; // consume ')'
    Ok(ScopeClause {
        list,
        paren: Span {
            start: paren_start,
            end: paren_end,
        },
    })
}

/// Parse an @scope prelude into a `PreludeValue::Selectors`.
///
/// CSS Syntax (css-cascade-6): `@scope [(<scope-start>)]? [to (<scope-end>)]?` —
/// **both clauses are independently optional**, so all four combinations are valid
/// (parseCss accepts each): a bare `@scope { … }`, root-only, limit-only, and both.
///
/// Examples:
/// - `` (empty) - bare `@scope { … }`, scopes to the enclosing context
/// - `(.card)` - scope root only
/// - `(.card) to (.footer)` - scope root and limit
/// - `to (.footer)` - scope limit only
/// - `(article > header)` - with combinator
///
/// The span covers the authored prelude (first clause start to last `)`); when both
/// clauses are absent it is a zero-width span at the cursor, so the public AST's
/// `prelude` string extracts to `""` (matching parseCss).
pub(super) fn parse_scope_prelude<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<PreludeValue<'arena>, ParseError> {
    // Leading gap comments (`@scope /* c */ …`) register here: the shared at-rule
    // name skip in `parse_atrule` is a plain skip that stops at a comment, so a leading
    // comment is the current token on entry. Capturing `start` *after* this keeps it out
    // of the wire prelude (extracted from `span`), matching parseCss, which drops it.
    parser.skip_whitespace_registering_comments()?;
    let start = parser.span_pos(parser.current_start);
    // Widens to each clause's closing `)`; stays at `start` when no clause is present.
    let mut end = start;

    // Optional root clause `(<scope-start>)`. After it, the between-clause (`) /* c */ to`)
    // and pre-`{` gaps register their comments so the printer can re-emit them.
    let root = if parser.check(TokenKind::LeftParen) {
        let clause = parse_scope_clause(parser, "@scope root selectors")?;
        end = clause.paren.end;
        parser.skip_whitespace_registering_comments()?; // between-clause / pre-`{` comment
        Some(clause)
    } else {
        None
    };

    // Optional limit clause `to (<scope-end>)` — valid with or without a root. `to` is a
    // case-insensitive grammar keyword (lowercased at the printer's ` to ` literal); its
    // span lets the printer tell a between-clause comment (before `to`) from an after-`to`
    // one. The after-`to` and pre-`{` gaps register comments the same way.
    let limit = if parser.check(TokenKind::Identifier)
        && parser.current_identifier().eq_ignore_ascii_case("to")
    {
        let to_span = Span {
            start: parser.span_pos(parser.current_start),
            end: parser.span_pos(parser.current_end),
        };
        parser.advance()?; // consume "to"
        parser.skip_whitespace_registering_comments()?; // after-`to` comment
        if !parser.check(TokenKind::LeftParen) {
            return Err(parser.error_expected_after("'('", "'to' in @scope prelude"));
        }
        let clause = parse_scope_clause(parser, "@scope limit selectors")?;
        end = clause.paren.end;
        parser.skip_whitespace_registering_comments()?; // pre-`{` comment
        Some(ScopeLimit { to_span, clause })
    } else {
        None
    };

    Ok(PreludeValue::Selectors {
        root,
        limit,
        span: Span { start, end },
    })
}

/// Parse @import prelude into structured values
///
/// CSS Syntax: `@import [ <url> | <string> ] [ layer | layer(<layer-name>) ]? <import-conditions> ;`
///
/// Examples:
/// - `url('styles.css')`
/// - `'styles.css'`
/// - `url('tabs.css') layer(framework)`
/// - `url('override.css') layer`
/// - `url('narrow.css') supports(display: flex) screen`
/// - `url('a.css') screen and (min-width: 5px)` (media-type-led query)
/// - `url('b.css') (max-width: 40px)` (bare `<media-condition>` query)
pub(super) fn parse_import_prelude<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<PreludeValue<'arena>, ParseError> {
    // Raw offset of the prelude's first token, for the raw fallback below.
    let prelude_start_raw = parser.current_start;
    let start = parser.span_pos(parser.current_start);
    let mut values = parser.bvec();

    // Register a leading comment between `@import` and the url()/string (e.g.
    // `@import /* c */ url(...)`). Svelte strips it from the prelude; the printer
    // reconstructs it from `self.comments`.
    parser.skip_whitespace_registering_comments()?;

    // Parse first value: url() function or bare string
    if is_function_token(parser) {
        // url() function
        values.push(parse_function_value(parser)?);
    } else if let TokenKind::String { .. } = &parser.current_kind {
        // Bare string — the inner text is recovered verbatim from `span` at print time
        // (span-for-verbatim, zero alloc); the quote char from `source[span.start]`.
        let value_start = parser.span_pos(parser.current_start);
        let value_end = parser.span_pos(parser.current_end);
        values.push(CssValue::String {
            content: StringCooked::Verbatim,
            span: Span {
                start: value_start,
                end: value_end,
            },
        });
        parser.advance()?;
    } else if matches!(parser.current_kind, TokenKind::Url)
        && !url_token_has_unclosed_paren(&parser.source()[parser.current_start..parser.current_end])
    {
        // Unquoted `url(...)` — the lexer consumed it as one opaque `<url-token>`. Mirror
        // `parse_function_value`'s empty-args url shape (name + span): the printer and the
        // public-AST conversion reconstruct the verbatim, inner-ws-trimmed `url(...)` from
        // the function span, so structured `@import url(…) layer/supports/media` wrapping
        // still works (unlike a raw fallback, which would drop that structure).
        //
        // A url-token with a *nested* `(` (e.g. `url(a(b))`) is excluded above: the lexer
        // stops the url scan at the first unescaped `)` (css-syntax §4.3.6), truncating the
        // token to `url(a(b)` and leaving a dangling `)`, so the structured split would
        // reject at the trailing `)`. parseCss reads such a prelude raw to `;` (and prettier
        // prints it verbatim), so fall through to the raw path below — the same one
        // `@namespace url(a(b))` already takes.
        let value_start = parser.span_pos(parser.current_start);
        let value_end = parser.span_pos(parser.current_end);
        // The name is the token text up to its `(` — the ident that opened the url-token,
        // so the `(` is the first one in it. The printer reads this span only as a
        // url-detection key (`function_name_is(.., "url")`, which decodes an escaped
        // spelling); the emitted text and the public AST both come from `span`, so real
        // casing and content are preserved regardless.
        let name_len = parser
            .current_value()
            .bytes()
            .position(|b| b == b'(')
            .unwrap_or(0);
        // A url-token exists only because the lexer read an ident and then a `(`
        // (`read_identifier`), so the `(` is always there; the `unwrap_or` degrades to an
        // empty name span, which simply fails the url test and prints the span verbatim.
        debug_assert!(
            name_len > 0,
            "a url-token with no `(`: {:?}",
            parser.current_value()
        );
        values.push(CssValue::Function {
            name_span: Span {
                start: value_start,
                end: value_start + name_len as u32,
            },
            args: &[],
            span: Span {
                start: value_start,
                end: value_end,
            },
        });
        parser.advance()?;
    } else {
        // Not a `<url>`/`<string>` first value, so this isn't a structurable @import.
        // Per CSS Syntax 3 the prelude is still consumed as component values (an invalid
        // `@import` is dropped at cascade, not a parse error); parseCss stores it raw and
        // prettier prints it verbatim. Reconsume the whole prelude — including the empty
        // `@import;` case (nothing to consume → a zero-width raw prelude → `""`).
        return super::reconsume_prelude_as_raw(parser, prelude_start_raw);
    }

    parser.skip_whitespace_registering_comments()?;

    // Parse optional layer(), supports() functions and other conditions
    while !parser.check(TokenKind::Semicolon) && !parser.check(TokenKind::Eof) {
        if is_function_token(parser) {
            // layer() or supports() function
            values.push(parse_function_value(parser)?);
            parser.skip_whitespace_registering_comments()?;
        } else if parser.check(TokenKind::Identifier) && parser.current_identifier() == "layer" {
            // Bare "layer" keyword (without function call); text recovered from
            // `span` at print time (span-for-verbatim).
            let value_start = parser.span_pos(parser.current_start);
            let value_end = parser.span_pos(parser.current_end);
            values.push(CssValue::Identifier {
                span: Span {
                    start: value_start,
                    end: value_end,
                },
            });
            parser.advance()?;
            parser.skip_whitespace_registering_comments()?;
        } else if parser.check(TokenKind::Identifier)
            || parser.check(TokenKind::LeftParen)
            || parser.check(TokenKind::Comma)
        {
            // Media-query-list — the last prelude component (css-cascade-5
            // §import-conditions). Consume the rest verbatim to `;`/EOF, preserving
            // original whitespace; the text is recovered from `span` at print time.
            // A query may lead with a media type (`screen and (…)`, an identifier) OR a
            // bare `<media-condition>` (`(max-width: 40px)`, `(width < 100px)`, a `(`) —
            // Media Queries 4 §media-query makes a lone `<media-condition>` a valid query,
            // so both starts are accepted. A leading `,` starts the list too: mediaqueries-4
            // §Syntax parses a `<media-query-list>` by parsing "a comma-separated list of
            // component values, then parsing each entry as a `<media-query>`", so an entry
            // that matches no `<media-query>` — an empty one included — is a grammar
            // mismatch that §"Error Handling" replaces with `not all`, never a parse error.
            // Rejecting it here made the structured `@import` reader stricter than the raw
            // `@media` one on the identical list (and stricter than `parseCss`, which keeps
            // the whole prelude raw). Any other leading token (e.g. a stray `)`) is not a
            // media-query start and falls through to the reject below.
            let media_local_start = parser.current_start;
            let media_start = parser.span_pos(media_local_start);
            let mut media_local_end = parser.current_end;

            while !parser.check(TokenKind::Semicolon) && !parser.check(TokenKind::Eof) {
                if !parser.check(TokenKind::Whitespace) {
                    media_local_end = parser.current_end;
                }
                parser.advance()?;
            }

            let media_end = parser.span_pos(media_local_end);

            // Media-query text recovered verbatim from `span` at print time.
            if media_local_end > media_local_start {
                values.push(CssValue::Identifier {
                    span: Span {
                        start: media_start,
                        end: media_end,
                    },
                });
            }
            break;
        } else {
            // Not a media-query start (e.g. a stray `)`); leave it for the caller to
            // reject as an unterminated at-rule prelude.
            break;
        }
    }

    let end = values.last().map_or(start, |v| v.span().end);

    Ok(PreludeValue::Values {
        values: values.into_bump_slice(),
        span: Span { start, end },
    })
}

/// Whether an unquoted `<url-token>`'s text has an unclosed `(` — i.e. the lexer stopped
/// the url scan at the first unescaped `)` (css-syntax §4.3.6) *inside* a nested group, so
/// the token is a truncated `url(a(b)` rather than a balanced `url(...)`. Escape-aware: a
/// `\(` / `\)` is literal url content, not a paren delimiter (matching the lexer's own
/// scan), so `url(a\(b)` (balanced) and `url(a\)b)` (escaped close) both read as closed.
fn url_token_has_unclosed_paren(text: &str) -> bool {
    let mut depth: u32 = 0;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next(); // the escaped code point is content, never a delimiter
            }
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth > 0
}

/// Consume a run of whitespace and comments, returning the run's comments as one
/// outer-trimmed span (`None` when it held none, which is the overwhelmingly common case
/// and leaves this a plain `skip_whitespace`).
///
/// The span is handed to a `CssValue::Identifier`, whose printer runs the comment-aware
/// `normalize_css_whitespace` over it — so a two-comment run comes back single-spaced like
/// every other CSS comment run. The comments are deliberately **not** registered (see
/// [`CssParser::skip_boundary_whitespace_and_comments`]): the span re-emits them verbatim, and
/// registering would additionally offer them to the `@import` prelude's gap emitters, which
/// print between the parsed values — outside this function, where they would print twice.
fn take_comment_run(parser: &mut CssParser<'_, '_>) -> Result<Option<Span>, ParseError> {
    let mut start = None;
    let mut end = 0u32;
    loop {
        if parser.check(TokenKind::Whitespace) {
            parser.advance()?;
        } else if parser.check(TokenKind::Comment) {
            start.get_or_insert_with(|| parser.span_pos(parser.current_start));
            end = parser.span_pos(parser.current_end);
            parser.advance()?;
        } else {
            break;
        }
    }
    Ok(start.map(|start| Span { start, end }))
}

/// Consume a function's argument list up to — but not including — its **matching** `)`,
/// returning the arguments' outer-trimmed span (`None` when the region held nothing but
/// whitespace).
///
/// Per CSS Syntax 3 §"consume a function" the contents are a component-value list, so a
/// nested `(…)`/`fn(…)` must not end the list at its first inner `)` — hence the depth
/// count. Whitespace is trimmed off both ends; a **comment** is not, because it is content
/// this region re-emits rather than a separator: §4.3.2 `consume comments` returns nothing,
/// which makes a comment transparent to the *grammar* while it still occupies the text.
///
/// Those comments are deliberately left **unregistered** (see
/// [`CssParser::skip_boundary_whitespace_and_comments`], whose doc states the rule): the span
/// re-emits them verbatim, and registering would additionally offer them to the `@import`
/// prelude's gap emitters, which print *between* the parsed values — outside this
/// function, where they would print a second time.
///
/// The single definition of how far a prelude function's arguments reach — the `layer()`
/// arm reads the span, the `<general-enclosed>` arm only needs the seek.
fn consume_function_args(parser: &mut CssParser<'_, '_>) -> Result<Option<Span>, ParseError> {
    let mut start = None;
    let mut end = 0u32;
    let mut depth: u32 = 0;
    while !parser.check(TokenKind::Eof) {
        match parser.current_kind {
            TokenKind::RightParen if depth == 0 => break,
            TokenKind::LeftParen => depth += 1,
            TokenKind::RightParen => depth -= 1,
            _ => {}
        }
        if !parser.check(TokenKind::Whitespace) {
            start.get_or_insert_with(|| parser.span_pos(parser.current_start));
            end = parser.span_pos(parser.current_end);
        }
        parser.advance()?;
    }
    Ok(start.map(|start| Span { start, end }))
}

/// Parse a function value (e.g., url(), layer(), supports())
fn parse_function_value<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<CssValue<'arena>, ParseError> {
    let value_start = parser.span_pos(parser.current_start);

    // Get function name (current token should be identifier). Kept verbatim from
    // source so the author's case survives to output; every *recognition* test below
    // is ASCII case-insensitive instead ("like keywords, function names are ASCII
    // case-insensitive" — css-values-4 §"Functional Notations"), so `SUPPORTS(a:b)`
    // and `LAYER( a )` normalize like their lowercase spellings rather than falling
    // through to the opaque unknown-function path.
    let name = if parser.check(TokenKind::Identifier) {
        parser.current_identifier_in_arena()
    } else {
        return Err(parser.error_expected("function name"));
    };
    // Recognition below is on the DECODED `name` (`\6c ayer(` is a `layer()`), but what
    // reaches the output is the author's own bytes: the value subtree is source-faithful,
    // so the stored name is this span, never the decoded text (prettier preserves the
    // escape too).
    let name_span = Span {
        start: parser.span_pos(parser.current_start),
        end: parser.span_pos(parser.current_end),
    };
    // `supports()` keeps a text name (its printer prints the condition, not a span), so
    // the same fact reaches it as the verbatim token slice — the copy is an `@import`
    // prelude's alone.
    let name_verbatim = parser.alloc_str_in(parser.current_value());

    parser.advance()?; // consume function name

    // Expect '('
    if !parser.check(TokenKind::LeftParen) {
        return Err(parser.error_expected_after("'('", "function name"));
    }

    // `supports()` is read *before* the `(` is consumed: that paren is the condition
    // part's own opening paren (see `parse_supports_function_condition`), which is what
    // lets the bare `<declaration>` alternative reuse the parenthesized-part grammar.
    if name.eq_ignore_ascii_case("supports") {
        let Some((condition, condition_span)) = parse_supports_function_condition(parser)? else {
            return Err(parser.error_expected("')' to close function"));
        };
        return Ok(CssValue::SupportsCondition {
            name: name_verbatim,
            condition,
            span: Span {
                start: value_start,
                end: condition_span.end,
            },
        });
    }

    parser.advance()?; // consume '('

    // For @import functions (url, layer, supports), parse arguments based on function type
    let mut args = parser.bvec();

    if name.eq_ignore_ascii_case("url") {
        // url() - parse the URL argument (string or bare URL)
        parser.skip_whitespace()?;
        if let TokenKind::String { .. } = &parser.current_kind {
            let arg_start = parser.span_pos(parser.current_start);
            let arg_end = parser.span_pos(parser.current_end);
            // Bare string arg — inner text recovered verbatim from `span` at print
            // time (span-for-verbatim, zero alloc); quote char from `source[span.start]`.
            args.push(CssValue::String {
                content: StringCooked::Verbatim,
                span: Span {
                    start: arg_start,
                    end: arg_end,
                },
            });
            parser.advance()?;
            // A comment may TRAIL the quoted argument (CSS Syntax 3 §4.3.2: a comment
            // yields no token, so it is transparent to the grammar). It is carried as its
            // own region rather than skipped, which keeps the argument on the structured
            // string path — and with it the quote normalization (`"a.css"` → `'a.css'`)
            // that the opaque bare-URL path would have lost. Before this it reached no arm
            // at all: it fell past the argument into the `)` check below and rejected the
            // whole stylesheet.
            //
            // Only the trailing side needs this. A *leading* comment never arrives here:
            // `consume_url_token` classifies `url(` as a function-token only when a QUOTE
            // follows (past whitespace — a comment is not whitespace, css-syntax §4.3.4),
            // so a leading comment makes the whole thing an opaque `<url-token>` whose
            // contents this function never sees.
            args.extend(take_comment_run(parser)?.map(|span| CssValue::Identifier { span }));
        } else {
            // Unquoted bare URL (`url(a.css)`, `url(a.css?x=1)`): an opaque token run
            // up to ')'. Leave args empty — both the public-AST conversion and the
            // printer reconstruct the `url(...)` verbatim from the function span.
            while !parser.check(TokenKind::RightParen) && !parser.check(TokenKind::Eof) {
                parser.advance()?;
            }
        }
    } else if name.eq_ignore_ascii_case("layer") {
        // `layer(<layer-name>)` — css-cascade-5 §layer-names spells the argument
        // `<ident> [ '.' <ident> ]*`, so the DOTTED form is the primary one and a single
        // identifier token cannot bound it. Reading one and stopping made every other
        // spelling a parse ERROR — `layer(a.b)` among them — rejecting the whole stylesheet
        // where `parseCss` and prettier both accept.
        //
        // The argument is therefore one region (`consume_function_args`, which owns the
        // reach rule), carried as a single `Identifier`. That routes it through
        // `build_identifier_doc`'s comment-aware `normalize_css_whitespace`, which is where
        // `layer(  a.b  )` → `layer(a.b)` comes from and which spaces a glued `a./* c */b`
        // to `a. /* c */ b` — prettier's value-parse answer, so this whole family matches.
        // A whitespace-only argument (`layer()`) yields `None`, leaving `args` empty and
        // routing the printer to its verbatim function span, the form this arm already
        // produced.
        args.extend(consume_function_args(parser)?.map(|span| CssValue::Identifier { span }));
    } else {
        // Other unknown functions (e.g. `scope((.a) to (.b))`, css-cascade-6 scoped
        // `@import`) — consume the args opaquely. Args stay empty: the printer and the
        // public-AST conversion both reconstruct the function verbatim from its span, so
        // the region's bounds are needed only to find the closing `)`.
        consume_function_args(parser)?;
    }

    if !parser.check(TokenKind::RightParen) {
        return Err(parser.error_expected("')' to close function"));
    }

    let value_end = parser.span_pos(parser.current_end);
    parser.advance()?; // consume ')'

    Ok(CssValue::Function {
        name_span,
        args: args.into_bump_slice(),
        span: Span {
            start: value_start,
            end: value_end,
        },
    })
}
