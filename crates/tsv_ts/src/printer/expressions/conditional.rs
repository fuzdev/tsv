// Conditional (ternary) expression printing for TypeScript
//
// Handles: a ? b : c, nested ternaries, comments in ternaries

use crate::ast::internal;
use crate::printer::{CommentVec, Printer, template_literal_has_newlines};
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::{INDENT, Span};

/// Check if an expression is a nullish coalescing expression (`??`)
///
/// Prettier wraps `??` in parens when inside a ternary for clarity.
fn is_nullish_coalescing(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr,
        internal::Expression::BinaryExpression(bin)
            if bin.operator == internal::BinaryOperator::QuestionQuestion
    )
}

/// A ternary consequent/alternate that gets clarity parens (prettier:
/// needs-parentheses.js, the `ConditionalExpression` parent case). `as`/`satisfies`
/// and an assignment bind tighter than `?:` so the parens are pure clarity (same
/// AST); `??` is always parenthesized under a conditional. Shared by the inline and
/// line-comment layouts so both branch paths agree.
fn ternary_branch_needs_parens(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr,
        internal::Expression::TSAsExpression(_)
            | internal::Expression::TSSatisfiesExpression(_)
            | internal::Expression::AssignmentExpression(_)
    ) || is_nullish_coalescing(expr)
}

/// A ternary TEST that gets parens (prettier: needs-parentheses.js). For arrow/yield
/// it is **semantic** — without parens the body absorbs the ternary (`() => 1 ? x : y`
/// parses as `() => (1 ? x : y)`; `yield 1 ? x : y` as `yield (1 ? x : y)`); for
/// `as`/`satisfies`/assignment/`??` it is clarity (same AST, they bind tighter than
/// `?:`). Shared by the inline and line-comment layouts so both agree — the
/// line-comment path must not drop the semantic arrow/yield parens.
fn ternary_test_needs_parens(expr: &internal::Expression<'_>) -> bool {
    is_nullish_coalescing(expr)
        || matches!(
            expr,
            internal::Expression::AssignmentExpression(_)
                | internal::Expression::AwaitExpression(_)
                | internal::Expression::ArrowFunctionExpression(_)
                | internal::Expression::YieldExpression(_)
                | internal::Expression::TSAsExpression(_)
                | internal::Expression::TSSatisfiesExpression(_)
        )
}

/// Where a ternary branch's value sits relative to the comment run in its gap —
/// [`Printer::emit_ternary_branch_comments`]'s answer, spent by
/// [`Printer::push_ternary_branch_value`].
///
/// A named pair rather than a `(bool, bool)`: the two flags have the same type and are read
/// at **two** gaps (`?`→consequent, `:`→alternate), so a positional tuple is one
/// transposition away from hanging a value that should trail and keeping a blank that should
/// collapse — a swap the compiler cannot see and every fixture in the file would still pass
/// on one of the two gaps.
#[derive(Clone, Copy)]
struct TernaryBranchPlacement {
    /// The value drops below the run: a comment in the gap can't share its line (a line
    /// comment, a later own-line comment, or a blank before the value).
    on_own_line: bool,
    /// The author left a blank line between the run and the value, which survives when the
    /// value takes its own line.
    blank_before: bool,
}

/// Check if an expression is a template literal containing newlines
///
/// When a template literal contains embedded newlines in its quasi strings,
/// it should be treated as "multiline" for formatting purposes. This is used
/// to force ternaries to break when their consequent or alternate is multiline.
fn is_multiline_template_literal(expr: &internal::Expression<'_>) -> bool {
    matches!(expr, internal::Expression::TemplateLiteral(t) if template_literal_has_newlines(t))
}

/// Prettier's `shouldExtraIndentForConditionalExpression` (`print/ternary.js`), asked
/// top-down instead of bottom-up.
///
/// Prettier walks **up** from a ternary through the wrappers that keep it on the left
/// spine — a member object, a call/`new` callee, a chain element, a non-null `!`, an
/// instantiation — stepping once past a binary cast (`as` / `satisfies`), and asks
/// whether it lands on one of a fixed set of value positions (`ancestorNameMap`:
/// assignment RHS, declarator init, `return`/`throw`/`await`/`yield`/unary argument).
/// tsv builds top-down and has no ancestor path, so the same question is asked by
/// walking **down** from the value and stripping the same wrappers.
///
/// Returns the ternary's span when its enclosing parens should **expand**
/// (`(⏎\tcond ? a : b⏎) as T`) rather than hang (`(cond⏎\t? a⏎\t: b) as T`).
///
/// `None` when the value **is** the ternary — prettier's `child === node` guard. That
/// guard is not an edge case to smooth over: it is exactly why a bare
/// `!(cond ? a : b)` keeps the hanging form in both formatters, while
/// `!((cond ? a : b) as T)` expands.
fn extra_indent_ternary_span(value: &internal::Expression<'_>) -> Option<Span> {
    let (span, stepped) = spine_ternary(value)?;
    stepped.then_some(span)
}

/// The ternary at the bottom of `expr`'s left-spine, plus whether reaching it took at
/// least one step. Shared by [`extra_indent_ternary_span`] (which requires a step) and
/// the chain-base query (which does not — a *sealed* base like `(c ? a : b)!` holds the
/// wrapper inside its own parens, so the ternary is already one step down when the base
/// is handed over).
fn spine_ternary(expr: &internal::Expression<'_>) -> Option<(Span, bool)> {
    let mut child = expr;
    let mut stepped = false;
    loop {
        let next = match child {
            internal::Expression::ConditionalExpression(cond) => {
                return Some((cond.span, stepped));
            }
            internal::Expression::MemberExpression(m) => m.object,
            internal::Expression::CallExpression(c) => c.callee,
            internal::Expression::NewExpression(n) => n.callee,
            internal::Expression::TSNonNullExpression(n) => n.expression,
            internal::Expression::TSAsExpression(a) => a.expression,
            internal::Expression::TSSatisfiesExpression(s) => s.expression,
            internal::Expression::TSInstantiationExpression(i) => i.expression,
            _ => return None,
        };
        stepped = true;
        child = next;
    }
}

