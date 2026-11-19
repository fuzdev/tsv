use super::CssParser;
use super::selectors::{
    NthTerminator, nth_arg_terminator, parse_complex_selector_list, parse_forgiving_selector_list,
    parse_relative_selector_list,
};
use crate::ast::*;
use crate::lexer::TokenKind;
use crate::whitespace::is_boundary_whitespace;
use tsv_lang::store::flush_run;
use tsv_lang::{Error, Span};

/// Parse pseudo-class or pseudo-element: :hover, ::before, :nth-child(2n+1)
pub(crate) fn parse_pseudo_selector(
    parser: &mut CssParser<'_>,
    start: usize,
) -> Result<SimpleSelector, Error> {
    parser.advance()?; // consume first :
    // selectors-4 forbids white space "between the ':'s, or between the ':' and
    // <ident-token> or <function-token>" — and a comment is not a `<whitespace-token>`,
    // so it may sit at either juncture and stays GLUED. Skipped BEFORE the `::` test:
    // with a comment between the colons (`:/* c */:before`) the second `:` is not the
    // current token yet. Registered so the printer's verbatim prefix keeps it in place.
    parser.register_and_skip_comments()?;

    // Check for :: (pseudo-element)
    let is_pseudo_element = parser.check(TokenKind::Colon);
    if is_pseudo_element {
        parser.advance()?; // consume second :
        parser.register_and_skip_comments()?;
    }

    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected("pseudo-class or pseudo-element name"));
    }

    // Classify the argument grammar from the DECODED name (`:nth-child` → Nth,
    // `:is`/`:not` → SelectorList) while the name token is still current, so
    // `current_identifier()`'s decoded borrow is consumed immediately — the classify
    // returns a `Copy` `PseudoArgKind` and no copy of the name is made. The
    // decoded name (not the raw source slice) must drive dispatch: `:\69s(…)` decodes
    // to `is` and must route to the forgiving grammar, so raw-source dispatch would
    // regress escaped names. The name is NOT stored on the node either: it's recovered
    // from `span` at convert/print time (half-decoded to match Svelte — see
    // convert/mod.rs), so a fully-decoded copy would be redundant and, for identity
    // escapes, wrong.
    //
    // Only classify when args actually follow, matching the prior post-advance
    // `check(LeftParen)` exactly: a functional pseudo's `(` is glued to the name (any
    // whitespace lexes as a separate token, so no args follow), so `peek_kind() ==
    // LeftParen` is that same condition one token earlier. `peek_kind` parks the `(` in
    // the lookahead slot without touching `current_decoded`, so `current_identifier()`
    // still yields the name.
    //
    // The peek stays out of line: an optimized build inlines this parse (through
    // `parse_simple_selector`) into `parse_complex_selector`, whose frame every level of a
    // nested selector-list argument stacks, and an inlined lex grows that frame.
    let arg_kind = if parser.peek_kind_outlined()? == TokenKind::LeftParen {
        Some(classify_pseudo_args(
            parser.current_identifier(),
            is_pseudo_element,
        ))
    } else {
        None
    };
    // The name token's end is kept on the node: it is where the `<function-token>`'s `(`
    // begins, and no later scan of the span can re-derive it (an identity-escaped `(`
    // inside the name — `:foo\(bar(.x)` — is an earlier one).
    let name_end = parser.span_pos(parser.current_end());
    let mut end = name_end;
    parser.advance()?;

    let args = if let Some(arg_kind) = arg_kind {
        let (args_opt, args_end) = parse_pseudo_args(parser, arg_kind)?;
        end = args_end; // Use end of closing paren
        args_opt
    } else {
        None
    };

    if is_pseudo_element {
        Ok(SimpleSelector::PseudoElement {
            args,
            name_end,
            span: Span {
                start: start as u32,
                end,
            },
        })
    } else {
        Ok(SimpleSelector::PseudoClass {
            args,
            name_end,
            span: Span {
                start: start as u32,
                end,
            },
        })
    }
}

