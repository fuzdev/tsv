// Literal value printing for TypeScript
//
// This module handles all primitive value types:
// - Numbers (with normalization: hex lowercase, scientific notation, etc.)
// - Strings (quote selection and escaping)
// - Booleans, null, undefined
// - Identifiers (with optional markers and type annotations)
// - Regex literals
// - Spread elements

use crate::ast::internal::{self, LiteralValue};
use crate::printer::Printer;
use crate::printer::decorators::DecoratorHost;
use smallvec::{SmallVec, smallvec};
use std::borrow::Cow;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::{format_string_literal, is_width_relevant, optimal_string_quote_in};

/// Does this string literal print as its own source slice — and if so, is that
/// slice one column a byte?
///
/// `Some(plain)` when the optimal quote is the one already in source: re-quoting
/// would re-emit the content verbatim between the same quotes, i.e. exactly
/// `literal.span`, so the printed text IS that span and `plain` is its width claim
/// (see `DocArena::source_span_plain`). `None` when the quote swaps and the text
/// has to be rebuilt by [`rebuild_string_literal`].
///
/// ⭐ **One function because the two callers ask one question.** The printer emits
/// the span or the rebuilt text; `is_short_arg` wants the printed length. Both
/// answers — and the width one the printer needs next — come out of a single pass
/// over the document's own words ([`optimal_string_quote_in`], whose doc carries
/// why the width answer is free there).
fn string_literal_prints_verbatim(literal: &internal::Literal<'_>, source: &str) -> Option<bool> {
    debug_assert!(
        matches!(&literal.value, LiteralValue::String(_)),
        "string_literal_prints_verbatim called on non-string literal"
    );
    // A string literal's source slice always includes both quote delimiters, so the
    // span is at least two bytes and the content is it minus one byte at each end.
    let (optimal, plain) = optimal_string_quote_in(
        source,
        literal.span.start as usize + 1,
        literal.span.end as usize - 1,
    );
    // The quote char is recovered from source (the byte at the span start) rather
    // than stored on the literal.
    (optimal == literal.string_quote(source) as char).then_some(plain)
}

/// The printed text of a string literal whose quote swaps — the `None` arm of
/// [`string_literal_prints_verbatim`], and the only path that allocates.
fn rebuild_string_literal(literal: &internal::Literal<'_>, source: &str) -> String {
    let raw_literal = literal.span.extract(source);
    format_string_literal(
        &raw_literal[1..raw_literal.len() - 1],
        literal.string_quote(source) as char,
    )
}

/// Format a string literal from the AST to its printed form.
///
/// The `Cow` face of [`string_literal_prints_verbatim`], for the caller that wants
/// the printed *text* (its length, in `is_short_arg`) rather than a doc node.
pub(crate) fn format_string_literal_from_ast<'s>(
    literal: &internal::Literal<'_>,
    source: &'s str,
) -> Cow<'s, str> {
    match string_literal_prints_verbatim(literal, source) {
        Some(_) => Cow::Borrowed(literal.span.extract(source)),
        None => Cow::Owned(rebuild_string_literal(literal, source)),
    }
}

/// Format a directive prologue string (`'use strict'`) for printing.
///
/// Mirrors Prettier's `printDirective` (`language-js/print/literal.js`), which is
/// deliberately distinct from `printString`: a directive is an exact code-unit
/// sequence, so its escapes are never re-encoded. Only the *outer* quote is
/// swapped to the preferred style (single) — and only when the content holds no
/// quote of either kind. If the content contains a `'` or `"`, the raw literal is
/// preserved verbatim (swapping would force re-escaping, changing the directive;
/// see prettier#1555).
///
/// `raw` is the literal *with* its surrounding quotes (the source slice).
pub(crate) fn format_directive(raw: &str) -> String {
    // `raw` is the directive literal with its surrounding quotes (≥2 bytes), so
    // stripping one quote from each end is in bounds.
    let content = &raw[1..raw.len() - 1];
    if content.contains('\'') || content.contains('"') {
        raw.to_string()
    } else {
        // Preferred quote is single (matches `format_string_literal`'s hardcoded tie-breaker).
        format!("'{content}'")
    }
}