/// Where a conditional sits relative to an enclosing conditional — the axis prettier's
/// `printTernaryOld` keys a nested ternary's TEST geometry on (`printTernaryTest` +
/// `printBranch`). The `?`/`:` lines land one level past the parent's either way; only
/// the test's continuation differs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TernaryNesting {
    /// Not a branch of another conditional: the ternary groups itself and indents its own
    /// `?`/`:` lines.
    Root,
    /// A parent's consequent — `a ? (b ? c : d) : e`. Prettier's `printBranch` indents the
    /// whole nested ternary under the `? `, so its test continues one level past the `?`.
    Consequent,
    /// A parent's alternate — the chain `a ? b : c ? d : e`. `printBranch` indents it the
    /// same way and `printTernaryTest` adds an `align(2)` under the `: `, so the test
    /// continues one level plus two columns past the `:`; a call's arguments inside it
    /// flush that align to a whole tab (two levels), its `)` back at the two-column offset.
    Alternate,
}

impl TernaryNesting {
    /// Nested in another conditional's branch (prettier's `forceNoIndent` /
    /// `parent === firstNonConditionalParent` axis): no group of its own, and the parent's
    /// break decision cascades.
    fn is_chained(self) -> bool {
        self != Self::Root
    }
}

/// What a chain base's left-spine ternary wants from the parens around it — see
/// [`Printer::chain_base_ternary`].
#[derive(Clone, Copy)]
pub(in crate::printer) struct ChainBaseTernary {
    /// The parens expand onto their own lines (`(⏎\tc ? a : b⏎).prop`).
    pub expands: bool,
    /// The base IS the ternary, so the member is its direct parent.
    pub direct: bool,
}

impl<'a> Printer<'a> {
    /// Record whether a ternary reached through `value`'s wrapper spine should take the
    /// expanded-paren layout — call from the value positions in prettier's
    /// `ancestorNameMap` (see [`extra_indent_ternary_span`]).
    ///
    /// Keyed by span and **not** consumed, like the sibling paren targets
    /// (`expr_stmt_paren_target`): a chain that rebuilds its base across
    /// conditional-group variants must answer the same way every time, and a
    /// same-shaped ternary nested deeper (a call argument, an array element) never
    /// matches the recorded span, so it keeps the hanging form prettier gives it.
    pub(in crate::printer) fn mark_ternary_extra_indent(&self, value: &internal::Expression<'_>) {
        self.ternary_hang_target
            .set(extra_indent_ternary_span(value));
    }

    /// Does `expr` need the expanded-paren layout its parent is about to wrap it in?
    ///
    /// Asked at each site that supplies a ternary's parens (a binary cast's operand, a
    /// non-null assertion's operand), against the span
    /// [`Self::mark_ternary_extra_indent`] recorded.
    pub(in crate::printer) fn ternary_takes_extra_indent(
        &self,
        expr: &internal::Expression<'_>,
    ) -> bool {
        matches!(expr, internal::Expression::ConditionalExpression(cond)
            if self.ternary_hang_target.get() == Some(cond.span))
    }

    /// The chain-base form of [`Self::ternary_takes_extra_indent`]: does this base's
    /// left-spine bottom out in a ternary, and if so is it the marked one?
    ///
    /// A ternary base that must NOT expand is the answer the plain query cannot give,
    /// because a chain base may arrive already wrapped: a sealed `(c ? a : b)!` base
    /// keeps the `!` inside its own parens, so the base node is the non-null, not the
    /// ternary.
    ///
    /// `direct` reports whether the base IS the ternary, which decides the non-expanding
    /// shape: prettier's `breakClosingParen` fires on a **member** parent, so a direct
    /// ternary base drops its `)` to its own line (`(c⏎\t? a⏎\t: b⏎).prop`) while a
    /// wrapped one keeps the `)` welded to the last arm (`(c⏎\t? a⏎\t: b)!.prop`) — there
    /// the ternary's parent is the `!`, not the member.
    pub(in crate::printer) fn chain_base_ternary(
        &self,
        expr: &internal::Expression<'_>,
    ) -> Option<ChainBaseTernary> {
        let (span, stepped) = spine_ternary(expr)?;
        Some(ChainBaseTernary {
            expands: self.ternary_hang_target.get() == Some(span),
            direct: !stepped,
        })
    }

    /// Wrap a ternary consequent/alternate doc in clarity parens when its `expr`
    /// needs them. The single seam both layouts (inline + line-comment) route
    /// through, so a branch can't get parenthesized in one and bare in the other.
    ///
    /// `shell_boundary` is the branch's trailing-gap end when `doc` came from
    /// [`Printer::build_ternary_branch_expr_doc`] — the builder that RETAINS the grouping
    /// shell around a line / own-line comment and so supplies the pair itself. "Did the
    /// callee already wrap?" is one question with one answer
    /// ([`Printer::shell_value_keeps_own_parens`]); spelling it only here was how the
    /// alternate came to print `((x = y // c))`, two pairs where the author wrote one, at
    /// every branch kind that takes clarity parens.
    ///
    /// It is asked with the `position_parens` the **builder received** (`false` — see
    /// `build_ternary_branch_expr_doc`), never with `ternary_branch_needs_parens`: the
    /// two are different questions here, and answering with the latter would report a
    /// same-line block comment's shell "kept" when that arm actually defers the comment
    /// past the `;` and prints no pair at all, dropping the branch's clarity parens.
    ///
    /// `None` at the line-comment layout, whose branches are built by a plain
    /// `build_expression_doc` that never retains a shell — there the gap's comment is
    /// emitted outside the pair by the branch's own trailing-gap scan.
    fn parenthesize_ternary_branch(
        &self,
        expr: &internal::Expression<'_>,
        doc: DocId,
        shell_boundary: Option<u32>,
    ) -> DocId {
        if !ternary_branch_needs_parens(expr) {
            return doc;
        }
        if shell_boundary.is_some_and(|end| self.shell_value_keeps_own_parens(expr, end, false)) {
            return doc;
        }
        self.d().parens(doc)
    }