/// The selector-list grammar a pseudo-class/element argument uses (CSS Selectors 4).
/// `classify_pseudo_args` maps each selector-list pseudo to one of these once, so
/// `parse_selector_list_args` dispatches on an exhaustive enum rather than re-matching the
/// pseudo name (no stringly-typed `unreachable!` fallback).
///
/// `Relative` and `Complex` dispatch to *identical* parsing today: tsv accepts a leading
/// combinator in every selector-list context (see `selectors::parse_complex_selector`), so
/// the `<<relative-selector-list>>` vs `<<complex-selector-list>>` split no longer changes
/// what parses. The variants are kept distinct because they name a real grammar-context
/// boundary — a leading combinator is *legal* only in a relative context (`:has()`) and
/// contextually-invalid in a complex one (`:not()`) — the exact distinction the deferred
/// diagnostics layer will consume. Only `Forgiving` (`:is()`/`:where()`) parses differently.
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
    /// dispatch note).
    Complex,
}

/// The argument grammar a pseudo-class/element uses, chosen once from the (lowercased)
/// pseudo name by `classify_pseudo_args`. Threaded into `parse_pseudo_args` as a `Copy`
/// descriptor so dispatch never re-consults the name string — which lets the name be
/// classified while its token is still current, avoiding a per-pseudo copy of the
/// never-stored name.
#[derive(Clone, Copy)]
enum PseudoArgKind {
    /// A selector-list argument in one of the `SelectorListGrammar` flavors:
    /// `:is()`/`:where()`/`:not()`/`:global()`/`:has()`, the identifier-arg pseudos
    /// (`:dir()`/`:lang()`/`::highlight()`), and `::slotted()`.
    SelectorList(SelectorListGrammar),
    /// `::part( <ident>+ )` — a space-separated identifier run, not selectors.
    Part,
    /// The `:nth-child()` family — a `<An+B> [of S]?` argument.
    Nth,
    /// Any other pseudo — its argument is tried as a selector list, else kept as an
    /// `<any-value>` (`PseudoClassArgs::AnyValue`).
    Unknown,
}

/// Classify a pseudo-class/element's argument grammar from its name (see `PseudoArgKind`).
/// A pure name → grammar mapping, split out of `parse_pseudo_args` so it can run against
/// the still-current name token (its decoded borrow is consumed immediately, so no
/// copy of the name is needed).
///
/// `is_pseudo_element` distinguishes `::slotted`/`::part` (the real pseudo-elements) from a
/// single-colon `:slotted(.x)`/`:part(foo)`, which Svelte accepts as an ordinary
/// pseudo-class. `::slotted` shares the strict complex-selector-list grammar with `:not()`
/// — parseCss models its arg as a `<complex-selector-list>` (accepting `::slotted(0)`,
/// `::slotted(.a > .b)`, `::slotted(.a, .b)`, rejecting garbage/empty), so tsv reuses the
/// same production. `::part` selects the dedicated `Part` arg — parseCss has no `::part`
/// grammar at all (it reads *every* pseudo-element argument as a selector list), so the
/// ident run is tsv's own model, re-projected onto that selector-list wire shape by the
/// writer. Gating on the flag keeps these off pseudo-classes, so the single-colon forms
/// fall through to `Unknown`'s generic selector-list path and convert to a
/// `PseudoClassSelector` matching Svelte.
///
/// `:dir()`/`:lang()`/`::highlight()` take a single identifier per CSS spec, but Svelte
/// parses their argument as an ordinary selector list (a comma-separated `:lang(en, fr)`
/// becomes two `TypeSelector`s, not one `"en, fr"` name), so they share the strict
/// complex-selector-list grammar with `:not()`/`:global()`. That also matches prettier's
/// argument formatting (comma-spacing normalization and wide-list breaking) and Svelte's
/// rejection of non-selector args like `:lang("en")`.
fn classify_pseudo_args(pseudo_name: &str, is_pseudo_element: bool) -> PseudoArgKind {
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

    match pseudo_name {
        "slotted" if is_pseudo_element => PseudoArgKind::SelectorList(SelectorListGrammar::Complex),
        "part" if is_pseudo_element => PseudoArgKind::Part,
        "has" => PseudoArgKind::SelectorList(SelectorListGrammar::Relative),
        "is" | "where" => PseudoArgKind::SelectorList(SelectorListGrammar::Forgiving),
        "not" | "global" | "dir" | "lang" | "highlight" => {
            PseudoArgKind::SelectorList(SelectorListGrammar::Complex)
        }
        "nth-child" | "nth-of-type" | "nth-last-child" | "nth-last-of-type" | "nth-col"
        | "nth-last-col" => PseudoArgKind::Nth,
        _ => PseudoArgKind::Unknown,
    }
}

