// Owned leading comments — the comment/paren binding seam.
//
// A comment glued to the token after it is **bound to that token**, and the node the
// token begins prints it (`Comment::owned_by_node`, set by the parser). Every gap
// emitter and range lookup skips an owned comment (`comments_to_emit_in_range`), so the comment
// travels inside its node's doc — and a paren the printer synthesizes around *any*
// enclosing expression therefore lands outside the pair instead of between the two.
//
// Without this, both emission paths put the comment in front of the parens: the gap
// emitters print it before the wrapped doc, and `prepend_removed_paren_comments` hoists
// a left-edge comment out to the front of the outermost expression that starts at the
// stripped `(`. Either way the comment ends up leading a paren it was never written
// against — inert for an annotation the next token was carrying (`/* @__PURE__ */`).

use crate::ast::internal::{self, Expression};
use crate::printer::Printer;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan;

/// The child on `expr`'s **left spine** — the one whose first token is also `expr`'s
/// first token, when there is one.
///
/// Callers must still check that the child actually *starts* where `expr` does: a
/// `NewExpression` (`new F()`) and a `ParenthesizedExpression` have children that start
/// later, so they are their own left edge.
///
/// Kept separate from `needs_parens`'s `leftmost_no_lookahead`, which walks the same spine
/// for a different question (prettier's `startsWithNoLookaheadToken` — "is the leftmost
/// token an object/function/class, so the expression statement needs parens"). That one
/// recurses to the leaf and stops at IIFE callees/tags; this one takes a single step and
/// asks only whether the child *starts where the parent does*. Merging them would be a
/// behavior change, not a cleanup.
fn left_spine_child<'x>(expr: &'x Expression<'x>) -> Option<&'x Expression<'x>> {
    Some(match expr {
        Expression::MemberExpression(m) => m.object,
        Expression::CallExpression(c) => c.callee,
        Expression::BinaryExpression(b) => b.left,
        Expression::ConditionalExpression(c) => c.test,
        Expression::AssignmentExpression(a) => a.left,
        // A destructuring default's binding (`[/* c */ a = 1]`). Its `left` starts where
        // the pattern does, so without this arm the seam claims the comment here AND again
        // when the left's own `build_expression_doc` runs — printing it TWICE. The object
        // pattern's twin never showed it: its property builder prints a shorthand key
        // directly instead of routing the `AssignmentPattern` through the seam.
        Expression::AssignmentPattern(a) => a.left,
        Expression::TaggedTemplateExpression(t) => t.tag,
        Expression::SequenceExpression(s) => s.expressions.first()?,
        Expression::TSNonNullExpression(n) => n.expression,
        Expression::TSAsExpression(a) => a.expression,
        Expression::TSSatisfiesExpression(s) => s.expression,
        Expression::TSInstantiationExpression(i) => i.expression,
        Expression::UpdateExpression(u) if !u.prefix => u.argument,
        _ => return None,
    })
}

/// The [`internal::JsdocCast`] whose comment is the FIRST thing `expr` prints — `expr`
/// itself, or the leftmost leaf reached through [`left_spine_child`].
///
/// A cast is the one owned comment that has to be reached as a **node**: `JsdocCast::span`
/// covers the `(`…`)` only, so the comment sits *outside* it and may be a newline above,
/// where the glued lookup every other owned comment answers through finds nothing.
///
/// And reaching it means walking the spine, because a value's printed content begins at its
/// leftmost leaf: `(x).prop`, `(x) + y`, `(x)(a)`, `(x) ? a : b` all print the cast's comment
/// first. Matching `expr` alone left each of those without the hang the cast's own hardline
/// requires — pass 1 stranded the `(` at the operator's own indent and pass 2 collapsed it, so
/// the authoring had no fixed point at all (`jsdoc_type_cast_spine_own_line`).
///
/// The span-start guard is the one [`Printer::prepend_owned_leading_comment`] makes against
/// this same walker: a child starting *later* than its parent (a `NewExpression` callee, a
/// parenthesized inner) means the parent prints something ahead of it, so nothing below it
/// leads the value.
///
/// ⚠️ The **unguarded** walk. Every caller goes through [`Printer::leading_jsdoc_cast`],
/// which carries the document-level short-circuit; this half exists only so that
/// short-circuit has exactly one spelling.
fn leading_jsdoc_cast_walk<'x>(expr: &'x Expression<'x>) -> Option<&'x internal::JsdocCast<'x>> {
    let start = expr.span().start;
    let mut node = expr;
    loop {
        if let Expression::JsdocCast(cast) = node {
            return Some(cast);
        }
        let child = left_spine_child(node)?;
        if child.span().start != start {
            return None;
        }
        node = child;
    }
}

/// An operator→value gap as the `printAssignment` family resolved it: where the gap starts,
/// and whether prettier's `chooseLayout` fourth disjunct fired over it.
///
/// The two travel together because the verdict is meaningless without the range it was taken
/// over — a second copy of either is a second answer waiting to drift. The HANG reads the
/// verdict ([`RhsCommentInfo::indentable_leads_value`]); the HOIST
/// ([`Printer::hoisted_owned_value_gap_run_opt`]) reads only the start and asks its own
/// licence, which is wider (every multi-line block, not just an indentable one).
///
/// [`RhsCommentInfo::indentable_leads_value`]: crate::printer::expressions::assignment::RhsCommentInfo::indentable_leads_value
#[derive(Clone, Copy)]
pub(crate) struct ValueGap {
    /// Just past the operator (`=` / `:`), where the leading-comment scan begins.
    pub(crate) start: u32,
    /// [`Printer::indentable_block_leads_value`] over this gap, as the caller resolved it.
    pub(crate) indentable_leads_value: bool,
}

