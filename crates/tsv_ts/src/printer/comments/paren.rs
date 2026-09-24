// Stripped-grouping-paren comment handling.
//
// When the parser strips redundant grouping parens, comments that lived inside
// them are orphaned in the source. These helpers preserve such comments in the
// user's position — trailing the expression, promoted before `=` / an operator,
// re-added with the parens when stripping would relocate them, or prepended at a
// chain base.

use super::{CommentSpacing, CommentVec, LeadingGlue, Printer, RunLeadingBlank, ShellRunOrder};
use crate::ast::internal;
use crate::printer::ParenContext;
use crate::printer::chain::chain_paren_leading_gap;
use crate::printer::expressions::conditional::ternary_test_needs_parens;
use crate::printer::expressions::operators::SeqLayout;
use crate::printer::ignore::FrozenOperandPair;
use crate::printer::needs_parens::needs_parens;
use crate::printer::statements::TerminatorGap;
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::range_too_narrow_for_a_comment;
use tsv_lang::source_scan::{
    TriviaProfile, find_char_skipping_comments, has_newline_after_position, skip_trivia,
};

/// What follows a stripped grouping shell, and so whether a trailing block comment has a
/// statement terminator to defer past.
///
/// The distinction cannot be read off the source at the shell: both spellings put a `;`
/// there, and only the POSITION knows whether it TERMINATES a statement or SEPARATES a
/// clause. Reading the byte alone sent a `for` header's init comment past the header's
/// own separator, out of the declarator that owned it.
///
/// Two positions name the tail outright — the header's own declarator and sequence-operand
/// builders ([`Printer::build_for_init_value_doc`]) — and every other value shell asks
/// [`Printer::shell_closes_for_clause`], whose answer is a fact about the shell's own
/// BOUNDARY rather than about the value's kind or the frame it was reached through.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ShellTail {
    /// A statement value position — a declarator initializer, an assignment RHS, a
    /// `return` / `throw` argument, `export default`. A following `;` ends the statement,
    /// so a trailing block defers past it (see [`Printer::build_expression_doc_with_paren_comments`]).
    StatementTerminator,
    /// A `for` header CLAUSE's own value — the init declarator, the last operand of a
    /// clause that is a sequence, and every value whose shell closes where the clause does
    /// ([`Printer::shell_closes_for_clause`]). A following `;` separates clauses, so nothing
    /// defers.
    ForClauseSeparator,
}

/// How [`Printer::build_paren_leading_value_doc`] splits a `(`→value gap: the run the
/// `(` line keeps, the value with the rest of the run prepended, and whether the caller
/// must open its parens.
///
/// A struct rather than a tuple because the three travel together to two callers (the
/// dynamic-import EXPRESSION and the TS import TYPE) and two of them are trivially
/// swappable at a call site.
pub(crate) struct ParenLeadingValue {
    /// Emitted by the caller directly after its `(`, before any break. Empty unless a
    /// comment the author left on the `(` line was pulled onto it.
    pub paren_line: DocBuf,
    /// `value_doc` with the leading run that does *not* ride the `(` line prepended.
    pub value: DocId,
    /// Whether the gap ends a line unconditionally.
    pub forces_break: bool,
}

/// Whether a REQUIRED paren pair around `expr` — at a call callee or a tagged
/// template's tag — keeps a leading run INSIDE it.
///
/// A function or arrow is the one operand kind prettier prints the comments inside
/// those parens for (`isIifeCalleeOrTaggedTemplateExpressionTag` feeding its
/// `printCommentsForFunction`, `src/language-js/print/index.js`); every other
/// required pair at those positions — a ternary, a sequence, a cast, a class
/// expression, a `new` callee (whose parent is not a call at all) — takes the run
/// out in front, and tsv matches. Read by the three positions that print such a
/// pair: the bare callee, the tag, and the chain's IIFE base.
/// The expression's left-side child — prettier's `getLeftSide` over the node kinds its
/// `hasNakedLeftSide` names: the one node whose first token is also this node's first token
/// (`a.b`'s `a`, `a ? b : c`'s `a`, `a + b`'s `a`, `(a, b)`'s `a`, `f()`'s `f`, `` t`x` ``'s
/// `t`, `a = b`'s `a`, `a!`'s `a`). `None` for a leaf, and for every node whose left side
/// is not naked.
///
/// The gap between a node's own start and its left child's holds only the grouping `(`s
/// the parser stripped from that child — and any comment inside them
/// (`(⏎// c⏎a as any).b` opens the member at the `(`, the `as` at `a`). The structural
/// half of [`Printer::hoisted_left_side_child`], which is what every walk asks: whether
/// the printer HOISTS such a run ahead of the node is a fact about the pair, not the
/// kind, and lives there.
///
/// Prettier's walk also descends a cast's and a postfix update's operand
/// (`isBinaryCastExpression`, `UpdateExpression && !prefix`); this one does not, on
/// purpose. Those two positions answer a shell whose leading gap holds a comment
/// themselves ([`Printer::build_asi_operand_shell_doc`] — RETAINED, the cataloged "a
/// leading comment alone still keeps the shell that holds it", or printed inline ahead of
/// the operand, [`AsiOperandShell::Bare`]) and never hoist it, so they have no hoisted left
/// side at all and are excluded here rather than at the pair gate.
fn left_side_child<'e>(expr: &'e internal::Expression<'e>) -> Option<&'e internal::Expression<'e>> {
    use internal::ExpressionKind;
    Some(match &expr.kind {
        ExpressionKind::CallExpression(call) => call.callee,
        ExpressionKind::MemberExpression(member) => member.object,
        ExpressionKind::TSNonNullExpression(non_null) => non_null.expression,
        ExpressionKind::TaggedTemplateExpression(tagged) => tagged.tag,
        ExpressionKind::ConditionalExpression(cond) => cond.test,
        ExpressionKind::BinaryExpression(binary) => binary.left,
        ExpressionKind::AssignmentExpression(assign) => assign.left,
        ExpressionKind::SequenceExpression(seq) => seq.expressions.first()?,
        _ => return None,
    })
}

/// Whether the [`left_side_child`] `child` of `expr` PRINTS inside a paren pair — the
/// position half of the left-spine walk, where [`left_side_child`] is the structural half.
///
/// One table, because "which paren context does a naked left side sit in?" has one answer
/// per parent kind and two askers with opposite needs:
/// [`Printer::left_shell_paren_is_emitted`] adds a source-shell precondition and reads it as
/// "is the erased shell RE-EMITTED", while the relational-chain rule
/// (`needs_parens`'s `relational_region_opens_on_a_kept_shell`) reads it bare, as "does the
/// printed form open on a `(`", and so must see a pair the printer SYNTHESIZES too. Split
/// into two matches, the two would answer one position two ways the first time a builder's
/// context moved.
///
/// Each arm is the context the corresponding builder passes for that child — a member's
/// object is a [`ParenContext::ChainBase`], a call's callee a [`ParenContext::Callee`], and
/// so on — except the ternary test, whose pair is its own printer's question
/// ([`ternary_test_needs_parens`], shared by the inline, line-comment and frozen layouts).
/// A sequence answers `false`: its operands ride the sequence's OWN envelope, so an operand
/// never prints a pair of its own — the sequence's envelope is the CALLER's to notice.
pub(in crate::printer) fn left_side_child_is_parenthesized(
    expr: &internal::Expression<'_>,
    child: &internal::Expression<'_>,
    in_for_init: bool,
) -> bool {
    use internal::ExpressionKind;
    let ctx = match &expr.kind {
        ExpressionKind::MemberExpression(_) => ParenContext::ChainBase,
        ExpressionKind::CallExpression(_) => ParenContext::Callee,
        ExpressionKind::TaggedTemplateExpression(_) => ParenContext::TaggedTemplateTag,
        ExpressionKind::TSNonNullExpression(_) => ParenContext::NonNull,
        ExpressionKind::AssignmentExpression(_) => ParenContext::AssignmentTarget,
        ExpressionKind::BinaryExpression(binary) => ParenContext::BinaryLeft {
            parent_op: binary.operator,
        },
        ExpressionKind::ConditionalExpression(_) => return ternary_test_needs_parens(child),
        _ => return false,
    };
    needs_parens(child, ctx, in_for_init)
}

/// Where a `SequenceExpression`'s OPERANDS stop — its last operand's end, which its span
/// overshoots by the grouping shell the parser erased from that operand.
///
/// The trailing-side twin of the FIRST OPERAND's start, which the same emitters already
/// take for the leading edge: both edges lie inside the sequence's own span, so no
/// enclosing gap can see them and whoever prints the operands owes them.
///
/// ⚠️ **Not [`crate::ast::internal::Expression::printed_end`], and the two are not
/// interchangeable.** That one answers the same shape of question — where does this node's
/// doc stop printing — over a DISJOINT node set (an `AssignmentPattern`, whose span
/// swallows the shell erased from its VALUE) and for a different consumer: it is the anchor
/// every list element's trailing-comment seam takes, so widening it with this case would
/// move that anchor at every one of them. The rule for choosing: ask `printed_end` where the
/// question is "how far did this ELEMENT print?" and this where it is "where do the OPERANDS
/// of a sequence end?" — the caller that then decides whether that region is the sequence's
/// own to print, or the enclosing shell's ([`Printer::shell_trailing_gap_start`]).
///
/// Not widening `printed_end` has its own cost, paid at the list families: an array element,
/// an object value or a call argument whose value is a sequence has no emitter for the shell
/// erased from its LAST operand, so the comment written there floats out of the pair
/// (`[(x, (y /* t */))]` → `[(x, y) /* t */]`).
fn shell_content_end(expr: &internal::Expression<'_>) -> u32 {
    match &expr.kind {
        internal::ExpressionKind::SequenceExpression(seq) => seq
            .expressions
            .last()
            .map_or_else(|| expr.span().end, |last| last.span().end),
        _ => expr.span().end,
    }
}

pub(crate) fn paren_pair_keeps_leading_run(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr.kind,
        internal::ExpressionKind::ArrowFunctionExpression(_)
            | internal::ExpressionKind::FunctionExpression(_)
    )
}

/// The index of the next byte in `source` that is neither whitespace nor trivia, at or
/// after `start` and before `end`; `None` when the range holds nothing else.
///
/// The "what actually comes next in the source?" step every shell question asks. A
/// comment occupies bytes even where nothing emits it, so a walk that stepped over
/// whitespace alone would stop on the `/` and answer about the wrong token.
///
/// A free function over the source because the chain LINEARIZER asks it too, before any
/// `Printer` method is reached — [`paren_shell_close_after`].
pub(crate) fn next_significant_byte(source: &str, start: u32, end: u32) -> Option<usize> {
    let bytes = source.as_bytes();
    let end = end as usize;
    let mut i = start as usize;
    while i < end {
        if let Some(next) = skip_trivia(bytes, i, end, TriviaProfile::JS) {
            i = next;
        } else if bytes[i].is_ascii_whitespace() {
            i += 1;
        } else {
            return Some(i);
        }
    }
    None
}

/// The index just past the `)` that closes a REQUIRED pair around an operand ending at
/// `operand_end`, when the author wrote that pair — `None` when the next thing in the
/// source is something else (a `(` of an argument list, a `` ` ``, a `.`), which means
/// the pair this position prints is tsv's own and the source has no trailing gap inside
/// it.
///
/// The distinction is load-bearing: `(fn /* t */)()` writes the pair around the callee
/// and the comment is INSIDE it, while `(/* t */ fn())` writes it around the whole call
/// and the comment belongs to the call, not to a pair the callee prints.
pub(crate) fn paren_shell_close_after(source: &str, operand_end: u32) -> Option<u32> {
    let bytes = source.as_bytes();
    let pos = next_significant_byte(source, operand_end, bytes.len() as u32)?;
    (bytes[pos] == b')').then_some(pos as u32 + 1)
}

/// What an ASI-sensitive operand's grouping-paren shell comes to
/// ([`Printer::build_asi_operand_shell_doc`]).
pub(crate) enum AsiOperandShell {
    /// The shell is kept: the operand rendered inside it, both of its gaps emitted. It
    /// supplies whatever parens the operand needs, so the caller adds none.
    Shell(DocId),
    /// No shell: the caller takes its ordinary inline path. The run is the shell's
    /// leading gap when it holds a comment the bare form can still place — the caller
    /// prints it ahead of the operand, inside any pair it adds, and owes it: no other
    /// emitter covers that gap.
    Bare(Option<DocId>),
}