    /// Whether either BRANCH gap holds an honored directive — the layout gate's half of
    /// [`Self::frozen_ternary_branch_doc`]. It locates the `?` / `:` itself because the gate
    /// runs one function earlier than the breaking layout that resolves those positions for
    /// its own emitters.
    ///
    /// Deliberately keyed on the `?`→consequent and `:`→alternate gaps rather than on the
    /// whole `test`→`alternate` span the sibling gates scan: a directive in the *operand*
    /// gaps (test→`?`, consequent→`:`) freezes nothing, so treating one as a branch directive
    /// would break a ternary open for a freeze that never fires. Behind the document-level
    /// flag, so a directive-free document never pays for the two operator scans.
    fn ternary_branch_gap_frozen(&self, cond: &internal::ConditionalExpression<'_>) -> bool {
        if !self.has_format_ignore {
            return false;
        }
        let question = self.find_char_outside_comments(
            cond.test.span().end,
            cond.consequent.span().start,
            b'?',
        );
        let colon = self.find_char_outside_comments(
            cond.consequent.span().end,
            cond.alternate.span().start,
            b':',
        );
        self.frozen_ternary_branch_span(cond.consequent, question)
            .is_some()
            || self
                .frozen_ternary_branch_span(cond.alternate, colon)
                .is_some()
    }

    /// The span an honored directive in a ternary branch's operator→value gap freezes,
    /// `None` where nothing does — the one spelling of that question, so the layout gate
    /// ([`Self::ternary_branch_gap_frozen`]) and the emitter
    /// ([`Self::frozen_ternary_branch_doc`]) cannot drift into two answers.
    fn frozen_ternary_branch_span(
        &self,
        expr: &internal::Expression<'_>,
        op_pos: Option<u32>,
    ) -> Option<Span> {
        op_pos.and_then(|p| self.value_head_frozen_span(p + 1, expr.span()))
    }

    /// The frozen doc for a ternary branch whose operator→value gap holds an honored
    /// directive ([`Printer::value_head_frozen_span`]), `None` where nothing freezes.
    ///
    /// The one seam both branches route through, so the `?`→consequent and `:`→alternate
    /// heads cannot drift apart — the same reason [`Self::push_ternary_branch_value`] exists
    /// one question down. It goes through [`Self::parenthesize_ternary_branch`] like every
    /// other branch doc: the clarity parens are the POSITION's, so they belong outside the
    /// frozen slice, and the `None` shell boundary is the line-comment layout's own (its
    /// branches never retain a shell — the gap's comment is emitted outside the pair by the
    /// branch's own trailing-gap scan).
    ///
    /// A frozen branch takes the ordinary branch's indent even where it is a nested
    /// conditional: the chain structure that arm exists to preserve is gone once the branch
    /// renders as a verbatim slice.
    fn frozen_ternary_branch_doc(
        &self,
        expr: &internal::Expression<'_>,
        op_pos: Option<u32>,
    ) -> Option<DocId> {
        let frozen = self.frozen_ternary_branch_span(expr, op_pos)?;
        Some(self.parenthesize_ternary_branch(
            expr,
            self.build_frozen_expression_doc(expr, frozen),
            None,
        ))
    }

    /// Wrap a ternary test doc in parens when its `expr` needs them (arrow/yield are
    /// load-bearing — see `ternary_test_needs_parens`). The shared seam for both
    /// layouts, mirroring `parenthesize_ternary_branch`.
    fn parenthesize_ternary_test(&self, expr: &internal::Expression<'_>, doc: DocId) -> DocId {
        if ternary_test_needs_parens(expr) {
            self.d().parens(doc)
        } else {
            doc
        }
    }

    /// Give a nested ternary's TEST the geometry its position takes ([`TernaryNesting`]).
    /// The shared seam for both layouts, applied to the finished test doc — parens, the
    /// for-init `in` shell and the stripped-paren comments included, since prettier's
    /// `print("test")` carries all of those inside its `align`.
    ///
    /// tsv keeps a nested ternary's `?`/`:` lines under its OWN `indent` rather than under
    /// the parent branch's (`build_conditional_doc_impl`), which lands them where prettier's
    /// do — but leaves the test at the parent's level, one indent short of prettier's
    /// `printBranch`. This is that indent, plus the alternate's `align(2)`
    /// (`ternary/nested_test_long`).
    fn place_nested_ternary_test(&self, nesting: TernaryNesting, test: DocId) -> DocId {
        let d = self.d();
        match nesting {
            TernaryNesting::Root => test,
            TernaryNesting::Consequent => d.indent(test),
            TernaryNesting::Alternate => d.indent(d.align(2, test)),
        }
    }

    /// Build a Doc for a conditional expression — the default layout.
    ///
    /// The paired [`Self::build_conditional_doc_with_binary_test_indent`] is the same
    /// thing with `indent_binary_test`, which is the only axis between them.
    pub(in crate::printer) fn build_conditional_doc(
        &self,
        cond: &internal::ConditionalExpression<'_>,
    ) -> DocId {
        self.build_conditional_doc_impl(cond, TernaryNesting::Root, false)
    }

    /// Build a Doc for a conditional expression in return/throw/call/new context.
    ///
    /// When the ternary's parent is ReturnStatement, ThrowStatement, CallExpression,
    /// or NewExpression, binary expressions in the test position use continuation
    /// indent. This matches Prettier's shouldNotIndent (binaryish.js:109-113) which
    /// exempts binaries from indent only when the grandparent is NOT one of these types.
    pub(in crate::printer) fn build_conditional_doc_with_binary_test_indent(
        &self,
        cond: &internal::ConditionalExpression<'_>,
    ) -> DocId {
        self.build_conditional_doc_impl(cond, TernaryNesting::Root, true)
    }

