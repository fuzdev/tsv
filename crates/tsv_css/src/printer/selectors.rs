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
// from a real combinator. See conformance_prettier_css.md §CSS: Selectors.

use std::borrow::Cow;
use std::fmt::Write;

use super::Printer;
use super::value_normalization;
use crate::ast::internal;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::printing::format_string_literal;
use tsv_lang::source_scan;
use tsv_lang::{Span, has_comments_to_emit_in_range};

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
        // The comma is located only to decide which side of it each comment falls on — it
        // is re-emitted as static text either way. With no comment in the gap both sides
        // are empty regardless of where the comma sits, so check that first and skip the
        // comment-aware byte scan entirely.
        if !has_comments_to_emit_in_range(self.comments, start, end) {
            return (String::new(), String::new());
        }
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
    /// divergence — prettier preserves the source whitespace; see
    /// conformance_prettier_css.md §CSS: Comments).
    pub(super) fn print_selector_list(&mut self, list: &internal::SelectorList<'_>) {
        if list.selectors.is_empty() {
            return;
        }
        if has_comments_to_emit_in_range(self.comments, list.span.start, list.span.end) {
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
    ///
    /// `paren_span` is the clause's full `(…)` span (end one past `)`, the pseudo-arg
    /// convention). Comments leading or trailing the selector list but inside the parens
    /// (`@scope (/* c */ .a)`) sit outside `list.span`, so they interleave here via
    /// `wrap_args_gap_comments` — the same wrapping `:is()`'s argument gaps use.
    pub(super) fn print_selector_list_nested(
        &mut self,
        list: &internal::SelectorList<'_>,
        paren_span: Option<Span>,
    ) {
        if list.selectors.is_empty() {
            return;
        }
        let d = self.d();
        let mut inner = self.build_nested_selector_list_doc(list);
        if let Some(paren_span) = paren_span {
            inner = self.wrap_args_gap_comments(inner, paren_span, list.span);
        }
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
                    parts.push(d.text_pooled(&before));
                }
                parts.push(d.text(","));
                parts.push(if breakable { d.line() } else { d.text(" ") });
                if !after.is_empty() {
                    parts.push(d.text_pooled(&after));
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
    pub(super) fn build_complex_selector_doc(
        &self,
        complex: &internal::ComplexSelector<'_>,
    ) -> DocId {
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
                // A preserved anchorless combinator (an empty compound — a consecutive
                // combinator like the leading `>` of `> > .a`) emits just its symbol, with
                // no trailing space: the following relative selector's separator supplies
                // the single space, so the run reads `> > .a`, not `>  > .a`. Such an RS
                // always carries an explicit combinator (Descendant is never anchorless).
                if rel.selectors.is_empty() {
                    if i == 0 {
                        parts.push(d.text(combinator.as_str()));
                    } else {
                        parts.push(d.concat(&[d.line(), d.text(combinator.as_str())]));
                    }
                } else if i == 0 {
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
                if j == 0 {
                    let kept = self.preserved_boundary_ws(simple.span().start);
                    if !kept.is_empty() {
                        parts.push(d.text_pooled(kept));
                    }
                }
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

    /// The tail of the boundary whitespace run before `start` that this printer must emit
    /// verbatim: everything from its first **non-ASCII** member on.
    ///
    /// The parser skipped the whole run because `parseCss` skips it (JS `\s` at every
    /// `allow_whitespace()` juncture — see `tsv_lang::is_js_whitespace`), but that is a
    /// statement about the AST, not a licence to drop source. Deleting one of those
    /// characters is a content difference the corpus SAFETY check reads as `content_lost` —
    /// its semantic-character count excludes only ASCII whitespace, so a `<NBSP>` or `<LS>` is
    /// a character like any other. Same call the escaped-selector names make: the AST mirrors
    /// Svelte, the printer emits the author's bytes.
    ///
    /// ⚠️ The run is scanned back over **both** classes and emitted from the first non-ASCII
    /// member — not scanned back over the non-ASCII class alone. The two differ exactly on a
    /// mixed run, and only the first is prettier's answer: `<NBSP><SP>div` keeps `<NBSP><SP>`
    /// there, where a non-ASCII-only scan stops on the space, finds nothing, and DELETES the
    /// `<NBSP>`. What the ASCII *head* of the run contributes is indentation, which the
    /// printer regenerates — so it is dropped, while an interior one rides along inside the
    /// slice as the author spelled it. prettier respells that interior run as a single space
    /// (`<NBSP><TAB><NBSP>div` → `<NBSP><SP><NBSP>div`); tsv keeps the bytes, which is a
    /// whitespace-SPELLING difference with no content at stake, in a run no authored
    /// stylesheet contains. Pinned in `tests/css_boundary_whitespace.rs`.
    fn preserved_boundary_ws(&self, start: u32) -> &'a str {
        let start = start as usize;
        let mut run_start = start;
        while run_start > 0 {
            let prev = self.source[..run_start]
                .chars()
                .next_back()
                .filter(|c| crate::whitespace::is_boundary_whitespace(*c));
            match prev {
                Some(c) => run_start -= c.len_utf8(),
                None => break,
            }
        }
        // ⚠️ A leading BOM is NOT preserved. `U+FEFF` is in JS `\s`, so a byte-order mark
        // reaches this scan like any other member — but tsv strips BOMs by policy (a
        // cataloged prettier divergence, `docs/conformance_prettier.md` §Whitespace: BOM
        // Handling), and re-emitting it here would quietly undo that. The exclusion is
        // anchored at offset 0 because that is what makes a `U+FEFF` a byte-order mark; one
        // anywhere else is an ordinary character and is preserved with the rest — including a
        // second one later in this same run, which the forward scan below still reaches.
        if run_start == 0 && self.source.starts_with('\u{feff}') {
            run_start = '\u{feff}'.len_utf8();
        }
        let kept = self.source[run_start..start]
            .char_indices()
            .find(|(_, c)| crate::whitespace::is_boundary_only_whitespace(*c))
            .map_or(start, |(i, _)| run_start + i);
        &self.source[kept..start]
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
        // Every gap the loop below probes lies inside the selector's own span, and a
        // comment registers only when it sits fully inside the queried range — so a
        // comment-free selector has no boundary comment. One probe answers that, instead
        // of one per simple selector.
        if !has_comments_to_emit_in_range(self.comments, complex.span.start, complex.span.end) {
            return false;
        }
        let mut prev_end = complex.span.start;
        for rel in complex.children {
            for simple in rel.selectors {
                let span = simple.span();
                if has_comments_to_emit_in_range(self.comments, prev_end, span.start) {
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
    /// space while prettier freezes the source layout — see conformance_prettier_css.md
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
            // A preserved anchorless combinator (empty compound — a consecutive combinator
            // like the leading `>` of `> > .a`) has no simple selector to anchor `first_start`
            // on. Emit any comment in the gap before it, then the combinator symbol with no
            // trailing space (the next relative selector's separator supplies the space, and
            // its before-range picks up any comment sitting after this combinator).
            if rel.selectors.is_empty() {
                if let Some(combinator) = rel.combinator {
                    if i > 0 {
                        // Explicit space (this path renders inline, no `line()` break points).
                        parts.push(d.text(" "));
                    }
                    if let Some(cs) = rel.combinator_span {
                        let before = self.comment_blocks_in_range(prev_end, cs.start);
                        if !before.is_empty() {
                            parts.push(d.text_pooled(&before));
                            parts.push(d.text(" "));
                        }
                    }
                    parts.push(d.text(combinator.as_str()));
                }
                prev_end = rel.span.end;
                continue;
            }
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
                            parts.push(d.text_pooled(&after));
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
                    !has_comments_to_emit_in_range(self.comments, complex.span.start, first_start),
                    "leading gap comment before the first compound has no emission path"
                );
            }
            let n = rel.selectors.len();
            for (j, simple) in rel.selectors.iter().enumerate() {
                let sspan = simple.span();
                if j == 0 {
                    let kept = self.preserved_boundary_ws(sspan.start);
                    if !kept.is_empty() {
                        parts.push(d.text_pooled(kept));
                    }
                }
                if j > 0 && has_comments_to_emit_in_range(self.comments, prev_end, sspan.start) {
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
                    // The gap's comments ride out inside the raw slice — see
                    // `tsv_lang::comment_ledger`.
                    #[cfg(feature = "comment_check")]
                    tsv_lang::comment_ledger::record_verbatim_range(
                        self.source,
                        gap.start,
                        gap.end,
                    );
                    parts.push(d.source_span(gap, self.source));
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
                    parts.push(d.text_pooled(&gap));
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
                    parts.push(d.text_pooled(&before));
                    parts.push(d.text(" "));
                }
                parts.push(d.text(other.as_str()));
                parts.push(d.text(" "));
                if !after.is_empty() {
                    parts.push(d.text_pooled(&after));
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
    /// A CSS **hex** escape consumes one following whitespace as its *terminator*,
    /// which the lexer captures into the selector's span (`.\1F600 ` before `{`). When
    /// this is the last simple selector in its compound, whatever follows is a
    /// structural separator (a combinator's space, `,`, `)`, or the block `{`) that
    /// terminates the escape on its own, so the captured terminator is dropped to
    /// avoid a doubled space. An internal terminator (`.\1F600 .b` inside one
    /// compound, or the first of `:\41 :\42`) is kept — it separates the escape from
    /// the next simple selector. This single leaf rule replaces the old
    /// buffer-popping `pop_selector_terminator`.
    ///
    /// A **literal** escape's whitespace is the opposite case and must NOT be trimmed:
    /// in `.a\ ` the space is the escape's *payload* (the class is named `a `), not a
    /// terminator, so dropping it strands the backslash — which then escapes the
    /// separator that follows, silently **merging a descendant combinator into the
    /// compound** (`.a\␣␣.b` would print as `.a\␣.b`, which re-parses as one compound,
    /// losing the `Combinator`). `trim_end_preserving_escape` draws exactly that line:
    /// a whitespace preceded by an odd-length backslash run is a payload and stays;
    /// anything else (a hex terminator, ordinary padding) still goes.
    fn span_leaf_doc(&self, span: Span, is_last_in_compound: bool) -> DocId {
        let raw = span.extract(self.source);
        let text = if is_last_in_compound {
            crate::escapes::trim_end_preserving_escape(raw)
        } else {
            raw
        };
        self.d().text_pooled(text)
    }

    /// Reconstruct an attribute selector (`[ns|name op 'value' flags]`) from source.
    /// The namespace prefix and the name are emitted raw (escapes preserved — `[f\oo]`
    /// stays `[f\oo]`, `[a\|b|attr]` keeps the escape that would otherwise read as the
    /// `<wq-name>` separator); the value is re-quoted from its raw token like a
    /// declaration string (the delimiter that needs fewer escapes wins, ties prefer
    /// single — matches prettier), except that a bare value stays bare when quoting it
    /// would force a re-escape.
    ///
    /// One probe splits the two builds: with no comment anywhere inside the selector
    /// there is no gap to interleave and no part to locate, so
    /// [`Self::build_commented_attribute_selector_text`] — and every scan it runs — is
    /// skipped outright.
    fn build_attribute_selector_text(
        &self,
        namespace_span: Option<Span>,
        name_span: Span,
        matcher: Option<internal::AttributeMatcher>,
        value_span: Option<Span>,
        flags: Option<&str>,
        span: Span,
    ) -> String {
        if has_comments_to_emit_in_range(self.comments, span.start, span.end) {
            return self.build_commented_attribute_selector_text(
                namespace_span,
                name_span,
                matcher,
                value_span,
                flags,
                span,
            );
        }
        let mut result = String::from("[");
        // `[` is one of the `allow_whitespace()` junctures, so the name can be preceded by a
        // skipped non-ASCII whitespace run the printer would otherwise drop — the same
        // preservation the compound path applies (`preserved_boundary_ws`).
        let leading = namespace_span.unwrap_or(name_span).start;
        result.push_str(self.preserved_boundary_ws(leading));
        if let Some(ns) = namespace_span {
            result.push_str(ns.extract(self.source));
            result.push('|');
        }
        result.push_str(name_span.extract(self.source));
        if let Some(m) = matcher {
            result.push_str(m.as_str());
            self.push_attribute_value(&mut result, value_span);
        }
        if let Some(f) = flags {
            result.push(' ');
            result.push_str(f);
        }
        result.push(']');
        result
    }

    /// [`Self::build_attribute_selector_text`] for a selector that holds a comment: the
    /// same parts, with each interior gap's comments interleaved at their authored
    /// position (the selector is REBUILT here, so nothing else can carry them).
    ///
    /// Two spacings apply, split by [`AttributeGap`]: the spacing-safe gaps pad, the
    /// whitespace-forbidden ones glue. Only the gap holding the comment changes — every
    /// other gap keeps its canonical (empty) spelling, which is what makes the output a
    /// fixed point.
    fn build_commented_attribute_selector_text(
        &self,
        namespace_span: Option<Span>,
        name_span: Span,
        matcher: Option<internal::AttributeMatcher>,
        value_span: Option<Span>,
        flags: Option<&str>,
        span: Span,
    ) -> String {
        // The `]`, and so the far bound of every interior gap.
        let close = span.end - 1;
        let open = span.start + 1;

        let mut result = String::from("[");
        match namespace_span {
            Some(ns) => {
                // `<wq-name>` = `[<ident-token> | '*']? '|' <ident-token>`. The gaps around
                // the `|` are whitespace-forbidden, so their comments glue; the gap between
                // `[` and the prefix is spacing-safe like the rest of the interior.
                self.push_attribute_gap_comments(
                    &mut result,
                    open,
                    ns.start,
                    AttributeGap::AFTER_OPEN,
                );
                // Verbatim, and the separator is then the next non-trivia byte past it —
                // never a scan for `|`, which an escaped one inside the prefix
                // (`[a\|b|attr]`) or one in a comment's content (`[svg/* | */|attr]`)
                // would both answer wrong.
                result.push_str(ns.extract(self.source));
                let pipe = skip_gap_trivia(self.source, ns.end, name_span.start);
                self.push_attribute_gap_comments(&mut result, ns.end, pipe, AttributeGap::Glued);
                result.push('|');
                self.push_attribute_gap_comments(
                    &mut result,
                    pipe + 1,
                    name_span.start,
                    AttributeGap::Glued,
                );
            }
            None => {
                self.push_attribute_gap_comments(
                    &mut result,
                    open,
                    name_span.start,
                    AttributeGap::AFTER_OPEN,
                );
            }
        }
        result.push_str(name_span.extract(self.source));

        let value_end = value_span.map_or(name_span.end, |v| v.end);
        match matcher {
            Some(m) => {
                let value_start = value_span.map_or(close, |v| v.start);
                let m_start = skip_gap_trivia(self.source, name_span.end, value_start);
                self.push_attribute_gap_comments(
                    &mut result,
                    name_span.end,
                    m_start,
                    AttributeGap::INTERIOR,
                );
                let text = m.as_str();
                // A two-character matcher is two `<delim-token>`s, not one token: a comment
                // may split it (glued), a `<whitespace-token>` may not — the parser rejects
                // that, so the gap holds only comments.
                let after_matcher = match text.strip_suffix('=').filter(|head| !head.is_empty()) {
                    Some(head) => {
                        let eq = skip_gap_trivia(self.source, m_start + 1, value_start);
                        result.push_str(head);
                        self.push_attribute_gap_comments(
                            &mut result,
                            m_start + 1,
                            eq,
                            AttributeGap::Glued,
                        );
                        result.push('=');
                        eq + 1
                    }
                    None => {
                        result.push_str(text);
                        m_start + text.len() as u32
                    }
                };
                self.push_attribute_gap_comments(
                    &mut result,
                    after_matcher,
                    value_start,
                    AttributeGap::INTERIOR,
                );
                self.push_attribute_value(&mut result, value_span);
            }
            // A bare presence selector has one interior gap left: name → `]`.
            None => {
                self.push_attribute_gap_comments(
                    &mut result,
                    name_span.end,
                    close,
                    AttributeGap::BEFORE_CLOSE,
                );
            }
        }

        if let Some(f) = flags {
            // The flag token can hold no comment, so its start alone splits the tail
            // into the comments that precede it and those that follow. A padded run
            // supplies the flag's separator, so the plain space goes in only when the
            // gap emitted nothing.
            let flag_start = skip_gap_trivia(self.source, value_end, close);
            if !self.push_attribute_gap_comments(
                &mut result,
                value_end,
                flag_start,
                AttributeGap::INTERIOR,
            ) {
                result.push(' ');
            }
            result.push_str(f);
            self.push_attribute_gap_comments(
                &mut result,
                flag_start,
                close,
                AttributeGap::BEFORE_CLOSE,
            );
        } else if matcher.is_some() {
            self.push_attribute_gap_comments(
                &mut result,
                value_end,
                close,
                AttributeGap::BEFORE_CLOSE,
            );
        }
        result.push(']');
        result
    }

    /// Append an attribute selector's value, re-quoted from its raw token.
    fn push_attribute_value(&self, result: &mut String, value_span: Option<Span>) {
        let Some(vs) = value_span else {
            return;
        };
        match internal::attribute_value_quote_and_text(self.source, vs) {
            (Some(quote), interior) => {
                // Quoted value: optimal-quote re-quoting over the raw interior,
                // swapping only the quote escapes (`[a="it's"]` keeps its double
                // quotes, `[a='it\'s']` becomes `[a="it's"]`).
                result.push_str(&format_string_literal(interior, quote));
            }
            (None, raw) if raw.bytes().any(|b| b == b'\'' || b == b'"') => {
                // A quote character reaches a bare value only via an escape
                // (`[a=x\']`); wrapping it in quotes would need re-escaping, so
                // it stays bare (matches prettier).
                result.push_str(raw);
            }
            (None, raw) => {
                result.push('\'');
                result.push_str(raw);
                result.push('\'');
            }
        }
    }

    /// Append the comments in `[from, to)` to an attribute selector being rebuilt,
    /// spaced per `gap`. Returns whether anything was emitted — a caller that owns its
    /// own separator (the case flag's leading space) must not add one on top of a
    /// padded run.
    fn push_attribute_gap_comments(
        &self,
        result: &mut String,
        from: u32,
        to: u32,
        gap: AttributeGap,
    ) -> bool {
        if from >= to || !has_comments_to_emit_in_range(self.comments, from, to) {
            return false;
        }
        if gap.pad_before() {
            result.push(' ');
        }
        self.push_comment_blocks_in_range(result, from, to, gap.separator());
        if gap.pad_after() {
            result.push(' ');
        }
        true
    }

    /// Fold a pseudo selector's `:name` / `::name` prefix to its canonical case,
    /// returning the text (the args, if any, format separately).
    ///
    /// Svelte decodes the internal name (`:\41 ` → name "A"), but the formatter
    /// keeps it verbatim like class/id/type selectors. Prettier lowercases
    /// case-insensitive pseudo keywords (`:HOVER` → `:hover`, `::-WEBKIT-` →
    /// `::-webkit-`) but preserves custom `:--Name` pseudos and, for an escaped
    /// name, folds only up to the escape's terminator whitespace (`:\4A b` →
    /// `:\4a b`, keeping the literal `B` in `::\41 B`).
    ///
    /// `name_end` comes from the parser (it equals `span.end` when there are no args),
    /// never from a scan for `(`: an identity escape can put one inside the name
    /// (`:foo\(bar(.x)`), where splitting at the first `(` truncates the name and drops
    /// the rest of it from the output.
    fn pseudo_name_text(&self, span: Span, name_end: u32) -> Cow<'a, str> {
        let text = Span {
            start: span.start,
            end: name_end,
        }
        .extract(self.source);
        // The sigil (`:` / `::`) and any comment glued to it are emitted verbatim; only
        // what follows is a name that can fold. Splitting here rather than trimming `:`
        // is what keeps a comment's CONTENT out of the fold (`:/* C */HOVER` must lower
        // the name, never the comment).
        let prefix_len = (crate::comments::pseudo_name_start(self.source.as_bytes(), span.start)
            - span.start) as usize;
        let (prefix, name) = text.split_at(prefix_len.min(text.len()));
        // A custom pseudo (`::--foo`) is emitted verbatim. Otherwise only the head
        // (up to the first whitespace) case-folds; a name whose head carries no
        // ASCII uppercase already IS its canonical form, so borrow the source slice
        // and skip the allocation — the overwhelmingly common case (`:where`,
        // `:hover`, `:root`, `:is`, `:not`). Every pseudo hits this path, so the
        // owned-String-then-pool-copy was the top CSS format-churn site.
        if name.starts_with("--") {
            return Cow::Borrowed(text);
        }
        let head_end = name
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(name.len());
        if name[..head_end].bytes().any(|b| b.is_ascii_uppercase()) {
            // Fold only the head; the tail (from the first whitespace on) is verbatim.
            let mut out = String::with_capacity(text.len());
            out.push_str(prefix);
            out.push_str(&name[..head_end].to_ascii_lowercase());
            out.push_str(&name[head_end..]);
            Cow::Owned(out)
        } else {
            Cow::Borrowed(text)
        }
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
                        return d.text_pooled(&text.to_ascii_lowercase());
                    }
                }
                // Only the NAMESPACED form's span can hold a comment — a `<wq-name>`
                // separator one (`svg/* c */|rect`) — and it rides out inside the raw
                // slice, so the range is declared to `tsv_lang::comment_ledger`. The gap
                // emitters never see it: it sits INSIDE a simple selector's span, not in
                // a boundary gap. The bare form's span is just its name (it ends at the
                // NEXT token's start, and a glued comment IS that token), so the
                // unconditional range stays tight.
                #[cfg(feature = "comment_check")]
                tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);
                self.span_leaf_doc(*span, is_last_in_compound)
            }
            internal::SimpleSelector::Universal {
                namespace_span,
                span,
            } => match namespace_span {
                // The namespaced form is emitted from its own span rather than rebuilt
                // from the prefix, so a `<wq-name>` separator comment (`|/* c */*`)
                // survives glued. The span is exactly the prefix through the `*`; the
                // bare `*` takes the constant instead, its span reaching to the NEXT
                // token's start.
                Some(_) => {
                    #[cfg(feature = "comment_check")]
                    tsv_lang::comment_ledger::record_verbatim_range(
                        self.source,
                        span.start,
                        span.end,
                    );
                    self.span_leaf_doc(*span, is_last_in_compound)
                }
                None => d.text("*"),
            },
            internal::SimpleSelector::Class { span } => {
                // A `.`-glued comment (`./* c */cls`) rides out inside the verbatim
                // slice, like the `<wq-name>` separator's — see the `Type` arm.
                #[cfg(feature = "comment_check")]
                tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);
                self.span_leaf_doc(*span, is_last_in_compound)
            }
            internal::SimpleSelector::Id { span } => self.span_leaf_doc(*span, is_last_in_compound),
            internal::SimpleSelector::Attribute {
                namespace_span,
                name_span,
                matcher,
                value_span,
                flags,
                span,
            } => d.text_pooled(&self.build_attribute_selector_text(
                *namespace_span,
                *name_span,
                *matcher,
                *value_span,
                *flags,
                *span,
            )),
            internal::SimpleSelector::PseudoClass {
                args,
                name_end,
                span,
            } => self.build_pseudo_doc(*span, *name_end, args.as_ref(), is_last_in_compound),
            internal::SimpleSelector::PseudoElement {
                args,
                name_end,
                span,
            } => self.build_pseudo_doc(*span, *name_end, args.as_ref(), is_last_in_compound),
            internal::SimpleSelector::Nesting { .. } => d.text("&"),
            internal::SimpleSelector::Percentage { value, .. } => {
                let mut w = d.pool_writer();
                // PoolTextWriter's write_str never errors; the Result exists
                // only to satisfy `fmt::Write`.
                let _ = write!(w, "{value}%");
                w.finish_text()
            }
            internal::SimpleSelector::Nth { span } => {
                // Normalize An+B operator spacing (`2n+1` → `2n + 1`) to match prettier,
                // exactly like the dedicated `:nth-child` args path. An `An+B of S` term
                // folds ` of ` into the value (matching Svelte — see `match_nth_value`):
                // split it off, normalize the An+B, and re-emit ` of ` with a single
                // trailing space so the following sibling selector (`S`) stays separated
                // in the glued compound.
                let raw = span.extract(self.source);
                match split_nth_of(raw) {
                    Some(anb) => {
                        let mut w = d.pool_writer();
                        w.push_str(&Self::normalize_an_plus_b(anb));
                        w.push_str(" of ");
                        w.finish_text()
                    }
                    None => d.text_pooled(&Self::normalize_an_plus_b(raw)),
                }
            }
            internal::SimpleSelector::Invalid { span } => {
                // A dropped forgiving-list item (`:is(.a > . > .b)`) is emitted
                // verbatim except that its whitespace runs (spaces, tabs, newlines)
                // collapse to single spaces — the same normalization every other
                // selector-argument position gets, and what prettier does inside a
                // selector.
                //
                // The OUTER trim is escape-aware for the same reason the collapse inside
                // is: `:is(.a > . > .b\ )` ends in an escape whose payload is that space,
                // and trimming it strands the backslash onto the `)` that closes the
                // pseudo — output tsv's own parser then rejects.
                //
                // The parser registers whatever comments it read before the parse error,
                // and they ride out inside this slice — see `tsv_lang::comment_ledger`.
                #[cfg(feature = "comment_check")]
                tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);
                let raw = span.extract(self.source);
                let item =
                    crate::escapes::trim_end_preserving_escape(crate::escapes::trim_start_css(raw));
                d.text_pooled(&value_normalization::collapse_whitespace_runs(item))
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
        name_end: u32,
        args: Option<&internal::PseudoClassArgs<'_>>,
        is_last_in_compound: bool,
    ) -> DocId {
        let d = self.d();
        match args {
            Some(args) => {
                // Unlike the no-args branch below, this one emits name + args rather than
                // the whole span, so the sigil-glued comment inside `name` is declared on
                // its own (`:/* c */not(.a)`).
                #[cfg(feature = "comment_check")]
                tsv_lang::comment_ledger::record_verbatim_range(
                    self.source,
                    span.start,
                    crate::comments::pseudo_name_start(self.source.as_bytes(), span.start),
                );
                let name = self.pseudo_name_text(span, name_end);
                d.concat(&[d.text_pooled(&name), self.build_pseudo_args_doc(args)])
            }
            None => {
                // No args: the whole span is the name. Drop a hex escape's whitespace
                // TERMINATOR when it ends the compound (the separator that follows
                // re-terminates it), but never a literal escape's PAYLOAD — the exact
                // rule `span_leaf_doc` applies to type/class/id leaves, on the pseudo
                // path. `:hover\ ` is a pseudo named `hover `; stranding its backslash
                // would escape the `,` or `{` that follows and destroy the selector list.
                //
                // The span also covers an *unreadable* argument list (`:state(/* c */ "x")`,
                // whose args the parser skipped to `)` after registering the gap comment) —
                // those comments ride out inside this verbatim slice. See
                // `tsv_lang::comment_ledger`.
                #[cfg(feature = "comment_check")]
                tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);
                let name = self.pseudo_name_text(span, name_end);
                let text = if is_last_in_compound {
                    crate::escapes::trim_end_preserving_escape(&name)
                } else {
                    name.as_ref()
                };
                d.text_pooled(text)
            }
        }
    }

    /// Build the parenthesized argument doc for a pseudo-class/element.
    ///
    /// Selector-list args (`:is()`, `:not()`, `:where()`, `:has()`, `::slotted()`,
    /// `:nth-child(... of S)`) wrap as `group("(" indent(softline join) softline ")")`
    /// — inline when they fit, one-per-line indented when they don't. `::part()` idents
    /// and identifier args never break.
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
                // A comment inside the An+B text is trivia like any other gap material:
                // the spacing normalizes around it and the comment itself is opaque to
                // the walk, so its content is never rewritten (`/* a-b */` must not
                // become `/* a - b */`). Prettier prints the whole *selector* verbatim
                // whenever it holds a comment — its selector parser gives up before
                // reaching the An+B — so that spelling is a parser artifact, not a rule.
                let normalized = Self::normalize_an_plus_b(value);
                // Comments in the gaps around the An+B text are not part of
                // `value`; interleave them like the SelectorList arm above.
                let leading = self.comment_blocks_in_range(span.start, value_span.start);
                match of_selector {
                    None => {
                        let trailing = self.comment_blocks_in_range(value_span.end, span.end);
                        let inner = self.wrap_inner_with_comments(
                            d.text_pooled(&normalized),
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
                        let inner = d.concat(&[d.text_pooled(&normalized), d.text(" of "), list]);
                        let inner = self.wrap_inner_with_comments(inner, &leading, &trailing);
                        self.wrap_pseudo_args(inner)
                    }
                }
            }
            internal::PseudoClassArgs::Part {
                idents,
                ident_spans,
                span,
            } => {
                // The identifier run spans the first name's start to the last name's
                // end (`ident_spans` is non-empty — the parser rejects empty
                // `::part()`); leading/trailing comments sit outside it.
                let run_span = Span {
                    start: ident_spans[0].start,
                    end: ident_spans[ident_spans.len() - 1].end,
                };
                // Interleave any interior comments between the names, then wrap the
                // leading/trailing comments outside the run
                // (`::part(/* lead */ a /* mid */ b /* trail */)`).
                let run = self.build_part_idents_doc(idents, ident_spans, run_span);
                let inner = self.wrap_args_gap_comments(run, *span, run_span);
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
        // Both gaps sit inside the argument parens, so comment-free parens mean both come
        // back empty and `wrap_inner_with_comments` hands `inner` straight back. One probe
        // replaces the two range-collects on the common path — this fires on every
        // `:not()` / `:where()` / `:is()`, which are dense in real stylesheets.
        if !has_comments_to_emit_in_range(self.comments, args_span.start, args_span.end) {
            return inner;
        }
        let leading = self.comment_blocks_in_range(args_span.start, content_span.start);
        let trailing =
            self.comment_blocks_in_range(content_span.end, args_span.end.saturating_sub(1));
        self.wrap_inner_with_comments(inner, &leading, &trailing)
    }

    /// Build the `::part()` identifier run, interleaving any comment that sits in
    /// an interior gap between two names at its authored position with single-space
    /// separation — the shared selector-comment normalization (`comments_to_emit_in_range`
    /// lookup, the same rule as `:is()`/combinator-boundary comments). The common
    /// comment-free case (`run_span` has no comment, or a single name) stays a
    /// plain single-space join. `run_span` bounds the run between the first and
    /// last name; leading/trailing comments sit outside it and are handled by the
    /// caller's `wrap_args_gap_comments`.
    fn build_part_idents_doc(
        &self,
        idents: &[&str],
        ident_spans: &[Span],
        run_span: Span,
    ) -> DocId {
        let d = self.d();
        if idents.len() < 2
            || !has_comments_to_emit_in_range(self.comments, run_span.start, run_span.end)
        {
            return d.text_pooled(&idents.join(" "));
        }
        let mut parts = DocBuf::new();
        for (i, ident) in idents.iter().enumerate() {
            if i > 0 {
                parts.push(d.text(" "));
                let gap =
                    self.comment_blocks_in_range(ident_spans[i - 1].end, ident_spans[i].start);
                if !gap.is_empty() {
                    parts.push(d.text_pooled(&gap));
                    parts.push(d.text(" "));
                }
            }
            parts.push(d.text_pooled(ident));
        }
        d.concat(&parts)
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
            parts.push(d.text_pooled(leading));
            parts.push(d.text(" "));
        }
        parts.push(inner);
        if !trailing.is_empty() {
            parts.push(d.text(" "));
            parts.push(d.text_pooled(trailing));
        }
        d.concat(&parts)
    }

    /// Wrap pseudo-argument content in literal `(`…`)` with no break points — the
    /// non-breaking counterpart of `wrap_pseudo_args`, for the two argument forms that
    /// never break: the `::part()` idents and the bare `:nth-*()` An+B. The
    /// selector-list forms (`:is()`/`:not()`/`::slotted()`/`:dir()`, and the `:nth-*(…
    /// of S)` of-list) route through the breakable `wrap_pseudo_args` instead.
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
    ///
    /// A `/* … */` comment is **opaque**: it is copied out byte for byte and separated
    /// from its neighbours by a single space, exactly like the whitespace it stands in
    /// for (css-syntax-3 §4.3.2 consumes it to nothing). Reading into it would respace
    /// operator characters in prose (`/* a-b */` → `/* a - b */`) — the value-scanner
    /// opacity class, and the reason this walk steps over a comment through
    /// [`crate::comments`] rather than treating it as content.
    fn normalize_an_plus_b(value: &str) -> Cow<'_, str> {
        // Lowercase the `n` variable when it carries a numeric coefficient
        // (`2N`→`2n`), matching prettier — a bare `N`/`-N` keeps its case (prettier
        // preserves it), as do the `even`/`odd` keywords (their `n` isn't
        // digit-preceded). Applied before the early return so `2N` (no operator)
        // is cased too.
        let cased = lowercase_an_plus_b_n(value.trim());

        // Simple cases: a keyword or plain number with nothing for the walk to do — no
        // operator to respace, no comment to space off — returns the cased form as-is
        // (borrowed when no `n` was recased). Both halves must be asked: a comment is
        // separated from its neighbours whether or not an operator is present, so gating
        // on the operators alone would leave `2n/* c */` glued.
        if !cased.contains(['+', '-']) && !cased.contains("/*") {
            return cased;
        }

        // Normalize spacing around + and - operators. Walk the chars directly (no
        // `Vec<char>` materialization) and build the result already-trimmed, so the
        // single `result` String is the only allocation — `cased` is `value.trim()`d
        // and the loop never emits a leading/trailing space, so no final trim copy
        // is needed.
        let mut result = String::with_capacity(cased.len() + 4);
        let mut rest = cased.as_ref();
        let mut prev_exists = false;

        while let Some(ch) = rest.chars().next() {
            // A comment is trivia standing in for whitespace: copy it verbatim, spaced
            // off from whatever sits on either side of it.
            if let Some(end) = crate::comments::leading_comment_end(rest) {
                if prev_exists && !result.ends_with(' ') {
                    result.push(' ');
                }
                result.push_str(&rest[..end]);
                rest = rest[end..].trim_start();
                if !rest.is_empty() {
                    result.push(' ');
                }
                prev_exists = true;
                continue;
            }

            rest = &rest[ch.len_utf8()..];

            // Handle + and - operators: always normalize to ` op `
            if (ch == '+' || ch == '-') && prev_exists {
                // Operator only when there is content before it (compute the trimmed
                // length before mutating so the immutable borrow is released first).
                let trimmed_len = result.trim_end().len();
                if trimmed_len != 0 {
                    result.truncate(trimmed_len);
                    result.push(' ');
                    result.push(ch);

                    // Skip any spaces after the operator and add a single space
                    // only when more content follows.
                    rest = rest.trim_start();
                    if !rest.is_empty() {
                        result.push(' ');
                    }
                    prev_exists = true;
                    continue;
                }
            }

            result.push(ch);
            prev_exists = true;
        }

        Cow::Owned(result)
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
    // No `N` at all — comment interiors included — means nothing to case, which is every
    // real An+B. A comment that merely spells one takes the walk and comes out unchanged.
    if !s.contains('N') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    let mut prev_digit = false;
    while let Some(ch) = rest.chars().next() {
        // A comment is prose, not a term: `/* 2N */` keeps its case (the value-scanner
        // opacity class — see `normalize_an_plus_b`), and it also breaks the digit run,
        // so an `N` just past it is bare rather than coefficient-carrying — which is what
        // the tokenizer sees too, since `2/* c */N` is a number and an ident, never one
        // `<n-dimension>`.
        if let Some(end) = crate::comments::leading_comment_end(rest) {
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            prev_digit = false;
            continue;
        }
        out.push(if ch == 'N' && prev_digit { 'n' } else { ch });
        prev_digit = ch.is_ascii_digit();
        rest = &rest[ch.len_utf8()..];
    }
    Cow::Owned(out)
}

