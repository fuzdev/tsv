// Composite type printing for TypeScript
//
// Handles:
// - Conditional types: `T extends U ? A : B`
// - Mapped types: `{ [K in T]: V }`
// - Tuple types: `[A, B, C]`
// - Array types: `T[]`
// - Type queries: `typeof x`
// - Entity names: `A.B.C`

use super::super::comments_to_emit_in_range;
use super::helpers::{
    type_needs_parens_for_array_element, type_needs_parens_for_conditional_check,
    type_needs_parens_for_conditional_extends, unwrap_parenthesized,
};
use super::{BlankRule, CommentFilter, CommentSpacing, KeywordValueHead, Printer, TrailingBlock};
use crate::ast::internal::{
    self, TSArrayType, TSConditionalType, TSMappedType, TSMappedTypeModifier, TSTupleType, TSType,
};
use crate::printer::CommentVec;
use crate::printer::layout::{bracketed_list_body, hang_after_operator};
use smallvec::smallvec;
use tsv_lang::Comment;
use tsv_lang::INDENT;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::{find_char_skipping_comments, has_newline_after_position};

/// The **effective** span of a tuple element — the node left once the element's redundant
/// paren shell is stripped ([`unwrap_parenthesized`]), which is the node
/// [`Printer::build_tuple_type_doc_with_line_comments`] emits.
///
/// Shared by that builder and by [`Printer::build_tuple_type_doc`]'s expansion gate so the
/// two cannot disagree about where an element starts. The width path below that gate
/// deliberately does NOT use it — see the ⚠️ on `build_tuple_type_doc`.
///
/// ⚠️ One shell deep, at the element's own top. A shell **nested inside** the element is
/// stripped by that inner node's printer instead, so a comment the author wrote in it still
/// falls inside this span and the tuple's own gaps never see it — an array-type element
/// (`[a, (⏎/* c */⏎number)[]]`) and a rest element (`[a, ...(⏎/* c */⏎T)]`) both collapse
/// inline where their bare authoring expands, and prettier expands both.
/// TODO: reach those by asking each element printer for the span it will emit from, rather
/// than peeling only the top shell here.
fn tuple_elem_span(ty: &TSType<'_>) -> Span {
    unwrap_parenthesized(ty).span()
}

/// How an array type renders its `[]` suffix — the verdict
/// [`Printer::array_suffix_layout`] resolves once for both the emitter and the
/// type-alias `=` gate.
#[derive(Clone, Copy)]
pub(in crate::printer) enum ArraySuffixLayout {
    /// One unsplit `[]` `text()` — the comment-free case.
    Fused,
    /// The `elementType → ]` region holds a comment, so the suffix splits at this
    /// `[`: the gap emits its comments and the bracket pair goes through the shared
    /// empty-brackets emitter.
    Split { bracket_open: u32 },
}

/// Whether each conditional branch gap holds an alone-on-line format-ignore directive
/// — the ROUTING verdicts, resolved by `build_conditional_type_doc`'s break gate (the
/// directive forces the broken layout) and handed to the broken builder rather than
/// re-derived there, so the gate and the emission can never disagree about a branch.
#[derive(Clone, Copy)]
struct BranchRoutes {
    /// The `extends`-type→true-branch gap (the `?` inside the window).
    true_route: bool,
    /// The true-branch→false-branch gap (the `:` inside the window).
    false_route: bool,
}

impl BranchRoutes {
    /// Whether either branch is routed — the break-gate term.
    fn any(self) -> bool {
        self.true_route || self.false_route
    }
}

impl<'a> Printer<'a> {
    //
    // Conditional Types
    //

    /// Build doc for conditional type WITHOUT the outer group wrapper.
    /// This is used for nested conditionals which should inherit breaking from their parent.
    ///
    /// Structure: `check extends extends_type [indent: line, "? ", true_type, line, ": ", false_type]`
    pub(super) fn build_conditional_type_doc_inner(&self, c: &TSConditionalType<'_>) -> DocId {
        let d = self.d();

        let extends_type_end = c.extends_type.span().end;
        let true_type_start = c.true_type.span().start;
        let true_type_end = c.true_type.span().end;
        let false_type_start = c.false_type.span().start;

        // Find ? and : token positions for comment categorization. These positions only
        // bound the comment scans below, so a conditional type with no comment anywhere in
        // the extends→false-branch span skips both position scans — `None` collapses to the
        // same empty comment docs a comment-free `Some` would (the arm builder emits nothing
        // for `None`, and every `needs_breaking` term that consults them scans a comment-free
        // sub-range either way). Paren-leading-line-comment terms below stay independent.
        let (question_pos, colon_pos) =
            if self.has_comments_to_emit_between(extends_type_end, false_type_start) {
                (
                    self.find_char_outside_comments(extends_type_end, true_type_start, b'?'),
                    self.find_char_outside_comments(true_type_end, false_type_start, b':'),
                )
            } else {
                (None, None)
            };

        // Check for comments that force breaking layout.
        // Line comments anywhere in the conditional force breaking (they end the line).
        // A multiline block between extends_type and ? forces breaking; a single-line
        // block there rides the flat path and trails the extends type.
        // Block comments between true_type and : are trailing (don't force breaking).
        // Also: leading line comments inside stripped parens around extends_type
        // (e.g., `a extends (// c\n  b)`, or the double-nested `((// c\n  b))`) —
        // these are relocated to trail extends_type and force breaking. The deep
        // predicate scans the whole stripped shell, not just the outer paren's own
        // gap, so a comment hiding one layer in still forces the break.
        let extends_paren_has_leading_line_comment =
            self.stripped_paren_has_leading_line_comment(c.extends_type);
        // Same for true_type / false_type: leading line comments inside their
        // parens get relocated to trail extends_type / true_type respectively.
        let true_paren_has_leading_line_comment =
            self.stripped_paren_has_leading_line_comment(c.true_type);
        let false_paren_has_leading_line_comment =
            self.stripped_paren_has_leading_line_comment(c.false_type);
        // In the `?`/`:`→branch gaps, only a comment that HANGS its branch breaks the
        // layout: a line comment, or a multiline block the author broke after
        // (`comments_force_own_line_between` — the shared keyword→value gate). A glued
        // multiline block stays inline (`? /* …⏎… */ B`, the way prettier keeps it),
        // and a single-line block in any position rides the inline path, whose
        // branch-gap run preserves an authored break after the comment as a
        // collapsible line (`build_branch_comment_run`). Before `?`, any multiline
        // block still breaks (it trails the extends-type, a different gap).
        let has_breaking_comments_around_question = self
            .has_line_comments_between(extends_type_end, true_type_start)
            || match question_pos {
                Some(q) => {
                    self.has_multiline_block_comments_on_page_between(extends_type_end, q)
                        || self.comments_force_own_line_between(q + 1, true_type_start)
                }
                None => self.has_multiline_block_comments_on_page_between(
                    extends_type_end,
                    true_type_start,
                ),
            }
            || extends_paren_has_leading_line_comment
            || true_paren_has_leading_line_comment;
        let colon_end = colon_pos.map_or(true_type_end, |c| c + 1);
        // `comments_force_own_line_between` also covers line comments (`!is_block`
        // hangs), so no separate line-comment scan is needed for this gap.
        let has_breaking_comments_after_colon = self
            .comments_force_own_line_between(colon_end, false_type_start)
            || false_paren_has_leading_line_comment;
        // Trailing line comments on true_type also force breaking (they end the line)
        let has_trailing_line_comment_on_true =
            colon_pos.is_some_and(|c| self.has_line_comments_between(true_type_end, c));

        // An alone-on-line format-ignore directive in either branch gap (previous
        // node's span end → branch span start, the `?`/`:` inside the window) forces
        // the broken layout: the own-line-preserving emission + freeze live there,
        // and the flat path's trailing emitters would relocate the directive to an
        // inert placement. A block-spelling directive is the only shape the terms
        // above miss (a line directive already breaks as a line comment). Resolved
        // once and handed to the broken builder, which routes each branch on it.
        let routes = BranchRoutes {
            true_route: self.member_gap_frozen(extends_type_end, true_type_start),
            false_route: self.member_gap_frozen(true_type_end, false_type_start),
        };

        let needs_breaking = has_breaking_comments_around_question
            || has_breaking_comments_after_colon
            || has_trailing_line_comment_on_true
            || routes.any();

        if needs_breaking {
            return self.build_conditional_type_doc_with_line_comments(c, routes);
        }

        // Branch docs, built lazily inside the arms: a union / intersection branch
        // rebuilds its doc from parts and never consults the one built here (see
        // `build_conditional_branch_tail_doc`).
        //
        // true_type: if it's a conditional (possibly wrapped in parens), don't wrap in group.
        // Add parens for readability only when flat (single-line), not when broken (multi-line).
        let true_type_doc = || {
            if let TSType::Conditional(inner) = unwrap_parenthesized(c.true_type) {
                // Nested conditional in true position:
                // - Flat: add parens for readability: `T extends A ? (T extends B ? C : D) : E`
                // - Broken: no parens (the line breaks provide clarity)
                let inner_doc = self.build_conditional_type_doc_inner(inner);
                if d.will_break(inner_doc) {
                    // Inner doc forces breaking — use broken layout directly
                    inner_doc
                } else {
                    d.if_break(inner_doc, d.parens(inner_doc))
                }
            } else {
                self.build_type_doc(c.true_type)
            }
        };

        // false_type: if it's a conditional, don't wrap in group.
        // No parens needed for nested conditionals in false position (right-associative).
        let false_type_doc = || {
            if let TSType::Conditional(inner) = unwrap_parenthesized(c.false_type) {
                self.build_conditional_type_doc_inner(inner)
            } else {
                self.build_type_doc(c.false_type)
            }
        };

        // Comments trailing on extends_type (between extends_type and ?). The mirror
        // of `trailing_on_true` below: this path assembles the conditional from its
        // parts, so it must claim every gap on its own seam (`docs/comments.md`
        // hazard 1). Only single-line blocks reach here — anything that hangs a
        // branch took the breaking path above.
        let trailing_on_extends = if let Some(q) = question_pos {
            self.build_inline_comments_between_doc(extends_type_end, q)
        } else {
            d.empty()
        };

        // Comments trailing on true_type (between true_type and :)
        // These stay with the true branch, preserving user intent. Kept separate from
        // `trailing_on_extends` above rather than folded into a shared helper: the
        // expression printer's parallel pair differs (its `?` arm falls back to a
        // scan when no `?` is located), so a common helper would either drop that
        // fallback or grow a parameter for it. Each seam is canary-covered on its own.
        let trailing_on_true = if let Some(c) = colon_pos {
            self.build_inline_comments_between_doc(true_type_end, c)
        } else {
            d.empty()
        };

        // Build extends_type doc - unions need special handling to avoid trailing space
        // after "extends" when the union breaks (e.g., `T extends\n\t| A\n\t| B`)
        // Comments around `extends`: `check /* c1 */ extends /* c2 */ extends_type`
        let check_type_end = c.check_type.span().end;
        let extends_type_start = c.extends_type.span().start;
        let extends_kw_start = find_char_skipping_comments(
            self.source.as_bytes(),
            check_type_end as usize,
            extends_type_start as usize,
            b'e',
        );
        let extends_kw_start = extends_kw_start.map_or(check_type_end, |p| p as u32);
        let extends_kw_end = extends_kw_start + "extends".len() as u32;
        let comments_before_extends =
            self.build_comments_between(check_type_end, extends_kw_start, CommentSpacing::Leading);
        let extends_type_doc = self.build_conditional_type_extends_doc(c, extends_kw_end);

        let true_arm = self.build_conditional_arm_doc(
            "?",
            c.true_type,
            true_type_doc,
            question_pos,
            true_type_start,
        );
        let false_arm = self.build_conditional_arm_doc(
            ":",
            c.false_type,
            false_type_doc,
            colon_pos,
            false_type_start,
        );

        d.concat(&[
            self.build_conditional_check_doc(c.check_type),
            comments_before_extends,
            d.text(" extends"),
            extends_type_doc,
            trailing_on_extends,
            d.indent(d.concat(&[d.line(), true_arm, trailing_on_true, d.line(), false_arm])),
        ])
    }