impl<'a> Printer<'a> {
    /// The [`internal::JsdocCast`] that leads `value`, or `None` — [`leading_jsdoc_cast_walk`]
    /// behind this document's owned-comment presence flag.
    ///
    /// **The short-circuit belongs here, not at the call sites.** A cast's comment is always
    /// owned ([`internal::JsdocCast`] is minted only for `/** … */ (expr)`), so a document
    /// carrying no owned comment carries no cast and the left-spine walk cannot find one —
    /// which is ~every document. That is a fact about the DOCUMENT, so every caller owes it,
    /// and stating it once here is what makes it unforgettable: the cannot-hang marks below
    /// run the walk once per Svelte braced head, so a caller reaching past this method pays
    /// the spine walk on every `{expr}` in the template. A caller that ALSO tests the flag
    /// does so for its own further work (a position lookup, a comment-body read), not for
    /// this walk.
    fn leading_jsdoc_cast<'x>(
        &self,
        value: &'x Expression<'x>,
    ) -> Option<&'x internal::JsdocCast<'x>> {
        if !self.has_owned_comments {
            return None;
        }
        leading_jsdoc_cast_walk(value)
    }

    /// Record that `value` sits in a gap that answers a leading cast's break by rule and
    /// **cannot hang** — a Svelte braced head (`EmbedContext::jsdoc_cast_cannot_hang`) —
    /// so the cast it LEADS with reflows its comment→`(` break to a space in every
    /// authoring, the own-line hardline arm included
    /// ([`Printer::jsdoc_cast_cannot_hang_target`]).
    ///
    /// The mark lands on the value's [`Self::leading_jsdoc_cast`] — the left-spine walk the TS
    /// hang predicates use — because a braced head's bug is exactly the hang's absence:
    /// wherever a TS value gap would answer the cast's hardline by ending the operator's
    /// line, a braced head has no operator line, so the reflow must reach every cast the
    /// hang would have.
    ///
    /// Called once per expression-root entry (`build_root_expression_doc`, which both the
    /// doc and string entries pass through), before any doc is built; nothing overwrites
    /// it during the walk. An **interior** cannot-hang gap (a computed key) must not use
    /// this — it would clobber the entry mark for every sibling built after it, including
    /// a left-spine cast rebuilt in a later `conditional_group` candidate; those wrap
    /// their build in [`Printer::with_jsdoc_cast_cannot_hang_gap`] instead.
    pub(in crate::printer) fn mark_jsdoc_cast_cannot_hang_gap(&self, value: &Expression<'_>) {
        self.jsdoc_cast_cannot_hang_target
            .set(self.leading_jsdoc_cast(value).map(|cast| cast.span));
    }

    /// Build `value`'s doc under a cannot-hang mark, restoring the previous mark after —
    /// the interior-gap form of [`Printer::mark_jsdoc_cast_cannot_hang_gap`], for the
    /// **computed-key** `[`→key gap (object/class members, both pattern spellings): a `[`
    /// starts the key's line, so there is no operator line to end and a leading cast's
    /// own-line hardline would strand the `(` — the same shape as a Svelte braced head,
    /// answered the same way (reflow; a plain block comment in this gap already reflows).
    /// The save/restore is what keeps an enclosing entry mark alive for subtrees built
    /// after the key.
    pub(in crate::printer) fn with_jsdoc_cast_cannot_hang_gap(
        &self,
        value: &Expression<'_>,
        build: impl FnOnce() -> DocId,
    ) -> DocId {
        let saved = self.jsdoc_cast_cannot_hang_target.get();
        self.jsdoc_cast_cannot_hang_target
            .set(self.leading_jsdoc_cast(value).map(|cast| cast.span));
        let doc = build();
        self.jsdoc_cast_cannot_hang_target.set(saved);
        doc
    }

    /// Whether this cast is the one a cannot-hang gap recorded — asked where the
    /// separator is chosen ([`Printer::build_jsdoc_cast_lead_doc`]), ahead of the own-line
    /// arm. The complement of [`Printer::jsdoc_cast_in_value_gap`], which reflows only
    /// the soft-`line` arm (its gaps CAN hang, so their own-line authoring keeps the
    /// hardline and the enclosing gap supplies the hang).
    pub(in crate::printer) fn jsdoc_cast_in_cannot_hang_gap(
        &self,
        cast: &internal::JsdocCast<'_>,
    ) -> bool {
        self.jsdoc_cast_cannot_hang_target.get() == Some(cast.span)
    }
}

/// Ownership's emitters and lookups — who PRINTS an owned comment, and where its content
/// begins — plus the one LAYOUT question ownership still answers,
/// [`Printer::is_own_line_jsdoc_cast`].
///
/// ⚠️ **The general operator→value hang rule is NOT here, and must not come back.** It is
/// the gap's ([`Printer::indentable_block_leads_value`], prettier's `chooseLayout` fourth
/// disjunct), and it subsumes every owned comment by construction: an owned comment is
/// whitespace-adjacent to the value's first token, so it lies inside that range and the
/// on-page axis counts it. A node-keyed retelling of the rule goes blind to every comment
/// something stands in front of — a discarded grouping paren, a second comment, a cast's
/// retained `(` — which is exactly the bug that retired the one that used to live here.
impl<'a> Printer<'a> {
    /// Build `build()`'s doc with the owned comment at `start` marked as **already claimed
    /// by an enclosing node**, so nothing beginning there claims it again
    /// ([`Printer::claimed_owned_comment_start`]) — then restore the previous mark.
    ///
    /// The one caller is a paren-less arrow's parameter list
    /// ([`Printer::build_arrow_params_doc_ungrouped`]): the arrow's span starts at its sole
    /// parameter, so both nodes answer the position-keyed lookup, and the arrow is the one
    /// that must keep the claim — it prints the synthesized `(` between the comment and the
    /// parameter. A new caller owes the same argument the reassembly seam does, in mirror
    /// image: that some enclosing node provably DOES claim the comment, or the suppression
    /// is a DROP rather than a de-duplication.
    ///
    /// Save/restore rather than set/clear, so an enclosing mark survives — and so nothing
    /// leaks past the parameter into the arrow's body, where a nested arrow's own owned
    /// comment lives ([`Printer::with_jsdoc_cast_cannot_hang_gap`] is the same shape).
    pub(in crate::printer) fn with_owned_comment_claimed_above<T>(
        &self,
        start: u32,
        build: impl FnOnce() -> T,
    ) -> T {
        let saved = self.claimed_owned_comment_start.replace(Some(start));
        let doc = build();
        self.claimed_owned_comment_start.set(saved);
        doc
    }

