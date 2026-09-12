//! CSS value printing
//!
//! Handles printing of all CSS value types:
//! - Simple values (identifiers, strings, dimensions, colors)
//! - Compound values (lists, functions)
//! - Semantic formatting with source fidelity
//!
//! ## Architecture
//!
//! This module uses a doc-first approach where all formatting logic lives in
//! `build_*_doc()` methods. The `print_*` methods are thin wrappers that call
//! the corresponding doc builder and write the result.
//!
//! The main entry point is `build_css_value_doc()`, which dispatches to
//! specialized doc builders for each value type.

use super::{Printer, value_normalization};
use crate::ast::internal::{Color, CssValue, StringCooked};
use std::borrow::Cow;
use tsv_lang::Span;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::printing::format_string_literal;

/// A value operator's kind, read back off the single byte
/// `parser::value::operators::split_value_run` emitted as a
/// [`CssValue::Operator`].
///
/// The kind is what the separator rule turns on, and it is not stored on the node: the
/// byte IS the kind, so reading it at print time keeps the AST at one field (the
/// span-for-verbatim idiom every other leaf takes).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ValueOperator {
    /// `/` — a division. Glued or spaced by its neighbours' kinds.
    Division,
    /// `*` — a multiplication. Always spaced.
    Multiplication,
    /// `+` — an addition. Like `Division`, but reached by a different disjunct.
    Addition,
    /// `-` — a subtraction. Never spaced by tsv where the author glued it.
    Subtraction,
    /// `:` — a key/value colon inside a group. Never spaced *before*; the space after it
    /// is the colon's own, not a separator.
    Colon,
}

impl ValueOperator {
    /// The operator this member is, or `None` when the member is an operand.
    fn of(value: &CssValue<'_>, source: &str) -> Option<Self> {
        let CssValue::Operator { span } = value else {
            return None;
        };
        match span.extract(source).as_bytes() {
            [b'/'] => Some(Self::Division),
            [b'*'] => Some(Self::Multiplication),
            [b'+'] => Some(Self::Addition),
            [b'-'] => Some(Self::Subtraction),
            [b':'] => Some(Self::Colon),
            // Unreachable: the value splitter is the only producer of the node and emits
            // one of the five above. Reading an operand rather than defaulting to a kind
            // keeps the conservative answer (an ordinary member gap) if it ever isn't.
            _ => {
                debug_assert!(
                    false,
                    "a CssValue::Operator over {:?}",
                    span.extract(source)
                );
                None
            }
        }
    }

    /// Is this one of the `<calc-sum>` operators css-values-4 requires whitespace around
    /// — the pair prettier prints exactly as authored inside `calc()`?
    const fn is_sum(self) -> bool {
        matches!(self, Self::Addition | Self::Subtraction)
    }
}

/// Where in the document a value sits, as its separator rule reads it — the part of
/// [`ValueCtx`] that is fixed before the value is reached and does not change as the
/// recursion descends.
///
/// prettier asks both of these of the *path* (`getPropOfDeclNode`, `insideAtRuleNode`),
/// walking up from the node. tsv has no path, so the printer parks the answers for the
/// declaration's duration (`Printer::value_scope`) instead of threading them through
/// every doc builder.
#[derive(Clone, Copy, Default)]
pub(crate) struct ValueScope {
    /// The declaration is `font` or a custom property, whose glued `/` beside a font-size
    /// operand stays glued (prettier's §"Formatting `font` property", `isPossibleFontSize`).
    pub(crate) font_shorthand: bool,
    /// The declaration sits in an `@utility` block, where a word followed by `*` is
    /// glued — Tailwind's `--value(--tab-size-*)`.
    pub(crate) in_utility_at_rule: bool,
}

/// What a value's surroundings contribute to its separator rule: its [`ValueScope`] plus
/// the one bit the value recursion itself accumulates.
#[derive(Clone, Copy)]
pub(crate) struct ValueCtx {
    /// Fixed before the value is reached.
    pub(crate) scope: ValueScope,
    /// Some enclosing function is `calc()` — prettier's
    /// `insideValueFunctionNode(path, "calc")`. Its `+` / `-` gaps print as authored,
    /// because css-values-4 makes that gap the difference between a valid expression and
    /// an invalid one, so a formatter must not invent or remove one. ⚠️ `calc()` only:
    /// `min()` / `max()` / `clamp()` take the ordinary rule, as they do in prettier.
    pub(crate) in_calc: bool,
}

impl ValueCtx {
    /// The context a function's arguments inherit: `calc()` is sticky once entered.
    fn within_function(self, is_calc: bool) -> Self {
        Self {
            in_calc: self.in_calc || is_calc,
            ..self
        }
    }
}