/// How an attribute selector's interior gap spaces the comments it holds.
///
/// Two answers, and which one a gap takes is a grammar fact rather than a taste call.
/// selectors-4 forbids white space "between **any** of the components of a `<wq-name>`"
/// and "between the components of an `<attr-matcher>`" — a space there would be a
/// `<whitespace-token>` the grammar rejects, while a comment is no token at all
/// (css-syntax-3 §4), so those gaps stay [`Glued`](AttributeGap::Glued). Every other
/// juncture is bounded by the brackets and takes a space safely, so the comment is padded
/// off its neighbours — glued only to `[` and `]` themselves, the answer `:is()` and
/// `::part()` already give inside their parens.
#[derive(Clone, Copy)]
enum AttributeGap {
    /// A `<wq-name>` or `<attr-matcher>` juncture: no space added anywhere, a run
    /// included (the same rule that keeps `.a/* c *//* d */.b` a compound).
    Glued,
    /// A spacing-safe juncture; the flags say which sides get their single space.
    Spaced { before: bool, after: bool },
}

impl AttributeGap {
    /// The gap just inside `[`: glued to the bracket, padded off the name.
    const AFTER_OPEN: Self = Self::Spaced {
        before: false,
        after: true,
    };
    /// A gap between two interior tokens: padded on both sides.
    const INTERIOR: Self = Self::Spaced {
        before: true,
        after: true,
    };
    /// The gap just inside `]`: padded off the content, glued to the bracket.
    const BEFORE_CLOSE: Self = Self::Spaced {
        before: true,
        after: false,
    };

