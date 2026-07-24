// Format-ignore directive honoring for type-member lists (union / intersection /
// tuple / type-parameter / type-argument members) and single-child type heads (the
// type after an annotation's `:`, an alias's `=`, a type parameter's `extends`/`=`,
// a named tuple member's `label:`, a mapped type's `]:` value and key-side gaps).
//
// One seam that knows what a directive is and where it sits, so the printers
// only ever ask "freeze this member / this child / this whole node?" and
// never re-derive directive recognition. Recognition itself stays centralized in
// `tsv_lang::is_format_ignore_directive`; this module owns the *placement*
// classification (the out-of-span leading run vs. an in-span inter-member gap) and
// the paren-transparent freeze emitter. A single-child head is composite-transparent:
// a Union/Intersection child declines (`single_child_frozen`) and freezes via its own
// leading-run walk, so the member rules and the head rules can never both claim one
// directive.
//
// **Rule A — list-item freeze** (the single symmetric rule, every member list
// alike), with a **total, placement-only classification** per directive,
// exception-free: a directive ALONE ON ITS LINE (nothing but whitespace before or
// after it on its physical line) in a member list's leading OR inter-item gap
// freezes the *following* member — the first member and every later member
// identically. ANYTHING ELSE — a directive sharing its line with anything (trailing
// a member, a separator, an opening delimiter, or a declaration head, or glued
// before a value) — is permanently inert: an ordinary comment. This is the same
// semantics every honored site carries (a directive alone on its line between `{`
// and the first class member freezes that member, not the body). See
// docs/conformance_prettier.md §Format-ignore directive for the behavior contract.
//
// **Gating.** Every entry is gated on the document-level `has_format_ignore` flag, so
// a document with no directive (≈ all of them) pays nothing. The leading-run walk is a
// pure backward byte scan bounded by the first non-run byte — no allocation. The
// in-span gap check reuses the container's existing double-gate (its comment window is
// already open at the call site).
//
// **Comment-model discipline** (docs/comments.md). The directive itself sits OUTSIDE
// every frozen span here — it stays in the enclosing gap / leading run and keeps being
// emitted by the existing emitters (no new trailing-comment emitter is minted). The
// comments *inside* a frozen span ride out in the verbatim slice and are recorded by
// `raw_source_range`'s `record_verbatim_range`, so the print-once ledger counts them.
// The paren-transparent freeze slices the INNER node span only when the paren shell is
// comment-free (dropping a redundant paren losslessly); a shell that holds a comment
// (`(/* c */ a1)`) is frozen WHOLE-span instead, keeping the redundant paren so the
// comment survives — comment preservation outranks redundant-paren removal under a freeze
// (`union_prettier_ignore_paren_shell_comment` exercises it, `comments:audit` guards it).

use super::Printer;
use super::has_newline_after_position;
use super::has_newline_before_position;
use super::unwrap_parenthesized;
use crate::ast::internal::{Comment, TSType, TSUnionType};
use tsv_lang::doc::arena::DocId;
use tsv_lang::{Span, comments_in_source_range, is_format_ignore_directive};

/// The freeze implied by a format-ignore directive alone on its line in a union's or
/// intersection's leading run (the out-of-span region before the node's `span.start`):
/// the FIRST member freezes (Rule A). `multiline` is set when the frozen slice spans
/// lines, forcing the broken layout (a verbatim span is `will_break`-opaque, so the
/// forcing is explicit).
#[derive(Clone, Copy)]
pub(in crate::printer) struct LeadingRunFreeze {
    pub(in crate::printer) multiline: bool,
}

impl LeadingRunFreeze {
    /// The `(freeze_first, multiline)` flag pair of a resolved leading-run freeze.
    pub(in crate::printer) fn first_member_flags(freeze: Option<Self>) -> (bool, bool) {
        freeze.map_or((false, false), |f| (true, f.multiline))
    }
}