    /// Build the conditional check-type doc. A redundant-paren-stripped union or
    /// intersection check uses the hanging layout Prettier applies via
    /// `printTernaryTest` + `shouldIndentUnionType`: a (non-hug) union breaks
    /// after the keyword and indents its leading-pipe members one level
    /// (`group(indent([softline, …]))`), while an intersection keeps its first
    /// member inline and wraps continuations one level
    /// (`intersection_hanging_with_indent`). Every other check keeps the inline
    /// `build_type_doc_maybe_parens` form (which still parenthesizes
    /// function/constructor/nested-conditional checks). Redundant comment-free
    /// parens are stripped via the shared `unwrap_redundant_parens`.
    fn build_conditional_check_doc(&self, check: &TSType<'_>) -> DocId {
        let d = self.d();
        match self.unwrap_redundant_parens(check) {
            // `union_prints_hugged`, not the bare syntactic `union_hug_shape`: a comment
            // that makes the printer expand the members must make this gate hang too, or
            // `extends` keeps its operand glued while they explode below it.
            TSType::Union(u) if !self.union_prints_hugged(u) => {
                let union_doc = self.build_union_type_doc(u);
                d.group(d.indent(d.concat(&[d.softline(), union_doc])))
            }
            TSType::Intersection(i) => self.intersection_hanging_with_indent(i),
            _ => self.build_type_doc_maybe_parens(check, type_needs_parens_for_conditional_check),
        }
    }

    /// Assemble one conditional arm in the non-breaking layout: `?`/`:`, any block
    /// comments between the operator and the branch (the only comment kind that
    /// reaches this path — they trail the operator, `? /* c */ …`), then the
    /// branch tail. The comments form a branch-gap run
    /// (`build_branch_comment_run`): a glued comment keeps its branch on the
    /// line, an authored break after a comment becomes a collapsible line that
    /// holds while the conditional is broken.
    fn build_conditional_arm_doc(
        &self,
        op: &'static str,
        branch_type: &TSType<'_>,
        branch_doc: impl FnOnce() -> DocId,
        op_pos: Option<u32>,
        branch_start: u32,
    ) -> DocId {
        let d = self.d();
        let run = op_pos.and_then(|p| {
            // A nested-conditional branch levels itself (see the Conditional arm of
            // the tail), so its soft separator shifts only the first line
            // (`indent(line)`); every other branch nests its run inside the branch's
            // structural indent with a bare `line`. Built inside the closure so the
            // comment-free path allocates nothing.
            let soft_sep = if matches!(
                self.unwrap_redundant_parens(branch_type),
                TSType::Conditional(_)
            ) {
                d.indent(d.line())
            } else {
                d.line()
            };
            self.build_branch_comment_run(p + 1, branch_start, soft_sep)
        });
        d.concat(&[
            d.text(op),
            self.build_conditional_branch_tail_doc(branch_type, branch_doc, false, run),
        ])
    }

    /// The branch tail of a conditional arm: the separator after `?`/`:` (and
    /// any comments already emitted by the caller) plus the branch itself.
    /// Matches Prettier's `printBranch` = `indent(print(branch))` layered over
    /// the arm `indent`:
    /// - A **union** branch puts its leading-pipe members ONE level past the
    ///   operator, with the first member glued to `? `/`: ` — Prettier 3.9's
    ///   "remove extra indention for union type in conditional type" (#18827):
    ///   `shouldIndentUnionType` is false for a conditional branch, so
    ///   `printUnionType` returns the bare `printed = group(members)` and only the
    ///   `printBranch` indent applies (pre-3.9 added a second `indent([line, …])`,
    ///   dropping the operator onto its own line with members two levels in).
    /// - An **intersection** branch keeps its first member on the operator's line
    ///   with continuations two levels in (unchanged).
    /// - Every other branch stays inline after the separator.
    ///
    /// `on_new_line` means a line or multiline block comment ended the operator's
    /// line (breaking layout only), so the branch starts on a fresh line instead —
    /// one level in (the first union member then taking its leading `| `).
    ///
    /// `run` is the operator's branch-gap comment run (inline layout only —
    /// mutually exclusive with `on_new_line`), separators included; it rides
    /// inside the branch's structural indent so a comment's collapsible break
    /// lands the branch one level past the operator, except for a
    /// nested-conditional branch, which levels itself (the run's own
    /// `indent(line)` separator shifts only its first line).
    ///
    /// `branch_doc` is a THUNK because the union and intersection arms rebuild the
    /// branch from parts and never consult it — building eagerly left a dead subtree in
    /// the arena for every such branch (build-once-reuse).
    fn build_conditional_branch_tail_doc(
        &self,
        branch_type: &TSType<'_>,
        branch_doc: impl FnOnce() -> DocId,
        on_new_line: bool,
        run: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        debug_assert!(
            !(on_new_line && run.is_some()),
            "the breaking layout emits its own comments"
        );
        // Union and intersection branches share one hang: the inner doc sits one
        // level past the operator (`indent`) — on a fresh line after an
        // operator-line comment (`on_new_line`, first union member then taking its
        // leading `| `), or glued after the operator's space.
        let hang = |inner: DocId| {
            if on_new_line {
                d.indent(d.concat(&[d.hardline(), inner]))
            } else {
                d.concat(&[d.text(" "), d.indent(self.prepend_opt(run, inner))])
            }
        };
        match self.unwrap_redundant_parens(branch_type) {
            // `union_prints_hugged`, not the bare syntactic `union_hug_shape` — see
            // `build_conditional_check_doc`; here a bare ask left the members one indent
            // level short of prettier's.
            TSType::Union(u) if !self.union_prints_hugged(u) => {
                // `build_union_type_doc` already returns `group(members)` (the bare
                // `printed`); the branch supplies only one `indent`, so the member
                // group breaks its continuations one level past the operator.
                hang(self.build_union_type_doc(u))
            }
            TSType::Intersection(i) => hang(self.intersection_hanging_with_indent(i)),
            // A nested conditional branch is already leveled by its OWN always-on
            // `indent(parts)` (the `d.indent` at the tail of
            // `build_conditional_type_doc_inner`), so it must NOT also take the
            // per-branch indent below. This mirrors Prettier's `forceNoIndent`:
            // a conditional in true/false position drops its own `indent(parts)`
            // (`forceNoIndent ? parts : indent(parts)`) precisely so the outer
            // `printBranch` indent lands it one level in — tsv reaches the same
            // one-level-per-nesting result from the other side (parts always
            // indented, branch never), so adding an indent here would double it.
            // Guarded by `conditional/branch_nested_chain`.
            TSType::Conditional(_) => {
                let branch_doc = branch_doc();
                if on_new_line {
                    d.concat(&[d.hardline(), d.text(INDENT), branch_doc])
                } else {
                    d.concat(&[d.text(" "), self.prepend_opt(run, branch_doc)])
                }
            }
            _ => {
                let branch_doc = branch_doc();
                if on_new_line {
                    // Literal tab text (not d.indent) shifts only the first line
                    // without increasing the structural indent level for nested
                    // content.
                    d.concat(&[d.hardline(), d.text(INDENT), branch_doc])
                } else {
                    // Prettier's `printBranch` = `indent(print(branch))` under
                    // useTabs: every non-conditional branch (tuple, mapped, object,
                    // function/constructor type, …) sits one level past the operator.
                    d.concat(&[d.text(" "), d.indent(self.prepend_opt(run, branch_doc))])
                }
            }
        }
    }