/// Parse a pseudo-class/element argument list `( … )` by its pre-classified
/// `PseudoArgKind` (from `classify_pseudo_args`), dispatching to the per-family helper.
/// Returns `(Option<PseudoClassArgs>, end_position_of_closing_paren)`; every family
/// returns `Some` (an unknown pseudo's non-selector argument is an `AnyValue`). Every
/// helper takes `args_start` — the source position just after `(`, where each family's
/// `span` begins — and owns its own `)` capture.
///
/// The selector-list arms all run with `in_pseudo_args` set (via `selector_list_args` →
/// `with_pseudo_args`), so a bare `<number>`/`<an+b>` term parses as an `Nth` simple
/// selector (Svelte's `inside_pseudo_class` gate) — including `::slotted` (so
/// `::slotted(0)`/`::slotted(2n+1)` read as `Nth`). `nth-*` scans its own An+B grammar
/// (`parse_nth_args`), so its leading term needs no flag — but its optional `of S`
/// selector list sets `in_pseudo_args` locally (a bare `<an+b>` term in `S` reads as an
/// `Nth`, matching `:not()`'s strict list). `::part` takes a bare ident list and never
/// needs it.
fn parse_pseudo_args(
    parser: &mut CssParser<'_>,
    arg_kind: PseudoArgKind,
) -> Result<(Option<PseudoClassArgs>, u32), Error> {
    parser.expect(TokenKind::LeftParen)?;

    let args_start = parser.current_start();

    match arg_kind {
        PseudoArgKind::SelectorList(grammar) => selector_list_args(parser, args_start, grammar),
        PseudoArgKind::Part => parse_part_args(parser, args_start),
        PseudoArgKind::Nth => parse_nth_args(parser, args_start),
        PseudoArgKind::Unknown => with_pseudo_args(parser, |p| parse_unknown_args(p, args_start)),
    }
}