    /// Implementation of conditional doc building
    ///
    /// `nesting` says whether this conditional is a branch of a parent conditional, and
    /// which ([`TernaryNesting`]). A nested one takes no group of its own, so the parent's
    /// break decision cascades to it, and its test takes the position's geometry.
    ///
    /// `indent_binary_test` indicates the ternary is inside a return/throw/call/new
    /// statement, so binary expressions in the test AND branch positions use continuation
    /// indent (matching Prettier's shouldNotIndent = false for these grandparents). It
    /// applies to THIS ternary's own positions only: a nested ternary's binaries have the
    /// outer ternary for a grandparent, never the return or the call, so the recursion
    /// below always passes `false` (`ternary/nested_binary_branch_long`).
    fn build_conditional_doc_impl(
        &self,
        cond: &internal::ConditionalExpression<'_>,
        nesting: TernaryNesting,
        indent_binary_test: bool,
    ) -> DocId {
        let d = self.d();
        let is_chained = nesting.is_chained();
        let test_end = cond.test.span().end;
        let consequent_start = cond.consequent.span().start;
        let consequent_end = cond.consequent.span().end;
        let alternate_start = cond.alternate.span().start;

        // Check for line comments that force breaking
        let has_line_comments = self.has_line_comments_between(test_end, consequent_start)
            || self.has_line_comments_between(consequent_end, alternate_start);

        // An honored directive in a BRANCH gap forces the break too, in both spellings: the
        // breaking layout is the only one that can keep the directive's own line, and a
        // directive glued to the operator is inert under the placement floor — so the inline
        // layout would freeze the branch on this pass and lose the freeze on the next.
        let has_branch_directive = self.ternary_branch_gap_frozen(cond);

        // A branch-gap comment separated from its value by a blank line forces the
        // break too — prettier breaks on `a ? /* c */⏎⏎b` even though an own-line
        // block comment with no blank stays inline (`a ? /* c */⏎b`). Scan the whole
        // test→consequent / consequent→alternate ranges (the `?`/`:` sit before the
        // gap comments, so the blank-after-comment check is unaffected by them).
        let has_blank_separated_comment = self
            .comment_followed_by_blank(test_end, consequent_start)
            || self.comment_followed_by_blank(consequent_end, alternate_start);

        // Check for multiline template literals in test, consequent, or alternate
        // Template literals with embedded newlines should force the ternary to break,
        // even though those newlines don't appear in the doc structure.
        let has_multiline_template = is_multiline_template_literal(cond.test)
            || is_multiline_template_literal(cond.consequent)
            || is_multiline_template_literal(cond.alternate);

        // If there are line comments, a blank-separated branch comment, or multiline
        // template literals, use a breaking layout. Other block comments after ? or :
        // are handled inline in the non-breaking path.
        if has_line_comments
            || has_branch_directive
            || has_blank_separated_comment
            || has_multiline_template
        {
            return self.build_conditional_doc_with_line_comments(cond, nesting);
        }

        // Prettier's shouldNotIndent (binaryish.js:109-113) exempts binaries whose
        // parent is ConditionalExpression from continuation indent, UNLESS the
        // grandparent is ReturnStatement, ThrowStatement, CallExpression, or
        // NewExpression. In those cases, shouldNotIndent = false and the binary
        // gets indent(rest) for its continuation lines. The caller keys the axis
        // (context-free: a template call arg indents its test the same as a
        // `<script>` one; at a template expression ROOT the generic
        // `build_conditional_doc` passes false, matching the plugin's flush test).
        let test = if indent_binary_test {
            // The term does not fire, so the ordinary dispatch's continuation-indent
            // default is already the answer.
            self.build_expression_doc(cond.test)
        } else {
            // shouldNotIndent = true (grandparent is assignment, variable, etc.) — an
            // explicit opt-out, through the seam that owns the owned-comment prepend so
            // the two arms cannot disagree about it.
            self.build_flat_chain_expression_doc(cond.test)
        };
        // Several test-position expressions get parens (Prettier: needs-parentheses.js).
        // See `ternary_test_needs_parens` for the arrow/yield semantics vs the
        // `as`/`satisfies`/assignment/`??` clarity cases.
        let test = self.parenthesize_ternary_test(cond.test, test);
        // Parenthesize an `in` test inside a for-header init (`for (a = (b in c) ? …;…)`);
        // a no-op elsewhere. The test is `[~In]`, so the parens are load-bearing.
        let test = self.wrap_for_init_in(cond.test, test);
        // Comments the parser stripped along with the test's grouping parens
        // (`(/* c */ a ?? b) ? x : y`) live in the gap between the conditional's own
        // start (the removed `(`) and the test's — no other emitter covers it, so
        // without this the comment is silently dropped. Outside the re-added parens,
        // matching every other operand position. A no-op when the test wasn't
        // parenthesized (the two starts coincide).
        let test =
            self.prepend_removed_paren_comments(cond.span.start, cond.test.span().start, test);
        let test = self.place_nested_ternary_test(nesting, test);
        // Prettier's shouldNotIndent (binaryish.js:109-113) also applies to binaries
        // in consequent/alternate positions: when parent is ConditionalExpression and
        // grandparent is ReturnStatement/ThrowStatement/CallExpression/NewExpression,
        // shouldNotIndent = false → binary gets indent(rest) for continuation lines.
        // In assignment/variable contexts, shouldNotIndent = true → flat (no indent).
        // Bound the consequent's own paren-comment scan at its end — the
        // consequent-to-`:` comment is emitted by `comments_before_colon` below, so a
        // wider boundary would double-emit it.
        let consequent =
            self.build_ternary_branch_expr_doc(cond.consequent, indent_binary_test, consequent_end);

        // Split comments around ? and : operators.
        // Comments before ? go after test, comments after ? go before consequent,
        // comments after : go before alternate. These positions only bound the comment
        // scans below, so a ternary with no comment anywhere skips both position scans.
        let ternary_has_comments = self.has_comments_to_emit_between(test_end, alternate_start);
        let (question_pos, colon_pos) = if ternary_has_comments {
            (
                self.find_char_outside_comments(test_end, consequent_start, b'?'),
                self.find_char_outside_comments(consequent_end, alternate_start, b':'),
            )
        } else {
            (None, None)
        };

        // The operator-leading comment slots (before `?`, before `:`) are empty on
        // the comment-free common path — each gap ⊆ [test_end, alternate_start], so no
        // comment there means every slot builds to `empty()`. Build them only when the
        // ternary span carries a comment; otherwise skip the redundant per-gap scan (the
        // `test_end → consequent_start` one below runs even with no `?` position) and the
        // four empty children in the `inner` concat. Byte-identical: empty slots render to
        // nothing, so the lean concat is the same output.
        let comment_slots = ternary_has_comments.then(|| {
            // Comments between test and ?
            let comments_before_question = if let Some(q) = question_pos {
                self.build_inline_comments_between_doc(test_end, q)
            } else {
                self.build_inline_comments_between_doc(test_end, consequent_start)
            };

            // Comments between consequent and : (e.g., `b ? c /* comment */ : d`)
            let comments_before_colon = if let Some(c) = colon_pos {
                self.build_inline_comments_between_doc(consequent_end, c)
            } else {
                d.empty()
            };

            (comments_before_question, comments_before_colon)
        });

        // Branch-gap comment runs (`? /* c */ b`, `: /* c */ c`): each comment's
        // separator is keyed on the source after it — glued stays glued, an authored
        // break becomes a collapsible line that holds while the ternary is broken and
        // yields when it is flat (`build_branch_comment_run`). A nested-conditional
        // branch levels itself, so its soft separator shifts only the first line
        // (`indent(line)`); a normal branch's run rides inside the branch's own
        // structural indent with a bare `line`, so the value and its continuations
        // land one level past the operator.

        // Handle nested conditional in consequent specially:
        // - When flat: parens for parsing `a ? (b ? c : d) : e`
        // - When broken: continue chain without parens (same as alternate)
        //
        // Prettier wraps each branch in indent() so that multiline content
        // (like arrow block bodies) gets proper nesting. Exception: nested
        // conditionals handle their own indentation, so no extra wrapper.
        let consequent_doc = if let internal::Expression::ConditionalExpression(nested) =
            cond.consequent
        {
            let run = question_pos.and_then(|q| {
                self.build_branch_comment_run(q + 1, consequent_start, d.indent(d.line()))
            });
            // Broken version: continue chain without parens. The nested ternary's own
            // binaries have THIS ternary for a grandparent, so `indent_binary_test` stops
            // here.
            let broken_consequent =
                self.build_conditional_doc_impl(nested, TernaryNesting::Consequent, false);
            let broken_consequent = self.prepend_opt(run, broken_consequent);
            if d.will_break(consequent) {
                // Consequent forces breaking (e.g., line comments produce hardlines).
                // Skip if_break and use broken layout directly — the outer group
                // will break because broken_consequent contains hardlines.
                // Matches Prettier's willBreak(consequentDoc) → shouldBreak check
                // in printTernaryOld (ternary-old.js).
                broken_consequent
            } else {
                // Normal if_break: parens when flat, chain when broken
                let flat_consequent = self.prepend_opt(run, d.parens(consequent));
                d.if_break(broken_consequent, flat_consequent)
            }
        } else {
            let run = question_pos
                .and_then(|q| self.build_branch_comment_run(q + 1, consequent_start, d.line()));
            let branch =
                self.parenthesize_ternary_branch(cond.consequent, consequent, Some(consequent_end));
            d.indent(self.prepend_opt(run, branch))
        };

        // Handle nested conditional in alternate: continue the chain
        // - Nested conditional does NOT need parens: `a ? b : c ? d : e`
        //   (right-associative, so naturally parsed as `a ? b : (c ? d : e)`)
        // - `as`/`satisfies` need parens to avoid `:` ambiguity: `a ? b : (c as T)`
        // - `??` needs parens for clarity: `a ? b : (c ?? d)`
        let alternate_doc = if let internal::Expression::ConditionalExpression(nested) =
            cond.alternate
        {
            let run = colon_pos.and_then(|c| {
                self.build_branch_comment_run(c + 1, alternate_start, d.indent(d.line()))
            });
            // Recursively build as chained (no group wrapper, no parens). No indent
            // wrapper — the nested conditional indents its own `?`/`:` lines and places
            // its test for this position. `indent_binary_test` stops here (see above).
            let nested_doc =
                self.build_conditional_doc_impl(nested, TernaryNesting::Alternate, false);
            self.prepend_opt(run, nested_doc)
        } else {
            let run = colon_pos
                .and_then(|c| self.build_branch_comment_run(c + 1, alternate_start, d.line()));
            let alternate = self.build_ternary_branch_expr_doc(
                cond.alternate,
                indent_binary_test,
                cond.span.end,
            );
            let branch =
                self.parenthesize_ternary_branch(cond.alternate, alternate, Some(cond.span.end));
            d.indent(self.prepend_opt(run, branch))
        };

        let inner = if let Some((comments_before_question, comments_before_colon)) = comment_slots {
            d.concat(&[
                test,
                comments_before_question,
                d.indent(d.concat(&[
                    d.line(),
                    d.text("? "),
                    consequent_doc,
                    comments_before_colon,
                    d.line(),
                    d.text(": "),
                    alternate_doc,
                ])),
            ])
        } else {
            // Comment-free common path: no comment slots, so omit the four empty children.
            d.concat(&[
                test,
                d.indent(d.concat(&[
                    d.line(),
                    d.text("? "),
                    consequent_doc,
                    d.line(),
                    d.text(": "),
                    alternate_doc,
                ])),
            ])
        };

        // If chained (nested in another conditional), don't wrap in group
        // This allows the parent's break decision to cascade
        if is_chained { inner } else { d.group(inner) }
    }