    fn separator(self) -> &'static str {
        match self {
            Self::Glued => "",
            Self::Spaced { .. } => " ",
        }
    }

    fn pad_before(self) -> bool {
        matches!(self, Self::Spaced { before: true, .. })
    }

    fn pad_after(self) -> bool {
        matches!(self, Self::Spaced { after: true, .. })
    }
}

/// The first position in `[from, to)` that begins the region's next real token — the
/// [`crate::comments::skip_trivia_forward`] scan in the printer's `u32` span
/// coordinates.
///
/// The attribute selector is rebuilt from its parts, so the printer locates the parts it
/// has no span for (the `|`, the matcher, the case flag) by stepping over the trivia ahead
/// of them. Comment-aware by construction, so no scan can read a comment's content as
/// structure.
fn skip_gap_trivia(source: &str, from: u32, to: u32) -> u32 {
    debug_assert!(from <= to, "attribute-gap scan bounds inverted");
    crate::comments::skip_trivia_forward(source.as_bytes(), from as usize, to as usize) as u32
}

#[cfg(test)]
mod tests {
    use super::{Printer, lowercase_an_plus_b_n};

    /// Preserved consecutive combinators produce empty-compound `RelativeSelector`s, which
    /// both selector printer paths must handle without panicking or losing idempotency. The
    /// comment-bearing cases exercise `build_complex_selector_doc_with_comments`, whose
    /// `selectors[0]` index panicked on the empty compound (a `_svelte_prettier_divergence`
    /// that can't be a fixture — parseCss rejects a combinator-boundary comment).
    #[test]
    fn preserved_consecutive_combinator_printing_is_stable() {
        use bumpalo::Bump;
        for input in [
            "> > .a { color: red }",
            "+ ~ .d { color: red }",
            ".a > > .b { color: red }",
            "> /* c */ > .a { color: red }",
            ".a > /* c */ > .b { color: red }",
            "> > /* c */ .a { color: red }",
        ] {
            let arena = Bump::new();
            let ast = crate::parse(input, &arena).expect("parses");
            let once = crate::format(&ast, input);
            let arena2 = Bump::new();
            let ast2 = crate::parse(&once, &arena2).expect("reparses");
            let twice = crate::format(&ast2, &once);
            assert_eq!(once, twice, "not idempotent for {input:?}");
        }
    }