    /// **Hoist the operator→value leading run OUT of the value's doc**, for the
    /// `printAssignment` family — returning the run and the value's doc built with the
    /// value's own claim suppressed.
    ///
    /// The reason the two are one call: emitting the run without suppressing DOUBLE-PRINTS
    /// the comment, suppressing without emitting DROPS it (`docs/comments.md` hazard 1),
    /// and the obligation `with_owned_comment_claimed_above` states — "some enclosing node
    /// provably DOES claim it" — is discharged here by construction rather than by a
    /// caller remembering to pair them.
    ///
    /// **Why hoist at all.** tsv's ownership is INNERMOST-wins
    /// ([`Self::prepend_owned_leading_comment`]'s `left_spine_child` check), so on
    /// `= /**⏎ * c⏎ */ b + c + d` the comment is claimed by `b` and travels *inside* the
    /// binary group; its reprinted body is a `MultilineText`, which the layout memo answers
    /// `LAYOUT_BREAKS_FORCED`, so the group breaks and a value prettier keeps flat explodes.
    /// A *preserved* multi-line block does the same through a `literalline` interior, which
    /// is why the licence below is the multi-line reading rather than the hang's.
    /// Prettier attaches such a comment to the OUTERMOST node starting there and its
    /// `printComments` wraps the whole group, which is the shape the seam already builds
    /// (`concat([comments_doc, right_doc])`) — the comment just never reached
    /// `comments_doc`, because the seam's run is built on the **to emit** axis and that
    /// axis skips owned comments by definition. Hoisting routes it there.
    ///
    /// The run is taken **on page**, so it carries the owned comment and any un-owned
    /// sibling in one run — prettier prints every leading comment outside the value's
    /// group, so a mixed run (`= /* c1 */ /**⏎ * c2 */ x`) must not be split across the two
    /// emitters. `end` is the value's printed start, and the separator stays
    /// `printLeadingComment`'s: a space where the author glued the value to the `*/`.
    ///
    /// ⚠️ **A JSDoc cast is excluded**: it prints its own copy from the `JsdocCast` node
    /// (`build_jsdoc_cast_lead_doc`), so hoisting would print the comment twice — and its
    /// value needs no hoist anyway, since the cast's retained parens already stand between
    /// the comment and anything breakable.
    ///
    /// ⚠️ Scoped to operator→value gaps, and to the run's multi-line member — NOT a change
    /// to the general ownership rule: everywhere else innermost-wins still holds, which is
    /// what keeps a paren a *parent* synthesizes from landing between a comment and its own
    /// token. The paren-less arrow reached the same place first and by the same means
    /// (`build_arrow_params_doc_ungrouped`), which is why it is the one shape in this
    /// family that was already flat.
    pub(in crate::printer) fn hoist_owned_value_gap_run(
        &self,
        gap_start: u32,
        value: &Expression<'_>,
        build_value: impl FnOnce() -> DocId,
    ) -> (Option<DocId>, DocId) {
        let run = self.hoisted_owned_value_gap_run_opt(gap_start, value);
        (run, self.build_gap_value_doc(run, value, build_value))
    }

    /// The value build of a gap seam whose closure builds the value ALONE: the hoist's
    /// suppression ([`Self::build_value_under_hoist`]) where the hoist fired, and where it
    /// declined, the value's multi-line owned comment claimed outside the value's own group
    /// ([`Self::build_value_with_outermost_owned_comment`]).
    ///
    /// The hoist declines for a run that is not glued through — a run the author broke after
    /// (`= /* c1 */⏎/* c2⏎*/ a ? b : c`), or one under an own-line comment — whose arm prints
    /// the rest of the run on the emit axis. Claimed by the innermost node there, the comment's
    /// hard break explodes a value prettier keeps flat. Under the hoist the claim declines on
    /// its own, since the hoist already claims the comment.
    ///
    /// ⚠️ **Only for a closure that builds the value alone.** The claim prepends after
    /// `build_value` returns, so a closure that also prepends the gap's leading run prints the
    /// owned comment AHEAD of that run — the arrow's `build_body` closures carry it, which is
    /// why the arrow claims beneath the run instead (`Printer::build_arrow_body_doc_with_leading`).
    pub(in crate::printer) fn build_gap_value_doc(
        &self,
        run: Option<DocId>,
        value: &Expression<'_>,
        build_value: impl FnOnce() -> DocId,
    ) -> DocId {
        self.build_value_under_hoist(run, value, || {
            self.build_value_with_outermost_owned_comment(value, build_value)
        })
    }

    /// The **other half of the hoist**: build `value`'s doc with its own claim suppressed
    /// exactly when `run` is `Some` — the hoisted run that will print the comment instead.
    ///
    /// One spelling of the pairing rule, so the three seams that hoist cannot answer it
    /// differently: suppressing where nothing hoisted is a DROP, hoisting without
    /// suppressing is a DOUBLE-PRINT, and both have already happened once each in this
    /// family. [`Self::hoist_owned_value_gap_run`] is the shape for a caller that can hand
    /// its whole value build over as one closure; a cascade that picks among many arms
    /// (the declarator, the object property's own-line arm) takes
    /// [`Self::hoisted_owned_value_gap_run_opt`] and this together.
    pub(in crate::printer) fn build_value_under_hoist(
        &self,
        run: Option<DocId>,
        value: &Expression<'_>,
        build_value: impl FnOnce() -> DocId,
    ) -> DocId {
        match run {
            Some(_) => self.with_owned_comment_claimed_above(value.span().start, build_value),
            None => build_value(),
        }
    }

