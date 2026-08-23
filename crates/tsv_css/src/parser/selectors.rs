use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::{Lexer, TokenKind};
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{ParseError, Span};

/// Register comment(s) sitting between a complex selector and its `,` separator
/// (comments are inter-token trivia removed at tokenization — no token, not even
/// whitespace — per css-syntax-3) — but only when a comma
/// actually follows, so a trailing comment before `{` is left for `parse_rule` (it
/// sits outside the list span and is inline-printed as a pre-brace comment). The
/// lookahead is non-destructive.
///
/// The comments are **registered** (not dropped) so the printer can interleave them
/// at the comma boundary via `comments_to_emit_in_range` — the principled replacement for the
/// old raw-source selector seam. Not needed by `parse_forgiving_selector_list`, whose
/// terminator is `)` (not `{`); it registers comments unconditionally before its comma check.
fn skip_comments_before_comma(parser: &mut CssParser<'_, '_>) -> Result<(), ParseError> {
    // Both halves are boundary-aware, and for the same reason: `read_selector_list` reaches
    // its `,` through one `allow_comment_or_whitespace`, so a `<NBSP>` between the comment
    // and the comma is trivia. A plain lookahead saw an identifier there and declined, and a
    // plain skip then left the run standing where the `,` was due — `a /* c */<NBSP>, b`
    // rejected on input canonical accepts.
    if matches!(&parser.current_kind, TokenKind::Comment)
        && parser.peek_past_boundary_whitespace()? == TokenKind::Comma
    {
        parser.skip_boundary_whitespace_registering_comments()?;
    }
    Ok(())
}

/// Parse a comma-separated selector list, applying `parse_item` to every selector — the
/// first, and each one after a comma. Within a list the first item and the tail items
/// parse identically, so the entire list body is shared here. `parse_item` is passed as a
/// fn pointer (`parse_complex_selector`, `fn(&mut CssParser) -> Result<ComplexSelector,
/// ParseError>`) rather than a closure — the complex and relative lists share one per-item
/// parser now that a leading combinator is accepted everywhere (see
/// `parse_complex_selector`).
///
/// Deliberately NOT shared with `parse_forgiving_selector_list`, whose loop has different
/// semantics — it wraps parse errors as `Invalid` selectors, terminates on `)` rather than
/// `{`, registers comments unconditionally, and skips no `<!-- -->` markers — so folding it
/// in would distort this helper rather than dedup it.
fn parse_selector_list_with<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    parse_item: fn(&mut CssParser<'_, 'arena>) -> Result<ComplexSelector<'arena>, ParseError>,
) -> Result<SelectorList<'arena>, ParseError> {
    // `read_selector_list` opens with `allow_comment_or_whitespace` and takes its `start`
    // only after it, so the list opens on the NAME. `parse_complex_selector` skips the same
    // run a moment later, which is why every name already agrees — but the list's own
    // `start` is captured HERE, and a run still standing at this point moves it (and, inside
    // a pseudo-class argument, the `args.start` the wire reads off it). At the stylesheet
    // level the run is long gone (`skip_html_comment_markers` ran in the body loop); in a
    // pseudo-class argument nothing has stepped it yet, so this skip is load-bearing.
    parser.skip_boundary_whitespace()?;
    let start = parser.span_pos(parser.current_start());
    let mut selectors = parser.bvec();

    // Parse the first selector; a selector list always has at least one.
    let first = parse_item(parser)?;
    let mut end = first.span.end;
    selectors.push(first);

    // Parse each additional selector after its comma.
    loop {
        // Selector-list boundaries (`read_selector_list`'s `allow_comment_or_whitespace`):
        // legacy `<!-- ... -->` markers are allowed after a complete selector (before
        // `{`/`,`) and after a comma — but NOT between compounds, so `h1 <!-- --> p`
        // still rejects (parse_combinator already stops at `<`, leaving the marker here).
        parser.skip_html_comment_markers()?;
        skip_comments_before_comma(parser)?;
        if !parser.check(TokenKind::Comma) {
            break;
        }
        parser.advance()?; // consume comma
        // `read_selector_list`'s post-`,` `allow_comment_or_whitespace`; boundary-aware so a
        // run behind a registered comment is stepped too (the `skip_html_comment_markers`
        // below steps only one that leads).
        parser.skip_boundary_whitespace_registering_comments()?;
        parser.skip_html_comment_markers()?;
        let sel = parse_item(parser)?;
        end = sel.span.end;
        selectors.push(sel);
    }

    Ok(SelectorList {
        selectors: selectors.into_bump_slice(),
        span: Span { start, end },
    })
}

/// Parse a complex selector list: `div, span > a, .class#id`
///
/// This parses a comma-separated list of complex selectors.
/// Complex selectors can contain combinators (>, +, ~, descendant); tsv also accepts one
/// *leading* the selector (spec-invalid outside a relative context, but parseCss accepts
/// it — see `parse_complex_selector`).
///
/// Used in:
/// - Top-level CSS rules: `div, span { }`
/// - :is(), :where(), :not() pseudo-classes
///
/// See: CSS Selectors Level 4 - `<complex-selector-list>` vs `<relative-selector-list>`
pub(crate) fn parse_complex_selector_list<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<SelectorList<'arena>, ParseError> {
    parse_selector_list_with(parser, parse_complex_selector)
}

/// Parse a forgiving complex selector list: `div, >>>invalid, span`
///
/// This is used for :is() and :where() pseudo-classes, which have "forgiving" parsing.
/// Per CSS Selectors Level 4, section 9.1:
///
/// "Any items in a forgiving selector list that are invalid
/// (whether explicitly, by using unknown selectors or syntax,
/// or merely contextually, using known syntax but in an invalid context)
/// must be treated as having zero specificity."
///
/// Algorithm (CSS Selectors Level 4 - "parse as a forgiving selector list"):
/// 1. Parse a list of `<complex-real-selector>`s from input
/// 2. For items that fail to parse: wrap as Invalid selector (preserves source)
/// 3. Keep all selectors (valid and invalid) in AST
/// 4. Return a selector list representing all items
///
/// Examples:
/// - `:is(.a, ., .b)` → [ClassSelector(.a), Invalid("."), ClassSelector(.b)]
/// - `:is(.a, ::before, .b)` → [ClassSelector(.a), PseudoElement(::before), ClassSelector(.b)]
/// - `:is(., [)` → [Invalid("."), Invalid("[")]
///
/// Note: Invalid selectors preserved for formatter output (not deleted).
/// Public AST conversion filters them out for Svelte compatibility.
///
/// See: CSS Selectors Level 4 - `<forgiving-selector-list>`
pub(crate) fn parse_forgiving_selector_list<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<SelectorList<'arena>, ParseError> {
    // The list's own `start`, past the boundary run — see `parse_selector_list_with`, whose
    // capture this mirrors. `:is()`/`:where()` reach here straight from the `(`, so this is
    // the only skip standing between the run and the `args.start` the wire emits.
    parser.skip_boundary_whitespace()?;
    let start = parser.span_pos(parser.current_start());
    let mut selectors = parser.bvec();

    loop {
        let selector_start = parser.base_offset() + parser.current_start();
        let source_start = parser.current_start; // Raw source position for extraction

        // Try to parse a selector
        match parse_complex_selector(parser) {
            Ok(selector) => {
                selectors.push(selector);
            }
            Err(_) => {
                // Parse error - advance past the invalid selector and wrap as Invalid
                // (its raw text is recovered from the span at print time).
                extract_selector_until_comma_or_end(parser, source_start)?;
                let selector_end = parser.base_offset() + parser.current_start();

                let invalid = create_invalid_selector(parser.arena, selector_start, selector_end);
                selectors.push(invalid);
            }
        }

        // Check for comma (more selectors) or end of list. Register comments so the
        // printer can interleave them (forgiving lists carry leading/comma/trailing
        // comments inside `:is()`/`:where()`). Both gaps are one
        // `allow_comment_or_whitespace` to `read_selector_list`, so both step the boundary
        // run behind whatever comments they register — a run left standing before the `,`
        // ended the list early and DROPPED every selector after it.
        parser.skip_boundary_whitespace_registering_comments()?;
        if parser.check(TokenKind::Comma) {
            parser.advance()?; // consume comma
            parser.skip_boundary_whitespace_registering_comments()?;
        } else {
            // End of list (hit right paren or other terminator)
            break;
        }
    }

    // Calculate end position from last selector, or use start if empty
    let end = selectors.last().map_or(start, |s| s.span.end);

    Ok(SelectorList {
        selectors: selectors.into_bump_slice(),
        span: Span { start, end },
    })
}

