use super::CssParser;
use super::selectors::{
    nth_arg_is_anb, parse_complex_selector_list, parse_forgiving_selector_list,
    parse_relative_selector_list,
};
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Parse pseudo-class or pseudo-element: :hover, ::before, :nth-child(2n+1)
pub(crate) fn parse_pseudo_selector<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    start: usize,
) -> Result<SimpleSelector<'arena>, ParseError> {
    parser.advance()?; // consume first :

    // Check for :: (pseudo-element)
    let is_pseudo_element = parser.check(TokenKind::Colon);
    if is_pseudo_element {
        parser.advance()?; // consume second :
    }

    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected("pseudo-class or pseudo-element name"));
    }

    // Decode the name only for parse-time argument dispatch (`:nth-child` → Nth,
    // `:is`/`:not` → SelectorList). It is NOT stored on the node: the name is recovered
    // from `span` at convert/print time (half-decoded to match Svelte — see convert/mod.rs),
    // so storing a fully-decoded copy would be redundant and, for identity escapes, wrong.
    // The arena copy is needed because `current_identifier()` borrows a buffer that
    // `advance()` overwrites, and dispatch happens after the name token is consumed.
    let name_ident = parser.current_identifier();
    let name = parser.alloc_str_in(name_ident);
    let mut end = parser.span_pos(parser.current_end); // Capture end of name token
    parser.advance()?;

    // Check for arguments: :nth-child(2n+1), :is(), :not(), etc.
    let args = if parser.check(TokenKind::LeftParen) {
        let (args_opt, args_end) = parse_pseudo_args(parser, name, is_pseudo_element)?;
        end = args_end; // Use end of closing paren
        args_opt
    } else {
        None
    };

    if is_pseudo_element {
        Ok(SimpleSelector::PseudoElement {
            args,
            span: Span {
                start: start as u32,
                end,
            },
        })
    } else {
        Ok(SimpleSelector::PseudoClass {
            args,
            span: Span {
                start: start as u32,
                end,
            },
        })
    }
}

/// The selector-list grammar a pseudo-class/element argument uses (CSS Selectors 4).
/// `parse_pseudo_args` maps each selector-list pseudo to one of these once, so
/// `parse_selector_list_args` dispatches on an exhaustive enum rather than re-matching the
/// pseudo name (no stringly-typed `unreachable!` fallback).
#[derive(Clone, Copy)]
enum SelectorListGrammar {
    /// `:has()` — relative selectors, which may lead with a combinator (`:has(> img)`).
    Relative,
    /// `:is()`/`:where()` — forgiving: invalid items are kept as `Invalid` and the list
    /// never fails to parse.
    Forgiving,
    /// `:not()`/`:global()` and the identifier-arg pseudos `:dir()`/`:lang()`/
    /// `::highlight()`/`::slotted()` — a strict complex-real-selector list. The
    /// identifier-arg pseudos live here because Svelte parses their argument as a selector
    /// list too (`:lang(en, fr)` is two `TypeSelector`s, and a non-selector arg like
    /// `:lang("en")` is a parse error the strict grammar reproduces); `::slotted()` because
    /// parseCss models its arg as a `<complex-selector-list>` (see the `is_pseudo_element`
    /// dispatch note) and drops it from the wire AST.
    Complex,
}

