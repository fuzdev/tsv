// Conditional (ternary) expression printing for TypeScript
//
// Handles: a ? b : c, nested ternaries, comments in ternaries

use crate::ast::internal;
use crate::printer::comments::ConditionalBranchPlacement;
use crate::printer::ignore::FrozenOperandPair;
use crate::printer::{Printer, template_literal_has_newlines};
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

/// A ternary TEST that gets parens (prettier: needs-parentheses.js). For arrow/yield and a
/// nested ternary it is **semantic** — without parens the body absorbs the ternary
/// (`() => 1 ? x : y` parses as `() => (1 ? x : y)`; `yield 1 ? x : y` as
/// `yield (1 ? x : y)`) and `?:` is right-associative, so a test-position ternary printed
/// bare re-binds into the enclosing one's alternate (`(a ? b : c) ? d : e` would print
/// `a ? b : c ? d : e`, which is `a ? b : (c ? d : e)`); for `as`/`satisfies`/assignment/`??`
/// it is clarity (same AST, they bind tighter than `?:`). Shared by the inline and
/// line-comment layouts so both agree — the line-comment path must not drop the semantic
/// arrow/yield parens — and by the test's FROZEN form, which takes its pair here too
/// ([`FrozenOperandPair::Emitted`] at the test's freeze seam).
pub(in crate::printer) fn ternary_test_needs_parens(expr: &internal::Expression<'_>) -> bool {
    is_nullish_coalescing(expr)
        || matches!(
            expr,
            internal::Expression::AssignmentExpression(_)
                | internal::Expression::AwaitExpression(_)
                | internal::Expression::ArrowFunctionExpression(_)
                | internal::Expression::ConditionalExpression(_)
                | internal::Expression::YieldExpression(_)
                | internal::Expression::TSAsExpression(_)
                | internal::Expression::TSSatisfiesExpression(_)
        )
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
    /// A multi-line block the test OWNS sits between the two — inside the indent, outside
    /// the align (`Printer::place_nested_ternary_test`).
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
            || self.chained_test_shell_holds_directive(cond.consequent)
            || self.chained_test_shell_holds_directive(cond.alternate)
    }

    /// Whether a branch is a nested conditional whose stripped test shell holds an honored
    /// directive — the same question the nested test's own freeze asks
    /// ([`Printer::left_spine_operand_frozen_span`]), asked here by the LAYOUT gate. The shell
    /// is this gap's to print ([`Self::branch_gap_end`]), and only the breaking
    /// layout keeps a directive on its own line: the inline layout's run collapses an own-line
    /// block onto the operator's line, where the floor calls it inert, so this pass froze the
    /// test and the next normalized it (the block-spelled chained cell of
    /// `left_spine_paren_prettier_ignore_interior`). The whole-branch freeze above does NOT
    /// fire for it — the directive's scope is the test — which is why this is a separate term.
    fn chained_test_shell_holds_directive(&self, branch: &internal::Expression<'_>) -> bool {
        match branch {
            internal::Expression::ConditionalExpression(nested) => self
                .left_spine_operand_frozen_span(nested.span.start, nested.test)
                .is_some(),
            _ => false,
        }
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
        // The ambient `for`-init `[~In]` pair, which the unfrozen branch takes one line down
        // (`wrap_for_init_in` at each branch's build site) and no `needs_parens` here
        // supplies: `parenthesize_ternary_branch` asks the branch-shape question only. Over
        // the SLICE, since a frozen branch has no inner positions of its own
        // ([`Printer::wrap_frozen_for_init_in`]) — and skipped where the branch's own pair
        // already encloses it.
        let branch_parens = ternary_branch_needs_parens(expr);
        let inner = self.wrap_frozen_for_init_in(
            expr,
            frozen,
            branch_parens,
            self.build_frozen_expression_doc(expr, frozen),
        );
        Some(self.parenthesize_ternary_branch(expr, inner, None))
    }

    /// Wrap a ternary test doc in parens when its `expr` needs them (arrow/yield are
    /// load-bearing — see `ternary_test_needs_parens`). The shared seam for both
    /// layouts, mirroring `parenthesize_ternary_branch`.
    ///
    /// A test that is itself a CONDITIONAL takes an **expanding** pair instead of a flat one
    /// — `group(["(", indent([softline, test]), softline, ")"])`
    /// ([`Printer::build_expanding_parens_doc`]) — so a test too wide for the line drops to
    /// its own indented line inside the parens rather than breaking its `?` / `:` under
    /// them. Prettier spells it from the other side, in the nested ternary's own printer
    /// (`ternary-old.js`'s `isParentTest ? group([indent([softline, result]), softline])`,
    /// with the pair supplied by `needs-parentheses.js`); the pair is tsv's own here, so the
    /// group carries it. Every other test in `ternary_test_needs_parens` — a `??`, an `=`,
    /// an `as` — hugs flat in both tools and keeps the plain pair.
    fn parenthesize_ternary_test(&self, expr: &internal::Expression<'_>, doc: DocId) -> DocId {
        if !ternary_test_needs_parens(expr) {
            return doc;
        }
        if matches!(expr, internal::Expression::ConditionalExpression(_)) {
            return self.build_expanding_parens_doc(doc);
        }
        self.d().parens(doc)
    }

    /// A ternary's TEST with every pair its position owes, around `ordinary` — the build the
    /// layout chooses for the bare test: the test's own erased-paren freeze
    /// ([`Printer::build_left_spine_operand_doc`], which also gives a frozen test the position
    /// pairs its builder would have; a paren around the whole conditional is a different
    /// authoring and lands in the enclosing head's gap, where it freezes the value whole),
    /// the test-position parens (`ternary_test_needs_parens` — the arrow/yield semantics vs
    /// the `as`/`satisfies`/assignment/`??` clarity cases, prettier's `needs-parentheses.js`)
    /// and the `[~In]` pair a for-header init requires (`for (a = (b in c) ? …;…)`, a no-op
    /// elsewhere). One seam for both layouts, so the line-comment layout cannot drop a pair the
    /// flat one applies; each layout places the result through
    /// [`Self::place_nested_ternary_test`].
    fn build_ternary_test_doc(
        &self,
        cond: &internal::ConditionalExpression<'_>,
        ordinary: impl FnOnce() -> DocId,
    ) -> DocId {
        let test = self.build_left_spine_operand_doc(
            cond.span.start,
            cond.test,
            FrozenOperandPair::Emitted,
            ordinary,
        );
        let test = self.parenthesize_ternary_test(cond.test, test);
        self.wrap_for_init_in(cond.test, test)
    }

    /// Give a nested ternary's TEST the geometry its position takes ([`TernaryNesting`]).
    /// The shared seam for both layouts, wrapped around the test's whole build — parens, the
    /// for-init `in` shell and a retained shell run included, since prettier's
    /// `print("test")` carries all of those inside its `align`.
    ///
    /// tsv keeps a nested ternary's `?`/`:` lines under its OWN `indent` rather than under
    /// the parent branch's (`build_conditional_doc_impl`), which lands them where prettier's
    /// do — but leaves the test at the parent's level, one indent short of prettier's
    /// `printBranch`. This is that indent, plus the alternate's `align(2)`
    /// (`ternary/nested_test_long`).
    ///
    /// **A multi-line block the alternate's test OWNS is claimed here, between the indent and
    /// the align.** Prettier binds a glued comment to the OUTERMOST node starting there — the
    /// nested conditional — and its `printComments` wraps the whole nested doc, so the
    /// comment sits inside `printBranch`'s indent but outside `printTernaryTest`'s `align(2)`,
    /// its continuation lines one level past the `:`. tsv's ownership is innermost-wins, so
    /// unclaimed the comment rides inside the test's doc and its `*/` lands two columns
    /// further in (`\t\t   */`), the form the gap-emitted spelling of the same comment never
    /// produces — a two-pass document (`ternary/nested_branch_multiline_block_comment`). The
    /// seam is the one every other outermost claim goes through
    /// ([`Printer::build_doc_with_outermost_owned_comment_at`], `docs/comments.md` §Owned
    /// comments); the node that would otherwise claim is the test itself, since the doc the
    /// claim wraps is the align around the test's own. The consequent has no align, so its
    /// indent alone already lands the comment where prettier's does.
    fn place_nested_ternary_test(
        &self,
        nesting: TernaryNesting,
        test: &internal::Expression<'_>,
        build_test: impl FnOnce() -> DocId,
    ) -> DocId {
        let d = self.d();
        match nesting {
            TernaryNesting::Root => build_test(),
            TernaryNesting::Consequent => d.indent(build_test()),
            TernaryNesting::Alternate => d.indent(self.build_doc_with_outermost_owned_comment_at(
                test.span().start,
                Some(test),
                || d.align(2, build_test()),
            )),
        }
    }

    /// Where a `?` / `:` → branch gap ENDS: the branch's own start, or for a nested
    /// conditional the nested TEST's start — the CHAINED conditional's test shell, the
    /// grouping parens the parser stripped from its test, which its own span still opens at
    /// (`(aaa) ? bbb : ccc` starts at the `(`, as acorn's does), belongs to the parent's gap.
    ///
    /// A comment inside that shell sits between the parent's operator and the first token the
    /// nested conditional prints, and once the shell is gone the reparse finds it in the
    /// parent's gap — so the parent's run is what lays it out, by that gap's rules (a soft
    /// separator, a pulled-up `//`), and the nested emits no shell run of its own. Emitted by
    /// the nested instead, inside the test's `align(2)` and with the shell emitter's own-line
    /// `hardline`, the comment took a form the shell-free reparse re-laid: two passes, exactly
    /// prettier's (`ternary/nested_branch_test_shell_comment`).
    ///
    /// A shell holding an honored DIRECTIVE is the parent's too, and has to be: the test's
    /// own erased-paren freeze ([`Printer::build_left_spine_operand_doc`]) is scoped to the
    /// test alone, the construct the directive precedes, so the run stays the gap's — and the
    /// parent's gap is the one emitter that keeps a directive on its own line. Emitted by the
    /// nested, the directive trailed the operator, a placement inert under tsv's floor, so the
    /// second pass normalized what the first had frozen (an F1 violation; the chained cells of
    /// `left_spine_paren_prettier_ignore_interior`). The next pass then reads the hoisted
    /// directive in the parent's gap and freezes the whole branch from it — the same coarser
    /// claim the root test's hoist reaches through the value head.
    ///
    /// Every scan over a branch gap — the line-comment and blank routing, the comment-slot
    /// gate, the run itself — reads this one bound, so none of them can see the shell's
    /// comments differently from the emitter that prints them.
    fn branch_gap_end(&self, branch: &internal::Expression<'_>) -> u32 {
        match branch {
            internal::Expression::ConditionalExpression(nested) => nested.test.span().start,
            _ => branch.span().start,
        }
    }

    /// The `[from, to)` range the breaking layout's `?` / `:`→branch gap emitter
    /// ([`Printer::push_conditional_branch_gap_run`]) covers: from just past the operator
    /// to where the gap ends. A FROZEN branch (`frozen`, [`Self::frozen_ternary_branch_doc`])
    /// is a verbatim slice from its own span start, shell and all, so its gap ends there;
    /// an unfrozen one's ends at [`Self::branch_gap_end`], past a nested conditional's
    /// stripped test shell — claiming past a frozen slice's start prints the shell's
    /// comments twice. With no operator found (`op_pos` is `None`, a defensive case: a
    /// ternary always has one) the range is empty. One spelling for both gaps, so the
    /// mirror cannot drift.
    fn breaking_branch_gap(
        &self,
        branch: &internal::Expression<'_>,
        op_pos: Option<u32>,
        frozen: bool,
    ) -> (u32, u32) {
        let to = if frozen {
            branch.span().start
        } else {
            self.branch_gap_end(branch)
        };
        (op_pos.map_or(to, |p| p + 1), to)
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
    /// indent. This matches Prettier's `shouldNotIndent` (`print/binaryish.js`) which
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
        // Where each branch gap ENDS — the branch's start, or past a nested conditional's
        // stripped test shell, whose comments this gap claims ([`Self::branch_gap_end`]).
        let consequent_start = self.branch_gap_end(cond.consequent);
        let consequent_end = cond.consequent.span().end;
        let alternate_start = self.branch_gap_end(cond.alternate);

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

        // Comments the parser stripped along with the test's grouping parens
        // (`(/* c */ a ?? b) ? x : y`) live in the gap between the conditional's own start
        // (the removed `(`) and the test's — no other emitter covers it, so without this the
        // comment is silently dropped. A no-op when the test wasn't parenthesized (the two
        // starts coincide).
        //
        // A ROOT conditional prepends the run OUTSIDE its doc, in both layouts, never onto
        // the test: an own-line `//` there carries a hardline, and inside the group that
        // hardline broke the `?` / `:` lines open for a comment that sits ahead of the whole
        // conditional (`(⏎// c⏎a as any) ? b : c` → `// c⏎(a as any)⏎\t? b⏎\t: c`), which is
        // not a fixed point — the reparse reads the comment as leading the conditional and
        // prints the group flat. Prettier's own second pass lands there for the same reason;
        // tsv gets there in one. (The breaking layout takes no group, so outside is the same
        // place as onto its test, and the one spelling here keeps the two layouts from
        // drifting.) A CHAINED conditional's shell is its parent's branch gap
        // ([`Self::branch_gap_end`]): the parent's run prints it, and the nested emits none.
        let root_run = (!is_chained)
            .then(|| self.removed_paren_comments_opt(cond.span.start, cond.test.span().start))
            .flatten();

        // If there are line comments, a blank-separated branch comment, or multiline
        // template literals, use a breaking layout. Other block comments after ? or :
        // are handled inline in the non-breaking path.
        if has_line_comments
            || has_branch_directive
            || has_blank_separated_comment
            || has_multiline_template
        {
            let doc = self.build_conditional_doc_with_line_comments(cond, nesting);
            return self.prepend_opt(root_run, doc);
        }

        // Prettier's `shouldNotIndent` (`print/binaryish.js`) exempts binaries whose
        // parent is ConditionalExpression from continuation indent, UNLESS the
        // grandparent is ReturnStatement, ThrowStatement, CallExpression, or
        // NewExpression. In those cases, shouldNotIndent = false and the binary
        // gets indent(rest) for its continuation lines. The caller keys the axis
        // (context-free: a template call arg indents its test the same as a
        // `<script>` one; at a template expression ROOT the generic
        // `build_conditional_doc` passes false, matching the plugin's flush test).
        //
        // The test's pairs and freeze are [`Self::build_ternary_test_doc`]'s; the test's
        // stripped shell run is `root_run` above.
        let test = self.place_nested_ternary_test(nesting, cond.test, || {
            self.build_ternary_test_doc(cond, || {
                if indent_binary_test {
                    // The term does not fire, so the ordinary dispatch's
                    // continuation-indent default is already the answer.
                    self.build_expression_doc(cond.test)
                } else {
                    // shouldNotIndent = true (grandparent is assignment, variable, etc.)
                    // — an explicit opt-out, through the seam that owns the owned-comment
                    // prepend so the two arms cannot disagree about it.
                    self.build_flat_chain_expression_doc(cond.test)
                }
            })
        });
        // Prettier's `shouldNotIndent` (`print/binaryish.js`) also applies to binaries
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
        // yields when it is flat (`build_branch_comment_run`). The run rides inside the
        // branch's indent at BOTH branch kinds — prettier's `printBranch` indents the whole
        // branch, comments included, and a nested conditional's run is where that shows: its
        // `?`/`:` lines level themselves, so the run's own `indent` is what puts a multi-line
        // comment's continuation lines one level past the operator, beside the nested test
        // (`ternary/nested_branch_multiline_block_comment`). A normal branch's run rides
        // inside the branch's own structural indent with the same bare `line`.

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
            let run = question_pos
                .and_then(|q| self.build_branch_comment_run(q + 1, consequent_start, d.line()))
                .map(|run| d.indent(run));
            // Broken version: continue chain without parens. The nested ternary's own
            // binaries have THIS ternary for a grandparent, so `indent_binary_test` stops
            // here.
            let broken_consequent =
                self.build_conditional_doc_impl(nested, TernaryNesting::Consequent, false);
            let broken_consequent = self.prepend_opt(run, broken_consequent);
            // The RUN's break counts too: a multi-line block comment in the gap is a hard
            // break prettier's `propagateBreaks` carries to the enclosing group, but tsv's
            // `if_break` deliberately softens a forced break in its flat arm
            // (`DocArena::subtree_layout_memo`), so asked of the arms alone the group stayed
            // flat around a comment that cannot print flat — parens kept, the comment's
            // closing line at the wrong indent.
            if d.will_break(consequent) || run.is_some_and(|run| d.will_break(run)) {
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
            let run = colon_pos
                .and_then(|c| self.build_branch_comment_run(c + 1, alternate_start, d.line()))
                .map(|run| d.indent(run));
            // Recursively build as chained (no group wrapper, no parens). No indent
            // wrapper around the nested doc — the nested conditional indents its own
            // `?`/`:` lines and places its test for this position; the run above carries
            // its own. `indent_binary_test` stops here (see above).
            // The alternate's OWN trailing gap — the interior of a stripped shell past the
            // nested conditional's end (`: (aaa ? bbb : ccc /* t */)`), which the nested
            // conditional's span stops short of and no other scan reaches (the consequent's
            // twin gap rides the `:`-gap scan above); without an emitter the comment was
            // DROPPED (`gaps:audit`). It takes the same tail every other stripped value shell
            // takes ([`Printer::build_stripped_shell_tail_doc`]): a block defers past a
            // statement `;` in one pass, as the non-nested branch's does and as prettier's
            // second pass lands it; a `//` retains the pair. Empty for a bare chain, whose two
            // ends coincide.
            let nested_doc = self.build_stripped_shell_tail_doc(
                cond.alternate,
                nested.span.end,
                cond.span.end,
                || self.build_conditional_doc_impl(nested, TernaryNesting::Alternate, false),
            );
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
        let doc = if is_chained { inner } else { d.group(inner) };
        self.prepend_opt(root_run, doc)
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

        // The test's stripped shell run is the caller's (`build_conditional_doc_impl`'s
        // `root_run`, prepended outside this doc), so this layout emits none of its own.
        let test = self.place_nested_ternary_test(nesting, cond.test, || {
            // The same seam as the non-breaking path, so the load-bearing arrow/yield parens
            // (and the `as`/`satisfies` clarity parens) are never dropped just because a
            // branch carries a line comment.
            self.build_ternary_test_doc(cond, || self.build_expression_doc(cond.test))
        });

        // Find the ? and : positions for proper comment categorization
        let question_pos = self.find_char_outside_comments(test_end, consequent_start, b'?');
        let colon_pos = self.find_char_outside_comments(consequent_end, alternate_start, b':');

        // The `?`→consequent and `:`→alternate value heads: an own-line directive in either
        // gap freezes the whole branch ([`Self::frozen_ternary_branch_doc`]). Resolved ahead
        // of the gap emitters because a frozen branch moves where its gap ENDS
        // ([`Self::breaking_branch_gap`]).
        let consequent_frozen = self.frozen_ternary_branch_doc(cond.consequent, question_pos);
        let consequent_gap =
            self.breaking_branch_gap(cond.consequent, question_pos, consequent_frozen.is_some());
        let alternate_frozen = self.frozen_ternary_branch_doc(cond.alternate, colon_pos);
        let alternate_gap =
            self.breaking_branch_gap(cond.alternate, colon_pos, alternate_frozen.is_some());

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

        // Comments between ? and consequent: first trails `?` inline (or keeps an own line
        // the author gave it beside a line comment), later ones take their own indented
        // line (author blanks preserved). The placement's
        // `on_own_line` is set when a comment can't share the consequent's line (the
        // blank, if any, is preserved below).
        let consequent_placement =
            self.push_conditional_branch_gap_run(&mut q_parts, consequent_gap.0, consequent_gap.1);

        // Consequent expression — when the outer ternary enters breaking layout
        // (line comments or multiline templates), nested conditionals in the
        // consequent must also break. Without group_break, the inner ternary's
        // group stays flat (content fits on one line), but Prettier cascades
        // the break from the parent to the entire ternary chain.
        let (consequent, is_nested_cond) = if let Some(frozen) = consequent_frozen {
            (frozen, false)
        } else if let internal::Expression::ConditionalExpression(nested) = cond.consequent {
            let chained =
                self.build_conditional_doc_impl(nested, TernaryNesting::Consequent, false);
            (d.group_break(chained), true)
        } else {
            // Clarity parens (`(a ?? b)`, `(x as T)`) exactly as the inline layout
            // applies them — the line-comment path must not drop them.
            let expr_doc =
                self.wrap_for_init_in(cond.consequent, self.build_expression_doc(cond.consequent));
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
            self.push_conditional_branch_gap_run(&mut q_parts, alternate_gap.0, alternate_gap.1);

        // Alternate expression - nested conditionals cascade the break without extra indent
        // The `:`→alternate value head, the consequent's mirror.
        let alternate_doc = if let Some(frozen) = alternate_frozen {
            d.indent(frozen)
        } else if let internal::Expression::ConditionalExpression(nested) = cond.alternate {
            // Recursively use breaking layout - no indent wrapper (has its own structure)
            self.build_conditional_doc_with_line_comments(nested, TernaryNesting::Alternate)
        } else {
            // Regular expressions get indent wrapper, plus the same clarity parens
            // the inline layout applies (`(a ?? b)`, `(x as T)`).
            let expr_doc =
                self.wrap_for_init_in(cond.alternate, self.build_expression_doc(cond.alternate));
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

    /// Emit a ternary branch's separator and its value — the one place a
    /// [`ConditionalBranchPlacement`] is spent, for both the `?`→consequent and `:`→alternate
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
        placement: ConditionalBranchPlacement,
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
        // `parent.type === "ConditionalExpression"`, `shouldNotIndent` in `print/binaryish.js`), so a binary here
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
        //
        // Skipped where this branch's shell closes the header's own CLAUSE — an alternate is
        // the branch a clause can end at — because the shell builder places that pair itself,
        // inside its trailing comment ([`Printer::shell_closes_for_clause`]).
        if self.shell_closes_for_clause(boundary_end) {
            return doc;
        }
        self.wrap_for_init_in(expr, doc)
    }
}