    /// The run half of [`Self::hoist_owned_value_gap_run`], for a cascade that cannot hand
    /// its value build over as one closure — the declarator and the type alias each pick
    /// among many arms.
    ///
    /// ⚠️ **A caller of this form owes the suppression itself**, through
    /// [`Self::build_value_under_hoist`] — which is the pairing rule's one spelling, so
    /// reach for it rather than calling `with_owned_comment_claimed_above` by hand.
    /// `comments:audit` is the standing guard, but it only sees documents someone wrote.
    ///
    /// ⚠️ **The run this returns REPLACES the seam's own, never joins it.** Both are the
    /// same gap's leading run and this is the wider (on-page) reading of it, so a caller
    /// takes `hoisted.or(its_own)` — concatenating the two prints every un-owned comment in
    /// the gap twice.
    pub(in crate::printer) fn hoisted_owned_value_gap_run_opt(
        &self,
        gap_start: u32,
        value: &Expression<'_>,
    ) -> Option<DocId> {
        // ⚠️ **The licence is the CAUSE — a multi-line block on the page — not prettier's
        // fourth disjunct.** The disjunct (an INDENTABLE block, which hangs the value) was
        // the first spelling, and it named the seams the bug was first seen at rather than
        // the thing that breaks the group: a *preserved* multi-line block reprints through
        // the same `MultilineText` body, answers the layout memo the same
        // `LAYOUT_BREAKS_FORCED`, and explodes the same values — at the same four seams the
        // disjunct already covered, plus every seam that declines the hang. Indentability is
        // a two-or-more-line property (`is_indentable_block_comment`), so this reading
        // strictly SUBSUMES the disjunct's; the hang keeps asking the narrow question
        // ([`Self::value_hangs_under_operator`]), which is why the two cannot be folded.
        //
        // Asked here rather than passed in, so the five seams cannot spell it five ways.
        // The document-level flag pays for it: the hoist exists only for a comment the value
        // OWNS, and a document with none has an emit-axis run identical to this on-page one
        // — the caller's `hoisted.or(its_own)` then prints the same bytes either way.
        if !self.has_owned_comments
            || !self.has_multiline_block_comments_on_page_between(gap_start, value.span().start)
        {
            return None;
        }
        // ⚠️ **Only a run this seam itself prints may be hoisted**, and the test for that
        // is that the WHOLE run is glued through to the value
        // ([`Printer::comment_run_glued_through`]). A gap holding anything the author gave
        // its own line routes to a different emitter — the declarator's broke-after-`=` arm,
        // its line-comment continuation, the property's own-line arm — each of which builds
        // its own run on the emit axis and never reads the hoisted one, so suppressing the
        // value's claim there leaves NOTHING printing the comment: a DROP
        // (`docs/comments.md` hazard 1).
        //
        // ⚠️ It is deliberately NOT `comments_force_own_line_between`, which was the first
        // spelling and is a different question — it asks whether the run forces the VALUE
        // onto its own line, so it answers `false` for a run whose LAST comment is glued
        // even when an earlier one stands on its own line (`= ⏎/* x */⏎⏎/* d1 */ /* d2⏎*/ v`).
        // That gap takes the broke-after arm, and the hoist dropped its owned tail comment.
        // Only `gaps:audit`'s injection found it: the shape needs a comment the author
        // isolated AND a second one glued to the value, which no fixture in the tree spelled
        // and which is invisible to a formatted corpus.
        if !self.comment_run_glued_through(gap_start, value.span().start) {
            return None;
        }
        // ⚠️ The cast test is the LEFT-SPINE one ([`Self::leading_jsdoc_cast`]), never a
        // match on the value node: a cast at the base of a chain
        // (`= /** @type {A} */ (x).y`) is the node the comment binds to, and a node-keyed
        // test sees only the MemberExpression and hoists a run the cast then prints again —
        // a DOUBLE-PRINT the nestled-cast fixture caught. Same reason
        // `indentable_block_leads_value` is a gap question rather than a node one, and the
        // same walk every other cast reading in this file goes through.
        if self.leading_jsdoc_cast(value).is_some() {
            return None;
        }
        self.build_hoisted_value_gap_comments_opt(gap_start, value.span().start)
    }