    #[test]
    fn test_lowercase_an_plus_b_n() {
        // Digit-preceded `n` lowercases (matches prettier).
        assert_eq!(lowercase_an_plus_b_n("2N"), "2n");
        assert_eq!(lowercase_an_plus_b_n("2N+1"), "2n+1");
        assert_eq!(lowercase_an_plus_b_n("0N"), "0n");
        // Bare `N`/`-N` keep their case (prettier preserves them).
        assert_eq!(lowercase_an_plus_b_n("N"), "N");
        assert_eq!(lowercase_an_plus_b_n("-N+3"), "-N+3");
        // `even`/`odd` are untouched (their `n` is not digit-preceded).
        assert_eq!(lowercase_an_plus_b_n("EVEN"), "EVEN");
        assert_eq!(lowercase_an_plus_b_n("ODD"), "ODD");
        // No `N` anywhere — every real An+B — is borrowed rather than copied.
        assert!(matches!(
            lowercase_an_plus_b_n("2n + 1"),
            std::borrow::Cow::Borrowed(_)
        ));
        // A comment is opaque: its interior keeps its case, and it breaks the digit run,
        // so an `N` just past one is bare (`2/* c */N` is a number and an ident, never
        // one `<n-dimension>`).
        assert_eq!(lowercase_an_plus_b_n("2n /* 2N */ + 1"), "2n /* 2N */ + 1");
        assert_eq!(lowercase_an_plus_b_n("2/* c */N"), "2/* c */N");
    }