/// Create an Invalid ComplexSelector from span positions (the raw text is
/// recovered verbatim from `span` at print time).
fn create_invalid_selector<'arena>(
    arena: &'arena Bump,
    start: usize,
    end: usize,
) -> ComplexSelector<'arena> {
    let span = Span {
        start: start as u32,
        end: end as u32,
    };

    let mut simple = BumpVec::new_in(arena);
    simple.push(SimpleSelector::Invalid { span });
    let mut children = BumpVec::new_in(arena);
    children.push(RelativeSelector {
        combinator: None,
        combinator_span: None,
        selectors: simple.into_bump_slice(),
        span,
    });

    ComplexSelector {
        children: children.into_bump_slice(),
        span,
    }
}

/// Extract raw text until we reach the next comma or end of selector list.
///
/// This is used for error recovery in forgiving selector lists.
/// We skip invalid tokens while tracking nesting depth (parens, brackets)
/// to avoid stopping at commas inside nested contexts.
///
/// Returns the raw text from `start_pos` to current position.
///
/// Stops at:
/// - Comma at depth 0 (next selector in list)
/// - Right paren at depth 0 (end of pseudo-class args)
/// - EOF (unexpected but handled)
fn extract_selector_until_comma_or_end<'a>(
    parser: &mut CssParser<'a, '_>,
    start_pos: usize,
) -> Result<&'a str, ParseError> {
    let mut depth = 0; // Track nesting depth for parens/brackets

    loop {
        match &parser.current_kind {
            TokenKind::RightParen if depth == 0 => {
                // End of selector list - don't consume the closing paren
                break;
            }
            TokenKind::Comma if depth == 0 => {
                // Next selector - don't consume the comma
                break;
            }
            TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace => {
                depth += 1;
                parser.advance()?;
            }
            TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace => {
                if depth > 0 {
                    depth -= 1;
                }
                parser.advance()?;
            }
            TokenKind::Eof => {
                // Unexpected EOF - stop here
                return Err(ParseError::unexpected_eof(
                    parser.base_offset() + parser.current_start(),
                ));
            }
            _ => {
                parser.advance()?;
            }
        }
    }

    // Extract raw text from source (from start_pos to current position)
    let end_pos = parser.current_start;
    let raw = &parser.source()[start_pos..end_pos];
    Ok(raw)
}

/// Parse a relative selector list: `:has(> img, + li, div)`
///
/// A thin alias for `parse_complex_selector_list` — kept as the name `:has()` (and
/// nested-rule discovery) reads at its callsite. tsv accepts a leading combinator in
/// **every** selector-list context (see `parse_complex_selector`), so the relative and
/// complex lists no longer differ; the `<<relative-selector-list>>` vs
/// `<<complex-selector-list>>` distinction (a leading combinator is *grammatically*
/// valid only in a relative context) is a static-semantic rule deferred to diagnostics.
///
/// See: CSS Selectors Level 4 - `<relative-selector-list>`
pub(crate) fn parse_relative_selector_list<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<SelectorList<'arena>, ParseError> {
    parse_selector_list_with(parser, parse_complex_selector)
}

/// Parse a complex selector: `div > span + .class`, or one leading with a combinator
/// (`> span`, `+ li`, `~ p`).
///
/// A complex selector's first compound leads with a combinator only in a
/// `<<relative-selector>>` context (`:has(> img)`, a nested rule). tsv nonetheless
/// **accepts** a leading combinator in every context — matching Svelte's `parseCss`,
/// which parses `> span { }` at the top level into a `ComplexSelector` whose first
/// `RelativeSelector` carries the combinator (then leaves the "leading combinator has no
/// anchor here" judgment to its *validator*, a stage tsv doesn't run). Rejecting it is a
/// static-semantic early-error deferred to the diagnostics layer, not a parse concern —
/// the same permissive-parser posture tsv takes elsewhere. prettier likewise formats it.
///
/// Examples:
/// - `> img` / `+ li` / `~ p` - leads with an explicit combinator
/// - `span` - no leading combinator, combinator field is null (NOT descendant!)
/// - `div > span` - combinator in the middle
///
/// See: CSS Selectors Level 4 - `<complex-selector>` vs `<relative-selector>`
pub(crate) fn parse_complex_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<ComplexSelector<'arena>, ParseError> {
    // The list's own start is an `allow_comment_or_whitespace` juncture — before the first
    // selector and after every `,` — and the skip must precede the capture, since this
    // offset becomes the rule's, the prelude's and the selector's `start`.
    parser.skip_boundary_whitespace()?;
    let start = parser.span_pos(parser.current_start());
    let mut children = parser.bvec();

    // The first compound MAY begin with an explicit combinator (`> span`, a nested/`:has()`
    // relative selector or a top-level leading combinator parseCss accepts).
    // parse_explicit_combinator (NOT parse_combinator) returns None for a bare compound,
    // so its combinator field stays null rather than becoming an implicit Descendant.
    //
    // EXCEPT a leading `<an+b>` term in a pseudo-class arg (`:is(+3)`, `:not(+n)`,
    // `:foo(+2n + 1)`): the leading `+`/`-` binds to the coefficient, not a next-sibling
    // combinator. Svelte's `read_selector` tries `REGEX_NTH_OF` before the combinator
    // (gated on `inside_pseudo_class`); `pseudo_arg_terminal_nth` is that exact gate (the
    // same `in_pseudo_args && match_nth_value` the `Nth` reader in `parse_simple_selector`
    // uses), so deferring to the bare compound there lets the `Nth` reader claim the term.
    // The combinator pending on the current compound (kept paired with its span). `None`
    // is a null combinator (a bare compound / the leading anchor).
    let mut pending: Option<(Combinator, Span)> = if !pseudo_arg_terminal_nth(parser)
        && let Some(cs) = parse_explicit_combinator(parser)?
    {
        Some(cs)
    } else {
        None
    };

    loop {
        // A combinator immediately followed by ANOTHER combinator has no compound. tsv
        // PRESERVES it as an empty-compound `RelativeSelector` and moves to the next
        // combinator, keeping every authored combinator (`+ ~ .d` → `[+, []], [~, [.d]]`).
        // This DIVERGES from parseCss, whose `read_selector` never emits an empty relative
        // selector — it drops all but the last combinator of a run — a lossy recovery tsv
        // declines: the dropped combinator is authorship the future diagnostics layer needs
        // to flag the invalid double (in a relative context the collapse even masks it
        // entirely, `:has(+ ~ .d)` → valid `:has(~ .d)`). The leading null-anchor with no
        // compound is still dropped (an RS is emitted only for an actual combinator here).
        // `!pseudo_arg_terminal_nth` keeps a following `<an+b>` term (`:is(> +3)`) out of
        // the run so its `+`/`-` binds to the coefficient, not a next-sibling combinator.
        if let Some((c, c_span)) = pending
            && !pseudo_arg_terminal_nth(parser)
            && let Some(next) = explicit_combinator_kind(parser.current_kind)
        {
            children.push(RelativeSelector {
                combinator: Some(c),
                combinator_span: Some(c_span),
                selectors: &[],
                span: c_span,
            });
            let next_span = Span {
                start: parser.span_pos(parser.current_start()),
                end: parser.span_pos(parser.current_end),
            };
            pending = Some((next, next_span));
            parser.advance()?; // consume the combinator token
            skip_combinator_gap(parser)?;
            continue;
        }

        // Parse the compound with the current (possibly null) combinator. A trailing
        // combinator — one followed by a `{`/`,`/`)` terminator rather than a compound —
        // errors here on the missing compound, which both parsers reject.
        let (combinator, combinator_span) = pending.unzip();
        let child = parse_relative_selector(parser, combinator, combinator_span)?;
        children.push(child);

        // Continue with the next combinator, or stop. The loop ends at `{`/`,`/`)`/EOF or
        // when no further combinator follows; a gap comment is registered inside
        // `parse_combinator` (`div /* c */ p`) and is never itself a terminator — a trailing
        // comment before a stop token is left unconsumed for the caller's pre-brace /
        // pseudo-arg handling.
        if parser.check(TokenKind::LeftBrace)
            || parser.check(TokenKind::Comma)
            || parser.check(TokenKind::RightParen)
            || parser.check(TokenKind::Eof)
        {
            break;
        }
        let Some(cs) = parse_combinator(parser)? else {
            break; // No more combinators, we're done
        };
        pending = Some(cs);
    }

    // The complex selector ends at its last relative selector (always a compound — a
    // combinator run ends in one, else `parse_relative_selector` errored above).
    let children = children.into_bump_slice();
    let end = children.last().map_or(start, |rel| rel.span.end);
    Ok(ComplexSelector {
        children,
        span: Span { start, end },
    })
}

