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
use super::boundary_ws::{closer_pos, skip_gap_trivia};
use super::value_normalization;
use crate::ast::internal;
use crate::whitespace::Edge;
use crate::whitespace::is_ascii_boundary_whitespace;
use tsv_lang::Span;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::printing::format_string_literal;
use tsv_lang::source_scan;

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
        if self.has_comments_to_emit_between(list.span.start, list.span.end) {
            let doc = self.build_comma_list_doc(list.selectors, false);
            self.write_arena_doc_with_suffix(doc, SELECTOR_SUFFIX_WIDTH);
            return;
        }
        if list.selectors.len() >= 2 {
            // Top-level list: each selector on its own line.
            for (i, complex) in list.selectors.iter().enumerate() {
                if i > 0 {
                    // The comment-free twin of `build_comma_list_doc`'s pre-comma
                    // preservation — this loop writes its `,` directly, so it owes the same
                    // run (`@keyframes k { 0%<NBSP>, 50% {…} }`).
                    let kept =
                        self.pre_comma_boundary_ws(&list.selectors[i - 1], complex.span.start);
                    if !kept.is_empty() {
                        self.write(&kept);
                    }
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
    /// (`@scope (/* c */ .a)`) sit outside `list.span`, and so does a boundary run at either
    /// end (`@scope (.a <NBSP>)`), so both are claimed here by
    /// [`Self::build_paren_selector_list_inner`] — the builder `:is()`'s argument uses.
    pub(super) fn print_selector_list_nested(
        &mut self,
        list: &internal::SelectorList<'_>,
        paren_span: Option<Span>,
    ) {
        if list.selectors.is_empty() {
            return;
        }
        let d = self.d();
        let inner = match paren_span {
            Some(paren_span) => self.build_paren_selector_list_inner(list, paren_span),
            None => self.build_nested_selector_list_doc(list),
        };
        // The caller already wrote `(`; emit `softline inner` indented, then a
        // trailing softline so the closing `)` (written by the caller) lands at the
        // base level when broken. Reserve `) {`-ish via a 3-col suffix.
        let doc = d.group(d.concat(&[d.indent(d.concat(&[d.softline(), inner])), d.softline()]));
        self.write_arena_doc_with_suffix(doc, 3);
    }

    /// The contents of a selector list in PARENS — a pseudo-class's argument list (`:is()`,
    /// `:where()`, `:has()`, …) or an `@scope` clause — with every gap between the parens and
    /// the list claimed: `paren` is the `(…)` span (end one past the `)`).
    ///
    /// One builder for every such list, because the gaps are one juncture kind: the parser
    /// steps a boundary run at the list's start and at its `)` in both places, and a claim
    /// made at one of them and not the other is the bug spelled once (`@scope (.a <NBSP>)` lost
    /// its run while `:is(a <NBSP>)` kept it).
    ///
    /// - The LEAD gap, which only the container can claim whole: the list's first compound
    ///   reaches the run CONTIGUOUS with its name by scanning back (`:is(<NBSP>b)`), but it has
    ///   no floor to sweep forward from, so a member a comment strands after the `(` is
    ///   invisible to it (`:is(<NBSP>/* c */ a)`). Claimed BEFORE the list is built, so its
    ///   first anchor stands down.
    /// - The run between the last selector and the `)` — see `spell_gap_items`. The edge
    ///   ahead of it is what keeps the last selector's own name off it (`:is(a <NBSP>)`
    ///   re-parsed as `:is(a<NBSP>)`).
    ///
    /// The comments inside the parens but outside the list (`:is(/* lead */ .a /* trail */)`)
    /// are interleaved on each side the two claims did not already print in place.
    pub(super) fn build_paren_selector_list_inner(
        &self,
        list: &internal::SelectorList<'_>,
        paren: Span,
    ) -> DocId {
        let d = self.d();
        let lead = self.claim_lead_gap(paren.start, first_anchor(list), Edge::Flush);
        let inner = self.build_nested_selector_list_doc(list);
        let kept = self.spell_gap_before_closer(list.span.end, paren, self.list_run_edge(list));
        let inner = self.wrap_args_gap_comments(
            inner,
            paren,
            list.span,
            (lead.is_some(), !kept.is_empty()),
        );
        let lead = lead.unwrap_or_default();
        if kept.is_empty() && lead.is_empty() {
            inner
        } else {
            d.concat(&[d.text_pooled(&lead), inner, d.text_pooled(&kept)])
        }
    }

    /// Build a doc for a selector list joined by `,`-`line` — the nested/forgiving
    /// form used inside pseudo arguments and `@scope`. Each selector is a full
    /// complex-selector doc; the group around it (added by the caller) decides
    /// whether the `line`s flatten to `, ` or break one-per-line.
    fn build_nested_selector_list_doc(&self, list: &internal::SelectorList<'_>) -> DocId {
        self.build_comma_list_doc(list.selectors, true)
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
    ///
    /// Takes the bare selector slice rather than a `SelectorList`, so it is the one spelling
    /// of the comma seam for every list, including the one with no `SelectorList` node of
    /// its own: a `@supports selector(.a, /* c */ .b)` argument
    /// (`ConditionSegment::Selectors`). Joining that one with a literal `", "` dropped every
    /// comma-adjacent comment — the alternate-layout-builder hazard (docs/comments.md
    /// hazard 4); the seam's two runs must partition the gap wherever a comma separates two
    /// selectors.
    pub(super) fn build_comma_list_doc(
        &self,
        selectors: &[internal::ComplexSelector<'_>],
        breakable: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for (i, complex) in selectors.iter().enumerate() {
            if i > 0 {
                let prev = &selectors[i - 1];
                // The common gap — no comment, no member — is the comma and its separator.
                if !self.has_comments_to_emit_between(prev.span.end, complex.span.start)
                    && !self.gap_may_hold_boundary_ws(prev.span.end, complex.span.start)
                {
                    parts.push(d.text(","));
                    parts.push(if breakable { d.line() } else { d.text(" ") });
                    parts.push(self.build_complex_selector_doc(complex));
                    continue;
                }
                // Each side of the comma is one gap, and a gap holding a boundary member is
                // printed whole by the run's claim, its comments in place among the members
                // (`Printer::spell_gap_items`); only a member-free side keeps the comment
                // spelling below. The comma is found comment-aware (a `,` inside a comment is
                // not the separator).
                let comma = self.comma_between(prev.span.end, complex.span.start);
                // A side's comments are collected only where no claim prints them: the
                // collector records each one it returns as printed, so asking a claimed side
                // would report its comments printed twice.
                let kept = self.pre_comma_boundary_ws(prev, complex.span.start);
                if kept.is_empty() {
                    // The comments trailing the previous selector.
                    let before = self.comment_blocks_in_range(prev.span.end, comma);
                    if !before.is_empty() {
                        parts.push(d.text(" "));
                        parts.push(d.text_pooled(&before));
                    }
                } else {
                    parts.push(d.text_pooled(&kept));
                }
                parts.push(d.text(","));
                parts.push(if breakable { d.line() } else { d.text(" ") });
                // The comma→selector gap: the next selector's first anchor claims only the
                // run contiguous with it, so a member a comment strands ahead of that run
                // (`a, <NBSP>/* c */ b`) is this seam's or nobody's.
                match self.claim_lead_gap(comma + 1, complex.span.start, Edge::Flush) {
                    Some(lead) => parts.push(d.text_pooled(&lead)),
                    None => {
                        // The comments leading the next selector.
                        let after = self.comment_blocks_in_range(comma, complex.span.start);
                        if !after.is_empty() {
                            parts.push(d.text_pooled(&after));
                            parts.push(d.text(" "));
                        }
                    }
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
            // Everything this relative selector may preserve lies past the previous one's
            // end — see `preserved_boundary_ws`. Spelled exactly as the comment-bearing twin
            // spells it, since the two answer the same question and a floor that differs
            // between them is the bug this floor exists for.
            let floor = (i > 0).then(|| complex.children[i - 1].span.end);
            let anchor_floor = Self::compound_anchor_floor(rel, floor);
            if let Some(combinator) = rel.combinator {
                // A preserved anchorless combinator (an empty compound — a consecutive
                // combinator like the leading `>` of `> > .a`) emits just its symbol, with
                // no trailing space: the following relative selector's separator supplies
                // the single space, so the run reads `> > .a`, not `>  > .a`. Such an RS
                // always carries an explicit combinator (Descendant is never anchorless).
                if rel.selectors.is_empty() {
                    // The symbol is this compound's only anchor, so it carries the gap's run
                    // too — the arm that forgot it was the one juncture of the family with no
                    // restore at all (`:has(<NBSP>> > a)`).
                    if i > 0 {
                        parts.push(d.line());
                    }
                    self.push_combinator_boundary_ws(
                        &mut parts,
                        floor,
                        rel.combinator_span,
                        Edge::Flush,
                        Edge::Presence,
                    );
                    parts.push(d.text(combinator.as_str()));
                } else if i == 0 {
                    // Leading combinator (e.g. `:has(> img)`): no break before it — but the
                    // run `parse_explicit_combinator` stepped ahead of it still belongs to
                    // the output (`:has(<NBSP>> b)`).
                    self.push_combinator_boundary_ws(
                        &mut parts,
                        floor,
                        rel.combinator_span,
                        Edge::Flush,
                        Edge::Presence,
                    );
                    let s = Self::leading_combinator_str(combinator);
                    if !s.is_empty() {
                        parts.push(d.text(s));
                    }
                } else {
                    // A Descendant carries no symbol of its own, so its run is claimed by
                    // the next compound's `preserved_boundary_ws` — claiming it here too
                    // would print it twice.
                    if combinator != internal::Combinator::Descendant {
                        // The separator doc opens with the `line` that stands AFTER the run,
                        // so the run's right edge is the printer's; its left edge is the
                        // author's — or a name's, which the run would otherwise glue into.
                        let lead = floor.map_or(Edge::Flush, |floor| self.name_run_edge(floor));
                        self.push_combinator_boundary_ws(
                            &mut parts,
                            floor,
                            rel.combinator_span,
                            lead,
                            Edge::Flush,
                        );
                    }
                    parts.push(self.combinator_separator_doc(combinator));
                }
            }
            let n = rel.selectors.len();
            for (j, simple) in rel.selectors.iter().enumerate() {
                if j == 0 {
                    let kept = self.gap_boundary_ws(anchor_floor, simple.span().start);
                    if !kept.is_empty() {
                        parts.push(d.text_pooled(&kept));
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

    /// Emit the boundary run stepped in an EXPLICIT combinator's own gap (`*<NBSP>> b`),
    /// ahead of the symbol.
    ///
    /// The combinator is the only anchor such a run has: it sits between two compounds, so no
    /// simple selector's start reaches back over it. Every arm of the combinator branch —
    /// leading (`:has(<NBSP>> b)`), interior, and the anchorless one (`:has(<NBSP>> > a)`,
    /// an empty compound whose symbol is its only anchor) — goes through here, because they
    /// differ only in whether a break precedes the symbol, not in what the gap owes.
    ///
    /// The two edges are the CALLER's, because the arms differ in what they print around the
    /// run ([`Edge`]). Where the separator doc follows (`combinator_separator_doc` opens with a
    /// `line`), the right edge is flush — the author's trailing space would stack with the
    /// regenerated one and grow a column on every pass — and the left edge is the author's,
    /// or [`Edge::Space`] where the previous compound ends in a name the run would glue into
    /// (`a <NBSP>> b` came back as the name `a<NBSP>`, a selector that matches nothing and is
    /// its own fixed point). Where the arm writes its own space ahead of the run and the
    /// symbol flush behind it (the anchorless combinator, the comment-bearing builder), the
    /// edges swap. The floor is the position to ask at — it *is* the previous compound's end
    /// — and its absence means the gap opens on a `(` or the selector's own start, where only
    /// the contiguous run is claimed, flush on its left.
    fn push_combinator_boundary_ws(
        &self,
        parts: &mut DocBuf,
        floor: Option<u32>,
        combinator_span: Option<Span>,
        lead: Edge,
        trail: Edge,
    ) {
        let Some(cs) = combinator_span else {
            return;
        };
        let kept = match floor {
            Some(floor) => self.spell_gap_items(floor, cs.start, lead, trail),
            None => self.contiguous_boundary_ws(0, cs.start, trail),
        };
        if !kept.is_empty() {
            parts.push(self.d().text_pooled(&kept));
        }
    }

    /// The floor for the gap ending at a compound's first simple selector: past an EXPLICIT
    /// combinator's symbol when the compound has one — everything before that symbol is the
    /// symbol's own claim ([`Self::push_combinator_boundary_ws`]), and re-claiming it here
    /// would print it twice — else the previous relative selector's end, and `None` for the
    /// first compound of the selector.
    ///
    /// A **descendant** combinator's span is the whitespace itself (it ends where the next
    /// compound starts), so it is deliberately not a floor: its gap belongs to the compound
    /// that follows it, whole.
    fn compound_anchor_floor(
        rel: &internal::RelativeSelector<'_>,
        prev_end: Option<u32>,
    ) -> Option<u32> {
        match (rel.combinator, rel.combinator_span) {
            (Some(c), Some(cs)) if c != internal::Combinator::Descendant => Some(cs.end),
            _ => prev_end,
        }
    }

    /// The boundary run a selector list's `,` juncture skipped, for emission flush against
    /// the comma.
    ///
    /// Bounded at the COMMA rather than at the next selector: a run on the far side is that
    /// selector's LEADING one, which its own `preserved_boundary_ws` already claims, and
    /// claiming it here too would print it twice. Both selector-list emitters call this —
    /// the doc builder and the comment-free top-level loop, which writes its `,` directly —
    /// because a preservation that only one of them makes is a drop through the other.
    ///
    /// ⚠️ Flush against the comma is safe on the run's RIGHT and says nothing about its left,
    /// which is the selector that just ended — so the left edge is the author's separation,
    /// forced to a space wherever that selector ends in a name (`a.x <NBSP>, c` re-parsed
    /// with the class `x<NBSP>`; `a[b] <NBSP>, c` and `* <NBSP>, c` never could, and neither
    /// can an `Nth` term — `selector_run_edge`). Included in the returned string rather
    /// than left to the two callers, which is what keeps them from disagreeing about it the
    /// way they once disagreed about the run itself.
    fn pre_comma_boundary_ws(
        &self,
        prev: &internal::ComplexSelector<'_>,
        next_start: u32,
    ) -> String {
        let prev_end = prev.span.end;
        // An all-ASCII gap can hold no member; settle before the comma scan (see
        // `spell_gap`, whose own guard this mirrors one step earlier — bounds check
        // included, so a caller's inverted or out-of-range pair can never slice).
        if !self.gap_may_hold_boundary_ws(prev_end, next_start) {
            return String::new();
        }
        let comma = self.comma_between(prev_end, next_start);
        self.spell_gap_items(prev_end, comma, self.selector_run_edge(prev), Edge::Flush)
    }

    /// The offset of the comma separating two selectors in the gap `[from, to)`, found
    /// comment-aware (a `,` inside a comment is not the separator); `to` when there is none,
    /// which only a malformed span pair could produce.
    fn comma_between(&self, from: u32, to: u32) -> u32 {
        source_scan::find_char_skipping_comments(
            self.source.as_bytes(),
            from as usize,
            to as usize,
            b',',
        )
        .map_or(to, |pos| pos as u32)
    }

    /// [`Self::name_run_edge`] for a run restored after a complex selector — the space that
    /// keeps its last NAME closed — unless the selector ends in an `Nth` term.
    ///
    /// An An+B term re-parses through the byte scanner (`match_nth_value` /
    /// `nth_arg_terminator`), not `read_identifier`: the scanner steps the run as the
    /// terminator's gap, so `2n<NBSP>)` comes back the same `Nth` with nothing glued, and a
    /// space ahead of the run would be an insertion the author did not write and prettier
    /// does not make (`:is(2n<NBSP>)`). The name predicate alone cannot tell — an An+B ends
    /// in a digit or an `n`, both of which continue a name.
    fn selector_run_edge(&self, complex: &internal::ComplexSelector<'_>) -> Edge {
        let ends_in_nth = complex
            .children
            .last()
            .and_then(|relative| relative.selectors.last())
            .is_some_and(|simple| matches!(simple, internal::SimpleSelector::Nth { .. }));
        if ends_in_nth {
            Edge::Presence
        } else {
            self.name_run_edge(complex.span.end)
        }
    }

    /// [`Self::selector_run_edge`] for the run a selector LIST's `)` gap restores: the list's
    /// last selector decides. An empty list has no name to close.
    fn list_run_edge(&self, list: &internal::SelectorList<'_>) -> Edge {
        list.selectors
            .last()
            .map_or(Edge::Presence, |complex| self.selector_run_edge(complex))
    }

    /// The `An+B` head and the ` of ` keyword text of an `:nth-*(An+B of S)` argument, with
    /// the boundary runs the parser skipped on either side of `of` restored.
    ///
    /// The gap `[value_span.end, selectors.span.start)` is this arm's whole wherever it holds
    /// a member, its comments in place; a member-free gap is the plain `" of "` with its
    /// comments leading `S`. Two halves:
    ///
    /// - the run BEFORE `of` (`2n + 1<NBSP> of`) has nothing but this arm to carry it, so it
    ///   is swept whole and printed after the An+B text — flush where the author glued it,
    ///   one space off where they did not (`2n + 1 <NBSP> of`) — ahead of the regenerated
    ///   space;
    /// - what follows the keyword is `S`'s lead gap, which this arm claims whole through
    ///   `claim_lead_gap` (so `S`'s first anchor stands down) where it holds a member, behind
    ///   the keyword's own space only where the author separated the gap from `of`: glued
    ///   (`of<NBSP>.x`, `of/* c */<NBSP> .x`) the gap IS the separator, as the author,
    ///   canonical and prettier all spell it. A member-free half keeps the plain spelling,
    ///   its comments where that spelling puts them — before the keyword, or leading `S`.
    ///
    /// The keyword is located by stepping the gap's trivia (`skip_gap_trivia`): the parser
    /// keeps no span for it. Settled to the plain `" of "` on the document precondition —
    /// this runs once per `of S` argument, and no real stylesheet holds a member.
    fn nth_of_keyword<'v>(
        &self,
        head: Cow<'v, str>,
        value_span: Span,
        selectors: &internal::SelectorList<'_>,
    ) -> NthOf<'v> {
        let plain = |head| NthOf {
            head,
            of: Cow::Borrowed(" of "),
            comments_after: self.comment_blocks_in_range(value_span.end, selectors.span.start),
        };
        if !self.gap_holds_member(value_span.end, selectors.span.start) {
            return plain(head);
        }
        let of_start = skip_gap_trivia(self.source, value_span.end, selectors.span.start);
        // The keyword is two ASCII letters, whatever their case.
        let of_end = (of_start + 2).min(selectors.span.start);
        // Each half of the gap is claimed whole — its comments in place — where it holds a
        // member, and otherwise keeps its comments where the member-free spelling puts them:
        // the half before `of` right where it stands, the half after it leading `S`.
        let before = if self.gap_holds_member(value_span.end, of_start) {
            self.spell_gap_items(value_span.end, of_start, Edge::Presence, Edge::Flush)
        } else {
            let comments = self.comment_blocks_in_range(value_span.end, of_start);
            if comments.is_empty() {
                comments
            } else {
                format!(" {comments}")
            }
        };
        // What follows is `S`'s lead gap. Holding a member, it is claimed whole here, its
        // comments in place — `S`'s first anchor has no floor to reach past a comment with —
        // and it keeps the author's separation from the keyword: a member or a comment glued
        // to `of` stays glued (`of<NBSP>.x`, `of/* c */<NBSP>.x`), the gap then being the
        // separator itself. A member-free gap gets the keyword's own space, its comments
        // leading `S` as they always have.
        let mut after = String::new();
        let comments_after = match self.claim_lead_gap(of_end, first_anchor(selectors), Edge::Flush)
        {
            Some(lead) => {
                let spaced = self.source[of_end as usize..].starts_with(|c: char| {
                    crate::whitespace::is_boundary_whitespace(c)
                        && !crate::whitespace::is_boundary_only_whitespace(c)
                });
                if spaced {
                    after.push(' ');
                }
                after.push_str(&lead);
                String::new()
            }
            None => {
                after.push(' ');
                self.comment_blocks_in_range(of_end, selectors.span.start)
            }
        };
        let head = if before.is_empty() {
            head
        } else {
            Cow::Owned(head.into_owned() + &before)
        };
        NthOf {
            head,
            of: of_keyword_text("", &after),
            comments_after,
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
        // Every gap the loop below probes lies inside the selector's own span, and a
        // comment registers only when it sits fully inside the queried range — so a
        // comment-free selector has no boundary comment. One probe answers that, instead
        // of one per simple selector.
        if !self.has_comments_to_emit_between(complex.span.start, complex.span.end) {
            return false;
        }
        let mut prev_end = complex.span.start;
        for rel in complex.children {
            for simple in rel.selectors {
                let span = simple.span();
                if self.has_comments_to_emit_between(prev_end, span.start) {
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
            let floor = (i > 0).then_some(prev_end);
            if rel.selectors.is_empty() {
                if let Some(combinator) = rel.combinator {
                    if i > 0 {
                        // Explicit space (this path renders inline, no `line()` break points).
                        parts.push(d.text(" "));
                    }
                    // A gap holding a member is the run claim's whole, comments in place.
                    if let Some(cs) = rel.combinator_span {
                        let before = self.comment_blocks_unless_claimed(prev_end, cs.start);
                        if !before.is_empty() {
                            parts.push(d.text_pooled(&before));
                            parts.push(d.text(" "));
                        }
                    }
                    // The symbol is this compound's only anchor, so it carries the gap's run
                    // too — the same claim the comment-free twin makes here. The space this
                    // path writes ahead of it stands on the run's left; the symbol follows
                    // flush, so the right edge is the author's.
                    self.push_combinator_boundary_ws(
                        &mut parts,
                        floor,
                        rel.combinator_span,
                        Edge::Flush,
                        Edge::Presence,
                    );
                    parts.push(d.text(combinator.as_str()));
                }
                prev_end = rel.span.end;
                continue;
            }
            let first_start = rel.selectors[0].span().start;
            if let Some(combinator) = rel.combinator {
                if i == 0 {
                    // Leading combinator (`:has(> /* c */ img)`): the run ahead of the
                    // symbol, the symbol, then any comment sitting between it and the first
                    // compound.
                    self.push_combinator_boundary_ws(
                        &mut parts,
                        floor,
                        rel.combinator_span,
                        Edge::Flush,
                        Edge::Presence,
                    );
                    let s = Self::leading_combinator_str(combinator);
                    if !s.is_empty() {
                        parts.push(d.text(s));
                    }
                    // …unless the gap holds a member: the first compound's claim then prints
                    // it whole, its comments in place (`gap_boundary_ws`, floored at `cs.end`).
                    if let Some(cs) = rel.combinator_span {
                        let after = self.comment_blocks_unless_claimed(cs.end, first_start);
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
                    !self.has_comments_to_emit_between(complex.span.start, first_start),
                    "leading gap comment before the first compound has no emission path"
                );
            }
            let anchor_floor = Self::compound_anchor_floor(rel, floor);
            let n = rel.selectors.len();
            for (j, simple) in rel.selectors.iter().enumerate() {
                let sspan = simple.span();
                if j == 0 {
                    // The same claim the comment-free twin makes, through the same helper:
                    // floored on the previous relative selector (or on an explicit
                    // combinator's symbol), so a run the preceding NAME carried out inside
                    // its own span is not emitted twice (`a<NBSP> b /* c */ d`), and swept
                    // FORWARD from that floor as well, so a run a comment strands earlier in
                    // the gap is not dropped (`a <NBSP>/* c */ b`). Both failures were live,
                    // and only on this path — a comment anywhere in the selector routes here.
                    let kept = self.gap_boundary_ws(anchor_floor, sspan.start);
                    if !kept.is_empty() {
                        parts.push(d.text_pooled(&kept));
                    }
                }
                if j > 0 && self.has_comments_to_emit_between(prev_end, sspan.start) {
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
        // A side of the gap that holds a boundary member is printed whole by the run's claim,
        // its comments in place among the members (`Printer::spell_gap_items`) — the next
        // compound's `gap_boundary_ws` for the side ahead of it, `push_combinator_boundary_ws`
        // for the side ahead of an explicit symbol. Only a member-free side keeps the comment
        // spelling here.
        let comments = |from: u32, to: u32| self.comment_blocks_unless_claimed(from, to);
        match combinator {
            internal::Combinator::Descendant => {
                let gap = comments(gap_start, gap_end);
                if !gap.is_empty() {
                    parts.push(d.text_pooled(&gap));
                    parts.push(d.text(" "));
                }
            }
            other => {
                let (before, after) = match combinator_span {
                    Some(cs) => (comments(gap_start, cs.start), comments(cs.end, gap_end)),
                    None => (comments(gap_start, gap_end), String::new()),
                };
                if !before.is_empty() {
                    parts.push(d.text_pooled(&before));
                    parts.push(d.text(" "));
                }
                // The run on the symbol's own side of the gap, flush against it — the
                // compound that follows floors its claim past `cs.end`, so this half is the
                // symbol's or nobody's.
                // This path writes its own space on the run's left and the symbol flush on
                // its right, so only the right edge is the author's.
                self.push_combinator_boundary_ws(
                    &mut parts,
                    Some(gap_start),
                    combinator_span,
                    Edge::Flush,
                    Edge::Presence,
                );
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
    ///
    /// The leaf is the document's own bytes, so it is emitted as a `source_span` — no
    /// copy into the arena's text pool, and its width read off the host's word rather
    /// than a scalar walk over a bare slice (see `DocArena::source_span`). The trim only
    /// ever shortens the END, so the kept text is still the slice at `span.start`; the
    /// slice itself is materialized only when there is a trim to run.
    fn span_leaf_doc(&self, span: Span, is_last_in_compound: bool) -> DocId {
        let kept = if is_last_in_compound {
            let text = crate::escapes::trim_end_preserving_escape(span.extract(self.source));
            Span {
                start: span.start,
                end: span.start + text.len() as u32,
            }
        } else {
            span
        };
        self.d().source_span(kept, self.source)
    }

    /// Reconstruct an attribute selector (`[ns|name op 'value' flags]`) from source.
    /// The namespace prefix and the name are emitted raw (escapes preserved — `[f\oo]`
    /// stays `[f\oo]`, `[a\|b|attr]` keeps the escape that would otherwise read as the
    /// `<wq-name>` separator); the value is re-quoted from its raw token like a
    /// declaration string (the delimiter that needs fewer escapes wins, ties prefer
    /// single — matches prettier), except that a bare value stays bare when quoting it
    /// would force a re-escape or move a boundary run out of it; the tail — the case flag
    /// and whatever surrounds it — is spelled from source by [`Self::push_attribute_tail`],
    /// so the AST's `flags` is the wire's alone.
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
        span: Span,
    ) -> String {
        if self.has_comments_to_emit_between(span.start, span.end) {
            return self.build_commented_attribute_selector_text(
                namespace_span,
                name_span,
                matcher,
                value_span,
                span,
            );
        }
        let close = closer_pos(span);
        let mut result = String::from("[");
        // `[` is one of the `allow_whitespace()` junctures, so the name can be preceded by a
        // skipped non-ASCII whitespace run the printer would otherwise drop — the same
        // preservation the compound path applies (`preserved_boundary_ws`).
        let leading = namespace_span.unwrap_or(name_span).start;
        result.push_str(&self.preserved_boundary_ws(span.start, leading));
        if let Some(ns) = namespace_span {
            result.push_str(ns.extract(self.source));
            result.push('|');
        }
        result.push_str(name_span.extract(self.source));
        // A matcher always brings a value (the parser reads them as one), so the two gaps
        // around it are claimed together — and SEPARATELY, each flush against the token that
        // follows it: the name→matcher run stays on its side of the `=` (`[a <NBSP>=b]`), the
        // matcher→value run on its (`[a=<NBSP>b]`). One sweep over both moved the first run
        // across the matcher, where it re-tokenizes as the head of the value.
        let tail_from = match (matcher, value_span) {
            (Some(m), Some(vs)) => {
                // The matcher has no span of its own: it is the first non-trivia byte past the
                // name (no comment can sit here — the commented twin took those).
                let m_start = skip_gap_trivia(self.source, name_span.end, vs.start);
                // Each run keeps the author's separation from the token after it — a quoted
                // value glued to a run is a different token sequence to css-syntax-3.
                let kept = self.spell_gap(name_span.end, m_start, Edge::Flush, Edge::Presence);
                self.push_boundary_ws_after_name(&mut result, &kept);
                let text = m.as_str();
                result.push_str(text);
                let kept = self.spell_gap(
                    m_start + text.len() as u32,
                    vs.start,
                    Edge::Flush,
                    Edge::Presence,
                );
                self.push_boundary_ws_after_name(&mut result, &kept);
                self.push_attribute_value(&mut result, vs);
                vs.end
            }
            _ => name_span.end,
        };
        self.push_attribute_tail(&mut result, tail_from, close);
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
        span: Span,
    ) -> String {
        // The `]`, and so the far bound of every interior gap.
        let close = closer_pos(span);
        let open = span.start + 1;

        let mut result = String::from("[");
        match namespace_span {
            Some(ns) => {
                // `<wq-name>` = `[<ident-token> | '*']? '|' <ident-token>`. The gaps around
                // the `|` are whitespace-forbidden, so their comments glue; the gap between
                // `[` and the prefix is spacing-safe like the rest of the interior.
                self.push_attribute_gap(&mut result, open, ns.start, AttributeGap::AFTER_OPEN);
                // Verbatim, and the separator is then the next non-trivia byte past it —
                // never a scan for `|`, which an escaped one inside the prefix
                // (`[a\|b|attr]`) or one in a comment's content (`[svg/* | */|attr]`)
                // would both answer wrong.
                result.push_str(ns.extract(self.source));
                let pipe = skip_gap_trivia(self.source, ns.end, name_span.start);
                self.push_attribute_gap(&mut result, ns.end, pipe, AttributeGap::Glued);
                result.push('|');
                self.push_attribute_gap(
                    &mut result,
                    pipe + 1,
                    name_span.start,
                    AttributeGap::Glued,
                );
            }
            None => {
                self.push_attribute_gap(
                    &mut result,
                    open,
                    name_span.start,
                    AttributeGap::AFTER_OPEN,
                );
            }
        }
        result.push_str(name_span.extract(self.source));

        let tail_from = match (matcher, value_span) {
            (Some(m), Some(vs)) => {
                let m_start = skip_gap_trivia(self.source, name_span.end, vs.start);
                self.push_attribute_gap(
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
                        let eq = skip_gap_trivia(self.source, m_start + 1, vs.start);
                        result.push_str(head);
                        self.push_attribute_gap(&mut result, m_start + 1, eq, AttributeGap::Glued);
                        result.push('=');
                        eq + 1
                    }
                    None => {
                        result.push_str(text);
                        m_start + text.len() as u32
                    }
                };
                self.push_attribute_gap(
                    &mut result,
                    after_matcher,
                    vs.start,
                    AttributeGap::INTERIOR,
                );
                self.push_attribute_value(&mut result, vs);
                vs.end
            }
            _ => name_span.end,
        };
        self.push_attribute_tail(&mut result, tail_from, close);
        result.push(']');
        result
    }

    /// Append an attribute selector's value, re-quoted from its raw token — unless a boundary
    /// run is glued to a bare one, which then stays bare and verbatim.
    ///
    /// The run is a separator to `parseCss` (the value ends at it) and identifier content to
    /// css-syntax-3, whose whitespace is ASCII only — so `[a=b<NBSP>]` is the value `b` to
    /// Svelte and the one ident `b<NBSP>` to a browser, and `[a=<NBSP>b]` the value `b` and
    /// the ident `<NBSP>b`. Re-quoting moves the run out of the ident (`[a='b'<NBSP>]`), which
    /// re-tokenizes as a string followed by a stray token and turns a selector the browser
    /// matched into one it drops. The author's bytes are the one spelling both readers take
    /// as they took the input, so they are what this emits; the run itself rides in the gap
    /// claim beside the value.
    fn push_attribute_value(&self, result: &mut String, vs: Span) {
        match internal::attribute_value_quote_and_text(self.source, vs) {
            (Some(quote), interior) => {
                // Quoted value: optimal-quote re-quoting over the raw interior,
                // swapping only the quote escapes (`[a="it's"]` keeps its double
                // quotes, `[a='it\'s']` becomes `[a="it's"]`).
                result.push_str(&format_string_literal(interior, quote));
            }
            (None, raw)
                if raw.bytes().any(|b| b == b'\'' || b == b'"')
                    || self.bare_value_touches_boundary_run(vs) =>
            {
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

    /// Whether a boundary run stands flush against either end of a bare attribute value in
    /// source — the glue [`Self::push_attribute_value`] keeps.
    ///
    /// The parser ended the value at the run (`boundary_split_offset`) or stepped it ahead of
    /// the value (the matcher→value juncture), so the run is never inside `vs`; it is the
    /// character just past it, or just before it.
    fn bare_value_touches_boundary_run(&self, vs: Span) -> bool {
        if !self.holds_boundary_ws {
            return false;
        }
        let touches =
            |c: Option<char>| c.is_some_and(crate::whitespace::is_boundary_only_whitespace);
        touches(self.source[..vs.start as usize].chars().next_back())
            || touches(self.source[vs.end as usize..].chars().next())
    }

    /// Append an attribute selector's tail — everything from the value's end (or, with no
    /// matcher, the name's) to its `]` — as the author tokenized it.
    ///
    /// The tail holds at most a case flag and comments, and the ASCII whitespace between them
    /// normalizes as everywhere else: one space ahead of the flag, one around a comment, none
    /// against the `]`. A boundary run changes what that whitespace means. To css-syntax-3
    /// the run is identifier content and the space beside it the token separator — `b<NBSP>i`
    /// is one ident, `b<NBSP> i` a value and a flag — while to `parseCss` both are the value
    /// `b` with the flag `i`. Keeping the author's bytes is the one spelling both readers take
    /// as they took the input, so the run is emitted where it stood (never hoisted ahead of
    /// the flag, as it once was), a flag glued to a run stays glued, and ASCII whitespace
    /// beside a run keeps its PRESENCE as a single space. A trailing ASCII run before the
    /// `]` and the count of an interior one are the only spellings normalized away, since
    /// no tokenizer reads either.
    ///
    /// A comment is padded off its neighbours as the interior gaps pad theirs
    /// (`AttributeGap`): it is no token to any reader, so the space costs nothing.
    fn push_attribute_tail(&self, result: &mut String, from: u32, close: u32) {
        if self.gap_holds_member(from, close) {
            self.push_attribute_tail_in_place(result, from, close);
            return;
        }
        let (from, close) = (from as usize, close as usize);
        if from >= close {
            return;
        }
        let gap = &self.source[from..close];
        let bytes = gap.as_bytes();
        // What this walk last emitted — what the space ahead of a flag is decided against.
        enum Last {
            Start,
            Run,
            Comment,
            Flag,
        }
        let mut last = Last::Start;
        // ASCII whitespace seen since the last emitted item: a separator to keep beside a
        // run, the ordinary normalized gap otherwise.
        let mut pending_space = false;
        let mut i = 0;
        while i < gap.len() {
            if crate::comments::is_comment_start(bytes, i) {
                let end = crate::comments::comment_end(bytes, i);
                if !result.ends_with(' ') {
                    result.push(' ');
                }
                // Through the recording emitter, over exactly this comment's span, so the
                // print-once ledger sees it printed here.
                self.push_comment_blocks_in_range(
                    result,
                    (from + i) as u32,
                    (from + end) as u32,
                    "",
                );
                pending_space = true;
                last = Last::Comment;
                i = end;
                continue;
            }
            let Some(c) = gap[i..].chars().next() else {
                break;
            };
            i += c.len_utf8();
            if crate::whitespace::is_boundary_only_whitespace(c) {
                if pending_space {
                    result.push(' ');
                }
                result.push(c);
                last = Last::Run;
            } else if crate::whitespace::is_boundary_whitespace(c) {
                pending_space = true;
                continue;
            } else {
                // The case flag's letters: one space ahead of the flag — the authored one
                // beside a run, the normalized one otherwise — and none between its letters
                // or against a run it is glued to.
                let spaced = match last {
                    Last::Flag => false,
                    Last::Run => pending_space,
                    Last::Start | Last::Comment => true,
                };
                if spaced {
                    result.push(' ');
                }
                result.push(c);
                last = Last::Flag;
            }
            pending_space = false;
        }
    }

    /// [`Self::push_attribute_tail`] for a tail that holds a boundary member: every item —
    /// the members, the comments and the case flag's letters — where the author put it, one
    /// space where they put ASCII whitespace and none where they glued it (the spelling of
    /// `Printer::spell_gap_items`, with the flag as one more item). A tail with a member in
    /// it is where the bytes are the claim — to css-syntax-3 the member is identifier content
    /// and a comment tokenizes to nothing, so a comment padded off a member it was glued to,
    /// or a flag moved off one, changes what the member is glued to — so no item is spaced
    /// that the author did not space.
    fn push_attribute_tail_in_place(&self, result: &mut String, from: u32, close: u32) {
        let (from, close) = (from as usize, close as usize);
        let gap = &self.source[from..close];
        let bytes = gap.as_bytes();
        let mut pending_space = false;
        let mut i = 0;
        while i < gap.len() {
            if crate::comments::is_comment_start(bytes, i) {
                let end = crate::comments::comment_end(bytes, i);
                if pending_space {
                    result.push(' ');
                }
                // Through the recording emitter, over exactly this comment's span, so the
                // print-once ledger sees it printed here.
                self.push_comment_blocks_in_range(
                    result,
                    (from + i) as u32,
                    (from + end) as u32,
                    "",
                );
                pending_space = false;
                i = end;
                continue;
            }
            let Some(c) = gap[i..].chars().next() else {
                break;
            };
            i += c.len_utf8();
            if crate::whitespace::is_boundary_whitespace(c)
                && !crate::whitespace::is_boundary_only_whitespace(c)
            {
                pending_space = true;
                continue;
            }
            if pending_space {
                result.push(' ');
            }
            result.push(c);
            pending_space = false;
        }
    }

    /// One interior gap of an attribute-selector rebuild: its boundary run and its comments.
    ///
    /// Every gap of this rebuild is also a juncture the parser stepped a boundary run at, and
    /// the rebuild is the only thing that can carry one — so the run goes in whether or not
    /// the gap holds a comment. A gap holding a member is spelled whole, its comments in place
    /// among the members (`Printer::spell_gap_items`); a member-free one keeps the comment
    /// spelling (`pad_before` / `pad_after`), so a `Glued` gap (a `<wq-name>`'s or an
    /// `<attr-matcher>`'s, where whitespace is forbidden) still emits nothing of its own
    /// around it.
    ///
    /// ⚠️ The run goes in through `push_boundary_ws_after_name`, never flush: the gap this
    /// rebuild resumes at most often is the one right behind the attribute NAME, where a
    /// flush run glues (`[a/* c */ <NBSP>=b]` came back with the name `a<NBSP>`). A `Glued`
    /// gap cannot reach that branch — the parser reads those with `register_and_skip_comments`
    /// and rejects a `<whitespace-token>` outright, so the only trivia in one is comments and
    /// the run is empty — which is what keeps the space out of the two junctures selectors-4
    /// forbids it at.
    ///
    /// Not for the selector's tail, whose runs keep their authored place among the flag and
    /// the comments — see [`Self::push_attribute_tail`].
    fn push_attribute_gap(&self, result: &mut String, from: u32, to: u32, gap: AttributeGap) {
        if from >= to {
            return;
        }
        if gap != AttributeGap::Glued && self.gap_holds_member(from, to) {
            let lead = if gap.pad_before() {
                Edge::Presence
            } else {
                Edge::Flush
            };
            let kept = self.spell_gap_items(from, to, lead, Edge::Presence);
            if kept.starts_with([' ', '/']) {
                // Already apart from the name: its own space, or a comment, which no name
                // can take in.
                result.push_str(&kept);
            } else {
                self.push_boundary_ws_after_name(result, &kept);
            }
            return;
        }
        self.push_attribute_gap_comments(result, from, to, gap);
    }

    /// Append the comments in `[from, to)` to an attribute selector being rebuilt,
    /// spaced per `gap`.
    fn push_attribute_gap_comments(
        &self,
        result: &mut String,
        from: u32,
        to: u32,
        gap: AttributeGap,
    ) {
        if from >= to || !self.has_comments_to_emit_between(from, to) {
            return;
        }
        if gap.pad_before() {
            result.push(' ');
        }
        self.push_comment_blocks_in_range(result, from, to, gap.separator());
        if gap.pad_after() {
            result.push(' ');
        }
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
                // the keyframes path. The trim is CSS-whitespace-only: a name glued to a
                // non-ASCII space (`FROM<NBSP>`) is not the keyword, and a Unicode trim
                // would both lowercase it and drop the space.
                if self.in_keyframes {
                    let text = crate::escapes::trim_css(span.extract(self.source));
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
                flags: _,
                span,
            } => d.text_pooled(&self.build_attribute_selector_text(
                *namespace_span,
                *name_span,
                *matcher,
                *value_span,
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
                // split it off, normalize the An+B, and re-emit the ` of ` with a single
                // trailing space so the following sibling selector (`S`) stays separated
                // in the glued compound — or with the boundary run the fold carried in its
                // place (`of_keyword_text`; `2n of<NBSP>.x` keeps its `<NBSP>`, which is
                // the separator the author wrote and the one prettier keeps).
                let raw = span.extract(self.source);
                match split_nth_of(raw) {
                    Some(split) => {
                        let mut w = d.pool_writer();
                        w.push_str(&Self::normalize_an_plus_b(split.anb));
                        let after = if split.after_of.is_empty() {
                            " "
                        } else {
                            &split.after_of
                        };
                        w.push_str(&of_keyword_text(&split.before_of, after));
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
                let item = crate::escapes::trim_css(raw);
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
                match &name {
                    // A borrowed name is the document's own bytes from `span.start` (the
                    // trim only shortens the end), so it is emitted as a `source_span` —
                    // no pool copy, the width read off the host's word — the same form
                    // `span_leaf_doc` gives a type / class / id leaf. Only a case-folded
                    // name (an owned `String`) is pooled.
                    Cow::Borrowed(_) => d.source_span(
                        Span {
                            start: span.start,
                            end: span.start + text.len() as u32,
                        },
                        self.source,
                    ),
                    Cow::Owned(_) => d.text_pooled(text),
                }
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
                self.wrap_pseudo_args(self.build_paren_selector_list_inner(selectors, *span))
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
                let normalized = self.normalize_an_plus_b_before_gap(value, value_span.end);
                // Comments in the gaps around the An+B text are not part of `value`;
                // interleave them like the SelectorList arm above — unless the gap holds a
                // boundary member, which `normalize_an_plus_b` would otherwise drop with the
                // rest of the gap (`:nth-child(<NBSP>2n)`): then the gap's claim prints it
                // whole, comments in place (`spell_gap_items`).
                let leading = self.comment_blocks_unless_claimed(span.start, value_span.start);
                let kept_leading =
                    self.spell_gap_items(span.start, value_span.start, Edge::Flush, Edge::Presence);
                let normalized = if kept_leading.is_empty() {
                    normalized
                } else {
                    Cow::Owned(kept_leading + normalized.as_ref())
                };
                match of_selector {
                    None => {
                        let trailing = self.comment_blocks_unless_claimed(value_span.end, span.end);
                        // …nor the run the `)` gap skipped (`:nth-child(2n<NBSP>)`), flush
                        // against the `)`. `span.end` IS the `)` here (the Nth span ends
                        // before it, unlike the other arms' — see `wrap_args_gap_comments`),
                        // so the gap needs no `closer_pos`. No separator ahead of it: an
                        // An+B re-parses through the byte scanner, which steps the run as
                        // the terminator's gap, so nothing glues (`list_run_edge`).
                        let kept_trailing = self.spell_gap_items(
                            value_span.end,
                            span.end,
                            Edge::Presence,
                            Edge::Flush,
                        );
                        let text = if kept_trailing.is_empty() {
                            normalized
                        } else {
                            Cow::Owned(normalized.into_owned() + &kept_trailing)
                        };
                        let inner = self.wrap_inner_with_comments(
                            d.text_pooled(&text),
                            &leading,
                            &trailing,
                        );
                        self.paren_wrap(inner)
                    }
                    Some(selectors) => {
                        // The keyword first: where the of-gap holds a member it claims `S`'s
                        // lead gap, which must happen before `S` is built.
                        let NthOf {
                            head,
                            of,
                            comments_after,
                        } = self.nth_of_keyword(normalized, *value_span, selectors);
                        // The of-gap comments the keyword's claims left (`of /* c */ .a`) lead
                        // the selector list.
                        let list = self.wrap_inner_with_comments(
                            self.build_nested_selector_list_doc(selectors),
                            &comments_after,
                            "",
                        );
                        let trailing =
                            self.comment_blocks_unless_claimed(selectors.span.end, span.end);
                        // The run the `)` gap skipped after `S` (`of .b <NBSP>)`), flush against
                        // the `)` behind the separator its last name needs — the same claim the
                        // SelectorList arm makes; `S` is a nested list with no other emitter.
                        let tail = self.spell_gap_items(
                            selectors.span.end,
                            span.end,
                            self.list_run_edge(selectors),
                            Edge::Flush,
                        );
                        let inner = if tail.is_empty() {
                            d.concat(&[d.text_pooled(&head), d.text_pooled(&of), list])
                        } else {
                            d.concat(&[
                                d.text_pooled(&head),
                                d.text_pooled(&of),
                                list,
                                d.text_pooled(&tail),
                            ])
                        };
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
                // The runs the argument-list start and the `)` gap skipped — `::part` rebuilds
                // its names from spans, so nothing else carries them. Each is claimed whole,
                // its comments in place, and the comment wrapper below then leaves that side
                // alone. The tail's edge is the last name's (`::part(a <NBSP>)` re-parsed with
                // the part `a<NBSP>`); the lead needs none, its left neighbour being the `(`.
                let lead =
                    self.spell_gap_items(span.start, run_span.start, Edge::Flush, Edge::Presence);
                let tail = self.spell_gap_before_closer(
                    run_span.end,
                    *span,
                    self.name_run_edge(run_span.end),
                );
                let inner = self.wrap_args_gap_comments(
                    run,
                    *span,
                    run_span,
                    (!lead.is_empty(), !tail.is_empty()),
                );
                let inner = if lead.is_empty() && tail.is_empty() {
                    inner
                } else {
                    d.concat(&[d.text_pooled(&lead), inner, d.text_pooled(&tail)])
                };
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
    ///
    /// Each side's comments are left out where a boundary claim has already printed that gap
    /// whole, in place (`claimed` is `(lead, tail)`).
    fn wrap_args_gap_comments(
        &self,
        inner: DocId,
        args_span: Span,
        content_span: Span,
        claimed: (bool, bool),
    ) -> DocId {
        // Both gaps sit inside the argument parens, so comment-free parens mean both come
        // back empty and `wrap_inner_with_comments` hands `inner` straight back. One probe
        // replaces the two range-collects on the common path — this fires on every
        // `:not()` / `:where()` / `:is()`, which are dense in real stylesheets.
        if !self.has_comments_to_emit_between(args_span.start, args_span.end) {
            return inner;
        }
        let leading = if claimed.0 {
            String::new()
        } else {
            self.comment_blocks_in_range(args_span.start, content_span.start)
        };
        let trailing = if claimed.1 {
            String::new()
        } else {
            self.comment_blocks_in_range(content_span.end, closer_pos(args_span))
        };
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
        // ⚠️ The fast path must ask about the boundary run too, not comments alone: every
        // inter-ident gap is an `allow_comment_or_whitespace` juncture the parser steps a run
        // at (`::part(a <NBSP>b)`), and `join(" ")` regenerates the gap from nothing. This
        // rebuild is the only thing that can carry those characters, so a comment-keyed gate
        // in front of it deletes them — which it did, on input canonical accepts.
        if idents.len() < 2
            || (!self.has_comments_to_emit_between(run_span.start, run_span.end)
                && !self.gap_may_hold_boundary_ws(run_span.start, run_span.end))
        {
            return d.text_pooled(&idents.join(" "));
        }
        let mut parts = DocBuf::new();
        for (i, ident) in idents.iter().enumerate() {
            if i > 0 {
                parts.push(d.text(" "));
                // A gap holding a member is printed whole, its comments in place among the
                // members, with the author's separation kept against the name that follows;
                // a member-free gap keeps the comment spelling. Never parked against the
                // PREVIOUS name, where it would glue and read as one longer identifier.
                let (from, to) = (ident_spans[i - 1].end, ident_spans[i].start);
                let kept = self.spell_gap_items(from, to, Edge::Flush, Edge::Presence);
                if kept.is_empty() {
                    let gap = self.comment_blocks_in_range(from, to);
                    if !gap.is_empty() {
                        parts.push(d.text_pooled(&gap));
                        parts.push(d.text(" "));
                    }
                } else {
                    parts.push(d.text_pooled(&kept));
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

    /// [`Self::normalize_an_plus_b`] for a term whose text ends where the gap at `gap_start`
    /// begins — reading the gap's first character too when it is a boundary member glued to
    /// a comment the term's text ends on (`2n/* c */<NBSP>)`).
    ///
    /// The normalizer spaces a comment off its neighbours unless one of them is a member it is
    /// glued to, and a member just past the term's text is one it cannot otherwise see: the
    /// An+B reader ends the text at the comment and leaves the member to the gap's claim. So
    /// the member is shown to the normalizer and taken back off its answer.
    fn normalize_an_plus_b_before_gap<'v>(&self, value: &'v str, gap_start: u32) -> Cow<'v, str> {
        let glued_member = if self.holds_boundary_ws && value.ends_with("*/") {
            self.source
                .get(gap_start as usize..)
                .and_then(|rest| rest.chars().next())
                .filter(|c| crate::whitespace::is_boundary_only_whitespace(*c))
        } else {
            None
        };
        let Some(member) = glued_member else {
            return Self::normalize_an_plus_b(value);
        };
        let mut read = String::with_capacity(value.len() + member.len_utf8());
        read.push_str(value);
        read.push(member);
        let mut normalized = Self::normalize_an_plus_b(&read).into_owned();
        normalized.truncate(normalized.len() - member.len_utf8());
        Cow::Owned(normalized)
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
    ///
    /// ⚠️ The whitespace it respaces is the ASCII half of the An+B gap's class
    /// ([`is_ascii_boundary_whitespace`]) — what it regenerates as the one space around an
    /// operator. A `<NBSP>` or `<ZWNBSP>` in the gap is a member the scanner stepped
    /// (`2n<NBSP>+<NBSP>1` is one term to `parseCss`) but not one this walk may drop:
    /// prettier keeps it in place (`2n<NBSP> + <NBSP>1`), and `str::trim` would have eaten
    /// it with the spaces. `<VT>` is in the ASCII half, so it collapses like a space (the
    /// uniform rule the `combinator_control_whitespace` divergence states).
    fn normalize_an_plus_b(value: &str) -> Cow<'_, str> {
        // Lowercase the `n` variable when it carries a numeric coefficient
        // (`2N`→`2n`), matching prettier — a bare `N`/`-N` keeps its case (prettier
        // preserves it), as do the `even`/`odd` keywords (their `n` isn't
        // digit-preceded). Applied before the early return so `2N` (no operator)
        // is cased too.
        let cased = lowercase_an_plus_b_n(value.trim_matches(is_ascii_boundary_whitespace));

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
            // off from whatever sits on either side of it — except where the author glued it
            // to a boundary member (`2n<NBSP>/* c */`, `2n/* c */<NBSP>`): a comment
            // tokenizes to nothing, so spacing it off either side would split what the member
            // is glued to (the in-place rule of `printer/boundary_ws.rs`).
            if let Some(end) = crate::comments::leading_comment_end(rest) {
                let tail = &rest[end..];
                let after_member = result
                    .chars()
                    .next_back()
                    .is_some_and(crate::whitespace::is_boundary_only_whitespace);
                let before_member = tail
                    .chars()
                    .next()
                    .is_some_and(crate::whitespace::is_boundary_only_whitespace);
                // A comment glued to a member on either side is part of that member's glue
                // chain: both of its sides keep the author's spacing.
                let in_chain = after_member || before_member;
                if prev_exists && !result.ends_with(' ') && !in_chain {
                    result.push(' ');
                }
                result.push_str(&rest[..end]);
                rest = tail.trim_start_matches(is_ascii_boundary_whitespace);
                let spaced_after = rest.len() != tail.len();
                if !rest.is_empty() && (!in_chain || spaced_after) {
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
                let trimmed_len = result.trim_end_matches(is_ascii_boundary_whitespace).len();
                if trimmed_len != 0 {
                    result.truncate(trimmed_len);
                    result.push(' ');
                    result.push(ch);

                    // Skip any spaces after the operator and add a single space
                    // only when more content follows.
                    rest = rest.trim_start_matches(is_ascii_boundary_whitespace);
                    if !rest.is_empty() {
                        result.push(' ');
                    }
                    prev_exists = true;
                    continue;
                }
            }

            // Any other ASCII whitespace stretch sits beside a boundary member the scanner
            // folded into the term (`2n<NBSP><TAB><NBSP> + 1`, `+ <NBSP>⏎1`): one space, the
            // spelling every boundary run takes (`printer/boundary_ws.rs`). A stretch ahead
            // of an operator is truncated by the operator arm above, which regenerates its own.
            if is_ascii_boundary_whitespace(ch) {
                rest = rest.trim_start_matches(is_ascii_boundary_whitespace);
                if !rest.is_empty() && !result.ends_with(' ') {
                    result.push(' ');
                }
                continue;
            }

            result.push(ch);
            prev_exists = true;
        }

        Cow::Owned(result)
    }
}

/// Where a selector list's first selector begins — the anchor whose lead gap a container
/// claims ([`Printer::claim_lead_gap`]).
pub(super) fn first_anchor(list: &internal::SelectorList<'_>) -> u32 {
    list.selectors
        .first()
        .map_or(list.span.start, |complex| complex.span.start)
}

/// The `of` keyword of an `:nth-*(An+B of S)` argument, as [`Printer::nth_of_keyword`] prints
/// it: the An+B head with any claim before the keyword appended, the keyword's own text, and
/// the gap comments no claim printed, which lead `S`.
struct NthOf<'v> {
    head: Cow<'v, str>,
    of: Cow<'static, str>,
    comments_after: String,
}

/// An `An+B of S` term's folded value, split at its `of`.
struct NthOfSplit<'a> {
    /// The An+B text, its trailing whitespace run removed.
    anb: &'a str,
    /// The boundary members (non-ASCII JS `\s`) of the `\s+` run ahead of `of`.
    before_of: String,
    /// The boundary members of the `\s+` run after `of`.
    after_of: String,
}

/// Split an `An+B of S` term's folded value (`"2n of "`, `"-n + 3 of "`) at its `of`, or
/// `None` for a bare An+B (`"2n"`, `"odd"`). The parser (`match_nth_value`) only ever
/// produces `"<An+B>\s+of\s+"` or `"<An+B>"`, so the `of` — when present — is a trailing
/// whole word preceded by whitespace; the whitespace check rejects a hypothetical An+B
/// ending in the letters `of`.
///
/// Both `\s` are the regex's JS `\s`, so each run is scanned over that class
/// ([`tsv_lang::is_js_whitespace`]) — a `<NBSP>` on either side of the keyword is part of
/// the run, not of the An+B text or of `S`. The runs' ASCII halves are the printer's to
/// regenerate; their boundary members ride along in the split so the emitter can put them
/// back where the author wrote them.
fn split_nth_of(value: &str) -> Option<NthOfSplit<'_>> {
    let before_after = value.trim_end_matches(tsv_lang::is_js_whitespace);
    let after_of = &value[before_after.len()..];
    let anb_and_run = before_after.strip_suffix("of")?;
    if !anb_and_run.ends_with(tsv_lang::is_js_whitespace) {
        return None;
    }
    let anb = anb_and_run.trim_end_matches(tsv_lang::is_js_whitespace);
    Some(NthOfSplit {
        anb,
        before_of: boundary_members(&anb_and_run[anb.len()..]),
        after_of: boundary_members(after_of),
    })
}

/// The boundary members (`is_boundary_only_whitespace`) of a whitespace run, in order —
/// the half of it the printer preserves; the ASCII half is regenerated. Empty, without
/// allocating, for the all-ASCII run every real stylesheet writes.
fn boundary_members(run: &str) -> String {
    if run.is_ascii() {
        return String::new();
    }
    run.chars()
        .filter(|c| crate::whitespace::is_boundary_only_whitespace(*c))
        .collect()
}

/// The ` of ` between an An+B term and its `S`, carrying the boundary runs the parser
/// skipped on either side of the keyword.
///
/// `before` sits flush after the term, ahead of the regenerated space (`2n + 1<NBSP> of`);
/// `after` is EXACTLY what follows the keyword — the regenerated space, or a run in its
/// place (`of<NBSP>.x`: the run IS the separator, as the author, canonical and prettier all
/// spell it, and an ASCII space beside it would be a second one), or nothing at all where
/// `S`'s own leading claim prints that run. The caller decides; an empty `after` here means
/// empty. The plain keyword is borrowed, so the common case allocates nothing.
fn of_keyword_text(before: &str, after: &str) -> Cow<'static, str> {
    if before.is_empty() && after == " " {
        return Cow::Borrowed(" of ");
    }
    let mut out = String::with_capacity(before.len() + 3 + after.len());
    out.push_str(before);
    out.push_str(" of");
    out.push_str(after);
    Cow::Owned(out)
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
/// off its neighbours — glued only to `[` itself, the answer `:is()` and `::part()` already
/// give inside their parens (the gap against `]` belongs to the tail's own emitter).
#[derive(Clone, Copy, PartialEq, Eq)]
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
    /// A gap between two interior tokens: padded on both sides. The gap just inside `]` is
    /// not one of these — the selector's tail has its own emitter (`push_attribute_tail`).
    const INTERIOR: Self = Self::Spaced {
        before: true,
        after: true,
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