/// Parse a pseudo-class/element argument list `( … )`, dispatching to a per-family
/// helper by the (lowercased) pseudo name. Returns `(Option<PseudoClassArgs>,
/// end_position_of_closing_paren)`; `None` args are unknown pseudo-classes whose
/// argument isn't a selector list.
///
/// `is_pseudo_element` distinguishes `::slotted`/`::part` (the real pseudo-elements,
/// whose args are dropped from the public AST) from a single-colon `:slotted(.x)`/
/// `:part(foo)`, which Svelte accepts as an ordinary pseudo-class with a selector-list
/// argument. `::slotted` shares the strict complex-selector-list grammar with
/// `:not()` — parseCss models its arg as a `<complex-selector-list>` (accepting
/// `::slotted(0)`, `::slotted(.a > .b)`, `::slotted(.a, .b)`, rejecting garbage/empty)
/// and drops it from the wire AST, so tsv reuses the same production and drops it at
/// the pseudo-element convert boundary. `::part` builds the dedicated `Part` arg.
/// Gating on the flag keeps these off pseudo-classes, so the single-colon forms fall
/// through to the generic selector-list path and convert to a `PseudoClassSelector`
/// matching Svelte — rather than reaching the convert layer, which exposes no
/// pseudo-element args.
///
/// Every helper takes `args_start` — the source position just after `(`, where each
/// family's `span` begins — and owns its own `)` capture.
fn parse_pseudo_args<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    pseudo_name: &str,
    is_pseudo_element: bool,
) -> Result<(Option<PseudoClassArgs<'arena>>, u32), ParseError> {
    parser.expect(TokenKind::LeftParen)?;

    let args_start = parser.current_start;

    // CSS pseudo-class/element names are ASCII case-insensitive (Selectors 4
    // §"Case-Sensitivity"), and Svelte's parser accepts the uppercase forms
    // (`:NTH-CHILD(2n+1)`, `:IS(…)`, `:GLOBAL(…)`); dispatch on the lowercased name so
    // they route to the correct argument grammar instead of degenerating onto the
    // generic path (which can't parse An+B, leaving the whole `name(args)` as an
    // opaque name). The printed name is recovered from `span` and lowercased there, so
    // this only affects argument dispatch. The `of` keyword inside `nth-*` stays
    // case-sensitive (Svelte rejects an uppercase `OF`).
    let lower_name;
    let pseudo_name: &str = if pseudo_name.bytes().any(|b| b.is_ascii_uppercase()) {
        lower_name = pseudo_name.to_ascii_lowercase();
        &lower_name
    } else {
        pseudo_name
    };

    // Dispatch by pseudo family.
    //
    // `::slotted` (strict complex-selector-list, like `:not()`) and `::part`
    // (dedicated ident list) match only the `::` form; a single-colon `:slotted`/
    // `:part` has no guard match and falls to `parse_unknown_args`' selector-list path
    // (matching Svelte's PseudoClassSelector).
    //
    // `:dir()`/`:lang()`/`::highlight()` take a single identifier per CSS spec, but
    // Svelte parses their argument as an ordinary selector list (a comma-separated
    // `:lang(en, fr)` becomes two `TypeSelector`s, not one `"en, fr"` name), so they
    // share the strict complex-selector-list grammar with `:not()`/`:global()`. That
    // also matches prettier's argument formatting (comma-spacing normalization and
    // wide-list breaking) and Svelte's rejection of non-selector args like
    // `:lang("en")`. `::highlight`'s args are dropped at the pseudo-element convert
    // boundary; the selector list only feeds the formatter.
    // The selector-list arms all run with `in_pseudo_args` set (via `selector_list_args` →
    // `with_pseudo_args`), so a bare `<number>`/`<an+b>` term parses as an `Nth` simple
    // selector (Svelte's `inside_pseudo_class` gate) — including `::slotted` (so
    // `::slotted(0)`/`::slotted(2n+1)` read as `Nth`). `nth-*` scans its own An+B grammar
    // (`parse_nth_args`), so its leading term needs no flag — but its optional `of S`
    // selector list sets `in_pseudo_args` locally (a bare `<an+b>` term in `S` reads as an
    // `Nth`, matching `:not()`'s strict list). `::part` takes a bare ident list and never
    // needs it.
    match pseudo_name {
        "slotted" if is_pseudo_element => {
            selector_list_args(parser, args_start, SelectorListGrammar::Complex)
        }
        "part" if is_pseudo_element => parse_part_args(parser, args_start),
        "has" => selector_list_args(parser, args_start, SelectorListGrammar::Relative),
        "is" | "where" => selector_list_args(parser, args_start, SelectorListGrammar::Forgiving),
        "not" | "global" | "dir" | "lang" | "highlight" => {
            selector_list_args(parser, args_start, SelectorListGrammar::Complex)
        }
        "nth-child" | "nth-of-type" | "nth-last-child" | "nth-last-of-type" | "nth-col"
        | "nth-last-col" => parse_nth_args(parser, args_start),
        _ => with_pseudo_args(parser, |p| parse_unknown_args(p, args_start)),
    }
}