    /// Build a conditional expression doc when there are line comments
    ///
    /// Line comments force the ternary to break because they end at newline.
    /// This produces:
    /// ```js
    /// test // comment
    ///   ? // comment
    ///     consequent // comment
    ///   : // comment
    ///     alternate
    /// ```
    fn build_conditional_doc_with_line_comments(
        &self,
        cond: &internal::ConditionalExpression<'_>,
        nesting: TernaryNesting,
    ) -> DocId {
        let d = self.d();
        let test_end = cond.test.span().end;
        let consequent_start = cond.consequent.span().start;
        let consequent_end = cond.consequent.span().end;
        let alternate_start = cond.alternate.span().start;

        // Build test expression with parens if needed — the same seam as the
        // non-breaking path, so the load-bearing arrow/yield parens (and the
        // `as`/`satisfies` clarity parens) are never dropped just because a branch
        // carries a line comment.
        let test = self.parenthesize_ternary_test(cond.test, self.build_expression_doc(cond.test));
        // Parenthesize an `in` test inside a for-header init (`for (a = (b in c) ? …;…)`);
        // a no-op elsewhere. The test is `[~In]`, so the parens are load-bearing.
        let test = self.wrap_for_init_in(cond.test, test);
        // Stripped-grouping-paren comments on the test — see the sibling in
        // `build_conditional_doc`; both layouts must emit them or the comment is lost.
        let test =
            self.prepend_removed_paren_comments(cond.span.start, cond.test.span().start, test);
        let test = self.place_nested_ternary_test(nesting, test);

        // Find the ? and : positions for proper comment categorization
        let question_pos = self.find_char_outside_comments(test_end, consequent_start, b'?');
        let colon_pos = self.find_char_outside_comments(consequent_end, alternate_start, b':');

        let mut parts = smallvec![test];

        // Comments between test and ? (see split_pre_operator_comments): same-line
        // comments trail the test, later-line comments precede the `?` on their own
        // lines.
        let comments_before_q_end = question_pos.unwrap_or(consequent_start);
        let mut pre_question_own_line = DocBuf::new();
        self.split_pre_operator_comments(
            test_end,
            comments_before_q_end,
            &mut parts,
            &mut pre_question_own_line,
        );

        // Start the indented part: own-line pre-? comments, then ? on a new line
        let mut q_parts = pre_question_own_line;
        q_parts.push(d.hardline());
        q_parts.push(d.text("?"));

        // Comments between ? and consequent: first trails `?` inline, later ones take
        // their own indented line (author blanks preserved). The placement's
        // `on_own_line` is set when a comment can't share the consequent's line (the
        // blank, if any, is preserved below).
        let consequent_placement =
            self.emit_ternary_branch_comments(&mut q_parts, question_pos, consequent_start);

        // Consequent expression — when the outer ternary enters breaking layout
        // (line comments or multiline templates), nested conditionals in the
        // consequent must also break. Without group_break, the inner ternary's
        // group stays flat (content fits on one line), but Prettier cascades
        // the break from the parent to the entire ternary chain.
        // The `?`→consequent value head: an own-line directive in the gap freezes the whole
        // branch ([`Self::frozen_ternary_branch_doc`]).
        let (consequent, is_nested_cond) =
            if let Some(frozen) = self.frozen_ternary_branch_doc(cond.consequent, question_pos) {
                (frozen, false)
            } else if let internal::Expression::ConditionalExpression(nested) = cond.consequent {
                let chained =
                    self.build_conditional_doc_impl(nested, TernaryNesting::Consequent, false);
                (d.group_break(chained), true)
            } else {
                // Clarity parens (`(a ?? b)`, `(x as T)`) exactly as the inline layout
                // applies them — the line-comment path must not drop them.
                let expr_doc = self
                    .wrap_for_init_in(cond.consequent, self.build_expression_doc(cond.consequent));
                (
                    self.parenthesize_ternary_branch(cond.consequent, expr_doc, None),
                    false,
                )
            };
        // A nested conditional handles its own indent via its chained structure;
        // any other consequent hangs one level deeper (its own multiline content
        // then aligns with the main layout, whether it sits on its own line after a
        // comment or trails a single block comment / bare `?`).
        let placed_consequent = if is_nested_cond {
            consequent
        } else {
            d.indent(consequent)
        };
        self.push_ternary_branch_value(&mut q_parts, consequent_placement, placed_consequent);

        // Comments between consequent and :. Mirrors the test→? handling above
        // (same shared helper): same-line comments trail the consequent, later-line
        // comments precede the `:` on their own lines — both flow into q_parts in
        // source order (trailing run first, then own-line run).
        let comments_before_colon_end = colon_pos.unwrap_or(alternate_start);
        let mut colon_own_line = DocBuf::new();
        self.split_pre_operator_comments(
            consequent_end,
            comments_before_colon_end,
            &mut q_parts,
            &mut colon_own_line,
        );
        q_parts.append(&mut colon_own_line);

        // : on new line
        q_parts.push(d.hardline());
        q_parts.push(d.text(":"));

        // Comments between : and alternate — same shape as the ?→consequent gap.
        let alternate_placement =
            self.emit_ternary_branch_comments(&mut q_parts, colon_pos, alternate_start);

        // Alternate expression - nested conditionals cascade the break without extra indent
        // The `:`→alternate value head, the consequent's mirror.
        let alternate_doc =
            if let Some(frozen) = self.frozen_ternary_branch_doc(cond.alternate, colon_pos) {
                d.indent(frozen)
            } else if let internal::Expression::ConditionalExpression(nested) = cond.alternate {
                // Recursively use breaking layout - no indent wrapper (has its own structure)
                self.build_conditional_doc_with_line_comments(nested, TernaryNesting::Alternate)
            } else {
                // Regular expressions get indent wrapper, plus the same clarity parens
                // the inline layout applies (`(a ?? b)`, `(x as T)`).
                let expr_doc = self
                    .wrap_for_init_in(cond.alternate, self.build_expression_doc(cond.alternate));
                d.indent(self.parenthesize_ternary_branch(cond.alternate, expr_doc, None))
            };

        self.push_ternary_branch_value(&mut q_parts, alternate_placement, alternate_doc);

        // The alternate's OWN trailing gap — everything between the alternate's inner end
        // and the ternary's end. Nothing else scans it, and the consequent's twin gap is
        // covered only by accident: the `:`-gap scan above runs
        // `[consequent_end, alternate_start]`, which already spans the consequent's
        // stripped paren shell and any comment inside it. The alternate has no following
        // gap, so without this the region is emitted by nobody and the comment is DROPPED
        // (`c ? a : (⏎// c⏎b // t⏎)` lost `// t` entirely). The two scans partition the
        // construct — see docs/comments.md §Trailing and dangling runs.
        //
        // The range is keyed on the alternate's own span, which for a parenthesized branch
        // stops INSIDE the stripped shell, so the shell's `)` and anything the author put
        // before it fall in here rather than nowhere
        // (docs/comments.md §A stripped-paren interior is a partition too). A nested
        // conditional alternate emits its own tail recursively and its span ends where
        // this one does, leaving the range empty.
        self.push_trailing_comments_in_range(
            &mut q_parts,
            cond.alternate.span().end,
            cond.span.end,
        );

        parts.push(d.indent(d.concat(&q_parts)));

        d.concat(&parts)
    }