    /// Build `value`'s doc with a MULTI-LINE comment it OWNS claimed by **this seam**
    /// rather than by the innermost node its token begins — printing it OUTSIDE the value's
    /// own group, where prettier's `printComments` puts it.
    ///
    /// The **no-gap** sibling of [`Self::hoist_owned_value_gap_run`], for a seam that has no
    /// run of its own to replace. Two families reach it, and neither can hoist *from* a
    /// gap — the only comment in play is the one the value owns:
    ///
    /// - a **list member** — a call argument, an array element, an expression statement —
    ///   whose leading run comes from the LIST it sits in, emitted on the **to emit** axis,
    ///   which never sees the owned member at all;
    /// - a **REQUIRED paren pair's operand** — an `as`/`satisfies` operand, an angle-bracket
    ///   assertion operand, a non-null operand, a binary/logical operand, a unary operand,
    ///   an `await` operand, a chain base, a sequence's own operand run, a `for` header's
    ///   sequence clause, and the ASI operand shell ([`Self::build_asi_operand_shell_doc`]).
    ///   Those emitters DO scan the pair's leading gap
    ///   ([`Self::build_required_pair_leading_shell_doc`] and the folded twin), but on the
    ///   **to emit** axis, which skips the owned comment by definition — so their run and
    ///   this claim partition the gap rather than competing for it, and the two print in
    ///   source order (the seam's run first, then the operand's doc, which now opens with
    ///   the comment it owns).
    ///
    /// Either way this claims exactly that one comment, and the seam's own run is untouched
    /// and cannot double-print.
    ///
    /// Why these seams and not `build_expression_doc` generally: innermost-wins is
    /// what keeps a paren a *parent* synthesizes from landing between a comment and its
    /// token, and that property is worth keeping everywhere it is not actively wrong. It is
    /// wrong exactly where the comment's reprinted body force-breaks a group the value would
    /// otherwise keep flat, which is the multi-line case — hence the `multiline` gate — and
    /// only at a seam whose value can carry such a group.
    ///
    /// ⚠️ **A required pair asks this only where the pair is KEPT.** Bare, the comment leads
    /// the *enclosing* construct's value and that seam already decides its position; taking
    /// the claim there would move a comment no pair encloses. The one member of that list
    /// with no pair of its own is the `for` header's sequence clause, which is there for the
    /// BREAK alone: the header strips the author's grouping parens, so the comment simply
    /// leads the clause — where prettier prints it too — and the operand's own group was the
    /// whole of the question.
    ///
    /// ⚠️ **The gap gate is what selects the emitter, so the seam list cannot be read off
    /// `needs_parens`.** An un-owned second comment in the same gap makes it non-empty on
    /// the **to emit** axis, which routes an `as` operand to
    /// [`Self::build_asi_operand_shell_doc`] instead of its plain arm — a different emitter
    /// printing its own pair and its own operand doc. Every single-comment probe cell is
    /// green while that one is unclaimed; a two-comment RUN is what reaches it.
    ///
    /// Declines when nothing below would claim (no left-spine child starts here, so `value`
    /// is already the outermost claimant and `build_expression_doc` prints the comment
    /// outside its group unaided), and at a **JSDoc cast on the left spine**, which prints
    /// its own copy — the same left-spine reading the hoist makes, and for the same reason:
    /// a node-keyed test sees a chain's MemberExpression and misses the cast at its base.
    pub(in crate::printer) fn build_value_with_outermost_owned_comment(
        &self,
        value: &Expression<'_>,
        build_value: impl FnOnce() -> DocId,
    ) -> DocId {
        self.build_doc_with_outermost_owned_comment_at(
            value.span().start,
            left_spine_child(value),
            build_value,
        )
    }

    /// The **span-keyed** form of [`Self::build_value_with_outermost_owned_comment`], for a
    /// seam with no enclosing `Expression` to key the claim on: a **sequence**, which prints
    /// its own paren envelope (and, in a `for` header, no parens at all), so the thing the
    /// claim sits outside is the comma-joined operand RUN rather than an operand node.
    ///
    /// `start` is where the run's first token begins — the sequence's own span start, which
    /// is also its first operand's — and `first_child` the node that would otherwise claim
    /// there. Both gates read the same way as the value form: `first_child` starting LATER
    /// than `start` means the seam prints something ahead of it, so nothing below leads the
    /// run.
    ///
    /// The gates live in one place ([`Self::outermost_owned_claim_applies`]) so the three
    /// forms cannot answer them differently, and the suppression and the prepend are one
    /// call in each — suppressing where nothing prepends is a DROP and
    /// prepending without suppressing a DOUBLE-PRINT (`docs/comments.md` hazard 1), and a
    /// hand-rolled copy of this pairing double-printed every JSDoc cast in a sequence.
    pub(in crate::printer) fn build_doc_with_outermost_owned_comment_at(
        &self,
        start: u32,
        first_child: Option<&Expression<'_>>,
        build: impl FnOnce() -> DocId,
    ) -> DocId {
        if !self.outermost_owned_claim_applies(start, first_child) {
            return build();
        }
        let doc = self.with_owned_comment_claimed_above(start, build);
        self.prepend_owned_leading_comment_at(start, doc)
    }

    /// The **two-body** form, for a pair emitter that takes a FLAT and a BROKEN rendering of
    /// the same operand ([`Self::build_owned_required_pair_doc`]'s chain-base caller).
    ///
    /// ⚠️ **Both bodies need the prepend, and the single-body form cannot give it to them.**
    /// The bodies are two docs, not one — the flat one shaped by the position, the broken one
    /// the operand's plain doc — and a caller that claims around only the shaped one leaves
    /// the other built under the SUPPRESSION with nothing printing the comment: a DROP
    /// (`docs/comments.md` hazard 1). Worse, a memoized `inner()` shared by the two arms
    /// makes the drop invisible from the call site, because the suppressed doc is what the
    /// memo already holds. `gaps:audit` is what found it — the shape needs a `//` in the
    /// pair's TRAILING gap, which is what makes the trailing emitter pick the broken body.
    pub(in crate::printer) fn build_value_pair_with_outermost_owned_comment(
        &self,
        value: &Expression<'_>,
        build: impl FnOnce() -> (DocId, DocId),
    ) -> (DocId, DocId) {
        let start = value.span().start;
        if !self.outermost_owned_claim_applies(start, left_spine_child(value)) {
            return build();
        }
        let (flat, broken) = self.with_owned_comment_claimed_above(start, build);
        (
            self.prepend_owned_leading_comment_at(start, flat),
            self.prepend_owned_leading_comment_at(start, broken),
        )
    }