/// Is this member one of prettier's `value-word`s — the kind that makes an adjacent
/// operator take a space?
///
/// A named or hex colour is one: postcss gives `red` and `#fff` a `value-word` node and
/// only flags them, where tsv folds the flag into the node kind. An `rgb()` / `hsl()`
/// colour is a **function** there, so it answers [`is_func_member`] instead.
fn is_word_member(value: &CssValue<'_>) -> bool {
    matches!(
        value,
        CssValue::Identifier { .. }
            | CssValue::Color {
                color: Color::Named | Color::Hex,
                ..
            }
    )
}

/// Is this member one of prettier's `value-func`s?
///
/// ⚠️ A **nameless** group is not: postcss gives `(1.5)` a `value-paren_group`, a node
/// neither this nor [`is_word_member`] claims, and the whole of the operator-before-group
/// rule turns on that. A colour *function* is one, tsv having folded it into `Color`.
fn is_func_member(value: &CssValue<'_>, source: &str) -> bool {
    match value {
        CssValue::Function { name_span, .. } => !name_span.extract(source).is_empty(),
        CssValue::Color {
            color: Color::Rgb { .. } | Color::Hsl { .. },
            ..
        } => true,
        CssValue::SupportsCondition { .. } => true,
        _ => false,
    }
}

/// Could this member stand where a `font` shorthand's font-size does — prettier's
/// `isPossibleFontSize`, the operand its `/` carve-out keys on?
fn is_possible_font_size(value: &CssValue<'_>, source: &str) -> bool {
    match value {
        CssValue::Dimension { .. } => true,
        CssValue::Function { name_span, .. } => {
            let name = name_span.extract(source);
            ["var", "calc", "min", "max", "clamp"]
                .iter()
                .any(|kw| name.eq_ignore_ascii_case(kw))
                || name.starts_with("--")
        }
        _ => false,
    }
}

/// Does the source hold nothing at all between these two members?
///
/// prettier's `hasEmptyRawBefore`, read off the document rather than off a `raws` field —
/// which is the whole of tsv's divergence at a parenthesized group, whose node prettier
/// synthesizes without one and therefore can only ever read as "not glued". See
/// [conformance_prettier_css.md §CSS: Values](../../../../docs/conformance_prettier_css.md#css-values).
fn authored_glued(prev: &CssValue<'_>, next: &CssValue<'_>) -> bool {
    prev.span().end == next.span().start
}

impl<'a> Printer<'a> {
    /// Format a CSS value
    ///
    /// Uses the doc builder which handles source fidelity and proper formatting.
    pub(super) fn print_css_value(&mut self, value: &CssValue<'_>) {
        let doc = self.build_css_value_doc(value);
        self.write_arena_doc(doc);
    }

    /// Format a nested value (function arg, list item)
    ///
    /// Alias for `print_css_value` - kept for semantic clarity in call sites.
    #[inline]
    pub(super) fn print_nested_value(&mut self, value: &CssValue<'_>) {
        self.print_css_value(value);
    }

    //
    // Doc Builders - all formatting logic expressed as doc IR
    //

    /// Build a doc for a CSS value
    ///
    /// Main entry point for value formatting. Dispatches to specialized doc
    /// builders for each value type. Handles source fidelity by extracting
    /// from source where appropriate.
    pub(super) fn build_css_value_doc(&self, value: &CssValue<'_>) -> DocId {
        self.build_css_value_doc_in(value, self.value_ctx())
    }

    /// The value context the printer's own state supplies: the two declaration-scoped
    /// bits, with `in_calc` left for the recursion to set where it enters one.
    pub(super) fn value_ctx(&self) -> ValueCtx {
        ValueCtx {
            scope: self.value_scope,
            in_calc: false,
        }
    }