impl<'a> Printer<'a> {
    /// Split the comments between an opening `(` and the value that follows into the run
    /// the `(` LINE keeps and the run that LEADS the value, and report whether the caller
    /// must open its parens ([`Printer::push_leading_comment_run`]'s own report — tsv
    /// has no `propagateBreaks`, so a hardline in here is invisible to the group).
    ///
    /// ⚠️ **The `(`-line share is the call family's, and `import(…)` is a call shape.**
    /// A `//` the author parked after the `(` stays there in every other spelling —
    /// `fn( // c`, `new Foo( // c`, `obj.fn( // c`, `require( // c`, a cataloged
    /// divergence from prettier, which relocates it to the argument's leading line — and
    /// both `import(…)` spellings take the same answer (a dissent there would be an
    /// unconsidered difference, not a decision). Routing the share through
    /// [`Printer::delimiter_line_comment_prefix`] is what makes it one rule: the same
    /// pull, the same "only when the shell breaks anyway" gate, and the same
    /// pulled-comment exclusion. Pinned by
    /// `expressions/calls/import_open_paren_comment_prettier_divergence`, which carries
    /// both spellings because they share this seam.
    ///
    /// The gap is a call-argument leading run — `import(…)`'s first argument, for both
    /// the dynamic-import EXPRESSION and the TS import TYPE — so it takes the argument
    /// list's mode and the shared emitter decides its separators per comment.
    ///
    /// ⚠️ **The three-way rule cannot be approximated by a whole-range scan**, which is
    /// what this site did before: it asked "is there a newline anywhere between the `(`
    /// and this comment", so the second comment of a run the author GLUED
    /// (`import(⏎/* a */ /* b */⏎'./a')`) read as own-line, split the pair, and forced
    /// the parens open — where prettier keeps the pair and, for a run with no line
    /// comment, collapses the whole call. Own-line-ness is per comment and anchored on
    /// its own neighbours ([`Printer::is_own_line_comment`]), and the author blank the
    /// old loop's bare `hardline` deleted is likewise the emitter's to preserve.
    /// The expression's left-side child whose stripped shell's run the printer HOISTS
    /// ahead of the expression's whole doc — prettier's `getLeftSide` ([`left_side_child`])
    /// minus the nodes whose pair RETAINS its leading run, where nothing is hoisted:
    ///
    /// - a member, call or non-null whose base pair keeps the run
    ///   ([`chain_paren_leading_gap`] — the sealed optional chain, the IIFE callee, the
    ///   non-null operand that needs its parens or carries a trailing `//`);
    /// - a tagged template whose tag pair keeps it ([`paren_pair_keeps_leading_run`]);
    /// - an assignment whose target needs its parens (a type-assertion target — the
    ///   cataloged "Assignment-target shell, leading comment").
    ///
    /// Two askers, one definition: a restricted production must hold a hoisted run inside a
    /// hanging pair ([`Printer::build_restricted_production_paren_doc`]) — and must NOT wrap
    /// a second pair around one that survives — and an assignment must lay its value out
    /// under the operator so the hoisted run's hardline lands at the value's indent
    /// ([`Printer::build_assignment_layout`] and the declarator's twin). A walk that
    /// descended a retained pair told the declarator to hang a value whose run never left
    /// its shell (`const a = ( // c⏎x + y⏎)!` hung under `=`, then hugged it again on the
    /// reparse).
    pub(crate) fn hoisted_left_side_child<'e>(
        &self,
        expr: &'e internal::Expression<'e>,
    ) -> Option<&'e internal::Expression<'e>> {
        use internal::ExpressionKind;
        let child = left_side_child(expr)?;
        let retains = match &expr.kind {
            ExpressionKind::MemberExpression(_) | ExpressionKind::CallExpression(_) => {
                chain_paren_leading_gap(expr, self.comments).is_some()
            }
            // The chain's base-pair answer covers the sealed optional chain and the
            // shell a trailing `//` retains; the non-null's own builder ALSO keeps the
            // pair — as the family's expanded shell, run inside — whenever the operand
            // needs its parens (`( // c⏎x + y⏎)!`), and that pair the linearizer never
            // sees.
            ExpressionKind::TSNonNullExpression(non_null) => {
                chain_paren_leading_gap(expr, self.comments).is_some()
                    || self.needs_parens(non_null.expression, ParenContext::NonNull)
            }
            ExpressionKind::TaggedTemplateExpression(tagged) => {
                expr.span.start < tagged.tag.span().start
                    && paren_pair_keeps_leading_run(tagged.tag)
            }
            ExpressionKind::AssignmentExpression(assign) => {
                self.needs_parens(assign.left, ParenContext::AssignmentTarget)
            }
            _ => false,
        };
        (!retains).then_some(child)
    }

    /// Whether the grouping shell the parser erased between `expr` and its left-spine
    /// `child` is one the printer RE-EMITS — a pair `needs_parens` requires there.
    ///
    /// The two comment classes answer the enclosing "does this run reach the keyword's
    /// line?" question differently, and this is the whole of the difference. A `//`, or any
    /// comment on its own line, is emitted by the SHELL GAP and hoisted out in front of the
    /// pair at a cast, a ternary test and their siblings — prettier does the same and tsv
    /// matches — so a walk reading that class descends through an emitted pair. A comment
    /// the leftmost leaf OWNS does not travel: ownership prints it from that leaf's own doc,
    /// which is *inside* the pair, so a pair the printer emits holds it and it can never
    /// reach the keyword at all. A walk reading THAT class stops here
    /// ([`Self::stripped_left_side_child`]).
    ///
    /// Reading it the other way is what doubles a pair: the restricted production wraps a
    /// hanging pair around a run that never needed holding
    /// (`return (⏎ (/* a⏎b */ a as any).b⏎);` for `return (/* a⏎b */ a as any).b;`), pinned by
    /// `statements/return_throw/operand_paren_required_inner_pair_multiline_block_prettier_divergence`.
    pub(crate) fn left_shell_paren_is_emitted(
        &self,
        expr: &internal::Expression<'_>,
        child: &internal::Expression<'_>,
    ) -> bool {
        // No shell between them at all, so there is no pair to re-emit. The predicate below
        // asks only whether the CHILD prints parenthesized at its position, which is a
        // question about the printed form and not about the erased shell — the two differ
        // exactly where the printer SYNTHESIZES a pair the author never wrote, and this
        // caller wants only the re-emitted ones.
        expr.span().start != child.span().start
            && left_side_child_is_parenthesized(expr, child, self.in_for_init.get())
    }

    /// [`Self::hoisted_left_side_child`] stopped at the first shell the printer re-emits —
    /// the walk for a comment the leftmost leaf OWNS, which a pair holds rather than hoists
    /// ([`Self::left_shell_paren_is_emitted`] states the difference).
    pub(crate) fn stripped_left_side_child<'e>(
        &self,
        expr: &'e internal::Expression<'e>,
    ) -> Option<&'e internal::Expression<'e>> {
        let child = self.hoisted_left_side_child(expr)?;
        (!self.left_shell_paren_is_emitted(expr, child)).then_some(child)
    }

    /// The leftmost node an expression's doc prints FIRST, for the OWNED-comment reading:
    /// `expr` itself, or the leaf past every grouping shell the parser erased from its left
    /// spine that the printer does not re-emit.
    ///
    /// The sibling of [`Self::left_spine_printed_start`], and deliberately not the same walk
    /// — the two answer the same shape of question for the two comment classes, so they take
    /// the two step functions. That one walks [`Self::hoisted_left_side_child`], because a
    /// gap-emitted run is hoisted out in front of a pair the printer re-emits; this one walks
    /// [`Self::stripped_left_side_child`], because an owned comment is printed from inside
    /// that pair and never travels ([`Self::left_shell_paren_is_emitted`]).
    pub(crate) fn stripped_left_spine_leaf<'e>(
        &self,
        expr: &'e internal::Expression<'e>,
    ) -> &'e internal::Expression<'e> {
        let mut node = expr;
        while let Some(child) = self.stripped_left_side_child(node) {
            node = child;
        }
        node
    }

    /// Whether a stripped shell on the expression's left spine holds a comment the hoist
    /// gives a line of its own — a `//`, or a block followed by a newline — so the
    /// expression's doc opens with that run and a hardline
    /// (`(⏎// c⏎a as any) ? b : c` prints as `// c⏎(a as any) ? b : c`).
    ///
    /// Walks [`Self::hoisted_left_side_child`] to the leaf, reading each node→child gap in
    /// source. A comment in such a gap always LEADS the child (the gap holds nothing that
    /// could own a trailing comment), so the one question is whether a newline follows it —
    /// prettier's `hasLeadingOwnLineComment`, asked of every node on the way down exactly as
    /// `returnArgumentHasLeadingComment` asks it. The member object→property gap is not
    /// this walk's: a comment there is the chain's own business
    /// (`Printer::has_line_comments_in_member_chain`).
    pub(crate) fn left_spine_shell_has_own_line_comment(
        &self,
        expr: &internal::Expression<'_>,
    ) -> bool {
        let mut node = expr;
        while let Some(child) = self.hoisted_left_side_child(node) {
            let shell_start = node.span().start;
            let child_start = child.span().start;
            if shell_start < child_start
                && self
                    .comments_in_source_between(shell_start, child_start)
                    .any(|c| has_newline_after_position(self.source, c.span.end))
            {
                return true;
            }
            node = child;
        }
        false
    }

    /// Where this expression's doc STARTS PRINTING: past every stripped grouping paren on
    /// its left spine, which is the leftmost leaf [`Self::hoisted_left_side_child`] reaches.
    ///
    /// The start twin of [`internal::Expression::printed_end`], and the same difference read
    /// from the other end — a region holding nothing but the `(`s the printer erases,
    /// whitespace, and any comment inside them. Unlike `printed_end` it is not a pure AST
    /// question: whether a pair is stripped or RETAINED depends on `needs_parens` and on what
    /// the pair holds, so it is the walk that answers it, never a paren scan. A retained pair
    /// prints its own parens and keeps its run inside them, so the walk stops there and the
    /// printed start is the node's own span start.
    ///
    /// This is what an ENCLOSING gap must read as its far end. A comment in a stripped shell
    /// sits *inside* the expression's span, yet the hoist prints it ahead of the node — in
    /// exactly the position a comment in the gap *before* the span would land. A seam that
    /// bounds its scan at `span().start` is therefore blind to it and answers one question two
    /// ways by authoring (`docs/comments.md` §The left-spine shell run: the seam above the node
    /// must know the run is coming). ⚠️ For the **layout** half of that question only — an
    /// emitter must keep bounding its scan at `span().start`, since the node's own doc prints
    /// the hoisted run and claiming it again DOUBLE-PRINTS it.
    pub(crate) fn left_spine_printed_start(&self, expr: &internal::Expression<'_>) -> u32 {
        let mut node = expr;
        while let Some(child) = self.hoisted_left_side_child(node) {
            node = child;
        }
        node.span().start
    }

    pub(crate) fn build_paren_leading_value_doc(
        &self,
        open_paren_end: u32,
        value_start: u32,
        value_doc: DocId,
    ) -> ParenLeadingValue {
        // The `(`-LINE share, on the shared delimiter-line rule: pull a comment the author
        // left on the `(` line onto that line only when the shell is breaking anyway, so a
        // call that would have fit still fits (`docs/comments.md` §The delimiter-line
        // question). `pull_pos` is the exclusion every consumer of that prefix owes.
        let (paren_line, pull_pos) = match self.paren_line_share_anchor(open_paren_end, value_start)
        {
            Some(paren) => {
                let (prefix, pull_pos) = self.delimiter_line_comment_prefix(paren, value_start);
                // The field stays a buffer (its two consumers read a slice and gate on
                // `is_empty`); the `Option` collects into it only at this cold site.
                (prefix.into_iter().collect(), pull_pos)
            }
            None => (DocBuf::new(), None),
        };
        let run = self.build_leading_comment_run_with_break(
            open_paren_end,
            value_start,
            LeadingGlue::AdjacentStrippedParen,
            pull_pos,
        );
        let (value, run_breaks) = match run {
            Some((run, force_break)) => (self.d().concat(&[run, value_doc]), force_break),
            None => (value_doc, false),
        };
        ParenLeadingValue {
            // A pull is only ever made on the break path, so it reports the break itself —
            // the pulled run is no longer in `value` for the run's own report to see.
            forces_break: run_breaks || pull_pos.is_some(),
            paren_line,
            value,
        }
    }

    /// The `(` a `(`-line share would be claimed against, or `None` when this gap has no
    /// share to claim.
    ///
    /// ⚠️ **The share is what the author wrote AFTER the `(`, so the anchor is the paren
    /// and not the gap's start.** The gap deliberately spans the `(` — one slot, one
    /// emitter, which is what keeps `import /* c */ ('m')` from being dropped — but a
    /// comment written *before* the paren belongs to the head, and the claim has to stay a
    /// **PREFIX** of the gap's comments (`docs/comments.md` §The element-comma seam):
    /// claiming only the after-`(` tail would render it AHEAD of the head comment it was
    /// written under. So a run that begins before the `(` is not split — it stays whole in
    /// the leading position, which is where both formatters already put it
    /// (`import /* pre */ ( // post`, the null control in
    /// `expressions/calls/import_open_paren_comment_prettier_divergence`).
    fn paren_line_share_anchor(&self, gap_start: u32, value_start: u32) -> Option<u32> {
        let paren = find_char_skipping_comments(
            self.source.as_bytes(),
            gap_start as usize,
            value_start as usize,
            b'(',
        )? as u32;
        self.comments_to_emit_between(gap_start, value_start)
            .next()
            .is_some_and(|first| first.span.start > paren)
            .then_some(paren)
    }

    /// Append the trailing comments in an operand's closing gap to a parts vec.
    ///
    /// The gap holds comments that belong to no node, so some caller has to place them.
    /// It arises both ways round: the parser *strips* grouping parens (`await (x /* c */)`
    /// → the arg is `x`, orphaning `/* c */` before the expression's span end), and the
    /// restricted-production hanging layout *retains* them (the comment prints inside the
    /// parens, bounded by the `)` rather than the span end). Layout per comment:
    /// - Glued to the operand's line, block: inline with leading space (`x /* c */`)
    /// - Own line: deferred via `line_suffix` with a hardline, keeping an author blank
    ///   above it (`x;\n\n/* c */`) — prettier's `printTrailingComment`, its
    ///   `hasNewline(…, { backwards: true })` + `isPreviousLineEmpty` arm
    /// - Anything else: deferred via `line_suffix` on the previous comment's line
    ///   (`x; // c`, `x;\n/* c1 */ /* c2 */`)
    ///
    /// ⚠️ **The question is the SOURCE, asked per comment, and only then the kind.** The
    /// anchor advances over every comment emitted here, so the second half of a run the
    /// author glued (`x⏎/* c1 */ /* c2 */`) is not own-line and keeps that line; a fixed
    /// `argument_end` anchor read it across `c1` and split the pair. Asking the KIND
    /// first is the mirror-image formulation `docs/comments.md` §Trailing and dangling
    /// runs names: it gave an own-line `//` the inline suffix, WELDING it onto the
    /// previous comment's output line (`x; // c1 // c2` — the second delimiter becomes
    /// text inside the first, and the comment stops existing).
    ///
    /// ⚠️ **A comment glued BEHIND a deferred one is deferred too.** Deferral is what
    /// carries this run past the terminator, so an inline block emitted after the run has
    /// started renders *ahead* of it and the authored pair comes out REORDERED. Unlike
    /// [`Printer::push_trailing_comments_in_range`], where only a `//` can open the run
    /// (and nothing can follow one on its line), an own-line **block** opens it here —
    /// this gap floats own-line comments out rather than keeping them inline — so the
    /// glued-behind case is reachable and needs its own arm.
    ///
    /// Keeps a same-line block comment with its operand (before any terminator) — the
    /// expression-level operand callers (await, yield, binary, sequence) where the
    /// comment is inside the stripped operand parens, plus `export =` (which, like
    /// `import =`, keeps a same-line trailing block before the `;`).
    ///
    /// Statement terminators that move the block *after* the `;` — `export default`, and
    /// return/throw's non-hanging paths — use `split_terminator_gap_comments` instead.
    /// return/throw's hanging layout uses **both**: this method for the region inside the
    /// retained parens, then that one for anything past the `)`.
    /// Returns whether the run **deferred** — whether anything went out on a
    /// `line_suffix` rather than inline. A caller whose construct has no break of its own
    /// to flush against needs that answer: a deferred run rides to the end of the output
    /// line, and where that line ends outside the construct the comment re-binds there
    /// (the `{#snippet}` head floated a `//` past `{/snippet}`, into template text). Such
    /// a caller pushes a flush-scoped break on `true` — and only on `true`, since forcing
    /// it for an inline block breaks a construct that had no reason to open, which the
    /// reparse then closes again. Callers that already end the line — the operand shells,
    /// whose deferral lands past their own terminator — ignore it.
    pub(crate) fn append_trailing_paren_comments(
        &self,
        parts: &mut DocBuf,
        argument_end: u32,
        span_end: u32,
    ) -> bool {
        // Whether anything has been deferred yet — a `//`, or an own-line comment.
        let mut deferred_run = false;
        // What physically precedes the next comment: an **in-source** cursor, so it
        // advances over every comment in the gap (docs/comments.md §the three axes).
        let mut prev_end = argument_end;
        for comment in self.comments_to_emit_between(argument_end, span_end) {
            // The *comment* line-break table, never the layout one: this decides whether
            // a `//` is followed by a break, so it must stay real under the canonical
            // reprint, where an erased read would weld the run.
            let own_line = self.comment_has_newline_between(prev_end, comment.span.start);
            parts.push(if own_line {
                self.build_trailing_comment_doc_own_line_blank(
                    comment,
                    self.previous_line_is_empty(
                        self.blank_scan_start(prev_end, comment.span.start),
                        comment.span.start,
                    ),
                )
            } else if deferred_run || !comment.is_block {
                self.build_trailing_line_comment_doc(comment)
            } else {
                self.build_trailing_comment_doc(comment)
            });
            deferred_run |= own_line || !comment.is_block;
            prev_end = comment.span.end;
        }
        deferred_run
    }

    /// Split the trailing comments in a statement terminator's content→`;` gap
    /// the way prettier 3.9 does, returning the docs to emit **after** the `;`.
    ///
    /// A same-line **block** comment trails *after* the `;` (`return x; /* c */`) —
    /// *unless* it is still enclosed by a grouping paren around the operand that this
    /// caller PRINTS (`return (a = b /* c */);`), in which case it stays inline before
    /// the `;` (it is attached to the operand, not the statement). `operand_parens_printed`
    /// is that caller fact, and it must be a fact about the OUTPUT, never about the source:
    /// a shell the caller strips leaves a `)` in the source and none in the output, so a
    /// source-keyed carve-out has no fixed point — the next pass sees no shell, reads the
    /// same comment as statement-trailing, and moves it. Line comments (`line_suffix`) and
    /// own-line block comments also trail after the `;`. The inline (operand-attached)
    /// comments are pushed into `parts`; the rest are returned.
    ///
    /// Caller idiom: `let after = self.split_terminator_gap_comments(parts, arg_end,
    /// span_end, keep_operand_line_inline); parts.push(";"); parts.extend(after);`.
    /// Used by return/throw, `export default`, and `export =` — the terminator callers
    /// whose argument may be parenthesized (unlike the expression-statement/var/
    /// class-property terminators, whose operand parens are consumed by inner printers —
    /// they use `push_semicolon_with_gap_comments`).
    ///
    /// `keep_operand_line_inline` is set by callers that render the operand inside
    /// conditional grouping parens (the binary return/throw path). A same-line **line**
    /// comment still enclosed by a stripped grouping paren (`return (a && b // c\n);`) is
    /// operand-attached: keeping it after the `;` would float it out of the parens
    /// (a #18837 over-reach). With the flag set it stays inline before the `)` (pushed to
    /// `parts`); the caller must force the group to break so the line comment never lands
    /// on the flat `expr // c;` path (which would swallow the `;`). Callers that render the
    /// operand bare (no parens) leave the flag `false` — there's nothing to keep it inside.
    ///
    /// `clause_tail` is the statement CONTAINER's deferral fact
    /// (`StatementContext::clause_tail`): `Some(dedent)` when the statement is a
    /// non-block clause body, whose tail line stays open to the enclosing construct —
    /// there each own-line member's interior break is wrapped in `dedent` levels
    /// ([`Printer::build_clause_tail_comment_doc`]), so the freed comment renders at
    /// the flushing construct's level in one pass instead of settling on the reparse.
    /// The same-line arms are indent-free (no interior break), so only the own-line
    /// arm reads it.
    pub(in crate::printer) fn split_terminator_gap_comments(
        &self,
        parts: &mut DocBuf,
        argument_end: u32,
        span_end: u32,
        keep_operand_line_inline: bool,
        operand_parens_printed: bool,
        gap: TerminatorGap,
    ) -> DocBuf {
        let clause_tail = gap.clause_tail();
        let d = self.d();
        let mut deferred = DocBuf::new();
        // What physically precedes each comment — an **in-source** cursor that ADVANCES
        // over every comment emitted, because own-line-ness is a question about a
        // comment's own neighbours (docs/comments.md §Trailing and dangling runs). Held
        // at `argument_end` it read the second half of an author-glued run across the
        // first, called it own-line, and split the pair onto two lines.
        let mut prev_end = argument_end;
        // Whether anything is already buffered in a `line_suffix`. Once it is, a
        // same-line block must buffer too: an inline emission renders at its doc
        // position — right after the `;` — while the buffer flushes at the line's end,
        // so the pair comes out REORDERED.
        let mut run_deferred = false;
        // ⚠️ The statement's own claim stops where the trailing TRIVIA run begins: past that
        // point the comments are the enclosing statement LIST's, and every caller here is one
        // of the kinds prettier ejects (`return` / `throw` / `export default` — see
        // [`Printer::statement_hands_off_terminator_gap`]). Emitting them as well is a
        // DOUBLE-PRINT; the list places them by the ordinary own-line / same-line split.
        // A non-block CLAUSE body has no such list, so it claims the whole gap as before.
        let claim_end = self.node_terminator_claim_end(argument_end, span_end, gap);
        for comment in self.comments_to_emit_between(argument_end, claim_end) {
            let same_line = !self.has_newline_between(prev_end, comment.span.start);
            let operand_enclosed =
                same_line && self.gap_has_close_paren(comment.span.end, span_end);
            if comment.is_block && operand_enclosed && operand_parens_printed {
                // Operand-attached (inside stripped parens): `return (x /* c */);`.
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if !comment.is_block && operand_enclosed && keep_operand_line_inline {
                // Operand-attached line comment (inside stripped parens):
                // `return (a && b // c\n);`. Stays inline before the `)`. Emitted as
                // plain text — the caller's forced break means the following softline
                // becomes the newline before `)`, so the comment never swallows it.
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if same_line {
                // Statement-trailing, on the line it was written on: a block trails
                // inline after the `;` (prettier 3.9), a line comment via `line_suffix`
                // (`return x; // c`).
                deferred.push(if run_deferred {
                    self.build_trailing_line_comment_doc(comment)
                } else {
                    self.build_trailing_comment_doc(comment)
                });
                run_deferred |= !comment.is_block;
            } else {
                // Own-line — of EITHER kind. The break rides *inside* the `line_suffix`
                // so it replays in run order after the `;`; a bare `hardline` here would
                // render ahead of an already-buffered line comment and reorder the pair,
                // and it dropped an author blank the terminator's own line survives.
                let blank = self.has_blank_line_between(prev_end, comment.span.start);
                deferred.push(match clause_tail {
                    Some(dedent) => self.build_clause_tail_comment_doc(comment, blank, dedent),
                    None => self.build_trailing_comment_doc_own_line_blank(comment, blank),
                });
                run_deferred = true;
            }
            prev_end = comment.span.end;
        }
        deferred
    }

    /// Whether a (comment-skipping) `)` appears in `[start, end)` — i.e. a stripped
    /// grouping paren follows a trailing comment before the terminator, marking the
    /// comment as operand-enclosed rather than statement-trailing.
    pub(crate) fn gap_has_close_paren(&self, start: u32, end: u32) -> bool {
        find_char_skipping_comments(self.source.as_bytes(), start as usize, end as usize, b')')
            .is_some()
    }

    /// Append the NODE's share of a stripped-paren interior — a spread's
    /// ([`internal::SpreadElement::paren_interior`]) or a destructuring-assignment rest's
    /// ([`internal::RestElement::paren_interior`]) — excluding own-line comments, which
    /// are the parent list's share (array/call/object literal, array/object pattern).
    ///
    /// The node's doc prints only what shares the argument's line: same-line blocks
    /// inline, and the same-line `//` deferred via `line_suffix` so text the parent
    /// appends inline (a comma, an after-comma block) still lands ahead of it. Every
    /// own-line comment — block or line — needs a line of its own that only the parent
    /// can give (a `line_suffix` raised here would escape past the enclosing `]`/`)`),
    /// so the parent picks them up via [`Self::paren_interior_own_line_comments`], in
    /// source order.
    ///
    /// At most ONE comment can defer: a `//` ends its line, so everything after it in
    /// the interior is own-line by construction — which is also what makes the deferred
    /// run structurally weld-free here.
    pub(crate) fn append_paren_interior_trailing_comments(
        &self,
        parts: &mut DocBuf,
        interior: Span,
    ) {
        let d = self.d();
        let argument_end = interior.start;
        for comment in self.comments_to_emit_between(argument_end, interior.end) {
            if self.has_newline_between(argument_end, comment.span.start) {
                // Own-line comments (line or block): skip — the parent's share.
                continue;
            }
            if comment.is_block {
                // Same-line block comment: `...x /* c */`
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                // Same-line line comment: defer past the argument's own text.
                parts
                    .push(d.line_suffix(d.concat(&[d.text(" "), self.build_comment_doc(comment)])));
            }
        }
    }

    /// The PARENT's share of a stripped-paren interior.
    ///
    /// When the parser strips grouping parens from a spread's or a destructuring rest's
    /// argument (`...(x⏎/* c */)`) the comments land in `[argument end, node end)` — the
    /// node's `paren_interior` — and that region has **two** emitters which must
    /// partition it exactly once (the seam in `docs/comments.md` §The element-comma seam,
    /// applied to a node's own interior rather than to a list gap). The split is the
    /// source's own-line question — the same question every other trailing seam asks:
    ///
    /// - [`Self::append_paren_interior_trailing_comments`] — the node's own doc — prints
    ///   what shares the argument's line: the same-line blocks inline, and the same-line
    ///   `//` deferred through `line_suffix`.
    /// - the parent (array element loop, call/`new`/member-chain last-argument emitter,
    ///   object property loop, array/object pattern loop) prints the **own-line
    ///   comments** — block or line — which need a line of their own that only the parent
    ///   can give (a `line_suffix` raised from inside the node would escape past the
    ///   enclosing `]`/`)`), each on its own line in source order.
    ///
    /// This function is the ONLY spelling of that share — the emitting form,
    /// [`Self::push_paren_interior_own_line_comments`], reads it from here rather than
    /// re-deriving the predicate. Expressing it as an *anchor shift* instead — scanning
    /// the parent's trailing gap from the argument's end rather than from the node's own
    /// end — reads as equivalent and is not: it hands the parent the node's share too, so
    /// every same-line block and same-line `//` prints twice.
    ///
    /// `interior` is `None` for an element with no stripped-paren interior (anything but a
    /// spread or a rest — [`internal::Expression::paren_interior`]), whose share is empty.
    pub(crate) fn paren_interior_own_line_comments(
        &self,
        interior: Option<Span>,
    ) -> CommentVec<'_> {
        let Some(interior) = interior else {
            return CommentVec::new();
        };
        let arg_end = interior.start;
        self.comments_to_emit_between(arg_end, interior.end)
            .filter(|c| self.has_newline_between(arg_end, c.span.start))
            .collect()
    }

    /// Whether the parent's share of a stripped-paren interior
    /// ([`Self::paren_interior_own_line_comments`]) ends in a `//` — the END-of-list
    /// ordering question: a share whose last comment is a `//` can have nothing glued after
    /// it, so the ordinary `[node end, closer)` gap run must emit FIRST, its inline blocks
    /// landing on the argument's line ahead of any deferred suffix. Where a block-ending
    /// share goes relative to that run is the list family's [`ShellRunOrder`].
    ///
    /// Asked of the LAST element only. Between two elements the comma gives the gap's run a
    /// home on the element's own line and the share follows it past the comma, whatever the
    /// share ends in (`...b, // c⏎/* i */`). [`Self::push_last_element_share_and_run`] is
    /// the one emitter of the ordering, for every list with a last element — the argument
    /// lists, the array and object literals, and the array and object patterns.
    fn paren_interior_share_ends_in_line_comment(&self, interior: Option<Span>) -> bool {
        self.paren_interior_own_line_comments(interior)
            .last()
            .is_some_and(|c| !c.is_block)
    }

    /// Emit a LAST element's stripped-paren share and its closer-gap run, in the order
    /// the list family names.
    ///
    /// A share ending in a `//` goes last under every family
    /// ([`Self::paren_interior_share_ends_in_line_comment`]): nothing can be glued behind
    /// it. A share ending in a block splits the families, and only over the run's BLOCKS
    /// ([`ShellRunOrder`]): the argument lists keep source order, a cataloged prettier
    /// divergence ("Spread stripped-paren comment then an outside block, as the LAST
    /// argument" in conformance_prettier_ts_comments.md §Comment relocation), while the
    /// array and object literals and patterns match prettier, which hoists a block written
    /// after the `)` onto the element's line. The run's `//` follows the share under both
    /// (`...b⏎/* i */ // t`), where prettier agrees with the call family.
    ///
    /// `interior` is `None` for an element with no interior (anything but a spread or a
    /// rest), which leaves the run alone. The run comes in two halves, `push_blocks` and
    /// `push_line`, whose shapes each list family keeps as its own; a family under
    /// [`ShellRunOrder::SourceOrder`] never separates them, so it may pass its whole run as
    /// `push_blocks`.
    ///
    /// Returns whether the share printed anything — an own-line comment, which a list that
    /// may still collapse must force open (the call-argument loop's `force_expansion`).
    pub(crate) fn push_last_element_share_and_run(
        &self,
        parts: &mut DocBuf,
        interior: Option<Span>,
        order: ShellRunOrder,
        push_blocks: impl FnOnce(&mut DocBuf),
        push_line: impl FnOnce(&mut DocBuf),
    ) -> bool {
        let mut pushed = false;
        let share_ends_in_line = self.paren_interior_share_ends_in_line_comment(interior);
        let block_share = interior.filter(|_| !share_ends_in_line);
        if order == ShellRunOrder::SourceOrder {
            pushed |= self.push_paren_interior_own_line_comments(parts, block_share);
        }
        push_blocks(parts);
        if order == ShellRunOrder::HoistBlocks {
            pushed |= self.push_paren_interior_own_line_comments(parts, block_share);
        }
        push_line(parts);
        pushed |= self
            .push_paren_interior_own_line_comments(parts, interior.filter(|_| share_ends_in_line));
        pushed
    }

    /// Whether a stripped-paren interior holds a comment the enclosing list must EXPAND
    /// around. Two kinds, for two different reasons:
    ///
    /// - an **own-line comment** — block or line, the parent's share above — the parent
    ///   prints it, and it needs a line of its own that only a broken list has;
    /// - a **same-line `//`** — the node's own doc defers it through `line_suffix`, and
    ///   on a list that stays collapsed that buffer flushes past the closer — for a call,
    ///   past its `)` *and* its `;`, re-binding the comment from the argument to the
    ///   statement (`fn(a, ...(b // c⏎))` → `fn(a, ...b); // c`). A deferred run must not
    ///   leave the construct it was written in — `docs/comments.md`. The array literal
    ///   has no such hazard: its brackets already break around a spread carrying a
    ///   comment. The array and object PATTERNS ask it of a rest element for both reasons.
    ///
    /// The predicate below spells the union as "any line comment, or any own-line
    /// comment" — the same set (a line comment is either same-line, forcing via the
    /// deferral, or own-line, forcing as the parent's share). `None` has no interior.
    pub(crate) fn paren_interior_forces_expansion(&self, interior: Option<Span>) -> bool {
        interior.is_some_and(|interior| {
            let arg_end = interior.start;
            self.comments_to_emit_between(arg_end, interior.end)
                .any(|c| !c.is_block || self.has_newline_between(arg_end, c.span.start))
        })
    }

    /// [`Self::paren_interior_forces_expansion`] asked of a whole element list — the
    /// **entry-gate** form, and the only one an argument-list builder should use to decide
    /// whether it must run its comment-aware path at all.
    ///
    /// Asked of EVERY element, not just the last: an interior lies *before* its own
    /// element's end, so no gap scan in any of these builders can see it, and a non-last
    /// spread's interior is exactly as invisible as a last one's. Spelling the gate on
    /// `arguments.last()` is what dropped it at three of the call family's entry points
    /// (the same reach `any_comment_forces_expansion` already has per argument).
    pub(crate) fn any_paren_interior_forces_expansion(
        &self,
        elements: &[&internal::Expression<'_>],
    ) -> bool {
        elements
            .iter()
            .any(|e| self.paren_interior_forces_expansion(e.paren_interior()))
    }

    /// Whether an element's own doc ends in a DEFERRED line comment — a spread or rest
    /// whose stripped grouping parens held a **same-line** `//` (`...(b // c⏎)`), which
    /// [`Self::append_paren_interior_trailing_comments`] emits through `line_suffix` (an
    /// own-line `//` is the parent's share and defers nothing). `None` has no interior.
    ///
    /// The caller that owns the gap *after* such a node must not let its own same-line
    /// `//` defer onto the same output line: two deferred line comments emitted back to
    /// back weld into ONE comment, the second `//` becoming text inside the first
    /// (`// c1 // c2`). That is the merge prettier performs here and tsv refuses — see
    /// `docs/comments.md` §Trailing and dangling runs.
    pub(crate) fn paren_interior_defers_line_comment(&self, interior: Option<Span>) -> bool {
        interior.is_some_and(|interior| {
            let arg_end = interior.start;
            self.comments_to_emit_between(arg_end, interior.end)
                .any(|c| !c.is_block && !self.has_newline_between(arg_end, c.span.start))
        })
    }

    /// Emit the parent's share of a stripped-paren interior
    /// ([`Self::paren_interior_own_line_comments`], empty for `None`) into `parts`, each on
    /// its own line with author blank lines preserved. Returns whether anything was emitted —
    /// which is also the caller's signal to force its list open, since an own-line comment
    /// is a sibling of the element rather than a trailer on its line.
    ///
    /// Where this sits relative to the caller's own `[node end, closer)` gap
    /// depends on whether a **comma** follows the element, and that is a position
    /// question, not a source-order one:
    ///
    /// - at the END of a list there is no comma, so both the interior and anything
    ///   written after the `)` merely trail the element; source order decides, and this
    ///   run's place is the list family's ([`Self::push_last_element_share_and_run`]).
    /// - between two elements the comma gives an outside block a home on the element's
    ///   own line, so the ordinary gap goes first and this run follows it, past the comma
    ///   ([`Printer::open_inter_arg_gap`], the array element loop, the object property
    ///   loop).
    ///
    /// Either way the caller does NOT carry a `prev_end` out of here: its own gap starts
    /// at the node's end, which already lies past every interior comment, so its blank
    /// scan cannot double-count a blank this loop already consumed.
    ///
    /// Every caller is a hard-broken (or comment-force-expanded) layout: an own-line
    /// comment is exactly the thing that forces one. A `//` in the run is always
    /// last-on-its-line (nothing can share a line behind it), so the hardline before
    /// the next comment — or the caller's own break after the run — is what keeps it
    /// from swallowing what follows.
    pub(crate) fn push_paren_interior_own_line_comments(
        &self,
        parts: &mut DocBuf,
        interior: Option<Span>,
    ) -> bool {
        self.push_paren_interior_own_line_comments_with_blanks(parts, interior, true)
    }

    /// [`Self::push_paren_interior_own_line_comments`] with the author-blank policy named.
    ///
    /// `preserve_blanks: false` is for a run the caller emits **past an elision comma** (the
    /// array element loop, when holes follow the spread). The blank was authored between the
    /// argument and the comment, a gap the run no longer occupies once a structural comma
    /// sits in front of it — and the array's own rule is that a hole carries **no** blank
    /// line after it (`has_blank_line_after_slot`, prettier's `node &&`), so the reprint
    /// drops it. Preserving it here would print a blank the next pass removes.
    pub(crate) fn push_paren_interior_own_line_comments_with_blanks(
        &self,
        parts: &mut DocBuf,
        interior: Option<Span>,
        preserve_blanks: bool,
    ) -> bool {
        let Some(interior) = interior else {
            return false;
        };
        let comments = self.paren_interior_own_line_comments(Some(interior));
        let mut prev_end = interior.start;
        let mut prev_comment: Option<&internal::Comment> = None;
        for comment in &comments {
            // A pair the author GLUED onto one line keeps that line, whichever blank policy
            // is in force ([`Printer::trailing_run_hugs_previous`]) — the glue question and
            // the author-blank question are separate, so the predicate is asked directly
            // rather than through `push_trailing_run_separator`, whose non-glue arm is
            // always blank-preserving.
            if self.trailing_run_hugs_previous(prev_comment, comment.span.start) {
                parts.push(self.d().text(" "));
            } else if preserve_blanks {
                self.push_blank_preserving_hardline(parts, prev_end, comment.span.start);
            } else {
                parts.push(self.d().hardline());
            }
            parts.push(self.build_comment_doc(comment));
            prev_end = comment.span.end;
            prev_comment = Some(comment);
        }
        !comments.is_empty()
    }

    /// Check if a grouping pair in `[expr_end, boundary_end)` holds trailing comments —
    /// comments the author wrote INSIDE the pair, between the operand and its `)`.
    ///
    /// True when the gap holds a grouping `)` at all (confirming the parser stripped a
    /// `ParenthesizedExpression`; without that check this would false-positive on normal
    /// operator comments, e.g. ternary `? c /* comment */ :`) **and** at least one
    /// comment lies before it.
    ///
    /// ⚠️ The window is `[expr_end, ')')`, not the caller's whole gap
    /// ([`Self::collapsed_grouping_close`]). Stated the other way round — "is there a `)`
    /// after the LAST comment" — one comment written *outside* the pair flipped the whole
    /// question false, and the shell that asked it then emitted **nothing** for the gap,
    /// dropping the run inside the pair along with it (`( // c⏎g // c9⏎) /* t */++` printed
    /// `( // c⏎g⏎)++`). A gap that spans the `)` holds two runs belonging to two emitters,
    /// so the predicate has to be about one of them.
    ///
    /// The existence check is **on page**, not *to emit*: every caller is a layout gate —
    /// does this gap's content force the shell open, keep the pair, break the arrow — and
    /// an owned comment still occupies the page it is not this gap's job to print
    /// (`docs/comments.md` §the three axes). No owned comment can reach one of these gaps
    /// today (ownership needs a node starting after the comment, and every shell gap ends
    /// at a delimiter), so the axis is stated for the reader and for the next boundary
    /// that widens, not for a behaviour it changes.
    ///
    /// The narrow-gap test is inline at the site — a shell gap is a `)` or nothing in
    /// nearly every ask — and only a gap that could hold a comment pays the call below.
    #[inline]
    pub(crate) fn has_trailing_paren_comments(&self, expr_end: u32, boundary_end: u32) -> bool {
        !range_too_narrow_for_a_comment(expr_end, boundary_end)
            && self.has_trailing_paren_comments_wide(expr_end, boundary_end)
    }

    /// The search half of [`Self::has_trailing_paren_comments`] — one outlined copy.
    #[inline(never)]
    fn has_trailing_paren_comments_wide(&self, expr_end: u32, boundary_end: u32) -> bool {
        // The wide window first: it is a binary search over `self.comments` (trivially
        // false in a comment-free document) and a strictly weaker precondition, so the
        // source walk below never runs for the overwhelmingly common gap.
        self.has_comments_on_page_between(expr_end, boundary_end)
            && self
                .collapsed_grouping_close(expr_end, boundary_end)
                .is_some_and(|close| self.has_comments_on_page_between(expr_end, close))
    }

    /// The **outermost** grouping `)` a self-parenthesizing value's shells collapse into.
    ///
    /// A sequence supplies its own required parens, so the printer emits ONE pair where
    /// the source may hold several — those parens plus every redundant shell the parser
    /// stripped around them. The single emitted `)` stands in for all of them, and no
    /// enclosing emitter can see inside a paren pair
    /// ([`Self::trailing_paren_comment_parts`]), so every comment up to the LAST close
    /// has to be emitted inside. Scanning to the FIRST one bounded the range at the
    /// innermost paren and dropped the comment that sits past it outright
    /// (`const k = ((a, b) /* c */);` → `const k = (a, b);`).
    ///
    /// The walk steps over `)` and trivia only, so it stops at the shell run's end rather
    /// than at the caller's boundary — an enclosing construct's own `)` is unreachable
    /// even where that boundary is loose (an `as` cast's operand shell, whose boundary
    /// spans the keyword and its type).
    ///
    /// `None` when the gap holds no `)` at all, leaving the caller its own fallback.
    pub(in crate::printer) fn collapsed_grouping_close(
        &self,
        expr_end: u32,
        boundary_end: u32,
    ) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let mut i = expr_end;
        let mut close = None;
        while let Some(pos) = self.next_significant_byte(i, boundary_end) {
            if bytes[pos] != b')' {
                break;
            }
            close = Some(pos as u32);
            i = pos as u32 + 1;
        }
        close
    }

    /// A sequence value sitting inside a stripped grouping shell: its own required parens
    /// ARE the pair the printer emits, and a trailing comment stays inside them
    /// (`const x = (a, b /* c */)`) rather than floating out after `)` or doubling the
    /// shell ([`Printer::build_sequence_doc_value`], prettier #19263).
    ///
    /// The one seam both shell builders route through, because locating that pair's close
    /// is one question with one answer: they held separate copies of the scan and both
    /// were wrong the same way, dropping the comment of a doubly-shelled value at every
    /// position they serve (`const k = ((a, b) /* c */);`, `() => ((a, b) /* c */)`).
    ///
    /// The `boundary_end` fallback is defensive: a value only reaches a *shell* builder
    /// with a grouping pair around it in source, so the walk has a `)` to find. The
    /// restricted-production site answers this differently and keeps its own scan —
    /// there a bare `return a, b;` has no pair at all, and sweeping to the boundary
    /// would pull the statement's own terminator-gap comments inside the parens.
    fn build_shell_sequence_doc(
        &self,
        seq: &internal::SequenceExpression<'_>,
        span: Span,
        expr_end: u32,
        boundary_end: u32,
        layout: SeqLayout,
    ) -> DocId {
        let grouping_close = self
            .collapsed_grouping_close(expr_end, boundary_end)
            .unwrap_or(boundary_end);
        self.build_sequence_doc_value(seq, span, grouping_close, layout)
    }

    /// The index of the next byte that is neither whitespace nor trivia, at or after
    /// `start` and before `end`; `None` when the range holds nothing else.
    ///
    /// The "what actually comes next in the source?" step both shell questions ask —
    /// [`Self::collapsed_grouping_close`] takes it once per `)`, and
    /// [`Self::shell_meets_statement_terminator`] once. A comment occupies bytes even
    /// where nothing emits it, so a walk that stepped over whitespace alone would stop
    /// on the `/` and answer about the wrong token.
    fn next_significant_byte(&self, start: u32, end: u32) -> Option<usize> {
        next_significant_byte(self.source, start, end)
    }

    /// Whether a comment in `[expr_end, boundary_end)` forces the operand's grouping
    /// parens to **survive**, at a gap the grammar marks `[no LineTerminator here]`.
    ///
    /// Two gaps qualify, and both are ASI-sensitive: an `as`/`satisfies` cast's
    /// operand→keyword gap and a postfix `++`/`--`'s operand→operator gap (tsc breaks
    /// out of each construct on `scanner.hasPrecedingLineBreak()`). A comment that
    /// occupies more than one line — a `//`, which runs to end of line, or a multi-line
    /// block — therefore cannot be inlined here, because inlining **rewrites the
    /// program**: the `//` swallows the tail (`(1 // c⏎) as const;` → `1 // c as const;`,
    /// which parses as a bare `1`), and the multi-line block puts a real line break
    /// before the keyword (output that does not reparse at all).
    ///
    /// Such a comment can only ever have been authored *inside* a grouping paren shell,
    /// since the bare form is unparseable — so the shell is what holds it in place, and
    /// stripping it is not available the way it is at the sibling keyword→value gaps
    /// ([`Self::build_expression_doc_with_paren_comments`]). A shell is redundant only
    /// when the stripped form can still express the comment's position.
    ///
    /// A single-line block comment inlines as before (`x /* c */ as A`, `x /* c */++`),
    /// matching prettier. Finding the `)` is what keeps this keyed on a real shell, and
    /// the window it bounds — the pair's INTERIOR, not the caller's whole gap — is the
    /// one this question is about: only a comment the pair actually HOLDS can need the
    /// pair kept, and a comment past the `)` belongs to the enclosing gap
    /// ([`Self::append_shell_outside_run`]).
    ///
    /// The multi-line predicate is asked LAST: the two steps before it are a binary
    /// search over `self.comments` and a short source walk, both false on nearly every
    /// cast and update in a real file.
    pub(crate) fn asi_gap_needs_parens(&self, expr_end: u32, boundary_end: u32) -> bool {
        if !self.has_comments_on_page_between(expr_end, boundary_end) {
            return false;
        }
        let Some(close) = self.collapsed_grouping_close(expr_end, boundary_end) else {
            return false;
        };
        self.has_line_spanning_comments_to_emit_between(expr_end, close)
    }

    /// Whether a REDUNDANT shell's leading gap holds a comment the operand OWNS whose own
    /// text spans a line — the second way that shell becomes load-bearing.
    ///
    /// The gap's ordinary trigger is a comment it EMITS, which stripping would drop
    /// outright. An owned comment is never dropped — it travels inside the operand's doc —
    /// so it needs no shell for its own sake. It needs one for ASI's: a `MultiLineComment`
    /// holding a `LineTerminator` *is* one for the syntactic grammar (ecma262 sec-comments),
    /// so stripping the shell puts a line terminator immediately before an `as` / `satisfies`
    /// keyword or a postfix `++`, none of which may start a line — and where the cast sits in
    /// a restricted production, before that keyword's argument as well
    /// (`throw (/* a⏎b */ a) as B;` printed bare is a DEAD document). It is the same
    /// question [`Self::asi_gap_needs_parens`] answers on the trailing side, asked on the
    /// leading one, so the `//` spelling and this one keep one shell between them.
    ///
    /// ⚠️ **Only where the pair is REDUNDANT.** A pair the operand needs anyway already holds
    /// the comment and prints flat around it (`const a = (/* c⏎d */ b + c + d) as T;`,
    /// `typescript/syntax/comments/required_pair_multiline_leading_comment_prettier_divergence`);
    /// retaining there would expand a pair that is not the ASI question at all. A SEQUENCE is
    /// the case `needs_parens` cannot answer for us — it reports false because the sequence's
    /// own printer emits the pair rather than because there is none — so it is named.
    fn redundant_shell_holds_line_spanning_owned_comment(
        &self,
        leading_start: u32,
        expr: &internal::Expression<'_>,
        operand_ctx: ParenContext,
    ) -> bool {
        // A SEQUENCE's own parens ARE the grouping — `needs_parens` says false because the
        // sequence's printer emits them itself, not because the pair is redundant — so the
        // comment is already inside a pair that prints flat around it and nothing is at risk
        // (`const s1 = (/* c⏎d */ b, c) as T;`).
        !matches!(expr.kind, internal::ExpressionKind::SequenceExpression(_))
            && !self.needs_parens(expr, operand_ctx)
            && self
                .comments_in_source_between(leading_start, expr.span().start)
                .any(|c| c.multiline)
    }

    /// The operand of an ASI-sensitive gap (an `as`/`satisfies` keyword, a postfix
    /// `++`/`--`) rendered inside the grouping-paren shell that holds its comments,
    /// emitting **both** of the shell's gaps — `(`→operand and operand→`)`.
    ///
    /// `None` when no shell needs keeping, so the caller falls through to its ordinary
    /// inline path.
    ///
    /// The trailing gap is what makes the shell load-bearing ([`Self::asi_gap_needs_parens`]):
    /// the keyword may not start a line, so a comment spanning lines has nowhere else to go.
    /// The **leading** gap keeps the shell for a different reason — nothing else emits it.
    /// A comment there that is neither glued to the operand (which would make it
    /// `owned_by_node`, printed from the operand's own doc) nor inside the operand's span
    /// belongs to no node at all, so stripping the shell **drops** it outright
    /// (`( // c⏎x) as A` → `x as A`, the comment gone). Retaining on either gap is one
    /// rule for one shell rather than two half-rules that disagree about `( // b⏎x // c⏎)`.
    ///
    /// A **sequence** operand's own required parens are the grouping, so re-wrapping
    /// would double them: with no leading run it delegates to the value builder, which
    /// keeps the trailing run inside the pair
    /// ([`Self::build_expression_doc_keep_paren_comments`]); with one, the pair is built
    /// HERE — the leading gap is the pair's and the value builder's window opens at the
    /// operand, so delegating dropped the run outright (`(// c⏎x, y) as A` →
    /// `(x, y) as A`). The sequence rides the expanded shell BARE
    /// ([`Printer::build_sequence_doc_bare`]), the same one-shell-for-both-gaps layering
    /// every required pair in the family takes.
    ///
    /// ⚠️ `boundary_end` is the KEYWORD / operator, so the gap it bounds spans the pair's
    /// `)` and holds **two** runs: the pair's own trailing run and, past the `)`, the
    /// enclosing gap's. The shell claims the whole window, so it emits both — the first
    /// inside the pair, the second after `close` — and the split is
    /// [`Self::collapsed_grouping_close`], the same `)` every window that opens after an
    /// operand takes its start from (`docs/comments.md` hazard 3). Emitting the window as
    /// one run instead would have carried the outside comment *into* the parens, and the
    /// all-or-nothing gate that preceded this dropped both runs outright.
    ///
    /// ⚠️ **Whether the shell exists may not turn on OWNERSHIP**, which is what the third
    /// answer ([`AsiOperandShell::Bare`] carrying a run) is for. A single-line block the
    /// author glued to a paren the printer strips (`(/* c */ (x)) as A`) is not owned — no
    /// node begins at that `(` — so the EMIT axis sees it and the shell above would open
    /// for it; but the shell's own output glues the comment to the operand, where the
    /// reparse OWNS it, the emit axis goes blind, and the second pass prints the bare form
    /// ([`Self::leading_run_reparses_owned`]). So that run takes the bare form at
    /// once: the caller prints it ahead of the operand, inside any pair the operand needs —
    /// the bytes the owned spelling `(/* c */ x) as A` already reaches. Returning the run
    /// rather than `None` is what keeps the gap's one emitter from forgetting it: the gap
    /// "belongs to no node at all", so a caller that drops the run DROPS the comment.
    pub(crate) fn build_asi_operand_shell_doc(
        &self,
        node_start: u32,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        operand_ctx: ParenContext,
    ) -> AsiOperandShell {
        let expr_start = expr.span().start;
        // The trailing gap opens where the operand's doc stops printing, which for a
        // sequence is its last operand ([`shell_content_end`]).
        let expr_end = shell_content_end(expr);
        let open = find_char_skipping_comments(
            self.source.as_bytes(),
            node_start as usize,
            expr_start as usize,
            b'(',
        )
        .map(|p| p as u32);

        let leading_start = open.map_or(node_start, |p| p + 1);
        let has_leading = open.is_some()
            && (self.has_comments_to_emit_between(leading_start, expr_start)
                || self.redundant_shell_holds_line_spanning_owned_comment(
                    leading_start,
                    expr,
                    operand_ctx,
                ));
        let needs_trailing = self.asi_gap_needs_parens(expr_end, boundary_end);
        if !has_leading && !needs_trailing {
            return AsiOperandShell::Bare(None);
        }
        // A shell the TRAILING gap keeps holds this run either way — glued inside it, the
        // comment is owned on the reparse and the same shell prints the same bytes.
        if !needs_trailing && self.leading_run_reparses_owned(leading_start, expr_start) {
            let run = self
                .build_inline_comments_between_doc_trailing_space_opt(leading_start, expr_start);
            // A SEQUENCE's own pair is the one that survives, and the run is inside it —
            // ahead of the first operand, where the reparse's owner prints it. Left to the
            // caller it would lead the sequence's `(` instead, a form that holds only until
            // the cast takes a pair of its own (`y && (/* c */ (a, b) as T)` hoists the
            // comment out of that pair on the next pass). So this state composes the pair
            // itself, flat, with the operands riding bare — the expanded shell's layering.
            if let (internal::ExpressionKind::SequenceExpression(seq), Some(run)) =
                (&expr.kind, run)
            {
                let d = self.d();
                let mut pair: DocBuf = smallvec![
                    d.text("("),
                    run,
                    self.build_sequence_doc_bare(seq, expr.span, expr_end),
                    d.text(")"),
                ];
                // The operand→keyword window holds single-line blocks at most here (the
                // trailing gap needed no shell), and the sequence's own envelope floats
                // those out past its `)` — so they trail the pair, wherever the author
                // put them relative to the closers this state collapses.
                if let Some(trailing) =
                    self.build_inline_comments_between_doc_opt(expr_end, boundary_end)
                {
                    pair.push(trailing);
                }
                return AsiOperandShell::Shell(d.concat(&pair));
            }
            return AsiOperandShell::Bare(run);
        }

        // The pair's `)` splits the window into the run it holds and the run the
        // enclosing gap holds. `None` is the shell kept for its LEADING run alone with no
        // `)` reachable past the operand, where the whole window is the pair's. The
        // interior window ends PAST the `)` (`close + 1`) — it is what proves a pair is
        // there, and [`Self::has_trailing_paren_comments`] looks for it inside the range
        // it is given.
        let close = self.collapsed_grouping_close(expr_end, boundary_end);
        let inner_end = close.map_or(boundary_end, |c| c + 1);

        if matches!(expr.kind, internal::ExpressionKind::SequenceExpression(_)) && !has_leading {
            let seq =
                self.build_expression_doc_keep_paren_comments(expr, inner_end, SeqLayout::Aligned);
            return AsiOperandShell::Shell(self.append_shell_outside_run(seq, close, boundary_end));
        }

        // The shell's `(` is the statement's first token whenever the statement's leftmost
        // node is inside the operand, so it already discharges what the expression-statement
        // wrap exists for — keeping the statement from starting with `{` / `function` /
        // `class`. Clearing the target stops the operand adding a second pair inside this
        // one (`(⏎\t({ a: 1 }) // c⏎) as const`). Not restored afterwards: a rebuild under a
        // conditional group must reach the same answer, and the target is cleared per
        // statement by `build_expression_statement_doc` regardless. A nested cast never
        // owns the target (it is set for the statement's leftmost node only), so this is a
        // no-op there.
        self.expr_stmt_paren_target.set(None);

        // The opening-delimiter rule at this shell's `(`: a `//` the author glued to it
        // keeps that line ([`Self::split_located_paren_glued_run`]), as at every other
        // opening delimiter. The doc carries its own leading space.
        let (paren_trailing, run_start) =
            self.split_located_paren_glued_run(leading_start, open, expr_start);
        let leading_run = self.build_rhs_comments_opt(run_start, expr_start);

        let mut body = DocBuf::new();
        if let Some(run) = leading_run {
            body.push(run);
        }
        body.push(match &expr.kind {
            // Bare: the shell composed below IS the sequence's required pair. Its own
            // printer takes the owned-comment claim ([`Self::build_sequence_doc_bare`] →
            // the run form), so this arm needs none.
            internal::ExpressionKind::SequenceExpression(seq) => {
                self.build_sequence_doc_bare(seq, expr.span, expr_end)
            }
            // A MULTI-LINE block the operand OWNS prints just inside this shell's `(`,
            // outside the operand's own group
            // ([`Printer::build_value_with_outermost_owned_comment`]). This shell is the
            // required pair at the seams that reach it, so it owes the family's rule like
            // every other emitter of one — and it is the arm the plain
            // `(/* c⏎d */ x) as T` path never reaches, since the shell is built only when
            // the gap holds a comment the EMIT axis can see (an un-owned run member) or
            // the operand→keyword gap needs the parens for ASI.
            // An alone-on-line format-ignore directive in the run just emitted freezes the
            // operand ([`Printer::build_left_spine_operand_doc`]): this pair is RETAINED, so
            // the directive keeps the line the author gave it inside it and the reparse
            // reads it in the very same place — a first pass that did not freeze would
            // normalize the author's bytes and then hold that form for good.
            _ => self.build_left_spine_operand_doc(
                leading_start,
                expr,
                FrozenOperandPair::Emitted,
                || self.build_expression_doc_claiming_outermost(expr),
            ),
        });
        if let Some((trailing, _needs_break)) =
            self.trailing_paren_comment_parts(expr_end, inner_end)
        {
            body.extend(trailing);
        }

        let shell = self.compose_expanded_shell_doc(paren_trailing, &body, ")");
        AsiOperandShell::Shell(self.append_shell_outside_run(shell, close, boundary_end))
    }

    /// Whether a paren pair's leading gap holds exactly the run the REPARSE would hand to
    /// the operand: one single-line block comment the author glued to what follows it,
    /// with nothing between it and the operand's first token but the grouping `(`s the
    /// printer strips (and whatever whitespace the author left inside them).
    ///
    /// Printed, that comment lands glued to the operand, which is the parser's ownership
    /// rule (`bind_leading_comment`) — so the next pass finds no comment this gap EMITS, and
    /// whatever the gap decides for it now has to be what the owned spelling gets then, or
    /// the two passes disagree. Every pair whose rendering turns on "does this gap emit a
    /// comment" asks it: the ASI operand shell ([`Self::build_asi_operand_shell_doc`]) and
    /// the required pairs' expand-or-fold ([`Self::build_required_pair_leading_shell_doc`]).
    ///
    /// The glue is the comment's own ([`Self::comment_hugs_next`]), never a distance to the
    /// operand: a line break the author put INSIDE a stripped `(` is erased with it, so
    /// `(/* c */ (⏎x))` reaches the operand glued all the same. Deliberately no wider: a
    /// second comment in the gap stays emit-visible on the reparse too (only the LAST one
    /// can glue to the operand), a multi-line one keeps an ASI shell
    /// ([`Self::redundant_shell_holds_line_spanning_owned_comment`]), and a comment the
    /// author broke after is not glued at all — each of those is stable as it stands.
    fn leading_run_reparses_owned(&self, gap_start: u32, operand_start: u32) -> bool {
        let mut run = self.comments_in_source_between(gap_start, operand_start);
        let (Some(comment), None) = (run.next(), run.next()) else {
            return false;
        };
        comment.is_block
            && !comment.multiline
            && !comment.owned_by_node
            && self.comment_hugs_next(comment)
    }

    /// Append the run written PAST a shell's `)` — the enclosing gap's, not the pair's —
    /// inline before the keyword / operator the caller adds next.
    ///
    /// Both arms of [`Self::build_asi_operand_shell_doc`] need it, and for the same
    /// reason: the shell claims the whole `[operand_end, keyword)` window, so whatever it
    /// does not emit is DROPPED. Only a single-line block can be here — anything
    /// occupying a second line would put a line terminator before an ASI-sensitive token,
    /// which is unparseable — so this is the same emitter the shell-LESS path uses for
    /// the whole gap, and the two agree on the one shape they share.
    fn append_shell_outside_run(
        &self,
        shell: DocId,
        close: Option<u32>,
        boundary_end: u32,
    ) -> DocId {
        let Some(start) = close.map(|c| c + 1).filter(|&start| {
            start < boundary_end && self.has_comments_to_emit_between(start, boundary_end)
        }) else {
            return shell;
        };
        self.d().concat(&[
            shell,
            self.build_inline_comments_between_doc(start, boundary_end),
        ])
    }

    /// The expanded paren-shell rendering every shell emitter shares — the
    /// leading-gap builders (via [`Self::build_leading_run_expanded_shell_doc`])
    /// and the anchored trailing-run shell
    /// ([`Self::build_paren_operand_comment_doc`]'s line arm): `( // c` when the
    /// author glued a `//` to the `(` (the comment runs to end of line and the
    /// indent's hardline supplies the break, so nothing following it is
    /// swallowed; the space is the glued split's), the body one indent in, the
    /// closer back out on its own line. `close` is `")"` where a separate node
    /// prints what follows, `")!"` where this doc owns the non-null's `!`.
    pub(in crate::printer) fn compose_expanded_shell_doc(
        &self,
        paren_trailing: Option<DocId>,
        body: &DocBuf,
        close: &'static str,
    ) -> DocId {
        let d = self.d();
        let open_doc = match paren_trailing {
            Some(comment) => d.concat(&[d.text("("), comment]),
            None => d.text("("),
        };
        d.concat(&[
            open_doc,
            d.indent_hardline(d.concat(body)),
            d.hardline(),
            d.text(close),
        ])
    }

    /// The family's expanded shell for a LEADING run — the one emission every
    /// required-pair leading gap shares: split the `(`-glued `//` (the
    /// opening-delimiter rule), emit the rest of the run above the operand with
    /// its authored own-line placements kept, close with `close`. Callers: the
    /// assignment-target / instantiation-head operand shell
    /// ([`Self::build_shell_operand_doc`]), the non-null's needs-parens arm, and
    /// the sealed optional chain (both `build_ts_non_null_doc` /
    /// `build_sealed_non_null_paren_doc`, whose `close` is `")!"`).
    ///
    /// A commented TRAILING gap rides the SAME shell rather than a second one — the
    /// pair is already expanded, so the operand→`)` run simply follows the operand
    /// inside it ([`Self::push_shell_trailing_run`]). Declining instead and folding both
    /// runs flat, which is what a shell that bailed on a commented trailing gap forced,
    /// left the one authoring that commented both gaps rendering with no shell at all:
    /// the `//` glued to a `(` that never broke, the operand at the ENCLOSING indent,
    /// and the `)` welded to it — the family's shape at neither gap, and prettier's at
    /// neither either.
    pub(in crate::printer) fn build_leading_run_expanded_shell_doc(
        &self,
        gap_start: u32,
        operand_start: u32,
        trailing_gap: Option<(u32, u32)>,
        inner_doc: DocId,
        close: &'static str,
    ) -> DocId {
        let (paren_trailing, run_start) =
            self.split_open_delimiter_glued_run(gap_start, operand_start);
        let mut body = DocBuf::new();
        if let Some(run) = self.build_rhs_comments_opt(run_start, operand_start) {
            body.push(run);
        }
        body.push(inner_doc);
        if let Some((start, end)) = trailing_gap {
            self.push_shell_trailing_run(&mut body, start, end);
        }
        self.compose_expanded_shell_doc(paren_trailing, &body, close)
    }

    /// The operand→`)` run inside a pair the LEADING run already expanded — the same
    /// two arms [`Self::build_paren_operand_comment_doc`] takes when it builds the shell
    /// itself, minus the shell (this body is already inside one).
    ///
    /// A `//` takes the anchored emitter, since every comment here was authored AFTER
    /// the operand and the closer's hardline below ends the line the run defers onto; a
    /// block-only run trails the operand inline.
    fn push_shell_trailing_run(&self, body: &mut DocBuf, start: u32, end: u32) {
        if self.has_line_comments_between(start, end) {
            self.push_anchored_trailing_run(body, start, end, RunLeadingBlank::Keep);
        } else if self.has_comments_to_emit_between(start, end) {
            body.push(self.build_chain_block_comments_doc(start, end, CommentSpacing::Leading));
        }
    }

    /// The family's answer for a REQUIRED pair's LEADING gap — the one rule every
    /// position that prints such a pair asks, rather than five spellings of it.
    ///
    /// `Some` is the expanded shell ([`Self::build_leading_run_expanded_shell_doc`]):
    /// the run occupies a line, so the `(`-glued `//` keeps the `(` line, the operand
    /// takes one indent and `close` comes back out. `None` says this gap does not take
    /// the shell and the caller folds the run flat above the operand instead — the run
    /// fits on one line (a glued single-line block run), which is the only reason left,
    /// and the one run whose newlines are not its own folds with it
    /// ([`Self::leading_run_reparses_owned`]).
    ///
    /// `trailing_gap` is `[operand_end, boundary_end)`, `None` at a position whose pair
    /// has no trailing gap of its own. A commented one does NOT decline the shell: it
    /// rides inside it, after the operand.
    pub(in crate::printer) fn build_required_pair_leading_shell_doc(
        &self,
        gap_start: u32,
        operand_start: u32,
        trailing_gap: Option<(u32, u32)>,
        inner: DocId,
        close: &'static str,
    ) -> Option<DocId> {
        if !self.has_comments_to_emit_between(gap_start, operand_start)
            || !self.has_newline_between(gap_start, operand_start)
            // A newline AROUND a glued block — ahead of it, or inside a `(` stripped from
            // behind it — is not the run's: the expanded shell would print the comment
            // glued to the operand, which the reparse owns and folds.
            || self.leading_run_reparses_owned(gap_start, operand_start)
        {
            return None;
        }
        Some(self.build_leading_run_expanded_shell_doc(
            gap_start,
            operand_start,
            trailing_gap,
            inner,
            close,
        ))
    }

    /// The index just past the `)` that closes a REQUIRED pair around an operand
    /// ending at `operand_end`, when the author wrote that pair — `None` when the next
    /// thing in the source is something else (a `(` of an argument list, a `` ` ``),
    /// which means the pair this position prints is tsv's own and the source has no
    /// trailing gap inside it.
    ///
    /// The distinction is load-bearing: `(fn /* t */)()` writes the pair around the
    /// callee and the comment is INSIDE it, while `(/* t */ fn())` writes it around the
    /// whole call and the comment belongs to the call, not to a pair the callee prints.
    pub(in crate::printer) fn paren_shell_close_after(&self, operand_end: u32) -> Option<u32> {
        paren_shell_close_after(self.source, operand_end)
    }

    /// The operand→`)` gap a REQUIRED pair owns, when the pair KEEPS ITS INTERIOR at
    /// this position **and** the author wrote it.
    ///
    /// Both halves matter. `keeps_interior` is the **position's** answer — a pair keeps
    /// its interior at a function/arrow callee or tag (prettier's
    /// `printCommentsForFunction` covers leading and trailing alike) and at a **sealed
    /// optional chain**, where the pair is what stops the chain and both formatters keep
    /// the comment between the operand and the `)`. It is deliberately a parameter and
    /// never re-derived from the operand's KIND here: a kind is a property of the
    /// subtree and holds wherever that subtree appears, while a gap belongs to exactly
    /// one seam, which is a property of the position — `(() => 1 /* c */).p` prints the
    /// same pair as an IIFE callee but its trailing gap is the member seam's, and
    /// claiming it here double-printed the comment. The position-shaped predicates are
    /// [`chain_paren_leading_gap`]'s
    /// family, and the leading half reads the same ones.
    /// The second half is the author's pair, since `(fn /* t */)()` writes it around the
    /// callee while `(/* t */ fn())` writes it around the whole call, where the comment
    /// belongs to the call and to no pair printed here
    /// ([`paren_shell_close_after`]).
    ///
    /// Every window that opens AFTER the operand — the type-argument gap, an argument
    /// list's own leading scan, a tag→`` ` `` gap, a member seam's `object_end` — takes
    /// its start from the returned `)` rather than from the operand's span end, so a
    /// comment emitted inside the parens is not emitted a second time outside them
    /// (`docs/comments.md` hazard 3).
    pub(in crate::printer) fn owned_pair_trailing_gap(
        &self,
        operand_end: u32,
        keeps_interior: bool,
    ) -> Option<(u32, u32)> {
        keeps_interior
            .then(|| self.paren_shell_close_after(operand_end))
            .flatten()
            .map(|close| (operand_end, close))
    }

    /// Whether neither gap of a paren pair holds a comment on the page: `leading` is the
    /// `(`→operand window, `trailing` the operand→`)` one (`None` where the pair has no
    /// trailing gap of its own). What a callee pair's caller hands
    /// `CalleeParens::build_body_doc`, and the chain base's twin of it — a curried arrow
    /// chain's callee shape opens the pair from inside the operand's doc, so a comment in
    /// either gap leaves the pair to the shell builders.
    pub(in crate::printer) fn pair_gaps_are_comment_free(
        &self,
        leading: (u32, u32),
        trailing: Option<(u32, u32)>,
    ) -> bool {
        !self.has_comments_on_page_between(leading.0, leading.1)
            && trailing.is_none_or(|(start, end)| !self.has_comments_on_page_between(start, end))
    }

    /// Where the window AFTER an operand opens: past the `)` of a pair that emitted its
    /// own trailing gap, or the operand's own span end where there is no such pair.
    ///
    /// `trailing_gap` is that operand's [`Self::owned_pair_trailing_gap`], passed in
    /// rather than re-derived so the offset and the claim come off ONE lookup. The one
    /// spelling of it, so the three positions that print such a pair — the bare callee,
    /// a member chain's callee, a tagged template's tag — and every window they open
    /// (type arguments, an argument list's leading scan, the tag→`` ` `` gap) cannot
    /// disagree about where the pair ends.
    pub(in crate::printer) fn gap_start_after_owned_pair(
        operand_end: u32,
        trailing_gap: Option<(u32, u32)>,
    ) -> u32 {
        trailing_gap.map_or(operand_end, |(_, close)| close)
    }

    /// A REQUIRED pair that owns BOTH of its gaps — `(`→operand and operand→`)`.
    ///
    /// The one emission for every position where the pair the printer emits is the
    /// author's own and nothing outside it can reach between the parens: the chain's
    /// sealed / IIFE base, the bare IIFE callee, a tagged template's function tag.
    /// Layering, which is the family's convention rather than this function's choice:
    ///
    /// - a leading run that occupies a line takes the expanded shell
    ///   ([`Self::build_required_pair_leading_shell_doc`]) — and a commented TRAILING
    ///   gap rides that SAME shell, after the operand, rather than declining it;
    /// - otherwise the leading run folds flat above the operand, and a commented
    ///   TRAILING gap then builds the shell around that
    ///   ([`Self::build_paren_operand_comment_doc`]);
    /// - with neither, the pair is bare.
    ///
    /// So exactly ONE shell is built however many of the two gaps hold comments. Two
    /// nest; none leaves the `//` glued to a `(` that never breaks and the operand at
    /// the enclosing indent.
    ///
    /// `flat_body` and `broken_body` are the operand's two renderings, which differ
    /// only where a caller shapes the flat one (the chain base's hang / ternary
    /// forms); the fold is applied to **both**, since the trailing emitter's line-comment
    /// arm prints the broken one and folding into the shaped body alone would DROP the
    /// leading run at exactly the authoring that reaches it. A caller whose SHELL body
    /// differs from its folded one asks the two halves separately —
    /// [`Self::build_folded_required_pair_doc`].
    pub(in crate::printer) fn build_owned_required_pair_doc(
        &self,
        leading_gap: (u32, u32),
        trailing_gap: Option<(u32, u32)>,
        flat_body: DocId,
        broken_body: DocId,
        close: &'static str,
    ) -> DocId {
        self.build_required_pair_leading_shell_doc(
            leading_gap.0,
            leading_gap.1,
            trailing_gap,
            flat_body,
            close,
        )
        .unwrap_or_else(|| {
            self.build_folded_required_pair_doc(
                leading_gap,
                trailing_gap,
                flat_body,
                broken_body,
                close,
            )
        })
    }

    /// [`Self::build_owned_required_pair_doc`] past its leading-shell arm — the form for
    /// a caller whose shell and fold take DIFFERENT bodies.
    ///
    /// The non-null's `needs_parens` arm is the one such caller: its operand may carry a
    /// ternary's width-driven expanding parens, which belong in the folded body but not
    /// inside a hard-expanded shell, where there is no width left to decide. Every other
    /// position hands one body to both and takes the combined form above.
    pub(in crate::printer) fn build_folded_required_pair_doc(
        &self,
        leading_gap: (u32, u32),
        trailing_gap: Option<(u32, u32)>,
        flat_body: DocId,
        broken_body: DocId,
        close: &'static str,
    ) -> DocId {
        let d = self.d();
        let (gap_start, operand_start) = leading_gap;
        let fold = |body: DocId| match self.build_rhs_comments_opt(gap_start, operand_start) {
            Some(lead) => d.concat(&[lead, body]),
            None => body,
        };
        let flat_body = fold(flat_body);
        if let Some((start, end)) = trailing_gap
            && let Some(doc) = self.build_paren_operand_comment_doc(
                start,
                end,
                flat_body,
                fold(broken_body),
                close,
            )
        {
            return doc;
        }
        d.concat(&[d.text("("), flat_body, d.text(close)])
    }

    /// An operand in a position that prints a REQUIRED pair around some operand
    /// kinds, with the node's `^`→operand gap emitted — the whole operand doc for
    /// an assignment expression's / assignment pattern's target
    /// ([`ParenContext::AssignmentTarget`]) and an instantiation expression's head
    /// ([`ParenContext::InstantiationExpression`]).
    ///
    /// The gap is `[node_start, operand.span().start)`, `node_start` being the
    /// enclosing node's own start — for a parenthesized operand, its authored `(`.
    /// Nothing else emits it: a comment there that is not glued to the operand
    /// (which would make it `owned_by_node`, printed from the operand's own doc)
    /// belongs to no node, so the bare `"(" + operand + ")"` spelling DROPPED it
    /// outright (`( // c⏎x as T) = 1;` → `(x as T) = 1;`).
    ///
    /// Where the position REQUIRES the pair — a type-assertion target (`x as T = 1;`
    /// is a parse error), an arrow instantiation head — the pair prints whatever the
    /// comment does, and tsv keeps the run INSIDE it, where the author wrote it;
    /// prettier hoists it out in front, re-binding it from the operand to the whole
    /// statement (cataloged: conformance_prettier_ts_comments.md §Comment
    /// relocation, "Assignment-target shell, leading comment"). A run that occupies
    /// a line — a `//`, an own-line block — expands the pair, with the `( // c` glue
    /// and the author's own-line placements kept
    /// ([`Self::build_asi_operand_shell_doc`]'s rendering one construct over); a
    /// glued single-line block run stays flat.
    ///
    /// Any other operand's pair is REDUNDANT and strips (`(// c⏎x) = 1;`,
    /// `(// c⏎fn)<string>;`), and the stripped form still expresses the run's
    /// position — it leads the statement, exactly where prettier lands it.
    pub(in crate::printer) fn build_shell_operand_doc(
        &self,
        node_start: u32,
        target: &internal::Expression<'_>,
        context: ParenContext,
    ) -> DocId {
        let d = self.d();
        let target_start = target.span().start;
        let needs_parens = self.needs_parens(target, context);
        // Zero-comment fast gate, emit-keyed on purpose: the layouts below differ
        // only in where EMITTED comments render — an owned (target-glued) block
        // prints identically inside the flat pair from the target's own doc, and
        // never asks the pair to expand.
        if !self.has_comments_to_emit_between(node_start, target_start) {
            let inner = self.build_expression_doc(target);
            return if needs_parens { d.parens(inner) } else { inner };
        }
        let open = find_char_skipping_comments(
            self.source.as_bytes(),
            node_start as usize,
            target_start as usize,
            b'(',
        )
        .map(|p| p as u32);
        let leading_start = open.map_or(node_start, |p| p + 1);
        // A comment that occupies a line expands the pair, on the family's shared
        // rule. The read is in-source over the whole gap, which cannot over-fire:
        // the gate above already found a comment to emit (a comment-free author
        // break never reaches here), a `//` always ends its line, and an own-line
        // block carries its break. This position's pair encloses the operand alone,
        // so it has no trailing gap to yield the shell to.
        if needs_parens
            && let Some(shell) = self.build_required_pair_leading_shell_doc(
                leading_start,
                target_start,
                None,
                self.build_expression_doc(target),
                ")",
            )
        {
            return shell;
        }
        // The flat tail serves both regimes: a glued-block run leads the operand —
        // inside the pair where one prints, ahead of the bare operand where the
        // stripped run leads the statement (and there a line comment's hardline
        // separator is what keeps the statement after it on its own line).
        let lead = self.build_rhs_comments_opt(leading_start, target_start);
        let inner = self.build_expression_doc(target);
        let core = match lead {
            Some(lead) => d.concat(&[lead, inner]),
            None => inner,
        };
        if needs_parens { d.parens(core) } else { core }
    }

    /// Build expression doc, stripping a redundant grouping paren around a trailing
    /// comment and keeping the comment inline after the expression.
    ///
    /// When the parser strips parens from `(expr /* c */)`, comments between
    /// `expr.span().end` and `boundary_end` would be lost. For an inline same-line
    /// block comment we keep it trailing the expression (`expr /* c */`), matching
    /// prettier — stripping the redundant parens does not move the comment. Line /
    /// own-line comments need the parens (a bare line comment would swallow the
    /// following token), so those defer to `build_expression_doc_keep_paren_comments`.
    ///
    /// `position_parens` says the CALLING POSITION will parenthesize this value anyway
    /// (`const x = (a = b)`), which makes the shell **retained** rather than stripped —
    /// see [`Self::shell_value_keeps_own_parens`], the one predicate that answer is read
    /// through.
    ///
    /// Used for variable init, assignment RHS, and ternary branches. A `for` header's
    /// init declarator takes [`Self::build_for_init_value_doc`] instead — same handling,
    /// minus the statement-terminator deferral its `;` does not license; a value of one of
    /// these positions that IS a header clause's own gets that answer here, through
    /// [`Self::shell_closes_for_clause`].
    pub(crate) fn build_expression_doc_with_paren_comments(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        position_parens: bool,
    ) -> DocId {
        self.build_shell_value_doc(
            expr,
            boundary_end,
            self.shell_tail(boundary_end),
            position_parens,
            None,
        )
    }

    /// Which tail a value shell closing at `boundary_end` has, for the positions that do not
    /// name one ([`ShellTail`]).
    fn shell_tail(&self, boundary_end: u32) -> ShellTail {
        if self.shell_closes_for_clause(boundary_end) {
            ShellTail::ForClauseSeparator
        } else {
            ShellTail::StatementTerminator
        }
    }

    /// Whether a value shell closing at `boundary_end` is the one a `for` header CLAUSE ends
    /// at ([`Printer::for_clause_end`]).
    ///
    /// ⚠️ **One fact, two consequences, which is why the builder and its caller both ask
    /// it.** For the builder it is the tail: the `;` behind this shell separates clauses, so
    /// a trailing block stays inline instead of deferring out of the header (a `line_suffix`
    /// there drains past the loop's whole BODY, and the form it leaves is its own fixed
    /// point, so no gate sees it). For the caller it says the builder placed the header's
    /// `[~In]` pair ITSELF — inside the trailing comment, where the pair belongs
    /// (`a = ('k' in o) /* c */`) — so a position that wraps the shell's result again
    /// ([`Printer::wrap_for_init_in`] at the assignment RHS and the ternary branch) doubles
    /// it. Answering the two separately is how one site comes to print `((a in b))`.
    pub(crate) fn shell_closes_for_clause(&self, boundary_end: u32) -> bool {
        self.for_clause_end.get() == Some(boundary_end)
    }

    /// Whether the value shell closing at `boundary` has ALREADY supplied the `for` header's
    /// `[~In]` pair, so the position must not add one through
    /// [`Printer::wrap_for_init_in`] / [`Printer::wrap_frozen_for_init_in`].
    ///
    /// Two shapes, both stated by [`Printer::build_shell_value_doc`] and neither visible
    /// from the doc it hands back: it RETAINED the author's pair, which parenthesizes the
    /// `in` itself ([`Self::shell_value_keeps_own_parens`]), or the shell closes the header's
    /// own CLAUSE, where the builder places the pair inside the trailing comment
    /// ([`Self::shell_closes_for_clause`]) — a second pair outside it would read as the
    /// author's. Asked by the assignment RHS on both its frozen and unfrozen paths; a
    /// `boundary` of `None` is a position with no shell, which supplied nothing.
    pub(crate) fn shell_supplies_for_init_pair(
        &self,
        expr: &internal::Expression<'_>,
        boundary: Option<u32>,
    ) -> bool {
        boundary.is_some_and(|end| {
            self.shell_closes_for_clause(end) || self.shell_value_keeps_own_parens(expr, end, false)
        })
    }

    /// [`Self::build_expression_doc_with_paren_comments`] for a value the position froze
    /// (`prettier-ignore` in its `=`→value gap): the same shell, with the verbatim slice
    /// standing in for the expression doc.
    ///
    /// ⚠️ **The freeze does not take the value out of its shell.** The slice is the value's
    /// own node span, so the author's grouping parens lie OUTSIDE it and the gap between
    /// the slice and that `)` is the shell's, exactly as in the unfrozen form — a frozen
    /// arm that skipped this builder printed its own bare `parens()` and left that gap with
    /// no emitter at all, DROPPING every comment in it (`docs/comments.md` hazard 4; a
    /// printer that synthesizes its own `(`…`)` owns the gap inside it, per
    /// [`Self::trailing_paren_comment_parts`]). Routing through here is what keeps the
    /// frozen and unfrozen forms answering the gap identically — which shell is retained,
    /// where the comment renders, and when it defers past the terminator are all questions
    /// about the GAP, not about what renders between the parens.
    pub(crate) fn build_frozen_value_shell_doc(
        &self,
        expr: &internal::Expression<'_>,
        frozen: Span,
        boundary_end: u32,
        position_parens: bool,
    ) -> DocId {
        self.build_shell_value_doc(
            expr,
            boundary_end,
            self.shell_tail(boundary_end),
            position_parens,
            Some(frozen),
        )
    }

    /// Whether [`Self::build_expression_doc_with_paren_comments`] supplies the value's
    /// paren pair ITSELF, so the calling position must not add a second one.
    ///
    /// The single predicate both re-parenthesizing positions ask — a declarator
    /// initializer and a ternary branch. They each own a `needs_parens` question of their
    /// own (`position_parens`), but "did the callee already wrap?" is one question with
    /// one answer, and answering it twice is how the pair gets doubled at one site and
    /// dropped at the other (a ternary CONSEQUENT bounds this scan empty, so the callee
    /// never wraps there however the position answers `needs_parens`).
    pub(crate) fn shell_value_keeps_own_parens(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        position_parens: bool,
    ) -> bool {
        // The gap's own content forces the pair open, whatever the position does — the
        // stronger half, spelled once ([`Self::value_shell_forced_open`]) and read from here
        // so a shell this predicate reports as retained is exactly one that seam can act on.
        self.value_shell_forced_open(expr, boundary_end)
            // Or the calling position parenthesizes this value anyway, so the pair is in the
            // output whatever the builder does and nothing in the gap may cross it — a
            // same-line block included. That pair never OPENS, which is why the seams that
            // choose the operator's layout read the disjunct above instead of this.
            || (position_parens && self.value_shell_pair_in_question(expr, boundary_end))
    }

    /// Whether the author's grouping pair around this value is one the shell builder decides
    /// about at all: there is a comment in its trailing gap to decide for, and the value is
    /// not one that supplies its own pair regardless.
    ///
    /// A **sequence self-parenthesizes on every path**, so its shell is never the retained
    /// one — the builder claims it at its own arm ([`Self::build_shell_value_doc`]) and a
    /// caller that wraps reads `false` here rather than doubling the pair. The exclusion is
    /// spelled once, for both retention predicates: giving it to only one had a sequence
    /// value answer "the shell is forced open" where nothing had retained a shell, which
    /// changed the operator's layout under a comment that still deferred past the `;`.
    fn value_shell_pair_in_question(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
    ) -> bool {
        !matches!(expr.kind, internal::ExpressionKind::SequenceExpression(_))
            && self.has_trailing_paren_comments(expr.span().end, boundary_end)
    }

    /// Add a value position's clarity parens around a shell-built value — unless the shell
    /// builder already supplied the pair ([`Self::shell_value_keeps_own_parens`]).
    ///
    /// The two declarator positions (statement-level and `for`-header) resolve
    /// `position_parens` themselves, then hand it here with the doc the shell builder
    /// returned for it. `position_parens` must be the value the **builder received**: the
    /// two sides answer one question, and asking it twice is how the pair gets doubled at
    /// one site and dropped at the other. A ternary branch answers a different pair
    /// question than the flag it passes the builder, so it applies the predicate at its own
    /// seam (`parenthesize_ternary_branch`) rather than through here.
    pub(crate) fn wrap_value_position_parens(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        position_parens: bool,
        inner: DocId,
    ) -> DocId {
        if position_parens
            && !self.shell_value_keeps_own_parens(expr, boundary_end, position_parens)
        {
            self.d().parens(inner)
        } else {
            inner
        }
    }

    /// Whether the gap's own content forces the shell to be RETAINED — the layout half of
    /// [`Self::shell_value_keeps_own_parens`], asked by the builder that acts on it and by
    /// the caller that must not double the pair.
    ///
    /// ⚠️ **One question, one predicate, one AXIS.** Spelling this on the two sides
    /// separately AND on different axes — the caller counting comments **on page**, the
    /// builder only those it would **emit** — has an owned comment in the gap make
    /// the caller skip a wrap the builder never makes, stripping the value's clarity parens.
    /// This is a layout gate ("does anything occupy the page here?"), so the on-page axis is
    /// the correct one for both (`docs/comments.md` §the three axes).
    fn shell_gap_retains_parens(
        &self,
        expr_end: u32,
        boundary_end: u32,
        position_parens: bool,
    ) -> bool {
        // The calling position parenthesizes this value anyway, so the pair is in the
        // output whatever this builder does — nothing may cross it.
        position_parens || self.shell_gap_holds_unplaceable_comment(expr_end, boundary_end)
    }

    /// Whether the shell's trailing gap holds a comment with **no inline placement**: a
    /// line comment, which would swallow whatever the output puts behind it on that line,
    /// or a comment the author gave a line of its own.
    ///
    /// One cause, and the two things that follow from it are why both sides ask it. Read
    /// with the position's own answer ([`Self::shell_gap_retains_parens`]) it says the pair
    /// must SURVIVE the strip — a stripped shell has nowhere to put such a comment. Read
    /// alone, where the pair is not in question but its SHAPE is, it says a
    /// self-parenthesizing value may not print its own `Aligned` pair: only the expanded
    /// shell has a line to spare above its `)`.
    fn shell_gap_holds_unplaceable_comment(&self, expr_end: u32, boundary_end: u32) -> bool {
        self.comments_on_page_between(expr_end, boundary_end)
            .any(|c| !c.is_block || self.has_newline_between(expr_end, c.span.start))
    }

    /// Whether the value's own grouping shell is RETAINED **and forced open** by what sits
    /// in its trailing gap — a `//`, or a comment the author gave a line of its own
    /// ([`Self::shell_gap_holds_unplaceable_comment`]).
    ///
    /// **The assignment family's layout rule, stated once.** That break is the COMMENT's,
    /// not a break point inside the value, so the operator must not take one as well: the
    /// assignment HUGS the shell (`x = (⏎\ta?.b! // c⏎);`) rather than hanging it below the
    /// operator (`x =⏎\t(⏎\t\ta?.b! // c⏎\t);`), which spends two indents on one comment and
    /// puts the `(` on a line of its own. It is the answer the shapes that never reach a
    /// hang already give — an object-literal value, a fluid call, a ternary — and the one
    /// `return` / `throw` / `export default` / an arrow body / a bare expression statement
    /// give at the same authoring.
    ///
    /// ⚠️ **"Retained VALUE shell", never "the doc starts with `(`".** The question is about
    /// the pair the AUTHOR wrote around this value and the comment that keeps it; a doc that
    /// merely opens with a parenthesis — an arrow's parameter list, a call's arguments — is
    /// not this, and hugging on that reading moves layouts that have nothing to do with the
    /// shell.
    ///
    /// Read by the two seams that hang a value under an operator, so they cannot answer it
    /// differently: [`crate::printer::Printer::build_assignment_layout`] (assignment
    /// expressions, class fields, object values) and the declarator's hand-rolled twin
    /// (`statements/variable.rs`, through its `is_layout_eligible` gate). Deliberately NOT
    /// keyed on the value's kind — the shell's break belongs to the comment whatever the
    /// value is.
    ///
    /// This is the STRONGER half of [`Self::shell_value_keeps_own_parens`], which reads it as
    /// its first disjunct so the implication holds by construction: a forced-open shell is a
    /// retained one. What it drops is that predicate's `position_parens` arm — a pair the
    /// POSITION prints anyway survives a same-line block too (`const k = (a = b /* c */);`),
    /// and that shell never opens, so it is no reason to change the operator's layout. The
    /// value-kind exclusion is shared ([`Self::value_shell_pair_in_question`]): a sequence
    /// retains no shell here, so it forces none open either.
    pub(in crate::printer) fn value_shell_forced_open(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
    ) -> bool {
        self.value_shell_pair_in_question(expr, boundary_end)
            && self.shell_gap_holds_unplaceable_comment(expr.span().end, boundary_end)
    }

    /// The `for`-header init counterpart of
    /// [`Self::build_expression_doc_with_paren_comments`].
    ///
    /// A header init declarator's shell is followed by the header's **clause separator**,
    /// not by a statement `;`, so the deferral arm below must not fire: a comment sent
    /// past that separator leaves the declarator it was written in, and there is nothing
    /// out there to hold it — prettier, which does relocate it, cannot keep it either
    /// (its next pass carries a run's later comment clean out of the header, past the
    /// `)`, into the body's leading position). Everything else is shared with the
    /// statement-level path, which is the point: the block comment strips inline and the
    /// line comment retains the shell for exactly the same reasons there.
    ///
    /// `position_parens` carries the declarator's own clarity-paren answer
    /// (`ParenContext::VariableInit` — an assignment as a value takes a pair), exactly as
    /// the statement-level path carries it: a header declarator is a declarator, and the
    /// `for` exemption prettier applies is to the init **clause's own expression**
    /// (`for (a = b = c; ;)`), not to a value one binding deeper.
    ///
    /// ⚠️ **This is the declarator's own value, not "lexically under a for header".** The
    /// ambient `in_for_init` flag spans nested function and class bodies, where a real
    /// statement terminator does exist and the deferral is correct
    /// (`for (let i = (() => { const k = (a /* c */); })(); ;)` keeps `k`'s comment past
    /// its `;`) — so the distinction is threaded from the one builder that knows it
    /// rather than read from that flag. A value the header reaches through some OTHER
    /// position — a clause that is an assignment, whose RHS shell is the assignment's own —
    /// draws the same line from its shell's boundary
    /// ([`Self::shell_closes_for_clause`]), never from the flag either.
    ///
    /// `frozen` is the value-head freeze this position resolved, exactly as
    /// [`Self::build_frozen_value_shell_doc`] carries it for the statement-level twin: the
    /// slice replaces the expression doc and nothing else moves, because which shell is
    /// retained and where its comment renders are questions about the GAP, not about what
    /// renders between the parens.
    pub(crate) fn build_for_init_value_doc(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        position_parens: bool,
        frozen: Option<Span>,
    ) -> DocId {
        self.build_shell_value_doc(
            expr,
            boundary_end,
            ShellTail::ForClauseSeparator,
            position_parens,
            frozen,
        )
    }

    /// [`Self::build_for_init_value_doc`] at a `for` header sequence clause's **last
    /// operand**, whose erased grouping shell closes at the clause's own end.
    ///
    /// Every earlier operand's shell gap is claimed by the comma that follows it; the last
    /// operand's has no claimant at all, so without this the comments in it are DROPPED
    /// (`docs/comments.md` hazard 4). It is the clause's gap in every respect the shell
    /// builder decides — the block strips inline, a `//` or an own-line comment retains the
    /// shell, nothing defers past a separator that terminates no statement, and a
    /// self-parenthesizing operand takes the gap inside the pair it prints for itself —
    /// which is why it routes to the clause's own builder rather than growing an emitter or
    /// a tail of its own. `position_parens` is always false: nothing parenthesizes a
    /// sequence operand, so the `[~In]` wrap is the only pair the position adds, exactly
    /// the one the clause's `build_elem` would.
    pub(crate) fn build_for_clause_operand_doc(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        frozen: Option<Span>,
    ) -> DocId {
        self.build_for_init_value_doc(expr, boundary_end, false, frozen)
    }

    /// The value's own doc inside a shell arm that does NOT print the pair itself: the
    /// verbatim frozen slice where the position resolved a freeze, else the ordinary
    /// expression doc. Both spellings supply a self-parenthesizing value's own required
    /// pair ([`Self::build_frozen_expression_doc`] is `build_expression_doc`'s twin in
    /// exactly that), so the arms that return it need no sequence case of their own.
    fn build_shell_inner_doc(
        &self,
        expr: &internal::Expression<'_>,
        frozen: Option<Span>,
    ) -> DocId {
        match frozen {
            Some(frozen) => self.build_frozen_expression_doc(expr, frozen),
            None => self.build_expression_doc(expr),
        }
    }

    /// Where a grouping shell's TRAILING gap opens: the position past everything the
    /// value's own doc will print, so the shell's claim and the value's PARTITION the
    /// source between them (`docs/comments.md` §The element-comma seam — unclaimed is a
    /// DROP, doubly-claimed a DOUBLE-PRINT).
    ///
    /// Three answers, each keyed on who prints the region behind the value:
    ///
    /// - a FROZEN value prints its source verbatim, shell interior and all, so the gap
    ///   opens at the end of the SLICE ([`Self::element_claim_anchor`]). Anchored inside
    ///   it, the shell re-emits bytes the slice already printed — and, since the emitted
    ///   form still carries the directive and re-freezes, one more copy on every later
    ///   pass;
    /// - a `SequenceExpression` whose gap holds the shell's `)` hands its operands' own
    ///   trailing region to the single pair it prints ([`shell_content_end`]): that pair
    ///   stands in for the shell the strip removed, so the comment stays where the shell
    ///   held it (`const x = (fff, (aaa /* c */));` → `(fff, aaa /* c */)`, prettier #19263).
    ///   Anchored at `span.end` instead, the gap reads as empty, this builder hands back the
    ///   plain doc, and the sequence's own float-out envelope carries the comment OUT of the
    ///   pair, where the statement's terminator moves it again on the next pass;
    /// - everything else stops at the span end, the sequence with no `)` in its gap
    ///   included. That gap is the caller's whole window, and a caller that bounds it at the
    ///   value's own span — a ternary CONSEQUENT, whose scan stops where the `:` gap's
    ///   emitter starts — leaves no room for a shell `)`, which is the tell that the region
    ///   is the sequence's own tail rather than a shell's. Reaching into it there hands the
    ///   comment to the pair on pass 1 and to the consequent→`:` emitter on pass 2
    ///   (`c ? (x, (y /* t */)) : z` → `(x, y /* t */)` → `(x, y) /* t */`).
    ///
    /// `printer_owns_grouping` is what the source `)` scan is a proxy for: the second answer
    /// asks whether a pair will enclose this region in the OUTPUT, and a caller whose pair
    /// is the printer's own — the restricted productions' hanging parens, emitted whatever
    /// the author wrote ([`Self::build_restricted_production_paren_doc`]) — already knows it
    /// will, so it says so instead of looking for a shell that may not be there.
    pub(in crate::printer) fn shell_trailing_gap_start(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        frozen: Option<Span>,
        printer_owns_grouping: bool,
    ) -> u32 {
        let span_end = expr.span().end;
        let unfrozen = if matches!(expr.kind, internal::ExpressionKind::SequenceExpression(_))
            && (printer_owns_grouping
                || self
                    .collapsed_grouping_close(span_end, boundary_end)
                    .is_some())
        {
            shell_content_end(expr)
        } else {
            span_end
        };
        Self::element_claim_anchor(frozen, unfrozen)
    }

    fn build_shell_value_doc(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        tail: ShellTail,
        position_parens: bool,
        frozen: Option<Span>,
    ) -> DocId {
        let expr_end = self.shell_trailing_gap_start(expr, boundary_end, frozen, false);
        // The for-header's `[~In]` parens are applied HERE rather than by the caller,
        // because only the paths below know where they belong relative to the shell's
        // comment. They are tsv's parens, not the author's, so a comment written AFTER
        // the shell has to land outside them (`(a in b) /* c */`, not `(a in b /* c */)`)
        // — the same rule that keeps a synthesized paren from landing inside an owned
        // comment (`docs/comments.md`). The two paths that return early supply their own
        // pair: a sequence self-parenthesizes, and the keep-paren path RETAINS the shell,
        // which already parenthesizes the `in` — wrapping either would double it. A FROZEN
        // value takes the wrap in the same place, but asks the SLICE's question rather than
        // the node's ([`Printer::wrap_frozen_for_init_in`]) — a verbatim slice has no inner
        // positions to parenthesize a deeper `in` at — and skips it where the calling
        // POSITION already supplies a pair (`position_parens`, which is how
        // `for (let k = (a = b in c); ;)` keeps exactly one).
        let wrap_in = |doc: DocId| match (tail, frozen) {
            (ShellTail::ForClauseSeparator, Some(frozen)) => {
                self.wrap_frozen_for_init_in(expr, frozen, position_parens, doc)
            }
            (ShellTail::ForClauseSeparator, None) => self.wrap_for_init_in(expr, doc),
            (ShellTail::StatementTerminator, _) => doc,
        };

        if !self.has_trailing_paren_comments(expr_end, boundary_end) {
            return wrap_in(self.build_shell_inner_doc(expr, frozen));
        }

        // Every position this serves — variable init, assignment RHS, ternary branch, a
        // `for` clause and its last operand — is prettier's default layout arm; the two
        // that hang (a `return`/`throw` argument, an arrow body) claim their sequence
        // before reaching here.
        if let internal::ExpressionKind::SequenceExpression(seq) = &expr.kind {
            // A FROZEN sequence prints verbatim, so the operand-per-line layout the
            // sequence builder chooses is not available to it — its required pair takes
            // the retained-shell rendering below instead, which is where this gap's
            // comment goes on either path.
            return match frozen {
                Some(frozen) => self.build_frozen_kept_paren_doc(frozen, boundary_end),
                // A comment that forces the pair OPEN cannot ride the sequence's own
                // `Aligned` pair at a CLAUSE separator: that layout has no line left before
                // its `)`, so a `//` takes the `line_suffix` out past the clause's `;` and
                // leaves the construct it was written in — and the next pass, reading it
                // from the header's clause gap, prints it somewhere else again. The pair is
                // the EXPANDED shell instead, with the sequence riding it BARE, since that
                // shell IS the required pair (the same one-shell-for-both-gaps layering the
                // ASI-sensitive operands take, [`Self::build_asi_operand_shell_doc`]). A
                // statement position keeps the `Aligned` pair: there the `;` is a terminator
                // and the comment deferring past it is prettier's answer and tsv's own.
                None if tail == ShellTail::ForClauseSeparator
                    && self.shell_gap_holds_unplaceable_comment(expr_end, boundary_end) =>
                {
                    let inner = self.build_sequence_doc_bare(seq, expr.span, expr_end);
                    self.build_kept_paren_shell_doc(inner, expr_end, boundary_end)
                        .unwrap_or_else(|| self.d().parens(inner))
                }
                None => self.build_shell_sequence_doc(
                    seq,
                    expr.span,
                    expr_end,
                    boundary_end,
                    SeqLayout::Aligned,
                ),
            };
        }

        // A FROZEN value inside a pair the trailing gap RETAINS (the two reasons are the
        // tail builder's, [`Self::build_stripped_shell_tail_doc_with`]) prints as its slice
        // inside that pair; the unfrozen twin of the same question is the tail builder's own
        // first arm.
        if let Some(frozen) = frozen
            && self.shell_gap_retains_parens(expr_end, boundary_end, position_parens)
        {
            return self.build_frozen_kept_paren_doc(frozen, boundary_end);
        }
        self.build_stripped_shell_tail_doc_with(
            expr,
            expr_end,
            boundary_end,
            tail,
            position_parens,
            || wrap_in(self.build_shell_inner_doc(expr, frozen)),
        )
    }

    /// The trailing half of [`Self::build_shell_value_doc`] for a value whose doc the
    /// CALLER builds: a chained conditional in a ternary ALTERNATE (`: (aaa ? bbb : ccc /* t */)`),
    /// whose branch arm builds the nested conditional in its chained geometry and so cannot
    /// hand the whole value to the shell builder. `expr_end` is where the value's own
    /// printed content ends, `boundary_end` where the stripped shell closes; the caller's
    /// position supplies no pair of its own. The tail policy is the position's
    /// ([`Self::shell_tail`]), read here so the caller cannot name it wrong.
    pub(in crate::printer) fn build_stripped_shell_tail_doc(
        &self,
        expr: &internal::Expression<'_>,
        expr_end: u32,
        boundary_end: u32,
        inner: impl FnOnce() -> DocId,
    ) -> DocId {
        self.build_stripped_shell_tail_doc_with(
            expr,
            expr_end,
            boundary_end,
            self.shell_tail(boundary_end),
            false,
            inner,
        )
    }

    /// A stripped value shell's TRAILING gap — `expr_end` (the value's printed end) to
    /// `boundary_end` (the shell's close) — laid out around the value's doc, which `inner`
    /// builds on demand. One spelling for [`Self::build_shell_value_doc`] and the
    /// caller-built values ([`Self::build_stripped_shell_tail_doc`]): the drop the
    /// ternary's nested-alternate arm carried was exactly a value this builder never saw.
    ///
    /// Two reasons the shell is RETAINED rather than stripped, and either sends the
    /// comment inside the pair:
    ///
    /// - a line / own-line comment needs the parens on its own account (a bare line
    ///   comment would swallow the following `;`);
    /// - the calling POSITION parenthesizes this value anyway (`const x = (a = b)`),
    ///   so the pair is in the output whatever this builder does.
    ///
    /// The second is what stops the deferral below from marching a comment across a
    /// `)` the output still prints. That arm's licence is "this output erases the
    /// `)`" — true for a plain value (`const a = (x /* t */);` → `const a = x; /* t */`),
    /// false here, and a licence stops where its argument stops: the block comment of
    /// a parenthesized assignment was relocating out of a surviving pair
    /// (`const k = (x = y /* c */);` → `const k = (x = y); /* c */`) while the same
    /// construct one comma over — a non-last declarator, with no terminator to defer
    /// past — already kept it inside, and prettier keeps it inside in both. A retained pair
    /// prints the value afresh through the keep-paren builder — inside the author's pair a
    /// chained conditional is a root one, so `inner`'s chained geometry is not wanted there.
    fn build_stripped_shell_tail_doc_with(
        &self,
        expr: &internal::Expression<'_>,
        expr_end: u32,
        boundary_end: u32,
        tail: ShellTail,
        position_parens: bool,
        inner: impl FnOnce() -> DocId,
    ) -> DocId {
        if !self.has_trailing_paren_comments(expr_end, boundary_end) {
            return inner();
        }
        if self.shell_gap_retains_parens(expr_end, boundary_end, position_parens) {
            return self.build_expression_doc_keep_paren_comments(
                expr,
                boundary_end,
                SeqLayout::Aligned,
            );
        }

        let d = self.d();
        let inner = inner();

        // Every comment left here is a same-line block. Where the shell is the last
        // thing before a statement `;`, that block defers past the terminator — the same
        // answer the statement's own value-to-`;` gap gives once the shell is gone
        // ([`Printer::push_semicolon_with_gap_comments`] and its terminator sibling), which is
        // what makes one pass enough. Keying the choice on the stripped `)` instead cannot
        // reach a fixed point: this output erases that `)`, so the next pass reads the
        // comment as statement-trailing and moves it (`(x /* t */);` → `x /* t */;` →
        // `x; /* t */`). A shell that is NOT terminator-adjacent — a ternary CONSEQUENT
        // (whose gap ends at the `:`), an object value, a non-last declarator, a nested
        // assignment — keeps the block inline, where it is already its own fixed point.
        // A ternary ALTERNATE is terminator-adjacent and does reach this arm, which is
        // prettier's answer there too; the pair its branch may print is applied outside,
        // by `parenthesize_ternary_branch`, so nothing here crosses a surviving `)`.
        if tail == ShellTail::StatementTerminator
            && self.shell_meets_statement_terminator(boundary_end)
        {
            let mut parts: DocBuf = smallvec![inner];
            for comment in self.comments_to_emit_between(expr_end, boundary_end) {
                let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                parts.push(d.line_suffix(suffix));
            }
            return d.concat(&parts);
        }

        let comments = self.build_comments_between(expr_end, boundary_end, CommentSpacing::Leading);
        d.concat(&[inner, comments])
    }

    /// True when the next significant byte at or after `boundary_end` — the end of a
    /// stripped grouping shell — is the statement's `;`.
    ///
    /// The question a deferred trailing block must ask: is this gap the statement's
    /// terminator gap? Asking it of the SOURCE (not of the stripped `)`, which the
    /// output deletes) is what makes the answer survive the strip, so pass 2 — which
    /// sees the same `;` and no shell — agrees.
    fn shell_meets_statement_terminator(&self, boundary_end: u32) -> bool {
        let bytes = self.source.as_bytes();
        self.next_significant_byte(boundary_end, bytes.len() as u32)
            .is_some_and(|pos| bytes[pos] == b';')
    }

    /// The comments between an expression's end and a following `)`, as ready-to-append
    /// separator+comment parts, plus whether they force the broken `(⏎\texpr // c⏎)` frame.
    ///
    /// **A printer that synthesizes its own `(`…`)` owns this gap** — no enclosing emitter
    /// can see between those parens, so a comment left unclaimed here is DROPPED, not
    /// relocated. Both such printers call this: the stripped-paren restorer below and
    /// `build_jsdoc_cast_doc`, which lacked the gap entirely
    /// (`parenthesized/jsdoc_cast_trailing_paren_comment`).
    ///
    /// The separator is newline-aware — a comment the author put on a new line relative to
    /// the *previous item* (the expression, or the prior comment) breaks; otherwise it
    /// trails inline. Tracking the previous item rather than `expr_end` keeps a same-line
    /// group together (`x⏎ /* a */ // b`) while stopping a line comment that follows
    /// another comment from being swallowed by it. In the inline case every comment is a
    /// same-line block comment, so the rule collapses to a plain space.
    ///
    /// Returns `None` when the gap is empty, so callers keep their no-comment fast path.
    pub(crate) fn trailing_paren_comment_parts(
        &self,
        expr_end: u32,
        boundary_end: u32,
    ) -> Option<(DocBuf, bool)> {
        if !self.has_trailing_paren_comments(expr_end, boundary_end) {
            return None;
        }
        let d = self.d();

        // A line comment runs to end-of-line, an own-line block comment was authored on
        // its own line, and a multi-line block spans lines of its own — in every case the
        // shell already occupies more than one line, and a shell that breaks expands
        // rather than gluing its content to the `(` (the same rule
        // `build_expanded_parenthesized_union_opt` states for a breaking paren). Without
        // the `multiline` term the two authorings of one comment disagreed: `(⏎\tx // c⏎)`
        // expanded while `(x /* m1⏎m2 */)` stayed glued, at the same gap and for the same
        // reason.
        let needs_break = self
            .comments_to_emit_between(expr_end, boundary_end)
            .any(|c| {
                !c.is_block || c.multiline || self.has_newline_between(expr_end, c.span.start)
            });

        let mut parts = DocBuf::new();
        let mut prev_end = expr_end;
        for comment in self.comments_to_emit_between(expr_end, boundary_end) {
            if self.has_newline_between(prev_end, comment.span.start) {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(self.build_comment_doc(comment));
            prev_end = comment.span.end;
        }
        Some((parts, needs_break))
    }

    /// Build expression doc re-adding the stripped grouping parens around trailing
    /// comments, producing `(expr /* c */)` or `(\n\texpr // c\n)`.
    ///
    /// Used where stripping the parens would relocate the comment — arrow bodies
    /// (prettier moves the comment into the params) and other non-sequence operands
    /// with an own-line/line trailing comment. Keeping the parens preserves the
    /// comment where the user wrote it. (Sequence operands take the dedicated
    /// `build_sequence_doc_value` path, which keeps the comment inside the sequence's
    /// own parens instead of adding a second pair.)
    pub(in crate::printer) fn build_expression_doc_keep_paren_comments(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        layout: SeqLayout,
    ) -> DocId {
        let expr_end = expr.span().end;

        // A sequence self-parenthesizes, so it takes the shared arm rather than the
        // paren-restoring path below — `build_expression_doc` would emit its parens and
        // this method would re-wrap them (`() => ((1, 2, 3) /* c */)`). `layout` is the
        // caller's: an arrow body hangs its operands, the ASI-shell operands align.
        if let internal::ExpressionKind::SequenceExpression(seq) = &expr.kind {
            return self.build_shell_sequence_doc(seq, expr.span, expr_end, boundary_end, layout);
        }

        let inner = self.build_expression_doc(expr);
        self.build_kept_paren_shell_doc(inner, expr_end, boundary_end)
            .unwrap_or(inner)
    }

    /// The RETAINED shell's rendering: the value inside the pair the author wrote, with
    /// the operand→`)` run behind it — inline where the run is a same-line block, on the
    /// operand's own indented line where a `//` or an own-line comment forces the pair
    /// open ([`Self::trailing_paren_comment_parts`] decides which).
    ///
    /// `inner` is whatever renders the value at this position, so the ordinary expression
    /// doc and a frozen verbatim slice share one rendering rather than two. `None` says
    /// the gap holds nothing to emit, leaving the caller its own bare form.
    fn build_kept_paren_shell_doc(
        &self,
        inner: DocId,
        expr_end: u32,
        boundary_end: u32,
    ) -> Option<DocId> {
        let d = self.d();
        let (comment_parts, needs_break) =
            self.trailing_paren_comment_parts(expr_end, boundary_end)?;

        Some(if needs_break {
            let mut indent_parts: DocBuf = smallvec![d.hardline(), inner];
            indent_parts.extend(comment_parts);
            d.concat(&[
                d.text("("),
                d.indent(d.concat(&indent_parts)),
                d.hardline(),
                d.text(")"),
            ])
        } else {
            let mut parts: DocBuf = smallvec![d.text("("), inner];
            parts.extend(comment_parts);
            parts.push(d.text(")"));
            d.concat(&parts)
        })
    }

    /// [`Self::build_kept_paren_shell_doc`] over a FROZEN value's verbatim slice — the
    /// retained arm of [`Self::build_shell_value_doc`] and the frozen sequence's own
    /// required pair, which is the same emission.
    ///
    /// The pair is unconditional here, unlike the unfrozen twin's bare fallback: every
    /// path that reaches this one prints a `)` in the output (the position's clarity pair,
    /// or the sequence's required one), so a gap that turns out to hold nothing to emit
    /// still owes the parens.
    ///
    /// The **arrow body** asks it directly rather than through [`Self::build_shell_value_doc`]:
    /// its retained-paren arm reassembles the body itself
    /// ([`Printer::build_arrow_expression_body`]), and answering that gap with a bare
    /// `parens()` would leave it with no emitter and DROP the comment inside it
    /// (`docs/comments.md` hazard 4). One emitter, so the frozen and unfrozen forms of every
    /// retained shell keep agreeing about where the comment renders.
    pub(crate) fn build_frozen_kept_paren_doc(&self, frozen: Span, boundary_end: u32) -> DocId {
        let inner = self.build_frozen_node_doc(frozen);
        self.build_kept_paren_shell_doc(inner, frozen.end, boundary_end)
            .unwrap_or_else(|| self.d().parens(inner))
    }

    /// Promote block comments that appear before an assignment operator to the LHS.
    ///
    /// In `a /* comment */ = b`, the comment is between `left.span().end` and `right.span().start`
    /// but positioned before the `=` in source. Prettier places such comments before the operator,
    /// so we promote them to the LHS doc.
    ///
    /// Returns the promoted comments doc (with leading space) and the new RHS comment start
    /// position, or None if no comments need promoting.
    ///
    /// **Block comments only** — the ones that can sit inline before the operator. A
    /// comment that cannot (a `//`, or a multiline block the author broke after)
    /// stays put and takes the operator's tail with it onto a continuation line;
    /// that gap is answered by [`Printer::build_operator_value_continuation`], which
    /// the caller consults first. Emitting such a comment here would swallow the
    /// operator into it; leaving it to the RHS emitter would relocate it *past* the
    /// operator.
    ///
    /// `op_pos` is the operator's offset, found once by the caller
    /// ([`Printer::find_operator_in_source`]) and shared with that gate.
    pub(crate) fn promote_comments_before_operator(
        &self,
        start: u32,
        op_pos: u32,
    ) -> Option<(DocId, u32)> {
        let d = self.d();
        // Collect block comments that appear before the operator
        let mut promoted_parts = DocBuf::new();
        let mut last_promoted_end = start;
        for comment in self.comments_to_emit_between(start, op_pos) {
            if comment.is_block {
                promoted_parts.push(d.text(" "));
                promoted_parts.push(self.build_comment_doc(comment));
                last_promoted_end = comment.span.end;
            }
        }

        if promoted_parts.is_empty() {
            None
        } else {
            Some((d.concat(&promoted_parts), last_promoted_end))
        }
    }

    /// Find the position of an operator string between two positions, skipping
    /// whitespace and comments in the source.
    ///
    /// The multi-byte sibling of [`Printer::find_equals_position`] (a bare `=`, with a
    /// midpoint fallback rather than `None`); both step over comments through
    /// [`tsv_lang::source_scan::skip_comment`], so a `//` or `/* */` in the gap can
    /// never hide the operator behind its text.
    pub(crate) fn find_operator_in_source(
        &self,
        start: u32,
        end: u32,
        operator: &str,
    ) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let op_bytes = operator.as_bytes();
        let op_len = op_bytes.len();
        let end_usize = end as usize;
        let mut i = start as usize;

        while i + op_len <= end_usize {
            if let Some(past_comment) = tsv_lang::source_scan::skip_comment(bytes, i, end_usize) {
                i = past_comment;
                continue;
            }
            if &bytes[i..i + op_len] == op_bytes {
                return Some(i as u32);
            }
            i += 1;
        }
        None
    }

    /// Prepend comments from removed parentheses to a doc.
    ///
    /// When parentheses are removed during parsing (e.g., `(/* comment */ expr)` becomes `expr`),
    /// the expression's span extends to include the removed parens. Comments between
    /// `outer_start` (the paren) and `inner_start` (the expression) need to be preserved.
    ///
    /// Returns the original doc unchanged if no comments or if `outer_start >= inner_start`.
    #[inline]
    pub(crate) fn prepend_removed_paren_comments(
        &self,
        outer_start: u32,
        inner_start: u32,
        doc: DocId,
    ) -> DocId {
        self.prepend_opt(
            self.removed_paren_comments_opt(outer_start, inner_start),
            doc,
        )
    }

    /// The run [`Self::prepend_removed_paren_comments`] prepends, on its own — for a
    /// caller that must place it around a doc it has not finished building (the root
    /// conditional, whose run goes OUTSIDE the group it wraps its test in: `docs/comments.md`
    /// §The left-spine shell run). `None` when the gap is empty or inverted, so the two
    /// spellings of the guard cannot drift.
    pub(crate) fn removed_paren_comments_opt(
        &self,
        outer_start: u32,
        inner_start: u32,
    ) -> Option<DocId> {
        (outer_start < inner_start)
            .then(|| self.build_rhs_comments_opt(outer_start, inner_start))
            .flatten()
    }

    /// The retained-shell form for a **statement value** whose authored grouping parens
    /// hold a `//` — `export default (⏎\tx // c⏎)`, `export = (⏎\tx // c⏎)`.
    ///
    /// These positions print no pair of their own around the value, so without this the
    /// comment falls to the statement's terminator gap and defers past the `)` and the
    /// `;`, onto a line that may already hold a `//` — where the two merge into one
    /// comment. Only a **line** comment needs the shell: a block trails without ending
    /// its line, so its relocation past the `;` is lossless and stable, and tsv matches
    /// prettier there.
    ///
    /// Returns the shell and the position just past the retained `)`, which is where the
    /// caller's terminator-gap scan resumes. `None` leaves the caller's plain path alone —
    /// no authored paren, or no `//` inside one.
    pub(crate) fn build_value_paren_line_comment_shell(
        &self,
        value_doc: DocId,
        value_end: u32,
        span_end: u32,
    ) -> Option<(DocId, u32)> {
        let close = self.value_paren_line_comment_close(value_end, span_end)?;
        let shell =
            self.build_paren_operand_comment_doc(value_end, close, value_doc, value_doc, ")")?;
        Some((shell, Self::past_grouping_close(close, span_end)))
    }

    /// Where the shell above closes, when it applies — the value's authored grouping `)`
    /// with a `//` inside it. Split out for the caller that must know the close *before*
    /// it can build the value doc (`export =` builds its value inside a keyword-header
    /// closure), so the two answer one question rather than two.
    pub(crate) fn value_paren_line_comment_close(
        &self,
        value_end: u32,
        span_end: u32,
    ) -> Option<u32> {
        let close = self.retained_grouping_close(value_end, span_end)?;
        self.has_line_comments_between(value_end, close)
            .then_some(close)
    }

    /// Where a caller's terminator-gap scan resumes: just past the retained `)`.
    pub(crate) fn past_grouping_close(close: u32, span_end: u32) -> u32 {
        close.saturating_add(1).min(span_end)
    }

    /// The one emitter for a comment the author wrote between a **parenthesized**
    /// operand and the `)` that closes its shell — `(x + y /* c */)!`,
    /// `(a?.b // c⏎)!`, `<T>(x /* c */)`.
    ///
    /// tsv keeps such a comment INSIDE the parens, where it was written; prettier
    /// relocates it past the `)` (cataloged as the non-null grouped-operand and
    /// angle-bracket assertion-operand divergences). Four constructs reach this gap and
    /// must answer it identically: the standalone non-null whose operand needs its
    /// parens (`build_ts_non_null_doc`'s needs-parens arm), the chain's parenthesized
    /// base (`ChainNode::Base`'s `paren_comment_end`), the required-paren positions
    /// that never enter a chain — a `new` callee and a template tag
    /// (`build_sealed_non_null_paren_doc`) — and the angle-bracket type assertion,
    /// whose own span is what ends at the `)` (`build_ts_type_assertion_doc`, which
    /// calls from each of its two return paths).
    ///
    /// `flat_body` and `broken_body` are the same doc at every caller but the chain
    /// base, which alone has two renderings of its operand — the split is earned there
    /// and nowhere else.
    ///
    /// Returns `None` when the gap holds nothing to emit, leaving the caller to
    /// render its own bare parens — which is what makes the retention the comment's
    /// doing: an empty gap still strips a redundant shell.
    ///
    /// `broken_body` renders the expanded layout — a `//` cannot trail inline before
    /// the `)` (it would swallow it), and a multi-line block already makes the shell
    /// span lines, where a shell that breaks expands rather than gluing its operand to
    /// the `(` (the rule [`Self::trailing_paren_comment_parts`] states for the same
    /// gap) — so the operand goes multiline with the comment inside; `flat_body`
    /// renders the inline single-line-block one. `close` is what follows the operand: `")"` where a
    /// separate node prints the `!` (or where nothing does), `")!"` where this doc owns
    /// it.
    pub(crate) fn build_paren_operand_comment_doc(
        &self,
        start: u32,
        end: u32,
        flat_body: DocId,
        broken_body: DocId,
        close: &'static str,
    ) -> Option<DocId> {
        let d = self.d();
        if self.has_line_spanning_comments_to_emit_between(start, end) {
            // Every comment in this gap was authored AFTER the operand — there is no
            // next node for one to lead — so the whole run trails, in authored order,
            // on the anchored emitter (the layout is vertical: the closer's hardline
            // below ends every line, and flushes the run's deferred `//`s; a boundary
            // instead would end the line first, landing a blank before the closer).
            // A block-only run holding a multi-line block takes the same emitter: it
            // trails inline on the operand's line, and the closer drops below it.
            // A chain-gap classification here is a category error: its `leading_*`
            // buckets would hoist an own-line comment above the operand.
            let mut body = DocBuf::with_capacity(3);
            body.push(broken_body);
            self.push_anchored_trailing_run(&mut body, start, end, RunLeadingBlank::Keep);
            return Some(self.compose_expanded_shell_doc(None, &body, close));
        }
        if self.has_comments_to_emit_between(start, end) {
            let trailing = self.build_chain_block_comments_doc(start, end, CommentSpacing::Leading);
            return Some(d.concat(&[d.text("("), flat_body, trailing, d.text(close)]));
        }
        None
    }
}