/// Map an explicit combinator token (`>`, `+`, `~`, `||`) to its `Combinator`, or `None`
/// for any other token. The single source of truth for the explicit-combinator set — both
/// combinator parsers and `is_explicit_combinator_kind` route through it, so the
/// token→variant set stays in lockstep (the same discipline `is_selector_start_kind`
/// applies to the selector-start set). Excludes the descendant combinator, which is
/// whitespace rather than a token (`parse_combinator` derives that separately).
fn explicit_combinator_kind(kind: TokenKind) -> Option<Combinator> {
    match kind {
        TokenKind::GreaterThan => Some(Combinator::Child),
        TokenKind::Plus => Some(Combinator::NextSibling),
        TokenKind::Tilde => Some(Combinator::SubsequentSibling),
        TokenKind::ColumnCombinator => Some(Combinator::Column),
        _ => None,
    }
}

/// Parse an EXPLICIT combinator only: `>`, `+`, `~`, `||` (but NOT whitespace/descendant)
/// Returns (combinator type, combinator span) only for explicit combinator symbols.
/// Used for checking leading combinators in relative selectors where descendant is NOT implied.
///
/// This is different from `parse_combinator()` which also returns Descendant for whitespace.
pub(crate) fn parse_explicit_combinator(
    parser: &mut CssParser<'_, '_>,
) -> Result<Option<(Combinator, Span)>, ParseError> {
    // `parseCss` reaches this position through `allow_comment_or_whitespace` + the
    // `allow_whitespace()` inside `read_combinator`, so the whole run — both whitespace
    // classes — belongs to the gap. It has to be stepped over BEFORE `combinator_start` is
    // taken: that offset is both the descendant combinator's `end` and the next compound's
    // start.
    parser.skip_boundary_whitespace()?;
    let combinator_start = parser.span_pos(parser.current_start());

    if let Some(comb) = explicit_combinator_kind(parser.current_kind) {
        let end = parser.span_pos(parser.current_end);
        parser.advance()?; // consume combinator token
        skip_combinator_gap(parser)?;

        Ok(Some((
            comb,
            Span {
                start: combinator_start,
                end,
            },
        )))
    } else {
        Ok(None)
    }
}

/// Parse a combinator: `>`, `+`, `~`, `||`, or whitespace (descendant)
/// Returns (combinator type, combinator span)
pub(crate) fn parse_combinator(
    parser: &mut CssParser<'_, '_>,
) -> Result<Option<(Combinator, Span)>, ParseError> {
    // Capture position before skipping whitespace for descendant combinator
    let whitespace_start = parser.span_pos(parser.current_start());
    // `read_combinator` opens with `allow_whitespace()`, so the gap is the whole boundary
    // run: this is the step the compound break above hands off to. Skipping only the
    // whitespace TOKENS left `combinator_start` sitting on a run the lexer had read as an
    // identifier, so no descendant was recognized at all (`&<NBSP>b` stayed one compound)
    // and, where one was, its `end` stopped short of the name (`a <NBSP>b`).
    parser.skip_boundary_whitespace()?;

    // A comment in the combinator gap is inter-token trivia — no token, not even
    // whitespace (css-syntax-3): register
    // it and continue only when the selector actually continues past it (a selector or
    // explicit combinator follows). A trailing comment before `{`/`,`/`)` is left
    // unconsumed for the caller's pre-brace / pseudo-arg handling. `had_gap_comment`
    // lets a descendant combinator be recognized even when the gap is comment-only.
    let had_gap_comment =
        matches!(&parser.current_kind, TokenKind::Comment) && comment_continues_selector(parser)?;
    if had_gap_comment {
        // Boundary-aware, to agree with the lookahead that just predicted it: the run behind
        // the comment is part of the same `allow_comment_or_whitespace`, and `combinator_start`
        // is captured right below — it is both the descendant combinator's `end` and the next
        // compound's start.
        parser.skip_boundary_whitespace_registering_comments()?;
    }

    let combinator_start = parser.span_pos(parser.current_start());

    // An explicit combinator token wins; otherwise a descendant combinator requires actual
    // whitespace (or a gap comment) between the selectors — an adjacent selector token is
    // part of the same compound (handled by the is_simple_selector_chain loop) and must
    // never fabricate a zero-width combinator.
    let combinator = explicit_combinator_kind(parser.current_kind).or_else(|| {
        if (combinator_start > whitespace_start || had_gap_comment)
            && (is_selector_start(parser) || pseudo_arg_terminal_nth(parser))
        {
            Some(Combinator::Descendant)
        } else {
            None
        }
    });

    let result = if let Some(comb) = combinator {
        let (start, end) = if comb == Combinator::Descendant {
            // Descendant is whitespace - span from end of previous to start of next
            (whitespace_start, combinator_start)
        } else {
            (combinator_start, parser.span_pos(parser.current_end))
        };

        if comb != Combinator::Descendant {
            parser.advance()?; // consume combinator token
            skip_combinator_gap(parser)?;
        }

        Some((comb, Span { start, end }))
    } else {
        None
    };

    Ok(result)
}

/// Skip the trivia after a just-consumed explicit combinator symbol, registering any
/// gap comment that follows it (`div > /* c */ p`, `:has(> /* c */ img)`) so the
/// printer can re-emit it. Shared by `parse_combinator` and `parse_explicit_combinator`.
fn skip_combinator_gap(parser: &mut CssParser<'_, '_>) -> Result<(), ParseError> {
    // The boundary loop, because this is `read_combinator`'s second `allow_whitespace()` and
    // `parseCss` reaches the comment through the same `allow_comment_or_whitespace` as every
    // other juncture: a plain skip leaves a run standing where the comment is due, the comment
    // arm never fires, and `a > <NBSP>/* c */ b` is REJECTED on input canonical accepts. One
    // call does both halves and loops, so a run on either side of the comment is stepped.
    parser.skip_boundary_whitespace_registering_comments()?;
    Ok(())
}