/// Run `f` with `in_pseudo_args` set, restoring the previous value afterward (on both
/// the ok and error paths). Nested pseudo-class args (`:is(:not(2n))`) restore to the
/// outer `true`, and the top level returns to `false`.
fn with_pseudo_args<'arena, R>(
    parser: &mut CssParser<'_, 'arena>,
    f: impl FnOnce(&mut CssParser<'_, 'arena>) -> Result<R, ParseError>,
) -> Result<R, ParseError> {
    let saved = parser.in_pseudo_args;
    parser.in_pseudo_args = true;
    let result = f(parser);
    parser.in_pseudo_args = saved;
    result
}

/// Finish a selector-list-style pseudo-class argument: register a trailing gap comment
/// (`:is(.a /* c */)`) so the printer interleaves it, consume the closing `)`, and build
/// the `SelectorList` args spanning `args_start..)`. The shared tail of the
/// `:is()`/`:not()`-family path, the `:nth-*()` selector-list fallback, and the
/// unknown-pseudo selector path.
fn finish_selector_list_args<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    args_start: usize,
    selectors: SelectorList<'arena>,
) -> Result<(Option<PseudoClassArgs<'arena>>, u32), ParseError> {
    parser.skip_whitespace_registering_comments()?;
    let end = parser.expect_and_capture(TokenKind::RightParen)?;
    Ok((
        Some(PseudoClassArgs::SelectorList {
            selectors,
            span: Span {
                start: parser.span_pos(args_start),
                end,
            },
        }),
        end,
    ))
}

/// `::part( <ident>+ )` — one or more space-separated identifiers, NOT selectors
/// (CSS Shadow Parts). Leading/trailing gap comments are registered for the printer;
/// a comment between two idents reads as whitespace, splitting the run — parseCss
/// rejects it. `value_span` covers the identifier run (first ident start .. last
/// ident end) so the printer locates the gap comments around it (mirrors `Nth`).
fn parse_part_args<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    args_start: usize,
) -> Result<(Option<PseudoClassArgs<'arena>>, u32), ParseError> {
    parser.skip_whitespace_registering_comments()?;

    let mut idents = parser.bvec();
    // Per-ident spans, parallel to `idents`, so the printer can find comments in
    // the interior gaps between the names (and derive the whole-run span: first
    // start .. last end).
    let mut ident_spans: bumpalo::collections::Vec<'_, Span> = parser.bvec();

    // Parse space-separated identifiers
    while !parser.check(TokenKind::RightParen) && !parser.check(TokenKind::Eof) {
        if !parser.check(TokenKind::Identifier) {
            return Err(parser.error_msg("::part() requires identifier arguments"));
        }

        let ident_str = parser.current_identifier();
        let ident = parser.alloc_str_in(ident_str);
        idents.push(ident);
        ident_spans.push(Span {
            start: parser.span_pos(parser.current_start),
            end: parser.span_pos(parser.current_end),
        });
        parser.advance()?;

        parser.skip_whitespace_registering_comments()?;
    }

    if idents.is_empty() {
        return Err(parser.error_msg_at(
            "::part() requires at least one identifier",
            parser.base_offset() + args_start,
        ));
    }

    let end = parser.expect_and_capture(TokenKind::RightParen)?;

    Ok((
        Some(PseudoClassArgs::Part {
            idents: idents.into_bump_slice(),
            ident_spans: ident_spans.into_bump_slice(),
            span: Span {
                start: parser.span_pos(args_start),
                end,
            },
        }),
        end,
    ))
}