    /// Build the extends clause doc for a conditional type, including comments
    /// between the `extends` keyword and the extends_type.
    /// Comments before `extends` are handled by the caller.
    /// `extends_kw_end` is the position after the `extends` keyword (caller already found it).
    fn build_conditional_type_extends_doc(
        &self,
        c: &TSConditionalType<'_>,
        extends_kw_end: u32,
    ) -> DocId {
        let d = self.d();
        let extends_type_start = c.extends_type.span().start;

        // A comment that can't share the `extends` line — a line comment or a
        // multiline block — stays with `extends`, the extends-type hanging on the next
        // line indented one level (the shared keyword→value layout), forcing the
        // conditional to break. A single-line block comment (own-line, trailing, or
        // glued) collapses inline (`extends /* c */ Y`, the fall-through below);
        // prettier relocates the collapsed comment before `extends`. See
        // check_extends_line_comment / extends_own_line_block_comment.
        if self.comments_force_own_line_between(extends_kw_end, extends_type_start) {
            // An alone-on-line format-ignore directive in the `extends`→type gap
            // freezes a non-composite extends type verbatim (`single_child_frozen`;
            // a union/intersection extends type declines and freezes via its own
            // leading-run walk). This emitter already keeps the directive own-line
            // (a keyword-trailing placement is inert, so the relocated form would
            // lose the freeze on the second pass). The required-paren rule matches
            // the unfrozen arm (`type_needs_parens_for_conditional_extends`); the
            // head builder carries the multi-line must-break.
            let value_doc = if self.single_child_frozen(extends_kw_end, c.extends_type) {
                self.build_frozen_head_doc(
                    c.extends_type,
                    type_needs_parens_for_conditional_extends,
                )
            } else {
                self.build_type_doc_maybe_parens(
                    c.extends_type,
                    type_needs_parens_for_conditional_extends,
                )
            };
            let mut parts = smallvec![];
            self.append_keyword_value_line_comments(
                &mut parts,
                extends_kw_end,
                extends_type_start,
                value_doc,
            );
            return d.concat(&parts);
        }

        // Comments between `extends` keyword and extends_type
        let comments_after_extends = self.build_comments_between(
            extends_kw_end,
            extends_type_start,
            CommentSpacing::Trailing,
        );

        // Special case: parenthesized extends_type with a **pure** leading line-comment
        // run inside the parens (`extends (// c\n  b)`, or the double-nested
        // `((// c\n  b))`). Strip EVERY redundant layer, build the fully-unwrapped
        // inner type, and append the line comment(s) as trailing on it — matching
        // prettier's relocation. The deep window catches a comment hiding one layer
        // in, which the shallow paren-own-gap window missed (non-idempotent). This
        // pure-line trail-on-inner is the conditional-extends' own canonical, pinned by
        // the non-divergence `extends_paren_leading_line_comment`; a mixed/trailing shell
        // declines it (below).
        if self.stripped_paren_has_leading_line_comment(c.extends_type) {
            let inner = unwrap_parenthesized(c.extends_type);
            let mut parts: DocBuf = smallvec![d.text(" "), comments_after_extends];
            parts.push(self.build_type_doc(inner));
            for comment in self.stripped_paren_leading_line_comments(c.extends_type) {
                parts.push(self.build_trailing_line_comment_doc(comment));
            }
            return d.concat(&parts);
        }

        // A MIXED (`(/* b */ // c\n B)`) or TRAILING (`(// c\n B /* t */)`) paren shell
        // carries a leading line comment alongside a leading block and/or a trailing
        // comment, so it declines the narrow trail-on-inner above — trailing a leading
        // block would move it from leading to trailing, and the trailing case would
        // reorder (a `//` must end its line). Instead it HANGS the unwrapped inner at the
        // same fixed point the bare (paren-free) authoring settles on — the shared
        // keyword→value seam (mirroring the prefix-operator site). `value_hang_start !=`
        // the shell start means the wide seam stripped it; pure-line already returned
        // above, so only mixed/trailing reach here.
        let (value_hang_start, value_hang_type) =
            self.keyword_value_stripped_paren_hang(c.extends_type);
        if value_hang_start != extends_type_start
            && self.comments_force_own_line_between(extends_kw_end, value_hang_start)
        {
            // A re-added extends-type paren carries the conditional check/extends indent
            // depth (`build_type_doc_maybe_parens`), matching this builder's other arms —
            // not the prefix operator's bare `d.parens`. Type position, so a trailing block
            // lifted from the shell trails the inner inline.
            let value_doc = self.with_stripped_paren_trailing(
                self.build_type_doc_maybe_parens(
                    value_hang_type,
                    type_needs_parens_for_conditional_extends,
                ),
                c.extends_type,
                value_hang_type,
                TrailingBlock::Inline,
            );
            let mut parts: DocBuf = smallvec![];
            self.append_keyword_value_line_comments(
                &mut parts,
                extends_kw_end,
                value_hang_start,
                value_doc,
            );
            return d.concat(&parts);
        }

        if let TSType::Union(union) = c.extends_type {
            if union.types.is_empty() {
                d.text(" ")
            } else {
                // Extends-type union: `shouldIndentUnionType` is true (extendsType
                // is not in the false list), so Prettier wraps the bare
                // `printed = group(members)` in `group(indent([softline, printed]))`
                // — break after `extends` onto an indented continuation line where
                // the member group re-fits before exploding to leading-pipe members
                // (Prettier 3.9 #18827). `build_union_type_doc` supplies the inner
                // `group(members)` (with the per-member offset and member-paren rules
                // the old hand-rolled loop lacked); the `softline` after the `text(" ")`
                // keeps a single space when flat (the loop double-spaced `extends  A`).
                let union_doc = self.build_union_type_doc(union);
                d.concat(&[
                    d.text(" "),
                    comments_after_extends,
                    d.group(d.indent(d.concat(&[d.softline(), union_doc]))),
                ])
            }
        } else {
            d.concat(&[
                d.text(" "),
                comments_after_extends,
                self.build_type_doc_maybe_parens(
                    c.extends_type,
                    type_needs_parens_for_conditional_extends,
                ),
            ])
        }
    }

    /// Emit the comments in a conditional-type branch gap — between `?` and the
    /// true branch, or between `:` and the false branch — into `parts`, returning
    /// whether the branch type must itself drop to its own indented line.
    ///
    /// The first comment trails the operator (` // c`); a line comment ends its
    /// line, so each subsequent comment drops to its own indented line rather than
    /// merging onto the operator's line (`// c1 // c2` would reparse as a single
    /// comment — a boundary loss). A single-line block stays inline (in-place
    /// collapse). A line comment or a multiline block forces the branch onto its
    /// own line (`needs_indent`). The `?`- and `:`-branch loops share this so they
    /// can't drift.
    fn push_conditional_branch_gap_comments(&self, parts: &mut DocBuf, from: u32, to: u32) -> bool {
        let d = self.d();
        let mut needs_indent = false;
        let mut prev_was_line_comment = false;
        for comment in comments_to_emit_in_range(self.comments, from, to) {
            if prev_was_line_comment {
                parts.push(d.hardline());
                parts.push(d.text(INDENT));
            } else {
                parts.push(d.text(" "));
            }
            parts.push(self.build_comment_doc(comment));
            if !comment.is_block || comment.multiline {
                needs_indent = true;
            }
            prev_was_line_comment = !comment.is_block;
        }
        needs_indent
    }