/// Token kinds that can begin a simple selector (`div`, `.c`, `#id`, `*`, `:hover`,
/// `[attr]`, `&`). Shared by `is_selector_start` (current token) and the comment-gap
/// lookaheads (a peeked token, by value).
fn is_selector_start_kind(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier
            | TokenKind::Dot
            | TokenKind::Hash
            | TokenKind::Asterisk
            | TokenKind::Colon
            | TokenKind::LeftBracket
            | TokenKind::Ampersand
    )
}

/// Whether a token is an explicit combinator symbol (`>`, `+`, `~`, `||`). A gap comment
/// followed by one of these still continues the selector (`i /* c */ > em`). Delegates to
/// `explicit_combinator_kind` so the set stays in lockstep with the combinator parsers.
fn is_explicit_combinator_kind(kind: TokenKind) -> bool {
    explicit_combinator_kind(kind).is_some()
}

/// Check if current token could start a selector
fn is_selector_start(parser: &CssParser<'_, '_>) -> bool {
    is_selector_start_kind(parser.current_kind)
}

/// Inside functional pseudo-class args, a bare `<number>`/`<an+b>` term terminated by
/// `,`/`)` is a valid `Nth` simple selector — the same production `parse_simple_selector`
/// recognizes in leading position (`:is(123)`), but in descendant position it must also
/// count as a selector start so the preceding whitespace becomes a `Descendant` combinator
/// (`:is(.a 123)`, `:not(.a 2n + 1)`), matching parseCss. Gated on `in_pseudo_args`; the
/// `match_nth_value` terminator requirement keeps `:is(.a 123 .b)` rejected (parseCss
/// rejects it too — the Nth must be the terminal simple selector).
fn pseudo_arg_terminal_nth(parser: &CssParser<'_, '_>) -> bool {
    parser.in_pseudo_args && match_nth_value(parser.source(), parser.current_start()).is_some()
}

/// After a compound, decide whether a `Comment` at the current position is an
/// inter-token gap comment — a selector or explicit combinator follows past it, so the
/// complex selector continues (`div /* c */ p`, `i /* c */ > em`) — or a trailing
/// comment before `{`/`,`/`)` that the caller captures (a rule's pre-brace comment, or
/// a pseudo-arg list's trailing comment). Assumes the current token is a `Comment`; the
/// lookahead is non-destructive.
///
/// The lookahead is the **boundary** one, because the skip it predicts is: the gap is one
/// `allow_comment_or_whitespace` to `parseCss`, so a `<NBSP>` behind the comment is trivia
/// and not the identifier the lexer read. Asking `peek_past_whitespace` here saw a selector
/// start in the run itself and called `a /* c */<NBSP>{` a descendant combinator.
fn comment_continues_selector(parser: &CssParser<'_, '_>) -> Result<bool, ParseError> {
    let after = parser.peek_past_boundary_whitespace()?;
    Ok(is_selector_start_kind(after) || is_explicit_combinator_kind(after))
}

/// Assuming the current token is a `Comment`, scan the directly-glued run of
/// comments (a temp lexer from the comment's end) and report whether a simple
/// selector start follows the whole run with **no** intervening whitespace — i.e.
/// the comment(s) are inter-token trivia *inside one compound* (`.a/* c */.b`,
/// `.a/* c *//* d */.b`), which must stay a compound (a space would tokenize as a
/// descendant `.a .b`). A whitespace anywhere in the run, or a non-selector token
/// after it, ends the compound (the combinator loop then reads the gap). This is
/// the multi-comment generalization of a single `peek_kind` — one lookahead can't
/// see past a *second* glued comment. Non-destructive.
///
/// ⚠️ "Whitespace" here is the **boundary** class, not the `Whitespace` token alone: an
/// identifier whose head is a JS-`\s` run is a run `parseCss` would have skipped before the
/// token started, so it ends the compound exactly as a space does. Reading it as a glued
/// selector start instead kept `.a/* c */<NBSP>{` in the compound and minted a second simple
/// selector out of the run itself. The sibling reading of the same question is
/// `comment_continues_selector`, and the two have to agree.
fn compound_continues_across_comments(parser: &CssParser<'_, '_>) -> Result<bool, ParseError> {
    let remaining = &parser.source()[parser.current_end..];
    let mut lexer = Lexer::at_offset(remaining, parser.base_offset() + parser.current_end);
    loop {
        let token = lexer.next_token()?;
        match token.kind {
            TokenKind::Comment => continue,
            TokenKind::Whitespace => return Ok(false),
            TokenKind::Identifier
                if crate::whitespace::boundary_prefix_len(
                    &remaining[token.start as usize..token.end as usize],
                ) > 0 =>
            {
                return Ok(false);
            }
            kind => return Ok(is_selector_start_kind(kind)),
        }
    }
}

