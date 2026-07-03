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

use std::borrow::Cow;

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
        // A comment at a combinator boundary (between compounds, or glued between two
        // simple selectors of one compound) takes the interleaving path: the comment is
        // re-emitted at its authored position with the surrounding gap whitespace
        // normalized to a single space — the same rule as every other selector-comment
        // position (`:is()`/`:nth-*()`/`::slotted()` args). Prettier freezes the source
        // whitespace instead; parseCss rejects these entirely — see conformance_prettier
        // §CSS: Comments and conformance_svelte §CSS Corrections. A comment *inside* a
        // simple selector (a pseudo's args) is not a boundary comment and takes the
        // normal path (its own normalize handling).
        if self.complex_has_boundary_comment(complex) {
            return self.build_complex_selector_doc_with_comments(complex);
        }
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

    /// Whether this complex selector carries a comment at a combinator boundary — any
    /// comment inside the selector span that falls in a gap rather than inside a simple
    /// selector's own span. Covered gaps: before the first simple selector (a leading
    /// combinator, `:has(> /* c */ img)`), between compounds (`div /* c */ p`,
    /// `a > /* c */ b`), and within one compound (`.a/* c */.b`). A comment inside a
    /// simple selector's span (a pseudo's `(...)` args) is NOT a boundary comment and is
    /// left to the normal builder's own normalize path. Drives the verbatim freeze in
    /// `build_complex_selector_doc`.
    fn complex_has_boundary_comment(&self, complex: &internal::ComplexSelector<'_>) -> bool {
        let mut prev_end = complex.span.start;
        for rel in complex.children {
            for simple in rel.selectors {
                let span = simple.span();
                if has_comments_in_range(self.comments, prev_end, span.start) {
                    return true;
                }
                prev_end = span.end;
            }
        }
        false
    }

    /// Build a comment-bearing complex selector inline, interleaving each gap comment
    /// at its combinator boundary with normalized single-space separation. This is the
    /// selector-comment normalization the rest of the CSS printer applies uniformly
    /// (`:is()`/`:nth-*()`/`::slotted()` args): tsv collapses the gap whitespace to one
    /// space while prettier freezes the source layout — see conformance_prettier.md
    /// §CSS: Comments. A glued compound-internal comment (`.a/* c */.b`) is emitted
    /// glued (no spaces) so a compound never reads as a descendant `.a .b`. The selector
    /// renders inline (explicit spaces, no combinator break points), matching the
    /// comment-bearing selector-list rule that never applies the always-break.
    fn build_complex_selector_doc_with_comments(
        &self,
        complex: &internal::ComplexSelector<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut prev_end = complex.span.start;
        for (i, rel) in complex.children.iter().enumerate() {
            let first_start = rel.selectors[0].span().start;
            if let Some(combinator) = rel.combinator {
                if i == 0 {
                    // Leading combinator (`:has(> /* c */ img)`): the symbol, then any
                    // comment sitting between it and the first compound.
                    let s = Self::leading_combinator_str(combinator);
                    if !s.is_empty() {
                        parts.push(d.text(s));
                    }
                    if let Some(cs) = rel.combinator_span {
                        let after = self.comment_blocks_in_range(cs.end, first_start);
                        if !after.is_empty() {
                            parts.push(d.text_owned(after));
                            parts.push(d.text(" "));
                        }
                    }
                } else {
                    parts.push(self.combinator_separator_doc_with_comments(
                        combinator,
                        rel.combinator_span,
                        prev_end,
                        first_start,
                    ));
                }
            } else {
                // No combinator ⇒ the first compound. `complex_has_boundary_comment`
                // scans from `complex.span.start`, but nothing here emits a comment
                // sitting between it and this first simple selector — today the two
                // positions coincide (the range is empty). Pin that so a future span
                // change folding leading trivia into `complex.span.start` can't silently
                // drop the comment — a block-comment content loss `swallow_audit` can't
                // catch (it only sees `//` line-comment swallows in rendered output).
                debug_assert!(
                    !has_comments_in_range(self.comments, complex.span.start, first_start),
                    "leading gap comment before the first compound has no emission path"
                );
            }
            let n = rel.selectors.len();
            for (j, simple) in rel.selectors.iter().enumerate() {
                let sspan = simple.span();
                if j > 0 && has_comments_in_range(self.comments, prev_end, sspan.start) {
                    // Glued compound-internal trivia: emit the source slice verbatim (no
                    // space normalization). The run is fully glued (the parser keeps a
                    // compound together only across glued comments), so normalizing the
                    // space *between* two comments (`/* c *//* d */`) would insert a
                    // whitespace token and turn the compound into a descendant on
                    // re-parse — non-idempotent. The gap holds only comments here.
                    let gap = Span {
                        start: prev_end,
                        end: sspan.start,
                    };
                    parts.push(d.text_owned(gap.extract(self.source).to_string()));
                }
                parts.push(self.build_simple_selector_doc(simple, j + 1 == n));
                prev_end = sspan.end;
            }
        }
        d.concat(&parts)
    }

    /// The inter-compound separator for the comment path: a single leading space, the
    /// combinator symbol (`>`/`+`/`~`/`||`; none for descendant), and the gap's comments
    /// placed on their authored side of the symbol, each single-spaced. The
    /// span-splitting mirrors the non-comment `combinator_separator_doc` but injects the
    /// normalized comment text.
    fn combinator_separator_doc_with_comments(
        &self,
        combinator: internal::Combinator,
        combinator_span: Option<Span>,
        gap_start: u32,
        gap_end: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        parts.push(d.text(" "));
        match combinator {
            internal::Combinator::Descendant => {
                let gap = self.comment_blocks_in_range(gap_start, gap_end);
                if !gap.is_empty() {
                    parts.push(d.text_owned(gap));
                    parts.push(d.text(" "));
                }
            }
            other => {
                let (before, after) = match combinator_span {
                    Some(cs) => (
                        self.comment_blocks_in_range(gap_start, cs.start),
                        self.comment_blocks_in_range(cs.end, gap_end),
                    ),
                    None => (
                        self.comment_blocks_in_range(gap_start, gap_end),
                        String::new(),
                    ),
                };
                if !before.is_empty() {
                    parts.push(d.text_owned(before));
                    parts.push(d.text(" "));
                }
                parts.push(d.text(other.as_str()));
                parts.push(d.text(" "));
                if !after.is_empty() {
                    parts.push(d.text_owned(after));
                    parts.push(d.text(" "));
                }
            }
        }
        d.concat(&parts)
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
                // Inside `@keyframes`, the `from`/`to` keyframe selectors are
                // case-insensitive keywords — lowercase them (`FROM`→`from`),
                // matching prettier. Any other type selector (and all type selectors
                // outside keyframes) stays verbatim, so only pay the extract+trim on
                // the keyframes path.
                if self.in_keyframes {
                    let text = span.extract(self.source).trim();
                    if text.eq_ignore_ascii_case("from") || text.eq_ignore_ascii_case("to") {
                        return d.text_owned(text.to_ascii_lowercase());
                    }
                }
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
            internal::SimpleSelector::Nth { span } => {
                // Normalize An+B operator spacing (`2n+1` → `2n + 1`) to match prettier,
                // exactly like the dedicated `:nth-child` args path. An `An+B of S` term
                // folds ` of ` into the value (matching Svelte — see `match_nth_value`):
                // split it off, normalize the An+B, and re-emit ` of ` with a single
                // trailing space so the following sibling selector (`S`) stays separated
                // in the glued compound.
                let raw = span.extract(self.source);
                match split_nth_of(raw) {
                    Some(anb) => d.text_owned(format!("{} of ", Self::normalize_an_plus_b(anb))),
                    None => d.text_owned(Self::normalize_an_plus_b(raw)),
                }
            }
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
                // Interleave leading/trailing comments that sit inside the parens but
                // outside the inner list span (`:is(/* lead */ .a /* trail */)`).
                let inner = self.build_nested_selector_list_doc(selectors);
                let inner = self.wrap_args_gap_comments(inner, *span, selectors.span);
                self.wrap_pseudo_args(inner)
            }
            internal::PseudoClassArgs::Nth {
                value,
                of_selector,
                span,
                value_span,
            } => {
                // A comment inside the An+B text freezes it verbatim: the spacing
                // normalizer would do string surgery inside the comment content
                // (`/* a-b */` → `/* a - b */`). Prettier also skips An+B
                // normalization when a comment is present, so verbatim matches.
                let normalized = if value.contains("/*") {
                    (*value).to_string()
                } else {
                    Self::normalize_an_plus_b(value)
                };
                // Comments in the gaps around the An+B text are not part of
                // `value`; interleave them like the SelectorList arm above.
                let leading = self.comment_blocks_in_range(span.start, value_span.start);
                match of_selector {
                    None => {
                        let trailing = self.comment_blocks_in_range(value_span.end, span.end);
                        let inner = self.wrap_inner_with_comments(
                            d.text_owned(normalized),
                            &leading,
                            &trailing,
                        );
                        self.paren_wrap(inner)
                    }
                    Some(selectors) => {
                        // The of-gap comments (`of /* c */ .a`) lead the selector list.
                        let of_gap =
                            self.comment_blocks_in_range(value_span.end, selectors.span.start);
                        let list = self.wrap_inner_with_comments(
                            self.build_nested_selector_list_doc(selectors),
                            &of_gap,
                            "",
                        );
                        let trailing = self.comment_blocks_in_range(selectors.span.end, span.end);
                        let inner = d.concat(&[d.text_owned(normalized), d.text(" of "), list]);
                        let inner = self.wrap_inner_with_comments(inner, &leading, &trailing);
                        self.wrap_pseudo_args(inner)
                    }
                }
            }
            internal::PseudoClassArgs::Slotted { selectors, span } => {
                // A compound selector (no combinators) — never breaks.
                let mut parts = DocBuf::new();
                let n = selectors.len();
                for (j, simple) in selectors.iter().enumerate() {
                    parts.push(self.build_simple_selector_doc(simple, j + 1 == n));
                }
                let compound = d.concat(&parts);
                // Interleave leading/trailing comments inside the parens but outside
                // the compound (`::slotted(/* lead */ div /* trail */)`), using the
                // compound's own bounds. The parser guarantees ≥1 selector, so the
                // fallback is unreachable in practice.
                let inner = match (selectors.first(), selectors.last()) {
                    (Some(first), Some(last)) => {
                        let content = Span {
                            start: first.span().start,
                            end: last.span().end,
                        };
                        self.wrap_args_gap_comments(compound, *span, content)
                    }
                    _ => compound,
                };
                self.paren_wrap(inner)
            }
            internal::PseudoClassArgs::Part {
                idents,
                span,
                value_span,
            } => {
                // Interleave leading/trailing comments outside the identifier run
                // (`::part(/* lead */ label /* trail */)`).
                let inner =
                    self.wrap_args_gap_comments(d.text_owned(idents.join(" ")), *span, *value_span);
                self.paren_wrap(inner)
            }
        }
    }

    /// Interleave the gap comments that sit inside a pseudo's argument parens
    /// (`args_span`) but outside the argument content (`content_span`), returning
    /// `inner` wrapped with them. Assumes `args_span.end` is one byte past the `)` —
    /// the `Slotted`/`Part`/`Identifier`/`SelectorList` convention, where `span` is
    /// the full `(...)` printer bound — so the trailing gap ends at `args_span.end - 1`,
    /// the `)` position. `Nth` can't share this helper: its `span` is instead the
    /// Svelte-matching public-AST node span (it ends *before* the `)`, and convert
    /// reads it verbatim — see `convert_pseudo_class_args`), so it interleaves inline.
    fn wrap_args_gap_comments(&self, inner: DocId, args_span: Span, content_span: Span) -> DocId {
        let leading = self.comment_blocks_in_range(args_span.start, content_span.start);
        let trailing =
            self.comment_blocks_in_range(content_span.end, args_span.end.saturating_sub(1));
        self.wrap_inner_with_comments(inner, &leading, &trailing)
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

    /// Wrap pseudo-argument content in literal `(`…`)` with no break points — the
    /// non-breaking counterpart of `wrap_pseudo_args`, for argument forms that never
    /// break (the `::slotted()` compound, `::part()` idents, `:dir()`/`:lang()`
    /// identifier, and the bare `:nth-*()` An+B).
    fn paren_wrap(&self, inner: DocId) -> DocId {
        let d = self.d();
        d.concat(&[d.text("("), inner, d.text(")")])
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
        // Lowercase the `n` variable when it carries a numeric coefficient
        // (`2N`→`2n`), matching prettier — a bare `N`/`-N` keeps its case (prettier
        // preserves it), as do the `even`/`odd` keywords (their `n` isn't
        // digit-preceded). Applied before the early return so `2N` (no operator)
        // is cased too.
        let cased = lowercase_an_plus_b_n(value.trim());
        let trimmed = cased.as_ref();

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

/// Split an `An+B of S` term's folded value (`"2n of "`, `"-n + 3 of "`) into its
/// `An+B` prefix, or `None` for a bare An+B (`"2n"`, `"odd"`). The parser
/// (`match_nth_value`) only ever produces `"<An+B>\s+of\s+"` or `"<An+B>"`, so the
/// `of` — when present — is a trailing whole word preceded by whitespace; the
/// whitespace check rejects a hypothetical An+B ending in the letters `of`.
fn split_nth_of(value: &str) -> Option<&str> {
    let anb = value.trim_end().strip_suffix("of")?;
    anb.ends_with(char::is_whitespace).then(|| anb.trim_end())
}

/// Lowercase the An+B `n` variable when it carries a numeric coefficient
/// (`2N`→`2n`), matching prettier. A bare `N`/`-N` keeps its case (prettier
/// preserves it), and the `even`/`odd` keywords are untouched (their `n` is not
/// digit-preceded). Borrows when there is nothing to change.
fn lowercase_an_plus_b_n(s: &str) -> Cow<'_, str> {
    let bytes = s.as_bytes();
    let needs = bytes
        .iter()
        .enumerate()
        .any(|(i, &b)| b == b'N' && i > 0 && bytes[i - 1].is_ascii_digit());
    if !needs {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut prev_digit = false;
    for ch in s.chars() {
        if ch == 'N' && prev_digit {
            out.push('n');
        } else {
            out.push(ch);
        }
        prev_digit = ch.is_ascii_digit();
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::lowercase_an_plus_b_n;

    #[test]
    fn test_lowercase_an_plus_b_n() {
        // Digit-preceded `n` lowercases (matches prettier).
        assert_eq!(lowercase_an_plus_b_n("2N"), "2n");
        assert_eq!(lowercase_an_plus_b_n("2N+1"), "2n+1");
        assert_eq!(lowercase_an_plus_b_n("0N"), "0n");
        // Bare `N`/`-N` keep their case (prettier preserves them).
        assert!(matches!(
            lowercase_an_plus_b_n("N"),
            std::borrow::Cow::Borrowed("N")
        ));
        assert_eq!(lowercase_an_plus_b_n("-N+3"), "-N+3");
        // `even`/`odd` are untouched (their `n` is not digit-preceded).
        assert_eq!(lowercase_an_plus_b_n("EVEN"), "EVEN");
        assert_eq!(lowercase_an_plus_b_n("ODD"), "ODD");
    }
}