    /// The four gates of the outermost-owned claim, in one place so the three forms above
    /// cannot answer them differently.
    fn outermost_owned_claim_applies(
        &self,
        start: u32,
        first_child: Option<&Expression<'_>>,
    ) -> bool {
        if !self.has_owned_comments {
            return false;
        }
        // Nothing below claims, so the seam has nothing to take over.
        if first_child.is_none_or(|c| c.span().start != start) {
            return false;
        }
        // An ENCLOSING seam already claims this comment; taking it again double-prints.
        if self.claimed_owned_comment_start.get() == Some(start) {
            return false;
        }
        if !self
            .owned_leading_comment_at(start)
            .is_some_and(|c| c.multiline)
        {
            return false;
        }
        // The left-spine walk, entered at the child rather than at the node above it: both
        // start at `start`, so the two readings are the same one, and the span-keyed form has
        // no node above to enter at.
        first_child.is_none_or(|c| self.leading_jsdoc_cast(c).is_none())
    }

    /// Prepend the comment `expr` owns, glued to its own first token.
    ///
    /// The single seam, called from `build_expression_doc` — so the comment is part of
    /// the node's doc at every one of the ~29 sites where a *parent* decides to wrap that
    /// doc in parens, present or future. Nothing else prints an owned comment.
    pub(crate) fn prepend_owned_leading_comment(&self, expr: &Expression<'_>, doc: DocId) -> DocId {
        // Document-level short-circuit: no comment in this document is owned, so nothing
        // here can prepend one. Skips even the byte gate below for the ~all documents with
        // no owned comment. (Every `JsdocCast` comment is owned, so a cast implies the flag.)
        if !self.has_owned_comments {
            return doc;
        }
        let start = expr.span().start;
        // Cheap byte gate next: this runs once per expression node (the highest-frequency
        // comment path), and almost every expression is mid-line — preceded by `(`/`,`/space,
        // not a block comment's `*/` — so the gate bails in a few instructions before the
        // JsdocCast match and the 13-arm left-spine walk.
        //
        // ⚠️ Deliberately **narrower** than the lookup it guards, which asks
        // `CommentGlue::AnyLine` because a JSDoc cast owns its comment from the line above
        // its `(`. Result-identical here regardless: the own-line spelling this gate rejects
        // is owned by a cast alone, and a cast is excluded two lines down anyway (it prints
        // its own copy). A second AnyLine producer would break that argument — widen the gate
        // with it, don't rediscover this comment.
        if source_scan::block_comment_end_before(
            self.source.as_bytes(),
            start as usize,
            source_scan::CommentGlue::SameLine,
        )
        .is_none()
        {
            return doc;
        }
        // A JSDoc cast holds its own copy of its comment and prints it against its own
        // `(` — see `build_jsdoc_cast_lead_doc`. Claiming it here would print it twice.
        if matches!(expr, Expression::JsdocCast(_)) {
            return doc;
        }
        // A node whose left-spine child starts here is not the innermost — that child is
        // (or something below it). Let the recursion reach it.
        if left_spine_child(expr).is_some_and(|c| c.span().start == start) {
            return doc;
        }
        // The enclosing-claim check is `prepend_owned_leading_comment_at`'s, so the
        // reassembly callers get it too — a frozen slice reached it without one and printed
        // a hoisted comment twice.
        self.prepend_owned_leading_comment_at(start, doc)
    }

    /// [`Self::prepend_owned_leading_comment`] keyed on a span start rather than a node,
    /// for a caller that already knows its node is the left edge.
    ///
    /// **The rule, once:** a builder that **reassembles** a node from its parts instead of
    /// routing it through `build_expression_doc` never runs the seam above, so it must claim
    /// the owned comment here or the comment is *dropped* (`docs/comments.md` hazard 1).
    /// The innermost-node check the seam above makes is skipped because every caller below
    /// already knows its span start is the left edge — **which is a per-caller fact, not a
    /// property of this function**, so a new caller owes that argument:
    ///
    /// - `build_arrow_sig_doc` (`calls/arg_wrapping.rs`) and `build_arrow_chain_doc`
    ///   (`expressions/functions.rs`) reassemble an arrow from signature + body; an arrow is
    ///   always its own left edge.
    /// - `build_export_default_declaration_doc` (`statements/modules/mod.rs`) reassembles
    ///   `export default @dec class {}`; the class expression is the left edge of what
    ///   follows the decorator run.
    /// - `build_assignment_pattern_doc` (`expressions/patterns.rs`) reassembles a
    ///   destructuring default's object-pattern left for the no-expand-on-nesting rule; the
    ///   pattern's `{` is that left's first token, and the `AssignmentPattern` above it hands
    ///   the claim *down* (`left_spine_child`), so nothing else catches it.
    /// - [`Self::build_frozen_node_doc`] and its layout-opaque sibling
    ///   [`Self::build_frozen_opaque_node_doc`] (`ignore.rs`) print a verbatim span; the frozen
    ///   span's start *is* the node's first printed byte by construction. The two differ only
    ///   in whether the slice's multi-line-ness reaches the container, never in the claim —
    ///   which is why the second exists rather than the member-expression freeze open-coding a
    ///   bare slice, as it did until a glued block ahead of a frozen chain base was found
    ///   dropped at six distinct gap positions.
    /// - `build_directive_doc` (`statements/mod.rs`) prints a directive from the literal's
    ///   own source bytes (`format_directive`, an exact code-unit sequence); a directive is a
    ///   bare string literal by grammar, so that span start is the statement's first printed
    ///   byte.
    /// - `build_test_callee_flat_doc` (`calls/test_patterns.rs`) reassembles a test call's
    ///   callee from its dotted parts (`test.describe.only`) for the break-free flat layout;
    ///   the chain's leftmost name is the callee's first token, and the general callee path
    ///   that would have claimed it is exactly what this arm replaces.
    ///
    /// Prefer collapsing a reassembly path onto `build_expression_doc` over adding another
    /// caller, and prefer wrapping the claim in a named builder (as the two frozen builders
    /// do) over open-coding it. ⚠️ **Keep the list above whole and count-free** — it has been
    /// wrong at nearly every size it has been, because a running total is the one part of it
    /// no compiler checks, and each new caller arrived alongside prose still describing the
    /// last one. `grep prepend_owned_leading_comment_at` is the authority; a bullet here is a
    /// per-caller *argument*, and a caller with no bullet is a claim nobody has justified.
    pub(crate) fn prepend_owned_leading_comment_at(&self, start: u32, doc: DocId) -> DocId {
        // Document-level short-circuit (also covers the arrow-reassembly callers, which
        // reach here without going through `prepend_owned_leading_comment`).
        if !self.has_owned_comments {
            return doc;
        }
        // An ENCLOSING node already claims this comment and prints text ahead of us — the
        // synthesized `(` of a paren-less arrow, whose span starts at this very parameter, or
        // the operator→value hoist ([`Self::hoist_owned_value_gap_run`]), which prints the
        // run outside the value's group (`Printer::claimed_owned_comment_start`).
        // Innermost-wins is the rule everywhere else precisely because the innermost node
        // prints first; here it does not.
        //
        // ⚠️ The check lives HERE rather than at the node-keyed seam above, because a
        // **reassembly** caller never runs that seam: a frozen member slice claims through
        // this entry point directly (`build_frozen_opaque_node_doc`), and under a hoist it
        // printed the comment a second time — the mirror image of the DROP the claim exists
        // to prevent (`prettier_ignore_base_comment`'s multi-line block).
        if self.claimed_owned_comment_start.get() == Some(start) {
            return doc;
        }
        let Some(comment) = self.owned_leading_comment_at(start) else {
            return doc;
        };
        let d = self.d();
        // The separator is the author's: a general owned comment is glued on the token's
        // own line, so it is always the space — but a JSDoc cast may own its comment from
        // the line ABOVE its `(`, and collapsing that onto one line is a relocation the
        // unfrozen path does not make (`build_jsdoc_cast_lead_doc` keeps the break on exactly
        // `Self::jsdoc_cast_comment_own_line`'s shape, and so does prettier). Reading it off
        // the source keeps the two producers answering with one rule rather than the
        // claim having to know which bound the comment.
        let separator = if self.comment_has_newline_between(comment.span.end, start) {
            d.hardline()
        } else {
            d.text(" ")
        };
        d.concat(&[self.build_comment_doc(comment), separator, doc])
    }