/// Parse a relative selector: combinator + simple selectors
fn parse_relative_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    combinator: Option<Combinator>,
    combinator_span: Option<Span>,
) -> Result<RelativeSelector<'arena>, ParseError> {
    // Ahead of the capture below, which is this compound's `start` when no combinator
    // precedes it — see `skip_boundary_whitespace`.
    parser.skip_boundary_whitespace()?;
    // Start position is either the combinator start (if present) or the current selector start
    let start = combinator_span.map_or_else(
        || parser.base_offset() + parser.current_start(),
        |s| s.start_usize(),
    );
    let mut selectors = parser.bvec();

    // Parse one or more simple selectors
    loop {
        let simple = parse_simple_selector(parser)?;
        selectors.push(simple);

        // A glued comment run (`.a/* c */.b`, `.a/* c *//* d */.b`) between two simple
        // selectors is inter-token trivia that keeps them in one compound — with no
        // surrounding whitespace it is NOT a descendant. Register the whole run and
        // continue only when a simple-selector start follows it glued; a whitespace
        // anywhere in the run (`.a/* c */ .b`) ends the compound and the combinator loop
        // reads it as a descendant.
        if matches!(&parser.current_kind, TokenKind::Comment) {
            if compound_continues_across_comments(parser)? {
                while matches!(&parser.current_kind, TokenKind::Comment) {
                    parser.register_current_comment();
                    parser.advance()?;
                }
                continue;
            }
            break;
        }

        // A boundary whitespace run at the head of the next token IS whitespace to
        // `parseCss` — `read_selector` runs `allow_comment_or_whitespace` after every simple
        // selector — so it ends the compound however the lexer read it. The break is only
        // half the rule and was long held back for the other half: on its own it makes the
        // caller expect a `{` where the selector continues, turning `&<NBSP>b` and `*<NBSP>b`
        // into parse errors. `parse_combinator` supplies the other half — it steps the same
        // run and reports the descendant combinator that replaces the break — so the two
        // must stay together. The run reaches here after every simple selector that is not
        // itself an identifier (`&`, `*`, a `)`, a `]`, a `%`); an identifier's own trailing
        // run was swallowed by `read_identifier` on both sides and never gets this far.
        if parser.boundary_run_len() > 0 {
            break;
        }

        // Check if another simple selector follows (no whitespace, no combinator)
        if !is_simple_selector_chain(parser) {
            break;
        }
    }

    let end = parser.span_pos(parser.current_start());

    if selectors.is_empty() {
        return Err(parser.error_expected_at("selector", start));
    }

    Ok(RelativeSelector {
        combinator,
        combinator_span,
        selectors: selectors.into_bump_slice(),
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Check if another simple selector follows in the chain (e.g., `div.class#id`, `&__a`, `div&`)
///
/// Whitespace is tokenized, so a directly-adjacent `Identifier`/`Asterisk`/`Ampersand` can only
/// appear mid-compound (`&__a`, `div&`, `&&`, `*&`) — a space yields a `Whitespace` token and ends
/// the chain. Type-not-first compounds (`&div`, `a&b`) are grammar-invalid per Selectors 4 but
/// parsed for parity with Svelte's `parseCss` (validity is the future diagnostics layer's job).
///
/// The continuing-token set is exactly `is_selector_start_kind` — a token that can *begin*
/// a simple selector is also one that *continues* a glued compound — so this delegates
/// rather than re-listing the kinds, keeping the two in lockstep.
fn is_simple_selector_chain(parser: &CssParser<'_, '_>) -> bool {
    is_selector_start_kind(parser.current_kind)
}

/// Parse a simple selector: type, class, id, attribute, pseudo-class, pseudo-element
pub(crate) fn parse_simple_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<SimpleSelector<'arena>, ParseError> {
    let start = parser.base_offset() + parser.current_start();

    // Inside functional pseudo-class args, a `<number>`/`<an+b>` term (followed by
    // `,`/`)`, or by ` of S`) is an `Nth` simple selector — checked before the
    // type-selector arm so `:foo(odd)`/`:is(n)` read as `Nth`, not a `TypeSelector`.
    // Mirrors Svelte's `read_selector`, whose `REGEX_NTH_OF` is gated on
    // `inside_pseudo_class` and tried before the combinator (so the `+` in `2n+1` is
    // An+B, not a next-sibling combinator). An `An+B of S` term folds ` of ` into the
    // span (`match_nth_value`) and leaves `S` to parse as ordinary sibling selectors.
    if parser.in_pseudo_args
        && let Some(value_end) = match_nth_value(parser.source(), parser.current_start())
    {
        // Consume the token run spanning the An+B value text (its boundary aligns
        // with a token boundary — the matcher only ends on complete lexical units).
        while parser.current_start() < value_end && !parser.check(TokenKind::Eof) {
            parser.advance()?;
        }
        return Ok(SimpleSelector::Nth {
            span: Span {
                start: start as u32,
                end: parser.span_pos(value_end),
            },
        });
    }

    match &parser.current_kind {
        TokenKind::Identifier => {
            // Type selector: div, span, etc. Could also be a namespace prefix:
            // svg|rect, *|div. Both forms recover their text verbatim from `span` at
            // print time, so nothing is copied into the arena either way.
            if matches!(parser.peek_past_comments()?, TokenKind::Pipe) {
                // Namespace prefix: identifier|element. A comment may split either of the
                // `<wq-name>`'s junctures (`svg/* c */|rect`, `svg|/* c */rect`) — it is
                // inter-token trivia producing no token, where the `<whitespace-token>` a
                // space would produce is forbidden between wq-name components. The
                // comments are registered so `span`'s verbatim emission keeps them glued.
                // The prefix is kept as a SPAN, not a decoded copy — see
                // `SimpleSelector::Type`.
                let namespace_span = Some(Span {
                    start: start as u32,
                    end: parser.span_pos(parser.current_end),
                });
                parser.advance()?; // consume the namespace identifier
                parser.register_and_skip_comments()?;
                parser.advance()?; // consume the pipe
                parser.register_and_skip_comments()?;

                // Must be followed by an identifier (element name)
                if !parser.check(TokenKind::Identifier) {
                    return Err(parser.error_expected_after("element name", "namespace prefix"));
                }
                let end = parser.span_pos(parser.current_end);
                parser.advance()?;

                Ok(SimpleSelector::Type {
                    namespace_span,
                    span: Span {
                        start: start as u32,
                        end,
                    },
                })
            } else {
                // No namespace, just a regular type selector; its text is recovered
                // from `span` at print time, so nothing is copied into the arena.
                parser.advance()?;
                let end = parser.span_pos(parser.current_start());
                Ok(SimpleSelector::Type {
                    namespace_span: None,
                    span: Span {
                        start: start as u32,
                        end,
                    },
                })
            }
        }
        TokenKind::Dot => {
            // Class selector: .class
            parser.advance()?; // consume .
            // selectors-4 forbids white space "between any of the components of a
            // <class-selector>", and a comment is not a `<whitespace-token>` — so a
            // comment may sit here and stays GLUED. Registered so `span`'s verbatim
            // emission keeps it in place.
            parser.register_and_skip_comments()?;
            if !parser.check(TokenKind::Identifier) {
                return Err(parser.error_expected_after("class name", "."));
            }
            // The class name's text is recovered from `span` at print time, so
            // nothing is copied into the arena.
            let end = parser.span_pos(parser.current_end);
            parser.advance()?;
            Ok(SimpleSelector::Class {
                span: Span {
                    start: start as u32,
                    end,
                },
            })
        }
        TokenKind::Hash => {
            // ID selector: #id
            parser.advance()?; // consume #
            if !parser.check(TokenKind::Identifier) {
                return Err(parser.error_expected_after("ID name", "#"));
            }
            // The ID name's text is recovered from `span` at print time, so nothing
            // is copied into the arena.
            let end = parser.span_pos(parser.current_end);
            parser.advance()?;
            Ok(SimpleSelector::Id {
                span: Span {
                    start: start as u32,
                    end,
                },
            })
        }
        TokenKind::Asterisk => {
            // Universal selector: *
            // Could also be universal namespace prefix: *|div
            // The comment lookahead runs BEFORE the `*` is consumed, so a comment that is
            // NOT a wq-name separator (`*/* c */ .b`) is left for the gap emitter.
            let namespaced = matches!(parser.peek_past_comments()?, TokenKind::Pipe);
            let namespace_span = Span {
                start: start as u32,
                end: parser.span_pos(parser.current_end),
            };
            parser.advance()?;
            if namespaced {
                parser.register_and_skip_comments()?;
            }

            // Check for namespace: *|element
            if parser.check(TokenKind::Pipe) {
                parser.advance()?; // consume pipe
                parser.register_and_skip_comments()?;

                // Must be followed by an identifier (element name)
                if !parser.check(TokenKind::Identifier) {
                    return Err(
                        parser.error_expected_after("element name", "universal namespace prefix")
                    );
                }

                // The element name's text is recovered from `span` at print time, so
                // nothing is copied into the arena.
                let end = parser.span_pos(parser.current_end);
                parser.advance()?;

                Ok(SimpleSelector::Type {
                    namespace_span: Some(namespace_span), // Universal namespace
                    span: Span {
                        start: start as u32,
                        end,
                    },
                })
            } else {
                // Just a universal selector (no namespace)
                let end = parser.span_pos(parser.current_start());
                Ok(SimpleSelector::Universal {
                    namespace_span: None,
                    span: Span {
                        start: start as u32,
                        end,
                    },
                })
            }
        }
        TokenKind::Colon => {
            // Pseudo-class or pseudo-element
            super::pseudo::parse_pseudo_selector(parser, start)
        }
        TokenKind::LeftBracket => {
            // Attribute selector: [attr], [attr="value"]
            super::attributes::parse_attribute_selector(parser, start)
        }
        TokenKind::Ampersand => {
            // Nesting selector: &
            let end = parser.span_pos(parser.current_end);
            parser.advance()?;
            Ok(SimpleSelector::Nesting {
                span: Span {
                    start: start as u32,
                    end,
                },
            })
        }
        TokenKind::Percentage => {
            // Percentage selector: 0%, 50%, 100% (used in @keyframes)
            // Extract value without the % suffix
            let value_str = &parser.source()[parser.current_start..parser.current_end - 1];
            let value = value_str.parse::<f64>().map_err(|_| {
                parser.error_msg_at(&format!("Invalid percentage value: {value_str}"), start)
            })?;
            let end = parser.span_pos(parser.current_end);
            parser.advance()?;
            Ok(SimpleSelector::Percentage {
                value,
                span: Span {
                    start: start as u32,
                    end,
                },
            })
        }
        TokenKind::Pipe => {
            // Explicit no-namespace selector: |div
            // This selects elements with no namespace (in contrast to *|div for any namespace).
            // The `<ns-prefix>`'s leading token is absent, so its span is empty at the `|`.
            let namespace_span = Some(Span {
                start: start as u32,
                end: start as u32,
            });
            parser.advance()?; // consume pipe
            parser.register_and_skip_comments()?;

            // Must be followed by an identifier (element name) or asterisk (universal)
            if parser.check(TokenKind::Identifier) {
                // The element name's text is recovered from `span` at print time, so
                // nothing is copied into the arena.
                let end = parser.span_pos(parser.current_end);
                parser.advance()?;

                Ok(SimpleSelector::Type {
                    namespace_span, // empty prefix = explicit no namespace
                    span: Span {
                        start: start as u32,
                        end,
                    },
                })
            } else if parser.check(TokenKind::Asterisk) {
                // |* - universal selector with explicit no namespace
                let end = parser.span_pos(parser.current_end);
                parser.advance()?;

                Ok(SimpleSelector::Universal {
                    namespace_span, // empty prefix = explicit no namespace
                    span: Span {
                        start: start as u32,
                        end,
                    },
                })
            } else {
                Err(parser.error_expected_after("element name or '*'", "no-namespace prefix '|'"))
            }
        }
        _ => Err(parser.error_msg_at(
            &format!("Unexpected token in selector: {}", parser.current_kind),
            start,
        )),
    }
}

/// ASCII whitespace as Svelte's `\s` sees it in An+B: space, tab, LF, CR, FF, and VT
/// (`U+000B`). This is JS `\s` restricted to ASCII — note it includes VT, which CSS
/// whitespace (`is_css_whitespace`) excludes, because the An+B grammar is Svelte's
/// `REGEX_NTH_OF` (a JS regex), not the CSS tokenizer; tsv's selector lexer treats VT
/// as `\s` for the same parity (see the `combinator_control_whitespace` divergence).
/// Multibyte Unicode `\s` (NBSP, …) is out of scope, matching tsv's ASCII-only `\s`.
fn is_anb_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c')
}