    /// [`Self::build_css_value_doc`] carrying the surroundings its separator rule reads
    /// ([`ValueCtx`]) — an enclosing `calc()` and the `font`/custom-property carve-out.
    /// Neither is a property of the value, so both ride down the recursion rather than
    /// being re-derived at each level.
    pub(super) fn build_css_value_doc_in(&self, value: &CssValue<'_>, ctx: ValueCtx) -> DocId {
        match value {
            CssValue::Identifier { span } => self.build_identifier_doc(*span),
            // The operator's single byte, verbatim. Whether a space stands beside it is
            // the enclosing list's question, never this node's (`value_gap_is_glued`).
            CssValue::Operator { span } => self.d().source_span(*span, self.source),
            CssValue::String { content, span } => self.build_string_doc(content, *span),
            CssValue::Dimension { span, .. } => self.build_dimension_doc(*span),
            CssValue::Color { color, span } => self.build_color_doc(color, *span),
            CssValue::Function {
                name_span,
                args,
                span,
            } => self.build_value_function_doc(*name_span, args, *span, ctx),
            CssValue::SupportsCondition {
                name, condition, ..
            } => self.build_supports_condition_doc(name, condition),
            CssValue::List { values, .. } => self.build_value_list_doc(values, ctx),
            CssValue::CommaSeparated { values, span } => {
                let joined = self.build_separated_values_doc(values, ", ");
                // Joining N elements writes N-1 commas, so an authored closing comma —
                // one that terminated no element — is spelled back here.
                if super::declarations::list_has_closing_comma(
                    self.source,
                    values,
                    span.end_usize(),
                ) {
                    let d = self.d();
                    d.concat(&[joined, d.text(",")])
                } else {
                    joined
                }
            }
        }
    }