    /// Emit the comments between a ternary operator (`?` or `:`) and its branch value
    /// into `parts`: the first trails the operator inline (`? /* c */`), each later one
    /// takes its own indented line (author blanks preserved). Shared by the
    /// ?→consequent and :→alternate gaps.
    ///
    /// Returns the branch's [`TernaryBranchPlacement`]: the value drops onto its own line
    /// when a comment can't share it — a line comment, a later own-line comment, or a blank
    /// line before the value — and that blank survives below the run. Spending the answer is
    /// [`Self::push_ternary_branch_value`]'s job, not the caller's.
    fn emit_ternary_branch_comments(
        &self,
        parts: &mut DocBuf,
        op_pos: Option<u32>,
        value_start: u32,
    ) -> TernaryBranchPlacement {
        let d = self.d();
        let comments: CommentVec<'_> = op_pos
            .map(|p| self.comments_to_emit_between(p + 1, value_start).collect())
            .unwrap_or_default();
        let mut has_line_comment = false;
        let mut last_own_line = false;
        for (i, comment) in comments.iter().enumerate() {
            // An HONORED directive keeps the line the author gave it, wherever in the run it
            // sits: sharing a line with the operator (or with the comment before it) is the
            // placement the floor classifies as inert, and the freeze it earns would be lost
            // on the second pass. The same rule `Printer::build_header_comment_run` states at
            // the declaration headers and [`Printer::comment_hangs_next`] states for what
            // follows a comment — the emitter never relocates a directive.
            let directive = self.is_honored_directive(comment);
            if i == 0 && directive {
                parts.push(d.hardline());
                parts.push(d.text(INDENT));
                last_own_line = true;
            } else if i == 0 {
                // First comment trails the operator inline (`? /* c */`).
                parts.push(d.text(" "));
            } else if !directive
                && self.trailing_run_hugs_previous(Some(comments[i - 1]), comment.span.start)
            {
                // Glued to the previous comment — keep the line the author wrote them on,
                // and take no INDENT: the run did not start a new line to indent onto
                // ([`Printer::trailing_run_hugs_previous`], the rule every comment run
                // reads). `last_own_line` stays as it was for the same reason.
                parts.push(d.text(" "));
            } else {
                // Subsequent comments take their own line (author blank preserved).
                self.push_blank_preserving_hardline(
                    parts,
                    comments[i - 1].span.end,
                    comment.span.start,
                );
                parts.push(d.text(INDENT));
                last_own_line = true;
            }
            parts.push(self.build_comment_doc(comment));
            if !comment.is_block {
                has_line_comment = true;
            }
        }
        // The same question [`Printer::push_blank_preserving_hardline`] answers three lines
        // above for the run's own separators, so it takes the same spelling: the STRICT
        // scan, never the table-only newline count. `value_start` is the branch's span
        // start, which for a parenthesized branch lies INSIDE the stripped shell, so the
        // `(` the printer erases sits between the comment and it — and counting newlines
        // reads that `(`'s two line breaks as an author blank (`a ? /* c */⏎(⏎b⏎) : c`
        // grew one, and since the blank also feeds the break gate below, the whole ternary
        // came open). A leading run measures forward from the previous comment
        // (`printLeadingComment`'s `skipNewline` + `hasNewline`), which lands on the `(`
        // and reports no blank — exactly what the strict reading says.
        //
        // ⚠️ And it takes the in-source CEILING as well as the strict reading, for the
        // mirror reason: `value_start` is an expression start, so a block comment the
        // author glued to the branch is OWNED — inside the gap, printed by the branch's
        // own doc, skipped by the run above — and scanning across it reads its interior
        // newlines as an author blank. That answer also feeds the break gate below, so the
        // fabricated blank drops the branch onto its own line too.
        let blank_before_value = comments.last().is_some_and(|c| {
            self.has_blank_line_between_strict(
                c.span.end,
                self.blank_scan_end_after(c, value_start),
            )
        });
        TernaryBranchPlacement {
            on_own_line: has_line_comment || last_own_line || blank_before_value,
            blank_before: blank_before_value,
        }
    }