    /// Build conditional type doc when comments force a breaking layout.
    /// This handles: line comments, multiline block comments, and comments
    /// before `?` or `:` operators.
    fn build_conditional_type_doc_with_line_comments(
        &self,
        c: &TSConditionalType<'_>,
        routes: BranchRoutes,
    ) -> DocId {
        let d = self.d();

        let extends_type_end = c.extends_type.span().end;
        let true_type_start = c.true_type.span().start;
        let true_type_end = c.true_type.span().end;
        let false_type_start = c.false_type.span().start;

        // An alone-on-line format-ignore directive in a branch gap (previous node's
        // span end → branch span start, the `?`/`:` inside the window) ROUTES that
        // branch's emission own-line and freezes the branch child verbatim. The
        // default emitters below trail-relocate every pre-operator comment onto the
        // extends/true line — an inert placement that would lose the freeze on the
        // second pass.
        //
        // The branch freezes WHOLE even when the child is a union or intersection —
        // the one head where composite-transparency (`single_child_frozen`, which
        // declines so the member rules apply instead) would be wrong. Those rules
        // only reach a directive the composite's OWN leading run finds, and that run
        // crosses whitespace and the transparent `|` / `&` / `(` alone: the interposing
        // `?` / `:` token stops it. Declining here would freeze nothing at all.
        //
        // So route and freeze coincide — the caller's gate already answered the gap
        // question for this exact window, and no freeze-TARGET arm is left to ask, so
        // `true_route` / `false_route` ARE the freeze verdicts.
        let BranchRoutes {
            true_route,
            false_route,
        } = routes;

        // Detect leading line comments inside parens around true_type / false_type
        // for relocation: prettier moves them to trail extends_type / true_type
        // (e.g., `extends b ? (// c\n  C) : D` → `extends b // c\n  ? C\n  : D`).
        // A FROZEN branch freezes its whole shell verbatim (shell comments ride the
        // slice), so it must not also collect them for relocation (print-once).
        let true_paren_leading_line_comments: CommentVec<'_> = if true_route {
            CommentVec::new()
        } else {
            self.stripped_paren_leading_line_comments(c.true_type)
        };
        let false_paren_leading_line_comments: CommentVec<'_> = if false_route {
            CommentVec::new()
        } else {
            self.stripped_paren_leading_line_comments(c.false_type)
        };

        // Find `extends` keyword position (reused for both extends_type_doc and comments_before_extends)
        let check_type_end = c.check_type.span().end;
        let extends_type_start = c.extends_type.span().start;
        let extends_kw_start = find_char_skipping_comments(
            self.source.as_bytes(),
            check_type_end as usize,
            extends_type_start as usize,
            b'e',
        )
        .map_or(check_type_end, |p| p as u32);
        let extends_kw_end = extends_kw_start + "extends".len() as u32;

        let extends_type_doc = self.build_conditional_type_extends_doc(c, extends_kw_end);

        let mut trailing_on_extends_parts: DocBuf = DocBuf::new();
        let mut q_parts = DocBuf::new();

        if true_route {
            let (trailing, branch) = self.build_routed_conditional_branch(
                extends_type_end,
                true_type_start,
                &true_paren_leading_line_comments,
                "?",
                self.build_frozen_single_child_doc(c.true_type),
            );
            trailing_on_extends_parts.push(trailing);
            q_parts.push(branch);
        } else {
            // Split comments around the `?` token by position so trailing line
            // comments on extends_type (e.g., `b // comment\n? c`) stay on
            // extends_type's line rather than being relocated past `?`.
            let q_pos = self.find_char_outside_comments(extends_type_end, true_type_start, b'?');
            let (before_q_end, after_q_start) = match q_pos {
                Some(q) => (q, q + 1),
                None => (true_type_start, extends_type_end),
            };

            // Comments BEFORE the `?` token, split by where the author put them: one on
            // the extends-type's line trails it, one on its own line keeps that line,
            // above the `?`. `build_own_line_preserving_run` is the seam for exactly this
            // question — what follows the run in the OUTPUT is the `?` operator, not the
            // node the comments lead — and it is what the value-level conditional already
            // does for the identical authoring.
            //
            // Emitting the whole gap as trailing (what this replaced) pulled an own-line
            // run up onto the extends-type's line, where the second `//` became text of
            // the first (`B extends C // c1 // c2`) and a comment was lost. Also includes
            // relocated leading line comments from inside true_type's parens.
            let (extends_trailing, own_line_before_q) =
                self.build_own_line_preserving_run(extends_type_end, before_q_end);
            trailing_on_extends_parts.push(extends_trailing);
            for comment in &true_paren_leading_line_comments {
                trailing_on_extends_parts.push(self.build_trailing_line_comment_doc(comment));
            }

            // Own-line comments sit above the `?`, at the branch indent.
            q_parts.extend(own_line_before_q);

            // ? on new line
            q_parts.push(d.hardline());
            q_parts.push(d.text("?"));

            // Comments AFTER the `?` token — emit between `?` and the true branch.
            let needs_indent_before_true = self.push_conditional_branch_gap_comments(
                &mut q_parts,
                after_q_start,
                true_type_start,
            );
            q_parts.push(self.build_conditional_branch_tail_doc(
                c.true_type,
                || {
                    self.build_relocated_conditional_branch_doc(
                        c.true_type,
                        &true_paren_leading_line_comments,
                    )
                },
                needs_indent_before_true,
                None,
            ));
        }

        if false_route {
            // The `:` branch mirrors the routed `?` emission, except that its
            // same-line comments trail the TRUE branch's line — already inside
            // `q_parts` — instead of a separate buffer.
            let (trailing, branch) = self.build_routed_conditional_branch(
                true_type_end,
                false_type_start,
                &false_paren_leading_line_comments,
                ":",
                self.build_frozen_single_child_doc(c.false_type),
            );
            q_parts.push(trailing);
            q_parts.push(branch);
        } else {
            // Comments trailing on true_type (between true_type and :) — preserve position.
            // Also includes relocated leading line comments from inside false_type's parens.
            // Split the same way as the `?` gap above — a comment on the true-branch's
            // line trails it, an own-line one keeps its line above the `:`. The two arms
            // ask one question through one seam, so they cannot drift.
            let colon = self.find_char_outside_comments(true_type_end, false_type_start, b':');
            let mut own_line_before_colon = DocBuf::new();
            if let Some(c_pos) = colon {
                let (true_trailing, own_line) =
                    self.build_own_line_preserving_run(true_type_end, c_pos);
                q_parts.push(true_trailing);
                own_line_before_colon = own_line;
            }
            for comment in &false_paren_leading_line_comments {
                q_parts.push(self.build_trailing_line_comment_doc(comment));
            }
            q_parts.extend(own_line_before_colon);

            // : on new line
            q_parts.push(d.hardline());
            q_parts.push(d.text(":"));

            // Comments after : only (between : and false_type)
            let colon_end = colon.map_or(true_type_end, |c| c + 1);
            let needs_indent_before_false = self.push_conditional_branch_gap_comments(
                &mut q_parts,
                colon_end,
                false_type_start,
            );
            q_parts.push(self.build_conditional_branch_tail_doc(
                c.false_type,
                || {
                    self.build_relocated_conditional_branch_doc(
                        c.false_type,
                        &false_paren_leading_line_comments,
                    )
                },
                needs_indent_before_false,
                None,
            ));
        }

        // Comments between check_type and `extends` keyword (reuses extends_kw_start from above)
        let comments_before_extends =
            self.build_comments_between(check_type_end, extends_kw_start, CommentSpacing::Leading);

        // `concat` short-circuits the no-trailing-comment case to `empty()`.
        let trailing_on_extends_doc = d.concat(&trailing_on_extends_parts);

        d.concat(&[
            self.build_conditional_check_doc(c.check_type),
            comments_before_extends,
            d.text(" extends"),
            extends_type_doc,
            trailing_on_extends_doc,
            d.indent(d.concat(&q_parts)),
        ])
    }

    /// The ordinarily-built doc for a conditional branch in the BREAKING layout, shared
    /// by the `?` and `:` arms (same nested-conditional logic as the non-breaking path).
    /// When leading line comments were relocated out of a parenthesized wrapper (any
    /// nesting depth, `paren_leading` non-empty), the doc is built from the
    /// fully-unwrapped inner so those comments aren't emitted twice.
    ///
    /// Built lazily by [`Self::build_conditional_branch_tail_doc`], whose union /
    /// intersection arms rebuild the branch from parts and would discard it.
    fn build_relocated_conditional_branch_doc(
        &self,
        branch: &TSType<'_>,
        paren_leading: &CommentVec<'_>,
    ) -> DocId {
        if !paren_leading.is_empty() {
            self.build_type_doc(unwrap_parenthesized(branch))
        } else if let TSType::Conditional(inner) = unwrap_parenthesized(branch) {
            self.build_conditional_type_doc_inner(inner)
        } else {
            self.build_type_doc(branch)
        }
    }

    /// The routed branch-gap emission shared by the `?` and `:` arms of
    /// [`Self::build_conditional_type_doc_with_line_comments`], for a gap holding an
    /// alone-on-line format-ignore directive: own-line-preserving
    /// ([`Self::build_own_line_preserving_run`]), so the placement that earned the
    /// freeze survives.
    ///
    /// Returns `(trailing, branch)` — the caller places them, since the two arms differ
    /// only in where a same-line comment goes (the `?` arm's trails the extends line, a
    /// separate buffer; the `:` arm's trails the true branch, already inside `q_parts`).
    /// Every own-line comment (the directive among them) sits ABOVE the operator's
    /// line, and the branch follows the operator on that line. The whole gap
    /// `[gap_start, branch_start)` is claimed here, both sides of the operator, so no
    /// comment is emitted twice (print-once). A routed composite's stripped-shell
    /// leading line comments (`paren_leading`) join the own-line run.
    ///
    /// `branch_doc` is the frozen verbatim slice — a routed branch always freezes — so
    /// the branch tail is spelled out here rather than taken from
    /// [`Self::build_conditional_branch_tail_doc`], whose union / intersection arms
    /// rebuild their doc from parts and would discard it. The shape is that function's
    /// `_` arm: a space, then the branch one level past the operator (an `indent` a
    /// verbatim span carries no doc lines to consume, kept so the two read alike).
    fn build_routed_conditional_branch(
        &self,
        gap_start: u32,
        branch_start: u32,
        paren_leading: &CommentVec<'_>,
        operator: &'static str,
        branch_doc: DocId,
    ) -> (DocId, DocId) {
        let d = self.d();
        let (trailing, mut parts) = self.build_own_line_preserving_run(gap_start, branch_start);
        for comment in paren_leading {
            parts.push(d.hardline());
            parts.push(self.build_comment_doc(comment));
        }
        parts.push(d.hardline());
        parts.push(d.text(operator));
        parts.push(d.text(" "));
        parts.push(d.indent(branch_doc));
        (trailing, d.concat(&parts))
    }

    //
    // Mapped Types
    //

    /// Push one of a mapped type's two keyword clauses — the key's `in <constraint>`
    /// and the `as <name type>` rename — with the gaps on **both** sides of the
    /// keyword. The two clauses are the same shape end to end, so they share one
    /// emitter: locate the keyword outside comments, route the pre-keyword gap
    /// ([`Printer::route_pre_keyword_gap`] — a line comment there keeps the comment
    /// trailing the head and defers the whole `<keyword> <value>` tail to a
    /// continuation line indented one level; a block trails the head in place), then
    /// the keyword, the keyword→value gap's comments (hang-aware, so a `//` can't
    /// swallow the value), and the value.
    ///
    /// `keyword` / `spaced_keyword` are the same literal with and without its leading
    /// space: the continuation arm's leading space is `build_continuation_indent`'s, so
    /// emitting a second one after the run's hardline would be a stray leading space on
    /// the continuation line — while the inline arm keeps its single text node. The
    /// keyword's own first byte and length drive the scan, so neither can drift from
    /// the text actually emitted.
    fn push_mapped_keyword_clause(
        &self,
        parts: &mut DocBuf,
        head_end: u32,
        keyword: &'static str,
        spaced_keyword: &'static str,
        value: &TSType<'_>,
    ) {
        let d = self.d();
        let value_start = value.span().start;
        // The keyword's first byte, skipping comments before it, so a matching byte
        // inside a comment (`K /* in */ in T`) isn't read as the keyword.
        let keyword_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            head_end as usize,
            value_start as usize,
            keyword.as_bytes()[0],
        );
        let keyword_end = keyword_pos.map_or(head_end, |p| (p + keyword.trim_end().len()) as u32);
        let keyword_pos = keyword_pos.map_or(head_end, |p| p as u32);
        let gap = self.route_pre_keyword_gap(parts, head_end, keyword_pos);
        let mut tail: DocBuf = smallvec![d.text(if gap.is_some() {
            keyword
        } else {
            spaced_keyword
        })];
        tail.push(self.build_trailing_comments_hang_next(keyword_end, value_start));
        tail.push(self.build_type_doc(value));
        let tail = d.concat(&tail);
        parts.push(match gap {
            Some((start, end)) => self.build_continuation_indent(start, end, tail),
            None => tail,
        });
    }

    /// Build doc for mapped type: `{ [K in T]: V }`
    ///
    /// Source-fidelity aware: preserves multi-line formatting when source is multi-line.
    /// - Source one-line, fits: `{[K in keyof T]: T[K]}`
    /// - Source one-line, long: `{\n\t[K in keyof T]: T[K];\n}`
    /// - Source multi-line: `{\n\t[K in keyof T]: T[K];\n}` (always)
    pub(super) fn build_mapped_type_doc(&self, m: &TSMappedType<'_>) -> DocId {
        let d = self.d();
        // Check if source was multi-line (preserve author's formatting choice).
        //
        // Intentionally NOT gated by `self.canonical` — and NOT merely "unimplemented".
        // This read is one half of a pair: `is_call_with_complex_type_arguments`
        // (printer/expressions/assignment.rs) approximates prettier's `willBreak` on a
        // mapped type-arg with the same source-newline test. The two must agree, or the
        // assignment's poorly-breakable classification is wrong.
        //
        // Gating BOTH on `canonical` looks like the obvious full-erasure fix. It is
        // unsound: canonical mode preserves comments, and a line comment inside the
        // mapped type still forces a break — so the doc force-breaks while a
        // canonical-gated newline test reports "doesn't break", the assignment is
        // misclassified poorly-breakable, and `is_poorly_breakable_chain`'s debug_assert
        // fires. The root cause is that canonical mode CHANGES what force-breaks
        // (authored newlines stop forcing; comments still do), which invalidates the
        // "source newline <=> willBreak" approximation both reads rest on. Erasing this
        // properly needs a canonical-aware willBreak approximation (forcing-comment-in-span
        // instead of newline-in-span), not a flag.
        //
        // So mapped-type multi-line-ness is a deliberate un-erased residual of the
        // canonicalizer; `format_canonical`'s docs record it as a contract hole.
        // Unreachable for the current consumer (compiled JS carries no TS types).
        let source_is_multiline = super::super::is_brace_block_multiline(self.source, m.span);

        // Find the start of the mapping content (after `{`)
        let content_start = m.span.start + 1; // after `{`
        let param_name_start = m.type_parameter.span.start; // start of `K`

        // The mapped bracket `[` splits the header comments into two positions:
        //  - between `{` and `[`: LEADING the mapped type — prettier 3.9 (#18731)
        //    keeps an inline-authored block comment before `[` (`{ /* c */ [K in T] }`);
        //  - between `[` and the key: INSIDE the brackets, before the key
        //    (`{ [/* c */ K in T] }`) — these stay after `[`.
        let bracket_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            content_start as usize,
            param_name_start as usize,
            b'[',
        )
        .map_or(param_name_start, |p| p as u32);
        let leading_comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, content_start, bracket_pos).collect();
        let bracket_inner_comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, bracket_pos + 1, param_name_start).collect();

        // Leading comments (before `[`): the node-adjacent (LAST) comment stays
        // inline iff it's a block comment with no newline after it; every earlier
        // comment, and any line/own-line comment, goes on its own line (and in a
        // single-line source forces the mapped type to break).
        let leading_n = leading_comments.len();
        let leading_last_inline = leading_comments
            .last()
            .is_some_and(|c| c.is_block && !has_newline_after_position(self.source, c.span.end));
        let leading_own_line_end = if leading_last_inline {
            leading_n - 1
        } else {
            leading_n
        };

        // Build the mapping body (starting from `[`)
        let mut body_parts = d.pooled_docbuf();

        // The node-adjacent inline block comment leads the body, before the
        // `readonly` modifier and `[` (prettier: `/* c */ readonly [K in T]`).
        if leading_last_inline {
            body_parts.push(self.build_comment_doc(leading_comments[leading_n - 1]));
            body_parts.push(d.text(" "));
        }

        // Whole-signature freeze: an alone-on-line format-ignore directive between `{`
        // and the `[` (`mapped_gap_frozen`) freezes the whole `[K in ...]: V` clause,
        // the mapped type's sole-member analog of Rule A. The braces and the member
        // `;` stay parent-emitted outside the slice, and the directive itself was
        // just emitted with the leading comments. A `readonly` modifier declines for
        // now — the slice would need the modifier's own start position.
        // TODO: extend the freeze slice to a `readonly`/`+`/`-` modifier.
        if m.readonly.is_none()
            && let Some(type_ann) = &m.type_annotation
            && self.mapped_gap_frozen(content_start, bracket_pos)
        {
            let type_end = type_ann.span().end;
            body_parts.push(self.raw_source_range(bracket_pos, type_end));
            let body_end = m.span.end.saturating_sub(1); // before `}`
            for comment in comments_to_emit_in_range(self.comments, type_end, body_end) {
                body_parts.push(self.build_trailing_comment_doc(comment));
            }
            return self.build_mapped_type_shell(
                source_is_multiline,
                &leading_comments[..leading_own_line_end],
                &body_parts,
            );
        }

        // readonly modifier: `readonly`, `+readonly`, or `-readonly`
        if let Some(readonly) = m.readonly {
            body_parts.push(d.text(match readonly {
                TSMappedTypeModifier::True => "readonly ",
                TSMappedTypeModifier::Plus => "+readonly ",
                TSMappedTypeModifier::Minus => "-readonly ",
            }));
        }

        // [K in constraint] — build the bracket interior (key + `in` + constraint +
        // optional `as` + pre-`]` comments) into a buffer so a leading line comment in
        // the `[`→key gap can break the whole `[…]` (mirrors `build_computed_key_bracket_doc`).
        let mut interior_parts: DocBuf = smallvec![];

        // Binding freeze: an alone-on-line format-ignore directive inside the
        // bracket (`mapped_gap_frozen` over the `[`→key gap) freezes
        // just the `K in ...` binding — the value keeps formatting normally, and the
        // directive itself is emitted by the bracket's own comment machinery (the
        // line-comment break shell, or the inline loop below).
        if self.mapped_gap_frozen(bracket_pos + 1, param_name_start) {
            interior_parts.push(
                self.raw_source_range(param_name_start, m.type_parameter.constraint.span().end),
            );
        } else {
            interior_parts
                .push(self.ident_name_doc(m.type_parameter.name, m.type_parameter.span.start));
            // Comments around the `in` keyword: `key /* c1 */ in /* c2 */ Constraint`
            let name_len = self.with_ident_name_at(
                m.type_parameter.name,
                m.type_parameter.span.start,
                str::len,
            );
            let name_end = m.type_parameter.span.start + name_len as u32;
            self.push_mapped_keyword_clause(
                &mut interior_parts,
                name_end,
                "in ",
                " in ",
                m.type_parameter.constraint,
            );
        }

        // as clause: `as NewKeyType`
        // Track the end of the last element inside brackets (for bracket-close comments)
        let mut last_inner_end = m.type_parameter.constraint.span().end;
        if let Some(name_type) = &m.name_type {
            // Comments around the `as` keyword: `Constraint /* c1 */ as /* c2 */ NewKey`
            // — the same clause shape as key→`in` above, same emitter.
            self.push_mapped_keyword_clause(
                &mut interior_parts,
                m.type_parameter.constraint.span().end,
                "as ",
                " as ",
                name_type,
            );
            last_inner_end = name_type.span().end;
        }

        // Comments between last inner element and `]`
        let bracket_close = self
            .find_char_outside_comments(last_inner_end, m.span.end, b']')
            .unwrap_or(last_inner_end);
        interior_parts.push(self.build_comments_between(
            last_inner_end,
            bracket_close,
            CommentSpacing::Leading,
        ));
        let after_key_line = self.has_line_comments_between(last_inner_end, bracket_close);

        // A line comment in the `[`→key gap (`[ // c⏎K in T]`) forces the whole bracket
        // to break: emitting the key inline right after a `//` would swallow `K in T`
        // (content loss, non-idempotent). Break so each comment and the key take their
        // own line and `]` drops — the same in-place break index signatures already use
        // (both match prettier); prettier only relocates the comment for value positions.
        // A same-line block comment (`[/* c */ K in T]`) keeps the flat inline form.
        let bracket_leading_line =
            self.has_line_comments_between(bracket_pos + 1, param_name_start);
        if bracket_leading_line {
            // The pre-`]` comments are already inside `interior_parts` (built above via
            // `build_comments_between`), so the shared helper takes the whole interior as
            // the body and owns only the `[`→key prefix and the break shell.
            body_parts.push(self.build_bracket_line_comment_break(
                "[",
                bracket_pos,
                param_name_start,
                d.concat(&interior_parts),
            ));
        } else {
            body_parts.push(d.text("["));
            // Same-line block comments before the key stay inline (`[/* c */ K in T]`).
            for comment in &bracket_inner_comments {
                body_parts.push(self.build_comment_doc(comment));
                body_parts.push(d.text(" "));
            }
            body_parts.push(d.concat(&interior_parts));
            // A line comment trailing the key constraint (before `]`) drops `]` to its
            // own line (`[K in T // c⏎]`) so emitting `]` inline can't swallow it.
            if after_key_line {
                body_parts.push(d.hardline());
            }
            body_parts.push(d.text("]"));
        }

        // optional modifier text: `?`, `+?`, or `-?`. Emitted per arm below — the
        // value path places the `]`→`:` region's comments around it; the frozen and
        // no-value paths keep it glued to `]`.
        let marker_text = m.optional.map(|optional| match optional {
            TSMappedTypeModifier::True => "?",
            TSMappedTypeModifier::Plus => "+?",
            TSMappedTypeModifier::Minus => "-?",
        });

        // Comments and value type
        if let Some(type_ann) = &m.type_annotation {
            let type_start = type_ann.span().start;
            let type_end = type_ann.span().end;

            // A format-ignore directive in the `]`→value gap freezes a non-composite
            // value verbatim (`single_child_frozen`; a union/intersection value
            // declines and freezes via its own walk). The frozen path keeps the
            // UNWIDENED window so an in-shell directive stays on the ordinary paths.
            let head = self.keyword_value_head(bracket_close, type_ann);

            if head.frozen {
                // Frozen: keep the unsplit emission — marker glued, `:`, then the
                // whole gap's comments own-line-preserving
                // (`append_keyword_value_line_comments`) so the directive keeps the
                // line that earned the freeze; the value is the verbatim slice.
                if let Some(marker) = marker_text {
                    body_parts.push(d.text(marker));
                }
                body_parts.push(d.text(":"));
                if self.has_line_comments_between(bracket_close, head.value_start) {
                    let value_doc = self.build_keyword_value_doc(&head, TrailingBlock::Inline);
                    self.append_keyword_value_line_comments(
                        &mut body_parts,
                        bracket_close,
                        head.value_start,
                        value_doc,
                    );
                } else {
                    body_parts.push(self.build_comments_between(
                        bracket_close,
                        head.value_start,
                        CommentSpacing::Leading,
                    ));
                    body_parts.push(d.text(" "));
                    body_parts.push(self.build_frozen_single_child_doc(type_ann));
                }
            } else {
                // The `]`→value region splits at the `:` (and the optional marker): a
                // comment before the `:` trails the `]`/marker in its authored slot —
                // the index-signature treatment (`[K in T] /* c */ : V`,
                // `[K in T]? /* c */ : V`, `[K in T] /* c */?: V`) — never re-binding
                // across the `:`; a comment after the `:` leads the value (the tail).
                // Prettier relocates all of these into the brackets, trailing the key
                // constraint (conformance_prettier_ts_comments.md §Comment relocation).
                let colon_pos = self
                    .find_char_outside_comments(bracket_close, type_start, b':')
                    .unwrap_or(bracket_close);
                // The marker's `?`; a comment between a `+`/`-` sign and its `?` sits
                // before `marker_pos` and folds to before the whole marker.
                let (marker_pos, after_marker) = if marker_text.is_some() {
                    let q = self
                        .find_char_outside_comments(bracket_close, colon_pos, b'?')
                        .unwrap_or(colon_pos);
                    (q, q + 1)
                } else {
                    (bracket_close, bracket_close)
                };

                let tail = self.build_mapped_value_tail_doc(&head, colon_pos, type_ann);

                if self.comments_force_own_line_between(bracket_close, marker_pos) {
                    // A line comment (or broke-after multiline block) before the
                    // marker: the marker joins the `: V` tail on the continuation line
                    // (the property-signature key→`?` treatment), each hanging comment
                    // ending its own line so a `//` can't swallow what follows.
                    let tail_with_marker = match marker_text {
                        Some(marker) => d.concat(&[d.text(marker), tail]),
                        None => tail,
                    };
                    body_parts.push(self.build_continuation_indent(
                        bracket_close,
                        colon_pos,
                        tail_with_marker,
                    ));
                } else {
                    // Before-marker comments stay before the marker, spaced off the
                    // `]`, the marker glued after (`] /* c */?: V` — the
                    // property-signature parity).
                    body_parts.push(self.build_comments_between(
                        bracket_close,
                        marker_pos,
                        CommentSpacing::Leading,
                    ));
                    if let Some(marker) = marker_text {
                        body_parts.push(d.text(marker));
                    }
                    if self.comments_force_own_line_between(after_marker, colon_pos) {
                        // A line comment (or broke-after multiline block) between the
                        // `]`/marker and the `:`: the comment trails in place, the
                        // `: V` tail drops to a continuation line indented one level
                        // (uniform forced-continuation indent).
                        body_parts.push(self.build_continuation_indent(
                            after_marker,
                            colon_pos,
                            tail,
                        ));
                    } else {
                        // Block-only comments stay inline before the `:`, spaced
                        // (`] /* c */ : V`); a comment-free gap keeps `:` glued.
                        if let Some(doc) = self.build_comments_between_filtered_opt(
                            after_marker,
                            colon_pos,
                            CommentSpacing::Leading,
                            CommentFilter::All,
                        ) {
                            body_parts.push(doc);
                            body_parts.push(d.text(" "));
                        }
                        body_parts.push(tail);
                    }
                }
            }

            // Trailing comments after the value type, via the shared trailing-gap
            // emitter: a block trails inline before the `;` (`V /* c */;`), a line
            // comment rides `line_suffix` so it floats to end-of-line *after* the `;`
            // (`V; // c`) instead of swallowing it — the `;` is emitted separately by
            // the multiline/one-line branch below. Open-coding the loop here dropped
            // the emitter's separator and ordering rules, welding a run onto one line
            // (`V; // c1 // c2`) and reordering an inline block ahead of a deferred
            // line comment; see [docs/comments.md](../../../../../docs/comments.md)
            // §Trailing and dangling runs.
            let body_end = m.span.end.saturating_sub(1); // before `}`
            self.push_trailing_comments_in_range(&mut body_parts, type_end, body_end);
        } else {
            // No value type (`{ [K in T] }`): comments after the `]` (or the
            // optional modifier) still trail the member the same way — dropping
            // through without collecting them would lose content.
            if let Some(marker) = marker_text {
                body_parts.push(d.text(marker));
            }
            let body_end = m.span.end.saturating_sub(1); // before `}`
            self.push_trailing_comments_in_range(&mut body_parts, bracket_close, body_end);
        }

        self.build_mapped_type_shell(
            source_is_multiline,
            &leading_comments[..leading_own_line_end],
            &body_parts,
        )
    }

    /// The mapped type's `: V` tail — the value `:` plus the `:`→value gap's comments
    /// and the value itself; the caller owns everything before the `:` (the `]`, the
    /// optional marker, and the pre-`:` comment slots).
    ///
    /// A line comment after the `:` stays trailing it, with the value type on the
    /// next line (preserve-in-place; prettier relocates the comment to trail the
    /// member `;`). A redundant paren shell with a leading line-comment run
    /// (`]: (// c\n V)`) strips to the same hang as bare `]: // c\n V`; the shared
    /// keyword→value seam routes it so the paren form is idempotent (the outer paren
    /// would otherwise hide the comment from the gate). A mixed / trailing shell
    /// hoists losslessly too — the trailing comment via `build_hang_value_doc`
    /// (`defer = false`: a type position keeps a lifted trailing block inline before
    /// the member `;`).
    ///
    /// A union/intersection value breaks after `:` and hangs (leading `| ` for
    /// unions, indented continuations for intersections) instead of gluing to the
    /// colon when it exceeds print width — matching prettier's `shouldIndent` →
    /// `indent(parts)`. Redundant comment-free parens around the value are stripped
    /// first (prettier does the same). A hugging union (`{ ... } | null`) keeps its
    /// inline `: ` since the object owns its own expansion; `union_prints_hugged`
    /// owns that question whole — this site used to pair the bare syntactic shape
    /// with its own NARROWER comment scan (line comments between members only), which
    /// let a block comment between members, or a line comment in the leading
    /// `|`→first-member gap, read as "hug" while the printer expanded them.
    fn build_mapped_value_tail_doc(
        &self,
        head: &KeywordValueHead<'_>,
        colon_pos: u32,
        type_ann: &TSType<'_>,
    ) -> DocId {
        let d = self.d();
        let mut tail_parts: DocBuf = smallvec![d.text(":")];
        if self.has_line_comments_between(colon_pos + 1, head.value_start) {
            let value_doc = self.build_keyword_value_doc(head, TrailingBlock::Inline);
            self.append_keyword_value_line_comments(
                &mut tail_parts,
                colon_pos + 1,
                head.value_start,
                value_doc,
            );
        } else {
            tail_parts.push(self.build_comments_between(
                colon_pos + 1,
                type_ann.span().start,
                CommentSpacing::Leading,
            ));
            match self.unwrap_redundant_parens(type_ann) {
                TSType::Union(u) => {
                    let type_doc = self.build_union_type_doc(u);
                    if self.union_prints_hugged(u) {
                        tail_parts.push(d.text(" "));
                        tail_parts.push(type_doc);
                    } else {
                        tail_parts.push(hang_after_operator(d, type_doc));
                    }
                }
                TSType::Intersection(i) => {
                    tail_parts.push(d.text(" "));
                    tail_parts.push(self.intersection_hanging_with_indent(i));
                }
                _ => {
                    tail_parts.push(d.text(" "));
                    tail_parts.push(self.build_type_doc(type_ann));
                }
            }
        }
        d.concat(&tail_parts)
    }

    /// The mapped type's brace shell around an already-built member body: own-line
    /// leading comments each take their own line before the body, then the member `;`
    /// and `}` — hardlines in the multi-line-source form, a width-decided group with
    /// bracketSpacing boundaries and an `if_break` `;` in the one-line form.
    fn build_mapped_type_shell(
        &self,
        source_is_multiline: bool,
        own_line_leading: &[&Comment],
        body_parts: &[DocId],
    ) -> DocId {
        let d = self.d();
        if source_is_multiline {
            // Multi-line source: preserve multi-line format with hardlines.
            // Own-line leading comments each take their own line before `[`; the
            // node-adjacent inline block comment (if any) already leads `body_parts`.
            let mut inner_parts: DocBuf = smallvec![];
            for comment in own_line_leading {
                inner_parts.push(d.hardline());
                inner_parts.push(self.build_comment_doc(comment));
            }
            inner_parts.push(d.hardline());
            inner_parts.push(d.concat(body_parts));
            inner_parts.push(d.text(";"));

            d.concat(&[
                d.text("{"),
                d.indent(d.concat(&inner_parts)),
                d.hardline(),
                d.text("}"),
            ])
        } else {
            // One-line source: width-aware (stays inline if fits, wraps if too long).
            // bracketSpacing boundaries: a space when flat (`{ [K in T]: U }`), a
            // newline when broken. An own-line leading comment (a line comment, or a
            // non-adjacent block) forces the break via its `hardline`.
            let mut all_parts: DocBuf = smallvec![];
            for comment in own_line_leading {
                all_parts.push(d.hardline());
                all_parts.push(self.build_comment_doc(comment));
            }
            all_parts.push(d.line());
            all_parts.extend(body_parts.iter().copied());
            all_parts.push(d.if_break(d.text(";"), d.empty()));

            d.group(d.concat(&[
                d.text("{"),
                d.indent(d.concat(&all_parts)),
                d.line(),
                d.text("}"),
            ]))
        }
    }

    //
    // Tuple Types
    //

    /// Render one tuple element. A tuple that breaks places each element on its own
    /// indented line, so an intersection element is an `own_line` context: its
    /// first-member comment hoist must not add a second continuation indent on top of
    /// the tuple's element indent (see `build_intersection_type_doc`'s `own_line`).
    /// Non-intersection elements — and intersections without a hoisting first-member
    /// comment, for which `own_line` is a no-op — route through the shared
    /// `build_type_doc` unchanged.
    ///
    // TODO: the sibling own-line container positions — a type-argument list `<…>`, an
    // arrow/function return type, and an `(A & B)[]` array element — still route their
    // intersection through the trailing-prefix `build_type_doc` default and so hit the
    // same first-member-hoist over-indent (a latent non-idempotency, near-zero real-code
    // frequency). Each needs the same own-line routing (plus, for the conditional branch,
    // the separate un-glue fix) as part of the intersection-printer convergence.
    fn build_tuple_element_doc(&self, elem: &TSType<'_>) -> DocId {
        match elem {
            TSType::Intersection(i) => self.build_intersection_type_doc(i, true, true),
            _ => self.build_type_doc(elem),
        }
    }

    /// Build a Doc for a tuple type: `[A, B, C]`
    ///
    /// Uses width-aware breaking: inline if fits, one element per line if not.
    ///
    /// ⚠️ Below the expansion gate this builder reads **raw** `TSType::span`s, not
    /// [`tuple_elem_span`], and the asymmetry with its expanding twin is deliberate:
    /// here each element's doc is `build_type_doc`, so a shell that survives to this path
    /// emits its own interior comments ([`Self::build_parenthesized_type_unwrap_doc`]) and
    /// the gaps this loop scans must stop at the shell, not inside it. Unwrapping the spans
    /// here without also unwrapping the docs would double-print every one of those comments
    /// ([`comments.md`](../../../../docs/comments.md) hazard 3).
    pub(super) fn build_tuple_type_doc(&self, t: &TSTupleType<'_>) -> DocId {
        let d = self.d();
        if t.element_types.is_empty() {
            return self.build_empty_brackets_inline_with_comments_doc(t.span);
        }

        // Zero-comment fast gate (see `build_params_doc_with_comments`): every
        // comment sub-query below is bounded within the tuple's span, so with no
        // comment there the expansion checks are provably false and the list is
        // plain elements joined by `,` + line (renders identically — the skipped
        // pushes are empty comment docs and the empty after-comma buffer).
        // **On-page**: a layout builder's zero-comment fast gate (an emit-keyed
        // answer would blind every gate it guards — same axis note as
        // `build_type_arguments_doc`; equivalent today, since ownership is set only
        // in expression position and a `[…]` window holds types).
        if !self.has_comments_on_page_between(t.span.start, t.span.end) {
            let mut parts = DocBuf::new();
            for (i, elem) in t.element_types.iter().enumerate() {
                if i > 0 {
                    parts.push(d.text(","));
                    parts.push(d.line());
                }
                parts.push(self.build_tuple_element_doc(elem));
            }
            return d.group(bracketed_list_body(d, "[", "]", d.concat(&parts), false));
        }

        // Check for comments that force expansion: line comments, multiline block comments,
        // or own-line single-line block comments. Also check for line comments BEFORE the
        // first element (between `[` and first element), e.g., `[// leading\n a, b]`.
        //
        // Every clause asks the **effective** element span (the free `tuple_elem_span`) —
        // the node left once the element's redundant paren shell is stripped, the
        // same node the expansion builder emits. A comment the author wrote inside that
        // shell physically lands in one of the tuple's own gaps (`[`→element,
        // element→element, element→`]`) and must route the list here like any other
        // comment there. Asking `TSType::span` left those gaps empty, so the shape fell
        // through to the width path and the comment rendered from the element's own doc —
        // reaching a different fixed point than the same comment written without the
        // parens (`[A, (⏎/* c */⏎B)]` collapsing inline where the bare authoring expands).
        // Both spellings are idempotent, so only a bare-vs-paren comparison shows the
        // split — the `unformatted_parens` variants are that claim.
        //
        // Own-line-ness is measured on the SOURCE, so a block glued to the shell's `(`
        // (`[A,⏎(/* c */⏎B)]`) still collapses inline, matching prettier: the `(` occupies
        // the comment's line whether or not the item span covers it — see
        // `has_own_line_block_comments_in_bracket_list`.
        //
        // The shell peels whatever it carries — [`unwrap_parenthesized`], not
        // [`Self::leading_paren_unwrapped`], so a **trailing** run comes here too. That
        // stops short of the type-argument family for a reason: the deferred-run rule
        // ([`Self::paren_retains_for_trailing_run`]) retains a shell only where the
        // comment would escape the construct it was written in, and a tuple element's
        // trailing gap is one the enclosing `[…]` DOES emit — the same argument
        // `type_member_separator_follows` makes for a `|`/`&` member, since the per-element
        // break ends the output line right where the shell ends. Routed here, the run
        // lands in the element→`,` / element→`]` gap and prints from the seam that already
        // owns it (`[A, (B // c⏎)]` → `[⏎A,⏎B // c⏎]`), inside the brackets either way.
        let has_leading_line_comment = t.element_types.first().is_some_and(|first| {
            self.has_line_comments_between(t.span.start + 1, tuple_elem_span(first).start)
        });
        if has_leading_line_comment
            || self.has_line_comments_in_delimited_list(
                t.element_types,
                tuple_elem_span,
                t.span.end - 1,
            )
            || self.has_own_line_block_comments_in_bracket_list(
                t.span,
                t.element_types,
                tuple_elem_span,
            )
        {
            return self.build_tuple_type_doc_with_line_comments(t);
        }

        // Build element docs with commas, inline block comments, and line breaks
        let mut parts = DocBuf::new();
        let mut prev_end = t.span.start + 1; // After opening `[`
        let mut force_break = false;
        // Block comment trailing the last element after its source comma — preserved
        // past where the comma was (no trailing comma; prettier relocates before; see
        // conformance_prettier_ts_comments.md §Comment relocation).
        let mut last_after_comma = DocBuf::new();
        for (i, elem) in t.element_types.iter().enumerate() {
            if i > 0 {
                parts.push(d.text(","));
                parts.push(d.line());
            }

            // Add inline leading block comments (after previous comma or `[`)
            let leading =
                self.build_inline_comments_between_doc_trailing_space(prev_end, elem.span().start);
            parts.push(leading);

            // Rule A: an alone-on-line directive in this element's gap freezes it
            // (the directive itself was just emitted by the gap emitter above); a
            // multi-line frozen slice forces the broken layout (a verbatim span is
            // `will_break`-opaque, so the forcing is explicit).
            let frozen = self.list_member_frozen(t.span.start + 1, t.element_types, i, false);
            if frozen {
                if self.frozen_list_member_multiline(elem) {
                    force_break = true;
                }
                parts.push(self.build_frozen_list_member_doc(elem));
            } else {
                parts.push(self.build_tuple_element_doc(elem));
            }

            let elem_end = elem.span().end;
            prev_end = if i + 1 < t.element_types.len() {
                let next_start = t.element_types[i + 1].span().start;
                let comma_pos = self.find_list_comma(elem_end, next_start);
                // Only the run that follows content on its line trails this element; the
                // rest leads the next one, so the leading scan resumes at the run's end
                // rather than past the comma (`Printer::inline_trailing_run_end`).
                let run_end = self.inline_trailing_run_end(elem_end, comma_pos);
                self.append_trailing_inline_block_comments(&mut parts, elem_end, run_end);
                run_end
            } else {
                let before_close = t.span.end - 1;
                self.append_last_trailing_block_comments_split(
                    &mut parts,
                    &mut last_after_comma,
                    elem_end,
                    before_close,
                );
                before_close
            };
        }

        // Width-aware breaking: inline if fits, one-per-line if not (no trailing
        // comma; trailingComma: 'none'). A multi-line frozen element forces the
        // broken form (see `bracketed_list_body`).
        let inner = d.concat(&[d.concat(&parts), d.concat(&last_after_comma)]);
        d.group(bracketed_list_body(d, "[", "]", inner, force_break))
    }

    /// Build tuple type with expanding comments (line comments or own-line block comments)
    ///
    /// Spans and docs both come from the free `tuple_elem_span` / [`unwrap_parenthesized`],
    /// so this builder's gap emitters and the item docs agree on where each element starts —
    /// see [`Self::build_tuple_type_doc`]'s expansion gate for why a paren shell must not
    /// reach here.
    fn build_tuple_type_doc_with_line_comments(&self, t: &TSTupleType<'_>) -> DocId {
        let d = self.d();
        // A comment trailing the opening `[` on its own line is kept on the `[`
        // line when the tuple expands (divergence from prettier, which relocates
        // it to its own line as the first element's leading comment). A
        // line/own-line comment is itself what forces this path. Tuple types have
        // no elision, so the first element is always present. See
        // conformance_prettier_ts_comments.md §Comment relocation (Tuple type `[`).
        let elem_span_at = |i: usize| tuple_elem_span(&t.element_types[i]);
        let first_elem_start = elem_span_at(0).start;
        let (bracket_line_prefix, delimiter_pull_pos) =
            self.delimiter_line_comment_prefix(t.span.start, first_elem_start);

        let mut inner_parts = DocBuf::new();
        let mut prev_end = t.span.start + 1; // After the opening `[`

        for (i, raw_elem) in t.element_types.iter().enumerate() {
            let elem = unwrap_parenthesized(raw_elem); // pairs with `tuple_elem_span`
            let elem_start = elem.span().start;
            let elem_end = elem.span().end;
            let is_last = i == t.element_types.len() - 1;

            // Leading comments (after previous comma or `[`). For the first
            // element, drop comments pulled onto the `[` line (emitted as the
            // bracket-line prefix below).
            let skip_delim = if i == 0 { delimiter_pull_pos } else { None };
            let leading = self.build_leading_comments_multiline(prev_end, elem_start, skip_delim);
            // Rule A: an alone-on-line directive in this element's gap freezes
            // it; the directive itself was just emitted by the leading run above.
            // No must-break question — this layout is already all-hardline.
            let frozen = self.list_item_frozen(t.span.start + 1, &elem_span_at, i);
            let elem_doc = if frozen {
                self.build_frozen_list_member_doc(elem)
            } else {
                self.build_tuple_element_doc(elem)
            };
            inner_parts.push(self.build_list_element_group(leading, elem_doc));

            if !is_last {
                let next_start = elem_span_at(i + 1).start;
                // Tuples preserve an author blank line before a member's own-line
                // leading comment (prettier does; type-param/arg lists do not).
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    elem_end,
                    next_start,
                    BlankRule::AfterComma,
                );
            } else {
                // Last element: no trailing comma under `trailingComma: 'none'`, then
                // comments before `]`.
                let before_close = t.span.end - 1;
                inner_parts.extend(self.build_trailing_comments_multiline(elem_end, before_close));
                prev_end = before_close;
            }
        }

        d.concat(&[
            d.text("["),
            d.concat(&bracket_line_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])),
            d.hardline(),
            d.text("]"),
        ])
    }

    //
    // Array Types
    //

    /// Build a Doc for an array type (e.g., `number[]`)
    pub(super) fn build_array_type_doc(&self, arr: &TSArrayType<'_>) -> DocId {
        let d = self.d();
        // A comment-free parenthesized union element EXPANDS its parens when it breaks
        // (`(⏎\t| A⏎\t| B⏎)[]`) instead of gluing the leading `|` to the `(`. Any other
        // parenthesized element (conditional / function / intersection) keeps glued
        // parens and breaks internally (`(T extends X⏎\t? Y⏎\t: Z)[]`). See the shared
        // `build_expanded_parenthesized_union_opt`.
        if let Some(union_doc) = self.build_expanded_parenthesized_union_opt(arr.element_type) {
            return d.concat(&[union_doc, self.build_array_suffix_doc(arr)]);
        }
        // An alone-on-line format-ignore directive INSIDE a paren-shell element
        // (`(⏎⇥// prettier-ignore⏎⇥(a: T) => void⏎)[]`) freezes the fully-unwrapped
        // inner only (`paren_interior_routed_inner`; a composite inner declines and freezes
        // via its own `(`-transparent leading-run walk). The parens expand around the
        // own-line run — the fall-through below would glue the synthesized `(` onto
        // the comment with a cascading over-break — and are always kept: the shell
        // holds a comment, and comment preservation outranks redundant-paren removal
        // under a freeze. Trailing shell-gap comments are lifted after the inner
        // (`with_stripped_paren_trailing`), so every shell comment prints once.
        if let Some(inner) = self.paren_interior_routed_inner(arr.element_type) {
            let inner_doc = self.build_routed_child_doc(inner);
            let value_doc = self.with_stripped_paren_trailing(
                inner_doc,
                arr.element_type,
                inner,
                TrailingBlock::Inline,
            );
            let mut parts: DocBuf = smallvec![d.text("(")];
            self.append_keyword_value_line_comments(
                &mut parts,
                arr.element_type.span().start + 1,
                inner.span().start,
                value_doc,
            );
            parts.push(d.hardline());
            parts.push(d.text(")"));
            parts.push(self.build_array_suffix_doc(arr));
            return d.concat(&parts);
        }
        let element_doc = self.build_type_doc(arr.element_type);
        let suffix_doc = self.build_array_suffix_doc(arr);
        if type_needs_parens_for_array_element(arr.element_type) {
            d.concat(&[d.text("("), element_doc, d.text(")"), suffix_doc])
        } else {
            d.concat(&[element_doc, suffix_doc])
        }
    }

    /// Everything after an array type's element: the element→`[` gap and the `[]`
    /// pair.
    ///
    /// The single suffix emitter for all four routes through
    /// [`Self::build_array_type_doc`] — bare element, synthesized parens, expanded
    /// parenthesized union, and paren-interior freeze. Each supplies its own closing
    /// `)`; what follows it is this one question, so a commented suffix can't survive
    /// on one route and be dropped on another (it was dropped on three of them).
    ///
    /// The element→`[` gap can hold only a **single-line block** comment: a `//` or a
    /// multiline block puts a line break in front of the `[`, and a type's array
    /// suffix may not follow one, so the construct stops being an array type at all
    /// (§Comment relocation's array-type entry carries the rule and its
    /// indexed-access sibling). That is why `CommentSpacing::Leading`'s missing tail
    /// separator cannot swallow the suffix here, the way it can at an ordinary
    /// pre-token gap. The bracket pair routes through the shared empty-brackets
    /// emitter the empty tuple type uses, so `[/* c */]` and a `//`-forced break are
    /// decided in one place for both bracket forms.
    ///
    /// The gap is measured from the element's own span end, which for a
    /// source-parenthesized element is already past its `)` — so a comment the author
    /// wrote *inside* the parens belongs to the element's doc, and only what follows
    /// the `)` lands here. An element the author left bare (`typeof x /* c */[]`)
    /// has no source `)` in the region at all: the comment simply follows the one the
    /// printer synthesizes.
    fn build_array_suffix_doc(&self, arr: &TSArrayType<'_>) -> DocId {
        let d = self.d();
        match self.array_suffix_layout(arr) {
            ArraySuffixLayout::Fused => d.text("[]"),
            ArraySuffixLayout::Split { bracket_open } => d.concat(&[
                self.build_inline_comments_between_doc(arr.element_type.span().end, bracket_open),
                self.build_empty_brackets_inline_with_comments_doc_range(
                    bracket_open,
                    arr.span.end,
                ),
            ]),
        }
    }

    /// How an array type renders its `[]` suffix.
    ///
    /// One question, one predicate, for two callers that must agree: whether
    /// [`Self::build_array_type_doc`] splits its fused suffix, and whether the
    /// type-alias `=` gate counts the suffix as an internal break point
    /// (`type_has_internal_breaking`). Answering them apart is the disagreement
    /// that gate's `Parenthesized` arm already records — the same `(…)[]` hugging
    /// `=` on a width-driven break and hanging after it on a comment-driven one.
    /// Returning the `[` position rather than a bool is what keeps the emitter
    /// from needing a second, silently-dropping fallback when the scan declines.
    ///
    /// **On-page**: a layout gate (the bracket body's group decides
    /// break-vs-inline), so an emit-keyed answer would blind it — same axis note
    /// as [`Self::build_tuple_type_doc`]. Comment-free input answers `Fused`
    /// before the byte scan, so the suffix stays one unsplit `text()`.
    ///
    /// Whether the element takes parens is a **separate** question, asked by the
    /// builder alone (`type_needs_parens_for_array_element`): the suffix region
    /// starts past the element's span either way, so folding paren-ness in here only
    /// ever excluded routes from the split — which is how three of them came to drop
    /// their comment.
    pub(in crate::printer) fn array_suffix_layout(
        &self,
        arr: &TSArrayType<'_>,
    ) -> ArraySuffixLayout {
        let elem_end = arr.element_type.span().end;
        if self.has_comments_on_page_between(elem_end, arr.span.end)
            // The `[` is FOUND, never computed from `arr.span.end - 2`: the brackets
            // may hold a comment (`string[/* c */]`) and the gap before them
            // whitespace or another comment, so the arithmetic form lands mid-token
            // (`span_arithmetic_needs_byte_check`).
            && let Some(bracket_open) = find_char_skipping_comments(
                self.source.as_bytes(),
                elem_end as usize,
                (arr.span.end as usize).min(self.source.len()),
                b'[',
            )
        {
            return ArraySuffixLayout::Split {
                bracket_open: bracket_open as u32,
            };
        }
        ArraySuffixLayout::Fused
    }

    //
    // Type Query and Entity Names
    //

    /// Build doc for type query expression name
    pub(super) fn build_type_query_expr_name_doc(
        &self,
        expr_name: &internal::TSTypeQueryExprName<'_>,
    ) -> DocId {
        match expr_name {
            internal::TSTypeQueryExprName::EntityName(entity) => self.build_entity_name_doc(entity),
            // `typeof import(...)` — identical to `TSType::Import`, including comment
            // preservation around the specifier, qualifier, and type arguments.
            internal::TSTypeQueryExprName::Import(i) => self.build_import_type_doc(i),
        }
    }
}