/// Advance past An+B whitespace (`\s*`) from `i`, returning the first non-whitespace
/// offset.
fn skip_anb_ws(bytes: &[u8], mut i: usize) -> usize {
    while bytes.get(i).copied().is_some_and(is_anb_ws) {
        i += 1;
    }
    i
}

/// Advance past ASCII digits (`\d*`) from `i`, returning the offset after the run.
fn skip_digits(bytes: &[u8], mut i: usize) -> usize {
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    i
}

/// A CSS name code point that would *continue* an identifier (so `of` glued to it is not
/// the standalone `of` keyword): ASCII alphanumerics, `-`, `_`, and non-ASCII bytes.
/// Used to tell the `of` keyword (`2n of.x`, `2n of )`) from a longer ident (`2n offset`).
fn is_css_name_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80
}

/// Match an `An+B` term inside pseudo-class args at byte offset `pos`, returning the
/// offset just past the term's value text. A port of Svelte's `read_selector` Nth
/// production, `REGEX_NTH_OF`, including both terminator branches:
///
/// - `(?=\s*[,)])` — a bare An+B (`2n`, `odd`): the lookahead consumes nothing, so the
///   value ends at the An+B.
/// - `\s+of\s+` — an `An+B of S` term (`2n of .x`): Svelte folds the ` of ` (and its
///   surrounding whitespace) into the `Nth` value and reads `S` as ordinary sibling
///   selectors — it is NOT a nested selector list here. So the returned end covers
///   `An+B` through the whitespace after `of`, and the caller's selector loop parses
///   `S` next. This matches Svelte even though the dedicated `:nth-*()` path
///   (`parse_nth_args`) deliberately diverges to a nested `Nth.selector` — the `of S`
///   form is spec-defined only for `nth-*`, so there tsv applies its principled
///   nesting, while here (where Svelte merely over-accepts An+B) tsv matches Svelte.
fn match_nth_value(source: &str, pos: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let value_end = match_an_plus_b(bytes, pos, false)?;
    // `(?=\s*[,)])`: optional whitespace then `,`/`)`.
    let after = skip_anb_ws(bytes, value_end);
    if matches!(bytes.get(after), Some(b',' | b')')) {
        return Some(value_end);
    }
    // `\s+of\s+`: at least one whitespace (`after > value_end`), the lowercase `of`
    // keyword (case-sensitive, like Svelte's flagless regex), then at least one
    // whitespace. The value folds through the trailing whitespace run.
    if after > value_end && bytes[after..].starts_with(b"of") {
        let of_end = after + 2;
        let after_of = skip_anb_ws(bytes, of_end);
        if after_of > of_end {
            return Some(after_of);
        }
    }
    None
}

/// The An+B value grammar, without the terminator. Two grammars share this scanner,
/// selected by `spec`:
///
/// - `spec = false` — Svelte's `REGEX_NTH_OF`:
///   `even | odd | \+?(\d+ | \d*n(\s*[+-]\s*\d+)?) | -\d*n(\s*\+\s*\d+)`. Used by
///   `match_nth_value` for the bare-An+B terms Svelte over-accepts inside
///   `:is()`/`:not()`/unknown pseudo-args (no spec basis there — An+B is not a
///   selector, so tsv matches parseCss quirk-for-quirk, including its rejection of
///   `-3`/`-2n`/`-n`).
/// - `spec = true` — the full css-syntax-3 [`<an+b>` microsyntax][anb], which additionally
///   accepts the negative forms `REGEX_NTH_OF` mishandles: a negative `<integer>`
///   (`-3`, `-0`), a negative `<n-dimension>` (`-2n`), bare `-n`, and a negative
///   coefficient with a `-` offset (`-2n-3`, `-n-3`, `-2n - 3`, `-n - 3`). Used for the
///   dedicated `:nth-*()` An+B term, where the spec grammar is the oracle and tsv
///   deliberately diverges from Svelte's buggy reader (`_svelte_divergence`). It also
///   accepts an uppercase `n` *unit* in an `<n-dimension>` (`2N` → normalized to `2n`),
///   but not a bare uppercase `N`/`EVEN`/`ODD` (those have a valid type-selector reading,
///   so they defer to parseCss) — the `nth_case` rule.
///
/// Returns the end offset of the value, or `None` if no An+B starts at `start`.
/// Case-sensitive on the `even`/`odd`/`n` literals, matching both grammars.
///
/// [anb]: https://drafts.csswg.org/css-syntax-3/#the-anb-type
fn match_an_plus_b(bytes: &[u8], start: usize, spec: bool) -> Option<usize> {
    // `even` / `odd` keywords (the terminator check rejects `evens`/`oddball`).
    // Case-sensitive in both grammars: an uppercase `EVEN`/`ODD`/`N` is a valid *type
    // selector*, so it falls through to the selector-list path and reads as
    // `TypeSelector`, matching parseCss (which never enters An+B for an uppercase ident).
    if bytes[start..].starts_with(b"even") {
        return Some(start + 4);
    }
    if bytes[start..].starts_with(b"odd") {
        return Some(start + 3);
    }

    let sign = bytes
        .get(start)
        .copied()
        .filter(|b| matches!(b, b'+' | b'-'));
    let after_sign = start + usize::from(sign.is_some());

    // `\d*` — the `A` coefficient when `n` follows, else the whole integer `B`.
    let after_digits = skip_digits(bytes, after_sign);
    let had_digits = after_digits > after_sign;

    // The `n` unit is case-insensitive only under the spec grammar and only in an
    // `<n-dimension>` (a coefficient precedes it, `2N`): the dimension has no
    // type-selector fallback and prettier canonicalizes the unit to lowercase (the
    // `nth_case` rule). A bare uppercase `N` is left to read as a type selector.
    let is_n = matches!(bytes.get(after_digits), Some(&b'n'))
        || (spec && had_digits && matches!(bytes.get(after_digits), Some(&b'N')));
    if is_n {
        let after_n = after_digits + 1; // `\d*n`
        // Optional `\s*[+-]\s*\d+` tail. Under `REGEX_NTH_OF` a leading `-` requires the
        // tail and permits only `+` (`-\d*n(\s*\+\s*\d+)`); the spec grammar permits a
        // `+`/`-` tail regardless of the leading sign (`-2n-3`, `-n-3`).
        let plus_only = !spec && sign == Some(b'-');
        match match_anb_tail(bytes, after_n, plus_only, spec) {
            Some(end) => Some(end),
            // `-n` / `-2n` alone (leading `-`, no tail) is valid An+B per spec, but not
            // under `REGEX_NTH_OF`.
            None => (spec || sign != Some(b'-')).then_some(after_n),
        }
    } else if had_digits && (spec || sign != Some(b'-')) {
        // `\+?\d+` — a plain integer `B` (no `n`). A leading `-` (`-3`) is a valid
        // `<integer>` per spec, but not permitted by `REGEX_NTH_OF`.
        Some(after_digits)
    } else {
        None
    }
}

