// CSS selector formatting
//
// Handles formatting of:
// - Selector lists (comma-separated)
// - Complex selectors (with combinators)
// - Relative selectors (simple selector chains / compounds)
// - Simple selectors (type, class, id, pseudo-class, pseudo-element, etc.)
//
// ## Architecture
//
// Doc-first (like `values.rs`): every selector is built as ONE `group`/`indent`/
// `line`/`softline` doc tree and rendered once via `write_arena_doc_with_suffix`. The
// renderer makes the wrapping decisions, so width measurement and emission share a
// single representation — there is no separate measurement pass to drift from
// emission. The list level (the "2+ selectors always break" rule and the
// comment-bearing raw seam) stays imperative because it carries no width logic to
// drift.
//
// ### Indent model (a deliberate divergence from prettier)
//
// A complex selector that spans more than one compound (i.e. it has a combinator)
// indents its continuation lines one level — `group(indent(...))`. A pseudo's
// broken arguments always indent one level relative to the pseudo. That is the
// whole rule: a single compound's pseudo args sit one level in (the `)` aligns
// with the selector), and on a combinator continuation they sit two levels in (the
// combinator indent plus the pseudo's own). Prettier instead keys an extra indent
// on a flat `nodes.length > 2` count, which shoves a single compound's pseudo args
// a gratuitous level deeper than the rule body with no combinator to align to.
// tsv's uniform rule is cleaner and needs no node counting — the `+1` comes only
// from a real combinator. See conformance_prettier.md §CSS: Selectors.

use super::Printer;
use crate::ast::internal;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::source_scan;
use tsv_lang::{Span, has_comments_in_range};

/// Trailing punctuation that follows a selector on its last line (`) {` / `,`),
/// reserved so a selector that would overflow once the brace is appended breaks
/// instead. Two columns reproduces the imperative printer's `+2`/`+4` overheads
/// exactly (the pseudo-args group already counts its own `()`).
const SELECTOR_SUFFIX_WIDTH: usize = 2;