    /// **on page**: whether the comment leading `value` is a JSDoc cast's that the author gave
    /// a line of its own — the hang **every** value gap owes, and the one comment shape
    /// [`Printer::indentable_block_leads_value`] cannot answer.
    ///
    /// The hang here is **structural, not a layout preference**: the cast prints a
    /// **hardline** between its comment and its `(` on exactly this shape
    /// ([`Self::jsdoc_cast_comment_own_line`], the source of truth for both halves), and a
    /// hardline with no hang leaves that `(` at the binding's own indent — a form the next
    /// pass collapses, so the authoring has no fixed point. That is why the test is the
    /// cast's SHAPE and not its comment's: an own-line cast hangs whether its comment is
    /// indentable or a single line, where the general rule keys on indentability alone.
    ///
    /// ⚠️ **Asked by every value gap, and that uniformity is the point.** The gaps that
    /// build their own layout (a binding default, an enum member) ask it because nothing
    /// else will; the `printAssignment` family asks it *beside* the general rule, which
    /// covers everything else its own reading used to. The families still differ, and the
    /// difference is the general rule's alone: prettier hangs an indentable block at a
    /// declarator and keeps it inline at a binding default and an enum member — where tsv
    /// matches it (`member_init_multiline_block_comment`), because those gaps ask only this.
    /// Every asker reads the same cast off [`Self::leading_jsdoc_cast`], so none can
    /// disagree about WHICH cast leads a value, and all take that lookup's document-level
    /// short-circuit.
    pub(crate) fn is_own_line_jsdoc_cast(&self, value: &Expression<'_>) -> bool {
        self.leading_jsdoc_cast(value)
            .is_some_and(|cast| self.jsdoc_cast_comment_own_line(cast))
    }

    /// **on page**: where `expr`'s printed content begins in source — the start of the owned
    /// comment it prints ahead of its own first token, or `None` when it owns none.
    ///
    /// The counterpart to [`Self::prepend_owned_leading_comment`] for a caller that needs the
    /// comment's *position* rather than its doc. A gap emitter must not ask this — an owned
    /// comment is not the gap's to print. A caller measuring the gap must: the comment travels
    /// inside `expr`'s doc, so a **to emit** lookup reports the gap as empty and any bound taken
    /// from one lands past the comment, on the element's own token. Anything the author wrote
    /// *before* the comment — a blank line, most of all — then falls outside the measured range
    /// and is silently dropped.
    ///
    /// The left-spine walk [`Self::prepend_owned_leading_comment`] makes is irrelevant here:
    /// whichever node on the spine ends up printing the comment, they all start where `expr`
    /// does, so the position is the same.
    pub(crate) fn owned_leading_comment_start(&self, expr: &Expression<'_>) -> Option<u32> {
        // Document-level short-circuit: no owned comment anywhere ⇒ no owned start
        // (and no `JsdocCast` exists, since a cast's comment is always owned).
        if !self.has_owned_comments {
            return None;
        }
        // A JSDoc cast carries its own copy and always prints it (`build_jsdoc_cast_lead_doc`), so
        // it is the one node that answers from the node rather than the position lookup.
        // It must: `JsdocCast::span` covers the `(`…`)` only — the comment sits *outside* it —
        // so the lookup below can only ever find the cast's comment when the cast is `expr`'s
        // own left edge, and it is asked at a POSITION, which cannot walk.
        // Resolved down the left spine ([`Self::leading_jsdoc_cast`]): the comment leads the value
        // from its leftmost leaf too, and a bound taken past it drops an authored blank line
        // (`[a,⏎⏎/** @type {A} */⏎(x).b]`) — the loss this function exists to prevent.
        if let Some(cast) = self.leading_jsdoc_cast(expr) {
            return Some(self.jsdoc_cast_comment(cast).span.start);
        }
        self.owned_leading_comment_at(expr.span().start)
            .map(|c| c.span.start)
    }