    /// The An+B spacing walk treats a comment as one opaque unit of trivia: separated
    /// from its neighbours by a single space, never read into. The fixtures pin the
    /// whole-selector forms; these pin the function on the shapes a fixture cannot
    /// reach (a term that is not a `:nth-*()` argument's whole value).
    #[test]
    fn normalize_an_plus_b_steps_over_comments() {
        // Both interior gaps, glued, space off.
        assert_eq!(
            Printer::normalize_an_plus_b("2n/* c */+1"),
            "2n /* c */ + 1"
        );
        assert_eq!(
            Printer::normalize_an_plus_b("2n+/* c */1"),
            "2n + /* c */ 1"
        );
        // A glued run separates single-spaced, like a glued run in a value list.
        assert_eq!(
            Printer::normalize_an_plus_b("2n/* a *//* b */+1"),
            "2n /* a */ /* b */ + 1"
        );
        // Operator characters inside the comment are never respaced — the corruption the
        // old verbatim freeze existed to avoid.
        assert_eq!(
            Printer::normalize_an_plus_b("2n/* a+b-c */+1"),
            "2n /* a+b-c */ + 1"
        );
        // A comment with no operator beside it still spaces off (the fast path must ask
        // about comments, not just operators).
        assert_eq!(Printer::normalize_an_plus_b("2n/* c */"), "2n /* c */");
        // Already-normalized forms are fixed points.
        assert_eq!(
            Printer::normalize_an_plus_b("2n /* c */ + 1"),
            "2n /* c */ + 1"
        );
    }
}
