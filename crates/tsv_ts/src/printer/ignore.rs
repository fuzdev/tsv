// Format-ignore directive honoring for type-member lists (union / intersection /
// tuple / type-parameter / type-argument members), single-child type heads (the
// type after an annotation's `:`, an alias's `=`, a type parameter's `extends`/`=`,
// a named tuple member's `label:`, a mapped type's `]:` value and key-side gaps),
// and annotation heads (the gap BEFORE a `:` — a binding, a property signature, an
// index signature's value, a signature's return type — where the whole `: type`
// freezes).
//
// Value-side argument and element lists join the list family through
// `args_frozen_span` / `element_frozen_span` and the freeze-aware item builder
// `build_arg_item_doc` — a call's, a `new`'s and a member chain's arguments, and an
// array literal's or array pattern's elements. A lone item whose gap anchor its
// printer already holds (dynamic `import()`'s two named fields, a JSDoc cast's
// interior) asks `gap_frozen_span` instead, the same question without the list
// index. Each freezes over the item's own node span, so a spread or rest `...` rides
// inside; an element SLOT may be a hole, which never freezes and contributes only
// its comma to the next slot's gap.
//
// Parameter lists (value-side and type-side alike) join the list family through
// `param_frozen_span`, over each parameter's RENDER span so a decorated parameter
// freezes from its first decorator. A decorated parameter has a SECOND freeze position
// inside itself — a directive between its decorators and its binding freezes just the
// binding — which lives with the decorator printer it composes with
// (`Printer::build_frozen_param_binding_doc`, `printer/decorators.rs`); an index
// signature's key parameter asks `member_gap_frozen` on the `[`→key gap directly.
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
// docs/conformance_prettier_ignore.md §Format-ignore directive for the behavior contract.
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

