// Format-ignore directive honoring for union / intersection type members.
//
// One seam that knows what a directive is and where it sits, so the union /
// intersection printers only ever ask "freeze this member / this whole node?" and
// never re-derive directive recognition. Recognition itself stays centralized in
// `tsv_lang::is_format_ignore_directive`; this module owns the *placement*
// classification (the out-of-span leading run vs. an in-span inter-member gap) and
// the paren-transparent freeze emitter.
//
// **Rule A — list-item freeze** (the single symmetric rule, union and intersection
// alike), with a **total, placement-only classification** per directive: an OWN-LINE
// directive (only whitespace before it on its physical line) in a member list's
// leading OR inter-item gap freezes the *following* member — the first member and
// every later member identically. A GLUED directive — on the same line as, and with
// nothing but spaces/tabs before, the value or member it precedes (block spelling by
// geometry; a line comment consumes to EOL) — freezes that node *whole*: the whole
// union/intersection at the leading position, the whole member at a member gap.
// ANYTHING ELSE — content before the directive on its line and no node glued after it
// (trailing a member, a separator, or a declaration head; an intervening `|`/`&`
// breaks the glue, while other comments in the run are transparent) — is permanently
// inert. This is the same semantics tsv's
// existing honored sites already carry (a directive between `{` and the first class
// member freezes that member, not the body). See docs/conformance_prettier.md
// §Format-ignore directive for the behavior contract.
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
use super::has_newline_before_position;
use super::unwrap_parenthesized;
use crate::ast::internal::{Comment, TSType, TSUnionType};
use tsv_lang::doc::arena::DocId;
use tsv_lang::{Span, comments_in_source_range, is_format_ignore_directive};

/// The freeze target implied by a format-ignore directive in a union's or
/// intersection's leading run (the out-of-span region before the node's `span.start`).
pub(in crate::printer) enum LeadingRunFreeze {
    /// Same-line glued directive (`type T = /* format-ignore */ A | B`) → the whole
    /// node is frozen verbatim.
    Whole,
    /// Own-line directive → only the FIRST member is frozen (Rule A); `multiline` is
    /// set when the frozen slice spans lines, forcing the broken layout (a verbatim
    /// span is `will_break`-opaque, so the forcing is explicit).
    FirstMember { multiline: bool },
}

impl LeadingRunFreeze {
    /// The `(freeze_first, multiline)` flag pair of a resolved leading-run freeze.
    /// The `Whole` arm must already be handled (early-returned) by the caller — it
    /// maps to `(false, false)` here, same as no freeze at all.
    pub(in crate::printer) fn first_member_flags(freeze: Option<Self>) -> (bool, bool) {
        match freeze {
            Some(Self::FirstMember { multiline }) => (true, multiline),
            _ => (false, false),
        }
    }
}