/// Advance past An+B whitespace **and `/* */` comments** — inter-token trivia the spec
/// ignores — from `i`, returning the first offset that is neither. Used by the spec
/// (`:nth-*()`) An+B scanner at **every** gap the microsyntax has: the terminator
/// (`nth_arg_is_anb`) and both of the term's own tail gaps (`match_anb_tail`). Comments
/// are trivia per css-syntax-3, so `:nth-child(2n /* c */)` and `:nth-child(2n /* c */ + 1)`
/// are alike spec-valid even though parseCss's comment-blind reader rejects both (the
/// `nth_comment` / `nth_comment_in_term` `_svelte_divergence`s). `REGEX_NTH_OF`'s
/// terminator (`skip_anb_ws`) stays comment-blind, matching parseCss for `:is()`/`:not()`.
fn skip_anb_ws_and_comments(bytes: &[u8], mut i: usize) -> usize {
    loop {
        i = skip_anb_ws(bytes, i);
        if !crate::comments::is_comment_start(bytes, i) {
            return i;
        }
        i = crate::comments::comment_end(bytes, i);
    }
}

/// Decide whether a `:nth-*()` argument starting at source offset `pos` is a clean
/// `<an+b> [of S]?` (the dedicated `Nth` path) rather than an ordinary selector-list
/// argument. The spec grammar (`:nth-child(<An+B> [of <complex-real-selector-list>]?)`,
/// [selectors-4]) accepts *only* a leading An+B term, optionally followed by `of S`;
/// anything else (`:nth-child(.a)`, `:nth-child(even odd)`, `:nth-child(2n, .foo)`) is
/// spec-invalid, and tsv structures it like parseCss for drop-in parity by falling
/// through to `parse_complex_selector_list`. Recognition is comment-tolerant (see
/// `skip_anb_ws_and_comments`) and terminator-gated: the An+B term must be immediately
/// followed by the closing `)` or a ` of ` clause — a trailing `,` (a list) or a bare
/// selector demotes the whole argument to the selector-list path.
///
/// [selectors-4]: https://drafts.csswg.org/selectors-4/#the-nth-child-pseudo
pub(crate) fn nth_arg_is_anb(source: &str, pos: usize) -> bool {
    let bytes = source.as_bytes();
    let Some(value_end) = match_an_plus_b(bytes, pos, true) else {
        return false;
    };
    let after = skip_anb_ws_and_comments(bytes, value_end);
    match bytes.get(after) {
        // A bare `<an+b>` terminated by the closing paren.
        Some(b')') => true,
        // `<an+b> of S`: whitespace/comments before `of` (so `after > value_end`) then
        // the standalone `of` keyword (not a longer ident like `offset`). `S` may be
        // glued to `of` (`2n of.x`) — spec-valid, since whitespace between the `of` and
        // `S` tokens is optional — so this does not require trailing whitespace (that
        // is parseCss's comment-blind `\s+of\s+` bug; tsv diverges per spec).
        _ => {
            after > value_end
                && bytes[after..].starts_with(b"of")
                && !bytes
                    .get(after + 2)
                    .copied()
                    .is_some_and(is_css_name_continue)
        }
    }
}

/// Match the `\s*[+-]\s*\d+` An+B tail at `pos`. When `plus_only` (the leading-`-`
/// branch) only `+` is accepted. Returns the end offset, or `None` if no tail is present.
///
/// The tail's two gaps are the only positions *inside* an An+B term where trivia can sit
/// (every other juncture is within one token), so `spec` decides them the same way it
/// decides the terminator: under the spec grammar a comment is trivia and the term matches
/// through it (`2n /* c */ + 1`), while `REGEX_NTH_OF` stays comment-blind for parseCss
/// parity. Asking the two gaps a different question than the terminator is what made a
/// split term reject — or, where the term had a type-selector reading, mis-parse.
fn match_anb_tail(bytes: &[u8], pos: usize, plus_only: bool, spec: bool) -> Option<usize> {
    let skip_trivia: fn(&[u8], usize) -> usize = if spec {
        skip_anb_ws_and_comments
    } else {
        skip_anb_ws
    };
    let op_pos = skip_trivia(bytes, pos);
    let op = *bytes.get(op_pos)?;
    if op != b'+' && (plus_only || op != b'-') {
        return None;
    }
    let digits_start = skip_trivia(bytes, op_pos + 1);
    let end = skip_digits(bytes, digits_start);
    (end > digits_start).then_some(end)
}

#[cfg(test)]
mod tests {
    use super::{
        Combinator, TokenKind, explicit_combinator_kind, is_explicit_combinator_kind,
        match_nth_value, nth_arg_is_anb,
    };

    /// `explicit_combinator_kind` is the single source of truth for the explicit-combinator
    /// token set (`>`, `+`, `~`, `||`) that both combinator parsers and
    /// `is_explicit_combinator_kind` route through, so this pins the full mapping — the rare
    /// `||`/`ColumnCombinator` → `Column` arm included — and asserts the boolean predicate
    /// stays exactly its `Some`/`None` projection.
    #[test]
    fn explicit_combinator_kind_maps_the_four_symbols() {
        assert_eq!(
            explicit_combinator_kind(TokenKind::GreaterThan),
            Some(Combinator::Child)
        );
        assert_eq!(
            explicit_combinator_kind(TokenKind::Plus),
            Some(Combinator::NextSibling)
        );
        assert_eq!(
            explicit_combinator_kind(TokenKind::Tilde),
            Some(Combinator::SubsequentSibling)
        );
        assert_eq!(
            explicit_combinator_kind(TokenKind::ColumnCombinator),
            Some(Combinator::Column)
        );

        // Not explicit combinators — including the whitespace-derived descendant, which is
        // never a token and so is deliberately absent from the mapping.
        for kind in [
            TokenKind::Identifier,
            TokenKind::Dot,
            TokenKind::Comma,
            TokenKind::Whitespace,
            TokenKind::LeftBrace,
        ] {
            assert_eq!(explicit_combinator_kind(kind), None);
        }

        // `is_explicit_combinator_kind` is exactly the `Some`/`None` projection.
        for kind in [
            TokenKind::GreaterThan,
            TokenKind::Plus,
            TokenKind::Tilde,
            TokenKind::ColumnCombinator,
            TokenKind::Identifier,
            TokenKind::Comma,
        ] {
            assert_eq!(
                is_explicit_combinator_kind(kind),
                explicit_combinator_kind(kind).is_some()
            );
        }
    }