/// Whether `child` is a freeze TARGET — the composite-transparency arm of
/// [`Printer::single_child_frozen`], asked alone by a head that has already resolved
/// the gap verdict as its own ROUTING question. The two are different questions (where
/// the directive sits vs. what freezes), and splitting them keeps a routed head from
/// re-running the gap scan it just ran.
///
/// A Union/Intersection (paren-unwrapped) declines, so the member rules keep applying
/// via the composite's own leading-run walk — the first member freezes (Rule A) — and
/// the head rules and the member rules can never both claim one directive.
pub(in crate::printer) fn is_freeze_target(child: &TSType<'_>) -> bool {
    !matches!(
        unwrap_parenthesized(child),
        TSType::Union(_) | TSType::Intersection(_)
    )
}

impl<'a> Printer<'a> {
    //
    // Directive recognition and routing — is there a freeze, and what does it target
    //

    /// The format-ignore directive comment in the adjacent leading run ending at
    /// `anchor` (a node's `span.start`), if any. The run is the maximal stretch of
    /// whitespace, transparent leading punctuation (`|`, `&`, `(`), and comment spans
    /// immediately before `anchor`; the backward walk stops at the first byte outside
    /// that set (`=`, `:`, `<`, `[`, …), which bounds the run without a `prev_end`. The
    /// directive nearest the anchor wins — the placement floor then decides honoring.
    ///
    /// A directive that sits above the whole statement instead belongs to
    /// `Program.body` (already honored at the statement site) and never reaches this
    /// run: the walk halts at the alias's `=` (or the annotation's `:`), so the two
    /// claims can't overlap. The in-span comment gate can't see this directive — it is
    /// physically before the node — so the caller gates on `has_format_ignore` alone.
    fn leading_run_directive(&self, anchor: u32) -> Option<&'a Comment> {
        let bytes = self.source.as_bytes();
        let mut pos = anchor as usize;
        loop {
            while pos > 0
                && matches!(
                    bytes[pos - 1],
                    b' ' | b'\t' | b'\n' | b'\r' | b'|' | b'&' | b'('
                )
            {
                pos -= 1;
            }
            // The run continues only across an immediately-preceding comment span;
            // anything else (an operator, a bracket, source text) bounds it.
            let c = self.comment_ending_at(pos as u32)?;
            if is_format_ignore_directive(c.content(self.source)) {
                return Some(c);
            }
            pos = c.span.start as usize;
        }
    }

    /// The comment whose span ends exactly at `pos`, if any. Comments are sorted by
    /// start and never overlap, so `end` is monotonic and a binary search locates it.
    /// **In-source axis** (an ownership-blind search over the raw table), like every
    /// directive-recognition seam in this module — see `member_gap_frozen`'s note for
    /// why the axes coincide for directives.
    fn comment_ending_at(&self, pos: u32) -> Option<&'a Comment> {
        let idx = self.comments.partition_point(|c| c.span.end < pos);
        self.comments.get(idx).filter(|c| c.span.end == pos)
    }

    /// Rule A leading-run freeze plan for a union or intersection whose `span.start` is
    /// `node_start` and whose first member's paren-stripped span is `first_inner`.
    /// Gated on `has_format_ignore`.
    ///
    /// A directive alone on its line freezes the first member (with `multiline` set
    /// when the frozen first slice spans lines, so the caller forces the broken
    /// layout); any other placement is inert.
    pub(in crate::printer) fn leading_run_freeze(
        &self,
        node_start: u32,
        first_inner: Option<Span>,
    ) -> Option<LeadingRunFreeze> {
        if !self.has_format_ignore {
            return None;
        }
        let directive = self.leading_run_directive(node_start)?;
        // The alone-on-line floor, the same one `member_gap_frozen` applies. Without
        // it, the walk's `|`/`&`/`(` transparency lets a NESTED composite member reach
        // a directive the enclosing list deliberately rejected — one TRAILING a
        // previous member or a declaration head (`type T = // prettier-ignore⏎ …`),
        // or glued before the value — and resurrect it as a first-member freeze.
        // Non-own-line placements are permanently inert (the module header).
        if !self.directive_alone_on_line(directive) {
            return None;
        }
        // A multi-line frozen slice makes the caller force the broken layout: a
        // `verbatim_source_span` is `will_break`-opaque, so the trigger is asked
        // here instead of propagating from the slice. `is_same_line` reads
        // `comment_line_breaks`, which stays populated in every printer mode —
        // the right table for a verbatim slice, whose emitted bytes physically
        // contain the newlines regardless of mode.
        let multiline = first_inner.is_some_and(|s| !self.is_same_line(s.start, s.end));
        Some(LeadingRunFreeze { multiline })
    }

    /// [`Self::leading_run_freeze`] for a union — resolves the first member's inner span
    /// so the caller doesn't repeat the paren-unwrap.
    pub(in crate::printer) fn union_leading_run_freeze(
        &self,
        union: &TSUnionType<'_>,
    ) -> Option<LeadingRunFreeze> {
        let first_inner = union.types.first().map(|t| unwrap_parenthesized(t).span());
        self.leading_run_freeze(union.span.start, first_inner)
    }

    /// True when the gap `[prev_end, member_start)` before a union / intersection member
    /// carries a format-ignore directive **alone on its line** — the one placement that
    /// freezes. Any other placement — TRAILING the previous member, the separator
    /// (`{ a: 1 } & // prettier-ignore`), a declaration head, or glued before the member
    /// (`a | /* prettier-ignore */ b`) — is inert (the wrong-node-misbind floor; the
    /// `trailing_inert` fixtures are its regression pins, the `glued_inert` fixtures pin
    /// the glued side).
    ///
    /// The test keys on the directive's own line, NOT on `is_same_line` against
    /// `prev_end`: a blank line injected between `prev_end` and a trailing directive would
    /// move the directive off `prev_end`'s line yet leave it trailing the separator, and
    /// keying on `prev_end` would flip the freeze on and off across that blank (a
    /// non-idempotency `blank_audit` catches). `prev_end` still bounds the comment window.
    ///
    /// Gated on `has_format_ignore`; the caller has already opened its comment window,
    /// so this only runs inside a directive-bearing document.
    ///
    /// **In-source axis** (`comments_in_source_range`) — the one deliberate axis every
    /// directive-recognition seam in this module uses (`leading_run_directive` walks
    /// physical comment spans; `frozen_paren_shell_has_comment` counts physical presence).
    /// Directive recognition is a physical-presence question; a directive is never owned
    /// (`owned` ⇒ a bundler annotation or JSDoc cast, never a `format-ignore` directive),
    /// so the to-emit and in-source axes coincide, but naming the in-source one keeps the
    /// module's axis choice single and deliberate (one question, one predicate).
    /// The same fact also answers the emission-routing question at heads whose default
    /// line-comment layout trails the first comment after the head (the annotation `:`
    /// via `build_continuation_indent`): an honored directive must stay own-line, since
    /// a head-trailing placement is inert and the relocated form would lose the freeze
    /// on the second pass. Asked for composite children too there — the routing is
    /// about the directive's own placement, not the freeze target.
    pub(in crate::printer) fn member_gap_frozen(&self, prev_end: u32, member_start: u32) -> bool {
        self.has_format_ignore
            && comments_in_source_range(self.comments, prev_end, member_start).any(|c| {
                is_format_ignore_directive(c.content(self.source))
                    && self.directive_alone_on_line(c)
            })
    }

    /// The placement floor: whether comment `c` is the only thing on its physical line
    /// (whitespace aside) — the sole placement a directive freezes from. A file
    /// boundary counts as a line boundary, so a directive at byte 0 or at EOF still
    /// qualifies. A line comment trivially satisfies the after side (it consumes to
    /// EOL); only a block spelling can share its line with what follows.
    pub(in crate::printer) fn directive_alone_on_line(&self, c: &Comment) -> bool {
        (c.span.start == 0 || has_newline_before_position(self.source, c.span.start))
            && (c.span.end as usize == self.source.len()
                || has_newline_after_position(self.source, c.span.end))
    }

    /// [`Self::member_gap_frozen`] for a mapped type's two key-side gaps, anchored per
    /// the delimited-list convention (the gap opens just past the `{` / `[`): the
    /// SIGNATURE gap (`{`→`[`), where an alone-on-line directive freezes the whole
    /// `[K in ...]: V` clause (the mapped type's sole-member analog of Rule A), and the
    /// BINDING gap (`[`→key), where a directive freezes just the `K in ...` binding —
    /// freezing the whole node there would freeze the `[` that *precedes* the
    /// directive, so prettier's whole-node redirect is deliberately not copied (the
    /// `mapped_prettier_ignore_key` divergence).
    pub(in crate::printer) fn mapped_gap_frozen(&self, gap_start: u32, target_start: u32) -> bool {
        self.member_gap_frozen(gap_start, target_start)
    }

    /// [`Self::member_gap_frozen`] for a head position's SINGLE child — the type after
    /// an annotation's `:`, an alias's `=`, a type parameter's `extends`/`=`, a named
    /// tuple member's `label:`, or a mapped type's `]:`. An alone-on-line directive
    /// in the head→child gap `[gap_start, child.span().start)` freezes the child whole.
    ///
    /// **Composite-transparent**: a Union/Intersection child (paren-unwrapped) declines,
    /// so the member rules keep applying via the composite's own leading-run walk —
    /// the first member freezes (Rule A).
    ///
    /// The window ends at the child's OWN span start, never a paren-stripped inner
    /// start: an in-shell directive (`extends (// format-ignore⏎ T)`) never triggers
    /// THIS seam, so a frozen whole-shell slice can never double-print a comment an
    /// enclosing gap emitter also sees. The heads that honor an in-shell directive do
    /// so via [`Self::paren_interior_routed_inner`] — the shell's own interior seam,
    /// which freezes the paren-stripped INNER slice only and leaves every shell-gap
    /// comment to the head's (window-widened) emitters, so the single-print invariant
    /// holds there too.
    pub(in crate::printer) fn single_child_frozen(
        &self,
        gap_start: u32,
        child: &TSType<'_>,
    ) -> bool {
        // The document-level bool first: several head sites (alias, named-tuple,
        // mapped value) ask unconditionally on hot paths, so a directive-free
        // document must pay exactly this one branch — never the paren-unwrap below
        // (`member_gap_frozen` re-checks the flag, harmlessly, for its other callers).
        if !self.has_format_ignore {
            return false;
        }
        if !is_freeze_target(child) {
            return false;
        }
        self.member_gap_frozen(gap_start, child.span().start)
    }

    /// The strip target for a head whose child is a paren shell whose INTERIOR leading
    /// gap — from just past the outermost `(` to the fully-unwrapped inner's span
    /// start, spanning every nested layer — holds an alone-on-line format-ignore
    /// directive: `Some(inner)`, the fully-unwrapped inner, is the ROUTING answer.
    ///
    /// An in-shell directive is invisible to [`Self::single_child_frozen`] (whose
    /// window deliberately ends at the shell's own span start), so a head that wants to
    /// honor it strips the shell and emits the interior leading run itself (window
    /// widened to the inner's start — the directive keeps its own line), then builds the
    /// inner with [`Self::build_routed_child_doc`]: the freeze slices the INNER only,
    /// never a whole-shell slice, so the shell-gap comments the routing hands to the
    /// head's emitters can't also ride a verbatim slice. Comments in the shell's
    /// TRAILING gaps are the caller's to lift (`with_stripped_paren_trailing`), so the
    /// freeze stays lossless and every shell comment prints exactly once.
    ///
    /// Routing only — whether the inner itself freezes is the separate
    /// composite-transparency question (`is_freeze_target`, answered by
    /// `build_routed_child_doc`): a Union/Intersection inner declines and freezes via
    /// its own leading-run walk, whose `(`-transparency reaches the same in-shell
    /// directive. Gated on `has_format_ignore`.
    pub(in crate::printer) fn paren_interior_routed_inner<'t>(
        &self,
        shell: &'t TSType<'t>,
    ) -> Option<&'t TSType<'t>> {
        if !self.has_format_ignore {
            return None;
        }
        if !matches!(shell, TSType::Parenthesized(_)) {
            return None;
        }
        let inner = unwrap_parenthesized(shell);
        self.member_gap_frozen(shell.span().start + 1, inner.span().start)
            .then_some(inner)
    }

    /// [`Self::member_gap_frozen`] for list item `i`, the single home of the
    /// gap-anchor convention: the FIRST item's gap opens at `container_start` (the
    /// container's span start for a bare list like a union, or just past the opening
    /// delimiter for a delimited one — `<`/`[` — where a leading-run freeze doesn't
    /// apply); a LATER item's gap opens at the previous item's RAW span end — never a
    /// comma- or trailing-comment-advanced cursor — so a directive after the
    /// separator still binds forward while the own-line floor keeps a trailing
    /// directive inert. Every container loop routes through here (or the slice
    /// wrapper [`Self::list_member_frozen`]) rather than picking its own anchors.
    /// Closure-shaped so the item type is family-agnostic (`TSType` members,
    /// `TSTypeParameter` declarations).
    pub(in crate::printer) fn list_item_frozen(
        &self,
        container_start: u32,
        item_span: &impl Fn(usize) -> Span,
        i: usize,
    ) -> bool {
        if i == 0 {
            self.member_gap_frozen(container_start, item_span(0).start)
        } else {
            self.member_gap_frozen(item_span(i - 1).end, item_span(i).start)
        }
    }

    /// [`Self::list_item_frozen`] over a `TSType` slice, plus the union /
    /// intersection `freeze_first` arm (the out-of-span leading-run directive, which
    /// applies only to the first member of an undelimited list).
    pub(in crate::printer) fn list_member_frozen(
        &self,
        container_start: u32,
        types: &[TSType<'_>],
        i: usize,
        freeze_first: bool,
    ) -> bool {
        (i == 0 && freeze_first) || self.list_item_frozen(container_start, &|j| types[j].span(), i)
    }

    //
    // Frozen doc builders — what a resolved freeze emits
    //
    // The must-break rule and its span form come first: every builder below either
    // ends in that rule or hands its multiline fact to a family layout instead.
    //

    /// The single-child heads' must-break, in one place: a frozen slice spanning
    /// `slice` appends a `break_parent`, so the ENCLOSING width-decided groups (a
    /// parameter list, an object type) break cleanly around it instead of gluing flat.
    /// A `verbatim_source_span` is `will_break`-opaque, so without the explicit signal
    /// the container never learns the slice spans lines. The list positions thread the
    /// same fact through their family layouts (`frozen_member_forces_break` /
    /// `frozen_list_member_multiline`); the heads have no family layout, so the signal
    /// rides in the doc.
    fn with_frozen_must_break(&self, doc: DocId, slice: Span) -> DocId {
        let d = self.d();
        if self.is_same_line(slice.start, slice.end) {
            doc
        } else {
            d.concat(&[doc, d.break_parent()])
        }
    }

    /// The frozen verbatim slice for a SPAN-shaped freeze target — a freeze whose
    /// following node is not a `TSType` (the function/constructor return annotation
    /// `=> T`, frozen whole when the directive sits in the `)`→`=>` gap) — with the
    /// single-child heads' must-break. The span analog of
    /// [`Self::build_frozen_single_child_doc`] (no paren-transparency: the span is
    /// taken as-is).
    pub(in crate::printer) fn build_frozen_span_doc(&self, span: Span) -> DocId {
        self.with_frozen_must_break(self.raw_source_range(span.start, span.end), span)
    }

    /// Paren-transparent frozen doc for a union / intersection member — and, through
    /// [`Self::build_frozen_head_doc`], for a single-child head's frozen value too (the
    /// type-parameter constraint/default site passes a `member_parens` that keeps a
    /// conditional value's clarity parens). Precedence parens are kept or dropped per
    /// `member_parens`, and the freeze stays lossless:
    ///
    /// - **paren dropped** (`member_parens(inner)` false — a redundant `(a1)` or a bare
    ///   `a1`) → freeze just the inner slice, so `(a1)` → `‹frozen a1›`;
    /// - **paren kept, already parenthesized** (`(a1&a2)`, `(A | B)`, `(// c⏎ a | b)`) →
    ///   freeze the member's WHOLE span verbatim, parens and any inner comments included.
    ///   Byte-identical to re-synthesizing the paren around the frozen inner when the
    ///   shell is comment-free, but lossless when it holds a comment (slicing the inner
    ///   would drop a comment between `(` and the inner type);
    /// - **paren kept, bare member** (`b1&b2` needing parens as a union member) →
    ///   re-synthesize the parens around the frozen slice.
    ///
    /// Separators (`| ` / ` & `) are parent-owned and emitted by the loop; parent-owned
    /// trailing punctuation stays out of the frozen slice (it is past the member span,
    /// and `raw_source_range` trims trailing whitespace).
    pub(in crate::printer) fn build_frozen_member_doc(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        self.build_frozen_member_doc_at(
            t,
            member_parens,
            self.frozen_member_slice_span(t, member_parens),
        )
    }

    /// [`Self::build_frozen_member_doc`] over an already-resolved slice span, so a
    /// caller that also needs the span (the heads' must-break test) resolves it once.
    fn build_frozen_member_doc_at(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
        slice: Span,
    ) -> DocId {
        let d = self.d();
        let inner = unwrap_parenthesized(t);
        let frozen = self.raw_source_range(slice.start, slice.end);
        // Re-synthesize the parens only for a BARE member that needs them; a
        // source-parenthesized member's slice already covers its own parens.
        if member_parens(inner) && !matches!(t, TSType::Parenthesized(_)) {
            d.concat(&[d.text("("), frozen, d.text(")")])
        } else {
            frozen
        }
    }

    /// The span of the verbatim slice [`Self::build_frozen_member_doc`] emits for `t`:
    ///
    /// - a parenthesized shell holding a comment (`(/* c */ a1)`, `(// c⏎ a1)`) freezes
    ///   the member's WHOLE span — slicing the inner would drop the shell comment
    ///   (overrides the redundant-paren drop below);
    /// - a redundant paren (`member_parens(inner)` false) is dropped — the slice is the
    ///   paren-stripped inner;
    /// - a kept, source-parenthesized member freezes whole-span (parens included); a
    ///   kept, bare member's slice is its own span (the caller re-synthesizes parens).
    fn frozen_member_slice_span(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> Span {
        let inner = unwrap_parenthesized(t);
        if self.frozen_paren_shell_has_comment(t) || member_parens(inner) {
            t.span()
        } else {
            inner.span()
        }
    }

    /// Whether the frozen slice for member `t` spans lines — the member-freeze
    /// must-break trigger: a `verbatim_source_span` is `will_break`-opaque, so a caller
    /// whose layout is width-decided forces the family broken explicitly when a frozen
    /// member is multi-line (the leading-run analog is `FirstMember.multiline`).
    fn frozen_member_multiline(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> bool {
        let slice = self.frozen_member_slice_span(t, member_parens);
        !self.is_same_line(slice.start, slice.end)
    }

    /// The one spelling of the Rule A must-break OR-tracking at the width-decided call
    /// sites: a `frozen` member whose slice spans lines forces the family's broken
    /// layout.
    pub(in crate::printer) fn frozen_member_forces_break(
        &self,
        frozen: bool,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> bool {
        frozen && self.frozen_member_multiline(t, member_parens)
    }

    /// [`Self::build_frozen_member_doc`] for a LIST position whose element never needs
    /// precedence parens (tuple elements, type arguments): a source paren there is
    /// always redundant, so it drops under the freeze unless its shell holds a comment.
    /// The single-child heads take [`Self::build_frozen_single_child_doc`], the same
    /// slice plus their must-break.
    pub(in crate::printer) fn build_frozen_list_member_doc(&self, t: &TSType<'_>) -> DocId {
        self.build_frozen_member_doc(t, |_| false)
    }

    /// [`Self::frozen_member_multiline`] for the paren-free list positions of
    /// [`Self::build_frozen_list_member_doc`]: whether the frozen slice spans lines.
    /// Unlike [`Self::frozen_member_forces_break`] the frozen flag is NOT taken —
    /// callers ask this only for an already-known-frozen member, so a width-decided
    /// caller forces its broken layout explicitly on a `true` answer (a
    /// `verbatim_source_span` is `will_break`-opaque).
    pub(in crate::printer) fn frozen_list_member_multiline(&self, t: &TSType<'_>) -> bool {
        self.frozen_member_multiline(t, |_| false)
    }

    /// [`Self::build_frozen_member_doc`] plus the single-child heads' must-break
    /// ([`Self::with_frozen_must_break`]) — the one spelling every head freeze uses,
    /// whatever its precedence-paren rule (`member_parens`: the type-parameter
    /// constraint site keeps a conditional value's clarity parens, the conditional
    /// `extends` site its own). Same catalog class as the multiline-member
    /// divergences: prettier glues the container flat around its printed-ignored slice
    /// (`annotation_prettier_ignore_multiline_value`).
    pub(in crate::printer) fn build_frozen_head_doc(
        &self,
        child: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        let slice = self.frozen_member_slice_span(child, member_parens);
        self.with_frozen_must_break(
            self.build_frozen_member_doc_at(child, member_parens, slice),
            slice,
        )
    }

    /// [`Self::build_frozen_head_doc`] for the heads whose child never needs precedence
    /// parens (annotation `:`, alias `=`, named-tuple `label:`, mapped value `]:`, a
    /// cast's type, a `=>` return): a source paren there is always redundant, so it
    /// drops under the freeze unless its shell holds a comment.
    pub(in crate::printer) fn build_frozen_single_child_doc(&self, child: &TSType<'_>) -> DocId {
        self.build_frozen_head_doc(child, |_| false)
    }

    /// The doc for the child of a head whose gap ROUTE has already fired (an
    /// alone-on-line directive sits in the gap, or in a stripped shell's interior): the
    /// frozen verbatim slice when the child is a freeze target, else the ordinarily-built
    /// child — a composite declines the freeze here and applies Rule A inside via its own
    /// leading-run walk, while the head still owns the directive's own-line emission.
    pub(in crate::printer) fn build_routed_child_doc(&self, child: &TSType<'_>) -> DocId {
        if is_freeze_target(child) {
            self.build_frozen_single_child_doc(child)
        } else {
            self.build_type_doc(child)
        }
    }

    /// Whether a parenthesized member's shell — the bytes between `(` and the inner type,
    /// or between the inner and `)` — physically holds a comment, in which case the
    /// paren-transparent freeze must keep the WHOLE member span (slicing the inner drops
    /// that comment). **In-source axis** (`comments_in_source_range`), not to-emit: a
    /// glued shell comment (`(/* c */ a1)`) is owned, so the to-emit axis would miss it,
    /// yet the frozen raw slice never routes it through `build_comment_doc` — it would be
    /// dropped either way, so the physical-presence question is the correct one.
    fn frozen_paren_shell_has_comment(&self, t: &TSType<'_>) -> bool {
        if !matches!(t, TSType::Parenthesized(_)) {
            return false;
        }
        let inner = unwrap_parenthesized(t);
        comments_in_source_range(self.comments, t.span().start, inner.span().start)
            .next()
            .is_some()
            || comments_in_source_range(self.comments, inner.span().end, t.span().end)
                .next()
                .is_some()
    }

    /// [`Self::build_frozen_member_doc`] with the union's per-member `align(2)` offset
    /// (mirroring `build_union_member_offset_doc`, so a frozen member aligns with its
    /// reformatted siblings in the broken layout). An object-literal member supplies its
    /// own layout, so — frozen verbatim (opaque) — it takes no offset.
    pub(in crate::printer) fn build_frozen_union_member_offset_doc(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        let d = self.d();
        let inner = unwrap_parenthesized(t);
        // A BARE object member supplies its own layout → verbatim, no offset. A
        // parenthesized object with a shell comment routes through `build_frozen_member_doc`
        // (whole-span freeze) so the shell comment is not dropped.
        if matches!(inner, TSType::TypeLiteral(_)) && !self.frozen_paren_shell_has_comment(t) {
            return self.raw_source_range(inner.span().start, inner.span().end);
        }
        d.align(2, self.build_frozen_member_doc(t, member_parens))
    }
}