/// Normalize a number literal to match Prettier's output format.
///
/// Mirrors Prettier's `printNumber` (`src/utilities/print-number.js`) so that
/// numerically-equal literals collapse to one canonical spelling:
/// - Lowercase everything: `0xFF` → `0xff`, `2E10` → `2e10`
/// - Strip `+` and leading zeros from the exponent: `1e+1` → `1e1`, `1.1e0010` → `1.1e10`
/// - Drop a zero exponent: `0.5e0` → `0.5`
/// - Leading decimal gets a zero: `.5` → `0.5`
/// - Strip trailing fractional zeros: `1.00500` → `1.005`
/// - Drop a trailing dot: `5.` → `5`, `1.e1` → `1e1`
///
/// BigInt literals are only lowercased (Prettier's `printBigInt`); the BigInt
/// grammar has no exponent or fraction, so the rest of the pipeline is inert.
pub fn normalize_number_literal(raw: &str) -> Cow<'_, str> {
    // Prettier short-circuits single-character literals (`0`, `1`) — a numeric
    // literal's only single-char form is one ASCII digit, already canonical, so
    // borrow (`len == 1` is the ASCII-fast equivalent of `chars().count() == 1`).
    if raw.len() == 1 {
        return Cow::Borrowed(raw);
    }

    // BigInt (`printBigInt`): lowercase only, suffix included (`0xFFn` → `0xffn`).
    if raw.ends_with('n') {
        return lower_ascii_cow(raw);
    }

    print_number(raw)
}

/// Lowercase `s` only when it holds an uppercase ASCII byte; otherwise borrow it
/// unchanged. This is the one unconditional allocation in the number pipeline's
/// common path, so borrowing the already-lowercase case is what lets a canonical
/// literal flow through to a `Cow::Borrowed` end-to-end.
fn lower_ascii_cow(s: &str) -> Cow<'_, str> {
    if s.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(s.to_ascii_lowercase())
    } else {
        Cow::Borrowed(s)
    }
}

/// Port of Prettier's `printNumber` regex pipeline (order matters). Each stage
/// threads a `Cow`: a stage that makes no change passes its input straight
/// through, so a literal already in canonical form returns `Cow::Borrowed` end to
/// end (no allocation); the first rewriting stage switches it to `Cow::Owned`.
fn print_number(raw: &str) -> Cow<'_, str> {
    let s = lower_ascii_cow(raw);
    let s = strip_exponent_plus_and_zeros(s);
    let s = strip_zero_exponent(s);
    let s = ensure_leading_digit(s);
    let s = strip_trailing_fraction_zeros(s);
    strip_trailing_dot(s)
}

/// `/^([+-]?[\d.]+e)(?:\+|(-))?0*(?=\d)/` → `$1$2`
/// Removes a `+` and any leading zeros from the exponent (keeps a `-`).
fn strip_exponent_plus_and_zeros(s: Cow<'_, str>) -> Cow<'_, str> {
    let bytes = s.as_bytes();
    let mut i = 0;
    // Optional leading sign.
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    // `[\d.]+` (at least one digit-or-dot).
    let mantissa_start = i;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if i == mantissa_start || i >= bytes.len() || bytes[i] != b'e' {
        return s;
    }
    i += 1; // consume 'e'
    let after_e = i; // prefix `[+-]?[\d.]+e` ends here
    // Optional `+` (dropped) or `-` (kept).
    let mut sign = "";
    match bytes.get(i) {
        Some(b'+') => i += 1,
        Some(b'-') => {
            sign = "-";
            i += 1;
        }
        _ => {}
    }
    // Leading zeros, but the lookahead `(?=\d)` requires a following digit.
    while i < bytes.len() && bytes[i] == b'0' {
        i += 1;
    }
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return s;
    }
    // Rebuild: prefix through 'e', kept sign, then the remaining digits.
    Cow::Owned(format!("{}{}{}", &s[..after_e], sign, &s[i..]))
}