/// Parse a selector-list pseudo argument with `in_pseudo_args` set — so a bare
/// `<number>`/`<an+b>` term reads as an `Nth` simple selector (Svelte's
/// `inside_pseudo_class` gate) — wrapping the per-`grammar` `parse_selector_list_args`.
/// Shared by the four selector-list pseudo families (`:has()`, `:is()`/`:where()`, the
/// `:not()` group, `::slotted()`).
fn selector_list_args<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    args_start: usize,
    grammar: SelectorListGrammar,
) -> Result<(Option<PseudoClassArgs<'arena>>, u32), ParseError> {
    with_pseudo_args(parser, |p| parse_selector_list_args(p, args_start, grammar))
}

/// Parse the selector list inside a selector-list pseudo argument per its `grammar` (see
/// `SelectorListGrammar`), then finish with the trailing-comment + `)` capture.
/// Leading/trailing gap comments are registered so the printer interleaves them.
fn parse_selector_list_args<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    args_start: usize,
    grammar: SelectorListGrammar,
) -> Result<(Option<PseudoClassArgs<'arena>>, u32), ParseError> {
    // Register a leading comment (`:is(/* c */ .a)`) so the printer interleaves it.
    parser.skip_whitespace_registering_comments()?;

    let selector_list = match grammar {
        SelectorListGrammar::Relative => parse_relative_selector_list(parser)?,
        SelectorListGrammar::Forgiving => parse_forgiving_selector_list(parser)?,
        SelectorListGrammar::Complex => parse_complex_selector_list(parser)?,
    };

    finish_selector_list_args(parser, args_start, selector_list)
}