    /// `nth_arg_is_anb` decides whether a `:nth-*()` argument takes the dedicated `Nth`
    /// path (a clean spec `<an+b> [of S]?`, comment-tolerant) or falls through to the
    /// selector-list path. Each input is the argument text starting after the `(`,
    /// terminated by the `)` (or a ` of ` clause). Broader than `REGEX_NTH_OF`: it
    /// accepts the spec's negative forms and an uppercase `n` unit, and treats `of`
    /// glued to `S` as valid.
    #[test]
    fn nth_arg_is_anb_spec_grammar() {
        // Clean `<an+b> [of S]?` — the `Nth` path.
        for input in [
            "2n)",
            "odd)",
            "even)",
            "0)",
            "-3)", // negative <integer> (spec, not REGEX_NTH_OF)
            "-0)",
            "-2n)",     // negative <n-dimension>
            "-n)",      // bare -n
            "-2n-3)",   // <ndashdigit-dimension>, negative
            "-n-3)",    // <dashndashdigit-ident>
            "-2n - 3)", // <n-dimension> '-' <signless-integer>
            "-n - 3)",  // -n '-' <signless-integer>
            "2N)",      // uppercase n unit in an <n-dimension>
            "2N+1)",
            "2n of .x)",       // of S
            "2n of.x)",        // of glued to S (spec-valid; parseCss's \s+of\s+ rejects)
            "-n + 3 of li.a)", // negative An+B with of
            "2n /* c */)",     // comment is inter-token trivia (spec); parseCss rejects
            "3 /* c */ of .x)",
            // A comment SPLITTING the term: both gaps of the `['+' | '-'] <signless-integer>`
            // tail are trivia positions too, so the term matches through one at either.
            "2n /* c */ + 1)",
            "2n + /* c */ 1)",
            "2n/* c */+1)",            // glued on both sides
            "2n /* a */ /* b */ + 1)", // a run in one gap
            "2n/* a *//* b */+1)",     // a run glued to itself
            "-2n /* c */ - 3)",        // composes with the spec-only negative forms
            "n /* c */ + 1)",          // the shape that used to MIS-PARSE as a selector list
            "2n- /* c */ 1)",          // <ndash-dimension> <signless-integer>
            "2n /* c */ + 1 of .x)",   // composes with of S
            "2n/* c */of .x)",         // and with a comment as the whole `of` separator
        ] {
            assert!(
                nth_arg_is_anb(input, 0),
                "expected {input:?} to take the Nth path"
            );
        }

        // Not a clean `<an+b> [of S]?` — the selector-list fallback.
        for input in [
            ".a)",        // a bare selector
            "div)",       // a type selector
            "#id)",       // an id selector
            "N)",         // bare uppercase N reads as a type selector
            "EVEN)",      // bare uppercase keyword reads as a type selector
            "even odd)",  // An+B keyword not terminated by `)`/`,`/` of `
            "2n .foo)",   // An+B followed by a descendant selector (no terminator)
            "2n, .a)",    // a comma makes it a selector list
            "2n OF .x)",  // `of` is case-sensitive (uppercase is a type selector)
            "2n offset)", // `of` prefix of a longer ident is not the keyword
            ")",          // empty argument
            // A comment cannot split a single TOKEN: `<n-dimension>` is one token, so
            // `2/* c */n` is a number and an ident, which no An+B production covers.
            "2/* c */n)",
            // An unterminated comment reaches end-of-input (§4.3.2), so no terminator.
            "2n /* c + 1)",
        ] {
            assert!(
                !nth_arg_is_anb(input, 0),
                "expected {input:?} to take the selector-list path"
            );
        }
    }

    /// `match_nth_value` recognizes the same An+B terms Svelte's `REGEX_NTH_OF` does,
    /// via both terminator branches: `(?=\s*[,)])` (a bare An+B) and `\s+of\s+` (an
    /// `An+B of S` term, whose ` of ` folds into the matched value). Each input carries
    /// a terminator; the expected value is the matched text (`Some(len)` means the term
    /// ends at `len`).
    #[test]
    fn nth_value_matches_svelte_regex() {
        // Accepted — the returned length is the matched value width (before `S`/the stop).
        for (input, value) in [
            ("2n)", "2n"),
            ("2n+1)", "2n+1"),
            ("2n + 1)", "2n + 1"),
            ("2n - 1)", "2n - 1"),
            // VT (`U+000B`) is whitespace to Svelte's `\s`, so it separates An+B tokens.
            ("2n\x0b+\x0b1)", "2n\x0b+\x0b1"),
            ("0)", "0"),
            ("123)", "123"),
            ("+3)", "+3"),
            ("+2n)", "+2n"),
            ("+2n+1)", "+2n+1"),
            ("n)", "n"),
            ("odd)", "odd"),
            ("even)", "even"),
            ("-n+3)", "-n+3"),
            ("-2n+1)", "-2n+1"),
            ("-n + 3)", "-n + 3"),
            // Terminator lookahead permits whitespace, and `,` as well as `)`.
            ("2n )", "2n"),
            ("2n,", "2n"),
            ("odd ,", "odd"),
            // `\s+of\s+`: the ` of ` (with trailing whitespace) folds into the value;
            // `S` follows and is left for the selector loop.
            ("2n of .x)", "2n of "),
            ("odd of .a, .b)", "odd of "),
            ("-n + 3 of .a .b)", "-n + 3 of "),
            ("2n  of  .x)", "2n  of  "),
        ] {
            assert_eq!(
                match_nth_value(input, 0),
                Some(value.len()),
                "expected {input:?} to match {value:?}"
            );
        }

        // Rejected — not an An+B in a pseudo-arg position (Svelte's regex fails too).
        for input in [
            "-n)",        // leading `-` requires a `+B` tail
            "-2n)",       // same
            "-1)",        // a plain integer may not lead with `-`
            "nth)",       // `n` followed by more ident chars (terminator fails)
            "evens)",     // `even` prefix, but terminator lands on `s`
            "div)",       // an ordinary type selector
            ".a)",        // a class selector
            "2n .foo)",   // no terminator after the An+B (a following selector)
            "+)",         // a sign with no digits/`n`
            "2nx)",       // `2n` followed by an ident char
            "2n of.x)",   // `\s+of\s+` needs whitespace after `of`
            "2nof .x)",   // `\s+of\s+` needs whitespace before `of`
            "2n often )", // `of` prefix, but the trailing `\s+` lands on `ten`
            // This grammar stays COMMENT-BLIND on purpose — An+B is not a selector, so
            // there is no spec to follow here and parity with parseCss is the whole
            // point. The spec `:nth-*()` reader matches every one of these (see
            // `nth_arg_is_anb_spec_grammar`); the two must not be unified.
            "2n /* c */ + 1)",
            "2n + /* c */ 1)",
            "2n/* c */+1)",
            "2n /* c */)",
        ] {
            assert_eq!(
                match_nth_value(input, 0),
                None,
                "expected {input:?} to not match an An+B"
            );
        }
    }
}