    /// Build a doc for an identifier value
    ///
    /// The ordinary leaf printer — everything the classifiers did not claim as a string,
    /// dimension, colour or function (`red`, `flex`, `100%`, 98.6% of identifier values on
    /// a real corpus). The text is source-extracted so escapes survive verbatim, with
    /// whitespace normalization for the interior runs of a token the value parser left
    /// opaque: a comment run, or a paren shell whose first `(` does not close at the value's
    /// end (`(a b c)(d)`). A *balanced* parenthesized group is not one of those — it parses
    /// as a nameless `CssValue::Function` (css-syntax-3's `()`-block) and prints through
    /// `build_value_function_doc`, so its members reach the ordinary value printers and its
    /// layout is the function group's.
    fn build_identifier_doc(&self, span: Span) -> DocId {
        let d = self.d();
        // The identifier text is recovered from source (escapes preserved verbatim).
        if span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            if !raw.is_empty() {
                // The verbatim case first, asked of the document's word: a value holding no
                // byte the normalizer acts on — the overwhelmingly-common single-token
                // identifier (`red`, `flex`, `100%`), 98.6% of identifier values on a real
                // corpus — is emitted as a zero-allocation `DocText::SourceSpan` (the same
                // source borrow the number / dimension normalizers take) without the
                // normalizer's own byte-at-a-time scan-skip test over the bare slice.
                if value_normalization::normalize_is_noop_in(self.source, span) {
                    return d.source_span(span, self.source);
                }
                // Normalize whitespace for parenthesized expressions
                // (e.g., "(  100%  -  40px  )" → "(100% - 40px)").
                let normalized = value_normalization::normalize_css_whitespace(raw);

                return match normalized {
                    // Verbatim value the host-word test refused (it holds a control byte the
                    // normalizer keeps, `<VT>` and its kin): normalized == source[span], so
                    // the same zero-allocation `DocText::SourceSpan`.
                    Cow::Borrowed(_) => d.source_span(span, self.source),
                    Cow::Owned(s) => d.text_pooled(&s),
                };
            }
        }
        // Empty / whitespace-only span (the empty-identifier sentinel) or an
        // out-of-range span (never in practice — spans index the printer's source):
        // nothing to emit.
        d.text("")
    }

    /// Build a doc for a string value
    ///
    /// Source-extracts the raw string so escape sequences are preserved verbatim
    /// (`\a`, `\41`, `\\`, line continuations), normalizing only the quote char
    /// (`"` → `'`) to match prettier. Re-serializing the *decoded* `content` would
    /// corrupt escapes — e.g. emit `\a` as a literal newline (content loss). Mirrors
    /// `build_identifier_doc` and the plain-declaration-value path
    /// (`extract_string_value`); the decoded `content` is only the fallback when the
    /// span is unavailable.
    fn build_string_doc(&self, content: &StringCooked<'_>, span: Span) -> DocId {
        if span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            // The span covers the full literal including quotes (see
            // `parse_string_literal`); strip them and re-emit with quote normalization.
            // The original quote is the first byte of the span (recovered from source,
            // not stored).
            if raw.len() >= 2 && (raw.starts_with('\'') || raw.starts_with('"')) {
                let quote = raw.as_bytes()[0] as char;
                let inner = &raw[1..raw.len() - 1];
                return self.d().text_pooled(&format_string_literal(inner, quote));
            }
        }
        // Fallback: span unavailable (never in practice — spans index the printer's
        // source). Re-emit the decoded content; a `Verbatim` value has no recoverable
        // text without its span, so emit nothing (mirrors `build_identifier_doc`).
        match content {
            StringCooked::Decoded(s) => self.d().text_pooled(&format_string_literal(s, '\'')),
            StringCooked::Verbatim => self.d().text(""),
        }
    }

    /// Build a doc for a dimension value (number + unit)
    ///
    /// Normalizes trailing zeros and adds leading zeros, preserving source
    /// characteristics like leading zeros and signs. An already-canonical
    /// dimension (`10px`, `0.5rem`) borrows its source slice, so it emits a
    /// zero-allocation `source_span`; only a rewritten dimension allocates.
    /// Mirrors the TS literal path (`Printer::build_number_literal_doc`).
    fn build_dimension_doc(&self, span: Span) -> DocId {
        let raw = span.extract(self.source);
        match value_normalization::normalize_dimension_from_source(raw) {
            Cow::Borrowed(_) => self.d().source_span(span, self.source),
            Cow::Owned(s) => self.d().text_pooled(&s),
        }
    }

    /// Build a doc for a color value
    ///
    /// Preserves color syntax (hex, rgb, hsl, etc.) from source.
    fn build_color_doc(&self, color: &Color, span: Span) -> DocId {
        // A verbatim named color comes back `Cow::Borrowed` (== source[span]) and is
        // emitted as a zero-allocation `DocText::SourceSpan`, like the identifier /
        // dimension paths; hex and function syntaxes own their reconstructed text.
        match value_normalization::format_color_from_source(color, self.source, span) {
            Cow::Borrowed(_) => self.d().source_span(span, self.source),
            Cow::Owned(s) => self.d().text_pooled(&s),
        }
    }

    /// Build a flat (non-wrapping) `name(args_doc)` function doc.
    ///
    /// The shared `name(` … `)` envelope for the `url()` and `var(--a,)`
    /// empty-fallback paths, which are kept flat by design (opaque / no break
    /// points). Every other function goes through the wrapping path
    /// (`build_value_function_doc`'s `group(…softline…)` structure) so it can
    /// break when it exceeds width.
    fn flat_function_doc(&self, name_span: Span, args_doc: DocId) -> DocId {
        let d = self.d();
        d.concat(&[
            d.source_span(name_span, self.source),
            d.text("("),
            args_doc,
            d.text(")"),
        ])
    }

    /// Build a doc for a function value with automatic wrapping
    ///
    /// Uses proper doc structure with group/softline/indent so the renderer
    /// decides wrapping based on actual line position (like Prettier). Every
    /// function gets break points (a softline after `(` and before `)`), so a
    /// single over-width arg wraps onto its own line just like a multi-arg list
    /// — matching prettier's `parenthesized-value-group`.
    ///
    /// - Multi-arg functions: wrap each arg on its own line when exceeds width
    /// - Single-arg List (e.g., drop-shadow): wrap on space separators
    /// - Single-arg non-List (e.g., `fn(token)`): wrap the arg onto its own line
    ///   when it exceeds width
    /// - `url()` and the `var(--a,)` empty fallback: kept flat — handled before
    ///   this point
    /// - a blank argument region: closes flush (`f(  )` → `f()`)
    ///
    /// A **nameless** `name_span` is the parenthesized group (css-syntax-3's `()`-block,
    /// `value/mod.rs::extract_function_parts`), and it takes every rule above unchanged —
    /// the name simply emits nothing. `url` / `var` recognition refuses it on length, so no
    /// arm needs a clause for it.
    pub(super) fn build_value_function_doc(
        &self,
        name_span: Span,
        args: &[CssValue<'_>],
        span: Span,
        ctx: ValueCtx,
    ) -> DocId {
        let d = self.d();
        // `calc()` is sticky down the whole subtree, which is prettier's
        // `insideValueFunctionNode(path, "calc")` — an ancestor walk, not a parent test.
        let ctx = ctx.within_function(self.function_name_is(name_span, "calc"));
        // `url` is opaque whether or not its content was parsed, so it answers first and
        // in one place — the prelude path leaves `@import url(a.css)` unparsed (empty
        // args) while a declaration value parses them, and both want the same verbatim
        // form. Matched ASCII-case-insensitively (css-syntax): `URL(…)` is a url too, so
        // it takes the same path (casing preserved via `span`) rather than generic-function
        // normalization — which would space an interior `/*` in the now-lexed `URL(x/*y)`
        // url-token.
        if self.function_name_is(name_span, "url") {
            // Quoted url() — a single string arg. Print it through the normal string
            // path so the quote is normalized (`"x"` → `'x'`), matching prettier.
            if let [arg @ CssValue::String { .. }] = args {
                return self.flat_function_doc(name_span, self.build_css_value_doc(arg));
            }
            // The same string plus a trailing comment region (`url('a.css' /* c */)`) —
            // still the string path, so the quote still normalizes; the region joins
            // single-spaced, the crate's uniform comment-spacing rule (prettier freezes the
            // authored spacing — conformance_prettier_css.md §CSS: Comments). Kept separate
            // from the arm above rather than folded into it: `join` mints a separator
            // `text` node the single-arg case would never spend, and that case is every
            // `url()` in a stylesheet. A *leading* comment cannot reach here at all — it
            // makes `url(` an opaque `<url-token>`, which lands on the verbatim arm below.
            if let [CssValue::String { .. }, ..] = args {
                return self
                    .flat_function_doc(name_span, self.build_separated_values_doc(args, " "));
            }
            // Unquoted url() — the content is opaque. Emit the raw source verbatim,
            // stripping only the whitespace right after `url(` and right before `)`
            // (prettier's `printer-postcss.js` url handling). Rejoining parsed args
            // would drop empty/trailing comma segments (`url(a,b,)` → `url(a,b)`),
            // silently changing the URL — the comma is part of the resource ref — so
            // raw text wins over the args even when it isn't parenthesized at all (which
            // a parsed function's span cannot be; the arm is defensive).
            //
            // Deliberately pooled, not `source_span`: this arm is hot on CSS corpora
            // (every `url(...)`) and the span form's render-time resolution hop measured
            // +0.07% instructions there for no allocation win (the pool is amortized).
            // TODO: re-measure that verdict — it predates the render's inlined `resolve_text`,
            // and TWO span moves have since read no hop at all: the selector leaf
            // (`span_leaf_doc`, ~12-byte slices, −0.34%) and this function's own name, whose
            // `name_span` retired both a copy and the guard that had made the site a wash.
            // A url is longer than either, so the copy it spends is larger. Only the
            // `None` arm here needs the pool; `trim_url_raw`'s `Borrowed` arm is the
            // document's own bytes and would take the span form directly.
            if span.end_usize() <= self.source.len() {
                let raw = span.extract(self.source);
                return match crate::url::trim_url_raw(raw) {
                    Some(trimmed) => d.text_pooled(&trimmed),
                    None => d.text_pooled(raw),
                };
            }
            // Fallback (span unavailable): rejoin args with no space after commas.
            let args_doc = d.join(args.iter().map(|arg| self.build_css_value_doc(arg)), ",");
            return self.flat_function_doc(name_span, args_doc);
        }

        // No argument came out of the region between the parens, which happens two ways.
        //
        // The region held nothing but whitespace — `parse_function_arguments` trims it away
        // with the same `trim_start_css` / `trim_end_preserving_escape` pair `escapes::trim_css`
        // composes, so the parser's verdict and the test below cannot disagree — and there is
        // then nothing to print, so the call closes flush: `f(  )` → `f()`,
        // `layer(  )` → `layer()`, and the nameless group `(  )` → `()`. That is the same
        // gap collapse every other value interior takes, and it is one rule for all three
        // (before the group parsed, its collapse came from the identifier path's whitespace
        // normalizer instead).
        //
        // Or the region was never parsed at all: an `@import` prelude consumes an unknown
        // function's argument opaquely (`scope((.a) to (.b))` — `preludes::consume_function_args`),
        // leaving real content behind no argument. That text is the only record of it, so it
        // stays verbatim.
        if args.is_empty() && span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            if let Some(interior) = function_paren_interior(raw, name_span, span)
                && crate::escapes::trim_css(interior).is_empty()
            {
                return self.flat_function_doc(name_span, d.text(""));
            }
            return d.text_pooled(raw);
        }

        // A comma **closing** the argument list (`rgb(1, 2, 3,)`, `var(--a,)`,
        // `linear-gradient(red, ,)`) terminated no argument, so joining the args would
        // drop it. `extract_function_parts` requires the closing paren to be the value's
        // last byte, which is what bounds the list at `span.end - 1`.
        let closing_comma = super::declarations::list_has_closing_comma(
            self.source,
            args,
            span.end_usize().saturating_sub(1),
        );

        // var()'s empty fallback (`var(--a,)`, `var(--a, ,)`) is kept flat: the generic
        // path below spells the same closing comma, but wraps the argument list in a
        // breakable group, and prettier never breaks a `var()`. `var(--a, red)` has no
        // closing comma and takes the generic path with the normal `, ` separator.
        if closing_comma && self.function_name_is(name_span, "var") {
            let args_doc = d.join(args.iter().map(|arg| self.build_css_value_doc(arg)), ", ");
            let comma = d.text(",");
            return self.flat_function_doc(name_span, d.concat(&[args_doc, comma]));
        }

        // Build with group/softline structure for automatic wrapping
        // Structure: name(
        //   arg1,
        //   arg2,
        //   arg3
        // )
        // When flat: name(arg1, arg2, arg3)
        let mut inner_parts = DocBuf::new();
        for (i, arg) in args.iter().enumerate() {
            // For List args (space-separated values like calc math expressions),
            // use fill with line() separators so content can break at operators.
            // Matches prettier's group(indent(fill(parts))) pattern.
            if let CssValue::List { values, .. } = arg {
                inner_parts.push(self.build_space_fill_value_doc(values, ctx));
            } else {
                inner_parts.push(self.build_css_value_doc_in(arg, ctx));
            }
            if i < args.len() - 1 {
                inner_parts.push(d.text(","));
                inner_parts.push(d.line()); // space when flat, newline when broken
            }
        }
        // The closing comma the args' own separators can't spell: `red, ` is a ONE-argument
        // list, so `linear-gradient(red,,)` would lose its empty argument — and with it the
        // reason the UA drops the declaration — while `rgb(1, 2, 3,)` would lose the comma
        // that makes it invalid in the first place. `var()` takes its own flat form above.
        // The comma joins the last argument rather than standing as its own part, so the
        // group's break can never strand it on a line of its own.
        if closing_comma && let Some(last) = inner_parts.pop() {
            inner_parts.push(d.concat(&[last, d.text(",")]));
        }

        let name_doc = d.source_span(name_span, self.source);
        let inner = d.concat(&inner_parts);
        d.group(d.concat(&[
            name_doc,
            d.text("("),
            d.indent(d.concat(&[d.softline(), inner])),
            d.softline(),
            d.text(")"),
        ]))
    }

    /// Build a doc for space-separated values inside a function argument
    ///
    /// Uses fill with line() separators so the renderer can break at space boundaries
    /// when content exceeds print width. Wrapped in group(indent(fill(...))) to match
    /// prettier's CSS value group pattern — continuation lines get extra indent.
    ///
    /// Example: `calc(0.5 * (100vw - var(--a)))` breaks as:
    /// ```text
    /// calc(
    ///   0.5 *
    ///     (100vw - var(--a))
    /// )
    /// ```
    pub(super) fn build_space_fill_value_doc(
        &self,
        values: &[CssValue<'_>],
        ctx: ValueCtx,
    ) -> DocId {
        let d = self.d();
        let parts = self.build_space_fill_parts(values, ctx);
        d.group(d.indent(d.fill(&parts)))
    }

    /// Is the function name at `name_span` — read **verbatim** from the source — the
    /// keyword `kw`?
    ///
    /// Function names are ASCII case-insensitive ("like keywords, function names are
    /// ASCII case-insensitive" — css-values-4 §"Functional Notations"), and a name may be
    /// escape-spelled: `\75 rl(` is a `url()`. The value subtree stores the author's own
    /// bytes (`CssValue::Function::name_span`), so recognition — not emission — is where
    /// an escape is resolved.
    ///
    /// Two byte tests answer every name a stylesheet really holds, which is what keeps
    /// the escape walk off this path (this question is asked of every function value):
    ///
    /// - **An escape spends at least two bytes on the one character it spells**, so a
    ///   decoded spelling of `kw` is always LONGER than `kw` — an exact-length name is
    ///   answered by the bytes alone, and 82% of names are three bytes.
    /// - **A longer name's first decoded character is either its first byte verbatim or
    ///   an escape's `\`**, so a first byte that is neither `\` nor `kw`'s own opening
    ///   character cannot spell `kw` however it decodes — `calc(` refuses `url` on one
    ///   compare.
    ///
    /// What survives both is a LONGER name whose first byte could still begin `kw`, which
    /// since the value classifier reads postcss's word rather than the spec's ident sequence
    /// (`parser::value::is_function_name`) is two things rather than one: an escape-spelled
    /// name, from an `@import` prelude or from that classifier, and a name carrying the
    /// punctuation a word admits (`u/rl`, `url/`). Only the first can match, and the walk
    /// below is where that is settled — it requires a `\` in the name before it decodes
    /// anything, so a punctuation-bearing name refuses on one `memchr` and `url/(a.png)`
    /// stays the generic function prettier also prints. No corpus holds either.
    #[inline]
    fn function_name_is(&self, name_span: Span, kw: &str) -> bool {
        let name = &self.source.as_bytes()[name_span.range()];
        let kw = kw.as_bytes();
        if name.len() == kw.len() {
            return name.eq_ignore_ascii_case(kw);
        }
        name.len() > kw.len()
            && (name[0].eq_ignore_ascii_case(&kw[0]) || name[0] == b'\\')
            && self.escaped_function_name_is(name_span, kw)
    }

    /// The escaped tail of `function_name_is` — outlined and cold, so the escape walk it
    /// runs stays off every function value's path. The walk compares the name as it
    /// decodes (`escapes::decodes_to_ascii_ignore_case`) rather than decoding it into a
    /// `String` first: the same verdict, with nothing allocated on a path no corpus reaches.
    #[cold]
    #[inline(never)]
    fn escaped_function_name_is(&self, name_span: Span, kw: &[u8]) -> bool {
        let name = name_span.extract(self.source);
        name.as_bytes().contains(&b'\\') && crate::escapes::decodes_to_ascii_ignore_case(name, kw)
    }

    /// The members of a space-separated value list, each gap decided on its own, with
    /// `separator` standing in every gap that takes one.
    ///
    /// **One walk for both shapes**: a flat list spends a `" "` and a wrapping fill spends
    /// a `line`, and that is the whole of the difference. Spelled twice, the two would
    /// drift on which gaps exist — the hazard the call-argument printers already paid for
    /// — and a gap that is a separator in one and glue in the other is a formatter that
    /// contradicts itself across the print-width boundary.
    ///
    /// A glued gap contributes **no part at all**, which is what keeps an operator the
    /// author glued from becoming a wrap point: `1.5/2.5` is three members and one fill
    /// item.
    fn build_value_member_parts(
        &self,
        values: &[CssValue<'_>],
        ctx: ValueCtx,
        separator: DocId,
    ) -> DocBuf {
        let mut parts = DocBuf::with_capacity(values.len() * 2);
        for (i, value) in values.iter().enumerate() {
            parts.push(self.build_css_value_doc_in(value, ctx));
            if i + 1 < values.len() && !self.value_gap_is_glued(values, i, ctx) {
                parts.push(separator);
            }
        }
        parts
    }

    /// Build a doc for a space-separated value list — the **flat** spelling of
    /// [`Self::build_value_member_parts`].
    pub(super) fn build_value_list_doc(&self, values: &[CssValue<'_>], ctx: ValueCtx) -> DocId {
        let d = self.d();
        let parts = self.build_value_member_parts(values, ctx, d.text(" "));
        d.concat(&parts)
    }

    /// The same members as `line`-separated fill parts — the **wrapping** spelling.
    ///
    /// `[val1, line, val2, line, val3]`, suitable for `DocArena::fill`, used by both
    /// declaration wrapping and function-argument wrapping.
    pub(super) fn build_space_fill_parts(&self, values: &[CssValue<'_>], ctx: ValueCtx) -> DocBuf {
        let d = self.d();
        self.build_value_member_parts(values, ctx, d.line())
    }

    /// Does the gap between members `i` and `i + 1` take **no** separator?
    ///
    /// prettier's `printCommaSeparatedValueGroup` loop, transcribed for the `css` parser:
    /// every arm below is one of its `continue`s, in its order, and anything that reaches
    /// the end takes the default `line`. Its SCSS/Less arms (interpolation, at-words,
    /// control directives, `~`, property lookups) are dropped — none of those nodes can
    /// exist here — as is the `url()` arm, whose interior tsv keeps opaque, and the
    /// `grid` arm, which tsv answers with its own row plan (`grid_multirow_plan`).
    ///
    /// ⚠️ One deliberate divergence, and it is in [`authored_glued`]: prettier asks
    /// `hasEmptyRawBefore`, a field its parser never sets on a synthesized
    /// `value-paren_group`, so an operator before a group can only ever read as
    /// un-glued there. tsv asks the document instead, so `1.5/(2.5)` keeps the glue
    /// `(1.5)/2.5` already keeps. See `docs/conformance_prettier_css.md` §CSS: Values.
    pub(super) fn value_gap_is_glued(
        &self,
        values: &[CssValue<'_>],
        i: usize,
        ctx: ValueCtx,
    ) -> bool {
        let source = self.source;
        let current = &values[i];
        let Some(next) = values.get(i + 1) else {
            return true;
        };
        let previous = i.checked_sub(1).map(|p| &values[p]);
        let next_next = values.get(i + 2);

        let current_op = ValueOperator::of(current, source);
        let next_op = ValueOperator::of(next, source);

        // "Ignore colon": the loop contributes no separator on either side of a `:`, and
        // prettier's `value-colon` node then prints `[":", line]` — so the space belongs
        // to the colon, not to the gap. ⚠️ The two arms are ORDERED: read the other way
        // round, a `:` followed by a `:` (`f(a::b)` → `f(a: : b)`) would lose the first
        // one's space to the second one's "nothing before a colon".
        if current_op == Some(ValueOperator::Colon) {
            return false;
        }
        if next_op == Some(ValueOperator::Colon) {
            return true;
        }

        // A `/` with nothing before it binds to what follows (prettier's
        // `!iPrevNode && isDivisionNode(iNode)`, written for `-fb-url(/abs/path/)`).
        if previous.is_none() && current_op == Some(ValueOperator::Division) {
            return true;
        }

        let is_word = |v: Option<&CssValue<'_>>| v.is_some_and(is_word_member);
        let is_func = |v: Option<&CssValue<'_>>| v.is_some_and(|v| is_func_member(v, source));
        let require_space_before = is_func(next_next)
            || is_word(next_next)
            || is_func(Some(current))
            || is_word(Some(current));
        let require_space_after =
            is_func(Some(next)) || is_word(Some(next)) || is_func(previous) || is_word(previous);

        // "Ignore Tailwind `@utility` directive": a word followed by `*` is glued there,
        // which is the one place a `*` takes no space (`--value(--tab-size-*)`).
        if ctx.scope.in_utility_at_rule
            && next_op == Some(ValueOperator::Multiplication)
            && is_word_member(current)
        {
            return true;
        }

        // "Formatting `/`, `+`, `-` sign". A `*` on either side takes the rule out, and so
        // does `calc()`, whose own arm follows.
        let beside_multiplication = current_op == Some(ValueOperator::Multiplication)
            || next_op == Some(ValueOperator::Multiplication);
        let sign_keeps_its_gap = (next_op == Some(ValueOperator::Division)
            && !require_space_before)
            || (current_op == Some(ValueOperator::Division) && !require_space_after)
            || (next_op == Some(ValueOperator::Addition) && !require_space_before)
            || (current_op == Some(ValueOperator::Addition) && !require_space_after)
            || next_op == Some(ValueOperator::Subtraction)
            || current_op == Some(ValueOperator::Subtraction);
        // A run of operators is glued from its first onward even where the author spaced
        // it — prettier's second disjunct, which reads the operator's own neighbours.
        let glued = authored_glued(current, next)
            || (current_op.is_some()
                && previous.is_none_or(|p| ValueOperator::of(p, source).is_some()));
        if !beside_multiplication && !ctx.in_calc && sign_keeps_its_gap && glued {
            return true;
        }

        // Inside `calc()` a `+` / `-` gap prints exactly as authored: css-values-4
        // requires whitespace on both sides of those two, so the gap is the difference
        // between a valid expression and an invalid one and is not a formatter's to move.
        if ctx.in_calc
            && (current_op.is_some_and(ValueOperator::is_sum)
                || next_op.is_some_and(ValueOperator::is_sum))
            && authored_glued(current, next)
        {
            return true;
        }

        // The `font` shorthand and a custom property keep a glued `/` beside a font-size
        // operand (prettier's §"Formatting `font` property"). ⚠️ The second arm reads the
        // gap on the operator's OWN left, not this gap.
        if ctx.scope.font_shorthand {
            if next_op == Some(ValueOperator::Division)
                && authored_glued(current, next)
                && is_possible_font_size(current, source)
            {
                return true;
            }
            if current_op == Some(ValueOperator::Division)
                && previous.is_some_and(|p| authored_glued(p, current))
                && previous.is_some_and(|p| is_possible_font_size(p, source))
            {
                return true;
            }
        }

        false
    }

    /// Build a doc for a value list joined by `sep` — `", "` for a comma-separated list
    /// (`CssValue::CommaSeparated`), `" "` for the `url()` string-plus-comment region.
    pub(crate) fn build_separated_values_doc(
        &self,
        values: &[CssValue<'_>],
        sep: &'static str,
    ) -> DocId {
        self.d()
            .join(values.iter().map(|v| self.build_css_value_doc(v)), sep)
    }
}

/// The text between a function's parentheses, read off `raw` — the function's own source
/// slice (`span`), whose last byte the value parser guarantees is the closing `)`.
///
/// The opening paren is found past the **name**, never from the start of `raw`: a name may
/// carry an escaped one (`a\(b(…)` is refused as a name, but `a\28 b(…)` is the name `a(b`),
/// and a `(` inside the name opens no argument list. `None` when the shape isn't
/// `<name>(<interior>)` at all, which is the defensive case the sole caller keeps verbatim.
fn function_paren_interior(raw: &str, name_span: Span, span: Span) -> Option<&str> {
    let after_name = raw.get(name_span.end_usize().checked_sub(span.start_usize())?..)?;
    after_name
        .strip_suffix(')')?
        .split_once('(')
        .map(|(_, i)| i)
}