    /// Emit a ternary branch's separator and its value — the one place a
    /// [`TernaryBranchPlacement`] is spent, for both the `?`→consequent and `:`→alternate
    /// gaps.
    ///
    /// The two gaps had this shape open-coded twice, which is the same re-derivation the
    /// blank rule itself was paying one level down (`docs/comments.md` §A gap emitter that
    /// re-derives the BLANK rule): a later change to how a branch hangs lands on one gap and
    /// not its mirror, and the tell — tsv disagreeing with itself between symmetric
    /// positions — is exactly what this file's own bugs have looked like.
    fn push_ternary_branch_value(
        &self,
        parts: &mut DocBuf,
        placement: TernaryBranchPlacement,
        value: DocId,
    ) {
        let d = self.d();
        if placement.on_own_line {
            // A comment can't share the value's line — the value takes a new one, below any
            // blank the author left above it.
            if placement.blank_before {
                parts.push(d.literalline());
            }
            parts.push(d.hardline());
            parts.push(d.text(INDENT));
        } else {
            // A single block comment, or none at all — a space, and the value trails it.
            parts.push(d.text(" "));
        }
        parts.push(value);
    }

    /// Split the comments in a ternary operand→operator gap into trailing vs
    /// own-line docs, shared by the test→`?` and consequent→`:` sites.
    ///
    /// A comment on the operand's own source line trails it (a block stays inline
    /// with its width counted; a line comment uses `line_suffix`, zero width, so a
    /// long trailing comment never forces a binary operand to break — see
    /// `test_trailing_long_comment`) and is pushed to `trailing`. A comment the
    /// author placed on a *later* line drops to its own line, aligned with the
    /// operator it precedes, and is pushed to `own_line` (a `d.hardline()` then the
    /// comment). A `//` ends its line, so a same-line run trails at most one line
    /// comment; everything after it already starts on a later line.
    ///
    /// This preserves the author's "before the operator" placement — prettier
    /// instead relocates later-line comments across the operator — and never merges
    /// consecutive line comments onto the operand line, which would reverse their
    /// order and fuse them into one node (the property-signature `// c2 // c1`
    /// quirk, here in a ternary). The two before-operator sites share this helper
    /// so they cannot drift apart (the original merge bug was exactly such a drift
    /// from the correct after-operator handling).
    // The same-line/later-line classification is shared via
    // `tsv_lang::ClassifiedComments` (also used by `calls/arg_comments.rs`
    // PartitionedComments and the member-chain `push_gap_comments_and_break`), so the
    // "same-line trails, later-line breaks, never merge" rule lives in one place. Only
    // the emission differs per shape — operator (here) / comma / dot — which is
    // intentional (separator placement genuinely differs), not drift.
    fn split_pre_operator_comments(
        &self,
        operand_end: u32,
        gap_end: u32,
        trailing: &mut DocBuf,
        own_line: &mut DocBuf,
    ) {
        let d = self.d();
        // Same shared same-line/later-line classification as the call-argument
        // (`PartitionedComments`) and member-chain (`push_gap_comments_and_break`)
        // gap printers.
        let classified = tsv_lang::ClassifiedComments::from_index(
            self.comment_free_gap.comments(),
            self.first_index_between(operand_end, gap_end),
            operand_end,
            gap_end,
            self.source.as_bytes(),
            self.comment_line_breaks,
        );
        // Same-line comments (blocks, then the at-most-one line comment) trail the
        // operand in source order; `build_trailing_comment_doc` keeps a block inline
        // and routes a line comment through `line_suffix`.
        for &comment in classified
            .trailing_block
            .iter()
            .chain(&classified.trailing_line)
        {
            trailing.push(self.build_trailing_comment_doc(comment));
        }
        // Later-line comments drop to their own line before the operator, in source
        // order.
        for comment in classified.leading_in_source_order() {
            own_line.push(d.hardline());
            own_line.push(self.build_comment_doc(comment));
        }
    }