impl<'a> Printer<'a> {
    /// The leading combinator string for the first compound in a complex selector
    /// (e.g. the `>` in `:has(> img)`). A descendant combinator has no leading
    /// symbol, so this returns `""`. Between two compounds a combinator renders as a
    /// breakable separator instead — see `combinator_separator_doc`.
    fn leading_combinator_str(combinator: internal::Combinator) -> &'static str {
        match combinator {
            internal::Combinator::Descendant => "",
            internal::Combinator::Child => "> ",
            internal::Combinator::NextSibling => "+ ",
            internal::Combinator::SubsequentSibling => "~ ",
            internal::Combinator::Column => "|| ",
        }
    }

    /// Split the comments in `[start, end)` — the gap between two selectors, which
    /// holds the separating comma — into the comments before the comma and those
    /// after, each joined as `/*…*/` text. The comma is found comment-aware (a `,`
    /// inside a comment is not the separator). This is the principled replacement for
    /// the old `normalize_selector_comment_spacing` string-replace: it preserves each
    /// comment's side of the comma while normalizing the surrounding whitespace.
    fn split_selector_comments_around_comma(&self, start: u32, end: u32) -> (String, String) {
        let comma = source_scan::find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            end as usize,
            b',',
        )
        .map(|pos| pos as u32);
        self.split_comments_at(start, end, comma)
    }

    //
    // Entry points
    //

    /// Format a top-level selector list (a rule's selector).
    ///
    /// Prettier's rule: a top-level list of 2+ selectors ALWAYS breaks (one per
    /// line); a single selector wraps only on width. A comment at a comma boundary
    /// keeps the whole list **inline** (matching prettier — a comment-bearing list is
    /// not subject to the always-break rule), with the comments interleaved at their
    /// boundaries and the surrounding whitespace normalized (the cataloged spacing
    /// divergence — prettier preserves the source whitespace; see conformance_prettier.md
    /// §CSS: Comments).
    pub(super) fn print_selector_list(&mut self, list: &internal::SelectorList<'_>) {
        if list.selectors.is_empty() {
            return;
        }
        if has_comments_in_range(self.comments, list.span.start, list.span.end) {
            let doc = self.build_comma_list_doc(list, false);
            self.write_arena_doc_with_suffix(doc, SELECTOR_SUFFIX_WIDTH);
            return;
        }
        if list.selectors.len() >= 2 {
            // Top-level list: each selector on its own line.
            for (i, complex) in list.selectors.iter().enumerate() {
                if i > 0 {
                    self.write(",\n");
                    self.write_indent();
                }
                self.print_complex_selector(complex);
            }
        } else {
            self.print_complex_selector(&list.selectors[0]);
        }
    }

    /// Format a selector list in a nested context that wraps inside its own
    /// parentheses — `@scope (root) to (limit)`. The caller writes the `(`/`)`; this
    /// renders the inner list, breaking each selector onto its own indented line
    /// when it exceeds the print width (never the always-break top-level rule).
    pub(super) fn print_selector_list_nested(&mut self, list: &internal::SelectorList<'_>) {
        if list.selectors.is_empty() {
            return;
        }
        let d = self.d();
        let inner = self.build_nested_selector_list_doc(list);
        // The caller already wrote `(`; emit `softline inner` indented, then a
        // trailing softline so the closing `)` (written by the caller) lands at the
        // base level when broken. Reserve `) {`-ish via a 3-col suffix.
        let doc = d.group(d.concat(&[d.indent(d.concat(&[d.softline(), inner])), d.softline()]));
        self.write_arena_doc_with_suffix(doc, 3);
    }

    /// Build a doc for a selector list joined by `,`-`line` — the nested/forgiving
    /// form used inside pseudo arguments and `@scope`. Each selector is a full
    /// complex-selector doc; the group around it (added by the caller) decides
    /// whether the `line`s flatten to `, ` or break one-per-line.
    fn build_nested_selector_list_doc(&self, list: &internal::SelectorList<'_>) -> DocId {
        self.build_comma_list_doc(list, true)
    }

    /// Build a comma-joined selector-list doc with comments interleaved at each comma
    /// boundary (pre-comma comments trail the previous selector, post-comma comments
    /// lead the next). `breakable` selects the separator after the comma: a `line`
    /// (the nested/forgiving form, where the enclosing group decides whether to break
    /// one-per-line) or a literal space (the top-level inline-with-comments form, which
    /// matches prettier by never breaking a comment-bearing list). Leading/trailing
    /// comments (inside `:is()` parens) are added by the caller — `build_pseudo_args_doc`
    /// via `comment_blocks_in_range` + `wrap_inner_with_comments` — since they sit
    /// outside the list span.
    fn build_comma_list_doc(&self, list: &internal::SelectorList<'_>, breakable: bool) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for (i, complex) in list.selectors.iter().enumerate() {
            if i > 0 {
                let (before, after) = self.split_selector_comments_around_comma(
                    list.selectors[i - 1].span.end,
                    complex.span.start,
                );
                if !before.is_empty() {
                    parts.push(d.text(" "));
                    parts.push(d.text_owned(before));
                }
                parts.push(d.text(","));
                parts.push(if breakable { d.line() } else { d.text(" ") });
                if !after.is_empty() {
                    parts.push(d.text_owned(after));
                    parts.push(d.text(" "));
                }
            }
            parts.push(self.build_complex_selector_doc(complex));
        }
        d.concat(&parts)
    }

    /// Render a single complex selector by building its doc and printing it,
    /// reserving the trailing `) {`/`,` so an over-width selector breaks.
    fn print_complex_selector(&mut self, complex: &internal::ComplexSelector<'_>) {
        let doc = self.build_complex_selector_doc(complex);
        self.write_arena_doc_with_suffix(doc, SELECTOR_SUFFIX_WIDTH);
    }

    //
    // Doc builders — all formatting logic expressed as doc IR
    //

    /// Build a doc for a complex selector (compounds joined by combinators).
    ///
    /// Multi-compound selectors get `group(indent(...))` so they break at their
    /// combinators and indent continuation lines (and any pseudo args on them) one
    /// level. A single compound needs no group/indent — its only break point is a
    /// pseudo's own arg group, which is self-contained.
    fn build_complex_selector_doc(&self, complex: &internal::ComplexSelector<'_>) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for (i, rel) in complex.children.iter().enumerate() {
            if let Some(combinator) = rel.combinator {
                if i == 0 {
                    // Leading combinator (e.g. `:has(> img)`): no break before it.
                    let s = Self::leading_combinator_str(combinator);
                    if !s.is_empty() {
                        parts.push(d.text(s));
                    }
                } else {
                    parts.push(self.combinator_separator_doc(combinator));
                }
            }
            let n = rel.selectors.len();
            for (j, simple) in rel.selectors.iter().enumerate() {
                parts.push(self.build_simple_selector_doc(simple, j + 1 == n));
            }
        }
        let body = d.concat(&parts);
        if complex.children.len() > 1 {
            d.group(d.indent(body))
        } else {
            body
        }
    }

    /// The break point between two compounds: a bare `line` for a descendant
    /// combinator (space when flat, newline when broken), or `line` + the
    /// combinator symbol + a trailing space for `>`/`+`/`~`/`||`.
    fn combinator_separator_doc(&self, combinator: internal::Combinator) -> DocId {
        let d = self.d();
        match combinator {
            internal::Combinator::Descendant => d.line(),
            other => d.concat(&[d.line(), d.text(other.as_str()), d.text(" ")]),
        }
    }

    /// Build a width-measurement-and-emission doc for a span-based simple selector
    /// (type / class / id / pseudo-without-args) extracted verbatim from source so
    /// escapes are preserved.
    ///
    /// A CSS hex escape consumes one following whitespace as its terminator, which
    /// the lexer captures into the selector's span (`.\1F600 ` before `{`). When
    /// this is the last simple selector in its compound, whatever follows is a
    /// structural separator (a combinator's space, `,`, `)`, or the block `{`) that
    /// terminates the escape on its own, so the captured terminator is dropped to
    /// avoid a doubled space. An internal terminator (`.\1F600 .b` inside one
    /// compound, or the first of `:\41 :\42`) is kept — it separates the escape from
    /// the next simple selector. This single leaf rule replaces the old
    /// buffer-popping `pop_selector_terminator`.
    fn span_leaf_doc(&self, span: Span, is_last_in_compound: bool) -> DocId {
        let raw = span.extract(self.source);
        let text = if is_last_in_compound {
            raw.trim_end()
        } else {
            raw
        };
        self.d().text_owned(text.to_string())
    }

    /// Reconstruct an attribute selector (`[ns|name op 'value' flags]`) verbatim
    /// from source. The name is emitted raw (escapes preserved — `[f\oo]` stays
    /// `[f\oo]`).
    fn build_attribute_selector_text(
        &self,
        namespace: Option<&str>,
        name_span: Span,
        matcher: Option<internal::AttributeMatcher>,
        value: Option<&str>,
        flags: Option<&str>,
    ) -> String {
        let mut result = String::from("[");
        if let Some(ns) = namespace {
            result.push_str(ns);
            result.push('|');
        }
        result.push_str(name_span.extract(self.source));
        if let Some(m) = matcher {
            result.push_str(m.as_str());
            if let Some(v) = value {
                // TODO: Determine if value needs quotes
                result.push('\'');
                result.push_str(v);
                result.push('\'');
            }
        }
        if let Some(f) = flags {
            result.push(' ');
            result.push_str(f);
        }
        result.push(']');
        result
    }

    /// Fold a pseudo selector's `:name` / `::name` prefix to its canonical case,
    /// returning the text (the args, if any, format separately).
    ///
    /// Svelte decodes the internal name (`:\41 ` → name "A"), but the formatter
    /// keeps it verbatim like class/id/type selectors. Prettier lowercases
    /// case-insensitive pseudo keywords (`:HOVER` → `:hover`, `::-WEBKIT-` →
    /// `::-webkit-`) but preserves custom `:--Name` pseudos and, for an escaped
    /// name, folds only up to the escape's terminator whitespace (`:\4A b` →
    /// `:\4a b`, keeping the literal `B` in `::\41 B`). With `has_args`, only the
    /// part before `(` is the name.
    fn pseudo_name_text(&self, span: Span, has_args: bool) -> String {
        let raw = span.extract(self.source);
        let name = if has_args {
            raw.split_once('(').map_or(raw, |(before, _)| before)
        } else {
            raw
        };
        let after_colons = name.trim_start_matches(':');
        if after_colons.starts_with("--") {
            return name.to_string();
        }
        let (head, tail) = match name.find(|c: char| c.is_ascii_whitespace()) {
            Some(i) => name.split_at(i),
            None => (name, ""),
        };
        let mut out = String::with_capacity(name.len());
        if head.bytes().any(|b| b.is_ascii_uppercase()) {
            out.push_str(&head.to_ascii_lowercase());
        } else {
            out.push_str(head);
        }
        out.push_str(tail);
        out
    }

    /// Build a doc for a simple selector, dispatched by kind.
    ///
    /// `is_last_in_compound` controls the escape-terminator strip (see
    /// `span_leaf_doc`): only the final simple selector of a compound drops its
    /// trailing terminator whitespace.
    fn build_simple_selector_doc(
        &self,
        simple: &internal::SimpleSelector<'_>,
        is_last_in_compound: bool,
    ) -> DocId {
        let d = self.d();
        match simple {
            internal::SimpleSelector::Type { span, .. } => {
                self.span_leaf_doc(*span, is_last_in_compound)
            }
            internal::SimpleSelector::Universal { namespace, .. } => match namespace {
                Some(ns) => d.text_owned(format!("{ns}|*")),
                None => d.text("*"),
            },
            internal::SimpleSelector::Class { span } => {
                self.span_leaf_doc(*span, is_last_in_compound)
            }
            internal::SimpleSelector::Id { span } => self.span_leaf_doc(*span, is_last_in_compound),
            internal::SimpleSelector::Attribute {
                namespace,
                name_span,
                matcher,
                value,
                flags,
                ..
            } => {
                d.text_owned(self.build_attribute_selector_text(
                    *namespace, *name_span, *matcher, *value, *flags,
                ))
            }
            internal::SimpleSelector::PseudoClass { args, span } => {
                self.build_pseudo_doc(*span, args.as_ref(), is_last_in_compound)
            }
            internal::SimpleSelector::PseudoElement { args, span } => {
                self.build_pseudo_doc(*span, args.as_ref(), is_last_in_compound)
            }
            internal::SimpleSelector::Nesting { .. } => d.text("&"),
            internal::SimpleSelector::Percentage { value, .. } => d.text_owned(format!("{value}%")),
            internal::SimpleSelector::Invalid { span } => {
                d.text_owned(span.extract(self.source).trim().to_string())
            }
        }
    }

    /// Build the doc for a pseudo-class or pseudo-element (`:name` / `::name`,
    /// optionally with arguments). Pseudo-classes and pseudo-elements share one
    /// path: the name folds case the same way and the argument group is identical
    /// (the historical extra-indent split between them is gone).
    fn build_pseudo_doc(
        &self,
        span: Span,
        args: Option<&internal::PseudoClassArgs<'_>>,
        is_last_in_compound: bool,
    ) -> DocId {
        let d = self.d();
        match args {
            Some(args) => {
                let name = self.pseudo_name_text(span, true);
                d.concat(&[d.text_owned(name), self.build_pseudo_args_doc(args)])
            }
            None => {
                // No args: the whole span is the name; drop its escape terminator
                // when it ends the compound.
                let name = self.pseudo_name_text(span, false);
                let text = if is_last_in_compound {
                    name.trim_end().to_string()
                } else {
                    name
                };
                d.text_owned(text)
            }
        }
    }

    /// Build the parenthesized argument doc for a pseudo-class/element.
    ///
    /// Selector-list args (`:is()`, `:not()`, `:where()`, `:has()`, `::slotted()`
    /// list form, `:nth-child(... of S)`) wrap as `group("(" indent(softline join)
    /// softline ")")` — inline when they fit, one-per-line indented when they
    /// don't. `::slotted()` compound args, `::part()` idents, and identifier args
    /// never break.
    fn build_pseudo_args_doc(&self, args: &internal::PseudoClassArgs<'_>) -> DocId {
        let d = self.d();
        match args {
            internal::PseudoClassArgs::SelectorList { selectors, span } => {
                let inner = self.build_nested_selector_list_doc(selectors);
                // Interleave leading/trailing comments that sit inside the parens but
                // outside the inner list span (`:is(/* lead */ .a /* trail */)`). The
                // args `span` covers `(content)`; the `)` is its last byte.
                let leading = self.comment_blocks_in_range(span.start, selectors.span.start);
                let trailing =
                    self.comment_blocks_in_range(selectors.span.end, span.end.saturating_sub(1));
                let inner = self.wrap_inner_with_comments(inner, &leading, &trailing);
                self.wrap_pseudo_args(inner)
            }
            internal::PseudoClassArgs::Nth {
                value, of_selector, ..
            } => {
                let normalized = Self::normalize_an_plus_b(value);
                match of_selector {
                    None => d.concat(&[d.text("("), d.text_owned(normalized), d.text(")")]),
                    Some(selectors) => {
                        let list = self.build_nested_selector_list_doc(selectors);
                        let inner = d.concat(&[d.text_owned(normalized), d.text(" of "), list]);
                        self.wrap_pseudo_args(inner)
                    }
                }
            }
            internal::PseudoClassArgs::Slotted { selectors, .. } => {
                // A compound selector (no combinators) — never breaks.
                let mut parts = DocBuf::new();
                let n = selectors.len();
                for (j, simple) in selectors.iter().enumerate() {
                    parts.push(self.build_simple_selector_doc(simple, j + 1 == n));
                }
                d.concat(&[d.text("("), d.concat(&parts), d.text(")")])
            }
            internal::PseudoClassArgs::Part { idents, .. } => {
                d.concat(&[d.text("("), d.text_owned(idents.join(" ")), d.text(")")])
            }
            internal::PseudoClassArgs::Identifier { value, .. } => {
                d.concat(&[d.text("("), d.text_owned(value.to_string()), d.text(")")])
            }
        }
    }

    /// Prepend a leading comment and append a trailing comment around an inner doc,
    /// each separated by a single space (a no-op for an empty side). Used for the
    /// comments inside a pseudo's parens that sit outside the inner selector list.
    fn wrap_inner_with_comments(&self, inner: DocId, leading: &str, trailing: &str) -> DocId {
        let d = self.d();
        if leading.is_empty() && trailing.is_empty() {
            return inner;
        }
        let mut parts = DocBuf::new();
        if !leading.is_empty() {
            parts.push(d.text_owned(leading.to_string()));
            parts.push(d.text(" "));
        }
        parts.push(inner);
        if !trailing.is_empty() {
            parts.push(d.text(" "));
            parts.push(d.text_owned(trailing.to_string()));
        }
        d.concat(&parts)
    }

    /// Wrap pseudo-argument content in the standard breakable envelope:
    /// `group("(" indent(softline inner) softline ")")`. Flat → `(inner)`; broken →
    /// the content indents one level and the `)` returns to the pseudo's level.
    fn wrap_pseudo_args(&self, inner: DocId) -> DocId {
        let d = self.d();
        d.group(d.concat(&[
            d.text("("),
            d.indent(d.concat(&[d.softline(), inner])),
            d.softline(),
            d.text(")"),
        ]))
    }

    /// Normalize An+B notation spacing (better than prettier)
    ///
    /// Per CSS Syntax spec: "Whitespace is valid (and ignored) between any other two tokens"
    /// This means we can normalize for consistency without changing semantics.
    ///
    /// Our normalization (better than prettier):
    /// - `2n+1` → `2n + 1` (always add spaces around +)
    /// - `3n-2` → `3n - 2` (always add spaces around -, unlike prettier)
    /// - `2n  +  1` → `2n + 1` (collapse multiple spaces)
    /// - `  n  ` → `n` (trim outer spaces)
    /// - `odd`, `even`, `3` → unchanged
    fn normalize_an_plus_b(value: &str) -> String {
        let trimmed = value.trim();

        // Simple cases: keywords and plain numbers (no operators)
        if !trimmed.contains('+') && !trimmed.contains('-') {
            return trimmed.to_string();
        }

        // Normalize spacing around + and - operators
        let mut result = String::with_capacity(trimmed.len() + 4);
        let chars: Vec<char> = trimmed.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            // Handle + and - operators: always normalize to ` op `
            if (ch == '+' || ch == '-') && i > 0 {
                // Check if this is an operator (has content before it)
                let trimmed_result = result.trim_end();
                if !trimmed_result.is_empty() {
                    // This is an operator - normalize spacing
                    result.truncate(trimmed_result.len());
                    result.push(' ');
                    result.push(ch);

                    // Skip any spaces after operator and add single space
                    i += 1;
                    while i < chars.len() && chars[i].is_whitespace() {
                        i += 1;
                    }
                    if i < chars.len() {
                        result.push(' ');
                    }
                    continue;
                }
            }

            result.push(ch);
            i += 1;
        }

        result.trim().to_string()
    }
}