use super::ParenContext;
use super::Printer;
use super::unwrap_parenthesized;
use crate::ast::internal::{self, Comment, TSType};
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{
    Span, directive_alone_on_line, is_format_ignore_directive, is_honored_format_ignore,
};

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
    /// `node_start` and whose first member's paren-stripped span is `first_inner`. The
    /// span-shaped core of [`Self::composite_leading_run_freeze`], which is the only
    /// entry — and which owns the `has_format_ignore` gate, so nothing here re-reads it.
    ///
    /// A directive alone on its line freezes the first member (with `multiline` set
    /// when the frozen first slice spans lines, so the caller forces the broken
    /// layout); any other placement is inert.
    fn leading_run_freeze(
        &self,
        node_start: u32,
        first_inner: Option<Span>,
    ) -> Option<LeadingRunFreeze> {
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

    /// [`Self::leading_run_freeze`] for a composite taken as its span start plus its
    /// member slice — the one entry every union and intersection site uses, so the first
    /// member's paren-strip is spelled here instead of at each of them.
    ///
    /// Opens on the document-level bool, so a directive-free document pays one predicted
    /// branch and never that paren-unwrap — every union and intersection in the document
    /// asks this question (the same reason [`Self::list_item_frozen`] and
    /// [`Self::single_child_frozen`] open with it).
    #[inline]
    pub(in crate::printer) fn composite_leading_run_freeze(
        &self,
        node_start: u32,
        types: &[TSType<'_>],
    ) -> Option<LeadingRunFreeze> {
        if !self.has_format_ignore {
            return None;
        }
        let first_inner = types.first().map(|t| unwrap_parenthesized(t).span());
        self.leading_run_freeze(node_start, first_inner)
    }

    /// True when the gap `[prev_end, member_start)` before a list item carries a
    /// format-ignore directive **alone on its line** — the one placement that freezes.
    /// Any other placement — TRAILING the previous member, the separator
    /// (`{ a: 1 } & // prettier-ignore`), an opening delimiter, a declaration head, or
    /// glued before the member (`a | /* prettier-ignore */ b`) — is inert (the
    /// wrong-node-misbind floor; the `trailing_inert` fixtures are its regression pins,
    /// the `glued_inert` fixtures pin the glued side).
    ///
    /// **The single Rule A predicate for every family**, value-level and type-level
    /// alike: statement lists (`Program.body`, a block body), class / interface / enum
    /// bodies, object properties and destructuring patterns, a member-chain `.` gap, and
    /// — through [`Self::list_item_frozen`], [`Self::mapped_gap_frozen`],
    /// [`Self::single_child_frozen`] and the annotation heads — every type position. All
    /// of them ask the identical question about the identical window, so they share one
    /// predicate rather than each re-deriving directive recognition.
    ///
    /// The test keys on the directive's own line, NOT on `is_same_line` against
    /// `prev_end`: a blank line injected between `prev_end` and a trailing directive would
    /// move the directive off `prev_end`'s line yet leave it trailing the separator, and
    /// keying on `prev_end` would flip the freeze on and off across that blank (a
    /// non-idempotency `blank_audit` catches). `prev_end` still bounds the comment window.
    ///
    /// Opens on the document-level `has_format_ignore` flag, so a directive-free document
    /// (≈ every document) pays one predicted branch instead of the range scan and the
    /// directive match — the gate every entry in this module shares.
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
            && self
                .comments_in_source_between(prev_end, member_start)
                .any(|c| self.is_honored_directive(c))
    }

    /// Whether comment `c` is a format-ignore directive that HONORS — the recognizer plus
    /// the placement floor below, in one predicate. The freeze tests ask it through
    /// [`Self::member_gap_frozen`]; an emitter asks it directly when a comment's own
    /// layout depends on the answer, because an honored directive must keep the line the
    /// author gave it — collapsing it onto the frozen value's line would make it glued,
    /// hence inert, and the freeze would be lost on the next pass.
    ///
    /// The document-level `has_format_ignore` gate lives HERE rather than at each call
    /// site: no directive in the document means no honored one, so a caller that spelled
    /// the flag itself was restating the predicate — and a caller that forgot to differed
    /// from its neighbors for no reason. [`Self::member_gap_frozen`] keeps its own copy,
    /// where the flag buys something this one can't: skipping the range scan entirely.
    pub(in crate::printer) fn is_honored_directive(&self, c: &Comment) -> bool {
        self.has_format_ignore && is_honored_format_ignore(self.source, c)
    }

    /// Whether the FIRST comment a run in `[start, end)` emits is an honored directive —
    /// the leading-separator half of the rule [`Self::comment_hangs_next`] states for the
    /// separators inside a run: an honored directive must own its line, so a run that opens
    /// with one opens on a line of its own rather than beside the token before it.
    pub(in crate::printer) fn leading_comment_is_honored_directive(
        &self,
        start: u32,
        end: u32,
    ) -> bool {
        self.has_format_ignore
            && self
                .comments_to_emit_between(start, end)
                .next()
                .is_some_and(|c| self.is_honored_directive(c))
    }

    /// The placement floor alone — [`tsv_lang::directive_alone_on_line`] against this
    /// document's source, for the emitters that ask about placement without the
    /// recognizer.
    pub(in crate::printer) fn directive_alone_on_line(&self, c: &Comment) -> bool {
        directive_alone_on_line(self.source, c)
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

    /// The frozen `: type` for an annotation HEAD gap — the gap *before* the `:`, between
    /// the head (a binding name plus any `?`/`!`, an index signature's `]`, a signature's
    /// `)`) and the annotation. An alone-on-line directive there freezes the WHOLE
    /// annotation node, the one it precedes: `Some` is the gap's own-line-preserving
    /// comment run followed by the verbatim `: type` slice; `None` leaves the caller's
    /// ordinary emission in place. The caller pushes the head itself (no trailing space)
    /// and concatenates this after it.
    ///
    /// **Span-shaped, and deliberately NOT composite-transparent** — unlike a single-child
    /// head ([`Self::single_child_frozen`]), where a union / intersection child declines so
    /// the member rules apply instead. Those rules can only apply where the composite's own
    /// leading run REACHES the directive, and that run crosses whitespace and the
    /// transparent `|` / `&` / `(` only — the annotation's `:` blocks it. So a composite
    /// value here rides *inside* the frozen span, and the head rules and the member rules
    /// still can never both claim one directive.
    ///
    /// The emission is [`Self::append_keyword_value_line_comments`], the same
    /// own-line-preserving run the single-child heads use and for the same reason: every
    /// one of these heads' default emitters trails the gap's first comment on the head's
    /// line, an inert placement that would lose the freeze on the second pass. The block
    /// spelling routes here too — placement, not spelling, keys the freeze, and the run
    /// keeps an own-line block on its own line.
    ///
    /// The document-level flag is read HERE rather than left to
    /// [`Self::member_gap_frozen`] two calls down: every annotation in the document asks
    /// this question, so a directive-free document pays one predicted branch at the entry
    /// instead of a call chain (the same reason [`Self::single_child_frozen`] opens with it).
    pub(in crate::printer) fn build_frozen_annotation_head_doc(
        &self,
        gap_start: u32,
        annotation: &internal::TSTypeAnnotation<'_>,
    ) -> Option<DocId> {
        if !self.has_format_ignore {
            return None;
        }
        self.build_frozen_head_span_doc(gap_start, annotation.span)
    }

    /// [`Self::build_frozen_annotation_head_doc`] for an annotation preceded by an optional
    /// `?` / definite `!` MARKER, asked on the key→marker gap: a directive alone on its line
    /// there precedes `?: T` as a unit, so the freeze starts at the marker and the caller
    /// must emit neither the marker nor the annotation itself. `Some` carries the frozen
    /// doc plus the cursor just past it (the annotation's span end), so the caller advances
    /// exactly as its ordinary arm would. `None` leaves the ordinary emission in place —
    /// including the case where the directive sits AFTER the marker, which the plain
    /// annotation-head ask then routes on the marker→`:` gap.
    ///
    /// `marker` and `annotation` are taken as options so the two hosts — a class property
    /// (`?` or `!`) and a property signature (`?` only) — share one seam instead of each
    /// spelling the have-both test. The marker scan runs only in a directive-bearing
    /// document (the flag is checked first) and is bounded by the annotation's `:`.
    pub(in crate::printer) fn build_frozen_marker_annotation_tail(
        &self,
        gap_start: u32,
        marker: Option<u8>,
        annotation: Option<&internal::TSTypeAnnotation<'_>>,
    ) -> Option<(DocId, u32)> {
        if !self.has_format_ignore {
            return None;
        }
        let (marker, annotation) = marker.zip(annotation)?;
        let marker_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            gap_start as usize,
            annotation.span.start as usize,
            marker,
        )? as u32;
        let frozen =
            self.build_frozen_head_span_doc(gap_start, Span::new(marker_pos, annotation.span.end))?;
        Some((frozen, annotation.span.end))
    }

    /// The shared core of the two annotation-head entries: the directive must be alone on its
    /// line in `[gap_start, frozen.start)`, and `frozen` is emitted verbatim after the gap's
    /// own-line-preserving comment run.
    fn build_frozen_head_span_doc(&self, gap_start: u32, frozen: Span) -> Option<DocId> {
        if !self.member_gap_frozen(gap_start, frozen.start) {
            return None;
        }
        let d = self.d();
        let mut parts: DocBuf = smallvec![];
        self.append_keyword_value_line_comments(
            &mut parts,
            gap_start,
            frozen.start,
            self.build_frozen_span_doc(frozen),
        );
        Some(d.concat(&parts))
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
    ///
    /// Opens on the document-level bool — ~25 container loops ask this PER ITEM, the
    /// hottest ignore-seam position by a wide margin, so a directive-free document must
    /// pay exactly that one predicted branch and never the span closure behind it (the
    /// same reason [`Self::single_child_frozen`] opens with it; `member_gap_frozen`
    /// re-checks the flag, harmlessly, for its other callers).
    #[inline]
    pub(in crate::printer) fn list_item_frozen(
        &self,
        container_start: u32,
        item_span: &impl Fn(usize) -> Span,
        i: usize,
    ) -> bool {
        if !self.has_format_ignore {
            return false;
        }
        if i == 0 {
            self.member_gap_frozen(container_start, item_span(0).start)
        } else {
            self.member_gap_frozen(item_span(i - 1).end, item_span(i).start)
        }
    }

    /// [`Self::list_item_frozen`] for parameter list item `i`, over the parameter's
    /// RENDER span ([`Self::param_render_span`]) rather than its node span, so a
    /// decorated parameter's gap ends where its first decorator begins — the directive
    /// precedes the decorator, not the binding.
    ///
    /// `Some` is the span to freeze: that same render span, so a parameter property's
    /// modifiers (`public p = 1`), a parameter decorator (`@dec() p: T`), and a rest
    /// parameter's `...` all ride inside the freeze, matching prettier. The list's `,` is
    /// parent-owned and stays outside. Handing back the span rather than a bool keeps the
    /// decorator scan to one pass — every caller freezes exactly what was measured.
    ///
    /// `open_paren` is the `(` position (the delimited-list anchor opens just past it);
    /// with no located paren the first item's gap is empty, so nothing can freeze there.
    #[inline]
    pub(in crate::printer) fn param_frozen_span(
        &self,
        open_paren: Option<u32>,
        params: &[internal::Expression<'_>],
        i: usize,
    ) -> Option<Span> {
        if !self.has_format_ignore {
            return None;
        }
        let item_span = |j: usize| self.param_render_span(&params[j]);
        let container_start = open_paren.map_or_else(|| item_span(0).start, |p| p + 1);
        self.list_item_frozen(container_start, &item_span, i)
            .then(|| item_span(i))
    }

    /// [`Self::member_gap_frozen`] resolved to the span it freezes, for a value-side item
    /// whose gap anchor the caller already holds — a lone argument (the JSDoc cast's
    /// interior, dynamic `import()`'s two named fields, which are not a slice at all) or
    /// an element slot, whose anchor skips the holes before it.
    ///
    /// `Some` is the span to freeze: the item's own node span, so a spread or rest `...`
    /// rides inside the freeze (the node starts at the `...`, so its span already covers
    /// it), matching prettier. The list's `,` is parent-owned and stays outside.
    #[inline]
    pub(in crate::printer) fn gap_frozen_span(&self, prev_end: u32, span: Span) -> Option<Span> {
        self.member_gap_frozen(prev_end, span.start).then_some(span)
    }

    /// The value-side analog of [`Self::single_child_frozen`]: a construct that holds ONE
    /// value behind a delimiter of its own — a `for` header's `(`→init / `;`→test /
    /// `;`→update clauses, a restricted production's grouping `(` (`return` / `throw` /
    /// `yield`), a Svelte `{…}` directive value or expression tag. An alone-on-line
    /// directive in the delimiter→value gap freezes the value WHOLE.
    ///
    /// `Some` is the span to freeze: the value's own node span, so the delimiter that
    /// CLOSES it (the header's `;`, the grouping `)`, the closing `}`) stays parent-owned —
    /// prettier's `for`-init slice swallows the `;` and emits a header that no longer
    /// parses (`init_declaration_prettier_ignore_head_prettier_divergence`).
    ///
    /// Span-shaped rather than node-shaped because a `for` init is a `VariableDeclaration`
    /// as often as an `Expression`, and **not** composite-transparent: a value's own
    /// member rules apply only where the value's leading run REACHES the directive, and
    /// every delimiter here blocks that run — so a sequence value rides whole inside the
    /// slice, which is exactly what prettier does, and the head rules and the member rules
    /// can never both claim one directive. The member half is the sequence's inter-operand
    /// gap, which asks [`Self::gap_frozen_span`] per operand.
    ///
    /// The same predicate as [`Self::gap_frozen_span`] — the gap is a gap either way, and
    /// the `has_format_ignore` gate is already spent inside it. The two names exist because
    /// the *rules* differ (a head claims the whole value; a member claims one item), and a
    /// call site that spells which one it is documents the shape it froze.
    #[inline]
    pub(in crate::printer) fn value_head_frozen_span(
        &self,
        gap_start: u32,
        value: Span,
    ) -> Option<Span> {
        self.gap_frozen_span(gap_start, value)
    }

    /// [`Self::value_head_frozen_span`] resolved to the doc it emits, for a value head whose
    /// unfrozen emission is the plain [`Self::build_expression_doc`] — the `export`→value and
    /// `export default`→value gaps, the twin heads that must not disagree, and an **enum
    /// member's** `=`→value gap, whose enclosing member loop claims the value→`,` gap
    /// (`Printer::collect_trailing_comments`) so no paren shell stands between the slice and
    /// the separator.
    ///
    /// Deliberately NOT for a head with an ordinary builder of its own (an `if` condition, a
    /// `for` update clause, an assignment RHS, an object property value): those resolve the
    /// span themselves and pair it with the builder the position calls for, and routing them
    /// through here would silently substitute the plain one. The statement analog
    /// ([`Self::build_statement_head_doc`]) takes its ordinary builder lazily for exactly
    /// that reason — every statement head has a different one.
    /// `ordinary` is taken LAZILY, for the same reason the statement analog takes its own
    /// ([`Self::build_statement_head_doc`]): the positions sharing this seam do not share a
    /// value builder. An enum member's initializer wants the plain
    /// [`Self::build_expression_doc`] — it supplies its own indent around the value — while
    /// `export default` and `export =` are continuation-indent positions and want
    /// [`Self::build_continuation_indent_expression_doc`]. A fixed builder here would silently
    /// substitute the plain one at a position that is not, which is exactly the bug the
    /// paragraph above warns about; making each caller name its builder retires the class.
    #[inline]
    pub(in crate::printer) fn build_value_head_doc(
        &self,
        gap_start: u32,
        value: &internal::Expression<'_>,
        ordinary: impl FnOnce() -> DocId,
    ) -> DocId {
        match self.value_head_frozen_span(gap_start, value.span()) {
            Some(frozen) => self.build_frozen_expression_doc(value, frozen),
            None => ordinary(),
        }
    }

    /// The statement-side analog of [`Self::value_head_frozen_span`]: a head that introduces
    /// ONE statement behind a delimiter of its own — an `if`'s `)`→consequent and
    /// `else`→alternate gaps, and every loop's header→body gap (`while`, do-while, all three
    /// `for` spellings, block and non-block bodies alike). An alone-on-line directive there
    /// freezes the body whole, and `ordinary` — the caller's own builder, taken lazily so a
    /// directive-free document never pays for the frozen path's setup — supplies the
    /// unfrozen doc.
    ///
    /// The frozen slice is the body statement's own span, so the introducing delimiter stays
    /// parent-owned. Unlike the value heads there is no paren shell to re-synthesize — a
    /// statement is never parenthesized by the printer — and the emission is
    /// [`Self::build_frozen_statement_doc`], so a frozen body whose kind owes a `;` (a bare
    /// `break` / `continue` / `debugger` relying on ASI) gets it restored, exactly as at the
    /// statement-list seams. The `export default` declaration VALUES take the span-shaped
    /// sibling ([`Self::build_declaration_value_head_doc`]) instead — those kinds never owe
    /// one.
    ///
    /// A head whose ordinary layout would pull the gap's comment onto the head's line must
    /// ask this BEFORE choosing that layout and keep the directive's own line when it fires:
    /// a head-trailing placement is inert under the floor, so the relocated form would lose
    /// the freeze on the second pass.
    #[inline]
    pub(in crate::printer) fn build_statement_head_doc(
        &self,
        gap_start: u32,
        stmt: &internal::Statement<'_>,
        ordinary: impl FnOnce() -> DocId,
    ) -> DocId {
        match self.gap_frozen_span(gap_start, stmt.span()) {
            Some(_) => self.build_frozen_statement_doc(stmt),
            None => ordinary(),
        }
    }

    /// [`Self::build_statement_head_doc`] for a head whose value is SPAN-shaped rather than
    /// a `Statement` — the `export default` declaration values (function / declare-function
    /// / class / interface), none of which ever owes a `;`, so the emission is the plain
    /// [`Self::build_frozen_node_doc`] over the slice.
    #[inline]
    pub(in crate::printer) fn build_declaration_value_head_doc(
        &self,
        gap_start: u32,
        value: Span,
        ordinary: impl FnOnce() -> DocId,
    ) -> DocId {
        match self.gap_frozen_span(gap_start, value) {
            Some(frozen) => self.build_frozen_node_doc(frozen),
            None => ordinary(),
        }
    }

    /// [`Self::list_item_frozen`] for a value-side ARGUMENT list item `i` — a call's,
    /// a `new`'s or a member chain's arguments. `Some` is the span to freeze, per
    /// [`Self::gap_frozen_span`].
    ///
    /// `container_start` is where the FIRST item's gap opens, per each family's own
    /// convention: just past the `[` for a bracketed list, and for a call the position
    /// the `(` follows (the callee's, or the type-argument list's, end) — the same window
    /// every leading-argument-comment emitter at those sites already opens, so a
    /// directive written above the `(` leads the first argument exactly as it does for
    /// those emitters (prettier-confirmed).
    #[inline]
    pub(in crate::printer) fn args_frozen_span(
        &self,
        container_start: u32,
        args: &[internal::Expression<'_>],
        i: usize,
    ) -> Option<Span> {
        if !self.has_format_ignore {
            return None;
        }
        let item_span = |j: usize| args[j].span();
        self.list_item_frozen(container_start, &item_span, i)
            .then(|| item_span(i))
    }

    /// [`Self::args_frozen_span`] over an ELEMENT slot slice — an array literal's or
    /// array pattern's `elements`, where a slot may be a HOLE (elision).
    ///
    /// A hole never freezes: it has no span for a verbatim slice to cover. And a run of
    /// holes before slot `i` contributes only its commas, so the gap window keeps the
    /// same shape (commas, whitespace, comments) when anchored at the previous PRESENT
    /// element's end — or at `container_start` when every earlier slot is a hole. That
    /// anchor rule is why this is spelled here beside [`Self::list_item_frozen`], the
    /// other home of the gap-anchor convention, rather than at the element loops.
    pub(in crate::printer) fn element_frozen_span(
        &self,
        container_start: u32,
        elements: &[Option<internal::Expression<'_>>],
        i: usize,
    ) -> Option<Span> {
        if !self.has_format_ignore {
            return None;
        }
        let elem = elements[i].as_ref()?;
        let prev_end = elements[..i]
            .iter()
            .rev()
            .flatten()
            .next()
            .map_or(container_start, |e| e.span().end);
        self.gap_frozen_span(prev_end, elem.span())
    }

    /// [`Self::list_item_frozen`] over a `TSType` slice, plus the union /
    /// intersection `freeze_first` arm (the out-of-span leading-run directive, which
    /// applies only to the first member of an undelimited list).
    ///
    /// Opens on the same document-level bool, which also subsumes the `freeze_first`
    /// arm: that flag comes from [`Self::leading_run_freeze`], itself gated on it, so it
    /// can only be set in a directive-bearing document.
    #[inline]
    pub(in crate::printer) fn list_member_frozen(
        &self,
        container_start: u32,
        types: &[TSType<'_>],
        i: usize,
        freeze_first: bool,
    ) -> bool {
        self.has_format_ignore
            && ((i == 0 && freeze_first)
                || self.list_item_frozen(container_start, &|j| types[j].span(), i))
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

    /// What a freeze over a NODE's own span EMITS: the verbatim slice
    /// ([`Self::build_frozen_span_doc`]) plus the one node-level fact the span alone can't
    /// carry — a block comment **glued** before the node is owned by it, rides *inside* the
    /// doc the slice replaces, and is skipped by every gap emitter, so the freeze must claim
    /// it or nothing prints it (docs/comments.md hazard 1, the ownership-is-about-who-PRINTS
    /// rule). Prettier keeps the comment right where it is, glued before the frozen slice.
    ///
    /// The one emitter every node-shaped freeze shares, whatever rule resolved it: a list
    /// item over its own span (a declarator, a namespace binding), a statement head's body
    /// ([`Self::build_statement_head_doc`], and the `export` / `export default` heads that
    /// resolve their own span because their gap emitter is the shared declaration-header run
    /// they need either way), and the delimiter-owned value heads whose value is span-shaped
    /// rather than an `Expression` (a `for` init clause, a for-in/of left, a restricted
    /// production's operand). [`Self::build_frozen_expression_doc`] is this plus the one rule
    /// that belongs to an `Expression` node rather than to its span.
    ///
    /// `gaps:audit` found every miss this claim exists to prevent, by injecting a block
    /// comment before a frozen node — which the print-once ledger over the fixtures as
    /// authored cannot see, since no fixture glues a comment to a frozen item: a frozen
    /// argument dropped it, and so did `if (a)⏎// prettier-ignore⏎/* c */ fn( b );` and
    /// `export default⏎// prettier-ignore⏎/* c */ @dec class {}`.
    #[inline]
    pub(in crate::printer) fn build_frozen_node_doc(&self, frozen: Span) -> DocId {
        self.prepend_owned_leading_comment_at(frozen.start, self.build_frozen_span_doc(frozen))
    }

    /// [`Self::build_frozen_node_doc`] for a frozen STATEMENT — a list member, a header's
    /// body ([`Self::build_statement_head_doc`]), a labeled statement's body, or an
    /// `export`ed declaration — plus the one terminator rule the span alone can't carry: a
    /// frozen statement whose kind always takes a `;` — and whose author relied on ASI, so
    /// the slice doesn't end with one — emits the `;` prettier emits. Prettier's slice for
    /// these kinds ends BEFORE any authored `;` (its `locEnd` override stops at the last
    /// declarator / the keyword / the label) and `shouldIgnoredNodePrintSemicolon`
    /// unconditionally re-appends it; tsv's slice keeps an authored `;`, so the append
    /// fires only when it is missing. Without it the freeze swallows the terminator ASI
    /// supplied, and the NEXT statement's printed form can fabricate a leading `(` that
    /// re-binds the pair into one broken statement (`const a = x` + `y => 1` →
    /// `x(y) => 1`) — output that does not reparse. Pinned by the
    /// `typescript/syntax/asi/prettier_ignore_semicolon*` family (list, case-consequent,
    /// header-body and `export`-head spellings).
    ///
    /// The kind set is prettier's exactly: `VariableDeclaration`, `break` / `continue` /
    /// `debugger`, and an `if` / loop / label whose TAIL statement is one of those
    /// ([`Self::frozen_statement_kind_takes_semicolon`]). The content-end family
    /// (`ExpressionStatement`, the whole-`export` list positions, `return` / `throw` /
    /// do-while) is deliberately not in it: prettier re-appends there only when the source
    /// carried a `;` — which tsv's slice already keeps — and gives the ASI spellings no
    /// `;` at all.
    // TODO: the ASI `ExpressionStatement` and list-position `export` spellings still
    // freeze terminator-less on both sides, and prettier's own output does not reparse
    // there — that half wants a deliberate divergence rule + fixture of its own.
    pub(in crate::printer) fn build_frozen_statement_doc(
        &self,
        stmt: &internal::Statement<'_>,
    ) -> DocId {
        let frozen = stmt.span();
        let doc = self.build_frozen_node_doc(frozen);
        if Self::frozen_statement_kind_takes_semicolon(stmt)
            && !frozen.extract(self.source).ends_with(';')
        {
            let d = self.d();
            return d.concat(&[doc, d.text(";")]);
        }
        doc
    }

    /// Whether a frozen statement's kind unconditionally ends in a `;` — prettier's
    /// `shouldIgnoredNodePrintSemicolon`, minus its content-end arm (see
    /// [`Self::build_frozen_statement_doc`]). A header kind resolves through its TAIL
    /// statement the way prettier does: the statement's terminator is its tail's.
    fn frozen_statement_kind_takes_semicolon(mut stmt: &internal::Statement<'_>) -> bool {
        loop {
            stmt = match stmt {
                internal::Statement::VariableDeclaration(_)
                | internal::Statement::BreakStatement(_)
                | internal::Statement::ContinueStatement(_)
                | internal::Statement::DebuggerStatement(_) => return true,
                internal::Statement::IfStatement(s) => s.alternate.unwrap_or(s.consequent),
                internal::Statement::ForStatement(s) => s.body,
                internal::Statement::ForInStatement(s) => s.body,
                internal::Statement::ForOfStatement(s) => s.body,
                internal::Statement::WhileStatement(s) => s.body,
                internal::Statement::LabeledStatement(s) => s.body,
                _ => return false,
            };
        }
    }

    /// [`Self::build_frozen_node_doc`] minus the must-break — the claim, over a slice whose
    /// multi-line-ness is deliberately kept **opaque** to its container.
    ///
    /// The two differ by exactly that signal, and the difference is a real rule rather than an
    /// oversight: a frozen MEMBER EXPRESSION is layout-opaque on purpose, so every container
    /// around it keeps its own tight layout (`f1(fn(2.0000)⏎// prettier-ignore⏎.c)` stays
    /// hugged, the ternary stays inline), which is what
    /// `typescript/expressions/member/prettier_ignore_containers` pins. Handing this site the
    /// must-break variant tears each of those containers open.
    ///
    /// The claim itself is not optional either way: the slice starts at the member's own first
    /// byte, so a block comment **glued** ahead of the chain base — owned by it, skipped by
    /// every gap emitter — sits outside the slice and nothing else prints it
    /// (`docs/comments.md` hazard 1, the replacement shape). Widening the slice is not the
    /// alternative: the comment would then be frozen text and stop reindenting with its
    /// context. Pinned by `typescript/expressions/member/prettier_ignore_base_comment`, and by
    /// the Svelte attribute spelling `svelte/syntax/prettier_ignore/attr_expression`, whose
    /// `={`→base gap is the same position one host out.
    #[inline]
    pub(in crate::printer) fn build_frozen_opaque_node_doc(&self, frozen: Span) -> DocId {
        self.prepend_owned_leading_comment_at(
            frozen.start,
            self.raw_source_range(frozen.start, frozen.end),
        )
    }

    /// The frozen verbatim slice for a value in a paren-context position, with the clarity
    /// parens that context supplies re-synthesized around it: an assignment prints as
    /// `fn((a = b))` as an argument and as `if ((a = b))` as a condition, and those parens
    /// are the printer's rather than the author's, so they belong OUTSIDE the frozen slice
    /// — prettier freezes the inner and keeps them too (`fn(⏎// prettier-ignore⏎(a = b  +
    /// c))`). Same rule, and the same reason, as [`Self::build_frozen_member_doc`]'s
    /// bare-member arm.
    ///
    /// `ctx` is the position's own [`ParenContext`] — the one its ordinary path passes to
    /// [`Self::needs_parens`], so the frozen and unfrozen forms can never disagree about
    /// the shell.
    ///
    /// The must-break rides in the doc ([`Self::build_frozen_span_doc`]): every layout in
    /// this family is width-decided, and a `verbatim_source_span` is `will_break`-opaque,
    /// so a multi-line frozen item has to say so explicitly.
    ///
    /// ⚠️ **This emits the pair and nothing else, so it is only for a position whose
    /// ENCLOSING seam claims the slice→`)` gap** — an argument or element (the element-comma
    /// seam), a statement test, a binding default. Those all float such a comment out past
    /// the `)`, which is what their unfrozen twins do too. A position that owns everything
    /// inside its own boundary — a declarator initializer, an assignment RHS — must take
    /// [`Self::build_frozen_value_shell_doc`] instead: nothing out there can see between the
    /// parens, so a bare pair leaves that gap with no emitter and DROPS the comment
    /// (`docs/comments.md` hazard 4), which is what both of those hosts did while they
    /// spelled the bare pair here.
    pub(in crate::printer) fn build_frozen_value_doc(
        &self,
        value: &internal::Expression<'_>,
        frozen: Span,
        ctx: ParenContext,
    ) -> DocId {
        let doc = self.build_frozen_expression_doc(value, frozen);
        if self.needs_parens(value, ctx) {
            // Parens outside the claimed comment, exactly as the ordinary path nests them
            // (`d.parens(build_expression_doc(..))`, whose owned-comment prepend is inside).
            self.d().parens(doc)
        } else {
            doc
        }
    }

    /// [`Self::build_frozen_value_doc`] at an ARGUMENT or ELEMENT item, the family that
    /// names its own emitter because every argument/element loop dispatches through it.
    #[inline]
    pub(in crate::printer) fn build_frozen_arg_doc(
        &self,
        arg: &internal::Expression<'_>,
        frozen: Span,
    ) -> DocId {
        self.build_frozen_value_doc(arg, frozen, ParenContext::Argument)
    }

    /// [`Self::build_frozen_node_doc`] for an EXPRESSION — the verbatim slice and the
    /// owned-comment claim, plus the one rule that is a property of the *node* rather than
    /// of its span.
    ///
    /// A **`SequenceExpression`** gets back the grouping parens it prints for itself. Those
    /// parens are required (`fn((0, 1))` passes ONE argument; `fn(0, 1)` passes two) but
    /// they are printed by `build_sequence_doc`, not by the enclosing context — which is why
    /// `needs_parens` deliberately answers `false` for it — and they sit OUTSIDE the node's
    /// span, so a verbatim slice of the span alone drops them and changes the meaning. The
    /// rule belongs here rather than at [`Self::build_frozen_value_doc`] because it is a
    /// property of the node, not of the position: the cast interior needs it too.
    ///
    /// The value heads whose value is span-shaped rather than an `Expression` (a `for` init
    /// clause, a for-in/of left, a restricted production's operand — where the layout's own
    /// hanging parens ARE the grouping, so a re-synthesized pair would double it) take
    /// [`Self::build_frozen_node_doc`] directly.
    pub(in crate::printer) fn build_frozen_expression_doc(
        &self,
        expr: &internal::Expression<'_>,
        frozen: Span,
    ) -> DocId {
        let doc = self.build_frozen_node_doc(frozen);
        if matches!(expr, internal::Expression::SequenceExpression(_)) {
            self.d().parens(doc)
        } else {
            doc
        }
    }

    /// The doc for argument item `i`: the frozen verbatim slice when Rule A fires
    /// ([`Self::args_frozen_span`]), else the ordinary argument-context doc. The one
    /// freeze-aware twin of `build_arg_expression_doc`, so no argument loop spells the
    /// dispatch itself.
    ///
    /// Its sole caller is `calls::build_joined_argument_doc`, which every call, `new` and
    /// member-chain argument loop routes through — the layer that adds the *other*
    /// per-argument question (a curried chain's progressive layout) to this one. Reach for
    /// that builder at a new argument loop, not for this directly: skipping it is how an
    /// argument silently loses that routing.
    ///
    /// The document-level flag is read HERE, with the ordinary build on its own arm, rather
    /// than left to [`Self::args_frozen_span`] one call down: this is the hottest position
    /// in the module — every argument of every call, `new` and chain call asks it — so a
    /// directive-free document (≈ every document) pays exactly one predicted branch and
    /// never the `Option` or the span closure behind it. The same reason
    /// [`Self::list_item_frozen`] and [`Self::single_child_frozen`] open with it. (Measured
    /// as null either way on real corpora — the shape is doctrine, not a won benchmark.)
    #[inline]
    pub(in crate::printer) fn build_arg_item_doc(
        &self,
        container_start: u32,
        args: &[internal::Expression<'_>],
        i: usize,
    ) -> DocId {
        if !self.has_format_ignore {
            return self.build_arg_expression_doc(&args[i]);
        }
        self.args_frozen_span(container_start, args, i).map_or_else(
            || self.build_arg_expression_doc(&args[i]),
            |frozen| self.build_frozen_arg_doc(&args[i], frozen),
        )
    }

    /// The doc for a comma-list item identified by SPAN alone — a module specifier, an
    /// import attribute, a variable declarator: the frozen verbatim slice when Rule A fires
    /// ([`Self::list_item_frozen`]), else `build_ordinary()`. The freeze-aware twin for the
    /// families whose item is not an `Expression`, so no list loop spells the dispatch itself.
    ///
    /// The slice is the item's own node span; the list's `,` is parent-owned and stays
    /// outside it — so the emission is the plain [`Self::build_frozen_node_doc`], which
    /// claims the glued comment the item owns (docs/comments.md hazard 1). The
    /// grouping-paren fact does not apply here: none of these item kinds prints parens of its
    /// own around its span. A caller whose ordinary path is the rest of a loop body rather
    /// than a closure (the variable declarator list) asks [`Self::list_item_frozen`] and that
    /// emitter directly, so the two dispatch shapes still share one emitter.
    ///
    /// `build_ordinary` is lazy so the common path builds the item doc exactly once;
    /// [`Self::list_item_frozen`] already opens on the document-level flag, so a
    /// directive-free document pays one predicted branch per item.
    #[inline]
    pub(in crate::printer) fn build_span_item_doc(
        &self,
        container_start: u32,
        item_span: &impl Fn(usize) -> Span,
        i: usize,
        build_ordinary: impl FnOnce() -> DocId,
    ) -> DocId {
        if self.list_item_frozen(container_start, item_span, i) {
            self.build_frozen_node_doc(item_span(i))
        } else {
            build_ordinary()
        }
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
    /// divergence (`union_prettier_ignore_multiline_member`): prettier glues the container
    /// flat around its printed-ignored slice.
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
        self.comments_in_source_between(t.span().start, inner.span().start)
            .next()
            .is_some()
            || self
                .comments_in_source_between(inner.span().end, t.span().end)
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