    /// Build expression doc for a ternary branch (consequent/alternate).
    ///
    /// A branch is one of prettier's `shouldNotIndent` positions, so a binary here is
    /// FLAT — except when `indent_binary` (the ternary is itself a return/throw/call/new
    /// value), where the term does not fire and the continuation-indent default stands.
    /// The verdict is routed by [`Printer::mark_flat_chain`], not by naming a builder;
    /// that seam says why.
    fn build_ternary_branch_expr_doc(
        &self,
        expr: &internal::Expression<'_>,
        indent_binary: bool,
        boundary_end: u32,
    ) -> DocId {
        // A branch is a `shouldNotIndent` position whenever the test is (both are
        // `parent.type === "ConditionalExpression"`, binaryish.js:109-112), so a binary here
        // takes the flat chain — except under `indent_binary`, where the term does not fire
        // and the ordinary dispatch's continuation-indent default is already the answer.
        //
        // MARKED rather than built directly: the shell below owns the gap between the value
        // and the `:` / terminator, and a builder named here would have to reach past it and
        // take that gap's comments with it (`docs/comments.md` hazard 4) — which is what the
        // indent arm used to do, for no reason of its own once the chain style stopped
        // needing a builder.
        if !indent_binary {
            self.mark_flat_chain(expr);
        }
        // `position_parens: false` — deliberately, not by oversight. It says "the
        // calling position does NOT parenthesize this value anyway", and at a branch
        // that is true of the *same-line block* case, which is the only one the flag
        // moves: a terminator-adjacent alternate defers that block past the `;` and
        // prints no pair, in tsv AND in prettier (`cond ? 0 : (b = c /* c */)` →
        // `cond ? 0 : (b = c); /* c */`, its fixed point at every branch kind that
        // takes clarity parens). Setting it would keep the comment inside instead and
        // manufacture a divergence where the two agree.
        //
        // The pair `parenthesize_ternary_branch` then adds is answered against this
        // same `false` — see that seam — so the two cannot double it.
        let doc = self.build_expression_doc_with_paren_comments(expr, boundary_end, false);
        // Parenthesize an `in` consequent/alternate inside a for-header init
        // (`for (a = c ? (b in c) : 0;…)`); a no-op elsewhere. Prettier wraps every
        // `in` under the init; the alternate is `[~In]` so there it is load-bearing.
        self.wrap_for_init_in(expr, doc)
    }
}