/// Run `f` with `in_pseudo_args` set, restoring the previous value afterward (on both
/// the ok and error paths). Nested pseudo-class args (`:is(:not(2n))`) restore to the
/// outer `true`, and the top level returns to `false`.
fn with_pseudo_args<R>(
    parser: &mut CssParser<'_>,
    f: impl FnOnce(&mut CssParser<'_>) -> Result<R, Error>,
) -> Result<R, Error> {
    let saved = parser.in_pseudo_args;
    parser.in_pseudo_args = true;
    // four wire levels: the argument `SelectorList`, a `ComplexSelector`, a
    // `RelativeSelector`, and the selector inside it (the depth limit's count)
    let result = parser.nested(4, f);
    parser.in_pseudo_args = saved;
    result
}

/// Finish a selector-list-style pseudo-class argument: register a trailing gap comment
/// (`:is(.a /* c */)`) so the printer interleaves it, consume the closing `)`, and build
/// the `SelectorList` args spanning `args_start..)`. The shared tail of the
/// `:is()`/`:not()`-family path, the `:nth-*()` selector-list fallback, and the
/// unknown-pseudo selector path.
fn finish_selector_list_args(
    parser: &mut CssParser<'_>,
    args_start: usize,
    selectors: SelectorList,
) -> Result<(Option<PseudoClassArgs>, u32), Error> {
    parser.skip_boundary_whitespace_registering_comments()?;
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
fn parse_part_args(
    parser: &mut CssParser<'_>,
    args_start: usize,
) -> Result<(Option<PseudoClassArgs>, u32), Error> {
    parser.skip_boundary_whitespace_registering_comments()?;

    // One span per name: the printer emits each name verbatim from its span, finds
    // comments in the interior gaps between them, and derives the whole-run span
    // (first start .. last end); the wire projects the run from them too.
    let mark = parser.stores.runs.part_span_scratch.mark();

    // Parse space-separated identifiers
    while !parser.check(TokenKind::RightParen) && !parser.check(TokenKind::Eof) {
        if !parser.check(TokenKind::Identifier) {
            return Err(parser.error_msg("::part() requires identifier arguments"));
        }

        let ident = Span {
            start: parser.span_pos(parser.current_start()),
            end: parser.span_pos(parser.current_end()),
        };
        parser.stores.runs.part_span_scratch.push(ident);
        parser.advance()?;

        parser.skip_boundary_whitespace_registering_comments()?;
    }

    if parser.stores.runs.part_span_scratch.len_since(mark) == 0 {
        return Err(parser.error_msg_at(
            "::part() requires at least one identifier",
            parser.base_offset() + args_start,
        ));
    }

    let end = parser.expect_and_capture(TokenKind::RightParen)?;

    let ident_spans = flush_run(
        &mut parser.stores.runs.part_span_scratch,
        &mut parser.stores.runs.part_spans,
        mark,
    );
    Ok((
        Some(PseudoClassArgs::Part {
            ident_spans,
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
fn selector_list_args(
    parser: &mut CssParser<'_>,
    args_start: usize,
    grammar: SelectorListGrammar,
) -> Result<(Option<PseudoClassArgs>, u32), Error> {
    with_pseudo_args(parser, |p| parse_selector_list_args(p, args_start, grammar))
}

/// Parse the selector list inside a selector-list pseudo argument per its `grammar` (see
/// `SelectorListGrammar`), then finish with the trailing-comment + `)` capture.
/// Leading/trailing gap comments are registered so the printer interleaves them.
fn parse_selector_list_args(
    parser: &mut CssParser<'_>,
    args_start: usize,
    grammar: SelectorListGrammar,
) -> Result<(Option<PseudoClassArgs>, u32), Error> {
    // Register a leading comment (`:is(/* c */ .a)`) so the printer interleaves it.
    parser.skip_boundary_whitespace_registering_comments()?;

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
/// comma-list `:nth-child(2n, .foo)`) is spec-invalid; `nth_arg_terminator` demotes it to
/// the ordinary complex-selector-list path so tsv reproduces parseCss's structured AST
/// (drop-in parity) instead of raw-capturing it as one opaque `Nth` value.
///
/// Comments in the gaps around the An+B text are registered for the printer; a comment
/// *inside* the An+B stays part of its verbatim value text. `value_span` covers just
/// the trimmed An+B so the printer can find the surrounding gap comments. The `of S`
/// list is parsed with `in_pseudo_args` set (see the call site), so it accepts the same
/// bare `<number>`/`<an+b>` terms a direct functional-pseudo arg does.
fn parse_nth_args(
    parser: &mut CssParser<'_>,
    args_start: usize,
) -> Result<(Option<PseudoClassArgs>, u32), Error> {
    parser.skip_boundary_whitespace_registering_comments()?;
    let anb_start = parser.current_start();

    // Not a clean `<an+b> [of S]?`: parse as an ordinary complex-selector-list (like
    // `:is()`/`:not()`), matching parseCss for selector-shaped `:nth-*()` arguments.
    let Some(terminator) = nth_arg_terminator(parser.source(), anb_start) else {
        let selectors = with_pseudo_args(parser, parse_complex_selector_list)?;
        return finish_selector_list_args(parser, args_start, selectors);
    };

    // The An+B text runs from the term to its terminator — whitespace and comments
    // included, so a comment in the gap before the `)` or the `of` stays part of the
    // verbatim value (`2n /* c */`), as it always has. The scanner's offset is the only
    // sound bound: the terminator can begin INSIDE the current token (`1<NBSP>of` is one
    // dimension token, the `of` its unit's tail), so the token stream is not walked to it.
    let (anb_end, found_of) = match terminator {
        NthTerminator::Paren(at) => (at, false),
        NthTerminator::Of(at) => (at, true),
    };

    // Trim to the real text and keep its span — the printer uses it to locate the gap
    // comments around the An+B. The class is the parser's boundary one (JS `\s` ∪
    // `White_Space`): a `<NBSP>` or `<ZWNBSP>` between the term and its terminator is the
    // run `parseCss` skipped there, and `str::trim` (Unicode `White_Space`) would leave the
    // `<ZWNBSP>` on the value.
    let anb_raw = &parser.source()[anb_start..anb_end];
    let anb_trimmed_start =
        anb_start + (anb_raw.len() - anb_raw.trim_start_matches(is_boundary_whitespace).len());
    let anb_trimmed = anb_raw.trim_matches(is_boundary_whitespace);
    let value_span = Span {
        start: parser.span_pos(anb_trimmed_start),
        end: parser.span_pos(anb_trimmed_start + anb_trimmed.len()),
    };
    let anb_value = parser.stores.text.push(anb_trimmed);

    // Re-seat the lexer at the terminator: on the `)` itself, or just past the `of`
    // letters so the boundary skip below steps whatever follows them — the run inside the
    // token the lexer had read as `of<NBSP>` included.
    parser.resume_at(if found_of { anb_end + 2 } else { anb_end })?;

    // Parse optional selector list after "of". `S` is a `<complex-real-selector-list>`
    // (CSS Selectors 4), parsed with `in_pseudo_args` set — the same production a direct
    // functional-pseudo arg uses (Svelte's `read_selector_list(inside_pseudo_class)`), so
    // a bare `<number>`/`<an+b>` term in `S` (`2n of 123`) reads as an `Nth` simple
    // selector rather than an unexpected `<number>`/`<dimension>` token.
    let of_selector = if found_of {
        parser.skip_boundary_whitespace_registering_comments()?;
        // four wire levels more than a selector-list argument's: the `Nth` reads its `of`
        // list inside the argument's own `SelectorList`, `ComplexSelector` and
        // `RelativeSelector` (the depth limit's count)
        Some(parser.nested(4, |p| with_pseudo_args(p, parse_complex_selector_list))?)
    } else {
        None
    };

    parser.skip_boundary_whitespace_registering_comments()?;
    // The internal span extends to `)` so the printer's `comments_to_emit_in_range` lookup
    // still finds a trailing gap comment (`:nth-child(2n /* c */)`). The wire `end`
    // is trimmed to the last content token at convert time (`write_pseudo_class_args`),
    // matching Svelte's `read_selector_list`, which captures its end before
    // `allow_comment_or_whitespace`.
    let span_end = parser.span_pos(parser.current_start()); // `)` position
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

/// Unknown pseudo-class/element (`:current()`, `:state()`, `::cue()`, …). Selectors 4
/// gives a functional pseudo the argument `<any-value>`, so tsv tries the common case
/// first — a complex selector list that runs exactly to the `)`, sharing `:is()`'s printer
/// arm, which already interleaves gap comments — and otherwise keeps the argument as an
/// `AnyValue` (`:foo(1 + 2)`, `:foo(f(1 2))`). A list that parses but stops short of the
/// `)` (`:foo(f(1 2))` reads `f` and stops at its `(`) is no selector argument either.
fn parse_unknown_args(
    parser: &mut CssParser<'_>,
    args_start: usize,
) -> Result<(Option<PseudoClassArgs>, u32), Error> {
    // The fallback rewinds every registration the selector attempt made — the leading gap
    // comment included — since an `AnyValue` prints its comments inside its own text.
    let comments_len = parser.comments.len();
    // Register leading/trailing gap comments (`:current(/* c */ .a)`); the
    // built `SelectorList` args share `:is()`'s printer arm, which already
    // interleaves them, so this path is parser-only.
    parser.skip_boundary_whitespace_registering_comments()?;

    #[cfg(feature = "_internal_ast_census")]
    let ast_census_mark = tsv_lang::ast_census_probe::mark();
    // A failed list leaves the frames it opened on the scratch stacks, above the enclosing
    // compound's, so its recovery rewinds every stack to here.
    let scratch_marks = parser.stores.scratch_marks();
    // Try to parse as a selector list (complex selectors, strict parsing)
    match parse_complex_selector_list(parser) {
        Ok(selector_list) => {
            parser.skip_boundary_whitespace_registering_comments()?;
            if parser.check(TokenKind::RightParen) {
                return finish_selector_list_args(parser, args_start, selector_list);
            }
        }
        // a depth limit is no verdict on the arguments (see `parse_forgiving_selector_list`)
        Err(error) if error.kind() == tsv_lang::ErrorKind::TooDeep => return Err(error),
        Err(_) => {}
    }
    #[cfg(feature = "_internal_ast_census")]
    tsv_lang::ast_census_probe::rewind(ast_census_mark);
    parser.stores.truncate_scratch(&scratch_marks);
    parser.rewind_to(args_start, comments_len)?;
    let end = scan_any_value(parser, args_start)?;
    let close = parser.expect_and_capture(TokenKind::RightParen)?;
    Ok((
        Some(PseudoClassArgs::AnyValue {
            span: Span {
                start: parser.span_pos(args_start),
                end,
            },
        }),
        close,
    ))
}

/// Step an `<any-value>` (css-syntax-3) from `args_start` to the `)` that closes the
/// pseudo's argument, leaving the parser on it and returning its host offset. Its comments
/// are registered — the wire's root `comments` list carries them — while the printer
/// prints them inside the `AnyValue`'s own text and claims them there.
///
/// `<any-value>` is one or more tokens, any but a `<bad-string-token>`, a
/// `<bad-url-token>`, and an unmatched `)`, `]` or `}` — a top-level `;` or `!` is content.
/// So a block opened inside it must close with its own bracket (a stack, not a paren
/// count: `:foo({)` and `:foo(1])` reject), and the lexer's two tokens it does not model as
/// bad are graded here (`bad_string`, `bad_url`). Whitespace and comments are no tokens,
/// so `:foo()`, `:foo( )` and `:foo(/* c */)` reject.
fn scan_any_value(parser: &mut CssParser<'_>, args_start: usize) -> Result<u32, Error> {
    let mut open: Vec<TokenKind> = Vec::new();
    let mut has_token = false;
    loop {
        let kind = parser.current_kind();
        match kind {
            TokenKind::Eof => return Err(parser.error_expected("')'")),
            TokenKind::RightParen if open.is_empty() => break,
            TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace => {
                open.push(kind);
            }
            TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace => {
                let opener = match kind {
                    TokenKind::RightParen => TokenKind::LeftParen,
                    TokenKind::RightBracket => TokenKind::LeftBracket,
                    _ => TokenKind::LeftBrace,
                };
                if open.pop() != Some(opener) {
                    return Err(parser.error_msg("Unmatched bracket in a pseudo argument"));
                }
            }
            TokenKind::String { .. } if bad_string(parser.current_value()) => {
                return Err(parser.error_msg("Unescaped newline in a string"));
            }
            TokenKind::Url => {
                if let Some(cause) = bad_url(parser.current_value()) {
                    return Err(parser.error_msg(&format!("Invalid url(): {cause}")));
                }
            }
            TokenKind::Comment => parser.register_current_comment(),
            _ => {}
        }
        if !matches!(kind, TokenKind::Whitespace | TokenKind::Comment) {
            has_token = true;
        }
        parser.advance()?;
    }
    if !has_token {
        return Err(parser.error_msg_at(
            "Expected a functional pseudo argument",
            parser.base_offset() + args_start,
        ));
    }
    Ok(parser.span_pos(parser.current_start()))
}

/// Is this string token a `<bad-string-token>` — one holding an unescaped newline
/// (css-syntax-3 §4.3.5)? An escaped newline is a line continuation, and content.
fn bad_string(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            // an escaped CRLF is one newline, continued as a whole
            b'\\' if bytes[i + 1..].starts_with(b"\r\n") => i += 3,
            b'\\' => i += 2,
            b'\n' | b'\r' | b'\x0C' => return true,
            _ => i += 1,
        }
    }
    false
}

/// Is this url token a `<bad-url-token>` (css-syntax-3 §4.3.6), and why? Past the
/// whitespace at either end of its content, a url holds no whitespace, quote, `(`,
/// non-printable code point, or `\` that opens no valid escape (one before a newline or
/// the end).
fn bad_url(text: &str) -> Option<&'static str> {
    let open = text.find('(')?;
    let inner = &text[open + 1..];
    let content = inner.strip_suffix(')').unwrap_or(inner);
    let content = content.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0C'));
    let mut chars = content.chars();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' | '\r' | '\x0C' => return Some("whitespace inside the url"),
            '"' | '\'' | '(' => return Some("a quote or '(' inside the url"),
            '\0'..='\x08' | '\x0B' | '\x0E'..='\x1F' | '\x7F' => {
                return Some("a non-printable character inside the url");
            }
            '\\' => {
                if matches!(chars.next(), None | Some('\n' | '\r' | '\x0C')) {
                    return Some("an invalid escape inside the url");
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{bad_string, bad_url};

    #[test]
    fn bad_string_is_an_unescaped_newline() {
        for good in [
            "'x'",
            "\"a b\"",
            "'a\\\nb'",
            "'a\\\r\nb'",
            "'a\\\rb'",
            "'a\\\x0Cb'",
        ] {
            assert!(!bad_string(good), "{good:?}");
        }
        for bad in ["'a\nb'", "'a\rb'", "'a\r\nb'", "'a\x0Cb'", "'a\\\r\r\nb'"] {
            assert!(bad_string(bad), "{bad:?}");
        }
    }

    #[test]
    fn bad_url_names_its_cause() {
        for good in [
            "url(x)",
            "url( x )",
            "url(\n\tx\n)",
            "url(a\\ b)",
            "url(a\\(b)",
            "url(x",
        ] {
            assert_eq!(bad_url(good), None, "{good:?}");
        }
        for bad in [
            "url(x y)",
            "url(a\tb)",
            "url(a\"b)",
            "url(a'b)",
            "url(a(b)",
            "url(a\x01b)",
            "url(a\x7Fb)",
            "url(a\\\nb)",
            "url(a\\)",
        ] {
            assert!(bad_url(bad).is_some(), "{bad:?}");
        }
    }
}