/// `/^([+-]?[\d.]+)e[+-]?0+$/` → `$1`  (removes a whole zero exponent: `0.5e0` → `0.5`)
//
// The `e`/`.` scans through this number-normalization pipeline use a byte
// `.position()` rather than `str::find(char)`: the haystack is a short numeric
// literal, where `find(char)`'s `CharSearcher` setup dominates and never
// amortizes (`e`/`.` are ASCII, so byte-position ≡ char-position).
fn strip_zero_exponent(s: Cow<'_, str>) -> Cow<'_, str> {
    let Some(e_idx) = s.as_bytes().iter().position(|&b| b == b'e') else {
        return s;
    };
    let mantissa = &s[..e_idx];
    let exp = &s[e_idx + 1..];
    if mantissa.is_empty() {
        return s;
    }
    // mantissa must be `[+-]?[\d.]+`
    let m = mantissa.strip_prefix(['+', '-']).unwrap_or(mantissa);
    if m.is_empty() || !m.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return s;
    }
    // exp must be `[+-]?0+`
    let e = exp.strip_prefix(['+', '-']).unwrap_or(exp);
    if !e.is_empty() && e.bytes().all(|b| b == b'0') {
        Cow::Owned(mantissa.to_string())
    } else {
        s
    }
}

/// `/^([+-])?\./` → `$10.`  (`.5` → `0.5`, `-.5` → `-0.5`)
fn ensure_leading_digit(s: Cow<'_, str>) -> Cow<'_, str> {
    if let Some(rest) = s.strip_prefix('.') {
        Cow::Owned(format!("0.{rest}"))
    } else if let Some(rest) = s.strip_prefix("+.") {
        Cow::Owned(format!("+0.{rest}"))
    } else if let Some(rest) = s.strip_prefix("-.") {
        Cow::Owned(format!("-0.{rest}"))
    } else {
        s
    }
}

/// `/(\.\d+?)0+(?=e|$)/` → `$1`  (first match only; `1.00500` → `1.005`, `1.50` → `1.5`)
fn strip_trailing_fraction_zeros(s: Cow<'_, str>) -> Cow<'_, str> {
    let Some(dot) = s.as_bytes().iter().position(|&b| b == b'.') else {
        return s;
    };
    let bytes = s.as_bytes();
    // `\.\d+?` — need at least one digit after the dot.
    let mut i = dot + 1;
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return s;
    }
    // Non-greedy `\d+?` keeps the first digit, then we look for trailing zeros
    // that run up to `e` or end-of-string.
    // Find the end of the fractional digit run.
    let frac_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let frac_end = i; // first non-digit after fraction (could be 'e' or len)
    // Boundary must be `e` or end.
    if frac_end != bytes.len() && bytes[frac_end] != b'e' {
        return s;
    }
    // Strip trailing zeros from [frac_start, frac_end), keeping at least one digit.
    let mut keep = frac_end;
    while keep > frac_start + 1 && bytes[keep - 1] == b'0' {
        keep -= 1;
    }
    if keep == frac_end {
        return s;
    }
    Cow::Owned(format!("{}{}", &s[..keep], &s[frac_end..]))
}

/// `/\.(?=e|$)/` → ``  (drop a trailing dot before `e` or end: `1.` → `1`, `1.e1` → `1e1`)
fn strip_trailing_dot(s: Cow<'_, str>) -> Cow<'_, str> {
    let bytes = s.as_bytes();
    if let Some(dot) = bytes.iter().position(|&b| b == b'.') {
        let after = dot + 1;
        if after == bytes.len() || bytes[after] == b'e' {
            return Cow::Owned(format!("{}{}", &s[..dot], &s[after..]));
        }
    }
    s
}

/// Sort regex flags alphabetically to match Prettier's output format.
///
/// Prettier normalizes regex flags to alphabetical order (dgimsvy).
/// Example: `/pattern/vg` → `/pattern/gv`
pub fn sort_regex_flags(flags: &str) -> String {
    let mut chars: SmallVec<[char; 8]> = flags.chars().collect();
    chars.sort_unstable();
    chars.into_iter().collect()
}

impl<'a> Printer<'a> {
    /// Build a Doc for a literal
    pub(in crate::printer) fn build_literal_doc(&self, literal: &internal::Literal<'_>) -> DocId {
        let d = self.d();
        match &literal.value {
            LiteralValue::Number(_) | LiteralValue::BigInt => {
                self.build_number_literal_doc(literal)
            }
            LiteralValue::String(_) => self.build_string_literal_doc(literal),
            LiteralValue::Boolean(b) => d.text(if *b { "true" } else { "false" }),
            LiteralValue::Null => d.text("null"),
        }
    }

    /// Build a Doc for a numeric or BigInt literal, emitting the verbatim source
    /// slice when `normalize_number_literal` is the identity (the literal is
    /// already canonical — any plain decimal integer, lowercase hex/radix, or
    /// canonical float) and allocating only when normalization rewrites it.
    /// Mirrors [`Self::build_string_literal_doc`].
    pub(in crate::printer) fn build_number_literal_doc(
        &self,
        lit: &internal::Literal<'_>,
    ) -> DocId {
        let raw = lit.span.extract(self.source);
        match normalize_number_literal(raw) {
            // ⭐ A numeric literal is plain ASCII **by grammar**, so the verbatim arm
            // never measures: `NumericLiteral` admits digits, `.`, `_`, `e`/`E`, the
            // radix letters `x`/`o`/`b`, the hex digits and a BigInt `n`, and nothing
            // at or above `0x80` — nor a `\t` or a line terminator, which would end
            // the token. 34,567 of the 157,073 non-name width measures a TypeScript
            // format run makes are one of these (22.0%, mean 1.5 bytes).
            Cow::Borrowed(_) => self.verbatim_literal_doc(lit.span, true),
            Cow::Owned(s) => self.d().text_pooled(&s),
        }
    }

    /// Emit a literal that prints as its own source slice — the doc-side width seam
    /// the number and string literal builders share.
    ///
    /// `plain` is the caller's claim that the slice holds no byte the width depends
    /// on (no `\t`, no `\n`, nothing at or above `0x80`), so its width IS its byte
    /// length and `DocArena::source_span_plain` need not measure it at all. Both
    /// callers know it for free, and neither learns it from a scan of its own: a
    /// number is ASCII by grammar, and a string's quote choice has already read
    /// every content byte (`printing::optimal_string_quote_in`).
    ///
    /// ⚠️ **A wrong claim is a SILENT width error** — nothing downstream re-derives
    /// the width, so it moves a fits verdict and changes no other observable. It is
    /// therefore graded BOTH ways on every verbatim literal in every debug build,
    /// exactly as [`Printer::ident_name_doc`] grades the name channel's: over-claiming
    /// is the silent error, and under-claiming silently retires the lever.
    fn verbatim_literal_doc(&self, span: Span, plain: bool) -> DocId {
        debug_assert_eq!(
            plain,
            !span.extract(self.source).bytes().any(is_width_relevant),
            "a verbatim literal's `plain` claim disagrees with its own bytes"
        );
        let d = self.d();
        if plain {
            d.source_span_plain(span)
        } else {
            d.source_span(span, self.source)
        }
    }

    /// Build a Doc for a string literal, emitting the verbatim source slice when
    /// the formatter wouldn't change it (no quote swap) and allocating only on the
    /// quote-swap path. Shared by string literal *values* and quoted object /
    /// import-attribute *keys*.
    pub(in crate::printer) fn build_string_literal_doc(
        &self,
        lit: &internal::Literal<'_>,
    ) -> DocId {
        match string_literal_prints_verbatim(lit, self.source) {
            Some(plain) => self.verbatim_literal_doc(lit.span, plain),
            None => self
                .d()
                .text_pooled(&rebuild_string_literal(lit, self.source)),
        }
    }

    /// Build a Doc for a private identifier
    pub(super) fn build_private_identifier_doc(
        &self,
        pid: &internal::PrivateIdentifier<'_>,
    ) -> DocId {
        let d = self.d();
        d.concat(&[
            d.text("#"),
            self.ident_name_doc(pid.name, pid.name_span().start),
        ])
    }

    /// Build a Doc for an identifier
    pub(in crate::printer) fn build_identifier_doc(&self, id: &internal::Identifier<'_>) -> DocId {
        self.build_identifier_doc_inner(id, false, false)
    }

    /// Build a Doc for an identifier with wrapping type arguments.
    ///
    /// Used in variable declarations where TypeReference type arguments should
    /// break internally (e.g., `let x: Map<LongA, LongB>` breaks inside `<>`).
    pub(in crate::printer) fn build_identifier_doc_with_wrapping_type(
        &self,
        id: &internal::Identifier<'_>,
    ) -> DocId {
        self.build_identifier_doc_inner(id, true, false)
    }

    /// Build an identifier param doc **without** its parameter decorators — used
    /// by the `TSParameterProperty` printer, which renders the decorators before
    /// the accessibility/`readonly` modifiers (`@dec private x`) even though acorn
    /// stores them on the inner identifier.
    pub(in crate::printer) fn build_identifier_doc_no_decorators(
        &self,
        id: &internal::Identifier<'_>,
    ) -> DocId {
        self.build_identifier_doc_inner(id, false, true)
    }

    /// Inner implementation for identifier doc building.
    fn build_identifier_doc_inner(
        &self,
        id: &internal::Identifier<'_>,
        wrap_type_args: bool,
        skip_decorators: bool,
    ) -> DocId {
        let d = self.d();

        // Fast path for the common bare identifier (no decorators, optional
        // marker, or type annotation): the doc is just the interned name.
        // Skips the single-element `parts` Vec the slow path heap-allocates and
        // the name-end scan that only the modifier/annotation branches consume.
        // Returns the exact DocId the `parts.len() == 1` tail would.
        let render_decorators = !skip_decorators && id.decorators().is_some();
        if !render_decorators && !id.optional && id.type_annotation().is_none() {
            return self.identifier_name_doc(id);
        }

        let mut parts = DocBuf::new();

        // The name plus any optional marker (`a?` in `function fn(a?: number) {}`),
        // preserving comments in the name→`?` gap (`a /* c */?`), and returning the
        // left edge of the annotation gap below.
        let after_modifier = self.push_identifier_head_doc(&mut parts, id);

        // Type annotation, handling a before-`:` comment between the name (and any
        // `?`) and `:` — line → indented continuation, block → inline before `:`.
        if let Some(type_annotation) = id.type_annotation() {
            parts.push(self.build_binding_type_annotation_doc(
                after_modifier,
                type_annotation,
                wrap_type_args,
            ));
        }

        // `concat` short-circuits a single part (just the name) to that part.
        let inner = d.concat(&parts);

        // Prefix parameter decorators (own-line in source → each on its own line and
        // the parameter list expands; inline → a single space), preserving any
        // comment authored between a decorator and the binding (`@dec /* c */ x`).
        // `id.span.start` is the name — the boundary for that after-decorator scan,
        // since acorn stores the decorators before it.
        if render_decorators && let Some(decorators) = id.decorators() {
            self.with_param_decorators(Some(decorators), inner, id.span.start, DecoratorHost::Plain)
        } else {
            inner
        }
    }

    /// Build a Doc for a regex literal
    /// Flags are sorted alphabetically to match prettier's output.
    pub(super) fn build_regex_doc(&self, regex: &internal::RegexLiteral) -> DocId {
        let mut w = self.d().pool_writer();
        w.push('/');
        w.push_str(regex.pattern(self.source));
        w.push('/');
        w.push_str(&sort_regex_flags(regex.flags(self.source)));
        w.finish_text()
    }

    /// Build a Doc for a spread element
    pub(in crate::printer) fn build_spread_doc(
        &self,
        spread: &internal::SpreadElement<'_>,
    ) -> DocId {
        let d = self.d();
        let needs_parens = self.needs_parens(spread.argument, super::ParenContext::SpreadArgument);
        // Check for comments between `...` and the argument (e.g., `.../* comment */ arr`)
        let dots_end = spread.span.start + "...".len() as u32;
        let arg_start = spread.argument.span().start;

        // Check for trailing comments from stripped grouping parens: `...(x /* c */)`
        let argument_end = spread.argument.span().end;
        let has_trailing_comments =
            self.has_comments_to_emit_between(argument_end, spread.span.end);

        // The `...`→argument value head ([`Printer::value_head_frozen_span`]): an own-line
        // directive in the gap freezes the whole argument.
        //
        // It returns EARLY, ahead of both layouts below, because both glue the gap's run to a
        // delimiter — the breaking arm to the `...` itself, the hug to the `...` or to the
        // shell's `(` — and a directive on a delimiter's line is inert under tsv's floor. Pass
        // 1 would print the frozen bytes with the directive welded to `...`, pass 2 would read
        // no freeze and normalize them: a silent loss with nothing dropped and no gate firing.
        // So an honored directive keeps its own line and the argument hangs below it, through
        // the uniform forced-continuation indent — the same answer `await`→operand and
        // `new`→callee give their own keyword gaps, and the same divergence: prettier pulls
        // the directive up onto the `...` line and honors it there anyway.
        //
        // Parens the argument REQUIRES are the printer's, not the author's, so they ride
        // OUTSIDE the slice — and the slice→`)` gap is answered by the shell's own trailing
        // emitter rather than a second pair open-coded here (`docs/comments.md` hazard 4).
        if let Some(frozen) = self.value_head_frozen_span(dots_end, spread.argument.span()) {
            let mut inner: DocBuf = DocBuf::new();
            if needs_parens {
                inner.push(d.text("("));
            }
            inner.push(self.build_frozen_expression_doc(spread.argument, frozen));
            if has_trailing_comments {
                self.append_spread_trailing_paren_comments(
                    &mut inner,
                    argument_end,
                    spread.span.end,
                );
            }
            if needs_parens {
                inner.push(d.text(")"));
            }
            let value = d.concat(&inner);
            return d.concat(&[
                d.text("..."),
                self.build_continuation_indent(dots_end, arg_start, value),
            ]);
        }

        // A binaryish spread argument indents its continuation when it breaks
        // (`...(a &&\n\tb && {…})`), matching Prettier — a `SpreadElement` parent is
        // not in binaryish.js's `shouldNotIndent` set, so the logical/binary chain
        // gets the continuation indent. Non-binary arguments are unaffected.
        let arg_doc = self.build_expression_doc_with_indent_on_break(spread.argument);

        // A block run the author broke AFTER, before an argument that WILL BREAK,
        // keeps its break — prettier's `printLeadingComment` newline-after `line`,
        // materialized by the argument's own break (`.../*#__PURE__*/⏎fn({…})` stays
        // broken, the argument opening un-indented at the `...`'s level). The gate is
        // the shared `breaking_value_leading_run` (geometry + `will_break` + the
        // owned-comment decline); an argument that FITS declines into the glued path
        // below, whose flat render is the same bytes prettier's collapsed `line`
        // produces. An argument that NEEDS parens keeps the glued path too — the compound
        // is unprobed, and the hug is that path's existing answer there (the run is emitted
        // outside the pair on both paths, so only the break survives as the difference).
        if !needs_parens
            && let Some((run, arg_doc)) =
                self.breaking_value_leading_run(dots_end, arg_start, || arg_doc)
        {
            let mut parts = smallvec![d.text("...")];
            self.push_leading_run_before_breaking_value(&mut parts, &run, arg_start);
            parts.push(arg_doc);
            if has_trailing_comments {
                self.append_spread_trailing_paren_comments(
                    &mut parts,
                    argument_end,
                    spread.span.end,
                );
            }
            return d.concat(&parts);
        }

        // Use trailing_space variant: `.../* comment */ arg` (space after comment, not before).
        // A single-line block glued to `...` hugs the argument even across a source
        // newline (`.../* c */⏎arg` → `.../* c */ arg`), matching prettier — the
        // argument that would BREAK took the gate above; only the fits case reaches
        // this hug.
        let comment_doc = self.build_rhs_comments_glued_opt(dots_end, arg_start);

        // The run is emitted OUTSIDE any parens the argument needs — the gap belongs to the
        // `...`, not to a pair the printer must emit, so it answers one way whether or not
        // the argument's precedence happens to require one. That is `await`→operand's rule
        // verbatim, and opening the shell first breaks it: it moves a comment
        // the author wrote BEFORE the `(` to after it (`.../* c */ (a ?? b)` →
        // `...(/* c */ a ?? b)`), re-associating it from the spread to the operand. A comment
        // the author wrote INSIDE those parens is glued to the argument and therefore owned,
        // so it never reaches this axis and keeps its place (the `grouped_operand_comment`
        // divergence).
        let mut parts = smallvec![d.text("...")];
        if let Some(c) = comment_doc {
            parts.push(c);
        }
        if needs_parens {
            parts.push(d.text("("));
        }
        parts.push(arg_doc);
        if has_trailing_comments {
            // Handle same-line block comments and line comments here.
            // Own-line block comments are skipped — they're handled by the parent
            // (array/call) which places them as siblings after the spread's comma.
            // Using line_suffix for own-line block comments in spread causes them to
            // escape past the enclosing array/call brackets entirely.
            self.append_spread_trailing_paren_comments(&mut parts, argument_end, spread.span.end);
        }
        if needs_parens {
            parts.push(d.text(")"));
        }
        d.concat(&parts)
    }
}

/// Check if a string is an identifier prettier would output unquoted as an object key.
///
/// This predicts prettier's quote-stripping (`is-es5-identifier-name`), which is a
/// distinct question from "is this a valid ECMAScript identifier?" — so it is kept
/// **off** the lexer's `ID_Start`/`ID_Continue` grammar (`lexer::ident`). The two
/// diverge only on rare grandfathered/NFKC code points (e.g. U+309B, an
/// `Other_ID_Start` sound mark prettier keeps quoted but the lexer accepts as an
/// identifier). Staying on the `XID_Start`/`XID_Continue` subset keeps key
/// formatting independent of the parser's identifier set and is sound for
/// idempotency (`XID ⊆ ID`, so any key we unquote still re-lexes). Reserved words
/// count as identifiers here (prettier outputs them unquoted).
//
// NOTE: prettier's actual rule is ES5 `UnicodeLetter` (general-category based), which
// is neither `XID` nor modern `ID` — matching it exactly needs Unicode general-category
// data we don't carry. `XID` is the closest dependency-free approximation.
pub(in crate::printer) fn is_valid_js_identifier(s: &str) -> bool {
    use unicode_ident::{is_xid_continue, is_xid_start};

    let mut chars = s.chars();

    match chars.next() {
        Some(c) if is_xid_start(c) || c == '_' || c == '$' => {}
        _ => return false,
    }

    chars.all(|c| is_xid_continue(c) || c == '$')
}

/// Check if an identifier name matches Prettier's `isFactory`: `/^[A-Z]|^[$_]+$/u`
/// (member-chain.js:273) — starts uppercase (`Object`, `React`) or is a pure
/// `$`/`_` name (`$`, `_`, `$__`). Drives chain factory-merge decisions; `$util`
/// / `_helper` are NOT factories.
pub(in crate::printer) fn is_factory_identifier_name(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
        || (!name.is_empty() && name.chars().all(|c| c == '$' || c == '_'))
}

#[cfg(test)]
mod tests {
    use super::format_directive as fd;
    use super::normalize_number_literal as norm;

    #[test]
    fn directive_swaps_outer_quote_to_single() {
        // Non-preferred (double) quote normalizes to single...
        assert_eq!(fd("\"use strict\""), "'use strict'");
        assert_eq!(fd("\"use asm\""), "'use asm'");
        // ...and a single-quoted directive is already canonical.
        assert_eq!(fd("'use strict'"), "'use strict'");
        // Inner escapes are preserved exactly — only the outer quote swaps.
        assert_eq!(fd("\"a\\nb\""), "'a\\nb'");
    }

    #[test]
    fn directive_kept_verbatim_when_content_has_a_quote() {
        // Content holds a `"`/`'` → verbatim (swapping would require re-escaping,
        // which would change an exact code-unit sequence).
        assert_eq!(fd("\"\\\"\""), "\"\\\"\""); // "\"" stays double
        assert_eq!(fd("'\\''"), "'\\''"); // '\'' stays single
        assert_eq!(fd("'a\"b'"), "'a\"b'"); // single-quoted, inner " → no swap
    }

    #[test]
    fn keeps_single_char_and_simple() {
        assert_eq!(norm("0"), "0");
        assert_eq!(norm("5"), "5");
        assert_eq!(norm("123"), "123");
        assert_eq!(norm("1.0"), "1.0"); // a lone fractional zero is kept
    }

    #[test]
    fn exponent_plus_and_leading_zeros() {
        assert_eq!(norm("2E+10"), "2e10");
        assert_eq!(norm("1e+1"), "1e1");
        assert_eq!(norm("1.1e0010"), "1.1e10");
        assert_eq!(norm("1e-05"), "1e-5"); // negative sign kept, zeros stripped
        assert_eq!(norm(".1e+0010"), "0.1e10");
    }

    #[test]
    fn zero_exponent_dropped() {
        assert_eq!(norm("0.5e0"), "0.5");
        assert_eq!(norm("1e0"), "1");
        assert_eq!(norm("1e-0"), "1");
    }

    #[test]
    fn leading_and_trailing_dot() {
        assert_eq!(norm(".5"), "0.5");
        assert_eq!(norm("-.5"), "-0.5");
        assert_eq!(norm("5."), "5");
        assert_eq!(norm("1.e1"), "1e1");
    }

    #[test]
    fn trailing_fraction_zeros() {
        assert_eq!(norm("1.00500"), "1.005");
        assert_eq!(norm("1.50"), "1.5");
        assert_eq!(norm("0.0000"), "0.0");
        assert_eq!(norm("500600.001230045000"), "500600.001230045");
    }

    #[test]
    fn radix_literals_only_lowercased() {
        assert_eq!(norm("0xFF"), "0xff");
        assert_eq!(norm("0xE5"), "0xe5"); // the 'e' is a hex digit, not an exponent
        assert_eq!(norm("0o17"), "0o17");
        assert_eq!(norm("0B101"), "0b101");
    }

    #[test]
    fn bigint_keeps_suffix() {
        assert_eq!(norm("100n"), "100n");
        assert_eq!(norm("0xFFn"), "0xffn");
        assert_eq!(norm("0x1Fn"), "0x1fn");
    }

    #[test]
    fn regex_flags_sorted_alphabetically() {
        use super::sort_regex_flags as srf;
        // Prettier normalizes flag order (e.g. /…/vg → /…/gv).
        assert_eq!(srf("vg"), "gv");
        assert_eq!(srf("yim"), "imy");
        // Already-sorted flags are unchanged; empty stays empty.
        assert_eq!(srf("gimsuy"), "gimsuy");
        assert_eq!(srf("g"), "g");
        assert_eq!(srf(""), "");
    }

    #[test]
    fn valid_js_identifier_accepts_and_rejects() {
        use super::is_valid_js_identifier as vid;
        // Valid identifiers (so prettier emits the object key unquoted).
        assert!(vid("foo"));
        assert!(vid("$x"));
        assert!(vid("_"));
        assert!(vid("a1"));
        assert!(vid("camelCase"));
        // Reserved words count as identifiers (prettier outputs them unquoted).
        assert!(vid("class"));
        // Invalid: empty, leading digit, or any char that isn't id_continue.
        assert!(!vid(""));
        assert!(!vid("1a"));
        assert!(!vid("a-b"));
        assert!(!vid("a b"));
        assert!(!vid("a.b"));
    }
}