/// `nth-*` pseudo-classes: the spec grammar is `:nth-child(<An+B> [of S]?)` (CSS
/// Selectors 4), where `S` is a `<complex-real-selector-list>`. Only a clean leading
/// `<an+b>` term (per the css-syntax-3 microsyntax, comment-tolerant) optionally
/// followed by `of S` takes this dedicated `Nth` path; the leading An+B follows the
/// *spec* grammar, so `:nth-child(-3)`/`(-2n)`/`(-n)` read as a single `Nth` where
/// Svelte's reader mishandles them (`_svelte_divergence`), and `of S` nests as
/// `Nth.of_selector` where Svelte flattens.
///
/// Any other argument (a bare selector `:nth-child(.a)`, a selector list
/// `:nth-child(.a, .b)`, an unterminated An+B keyword `:nth-child(even odd)`, a
/// comma-list `:nth-child(2n, .foo)`) is spec-invalid; `nth_arg_is_anb` demotes it to
/// the ordinary complex-selector-list path so tsv reproduces parseCss's structured AST
/// (drop-in parity) instead of raw-capturing it as one opaque `Nth` value.
///
/// Comments in the gaps around the An+B text are registered for the printer; a comment
/// *inside* the An+B stays part of its verbatim value text. `value_span` covers just
/// the trimmed An+B so the printer can find the surrounding gap comments. The `of S`
/// list is parsed with `in_pseudo_args` set (see the call site), so it accepts the same
/// bare `<number>`/`<an+b>` terms a direct functional-pseudo arg does.
fn parse_nth_args<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    args_start: usize,
) -> Result<(Option<PseudoClassArgs<'arena>>, u32), ParseError> {
    parser.skip_whitespace_registering_comments()?;
    let anb_start = parser.current_start;

    // Not a clean `<an+b> [of S]?`: parse as an ordinary complex-selector-list (like
    // `:is()`/`:not()`), matching parseCss for selector-shaped `:nth-*()` arguments.
    if !nth_arg_is_anb(parser.source(), anb_start) {
        let selectors = with_pseudo_args(parser, parse_complex_selector_list)?;
        return finish_selector_list_args(parser, args_start, selectors);
    }

    // Scan tokens until we hit "of" keyword or closing paren
    let mut anb_end = anb_start;
    let mut found_of = false;
    let mut depth = 0;

    while !parser.check(TokenKind::Eof) {
        if parser.check(TokenKind::RightParen) && depth == 0 {
            // End of nth args
            break;
        }
        if depth == 0 && parser.check(TokenKind::Identifier) && parser.current_identifier() == "of"
        {
            // Found "of" keyword - An+B part ends here
            found_of = true;
            parser.advance()?; // consume "of"
            break;
        }
        if parser.check(TokenKind::LeftParen) {
            depth += 1;
        } else if parser.check(TokenKind::RightParen) {
            depth -= 1;
        }
        // Everything else — idents (`odd`, `n`), numbers, `+`/`-`,
        // whitespace, comments — extends the raw An+B text.
        anb_end = parser.current_end;
        parser.advance()?;
    }

    // Extract An+B value. The scan extends `anb_end` over whitespace and
    // comment tokens, so trim to the real text and keep its span — the
    // printer uses it to locate the gap comments around the An+B.
    let anb_raw = &parser.source()[anb_start..anb_end];
    let anb_trimmed_start = anb_start + (anb_raw.len() - anb_raw.trim_start().len());
    let anb_value = parser.alloc_str_in(anb_raw.trim());
    let value_span = Span {
        start: parser.span_pos(anb_trimmed_start),
        end: parser.span_pos(anb_trimmed_start + anb_value.len()),
    };

    // Parse optional selector list after "of". `S` is a `<complex-real-selector-list>`
    // (CSS Selectors 4), parsed with `in_pseudo_args` set — the same production a direct
    // functional-pseudo arg uses (Svelte's `read_selector_list(inside_pseudo_class)`), so
    // a bare `<number>`/`<an+b>` term in `S` (`2n of 123`) reads as an `Nth` simple
    // selector rather than an unexpected `<number>`/`<dimension>` token.
    let of_selector = if found_of {
        parser.skip_whitespace_registering_comments()?;
        Some(with_pseudo_args(parser, |p| {
            parse_complex_selector_list(p)
        })?)
    } else {
        None
    };

    parser.skip_whitespace_registering_comments()?;
    // The internal span extends to `)` so the printer's `comments_in_range` lookup
    // still finds a trailing gap comment (`:nth-child(2n /* c */)`). The wire `end`
    // is trimmed to the last content token at convert time (`write_pseudo_class_args`),
    // matching Svelte's `read_selector_list`, which captures its end before
    // `allow_comment_or_whitespace`.
    let span_end = parser.span_pos(parser.current_start); // `)` position
    let paren_end = parser.expect_and_capture(TokenKind::RightParen)?; // End after closing paren

    Ok((
        Some(PseudoClassArgs::Nth {
            value: anb_value,
            of_selector,
            span: Span {
                start: parser.span_pos(args_start),
                end: span_end,
            },
            value_span,
        }),
        paren_end,
    ))
}

/// Unknown pseudo-class/element (`:current()`, `:state()`, `::cue()`, …). Try to
/// parse the argument as a complex selector list first (the common case, sharing
/// `:is()`'s printer arm which already interleaves gap comments); if that fails the
/// argument isn't a selector, so skip to the matching `)` and return `None`.
fn parse_unknown_args<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    args_start: usize,
) -> Result<(Option<PseudoClassArgs<'arena>>, u32), ParseError> {
    // Register leading/trailing gap comments (`:current(/* c */ .a)`); the
    // built `SelectorList` args share `:is()`'s printer arm, which already
    // interleaves them, so this path is parser-only.
    parser.skip_whitespace_registering_comments()?;

    // Try to parse as a selector list (complex selectors, strict parsing)
    match parse_complex_selector_list(parser) {
        Ok(selector_list) => finish_selector_list_args(parser, args_start, selector_list),
        Err(_) => {
            // Parsing as selector list failed - skip arguments
            // This handles pseudo-classes with non-selector arguments
            let mut depth = 1;
            while depth > 0 && !parser.check(TokenKind::Eof) {
                if parser.check(TokenKind::LeftParen) {
                    depth += 1;
                } else if parser.check(TokenKind::RightParen) {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                if depth > 0 {
                    parser.advance()?;
                }
            }
            let end = parser.expect_and_capture(TokenKind::RightParen)?;
            Ok((None, end))
        }
    }
}