    /// The owned comment ending immediately before `start` —
    /// [`tsv_lang::owned_leading_comment_at`] against this document.
    ///
    /// Whitespace-adjacent, not same-line-glued: ownership's two producers use two glues,
    /// so the lookup takes their union and `owned_by_node` decides (see that function). A
    /// JSDoc cast's comment therefore IS found here even from the line above its `(` — the
    /// three callers that must not print it twice each resolve the cast off the node first
    /// ([`Self::leading_jsdoc_cast`], which also walks the left spine this position cannot).
    pub(crate) fn owned_leading_comment_at(&self, start: u32) -> Option<&'a internal::Comment> {
        tsv_lang::owned_leading_comment_at(self.source, self.comments, start)
    }

    /// A JSDoc cast's owned comment **as this printer's array holds it**.
    ///
    /// `JsdocCast` is the one node carrying a `Comment` copy of its own, taken at parse.
    /// The format path's comment view merges a nestled pair into one entry
    /// (`tsv_lang::merge_nestled_block_comments`), and a cast's comment can be the pair's
    /// TAIL — the merge is keyed on `*/`→`/*` adjacency and the cast on what precedes its
    /// `(`, so `/** a⏎ *//** @type {T}⏎ */ (x)` satisfies both. The copy is then only half
    /// of what the array holds, and printing it **drops** the predecessor: the merged entry
    /// is `owned_by_node`, so no gap emitter will claim it (hazard 1).
    ///
    /// The lookup is the ordinary ownership one at the cast's `(` — a merge only ever
    /// extends an entry to the LEFT, so the array entry still ends where the copy does and
    /// [`Self::owned_leading_comment_at`] finds it from the same position that bound it.
    /// Unconditional rather than gated on a document flag: a cast is rare, and a gate that
    /// can be forgotten at a fourth read site is worth less than the two compares it saves.
    pub(crate) fn jsdoc_cast_comment(&self, cast: &internal::JsdocCast<'_>) -> internal::Comment {
        self.owned_leading_comment_at(cast.span.start)
            .copied()
            .unwrap_or(cast.comment)
    }

    /// Whether the author gave a JSDoc cast's comment a line of its own — a newline on
    /// **both** sides of it, as in `const a =⏎\t/** @type {A} */⏎\t(expr)`.
    ///
    /// Both sides is the rule prettier applies, and only that shape hangs. A newline on
    /// one side alone collapses to a space:
    ///
    /// ```js
    /// const a = /** @type {A} */⏎  (expr);  // →  const a = /** @type {A} */ (expr);
    /// const a =⏎  /** @type {A} */ (expr);  // →  const a = /** @type {A} */ (expr);
    /// ```
    ///
    /// The single source of truth for both consequences of that shape: the hang itself
    /// (`expressions::assignment::choose_layout`, and the declarator's own predicates in
    /// `statements/variable.rs`) and the **hardline** the cast prints between the comment and
    /// its `(` (`build_jsdoc_cast_lead_doc`). They must agree — a hang without the hardline
    /// leaves the `(` stranded, and a hardline without the hang un-indents it. A gap that
    /// CANNOT hang at all (a Svelte braced head, a computed key —
    /// [`Printer::jsdoc_cast_cannot_hang_target`]) opts out of both halves together: the
    /// cast reflows to a space there without consulting this predicate, which is the only
    /// way the two can still agree where no operator line exists to end.
    ///
    /// ⚠️ **This answers only the HANG, not "is there a separator".** The cast is the last
    /// comment of whatever leading run precedes it, so when this returns `false` the gap is
    /// still prettier's `printLeadingComment` question — a space when something follows the
    /// `*/` on its line, otherwise the soft `line` whose fate the enclosing group decides.
    /// Reading `false` as "space" collapsed a break the author left after the `*/`
    /// (`a();⏎/* c */ /** @type {A} */⏎(b);`) at statement position, where the list keeps
    /// lines and every *other* leading comment — a plain glued run, a bundler annotation —
    /// kept it. `build_jsdoc_cast_lead_doc` owns that three-way split.
    ///
    /// ⚠️ **The soft `line` arm is scoped to gaps that are not VALUE gaps**, because a value
    /// gap answers the break with a rule rather than with width: an unforced break there
    /// reflows (`docs/conformance_prettier.md` §Authored breaks in value position),
    /// so the separator is a space and the two authorings reach one fixed point. Letting the
    /// soft `line` decide it instead put the `(` at the statement's own indent whenever the
    /// enclosing group broke — a break with no hang, which is the second failure this doc
    /// names, reached from the other side. `Printer::jsdoc_cast_value_gap_target` is how the
    /// value gap says so.
    ///
    /// ⚠️ **It reads the ARRAY's comment, not the node's copy** ([`Self::jsdoc_cast_comment`]):
    /// a nestled predecessor merges into the cast's comment, and the line the run opens is
    /// then the predecessor's. The resolution lives inside this predicate rather than at its
    /// caller so the two cannot part.
    pub(crate) fn jsdoc_cast_comment_own_line(&self, cast: &internal::JsdocCast<'_>) -> bool {
        let comment = self.jsdoc_cast_comment(cast);
        let bytes = self.source.as_bytes();
        // Only whitespace between the start of the line and the comment.
        let mut i = comment.span.start as usize;
        let newline_before = loop {
            if i == 0 {
                break true;
            }
            i -= 1;
            match bytes[i] {
                b'\n' => break true,
                b' ' | b'\t' | b'\r' => {}
                _ => break false,
            }
        };
        newline_before
            && !tsv_lang::printing::is_same_line(self.source, comment.span.end, cast.span.start)
    }
}
