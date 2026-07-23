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
use super::{BlankRule, CommentSpacing, Printer};
use crate::ast::internal::{
    self, TSArrayType, TSConditionalType, TSMappedType, TSMappedTypeModifier, TSTupleType, TSType,
};
use crate::printer::CommentVec;
use crate::printer::analysis::has_newline_after_position;
use crate::printer::layout::hang_after_operator;
use smallvec::smallvec;
use tsv_lang::INDENT;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

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

        let needs_breaking = has_breaking_comments_around_question
            || has_breaking_comments_after_colon
            || has_trailing_line_comment_on_true;

        if needs_breaking {
            return self.build_conditional_type_doc_with_line_comments(c);
        }

        // Build true_type doc: if it's a conditional (possibly wrapped in parens), don't wrap in group
        // Add parens for readability only when flat (single-line), not when broken (multi-line)
        let true_type_doc = if let TSType::Conditional(inner) = unwrap_parenthesized(c.true_type) {
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
        };

        // Build false_type doc: if it's a conditional, don't wrap in group
        // No parens needed for nested conditionals in false position (right-associative)
        let false_type_doc = if let TSType::Conditional(inner) = unwrap_parenthesized(c.false_type)
        {
            self.build_conditional_type_doc_inner(inner)
        } else {
            self.build_type_doc(c.false_type)
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
        branch_doc: DocId,
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
    fn build_conditional_branch_tail_doc(
        &self,
        branch_type: &TSType<'_>,
        branch_doc: DocId,
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
        // TODO: the Union/Intersection arms below rebuild their doc bare and ignore
        // `branch_doc`, so the caller's pre-built `build_type_doc` subtree is dead
        // for those branches — build the branch docs lazily (or match on the shape
        // before building) to drop the double build
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
                if on_new_line {
                    d.concat(&[d.hardline(), d.text(INDENT), branch_doc])
                } else {
                    d.concat(&[d.text(" "), self.prepend_opt(run, branch_doc)])
                }
            }
            _ => {
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
            let value_doc = self.build_type_doc_maybe_parens(
                c.extends_type,
                type_needs_parens_for_conditional_extends,
            );
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

        // Special case: parenthesized extends_type with a leading line comment
        // inside the parens (`extends (// c\n  b)`, or the double-nested
        // `((// c\n  b))`). Strip EVERY redundant layer, build the fully-unwrapped
        // inner type, and append the line comment(s) as trailing on it — matching
        // prettier's relocation. The deep window catches a comment hiding one layer
        // in, which the shallow paren-own-gap window missed (non-idempotent).
        if self.stripped_paren_has_leading_line_comment(c.extends_type) {
            let inner = unwrap_parenthesized(c.extends_type);
            let mut parts: DocBuf = smallvec![d.text(" "), comments_after_extends];
            parts.push(self.build_type_doc(inner));
            for comment in self.stripped_paren_leading_line_comments(c.extends_type) {
                parts.push(self.build_trailing_line_comment_doc(comment));
            }
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
    fn build_conditional_type_doc_with_line_comments(&self, c: &TSConditionalType<'_>) -> DocId {
        let d = self.d();

        let extends_type_end = c.extends_type.span().end;
        let true_type_start = c.true_type.span().start;
        let true_type_end = c.true_type.span().end;
        let false_type_start = c.false_type.span().start;

        // Detect leading line comments inside parens around true_type / false_type
        // for relocation: prettier moves them to trail extends_type / true_type
        // (e.g., `extends b ? (// c\n  C) : D` → `extends b // c\n  ? C\n  : D`).
        let true_paren_leading_line_comments: CommentVec<'_> =
            self.stripped_paren_leading_line_comments(c.true_type);
        let false_paren_leading_line_comments: CommentVec<'_> =
            self.stripped_paren_leading_line_comments(c.false_type);

        // Build branch type docs (same nested-conditional logic as non-breaking path).
        // When we relocated leading line comments from a parenthesized wrapper (any
        // nesting depth), build the fully-unwrapped inner type directly so the
        // relocated comments aren't emitted twice.
        let true_type_doc = if !true_paren_leading_line_comments.is_empty() {
            self.build_type_doc(unwrap_parenthesized(c.true_type))
        } else if let TSType::Conditional(inner) = unwrap_parenthesized(c.true_type) {
            self.build_conditional_type_doc_inner(inner)
        } else {
            self.build_type_doc(c.true_type)
        };

        let false_type_doc = if !false_paren_leading_line_comments.is_empty() {
            self.build_type_doc(unwrap_parenthesized(c.false_type))
        } else if let TSType::Conditional(inner) = unwrap_parenthesized(c.false_type) {
            self.build_conditional_type_doc_inner(inner)
        } else {
            self.build_type_doc(c.false_type)
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

        // Split comments around the `?` token by position so trailing line
        // comments on extends_type (e.g., `b // comment\n? c`) stay on
        // extends_type's line rather than being relocated past `?`.
        let q_pos = self.find_char_outside_comments(extends_type_end, true_type_start, b'?');
        let (before_q_end, after_q_start) = match q_pos {
            Some(q) => (q, q + 1),
            None => (true_type_start, extends_type_end),
        };

        // Comments BEFORE the `?` token — emit as trailing on extends_type
        // (before the hardline that ends extends_type's line). Also includes
        // relocated leading line comments from inside true_type's parens.
        let mut trailing_on_extends_parts: DocBuf = DocBuf::new();
        for comment in comments_to_emit_in_range(self.comments, extends_type_end, before_q_end) {
            trailing_on_extends_parts.push(self.build_trailing_comment_doc(comment));
        }
        for comment in &true_paren_leading_line_comments {
            trailing_on_extends_parts.push(self.build_trailing_line_comment_doc(comment));
        }

        let mut q_parts = DocBuf::new();

        // ? on new line
        q_parts.push(d.hardline());
        q_parts.push(d.text("?"));

        // Comments AFTER the `?` token — emit between `?` and the true branch.
        let needs_indent_before_true =
            self.push_conditional_branch_gap_comments(&mut q_parts, after_q_start, true_type_start);
        q_parts.push(self.build_conditional_branch_tail_doc(
            c.true_type,
            true_type_doc,
            needs_indent_before_true,
            None,
        ));

        // Comments trailing on true_type (between true_type and :) — preserve position.
        // Also includes relocated leading line comments from inside false_type's parens.
        let colon = self.find_char_outside_comments(true_type_end, false_type_start, b':');
        if let Some(c_pos) = colon {
            for comment in comments_to_emit_in_range(self.comments, true_type_end, c_pos) {
                q_parts.push(self.build_trailing_comment_doc(comment));
            }
        }
        for comment in &false_paren_leading_line_comments {
            q_parts.push(self.build_trailing_line_comment_doc(comment));
        }

        // : on new line
        q_parts.push(d.hardline());
        q_parts.push(d.text(":"));

        // Comments after : only (between : and false_type)
        let colon_end = colon.map_or(true_type_end, |c| c + 1);
        let needs_indent_before_false =
            self.push_conditional_branch_gap_comments(&mut q_parts, colon_end, false_type_start);
        q_parts.push(self.build_conditional_branch_tail_doc(
            c.false_type,
            false_type_doc,
            needs_indent_before_false,
            None,
        ));

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

    //
    // Mapped Types
    //

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

        interior_parts
            .push(self.ident_name_doc(m.type_parameter.name, m.type_parameter.span.start));
        // Comments around `in` keyword: `key /* c1 */ in /* c2 */ Constraint`
        let name_len =
            self.with_ident_name_at(m.type_parameter.name, m.type_parameter.span.start, str::len);
        let name_end = m.type_parameter.span.start + name_len as u32;
        let constraint_start = m.type_parameter.constraint.span().start;
        // Find `i` of `in` keyword, skipping comments before it
        let in_start = find_char_skipping_comments(
            self.source.as_bytes(),
            name_end as usize,
            constraint_start as usize,
            b'i',
        );
        let in_end = in_start.map_or(name_end, |p| (p + "in".len()) as u32);
        let in_start = in_start.map_or(name_end, |p| p as u32);
        // Comments between key name and `in` keyword
        // Comment gaps break a line comment onto its own line so it can't swallow the
        // following `in`/constraint.
        interior_parts.push(self.build_leading_comments_break_for_line(name_end, in_start));
        interior_parts.push(d.text(" in "));
        interior_parts.push(self.build_trailing_comments_hang_next(in_end, constraint_start));
        interior_parts.push(self.build_type_doc(m.type_parameter.constraint));

        // as clause: `as NewKeyType`
        // Track the end of the last element inside brackets (for bracket-close comments)
        let mut last_inner_end = m.type_parameter.constraint.span().end;
        if let Some(name_type) = &m.name_type {
            // Comments around `as` keyword: `Constraint /* c1 */ as /* c2 */ NewKey`
            let constraint_end = m.type_parameter.constraint.span().end;
            let name_type_start = name_type.span().start;
            // Find `a` of `as` keyword, skipping comments before it
            let as_start = find_char_skipping_comments(
                self.source.as_bytes(),
                constraint_end as usize,
                name_type_start as usize,
                b'a',
            );
            let as_end = as_start.map_or(constraint_end, |p| (p + "as".len()) as u32);
            let as_start = as_start.map_or(constraint_end, |p| p as u32);
            // Comment gaps break a line comment so it can't swallow `as`/the name type.
            interior_parts
                .push(self.build_leading_comments_break_for_line(constraint_end, as_start));
            interior_parts.push(d.text(" as "));
            interior_parts.push(self.build_trailing_comments_hang_next(as_end, name_type_start));
            interior_parts.push(self.build_type_doc(name_type));
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

        // optional modifier: `?`, `+?`, or `-?`
        if let Some(optional) = m.optional {
            body_parts.push(d.text(match optional {
                TSMappedTypeModifier::True => "?",
                TSMappedTypeModifier::Plus => "+?",
                TSMappedTypeModifier::Minus => "-?",
            }));
        }

        // Comments and value type
        if let Some(type_ann) = &m.type_annotation {
            let type_start = type_ann.span().start;
            let type_end = type_ann.span().end;

            // Comments between `]` (or `?`/`+?`/`-?`) and value type
            // Start from bracket_close to avoid double-counting pre-bracket comments
            let comments_before_value: CommentVec<'_> =
                comments_to_emit_in_range(self.comments, bracket_close, type_start).collect();

            body_parts.push(d.text(":"));

            // A redundant paren shell with a leading line-comment run (`]: (// c\n V)`)
            // strips to the same hang as bare `]: // c\n V`; route it through the shared
            // keyword→value seam so the paren form is idempotent (the outer paren would
            // otherwise hide the comment from the gate). A mixed / trailing shell hoists
            // losslessly too — the trailing comment via `build_hang_value_doc`.
            let (value_start, value_type) = self.keyword_value_stripped_paren_hang(type_ann);
            // A line comment after `:` stays trailing it, with the value type on
            // the next line (preserve-in-place; prettier relocates the comment to
            // trail the member `;`).
            if self.has_line_comments_between(bracket_close, value_start) {
                // Type position: a trailing block lifted from the shell trails the value
                // inline before the member `;` (`defer = false`).
                let value_doc = self.build_hang_value_doc(type_ann, value_type, false);
                self.append_keyword_value_line_comments(
                    &mut body_parts,
                    bracket_close,
                    value_start,
                    value_doc,
                );
            } else {
                for comment in &comments_before_value {
                    body_parts.push(d.text(" "));
                    body_parts.push(self.build_comment_doc(comment));
                }

                // A union/intersection value breaks after `:` and hangs (leading `| `
                // for unions, indented continuations for intersections) instead of
                // gluing to the colon when it exceeds print width — matching prettier's
                // `shouldIndent` → `indent(parts)`. Redundant comment-free parens around
                // the value are stripped first (prettier does the same). A hugging union
                // (`{ ... } | null`) keeps its inline `: ` since the object owns its own
                // expansion.
                match self.unwrap_redundant_parens(type_ann) {
                    TSType::Union(u) => {
                        let type_doc = self.build_union_type_doc(u);
                        // A hugging union (`{ ... } | null`) keeps its inline `: ` since the
                        // object owns its own expansion; everything else hangs after `:` so
                        // it breaks to leading `| ` instead of gluing. `union_prints_hugged`
                        // owns that question whole — this site used to pair the bare
                        // syntactic shape with its own NARROWER comment scan (line comments
                        // between members only), which let a block comment between members,
                        // or a line comment in the leading `|`→first-member gap, read as
                        // "hug" while the printer expanded them.
                        if self.union_prints_hugged(u) {
                            body_parts.push(d.text(" "));
                            body_parts.push(type_doc);
                        } else {
                            body_parts.push(hang_after_operator(d, type_doc));
                        }
                    }
                    TSType::Intersection(i) => {
                        body_parts.push(d.text(" "));
                        body_parts.push(self.intersection_hanging_with_indent(i));
                    }
                    _ => {
                        body_parts.push(d.text(" "));
                        body_parts.push(self.build_type_doc(type_ann));
                    }
                }
            }

            // Trailing comments after the value type. A block comment trails
            // inline before the `;` (`V /* c */;`); a line comment goes through
            // `line_suffix` (`build_trailing_comment_doc`) so it floats to
            // end-of-line *after* the `;` (`V; // c`) instead of swallowing it —
            // the `;` is emitted separately by the multiline/one-line branch below.
            let body_end = m.span.end.saturating_sub(1); // before `}`
            for comment in comments_to_emit_in_range(self.comments, type_end, body_end) {
                body_parts.push(self.build_trailing_comment_doc(comment));
            }
        } else {
            // No value type (`{ [K in T] }`): comments after the `]` (or the
            // optional modifier) still trail the member the same way — dropping
            // through without collecting them would lose content.
            let body_end = m.span.end.saturating_sub(1); // before `}`
            for comment in comments_to_emit_in_range(self.comments, bracket_close, body_end) {
                body_parts.push(self.build_trailing_comment_doc(comment));
            }
        }

        if source_is_multiline {
            // Multi-line source: preserve multi-line format with hardlines.
            // Own-line leading comments each take their own line before `[`; the
            // node-adjacent inline block comment (if any) already leads `body_parts`.
            let mut inner_parts: DocBuf = smallvec![];
            for comment in &leading_comments[..leading_own_line_end] {
                inner_parts.push(d.hardline());
                inner_parts.push(self.build_comment_doc(comment));
            }
            inner_parts.push(d.hardline());
            inner_parts.push(d.concat(&body_parts));
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
            for comment in &leading_comments[..leading_own_line_end] {
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

    /// Build a Doc for a tuple type: `[A, B, C]`
    ///
    /// Uses width-aware breaking: inline if fits, one element per line if not.
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
        if !self.has_comments_to_emit_between(t.span.start, t.span.end) {
            let mut parts = DocBuf::new();
            for (i, elem) in t.element_types.iter().enumerate() {
                if i > 0 {
                    parts.push(d.text(","));
                    parts.push(d.line());
                }
                parts.push(self.build_type_doc(elem));
            }
            let inner = d.concat(&[d.softline(), d.concat(&parts)]);
            return d.group(d.concat(&[d.text("["), d.indent(inner), d.softline(), d.text("]")]));
        }

        // Check for comments that force expansion: line comments, multiline block comments,
        // or own-line single-line block comments. Also check for line comments BEFORE the
        // first element (between `[` and first element), e.g., `[// leading\n a, b]`.
        let has_leading_line_comment = t.element_types.first().is_some_and(|first| {
            self.has_line_comments_between(t.span.start + 1, first.span().start)
        });
        if has_leading_line_comment
            || self.has_line_comments_in_delimited_list(
                t.element_types,
                TSType::span,
                t.span.end - 1,
            )
            || self.has_own_line_block_comments_in_bracket_list(
                t.span,
                t.element_types,
                TSType::span,
            )
        {
            return self.build_tuple_type_doc_with_line_comments(t);
        }

        // Build element docs with commas, inline block comments, and line breaks
        let mut parts = DocBuf::new();
        let mut prev_end = t.span.start + 1; // After opening `[`
        // Block comment trailing the last element after its source comma — preserved
        // past where the comma was (no trailing comma; prettier relocates before; see
        // conformance_prettier.md §Comment relocation).
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

            parts.push(self.build_type_doc(elem));

            let elem_end = elem.span().end;
            prev_end = if i + 1 < t.element_types.len() {
                let next_start = t.element_types[i + 1].span().start;
                let comma_pos = self.find_list_comma(elem_end, next_start);
                self.append_trailing_inline_block_comments(&mut parts, elem_end, comma_pos);
                comma_pos + 1 // After comma
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
        // comma; trailingComma: 'none').
        let inner = d.concat(&[d.softline(), d.concat(&parts), d.concat(&last_after_comma)]);

        d.group(d.concat(&[d.text("["), d.indent(inner), d.softline(), d.text("]")]))
    }

    /// Build tuple type with expanding comments (line comments or own-line block comments)
    fn build_tuple_type_doc_with_line_comments(&self, t: &TSTupleType<'_>) -> DocId {
        let d = self.d();
        // A comment trailing the opening `[` on its own line is kept on the `[`
        // line when the tuple expands (divergence from prettier, which relocates
        // it to its own line as the first element's leading comment). A
        // line/own-line comment is itself what forces this path. Tuple types have
        // no elision, so the first element is always present. See
        // conformance_prettier.md §Comment relocation (Tuple type `[`).
        let first_elem_start = t.element_types[0].span().start;
        let (bracket_line_prefix, delimiter_pull_pos) =
            self.delimiter_line_comment_prefix(t.span.start, first_elem_start);

        let mut inner_parts = DocBuf::new();
        let mut prev_end = t.span.start + 1; // After the opening `[`

        for (i, elem) in t.element_types.iter().enumerate() {
            let elem_start = elem.span().start;
            let elem_end = elem.span().end;
            let is_last = i == t.element_types.len() - 1;

            // Leading comments (after previous comma or `[`). For the first
            // element, drop comments pulled onto the `[` line (emitted as the
            // bracket-line prefix below).
            let skip_delim = if i == 0 { delimiter_pull_pos } else { None };
            let leading = self.build_leading_comments_multiline(prev_end, elem_start, skip_delim);
            inner_parts.push(self.build_list_element_group(leading, self.build_type_doc(elem)));

            if !is_last {
                let next_start = t.element_types[i + 1].span().start;
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
            return d.concat(&[union_doc, d.text("[]")]);
        }
        let needs_parens = type_needs_parens_for_array_element(arr.element_type);
        let element_doc = self.build_type_doc(arr.element_type);
        if needs_parens {
            d.concat(&[d.text("("), element_doc, d.text(")[]")])
        } else {
            d.concat(&[element_doc, d.text("[]")])
        }
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