impl<'a> Printer<'a> {
    /// The format-ignore directive comment in the adjacent leading run ending at
    /// `anchor` (a node's `span.start`), if any. The run is the maximal stretch of
    /// whitespace, transparent leading punctuation (`|`, `&`, `(`), and comment spans
    /// immediately before `anchor`; the backward walk stops at the first byte outside
    /// that set (`=`, `:`, `<`, `[`, …), which bounds the run without a `prev_end`. The
    /// directive nearest the anchor wins — its placement keys glued-vs-own-line.
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
    /// A same-line glued directive freezes the whole node; an own-line directive freezes
    /// only the first member (with `multiline` set when the frozen first slice spans
    /// lines, so the caller forces the broken layout).
    pub(in crate::printer) fn leading_run_freeze(
        &self,
        node_start: u32,
        first_inner: Option<Span>,
    ) -> Option<LeadingRunFreeze> {
        if !self.has_format_ignore {
            return None;
        }
        let directive = self.leading_run_directive(node_start)?;
        if self.is_same_line(directive.span.end, node_start) {
            Some(LeadingRunFreeze::Whole)
        } else {
            // Own-line floor, the same one `member_gap_frozen` applies: a FirstMember
            // freeze requires the directive to lead its physical line. Without it, the
            // walk's `|`/`&`/`(` transparency lets a NESTED composite member reach a
            // directive the enclosing list deliberately rejected — one TRAILING a
            // previous member or a declaration head (`type T = // prettier-ignore⏎ …`)
            // — and resurrect it as a first-member freeze. Trailing placements are
            // permanently inert (the classification in the module header).
            if !has_newline_before_position(self.source, directive.span.start) {
                return None;
            }
            // A multi-line frozen slice makes the caller force the broken layout: a
            // `verbatim_source_span` is `will_break`-opaque, so the trigger is asked
            // here instead of propagating from the slice. `is_same_line` reads
            // `comment_line_breaks`, which stays populated in every printer mode —
            // the right table for a verbatim slice, whose emitted bytes physically
            // contain the newlines regardless of mode.
            let multiline = first_inner.is_some_and(|s| !self.is_same_line(s.start, s.end));
            Some(LeadingRunFreeze::FirstMember { multiline })
        }
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
    /// carries a format-ignore directive that freezes that member — either **own-line
    /// placed** (the first non-whitespace on its physical line,
    /// `has_newline_before_position`) or **glued** directly before the member (nothing but
    /// spaces/tabs and other comments between the directive's end and `member_start` —
    /// block spelling by geometry). A directive that is neither —
    /// TRAILING the previous member, the separator (`{ a: 1 } & // prettier-ignore`), or
    /// sitting before the separator (`a /* prettier-ignore */ | b` — the `|` breaks the
    /// glue) — is inert (the wrong-node-misbind floor; the `trailing_inert` fixture is its
    /// regression pin, `glued_member` pins the glued arm).
    ///
    /// The own-line test keys on the directive's own line, NOT on `is_same_line` against
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
    fn member_gap_frozen(&self, prev_end: u32, member_start: u32) -> bool {
        if !self.has_format_ignore {
            return false;
        }
        comments_in_source_range(self.comments, prev_end, member_start).any(|c| {
            is_format_ignore_directive(c.content(self.source))
                && (has_newline_before_position(self.source, c.span.start)
                    || self.glued_directly_before(c.span.end, member_start))
        })
    }

    /// True when everything in `[directive_end, member_start)` is spaces/tabs or comment
    /// bytes — the directive is GLUED directly before the member. A bare newline or an
    /// intervening separator (`|`/`&`) breaks the glue, so a pre-separator
    /// (`a /* d */ | b`) or end-of-line directive stays trailing (inert). OTHER comments
    /// in the run are transparent (`/* d */ /* other */ member` still freezes): prettier
    /// honors a directive anywhere in a glued leading run, and the run travels with the
    /// member either way, so there is no placement signal to preserve by refusing.
    fn glued_directly_before(&self, directive_end: u32, member_start: u32) -> bool {
        let bytes = self.source.as_bytes();
        let ws_only = |lo: u32, hi: u32| {
            bytes[lo as usize..hi as usize]
                .iter()
                .all(|&b| b == b' ' || b == b'\t')
        };
        let mut pos = directive_end;
        for c in comments_in_source_range(self.comments, directive_end, member_start) {
            if !ws_only(pos, c.span.start) {
                return false;
            }
            pos = c.span.end;
        }
        ws_only(pos, member_start)
    }

    /// [`Self::member_gap_frozen`] for list member `i`, the single home of the
    /// gap-anchor convention: the FIRST member's gap opens at the container's span
    /// start (and is the only member `freeze_first` — the out-of-span leading-run
    /// directive — applies to); a LATER member's gap opens at the previous member's
    /// RAW span end — never a comma- or trailing-comment-advanced cursor — so a
    /// directive after the separator still binds forward while the own-line floor
    /// keeps a trailing directive inert. Every container loop routes through here
    /// rather than picking its own anchors.
    pub(in crate::printer) fn list_member_frozen(
        &self,
        container_start: u32,
        types: &[TSType<'_>],
        i: usize,
        freeze_first: bool,
    ) -> bool {
        if i == 0 {
            freeze_first || self.member_gap_frozen(container_start, types[0].span().start)
        } else {
            self.member_gap_frozen(types[i - 1].span().end, types[i].span().start)
        }
    }

    /// Paren-transparent frozen doc for a union / intersection member. Precedence parens
    /// are kept or dropped per `member_parens`, and the freeze stays lossless:
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
        let d = self.d();
        let inner = unwrap_parenthesized(t);
        let slice = self.frozen_member_slice_span(t, member_parens);
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
