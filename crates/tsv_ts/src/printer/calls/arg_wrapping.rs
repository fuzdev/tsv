// Argument classification and wrapping utilities for call expressions
//
// Handles:
// - Argument classification for chain contexts
// - Call expression wrapping with soft/hard breaks
// - Building argument lists split into head/last patterns

use super::super::{
    ArrowChainContext, AssumedBreak, CommentSpacing, Printer, ShareTag, arrow_chain_should_break,
    is_curried_arrow_chain, is_multiline_template_expression,
};
use super::arg_comments::{
    any_arg_gap_has_comment_on_page, build_arg_gap_docs, build_empty_args_parens_doc,
    emit_first_arg_leading_comments, emit_last_arg_trailing_comments,
};
use super::arg_predicates::{
    arrow_body_has_leading_own_line_comment, arrow_body_is_call_through_non_null,
    is_block_function, is_react_hook_call_with_deps_array, is_short_second_arg_for_expand_first,
    is_ternary_arrow_body,
};
use crate::ast;
use crate::printer::expressions::functions::{
    arrow_signature_has_breaking_comments, arrow_signature_refuses_hug, arrow_token_end,
    flat_sole_param_literal, function_return_hug_break, prepend_leading,
};
use smallvec::{SmallVec, smallvec};
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::source_scan::has_newline_before_position;

/// Build one call-argument doc, routing a curried arrow chain through the progressive
/// call-arg chain layout ([`Printer::build_arrow_chain_doc`], prettier's
/// `printArrowFunctionSignatures`, reached via its `isCallLikeExpression(parent)`).
///
/// **This is the shape of prettier's `printedArguments`** — printed with no
/// `expandLastArg`, so `shouldPrintAsChain` holds and the heads take their own lines.
/// `allArgsBrokenOut()` and the default `contents` path both read that array, so every
/// builder feeding either must build its arguments through here. A builder that instead
/// reused a `skip_arrow_chain` doc (tsv's spelling of `expandLastArg`) for its broken-out
/// state had no chain layout at all, which is the whole bug class this helper closes.
///
/// Prettier's other printing — the `expandLastArg` `lastArg` its hug states read — is
/// [`build_expand_last_arg_doc`]'s, and it is built BESIDE this one only where a hug state
/// renders the argument broken: the object/array terminal, in each of the four argument
/// printers. Every other shape reads one doc, this one. [`build_arrow_hug_arg_docs`] states
/// the rule per shape, and why paying for a second build off the injected-body path would be
/// 2^depth.
///
/// **Every** curried chain is routed, break-forcing or not — the binaryish site
/// (`build_chain_aware_operand_doc`) has always done the same, and the refusal lives in one
/// place, `should_use_arrow_chain_layout`. A second copy of it here is what kept prettier's
/// `shouldBreakChain` from ever reaching a call argument: the trigger it names (a return
/// type with params, type params, a non-identifier param) is a **break** decision inside the
/// chain layout, not a reason to leave it, and gating the context on it meant the outer
/// arrow fell back to the default layout, which owns no chain break in this position.
///
/// `build` stays a closure because the callers disagree on which builder to run
/// (`build_expression_doc` vs `build_arg_expression_doc`) — only the context is shared.
/// ⚠️ It also **clears `skip_arrow_chain` for the duration of the build**, and that is a
/// correctness fix rather than hygiene. Prettier's `expandLastArg` is an argument to one
/// `print()` call — it describes the node being printed and nested prints never inherit it —
/// whereas tsv spells it as ambient `Printer` state, so without a clear it survives the
/// descent: the hug of a chain `arrow_chain_should_break` forces open (one of the positions
/// that set it) suppressed the progressive layout of an entirely different chain
/// nested in its body
/// (`fn(({ a }) => ({ b }) => { return g((x, …) => (y) => z); })`). Every argument of every
/// nested call routes through here, which is exactly the seam where the flag stops applying.
///
/// ⚠️ Only the CHAIN half stops here, and the asymmetry is the two halves' consumers rather
/// than an omission. `expand_last_arg_flat_params` — prettier's `removeLines` — is consumed
/// one level in, at the arrow's own body seam (`build_arrow_doc_wrapping`), which is where
/// prettier's single `print("body", args)` edge ends: the flag reaches the hugged argument
/// and travels only down an arrow-body spine, so nothing under a non-arrow body ever sees it
/// and this seam has nothing left to clear. `skip_arrow_chain` is read ABOVE any body, by the
/// chain-layout gate at the top of that same builder, so a nested argument would still enter
/// it under the outer flag — hence the clear here.
pub(super) fn build_printed_argument_doc(
    printer: &Printer<'_>,
    arg: &ast::Expression<'_>,
    build: impl FnOnce() -> DocId,
) -> DocId {
    let outer_skip = printer.skip_arrow_chain.replace(false);
    let doc = if is_curried_arrow_chain(arg) {
        printer.build_with_arrow_chain_context(ArrowChainContext::CallArgOrBinaryish, build)
    } else {
        build()
    };
    printer.skip_arrow_chain.set(outer_skip);
    doc
}

/// Build an arrow function signature doc for a call-argument state.
///
/// Does NOT include the ` =>` — the caller adds that, which is why the signature→`=>` gap
/// is claimed here (see [`Printer::append_pre_arrow_comments`]).
///
/// One builder for both shapes. An untyped arrow keeps its "no break points" property from
/// `remove_lines` rather than from a second builder: the hand-rolled twin that used to
/// serve it joined its parameters with a bare `", "` and asked no gap anything, so **every**
/// comment in the list was dropped (`fn((a /* c */, b) => …)`, and the leading / later /
/// last positions alike) while the plain arrow and the member chain printed them — the
/// `docs/comments.md` hazard-4 shape, down to its empty arm being comment-aware and its
/// non-empty arm not. Flattening loses nothing the twin kept: a `//` here is a parse error
/// (`[no LineTerminator here]` precedes `=>`), and a multiline block renders inline either
/// way — it merely stays flatter than prettier, which expands the whole call around it,
/// a layout divergence this shape already had while the comment was being dropped.
pub(super) fn build_arrow_sig_doc(
    printer: &Printer<'_>,
    arrow: &ast::ArrowFunctionExpression<'_>,
    span: Span,
) -> DocId {
    let d = printer.d();
    // The signature→`=>` gap rides inside the group, exactly as the plain arrow path
    // composes it. Without it this reassembly drops every comment there — the states
    // below emit `" =>"` themselves, so no other emitter can reach the gap.
    let sig = printer.append_pre_arrow_comments(
        arrow,
        span,
        printer.build_arrow_signature_doc(arrow, span),
    );
    // ⚠️ The two arms are NOT interchangeable, and a green fixture suite does not say they
    // are: `remove_lines` and `group` differ inside an outer FLAT `fits` walk, which no
    // fixture here reaches (the TODO records two such "equivalent" deletions that had to be
    // reversed). This keeps each arm's historical layout exactly — the fix above is the gap
    // claim, not the break policy.
    //
    // An empty list holding a comment takes the grouped arm too: prettier's
    // `printFunctionParameters` returns an empty list ahead of its `removeLines`, so a
    // dangling `//` keeps its own line (`(⏎\t// c⏎) =>`) where `remove_lines` welded it to
    // the `(` and left the `)` at the line start.
    let sig = if arrow_has_return_or_type_params(arrow)
        || printer.arrow_empty_params_hold_comments(arrow)
    {
        d.group(sig)
    } else {
        d.remove_lines(sig)
    };
    // Every call-argument state that reassembles an arrow from signature + body starts
    // here, and none of them route the arrow through `build_expression_doc` — so this is
    // the only place its owned leading comment can be claimed. An owned comment nothing
    // prints is a *dropped* comment (`f(/** @param {any} n */ (n) => g(n))`), so the
    // claim must live on the same seam the reassembly does. See `comments/owned.rs`.
    printer.prepend_owned_leading_comment_at(span.start, sig)
}

/// The signature a reassembling hug state prints ([`build_arrow_sig_doc`]), or `None` when
/// the signature refuses the hug ([`arrow_signature_refuses_hug`]).
///
/// The states print the signature flattened on the callee's line, so a signature that will
/// break has no state here: printed there it would either collapse the break the author
/// wrote or break mid-hug. The caller's ladder is left with the all-broken-out state, which
/// prints the argument whole — and nothing is built for a signature no state prints. The one
/// break that is not refused is a comment in an empty parameter list, which prettier's
/// bailout never reads: the signature keeps it on its own line and the ladder hugs the
/// argument broken open ([`build_break_body_ladder`]).
pub(super) fn build_hugged_arrow_sig_doc(
    printer: &Printer<'_>,
    arrow: &ast::ArrowFunctionExpression<'_>,
    span: Span,
) -> Option<DocId> {
    (!arrow_signature_refuses_hug(printer, arrow, span))
        .then(|| build_arrow_sig_doc(printer, arrow, span))
}

/// Prepend any comments between arrow `=>` and body expression to `body_doc`, for a state
/// that puts the body BELOW `=>` (`(sig =>\n  body,\n)`).
///
/// When call argument paths build `sig_doc` and `body_doc` separately, comments between
/// `=>` and the body are not part of either doc. This finds them and prepends to
/// `body_doc`, returning it unchanged if none exist. A state that keeps the body on the
/// `=>` line takes [`prepend_hugged_arrow_body_comments`] instead.
pub(super) fn prepend_arrow_body_comments(
    printer: &Printer<'_>,
    arrow: &ast::ArrowFunctionExpression<'_>,
    body_start: u32,
    body_doc: DocId,
) -> DocId {
    let arrow_end = arrow_token_end(arrow);

    // A run the author broke AFTER keeps that break. The body sits below `=>` in every state
    // this doc reaches, and there prettier's leading comment that ends its line
    // (`hasLeadingOwnLineComment`) prints the `line` after it in the group the `=>` break
    // broke, so it breaks too (`(a) =>⏎\t/* c */⏎\ta ? b : c`) — whatever the body does,
    // where the arrow's own arms ask whether the break is forced
    // (`Printer::break_or_hang_after_operator_run_doc`) because their body may stay on the
    // `=>` line. Glued, the run welded onto the body (`/* c */ a ? b : c`), a form the next
    // pass reads as a comment that does not end its line — and measures the `=>` line
    // differently ([`arrow_body_has_leading_own_line_comment`]).
    //
    // The author blank after the run answers as at every other arrow host (the default
    // layout's dispatch, `Printer::build_arrow_expression_body`'s non-hug arms): a run no
    // comment of which owns its line ([`Printer::value_gap_soft_broke_after_run`]) whose
    // break is not forced ([`Printer::broke_after_run_break_is_forced`]) takes the soft
    // half ([`Printer::push_leading_run_with_soft_line`]), whose `line` yields the blank.
    // Here that `line` always renders broken — its group is pre-broken, since this state
    // already put the body below `=>` — so the comment keeps its own line and the blank
    // goes, as `const v = (a) => /* c */⏎⏎body` does. A forced break, or a comment that
    // owns its line, takes the forced emitter, which keeps the blank.
    if let Some(run) = printer.value_gap_soft_broke_after_run(arrow_end, body_start)
        && !printer.broke_after_run_break_is_forced(&run, body_doc)
    {
        let d = printer.d();
        let mut parts = DocBuf::new();
        printer.push_leading_run_with_soft_line(&mut parts, &run);
        return d.concat(&[d.group_break(d.concat(&parts)), body_doc]);
    }
    if let Some(run) = printer.broke_after_value_leading_run(arrow_end, body_start) {
        return printer.leading_run_before_breaking_value_doc(&run, body_start, body_doc);
    }
    prepend_hugged_arrow_body_comments(printer, arrow, body_start, body_doc)
}

/// Prepend the `=>`→body gap's comments to a body that stays on the `=>` line — the member
/// chain's hugged `(sig => <body>)` state. Glued: a single-line block hugged to `=>` stays
/// with the body across a source newline, matching the main arrow-body path
/// (`has_own_line_post_arrow_comment`) and prettier.
///
/// ⚠️ **Never the forced run of [`prepend_arrow_body_comments`].** Its hardlines break AFTER
/// the comment while this state keeps `=> /* c */` on the head line, so the body landed at the
/// statement's indent (`map((item) => /* c */⏎⏎({`), a form the next pass reads as the
/// gap-break state and prints differently — `blanks:audit` found it.
pub(super) fn prepend_hugged_arrow_body_comments(
    printer: &Printer<'_>,
    arrow: &ast::ArrowFunctionExpression<'_>,
    body_start: u32,
    body_doc: DocId,
) -> DocId {
    let arrow_end = arrow_token_end(arrow);
    if let Some(lc) = printer.build_rhs_comments_glued_opt(arrow_end, body_start) {
        printer.d().concat(&[lc, body_doc])
    } else {
        body_doc
    }
}

/// Wrap arguments in a groupable call expression: `callee(args)`
/// Uses soft breaks so the call can collapse to a single line if it fits.
///
/// The callee spelling of [`ArgOpener::wrap_soft`], kept as a free function because the
/// plain-call and `new` printers hold a bare callee doc rather than an opener.
#[inline]
pub(super) fn wrap_call_with_soft_breaks(d: &DocArena, callee: DocId, args: DocId) -> DocId {
    ArgOpener::callee(d, callee).wrap_soft(d, args)
}

/// The callee spelling of [`ArgOpener::wrap_hard_broken`], kept as a free function for the
/// same reason [`wrap_call_with_soft_breaks`] is: the plain-call and `new` printers hold a
/// bare callee doc rather than an opener.
///
/// Every caller goes through THIS rather than a paren-line-less spelling, since a `(`-line
/// comment run is always a possibility here and dropping it on the floor is content loss.
#[inline]
pub(super) fn wrap_call_with_hard_breaks_paren_line(
    d: &DocArena,
    callee: DocId,
    paren_line: &[DocId],
    args: DocId,
) -> DocId {
    ArgOpener::callee(d, callee).wrap_hard_broken(d, paren_line, args)
}

/// Wrap arguments with a `will_break` guard: if any arg contains hardlines
/// (e.g., multi-line arrow bodies, block functions), force the group to break
/// so args expand onto separate lines. Otherwise use soft breaks.
///
/// Matches Prettier's `group(contents, { shouldBreak: printedArguments.some(willBreak) })`.
#[inline]
pub(super) fn wrap_call_with_will_break_guard(d: &DocArena, callee: DocId, args: DocId) -> DocId {
    if d.will_break(args) {
        d.concat(&[
            callee,
            d.group_break(d.concat(&[
                d.text("("),
                d.indent_softline(args),
                d.softline(),
                d.text(")"),
            ])),
        ])
    } else {
        wrap_call_with_soft_breaks(d, callee, args)
    }
}

/// Check if a single argument needs soft-break wrapping (not huggable)
///
/// Call expressions, member expressions, new expressions, identifiers, and conditionals
/// should allow breaking after "(" so the outer call can break before the inner expression.
/// Objects and arrays are "huggable" and don't need soft wrapping.
///
/// Conditionals (ternaries) are included because when a call's ternary argument exceeds
/// print width, Prettier breaks after "(" and keeps the ternary on one line (if it fits),
/// rather than keeping "(cond" hugged and breaking the ternary at ? and :.
pub(super) fn arg_needs_soft_wrap(arg: &ast::Expression<'_>) -> bool {
    matches!(
        arg.kind,
        ast::ExpressionKind::CallExpression(_)
            | ast::ExpressionKind::MemberExpression(_)
            | ast::ExpressionKind::NewExpression(_)
            | ast::ExpressionKind::Identifier(_)
            | ast::ExpressionKind::ThisExpression(_)
            | ast::ExpressionKind::ConditionalExpression(_)
    )
}

/// How a single argument should be formatted in chain context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChainArgKind {
    /// Hugs naturally - objects, arrays, block bodies have their own formatting
    HugsNaturally,
    /// Needs huggable wrapper - ternaries hug but need trailing comma wrapper
    NeedsWrapper,
    /// Needs soft wrap - long strings, identifiers need call to expand
    NeedsSoftWrap,
}

/// Classify how a single argument should be formatted in chain context.
///
/// Most expressions need soft wrapping so the call can break before the argument.
/// Only objects, arrays and `function`s hug naturally, including when wrapped in TS type
/// assertions (`{...} as T`, `[...] satisfies T`); arrow functions are classified by their
/// body type. A class expression is NOT huggable: prettier's `couldExpandArg` has no class
/// arm, so a member call breaks a sole class argument onto its own line exactly as a plain
/// call does (`calls/chain_class_arg_no_hug`).
pub(super) fn classify_chain_arg(arg: &ast::Expression<'_>) -> ChainArgKind {
    match &arg.kind {
        // Block-like expressions hug the call parens naturally
        ast::ExpressionKind::ObjectExpression(_)
        | ast::ExpressionKind::ArrayExpression(_)
        | ast::ExpressionKind::FunctionExpression(_) => ChainArgKind::HugsNaturally,
        // TS cast wrappers: classify based on the inner expression
        // e.g., `{...} as any` hugs, `longExpr as T` soft-wraps. Mirrors prettier's
        // couldExpandArg, which looks through `as`/`satisfies`/`<T>` but NOT a
        // non-null assertion, so `{...}!` / `[...]!` soft-wraps rather than hugging.
        ast::ExpressionKind::TSAsExpression(e) => classify_chain_arg(e.expression),
        ast::ExpressionKind::TSSatisfiesExpression(e) => classify_chain_arg(e.expression),
        ast::ExpressionKind::TSTypeAssertion(e) => classify_chain_arg(e.expression),
        // Arrow functions: prettier's couldExpandArg keys on the body type and
        // looks through the return-type annotation, so arrows are classified by
        // their body regardless of any return type.
        ast::ExpressionKind::ArrowFunctionExpression(arrow) => classify_arrow_body(arrow),
        // Everything else needs soft wrapping so the call can break
        // before the argument, giving the argument a fresh line to fit on
        _ => ChainArgKind::NeedsSoftWrap,
    }
}

/// Check if an arrow function has a return type or type parameters.
///
/// These are the parts that need real break points, so an arrow carrying either takes the
/// grouped signature in a call-argument state while the rest take the flattened one
/// ([`build_arrow_sig_doc`]). A param-level type annotation does NOT count: it introduces
/// no break point of its own, so a params-only-typed arrow renders identically either way.
fn arrow_has_return_or_type_params(arrow: &ast::ArrowFunctionExpression<'_>) -> bool {
    arrow.return_type.is_some() || arrow.type_parameters.is_some()
}

/// Classify how an arrow function body should be formatted in chain context.
fn classify_arrow_body(arrow: &ast::ArrowFunctionExpression<'_>) -> ChainArgKind {
    match &arrow.body {
        ast::ArrowFunctionBody::BlockStatement(_) => ChainArgKind::HugsNaturally,
        ast::ArrowFunctionBody::Expression(expr) => classify_expression_body(expr),
    }
}

/// Check if a nested arrow chain has an expandable terminal body.
///
/// Matches prettier's `couldExpandArg(arg.body, true)` — the `arrowChainRecursion`
/// flag disables Call and Conditional expansion inside arrow chains, but Block,
/// Object, and Array bodies remain expandable at any nesting depth.
///
/// Examples:
/// - `() => () => { block }` → true (block body)
/// - `() => () => ({obj})` → true (object body)
/// - `() => () => [arr]` → true (array body)
/// - `() => () => call()` → false (call in arrow chain)
/// - `() => () => cond ? a : b` → false (conditional in arrow chain)
///
/// The chain walk is [`arrow_terminal_expression_body`]'s — `None` there IS the block
/// terminal, so this is that predicate plus the two literal kinds, and the two cannot drift
/// on how deep a chain reaches.
pub(super) fn could_expand_arrow_chain(arrow: &ast::ArrowFunctionExpression<'_>) -> bool {
    arrow_terminal_expression_body(arrow).is_none_or(|body| {
        matches!(
            body.kind,
            ast::ExpressionKind::ObjectExpression(_) | ast::ExpressionKind::ArrayExpression(_)
        )
    })
}

/// Prettier's `couldExpandArg` for an **arrow** argument — the pure body-kind half of
/// `shouldExpandLastArg`, with none of the comment refusals layered over it.
///
/// The chain half is [`could_expand_arrow_chain`] (block / object / array terminal, at any
/// nesting); the two direct-body kinds beside it are the ones `arrowChainRecursion` turns
/// off one level down, so they are asked of THIS arrow's body rather than the terminal's.
/// Anything else — a member, an identifier, a binary, a template, a `new` — prettier never
/// prints with `expandLastArg` at all: `printCallArguments` falls through to its default
/// `group(contents)`, where a break anywhere inside the arguments breaks every one of them
/// out.
///
/// Kept separate from the hug-eligibility spellings that layer
/// [`arrow_hug_refused_by_comments`] on top: a comment refusal answers "can THIS arm render
/// the argument", a different question from "would prettier expand it at all", and the gap
/// break below needs the second one alone.
fn could_expand_arrow_arg(arrow: &ast::ArrowFunctionExpression<'_>) -> bool {
    could_expand_arrow_chain(arrow)
        || match &arrow.body {
            ast::ArrowFunctionBody::Expression(body) => {
                arrow_body_is_call_through_non_null(body) || is_ternary_arrow_body(body)
            }
            ast::ArrowFunctionBody::BlockStatement(_) => false,
        }
}

/// Classify how an expression body should be formatted.
///
/// Note: for arrows without TSTypeReference return types, object/array bodies
/// are caught earlier by the conditional_group path in chain_args.rs. This
/// HugsNaturally classification is primarily reached for arrows with
/// TSTypeReference returns (which bypass that path) and nested arrow chains.
fn classify_expression_body(expr: &ast::Expression<'_>) -> ChainArgKind {
    match &expr.kind {
        // Objects and arrays hug naturally (reached mainly for typed-return arrows)
        ast::ExpressionKind::ObjectExpression(_) | ast::ExpressionKind::ArrayExpression(_) => {
            ChainArgKind::HugsNaturally
        }
        // Ternaries hug but need trailing comma wrapper
        ast::ExpressionKind::ConditionalExpression(_) => ChainArgKind::NeedsWrapper,
        // Nested arrows inherit their body's classification
        ast::ExpressionKind::ArrowFunctionExpression(inner) => classify_arrow_body(inner),
        // Everything else needs soft wrap
        _ => ChainArgKind::NeedsSoftWrap,
    }
}

/// Build an arrow's expression body the same way the whole arrow's own body build does
/// (`build_arrow_doc_wrapping` clears `arrow_chain_context` before building the body — a
/// nested curried arrow in the body must not inherit the outer chain context), so the
/// pre-built DocId is byte-identical to what the arrow would build.
fn build_arrow_body_like_arrow(printer: &Printer<'_>, body_expr: &ast::Expression<'_>) -> DocId {
    let prev = printer.arrow_chain_context.replace(ArrowChainContext::None);
    let doc = printer.build_arrow_arg_body_doc(body_expr);
    printer.arrow_chain_context.set(prev);
    doc
}

impl Printer<'_> {
    /// The BODY of an arrow that a call-argument state reassembles from signature + body,
    /// built as the whole arrow's own body build builds it
    /// (`Printer::build_arrow_body_doc_with_leading`): a multi-line comment the body owns
    /// prints outside the body's own group ([`Printer::build_value_with_outermost_owned_comment`]).
    ///
    /// Built here by the pre-built injection, the lone ternary ladders of the plain call, `new`
    /// and the member chain, and the break-body ladder of a call's or `new`'s last argument.
    /// Claimed by the innermost node instead, the comment's hard break explodes a ternary the
    /// break-body state keeps flat, while the own-line authoring that state prints takes a
    /// claimed route on the next pass — so the state wobbled. (The chain's force-expanded and
    /// multi-argument states build their body plainly: no authoring reaches either as the
    /// printed state of a ternary body, so a claim there could not be pinned.)
    ///
    /// A body whose leftmost token takes its own pair INSIDE it is built through that seam
    /// ([`Printer::build_inner_leftmost_parens_arrow_body_doc`]), as the whole arrow builds it.
    pub(super) fn build_arrow_arg_body_doc(&self, body_expr: &ast::Expression<'_>) -> DocId {
        self.build_inner_leftmost_parens_arrow_body_doc(body_expr)
            .unwrap_or_else(|| self.build_expression_doc_claiming_outermost(body_expr))
    }

    /// [`Self::build_arrow_arg_body_doc`] for the states that build their body unclaimed (the
    /// member chain's force-expanded and multi-argument states): the leftmost pair, else the
    /// plain expression doc.
    pub(super) fn build_arrow_arg_body_doc_unclaimed(
        &self,
        body_expr: &ast::Expression<'_>,
    ) -> DocId {
        self.build_inner_leftmost_parens_arrow_body_doc(body_expr)
            .unwrap_or_else(|| self.build_expression_doc(body_expr))
    }
}

/// Pre-build an expand-last-arg arrow's **break-body-state** body **once** so the whole-arrow
/// argument doc and the break-body state can share it, keeping the doc-node count linear
/// instead of O(2^depth) (see the `arrow_body_inject` field on `Printer`).
///
/// Returns `(body-expr span start, body DocId)` when `last_arg` is an arrow whose expression
/// body routes through [`build_break_body_ladder`] — a **call** (through a trailing `!`) or a
/// **conditional** (ternary). The caller injects it via `Printer::inject_arrow_body` before
/// `build_args_split_last`; the whole arrow reuses it (a call body via `build_arrow_body_doc`,
/// a conditional body via the conditional arm of `build_arrow_expression_body`), and the
/// break-body state reuses the same DocId. A body whose leftmost token takes its own pair is
/// built through the seam the whole arrow's body build asks first
/// ([`Printer::build_arrow_arg_body_doc`]), so it matches too. Returns `None` (unchanged behavior)
/// when the last arg isn't such an arrow.
///
/// A commented call is injected too: one comment in the innermost body puts every enclosing
/// call on page, so declining there rebuilt the body in both docs at every level of nested
/// call-bodied callbacks (`a(l, (x) => b(l, (y) => … z(/* c */ q)))`, O(2^depth)). The
/// comment positions that need a different composition never reach the states that read
/// the injection: the reassembling arms refuse them first (`arrow_hug_refused_by_comments`,
/// and a call's leading or trailing comment on the last argument), and the own-line
/// `=>`→body gap takes its own state, built from its own prebuild.
pub(super) fn prebuild_expand_last_break_body(
    printer: &Printer<'_>,
    last_arg: Option<&ast::Expression<'_>>,
) -> Option<(u32, DocId)> {
    if let Some(ast::Expression {
        kind: ast::ExpressionKind::ArrowFunctionExpression(arrow),
        ..
    }) = last_arg
        && let ast::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        && (arrow_body_is_call_through_non_null(body_expr)
            || matches!(
                body_expr.kind,
                ast::ExpressionKind::ConditionalExpression(_)
            ))
    {
        // Shared across a member chain's `conditional_group` candidates, as the object/array
        // twin's is ([`prebuild_expand_last_obj_array_body`]): this prebuild runs per
        // candidate, and without the share the second candidate rebuilds the whole body.
        // Outside a chain the key is `None` and it builds as before.
        let share_key = printer.chain_share_key(body_expr, ShareTag::ExpandLastBreakBody);
        let body_doc = printer.chain_shared_doc(share_key, || {
            build_arrow_body_like_arrow(printer, body_expr)
        });
        return Some((body_expr.span().start, body_doc));
    }
    None
}

/// Whether the parens an arrow's object/array body is rendered inside are the GRAMMAR's.
///
/// Only an **object** body needs them — a brace-less `{` after `=>` would parse as a block —
/// and an array body needs none, which is what prettier emits (`(x) => [`). Parenthesizing an
/// array's hug slot fabricated a pair prettier never writes (`fn(a, (x) => ([⏎…⏎]))`), and
/// because the fabricated form is its own fixed point, F1, the fuzzer, the round-trip and every
/// comment gate were blind to it — only a prettier compare could see it. One site, so the
/// inject doc and the hug doc cannot drift apart on it.
fn arrow_hug_body_needs_parens(body_expr: &ast::Expression<'_>) -> bool {
    matches!(body_expr.kind, ast::ExpressionKind::ObjectExpression(_))
}

/// The **terminal** arrow of a curried run (`(a) => (b) => X`) — the innermost `=>`, whose
/// body is not itself an arrow; `arrow` unchanged when it heads no chain.
///
/// The chain is walked because prettier's `expandLastArg` print has no chain layout
/// (`printArrowFunction`: `shouldPrintAsChain = !args.expandLastArg && …`), so a curried
/// argument is just a run of signatures ending here — which is why every hug question is
/// keyed on the terminal, at any depth: the body kind, the `=>`→body gap, and the body's
/// pre-built doc are all THIS arrow's.
///
/// One walk, because three call sites had hand-rolled it into three spellings that agreed
/// only by luck — and "how deep does a chain reach" is exactly the kind of question a second
/// spelling answers differently the first time a shape is added.
///
/// The span is walked with it: an arrow node holds no span of its own, so the terminal's
/// span is the one on the body `Expression` that holds it — `None` when `arrow` is its own
/// terminal, whose span only the caller holds. A span-keyed question about the terminal must
/// read this one, never the chain head's: the head's span ends past every stripped `)` of
/// the chain (`x => (y => …)`), so a gap scan over it reads the shell's comments as the
/// terminal's.
fn terminal_arrow<'a, 'arena>(
    arrow: &'a ast::ArrowFunctionExpression<'arena>,
) -> (&'a ast::ArrowFunctionExpression<'arena>, Option<Span>) {
    let mut current = arrow;
    let mut span = None;
    loop {
        let ast::ArrowFunctionBody::Expression(body) = &current.body else {
            return (current, span);
        };
        let ast::ExpressionKind::ArrowFunctionExpression(inner) = &body.kind else {
            return (current, span);
        };
        current = inner;
        span = Some(body.span);
    }
}

/// An arrow's **terminal** expression body — its own, or, through a curried chain
/// (`(a) => (b) => X`), the last head's. `None` for a block terminal.
///
/// [`terminal_arrow`] plus the block-vs-expression split.
fn arrow_terminal_expression_body<'arena>(
    arrow: &ast::ArrowFunctionExpression<'arena>,
) -> Option<&'arena ast::Expression<'arena>> {
    match &terminal_arrow(arrow).0.body {
        ast::ArrowFunctionBody::Expression(body) => Some(body),
        ast::ArrowFunctionBody::BlockStatement(_) => None,
    }
}

/// Whether an arrow's body **expands internally** — an object or array literal, whose own doc
/// breaks while the arrow itself stays hugged to the callee's `(`.
///
/// Asked of the [terminal](arrow_terminal_expression_body), so a curried chain whose last head
/// returns an object or array answers `true`: prettier hugs the whole run of signatures and
/// expands only the terminal (`fn((a) => (b) => ({⏎…⏎}))`).
///
/// ⚠️ **The states this selects must render the argument's `expandLastArg` printing**
/// ([`build_expand_last_arg_doc`]), never its `printedArguments` one. Wrapping the *chain*
/// doc in `group_break` breaks the chain's own heads onto separate lines, and the truncated
/// `fits` walk then reports that as fitting — hugging a many-head chain prettier breaks out
/// on width, since its `expandLastArg` print has no chain layout to break
/// (`expressions/arrow/curried_untyped_call_arg_long` catches exactly that).
pub(super) fn arrow_body_expands_internally(arrow: &ast::ArrowFunctionExpression<'_>) -> bool {
    arrow_terminal_expression_body(arrow).is_some_and(|body| {
        matches!(
            body.kind,
            ast::ExpressionKind::ObjectExpression(_) | ast::ExpressionKind::ArrayExpression(_)
        )
    })
}

/// Pre-build an expand-last-arg arrow's **object/array** terminal body once, returning
/// `(body span, body doc)`.
///
/// The doc is what the whole arrow's `build_arrow_body_doc` produces for this body —
/// `d.parens(obj)` for an object (the leftmost-object parens: `build_arrow_body_doc` wraps the
/// whole-body object in `d.parens` exactly as this does), or the bare array doc for an array.
/// The caller injects it ([`Printer::inject_arrow_body`]) so **both** printings of the
/// argument reuse the one build: prettier's `printedArguments` entry and its `expandLastArg`
/// `lastArg`. Without that, the second printing recurses into any call nested in the body and
/// `f(lead, x => ({ k: f(lead, y => …) }))` costs 2^depth doc nodes (`fanout:audit`).
///
/// Returns `Some` for exactly the shape [`arrow_body_expands_internally`] claims — the two ask
/// the same question of the same [terminal](arrow_terminal_expression_body) — and `None`
/// otherwise. **Deliberately not gated on the call carrying comments**, unlike its
/// `prebuild_expand_last_break_body` sibling: this body kind reaches `build_arrow_body_doc`'s
/// leftmost-object arm, whose only work beside the build is the parens reproduced here, so the
/// injected doc is what that arm would produce with or without comments — and a comment-gated
/// prebuild would leave the second printing to rebuild the subtree, which is the 2^depth shape
/// on any nesting the gate happens to open.
pub(super) fn prebuild_expand_last_obj_array_body(
    printer: &Printer<'_>,
    last_arg: Option<&ast::Expression<'_>>,
) -> Option<(u32, DocId)> {
    let d = printer.d();
    let ast::ExpressionKind::ArrowFunctionExpression(arrow) = &(last_arg?).kind else {
        return None;
    };
    let body_expr = arrow_terminal_expression_body(arrow)?;
    if !matches!(
        body_expr.kind,
        ast::ExpressionKind::ObjectExpression(_) | ast::ExpressionKind::ArrayExpression(_)
    ) {
        return None;
    }
    // Shared across a member chain's `conditional_group` candidates like every other build
    // under one — this prebuild runs per candidate, and without the share it rebuilds the
    // whole terminal subtree each time (`fanout:audit`'s `ts_nested_curried_arrow_obj_chain`
    // caught exactly that). Outside a chain the key is `None` and it builds as before.
    let share_key = printer.chain_share_key(body_expr, ShareTag::ExpandLastBody);
    let doc = printer.chain_shared_doc(share_key, || {
        let raw = build_arrow_body_like_arrow(printer, body_expr);
        // One paren rule, one site ([`arrow_hug_body_needs_parens`]) — the injected doc stands
        // in for what the arrow's own body build would have produced, so it must answer
        // identically.
        if arrow_hug_body_needs_parens(body_expr) {
            d.parens(raw)
        } else {
            raw
        }
    });
    Some((body_expr.span().start, doc))
}

/// Build argument docs split into head parts (with commas), last arg, and broken form
///
/// Used for patterns that keep short args inline with the last arg.
/// Returns (head_parts, last_arg_doc, all_args_broken) where:
/// - head_parts: all but last arg with ", " separators (includes inline block comments)
/// - last_arg_doc: the last argument doc
/// - all_args_broken: all args joined with comma_line() for fallback (includes inline block comments)
pub(super) fn build_args_split_last(
    arguments: &[&ast::Expression<'_>],
    printer: &Printer<'_>,
    paren_open: u32,
    has_comments: bool,
) -> (DocBuf, DocId, DocId) {
    let d = printer.d();
    // Build all args (using build_arg_expression_doc for argument-context parens on
    // assignments, and the indented binary/conditional layouts).
    //
    // A curried arrow-chain argument (`fn(x, (a) => (b) => …)`) routes through the
    // progressive call-arg chain layout — see [`build_printed_argument_doc`], which owns
    // that decision for every caller. (`should_use_arrow_chain_layout` still gates on
    // untyped, and on every comment sitting in a region the chain doc emits.)
    let arg_docs: DocBuf = arguments
        .iter()
        .enumerate()
        .map(|(i, _)| build_joined_argument_doc(printer, paren_open, arguments, i))
        .collect();

    // Leading comments between `(` and the first argument (e.g., /** @type {T} */).
    // Not handled by per-arg building — prepended to both head_parts and all_args_broken.
    // Zero-comment fast gate: the leading + per-gap inline block-comment lookups below
    // are skipped when the whole call has no comment (canonical reference:
    // build_params_doc_with_comments); the structural commas stay unconditional.
    let leading_comment_doc = if has_comments {
        printer.build_rhs_comments_glued_opt(paren_open, arguments[0].span().start)
    } else {
        None
    };

    // Build head docs (all but last) with commas and inline block comments
    // Comments are placed relative to the comma based on their source position
    let mut head_parts = DocBuf::new();
    if let Some(lc) = leading_comment_doc {
        head_parts.push(lc);
    }

    // Each gap's comments as the two docs the break sits between — computed ONCE per gap
    // and read by both states below, because the two states describing the same gap
    // differently is how the broken-out one came to drop the after-comma half entirely.
    // Only the comma is structural; the gap scan is pure comment placement, so gate it.
    let gap_blocks: SmallVec<[(DocBuf, DocBuf); 4]> = if has_comments {
        arguments
            .windows(2)
            .map(|pair| build_arg_gap_docs(printer, pair[0].span().end, pair[1].span().start))
            .collect()
    } else {
        SmallVec::new()
    };

    for (i, doc) in arg_docs.iter().take(arg_docs.len() - 1).enumerate() {
        head_parts.push(*doc);

        // This state is measured flat, so the gap's separator is the space a collapsed
        // `line` renders as; the leading run's own soft `line` collapses with it.
        if let Some((through_comma, leading)) = gap_blocks.get(i) {
            head_parts.extend(through_comma.iter().copied());
            head_parts.push(d.text(" "));
            head_parts.extend(leading.iter().copied());
        } else {
            head_parts.push(d.text(", "));
        }
    }
    let last_arg_doc = arg_docs[arg_docs.len() - 1];

    // Build all_args_broken with the same comma-aware split.
    let mut all_args_parts = DocBuf::new();
    if let Some(lc) = leading_comment_doc {
        all_args_parts.push(lc);
    }

    for (i, doc) in arg_docs.iter().enumerate() {
        all_args_parts.push(*doc);

        if i + 1 < arg_docs.len() {
            // ⚠️ The gap is claimed by TWO emitters that must PARTITION it — everything
            // through the comma trails the previous argument, the leading run leads the
            // next. Emitting only one half here DROPPED every comment on the other side
            // whenever this state was the one selected (`fn(a, /* c */ (b), {})`), while
            // the head state printed it — the same gap, two answers, and only the losing
            // one reachable. Both halves now come from one call, placed together rather
            // than split across two iterations, so the break can only land between them.
            // `docs/comments.md` §The element-comma seam.
            if let Some((through_comma, leading)) = gap_blocks.get(i) {
                all_args_parts.extend(through_comma.iter().copied());
                all_args_parts.push(d.line());
                all_args_parts.extend(leading.iter().copied());
            } else {
                all_args_parts.push(d.comma_line());
            }
        }
    }
    let all_args_broken = d.concat(&all_args_parts);

    (head_parts, last_arg_doc, all_args_broken)
}

/// Prettier's React-hook deps-array layout when the arguments are that shape, else `None`
/// — the SHAPE question ([`super::arg_predicates::is_react_hook_call_with_deps_array`]),
/// its comment conjunct, and the doc, as one seam. Every call-like printer asks it as its
/// first argument-layout question, since that is where `printCallArguments` asks it: above
/// `anyArgEmptyLine` and above every specialized layout.
///
/// `has_comments` is the caller's whole-argument-window gate, so a comment-free call never
/// pays the per-gap scan. `opener` carries the callee-and-`(` for a call or `new` and the
/// chain's own `(` / `?.(`.
///
/// `import(…)` cannot use this — its AST carries `source` + `options` rather than a slice —
/// and asks [`super::arg_predicates::is_hook_callback_with_deps`] with its own two
/// expressions instead.
pub(super) fn try_hook_deps_args_doc(
    printer: &Printer<'_>,
    args: &[&ast::Expression<'_>],
    paren_open: u32,
    call_end: u32,
    has_comments: bool,
    opener: DocId,
) -> Option<DocId> {
    is_react_hook_call_with_deps_array(args, || {
        has_comments && any_arg_gap_has_comment_on_page(args, printer, paren_open, call_end)
    })
    .then(|| build_hook_deps_args_doc(printer, args, paren_open, opener))
}

/// The flat layout itself: every argument on the callee's line, joined by `", "`, inside no
/// group of its own — prettier builds literally `["(", …, ", ", …, ")"]` there. Nothing here
/// can break, so the callback's block body and the deps array break on their own groups and
/// the call never wraps around them.
fn build_hook_deps_args_doc(
    printer: &Printer<'_>,
    args: &[&ast::Expression<'_>],
    paren_open: u32,
    prefix: DocId,
) -> DocId {
    let d = printer.d();
    let mut parts = DocBuf::new();
    parts.push(prefix);
    for i in 0..args.len() {
        if i > 0 {
            parts.push(d.text(", "));
        }
        parts.push(build_joined_argument_doc(printer, paren_open, args, i));
    }
    parts.push(d.text(")"));
    d.concat(&parts)
}

/// A last-argument arrow whose `=>`→body gap **breaks**, plus the body doc that answering
/// the question produced — see [`last_arg_arrow_gap_break`].
pub(super) struct ArrowGapBreak {
    /// The terminal body, pre-built while answering the gap question, for the caller to arm
    /// ([`Printer::with_arrow_body_inject`]) around the state's two printings — the same
    /// injection those printings would have built for themselves, so answering costs no
    /// build of its own. `None` for a shape [`prebuild_arrow_gap_break_body`] does not
    /// reproduce, where the ladder degenerates to one printing.
    pub(crate) inject: Option<(u32, DocId)>,
}

/// Does the last argument's arrow **break its `=>`→body gap**, dropping the body onto its
/// own line — and therefore the call's `)` with it?
///
/// Prettier decides this inside the arrow printer, above every argument layout:
/// `shouldPutBodyOnSameLine` opens with `!hasLeadingOwnLineComment(text, functionBody)`, so
/// such a comment drops the body below `=>` — and the branch that takes it appends
/// `trailingComma + trailingSpace`, where `trailingSpace` is a **softline under
/// `expandLastArg`** (`print/arrow-function.js`, `printArrowFunctionBody`). That softline is
/// the only thing that lands the call's `)` on its own line, and it is appended for **every
/// body kind** — block, object, array, arrow chain alike — which is why, *among the bodies
/// that reach that printing at all*, this question is asked of the gap and not of the body's
/// type.
///
/// ⚠️ **Which bodies reach it is still `couldExpandArg`'s answer** ([`could_expand_arrow_arg`],
/// the gate this opens with). `expandLastArg` is set by `shouldExpandLastArg`, so a member,
/// identifier, binary or template body never enters the arrow printer's hug branch: prettier
/// falls through to `printCallArguments`' default `group(contents)`, where the comment's own
/// forced break breaks every argument out. Answering the gap above that gate hugged a body
/// the same call expands when the comment is absent — the comment deciding a layout it has
/// nothing to do with (`calls/arrow_member_body_own_line_comment`).
///
/// ⚠️ **It is the gap's BREAK, not one spelling of it.** Prettier's `line` (which drops the
/// body) and its `trailingSpace` (which drops the `)`) sit in one group, so the two cannot
/// disagree; tsv's `)` is a separate state, and asking a NARROWER question than the arrow's
/// own is an F1 bug rather than a divergence — the body dropped, `))` stayed glued, and pass
/// 2 re-read the emitted break as an own-line comment and moved the `)`. So both of the
/// arrow's break arms are asked here: [`Printer::has_own_line_post_arrow_comment`] and
/// [`Printer::arrow_gap_broke_after_run`] (whose `will_break` half is answered against the
/// injection below — the very doc the arrow's hug arm will ask it of).
///
/// **The chain walks to the TERMINAL arrow.** `expandLastArg` turns off
/// `shouldPrintAsChain` (`!args.expandLastArg && body is Arrow`), so a curried argument is
/// printed as nested arrows and the softline is appended by the innermost one — the gap that
/// carries the comment in `fn(() => () =>⏎\t// c⏎\t({ a: 1 }))`.
pub(super) fn last_arg_arrow_gap_break(
    printer: &Printer<'_>,
    last_arg: &ast::Expression<'_>,
) -> Option<ArrowGapBreak> {
    let ast::ExpressionKind::ArrowFunctionExpression(outer) = &last_arg.kind else {
        return None;
    };
    // The softline is appended for every body KIND, but only inside a printing prettier
    // reaches with `expandLastArg` — which `shouldExpandLastArg` gates on `couldExpandArg`.
    // A body it would not expand (a member, an identifier, a binary, a template) never gets
    // that printing at all: prettier takes `printCallArguments`' default `group(contents)`,
    // which the comment's own forced break then breaks out ARGUMENT BY ARGUMENT. So the gap
    // question is asked below this gate, not above it — hugging a non-expandable body
    // because a comment happens to sit in its gap makes the comment decide a layout it has
    // nothing to do with, and contradicts the same call's own comment-free answer.
    if !could_expand_arrow_arg(outer) {
        return None;
    }
    let (arrow, terminal_span) = terminal_arrow(outer);
    let arrow_span = terminal_span.unwrap_or(last_arg.span);
    let sig_end = arrow_token_end(arrow);
    let body_start = arrow.body.span().start;
    // The state renders the signature's head on the callee's line, so a signature that will
    // break refuses it as it refuses every other hug ([`arrow_signature_refuses_hug`]) — and
    // the call breaks every argument out, where the signature breaks on a line of its own.
    // Asked here, once for every callee spelling, and last: it builds a signature, which the
    // cheap gap questions spare every call whose gap does not break.
    let refused = || arrow_signature_refuses_hug(printer, outer, last_arg.span);
    if printer.has_own_line_post_arrow_comment(sig_end, body_start) {
        return (!refused()).then(|| ArrowGapBreak {
            inject: prebuild_arrow_gap_break_body(printer, Some(last_arg)),
        });
    }
    // The broke-after arm costs a body doc, so it is asked last and behind its own cheap
    // geometry: the injection IS that body, and `will_break` on it is the same question
    // the hug arm asks of the same DocId once the caller arms it.
    printer.arrow_gap_broke_after_run(arrow, arrow_span, sig_end, body_start)?;
    let inject = prebuild_arrow_gap_break_body(printer, Some(last_arg))?;
    (printer.d().will_break(inject.1) && !refused()).then_some(ArrowGapBreak {
        inject: Some(inject),
    })
}

/// Whether a comment sits in the arrow's body-end→arrow-end gap — an
/// author-parenthesized body's stripped `)` region, or (for an object body) the
/// grammar-required parens a hug layout synthesizes.
///
/// The other end of the arrow body from [`last_arg_arrow_gap_break`],
/// and asked for the same reason: **most** hug states reassemble the argument from a
/// signature doc and a body doc rather than printing the whole arrow, so they are blind to
/// this gap — a comment there reaches no emitter and is DROPPED. The arm must decline,
/// leaving the generic path to print the argument through the comment-aware body
/// cascade, which retains the parens and is also prettier's settled form for the
/// commented shape. **On page**, not to-emit: an owned comment still rides that region.
///
/// ⚠️ **The multi-argument object/array-terminal arms do NOT ask it**, and that is a rule
/// rather than an oversight: they render the argument's own `expandLastArg` printing
/// ([`build_expand_last_obj_array_doc`]), so the gap has an emitter there and the refusal
/// bought nothing. It cost two things. The *single*-argument twin of the same layout
/// (`build_block_arrow_hug_states`) never asked, so tsv hugged
/// `f((x) => ({ … } /* c */))` and broke every argument out for
/// `f(1, (x) => ({ … } /* c */))` — one construct answered two ways. And the broken-out form
/// was **not a fixed point**: with the object now written multi-line, the second pass reads it
/// as a source-multiline `group_break`, the `fits` walk stops at that break, and the inline
/// state wins — F1, from ordinary authored code. Both are pinned by
/// `calls/arrow_hugged_body_paren_comment_long_prettier_divergence`, whose
/// `unformatted_ours_compact` carries the one-line authoring the fixed point erases.
///
/// Asked last in each arm's guard, after the body kind, so an arrow heading for another
/// arm pays no comment lookup.
pub(super) fn arrow_body_tail_has_comments(
    printer: &Printer<'_>,
    span: Span,
    body_expr: &ast::Expression<'_>,
) -> bool {
    printer.has_comments_on_page_between(body_expr.span().end, span.end)
}

/// Does a comment make an expression-body arrow unusable by a **reassembling hug arm** —
/// one that prints a signature doc and a body doc rather than the arrow itself?
///
/// The two causes are independent and both fatal, so every such arm asks for both and the
/// pair is stated here once rather than spelled out at each:
///
/// - [`arrow_signature_has_breaking_comments`] — the hug renders the callee and the
///   signature's head on **one line**, which a break forced inside the signature makes
///   impossible;
/// - [`arrow_body_tail_has_comments`] — the reassembly skips the body-end→arrow-end gap, so
///   a comment there reaches no emitter and is DROPPED.
///
/// Different reasons, one consequence: the arm declines and the general path prints the
/// argument whole. They had drifted into six hand-spelled conjunctions across the three
/// argument printers, half of them wrapped in a call-level `has_comments &&` fast gate and
/// half bare — a shape where a new arm forgets one half and nothing says so. One arm still
/// asks only the tail question; see the TODO at the chain's forced-expansion ARRAY body.
///
/// ⚠️ **Only a REASSEMBLING arm may ask this pair.** An arm that renders the argument's own
/// `expandLastArg` printing asks the signature half alone — the tail half's whole argument is
/// the reassembly, and asking it there is a layout bug rather than caution
/// ([`arrow_body_tail_has_comments`]).
///
/// Ask it **after** the body kind, so an arrow heading for another arm pays no comment
/// lookup; a caller holding a call-level "any comment on page" flag may gate it on that too,
/// which is free — either question implies a comment inside the call's own window.
pub(super) fn arrow_hug_refused_by_comments(
    printer: &Printer<'_>,
    arrow: &ast::ArrowFunctionExpression<'_>,
    span: Span,
    body_expr: &ast::Expression<'_>,
) -> bool {
    arrow_signature_has_breaking_comments(printer, arrow, span)
        || arrow_body_tail_has_comments(printer, span, body_expr)
}

/// Assemble the single `expandLastArg` state
/// [`last_arg_arrow_gap_break`] selects: the head arguments stay inline, the
/// last one breaks, and the softline drops `)` to its own line
/// (`fn(a, () =>⏎\t// c⏎\t({ b: 1 })⏎);`). Without it every hug state glues `))` onto the
/// body line, which prettier never emits.
///
/// ⚠️ **The pair must be asked BEFORE the hug states, not selected among them.** The
/// comment's forced break truncates the `fits()` walk (tsv has no `propagateBreaks`, so a
/// `conditional_group` measures a state flat to its first hardline), which then reports the
/// hug as fitting — the wrong state, chosen on a measurement that stopped at the comment.
/// Split from the predicate so a caller can ask the cheap comment question first and build
/// the argument doc only on the rare arm that needs it: a second build of an argument
/// recurses into any call nested in its body, so an eager one costs 2^depth doc nodes.
///
/// One rule for all six sites — the single-argument hug and the multi-argument expand-last
/// of each of the three call-argument printers — because they have drifted apart on exactly
/// this kind of arm before. `head_parts` is empty at the single-argument sites.
///
/// ⚠️ **It is a STATE, not a layout — the ladder below is what makes it one.** Prettier does
/// not have a state for this shape at all: the own-line comment merely lands a hardline
/// inside the `expandLastArg` printing, which stays inside `printCallArguments`'
/// `conditionalGroup` and therefore keeps `allArgsBrokenOut()` behind it. See
/// [`build_arrow_gap_break_ladder`], which every caller takes instead of this.
fn build_arrow_gap_break_state(
    d: &DocArena,
    opener: ArgOpener,
    head_parts: &[DocId],
    last_arg_doc: DocId,
) -> DocId {
    d.group_break(d.concat(&[
        opener.open,
        d.concat(head_parts),
        d.group_break(last_arg_doc),
        d.softline(),
        d.text(")"),
    ]))
}

/// The own-line-post-arrow **ladder**: that state, then `allArgsBrokenOut()`.
///
/// The fallback is the whole point. The state renders the signature on the callee's line, and
/// with a hardline waiting in the body the `fits()` walk stops right after it — so the only
/// thing measured IS that signature, and a signature too wide for the line has nowhere to go
/// without a second state. tsv had none, and the two spellings of "nowhere to go" were both
/// wrong: where the signature stayed breakable it **shattered in place**
/// (`fn(({⏎\ta,⏎\tb⏎}) =>⏎\t// c⏎\t…)`), and where a caller had flattened it
/// (`build_flat_params_arg_doc`) it simply **overflowed**, printing past the print width with
/// no break candidate left. Prettier does neither: its `allArgsBrokenOut()` drops the whole
/// argument to its own line and the signature then breaks there if it still must. Pinned by
/// `calls/arrow_object_body_own_line_comment_long`.
///
/// ⚠️ **The two states read the two PRINTINGS, and both halves are load-bearing.** The state
/// takes the `expandLastArg` one, whose parameters are flat — without that, `fits()` reaches
/// the signature's first parameter `line` (a plain `group` inherits BREAK mode from the
/// state's own `group_break`, prettier's `fits` rule) and stops there, so the state is
/// selected on a measurement that never saw the signature's width and the parameters shatter
/// in place. The fallback takes the `printedArguments` one, whose parameters can still break
/// once the argument has a line of its own. Handing either state the other's doc reproduces
/// one of the two bugs exactly.
fn build_arrow_gap_break_ladder(
    d: &DocArena,
    opener: ArgOpener,
    head_parts: &[DocId],
    expanded_last_arg_doc: DocId,
    all_args_broken: DocId,
) -> DocId {
    opener.ladder(
        d,
        &[
            build_arrow_gap_break_state(d, opener, head_parts, expanded_last_arg_doc),
            opener.expand_all_state(d, all_args_broken),
        ],
    )
}

/// The two printings [`build_arrow_gap_break_ladder`]'s states read, for the two
/// **single-argument** sites (the plain call's and `new`'s lone-arrow hug).
///
/// Unlike [`build_arrow_hug_arg_docs`] this always pays for both, because here the states are
/// told apart for **every** body kind rather than only at an object/array terminal — a block
/// or call body shatters its parameters exactly as an object one does. The object/array body
/// injection is still armed around the pair, so that shape stays linear; the rest costs one
/// extra traversal of the argument, which is what prettier pays unconditionally
/// (`printedArguments` beside `printedExpanded`).
///
/// `inject` is [`ArrowGapBreak::inject`] — the body the gate already built to answer its own
/// width half, handed in rather than rebuilt here.
fn build_arrow_gap_break_arg_docs(
    printer: &Printer<'_>,
    inject: Option<(u32, DocId)>,
    arg: &ast::Expression<'_>,
    build: impl Fn() -> DocId,
) -> HugArgDocs {
    let Some(inject) = inject else {
        let one = build_printed_argument_doc(printer, arg, &build);
        return HugArgDocs {
            expanded: one,
            printed: one,
        };
    };
    printer.with_arrow_body_inject(Some(inject), || HugArgDocs {
        printed: build_printed_argument_doc(printer, arg, &build),
        expanded: build_expand_last_arg_doc(printer, &build),
    })
}

/// The `expandLastArg` printing the own-line-post-arrow state reads, for the two
/// **multi-argument** sites — whose `printedArguments` docs already exist from
/// `build_args_split_last`, so only this second one is left to build.
///
/// `inject` is [`prebuild_arrow_gap_break_body`]'s answer, armed by the caller around
/// `build_args_split_last` as well so the body is built once and shared. When it is `None` no
/// prebuild covers the shape, and the caller's single printing is returned instead: the ladder
/// degenerates to one doc rather than paying a 2^depth second build.
fn build_arrow_gap_break_expanded(
    printer: &Printer<'_>,
    inject: Option<(u32, DocId)>,
    printed: DocId,
    build: impl FnOnce() -> DocId,
) -> DocId {
    if inject.is_none() {
        return printed;
    }
    printer.with_arrow_body_inject(inject, || build_expand_last_arg_doc(printer, build))
}

/// The whole gap-break layout for a **lone** argument — the entry point all three
/// single-argument printers take (plain call, `new`, member chain).
///
/// Three near-copies of these four lines, one per printer, is the standing
/// hazard this file exists to hold: the three trees answer argument layout separately and
/// drift. They differ only in what a caller genuinely owns — its `opener`, its argument
/// builder, and whether it has a leading run to prepend — so those are the parameters and the
/// rest is stated once.
pub(super) fn build_arrow_gap_break_single_arg_doc(
    printer: &Printer<'_>,
    opener: ArgOpener,
    arg: &ast::Expression<'_>,
    gap_break: &ArrowGapBreak,
    leading: Option<DocId>,
    build: impl Fn() -> DocId,
) -> DocId {
    let d = printer.d();
    let docs = build_arrow_gap_break_arg_docs(printer, gap_break.inject, arg, build)
        .with_leading(d, leading);
    build_arrow_gap_break_ladder(d, opener, &[], docs.expanded, docs.printed)
}

/// The same layout for the **multi-argument** sites, whose `printedArguments` docs already
/// exist from `build_args_split_last` — so the caller passes them in and only the second
/// printing is built here.
///
/// The head-break bail is prettier's `if (headArgs.some(willBreak)) return allArgsBrokenOut()`,
/// and it belongs here rather than at each caller for the same reason the ladder does: it is
/// one rule about this layout. A caller that already asked it (the plain-call/`new` expand-last
/// path asks it for every arm below this one too) simply gets the same answer twice, which
/// `will_break` is a memoized lookup for.
pub(super) fn build_arrow_gap_break_multi_arg_doc(
    printer: &Printer<'_>,
    opener: ArgOpener,
    inject: Option<(u32, DocId)>,
    head_parts: &[DocId],
    printed_last_arg_doc: DocId,
    all_args_broken: DocId,
    build: impl FnOnce() -> DocId,
) -> DocId {
    let d = printer.d();
    if let Some(bail) = opener.expand_all_if_head_breaks(d, head_parts, all_args_broken) {
        return bail;
    }
    // The state reads the `expandLastArg` printing (flat parameters) and the fallback the
    // `printedArguments` one — see [`build_arrow_gap_break_ladder`] for why neither can stand
    // in for the other.
    let expanded = build_arrow_gap_break_expanded(printer, inject, printed_last_arg_doc, build);
    build_arrow_gap_break_ladder(d, opener, head_parts, expanded, all_args_broken)
}

/// The body an own-line-post-arrow argument can have **pre-built and injected**, so its two
/// printings cost signatures rather than the whole subtree — `None` when no prebuild
/// reproduces what the body's own builder would emit, in which case the caller must fall back
/// to ONE printing (the ladder then degenerates, exactly as
/// [`build_arrow_hug_arg_docs`] does for the shapes it cannot inject).
///
/// The route is what makes this wider than its siblings: an own-line comment in the `=>`→body
/// gap sends **every** expression body through `build_arrow_body_doc_with_leading`, which
/// consults the injection above any body-kind branch. So the terminal is claimed by whichever
/// prebuild reproduces that branch — the object/array one adds the grammar's parens, the
/// block one is the block builder itself, and a **leftmost-object** expression (an object
/// behind a cast, a member access, …) is built through the same leftmost-pair seam
/// ([`Printer::build_arrow_arg_body_doc`]) the body's own builder asks first. A **ternary**
/// needs no exclusion on this route
/// even though it takes `if_break` layout parens elsewhere — the own-line run that selects
/// this whole state is a forced break, and `build_arrow_body_doc_with_leading`'s conditional
/// arm answers a breaking body with the NO-parens form, which is exactly the bare doc.
///
/// ⚠️ **Cost, not taste.** Without an injection the second printing rebuilds the argument's
/// whole subtree, so a nest of these shapes goes 2^depth — `fanout:audit`'s
/// `ts_nested_arrow_own_line_comment_block` measured 204,783 doc nodes at depth 12 before the
/// block hook existed.
fn prebuild_arrow_gap_break_body(
    printer: &Printer<'_>,
    last_arg: Option<&ast::Expression<'_>>,
) -> Option<(u32, DocId)> {
    if let Some(obj) = prebuild_expand_last_obj_array_body(printer, last_arg) {
        return Some(obj);
    }
    let ast::ExpressionKind::ArrowFunctionExpression(arrow) = &(last_arg?).kind else {
        return None;
    };
    // The TERMINAL arrow's body, for the same reason every other hug question walks there:
    // under `expandLastArg` a curried argument is a plain run of signatures ending in it.
    match &terminal_arrow(arrow).0.body {
        ast::ArrowFunctionBody::BlockStatement(block) => {
            Some((block.span.start, printer.arrow_block_body_doc(block)))
        }
        ast::ArrowFunctionBody::Expression(body) => Some((
            body.span().start,
            build_arrow_body_like_arrow(printer, body),
        )),
    }
}

/// The lone **container** argument's hug — an object or array literal, including one behind a
/// type cast (`callee({ … })`, `callee([ … ])`, `callee({ … } as T)`). Shared by the plain-call
/// and `new` single-argument printers.
///
/// **The cast is looked through** by the caller's guard
/// ([`super::arg_predicates::is_array_or_object_unwrapped`], prettier's `couldExpandArg`
/// reading past `as` / `satisfies` / `<T>x`) — the `new` twin's hand-rolled arm matched the raw
/// node kind instead, which left `new A({ … } as T)` off the hug entirely, so it broke out
/// where the plain call expanded the object inside the cast.
///
/// The hug itself is prettier's three-state ladder ([`ArgOpener::lone_hug_ladder`]), which is
/// the whole of this function. ⚠️ An `is_truly_empty` special case used to stand here, routing
/// `callee({})` / `callee([])` to a soft-wrapped group so the call had *some* break point (an
/// unconditional hug has none, so an enclosing fluid assignment measured the whole flat width
/// and broke at `=` instead). The ladder answers that structurally and for every argument, not
/// just an empty one: an argument with nothing to break renders its hug state identically to
/// its flat state, so both fail the same fits check and the last state takes it. That also
/// reaches the case the predicate could not see — a NON-empty container whose hug's own first
/// line is already past the width (`calls/chain_single_arg_hug_long`,
/// `new/single_arg_container_long`, and flowbite-svelte's TimelineStepper.svelte in the
/// corpus).
pub(super) fn build_single_container_arg_doc(
    printer: &Printer<'_>,
    callee: DocId,
    arg: &ast::Expression<'_>,
) -> DocId {
    let d = printer.d();
    let arg_doc = printer.build_expression_doc(arg);
    ArgOpener::callee(d, callee).lone_hug_ladder(d, arg_doc, false)
}

/// Prettier's `lastArg` — the hugged argument printed with `expandLastArg: true`, the
/// counterpart of [`build_printed_argument_doc`]'s `printedArguments` entry.
///
/// Two things follow from that flag, and this is the one place both are set:
///
/// - **no chain layout** (`skip_arrow_chain`), because `printArrowFunction` computes
///   `shouldPrintAsChain = !args.expandLastArg && …` — so a curried chain prints as a plain
///   run of `sig =>` with the terminal body hugged, and a long one simply fails on width;
/// - **flat parameters** (`expand_last_arg_flat_params`), prettier's `removeLines` over
///   `printFunctionParameters(…, expandArg)`, so a destructured head cannot shatter inside a
///   hug state and the ladder falls through to the broken-out one instead.
///
/// `build` stays a closure for the same reason [`build_printed_argument_doc`]'s does — the
/// callers disagree on which builder to run (`build_expression_doc` vs
/// `build_arg_expression_doc`).
///
/// ⚠️ **Only build this where a hug state actually renders the argument BROKEN.** It is a
/// second build of the argument, and a second build recurses into any call nested in the body
/// — 2^depth doc nodes (`fanout:audit`) unless the body is injected
/// ([`prebuild_expand_last_obj_array_body`]). That injection covers exactly the object/array
/// terminal, which is exactly where the middle state exists.
pub(super) fn build_expand_last_arg_doc(
    printer: &Printer<'_>,
    build: impl FnOnce() -> DocId,
) -> DocId {
    let prev_skip = printer.skip_arrow_chain.replace(true);
    let prev_flat = printer.expand_last_arg_flat_params.replace(true);
    let doc = build();
    printer.skip_arrow_chain.set(prev_skip);
    printer.expand_last_arg_flat_params.set(prev_flat);
    doc
}

/// The **flat-parameter** half of [`build_expand_last_arg_doc`] alone — for the chain's
/// forced-expansion loop, which renders a lone arrow argument's signature flat (prettier's
/// `expandLastArg` `removeLines`) while leaving its chain layout alone, and for the
/// expand-FIRST hug states ([`build_expand_first_arg_doc`], prettier's `expandFirstArg`),
/// whose broken-out state reads a printing of its own.
///
/// ⚠️ It **restores** the previous value rather than clearing, which is the difference between
/// a scoped flag and a global one: an outer hug can still hold the flag when this layout runs
/// beneath it — on an arrow-body spine, or inside a signature the outer hug is building — and
/// a bare `set(false)` on the way out silently unhugs everything the outer printing had left
/// to do. Same save/restore shape as [`build_expand_last_arg_doc`].
///
/// ⚠️ **Flattening is only safe where the layout cannot fall through.** This loop is the
/// terminal one — its argument already sits on its own broken-out line, so the remaining break
/// belongs after `=>` and never inside the parameter list. A layout with a state ladder must
/// NOT reuse this: a flat signature that overflows has no break candidate left, which is
/// exactly the bug the own-line-post-arrow state carried while it flattened its one printing
/// ([`build_arrow_gap_break_ladder`]).
pub(super) fn build_flat_params_arg_doc(
    printer: &Printer<'_>,
    build: impl FnOnce() -> DocId,
) -> DocId {
    let prev_flat = printer.expand_last_arg_flat_params.replace(true);
    let doc = build();
    printer.expand_last_arg_flat_params.set(prev_flat);
    doc
}

/// Prettier's two printings of one hugged argument, named apart so a state cannot read the
/// wrong one.
///
/// `printCallArguments` builds both — `printedArguments`, which `allArgsBrokenOut()` reads,
/// and a separate `lastArg` printed with `expandLastArg: true`, which the hug states read.
/// They differ for a curried chain (chain layout vs none) and for a breakable parameter list
/// (grouped vs `removeLines`), and agree flat otherwise.
#[derive(Clone, Copy)]
pub(super) struct HugArgDocs {
    /// `lastArg` — [`build_expand_last_arg_doc`]. The hug states.
    pub(crate) expanded: DocId,
    /// The `printedArguments` entry — [`build_printed_argument_doc`]. The broken-out state.
    pub(crate) printed: DocId,
}

impl HugArgDocs {
    /// Prepend a leading-comment run to both printings.
    ///
    /// Keeps them the SAME `DocId` where they already are (every one-doc shape), so the
    /// one-doc property survives the prepend instead of becoming two equal-but-distinct
    /// nodes.
    pub(super) fn with_leading(self, d: &DocArena, leading: Option<DocId>) -> Self {
        let Some(leading) = leading else {
            return self;
        };
        let printed = prepend_leading(d, Some(leading), self.printed);
        let expanded = if self.expanded == self.printed {
            printed
        } else {
            prepend_leading(d, Some(leading), self.expanded)
        };
        Self { expanded, printed }
    }
}

/// Build the argument docs a lone huggable arrow's hug states read, paying for the second
/// printing only where a state can tell the two apart.
///
/// - **Object/array terminal** (at any curried depth) — the ladder has a middle state that
///   renders the argument BROKEN, where the chain layout would break the chain's own heads
///   (see [`arrow_body_expands_internally`]), so both printings are built. The pair stays
///   linear because the terminal body is [pre-built and
///   injected](prebuild_expand_last_obj_array_body) around both builds — which is why that
///   prebuild answers this exact shape unconditionally.
/// - **Break-forcing chain** ([`arrow_chain_should_break`]: a return type with params, type
///   params, or a non-identifier param anywhere) with a **block** terminal — one doc, the
///   chain-free one, in every state: the chain layout is built OUTSIDE
///   [`build_printed_argument_doc`] (no `ArrowChainContext` in scope, so it declines on the
///   context) with `skip_arrow_chain` set for its *other* job, suppressing the nested-arrow
///   break so the body still hugs `=>`. `calls/curried_arrow_chain` pins it. No body injection
///   reaches a block terminal, so a second build here would be 2^depth
///   (`fanout:audit`'s `ts_nested_curried_arrow_typed`).
/// - **Everything else** — one doc, the progressive layout. Its FLAT rendering is
///   byte-identical to the hugged one, so the two remaining states select identically.
///
/// `build` is the caller's argument builder (`build_expression_doc` vs
/// `build_arg_expression_doc`), the one thing the plain-call, `new` and member-chain sites
/// disagree on — everything above is shared so the three cannot drift on which printing a
/// state reads.
fn build_arrow_hug_arg_docs(
    printer: &Printer<'_>,
    arg: &ast::Expression<'_>,
    arrow: &ast::ArrowFunctionExpression<'_>,
    build: impl Fn() -> DocId,
) -> HugArgDocs {
    let inject = prebuild_expand_last_obj_array_body(printer, Some(arg));
    if inject.is_none() {
        let one = build_arrow_hug_printed_doc(printer, arg, arrow, build);
        return HugArgDocs {
            expanded: one,
            printed: one,
        };
    }
    printer.with_arrow_body_inject(inject, || {
        let printed = build_printed_argument_doc(printer, arg, &build);
        let expanded = build_expand_last_arg_doc(printer, &build);
        HugArgDocs { expanded, printed }
    })
}

/// The `printedArguments` half of [`build_arrow_hug_arg_docs`] alone — what a state that
/// renders the argument BROKEN OUT reads, and the only printing the comment-refusal states
/// read at all.
///
/// Split out so a refusal pays ONE build: the `expandLastArg` printing beside it is a second
/// build of the argument, which recurses into any call nested in its body. A caller whose
/// early-return arms read only this must therefore ask its comment questions **before**
/// reaching for the pair — the constant-factor shape `docs/audits.md` §Build-Fanout Audit
/// says to find syntactically, since no depth curve can separate it from the baseline.
pub(super) fn build_arrow_hug_printed_doc(
    printer: &Printer<'_>,
    arg: &ast::Expression<'_>,
    arrow: &ast::ArrowFunctionExpression<'_>,
    build: impl FnOnce() -> DocId,
) -> DocId {
    // The break-forcing-chain arm is the pair's `None`-injection arm, keyed on the same
    // question the prebuild answers ([`arrow_body_expands_internally`]) so the two agree on
    // which shape has no second printing to tell apart.
    if !arrow_body_expands_internally(arrow)
        && is_curried_arrow_chain(arg)
        && arrow_chain_should_break(arrow)
    {
        let prev = printer.skip_arrow_chain.replace(true);
        let doc = build();
        printer.skip_arrow_chain.set(prev);
        return doc;
    }
    build_printed_argument_doc(printer, arg, build)
}

/// The whole hug layout for a **lone** huggable arrow argument (`callee((x) => …)`) — the
/// entry point all three single-argument printers take (plain call, `new`, member chain).
///
/// Three near-copies of these two steps, one per printer, is the standing
/// hazard this file exists to hold: the three trees answer argument layout separately and
/// drift. They differ only in what a caller genuinely owns — its `opener`, its argument
/// builder, and whether it has a leading run to prepend — so those are the parameters and the
/// two printings, the ladder and its body-kind key are stated once.
///
/// ⚠️ Every comment refusal a caller makes must be asked **before** this, and must read only
/// [`build_arrow_hug_printed_doc`]: the pair below is a second build of the argument, which
/// recurses into any call nested in its body.
pub(super) fn build_single_arrow_hug_doc(
    printer: &Printer<'_>,
    opener: ArgOpener,
    arg: &ast::Expression<'_>,
    arrow: &ast::ArrowFunctionExpression<'_>,
    leading: Option<DocId>,
    build: impl Fn() -> DocId,
) -> DocId {
    let d = printer.d();
    let expands_internally = arrow_body_expands_internally(arrow);
    let assumed = if expands_internally {
        None
    } else {
        AssumedParamHug::new(printer, arg)
    };
    let docs = AssumedParamHug::sharing(assumed.as_ref(), printer, || {
        build_arrow_hug_arg_docs(printer, arg, arrow, &build)
    })
    .with_leading(d, leading);
    let assumed = assumed.and_then(|hug| {
        hug.build(printer, false, || {
            build_arrow_hug_printed_doc(printer, arg, arrow, &build)
        })
        .literal
        .map(|assumed| assumed.map(|doc| prepend_leading(d, leading, doc)))
    });
    build_single_arrow_arg_states(d, opener, docs, expands_internally, assumed)
}

/// The width-broken sole-parameter literal's hug — a convergence state for a block-bodied
/// callback argument whose hugged sole parameter's object type the author wrote flat
/// ([`flat_sole_param_literal`]).
///
/// Prettier's hug prints that parameter list flat, so when the flat literal does not fit it
/// breaks every argument out and the literal breaks for width on the argument's own line.
/// Read back, that break is an authored expansion, which keeps the hug a block body takes
/// (`docs/conformance_prettier_ts.md` §Hugged sole-parameter object type) — so prettier's
/// form is not a tsv fixed point. The hug ladders admit the hug with the literal assumed
/// expanded instead ([`AssumedBreak::TypeLiteral`]), gated on the broken-out argument line:
/// [`AssumedHugDoc::state`]'s probe is the broken-out printing of the argument, whose flat
/// measurement ends at the block body's forced break, so it measures exactly the line the
/// literal would break for width on.
///
/// A `function` callback whose parameters group on their own ends that line sooner, at its
/// object return type's `{`, where its hug flattens its parameters — a plain call's last of
/// several arguments, prettier's `shouldExpandParameters` (`printFunction` reads
/// `isCallExpression(parent)`, which `new` is not). There the hug never breaks the literal,
/// so it breaks only on the broken-out line, and prettier's params group measures the flat
/// parameters only up to that `{`: the probe is the argument printed once more with the `{`
/// assumed broken
/// ([`FlatSoleParamLiteral::return_break`](crate::printer::expressions::functions::FlatSoleParamLiteral::return_break)).
/// Everywhere else the hug breaks the literal itself, and the body's break ends the line.
///
/// The same family holds a second state, for a `function` callback with ONE parameter of any
/// type whose hug flattens its parameters: the hug with its return type's first `{` assumed
/// broken ([`function_return_hug_break`]). Prettier measures that hug only up to the `{`, so
/// it hugs where the inline state, measured to the body's `{`, does not fit. That state needs
/// no gate — it is the ordinary hug fit, which stops at the assumed break — and sits ahead
/// of the literal's state. With a flat sole-parameter literal too, its printing IS that
/// state's probe (the same build under the same break), so the one printing serves both
/// ([`ShareTag::AssumedHugProbe`]).
///
/// The callback's body is built ONCE and shared by every printing of it (the body inject),
/// so each further printing the state needs costs a signature and not the subtree — nested
/// callbacks of this shape stay linear.
pub(in crate::printer) struct AssumedParamHug<'a> {
    arg: &'a ast::Expression<'a>,
    /// The flat sole-parameter literal's position, when the callback has one
    /// ([`flat_sole_param_literal`]).
    literal: Option<u32>,
    /// The literal state's return-type probe break
    /// ([`FlatSoleParamLiteral::return_break`](crate::printer::expressions::functions::FlatSoleParamLiteral::return_break)).
    return_break: Option<AssumedBreak>,
    /// The return-type hug's break ([`function_return_hug_break`]).
    return_hug: Option<AssumedBreak>,
    body: (u32, DocId),
}

/// Which of [`AssumedParamHug`]'s states a member chain asks a call to print outright
/// (`Printer::force_assumed_param_hug`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::printer) enum AssumedHugKind {
    /// The hug with the sole parameter's literal assumed expanded.
    ParamLiteral,
    /// The hug with the return type's first `{` assumed broken.
    ReturnType,
}

/// [`AssumedParamHug::build`]'s states for one site: the literal's gated state and the
/// return-type hug, each `None` where the callback or the site has none.
#[derive(Clone, Copy, Default)]
pub(super) struct AssumedHugStates {
    /// The literal's state ([`AssumedHugDoc`]).
    pub(super) literal: Option<AssumedHugDoc>,
    /// The callback printed with its return type's first `{` assumed broken — the
    /// return-type hug's contents.
    pub(super) return_hug: Option<DocId>,
}

/// [`AssumedParamHug::build`]'s printings: the state's contents, and the line its gate
/// measures when that is not the site's own broken-out line.
#[derive(Clone, Copy)]
pub(super) struct AssumedHugDoc {
    /// The callback printed with its sole parameter's literal assumed expanded.
    pub(super) doc: DocId,
    /// The callback printed with its return type's `{` assumed broken, standing for the
    /// broken-out line — `None` when the site's own line stands for it.
    probe: Option<DocId>,
}

impl AssumedHugDoc {
    /// The same printings with `doc` replaced — a site's own wrapping of the contents.
    #[inline]
    pub(super) fn map(self, f: impl FnOnce(DocId) -> DocId) -> Self {
        Self {
            doc: f(self.doc),
            probe: self.probe,
        }
    }

    /// The state: `contents` (the hug rendering [`Self::doc`]), admitted only while `line`
    /// — the argument's own line in the broken-out state, the argument as that state prints
    /// it plus any separator the line ends with — cannot fit there, or the return-type probe
    /// in its place when there is one. The line is measured as a line
    /// ([`DocArena::line_probe`]): an empty body puts nothing after it that would end the
    /// measurement, and the rest commands that complete the gate's probe (`)`, `;`) sit on
    /// later lines of that state.
    #[inline]
    pub(super) fn state(self, d: &DocArena, line: DocId, contents: DocId) -> DocId {
        d.gated_state(d.line_probe(0, self.probe.unwrap_or(line)), contents)
    }
}

impl<'a> AssumedParamHug<'a> {
    /// The state's ingredients for `arg`, with its body built — `None` unless `arg` is the
    /// shape [`flat_sole_param_literal`] names.
    ///
    /// Under a member chain the body is shared across the chain's candidates like any
    /// argument ([`ShareTag::AssumedHugBody`]): each candidate rebuilds the call's arguments,
    /// and a body built per candidate would compound with the nesting.
    pub(in crate::printer) fn new(
        printer: &Printer<'_>,
        arg: &'a ast::Expression<'a>,
    ) -> Option<Self> {
        let flat = flat_sole_param_literal(printer, arg);
        let return_hug = function_return_hug_break(printer, arg);
        let body = match (&flat, &arg.kind) {
            (Some(flat), _) => flat.body,
            (None, ast::ExpressionKind::FunctionExpression(func)) if return_hug.is_some() => {
                &func.body
            }
            _ => return None,
        };
        let is_arrow = flat.as_ref().is_some_and(|flat| flat.is_arrow);
        let body_doc = shared_callback_body_doc(printer, arg, body, is_arrow);
        Some(Self {
            arg,
            literal: flat.as_ref().map(|flat| flat.literal),
            return_break: flat.and_then(|flat| flat.return_break),
            return_hug,
            body: (body.span.start, body_doc),
        })
    }

    /// The body injection every printing of the callback is built under.
    #[inline]
    pub(super) fn body_inject(&self) -> (u32, DocId) {
        self.body
    }

    /// Run `build` — a printing of the callback, or of the arguments holding it — with the
    /// shared body armed, or plainly when there is no state.
    #[inline]
    pub(super) fn sharing<R>(
        hug: Option<&Self>,
        printer: &Printer<'_>,
        build: impl FnOnce() -> R,
    ) -> R {
        printer.with_arrow_body_inject(hug.map(Self::body_inject), build)
    }

    /// The callback printed with the literal assumed expanded — what the state renders.
    ///
    /// Shared across a member chain's candidates ([`ShareTag::AssumedHugArg`]) for the
    /// reason [`Self::new`] shares the body. The key is taken before the assumption is set:
    /// under it, the argument's own key would refuse to share at all. The key names the
    /// argument alone, not the hug site whose `build` fills it, and that is sound because
    /// one argument reaches exactly one hug site per chain session: the hug family (lone,
    /// expand-last, expand-first) is decided per call, not per candidate.
    ///
    /// `flattens_params` says the site's hug flattens the callback's parameters (prettier's
    /// `shouldExpandParameters`): a `function` whose return type then ends the broken-out
    /// line is printed a third time for the gate ([`AssumedHugDoc::probe`]), with the same
    /// `build` and sharing — and only there does the return-type hug exist. Where both do,
    /// their breaks are the one return type's `{` (both read the object or mapped return
    /// type; only the hug also reads a `Promise<{` or a hugged union, which never groups the
    /// parameters), so one printing is both.
    pub(super) fn build(
        &self,
        printer: &Printer<'_>,
        flattens_params: bool,
        build: impl Fn() -> DocId,
    ) -> AssumedHugStates {
        let probe_break = self.return_probe(flattens_params);
        let hug_break = self.return_hug.filter(|_| flattens_params);
        debug_assert!(
            probe_break.is_none() || hug_break.is_none() || probe_break == hug_break,
            "the literal's probe and the return-type hug read one return type's `{{`"
        );
        let return_printed = probe_break
            .or(hug_break)
            .map(|assumed| self.print(printer, ShareTag::AssumedHugProbe, assumed, &build));
        AssumedHugStates {
            literal: self.literal.map(|literal| AssumedHugDoc {
                doc: self.print(
                    printer,
                    ShareTag::AssumedHugArg,
                    AssumedBreak::TypeLiteral(literal),
                    &build,
                ),
                probe: probe_break.and(return_printed),
            }),
            return_hug: hug_break.and(return_printed),
        }
    }

    /// The argument's broken-out printing by `build` as the state's gate measures it: the
    /// return-type probe [`Self::build`] takes, else `build`'s own printing — for a site that
    /// gates the state itself (the member chain's spelling of it).
    ///
    /// Not shared: the probe [`Self::build`] shares is keyed by the argument alone, which is
    /// sound only for the one hug site that fills it, and this printing is another site's.
    pub(in crate::printer) fn gate_line(
        &self,
        printer: &Printer<'_>,
        flattens_params: bool,
        build: impl Fn() -> DocId,
    ) -> DocId {
        match self.return_probe(flattens_params) {
            Some(assumed) => printer.with_assumed_break(assumed, || {
                printer.with_arrow_body_inject(Some(self.body), build)
            }),
            None => build(),
        }
    }

    /// The break that ends the broken-out line before the body does, at a site whose hug
    /// flattens the parameters or not.
    #[inline]
    fn return_probe(&self, flattens_params: bool) -> Option<AssumedBreak> {
        self.return_break.filter(|_| flattens_params)
    }

    /// One printing of the callback by `build`, with `assumed` read as authored and the body
    /// shared — shared itself across a member chain's candidates under `tag`.
    fn print(
        &self,
        printer: &Printer<'_>,
        tag: ShareTag,
        assumed: AssumedBreak,
        build: impl Fn() -> DocId,
    ) -> DocId {
        let key = printer.chain_share_key(self.arg, tag);
        printer.chain_shared_doc(key, || {
            printer.with_assumed_break(assumed, || {
                printer.with_arrow_body_inject(Some(self.body), build)
            })
        })
    }

    /// The contents of the state of `kind` a member chain asked this callback for outright
    /// (`Printer::force_assumed_param_hug`), printed by `build` with no gate to measure —
    /// consuming the request, which is how the chain learns the state exists. The site then
    /// returns those contents alone. `None` when no such request names this callback, or
    /// the callback has no state of that kind (the request stands, and the chain drops it).
    ///
    /// The return-type hug is asked for only at a site whose hug flattens the parameters
    /// (the chain asks for it on the last of several arguments).
    pub(super) fn take_forced(
        &self,
        printer: &Printer<'_>,
        kind: AssumedHugKind,
        build: impl Fn() -> DocId,
    ) -> Option<DocId> {
        if printer.force_assumed_param_hug.get() != Some((self.arg.span.start, kind)) {
            return None;
        }
        let (tag, assumed) = match kind {
            AssumedHugKind::ParamLiteral => (
                ShareTag::AssumedHugArg,
                AssumedBreak::TypeLiteral(self.literal?),
            ),
            AssumedHugKind::ReturnType => (ShareTag::AssumedHugProbe, self.return_hug?),
        };
        printer.force_assumed_param_hug.set(None);
        Some(self.print(printer, tag, assumed, build))
    }
}

/// The state ladder [`build_single_arrow_hug_doc`] selects among.
///
/// Two shapes, keyed on whether the arrow's body **expands internally** — an object or array
/// literal terminal, whose own doc can break while the arrow stays hugged to `callee(`:
///
/// - such a terminal gets three states — hug flat, hug with the body broken
///   (`group_break(expanded)`, prettier's `group(lastArg, { shouldBreak: true })`), then the
///   argument on its own line;
/// - every other huggable body (a block, a block-terminal arrow chain) has only the outer two,
///   since there is nothing between "hugged" and "broken out".
///
/// ⚠️ **The middle state is what a hand-rolled two-state copy loses, and its absence cannot be
/// seen from a formatted source.** A source-multiline object carries its own `group_break`, so
/// `state_hug` — measured only to that first forced break — already renders as the hug; the
/// ladder is reached as a *width* question only when the body is written FLAT. So the `new`
/// twin printed `new A(⏎\t(x) => ({ … })⏎)` where prettier hugs, and every fixed-point gate
/// agreed with it. Pinning that needs an `unformatted_*` variant
/// (`expressions/new/single_arg_arrow_body_long`), not an input-level case.
///
/// `assumed` is the [`AssumedParamHug`] printing of the argument, which gates a hug state in
/// between for a block body.
fn build_single_arrow_arg_states(
    d: &DocArena,
    opener: ArgOpener,
    docs: HugArgDocs,
    body_expands_internally: bool,
    assumed: Option<AssumedHugDoc>,
) -> DocId {
    if !body_expands_internally {
        let inline = opener.inline(d, &[], docs.expanded);
        let break_out = opener.break_out_state(d, docs.printed);
        return match assumed {
            Some(assumed) => opener.ladder(
                d,
                &[
                    inline,
                    assumed.state(d, docs.printed, opener.inline(d, &[], assumed.doc)),
                    break_out,
                ],
            ),
            None => opener.ladder(d, &[inline, break_out]),
        };
    }
    opener.inline_hug_or_expand_all(d, &[], docs.expanded, docs.printed)
}

/// What an argument layout opens with, and the callee it follows.
///
/// The printers disagree on exactly one thing here: whether the argument doc carries the
/// callee. The plain-call and `new` printers return a doc that does ([`Self::callee`]); the
/// member chain's callee is a separate group the chain printer places, so its argument doc
/// starts at the paren and the `?.(` optional spelling rides in the opening text
/// ([`Self::chain`]). Every ladder below takes one of these rather than being written twice.
///
/// ⚠️ **The callee is outside every state.** Prettier prints a call as
/// `[callee, typeArgs, printCallArguments()]`, and the `conditionalGroup` its argument
/// ladders build covers the arguments alone — so a state is measured from the `(`, wherever
/// the callee left the line. Each layout here builds its states from [`Self::open`] and
/// places the callee once, in front of the whole layout ([`Self::after_callee`]). A callee
/// written into each state instead was measured as part of it: the fits walk then stopped
/// at the callee's first break — a width-broken type-argument list
/// (`calls/type_args_broken_last_arg_hug_long`), a multiline tagged template
/// (`calls/tagged_template_callee_multiline_last_arg_long`) — and judged the state on a line
/// the arguments never print on.
#[derive(Clone, Copy)]
pub(super) struct ArgOpener {
    /// The whole head the `(` follows — for `new` that includes the keyword and type
    /// arguments. `None` for the member chain, and for a layout built to sit inside another
    /// layout's states ([`Self::args_only`]).
    callee: Option<DocId>,
    /// The opening delimiter: `(`, or the chain's own `(` / `?.(` as the doc the chain built.
    open: DocId,
}

impl ArgOpener {
    /// The plain-call and `new` spelling: `callee(`.
    #[inline]
    pub(super) fn callee(d: &DocArena, callee: DocId) -> Self {
        Self {
            callee: Some(callee),
            open: d.text("("),
        }
    }

    /// The member chain's spelling: its `(` / `?.(`, with no callee in the doc.
    #[inline]
    pub(super) fn chain(prefix: DocId) -> Self {
        Self {
            callee: None,
            open: prefix,
        }
    }

    /// The same opener without its callee — for an argument layout one of the ladders below
    /// takes as a STATE, which must not carry the callee the ladder places outside it.
    #[inline]
    pub(super) fn args_only(self) -> Self {
        Self {
            callee: None,
            open: self.open,
        }
    }

    /// Place `layout` after the callee this opener carries, if any. The callee stays
    /// **outside** whatever `layout` is — its own groups measure themselves, a hardline in
    /// the callee must not force the argument list open, and a state of the layout is
    /// measured from the `(` (see the type's doc).
    #[inline]
    pub(super) fn after_callee(self, d: &DocArena, layout: DocId) -> DocId {
        match self.callee {
            Some(callee) => d.concat(&[callee, layout]),
            None => layout,
        }
    }

    /// A ladder: the states, measured from the `(`, behind the callee.
    #[inline]
    fn ladder(self, d: &DocArena, states: &[DocId]) -> DocId {
        self.after_callee(d, d.conditional_group(states))
    }

    /// `(before after)` — a whole argument list on one line, as two already-built halves.
    ///
    /// The one flat-run spelling, which both ladders' first states are: the expand-LAST
    /// family passes its head arguments and the last one ([`Self::inline`]), the
    /// expand-FIRST family the first argument and its tail
    /// ([`Self::expand_first_state`]) — the same run, pivoting at opposite ends. Built for
    /// every call-argument list in the document, so the run stays flat and allocates no
    /// buffer.
    #[inline]
    fn flat(self, d: &DocArena, before: DocId, after: DocId) -> DocId {
        d.concat(&[self.open, before, after, d.text(")")])
    }

    /// `(head_parts last_arg)` — the expand-LAST families' flat state.
    #[inline]
    pub(super) fn inline(self, d: &DocArena, head_parts: &[DocId], last_arg_doc: DocId) -> DocId {
        self.flat(d, d.concat(head_parts), last_arg_doc)
    }

    /// The hug state: head arguments inline, the last one broken open
    /// (prettier's `group(lastArg, { shouldBreak: true })`).
    #[inline]
    fn hug(self, d: &DocArena, head_parts: &[DocId], last_arg_doc: DocId) -> DocId {
        self.inline(d, head_parts, d.group_break(last_arg_doc))
    }

    /// The LONE argument on its own line, as a state ([`Self::break_out`]).
    #[inline]
    fn break_out_state(self, d: &DocArena, printed: DocId) -> DocId {
        d.concat(&[
            self.open,
            d.indent(d.concat(&[d.softline(), printed])),
            d.softline(),
            d.text(")"),
        ])
    }

    /// The LONE argument on its own line, with SOFT lines rather than [`Self::expand_all`]'s
    /// forced ones: a one-argument list has no middle state to fall through to, so this state
    /// must still be able to collapse and re-measures instead of rendering broken.
    ///
    /// Also the whole layout wherever the hug is refused outright (a comment forcing the
    /// callback's parameter list multiline).
    #[inline]
    pub(super) fn break_out(self, d: &DocArena, printed: DocId) -> DocId {
        self.after_callee(d, self.break_out_state(d, printed))
    }

    /// The ladder for a LONE huggable argument, in prettier's own shape: flat, hugged with the
    /// argument's own group broken (`group(lastArg, { shouldBreak: true })`), then
    /// `allArgsBrokenOut()`.
    ///
    /// `params_render_flat` folds in the one case with no middle state — prettier renders
    /// that parameter list flat in both hug states, so the hug is bit-for-bit its own flat
    /// state ([`super::arg_predicates::lone_arg_params_render_flat`]). Answered by the caller
    /// because prettier's gate is `isCallExpression(parent)`: the plain-call and member-chain
    /// spellings ask the predicate, the `new` twin passes `false`.
    ///
    /// ⚠️ **One seam for three printers.** Each of them hand-rolled a bare `callee(arg)` concat
    /// here, and a concat has no state below the hug: an argument with nothing to break inside
    /// it — `{}`, `[]`, `function () {}` — ran past the print width instead of dropping to its
    /// own line (`calls/single_arg_function_expression_long`, `calls/chain_single_arg_hug_long`).
    /// One `printCallArguments` prints all three, so none of them may answer this alone.
    #[inline]
    pub(super) fn lone_hug_ladder(
        self,
        d: &DocArena,
        arg_doc: DocId,
        params_render_flat: bool,
    ) -> DocId {
        if params_render_flat {
            self.inline_or_expand_all(d, &[], arg_doc, arg_doc)
        } else {
            self.inline_hug_or_expand_all(d, &[], arg_doc, arg_doc)
        }
    }

    /// Prettier's `allArgsBrokenOut()` as a state.
    ///
    /// Wrapped in `group_break` to force break mode, matching prettier's
    /// `group(contents, { shouldBreak: true })`. Without the group the `line`s would inherit
    /// the enclosing mode and render as spaces wherever that mode is Flat — which a short
    /// chain measured inside assignment layout's `fits()` really is.
    #[inline]
    fn expand_all_state(self, d: &DocArena, all_args_broken: DocId) -> DocId {
        d.group_break(d.concat(&[
            self.open,
            d.indent(d.concat(&[d.line(), all_args_broken])),
            d.line(),
            d.text(")"),
        ]))
    }

    /// Prettier's `allArgsBrokenOut()` — every argument on its own line — as the whole layout.
    #[inline]
    pub(super) fn expand_all(self, d: &DocArena, all_args_broken: DocId) -> DocId {
        self.after_callee(d, self.expand_all_state(d, all_args_broken))
    }

    /// Prettier's `if (headArgs.some(willBreak)) return allArgsBrokenOut()`, the one guard
    /// every expand-last ladder opens with — asked once here rather than re-spelled per
    /// arm, which is how the three trees drifted on which arms ask it at all.
    ///
    /// `Some` is the bail; `None` means the caller's ladder still applies. A caller that has
    /// already answered it for an enclosing arm simply gets the same answer again, which
    /// `will_break` is a memoized lookup for.
    #[inline]
    pub(super) fn expand_all_if_head_breaks(
        self,
        d: &DocArena,
        head_parts: &[DocId],
        all_args_broken: DocId,
    ) -> Option<DocId> {
        head_parts
            .iter()
            .any(|&id| d.will_break(id))
            .then(|| self.expand_all(d, all_args_broken))
    }

    /// Soft-break wrapping: `callee(args)` collapses to one line when it fits and breaks its
    /// own parens when it does not. The default argument-list layout in every printer.
    #[inline]
    pub(super) fn wrap_soft(self, d: &DocArena, args: DocId) -> DocId {
        self.after_callee(d, d.group(self.soft_list_contents(d, args)))
    }

    /// [`Self::wrap_soft`] without its group — prettier's default tail for a long-curried
    /// call (`isLongCurriedCallExpression`), whose list then breaks with the nearest group
    /// around it, ahead of the outer call's own list.
    #[inline]
    pub(super) fn soft_list(self, d: &DocArena, args: DocId) -> DocId {
        self.after_callee(d, self.soft_list_contents(d, args))
    }

    /// `(`, the arguments behind a soft break, a soft break, `)` — the default tail's contents.
    #[inline]
    fn soft_list_contents(self, d: &DocArena, args: DocId) -> DocId {
        d.concat(&[
            self.open,
            d.indent_softline(args),
            d.softline(),
            d.text(")"),
        ])
    }

    /// A single huggable argument: hugs the opening delimiter and drops the `)` to its own
    /// line when the content breaks internally.
    ///
    /// For expressions with natural break points (objects, arrays, ternaries). Under tsv's
    /// hardcoded `trailingComma: 'none'` no trailing comma is added.
    #[inline]
    pub(super) fn hug_arg(self, d: &DocArena, arg: DocId) -> DocId {
        self.after_callee(
            d,
            d.group(d.concat(&[self.open, arg, d.softline(), d.text(")")])),
        )
    }

    /// The two-state ladder: inline → expand all.
    ///
    /// Prettier's expand-last shape for a last argument whose own doc expands (a block-bodied
    /// callback, a same-type array/object pair): there is nothing between "everything on one
    /// line" and "every argument on its own", so no hug state sits between them.
    #[inline]
    pub(super) fn inline_or_expand_all(
        self,
        d: &DocArena,
        head_parts: &[DocId],
        last_arg_doc: DocId,
        all_args_broken: DocId,
    ) -> DocId {
        self.ladder(
            d,
            &[
                self.inline(d, head_parts, last_arg_doc),
                self.expand_all_state(d, all_args_broken),
            ],
        )
    }

    /// [`Self::inline_or_expand_all`] for a block-bodied callback last argument, with the
    /// [`AssumedParamHug`] states between its two, each where `assumed` has it: the
    /// return-type hug, an ungated inline state over the callback printed with its return
    /// type's first `{` assumed broken, so it is measured to that `{`; then the literal's
    /// state, over the callback printed with its literal assumed expanded, gated on
    /// `last_arg_doc`, the argument as the broken-out state prints it.
    ///
    /// The return-type hug comes first: where the broken-out line cannot hold the literal
    /// flat either, both are admitted, and prettier keeps the literal flat and breaks the
    /// return type (`calls/function_arg_hugged_return_type_long`).
    #[inline]
    pub(super) fn inline_assumed_or_expand_all(
        self,
        d: &DocArena,
        head_parts: &[DocId],
        last_arg_doc: DocId,
        assumed: AssumedHugStates,
        all_args_broken: DocId,
    ) -> DocId {
        let mut states: SmallVec<[DocId; 4]> = smallvec![self.inline(d, head_parts, last_arg_doc)];
        if let Some(return_hug) = assumed.return_hug {
            states.push(self.inline(d, head_parts, return_hug));
        }
        if let Some(literal) = assumed.literal {
            states.push(literal.state(d, last_arg_doc, self.inline(d, head_parts, literal.doc)));
        }
        states.push(self.expand_all_state(d, all_args_broken));
        self.ladder(d, &states)
    }

    /// The three-state ladder for a **different-type** expand-last argument:
    /// inline → hug → expand all.
    ///
    /// - State 0: everything inline — `fn('x', [a, b])`
    /// - State 1: hug — head args inline, the last one expands internally
    ///   (`fn('x', [⏎\ta,⏎\tb⏎])`)
    /// - State 2: every argument on its own line
    ///
    /// The hug wraps `last_arg_doc` in `group_break` (prettier's
    /// `group(lastArg, { shouldBreak: true })`), which is what lets `fits()` answer on the
    /// last argument's *first* line and so select the hug whenever the head plus the opening
    /// bracket fit.
    ///
    /// A last argument that WILL break — a block body with a statement in it, a hardline
    /// from an interior comment, a source-multiline `group_break` — gets prettier's other
    /// ladder, `[breakParent, conditionalGroup([hug, allArgsBrokenOut()])]`: the hug is state
    /// 0 and there is no inline state. Rendered directly the two ladders agree, because such
    /// an argument falls out of the inline state onto the hug anyway. They part where the
    /// call is MEASURED from outside: a member chain's `oneLine` walks a nested conditional
    /// group's first state only, so with the inline state first it read the whole hugged
    /// signature flat and every chain state failed — where prettier's walk enters the hug's
    /// broken group, stops at the parameter list's own softline, and keeps the chain flat
    /// with the parameters breaking inside the hug
    /// (`calls/chained/function_arg_params_break_long`). The *same*-type path, which has no
    /// hug state, screens for the forced break at its own call site.
    #[inline]
    pub(super) fn inline_hug_or_expand_all(
        self,
        d: &DocArena,
        head_parts: &[DocId],
        last_arg_doc: DocId,
        all_args_broken: DocId,
    ) -> DocId {
        if d.will_break(last_arg_doc) {
            return self.ladder(
                d,
                &[
                    self.hug(d, head_parts, last_arg_doc),
                    self.expand_all_state(d, all_args_broken),
                ],
            );
        }
        self.ladder(
            d,
            &[
                self.inline(d, head_parts, last_arg_doc),
                self.hug(d, head_parts, last_arg_doc),
                self.expand_all_state(d, all_args_broken),
            ],
        )
    }

    /// One expand-FIRST state: `(first_arg tail)` — [`Self::flat`] pivoted at the other end,
    /// with the argument that may open in the FIRST position and the rest of the list riding
    /// flat behind it.
    #[inline]
    fn expand_first_state(self, d: &DocArena, first_arg_doc: DocId, tail: DocId) -> DocId {
        self.flat(d, first_arg_doc, tail)
    }

    /// The expand-FIRST ladder — prettier's `shouldExpandFirstArg` conditional group:
    ///
    /// - State 0: everything inline — `fn(() => {}, 100)`
    /// - State 1: the first argument breaks open and the tail still rides past its close
    ///   (prettier's `group(firstArg, { shouldBreak: true })`) — `fn(function (⏎\ta,⏎\tb⏎) {}, e)`
    /// - State 2: every argument on its own line
    ///
    /// All three are needed. State 1 is not reachable by collapsing the ladder to two:
    /// prettier breaks the first argument's own parameter list before it breaks the
    /// argument list, and a two-state ladder would take the last state there instead.
    /// State 2 is not optional either — without it a first argument with nothing of its
    /// own to break (`() => {}`) holds an over-width line forever, which is the shape this
    /// ladder replaced. `all_args_broken` is that state whole, built through
    /// [`Self::args_only`] so it carries no callee of its own.
    ///
    /// A first argument that already carries a forced break (the usual block body) simply
    /// falls out of state 0 onto state 1, so no pre-check screens for one, and prettier's
    /// own `willBreak(firstArg)` two-state branch reduces to exactly that. The expand-LAST
    /// twin does screen ([`Self::inline_hug_or_expand_all`]) — not because the render
    /// differs, but because a chain measuring the call from outside walks only a nested
    /// ladder's FIRST state, and there the two orderings part. Nothing measures an
    /// expand-first call that way.
    ///
    /// `assumed` is the [`AssumedParamHug`] printing of the first argument, which gates one
    /// more hug state ahead of the last, on `first_arg_doc` and its comma standing for the
    /// broken-out line.
    #[inline]
    pub(super) fn inline_hug_first_or_expand_all(
        self,
        d: &DocArena,
        first_arg_doc: DocId,
        tail: DocId,
        assumed: Option<AssumedHugDoc>,
        all_args_broken: DocId,
    ) -> DocId {
        let flat = self.expand_first_state(d, first_arg_doc, tail);
        let hug = self.expand_first_state(d, d.group_break(first_arg_doc), tail);
        match assumed {
            Some(assumed) => {
                let assumed_hug = self.expand_first_state(d, d.group_break(assumed.doc), tail);
                let first_line = d.concat(&[first_arg_doc, d.text(",")]);
                self.ladder(
                    d,
                    &[
                        flat,
                        hug,
                        assumed.state(d, first_line, assumed_hug),
                        all_args_broken,
                    ],
                )
            }
            None => self.ladder(d, &[flat, hug, all_args_broken]),
        }
    }

    /// The always-multiline argument layout: `callee(⏎\targs⏎)`, with the
    /// `(`→first-argument gap's same-line comment run injected on the `(` line when there
    /// is one (`fn( // c⏎\targs⏎)`).
    ///
    /// That run is [`emit_first_arg_leading_comments`]'s `paren_line` output, and the
    /// reason it obliges a hard break: it ends in a `//` whose `line_suffix` needs a
    /// following break to flush against, and any argument printed onto that line would be
    /// swallowed by the comment.
    ///
    /// The callee stays outside, so a hardline inside it (a multiline array, say) cannot
    /// force the arguments open — the args make their own flat/break decision. The soft
    /// counterpart is [`Self::wrap_soft`]. No trailing comma is emitted
    /// (`trailingComma: 'none'`).
    #[inline]
    fn wrap_hard_broken(self, d: &DocArena, paren_line: &[DocId], args: DocId) -> DocId {
        // One flat concat rather than a nest of them: the run is short and fixed, and this
        // layout is reached once per forced-expansion construct.
        let mut parts: DocBuf = DocBuf::new();
        parts.push(self.open);
        parts.extend(paren_line.iter().copied());
        parts.push(d.indent_hardline(args));
        parts.push(d.hardline());
        parts.push(d.text(")"));
        self.after_callee(d, d.concat(&parts))
    }
}

/// The expand-last layout for an arrow with an **object / array terminal** — the head
/// arguments stay inline and the terminal expands internally
/// (`fn(arg, (x) => ({⏎\ta: x⏎}))`), through any number of curried heads.
///
/// One body for the plain-call / `new` printer and the member-chain one, which had drifted
/// into two copies of the same four steps. Everything here is load-bearing and was stated
/// twice before:
///
/// - the **willBreak bail** must precede the second printing below, since only the two states
///   past it read one (prettier's `if (headArgs.some(willBreak)) return allArgsBrokenOut()`).
///   The plain-call caller has already answered it for every arm, so there it is provably
///   false — the ask lives here so a caller cannot reach the ladder without it;
/// - both hug states read the argument's **`expandLastArg`** printing
///   ([`build_expand_last_arg_doc`]), never its `printedArguments` one — wrapping the chain
///   doc in `group_break` would break the chain's own heads and the truncated `fits` walk
///   would then hug a chain prettier breaks out on width;
/// - the pre-built terminal body is **re-injected** around that printing, so the second build
///   costs signatures rather than the whole subtree (`fanout:audit`).
///
/// `build` is the caller's argument builder — `build_expression_doc` vs
/// `build_arg_expression_doc`, the one thing the two sites still disagree on. (The
/// expand-FIRST family no longer does: it builds every argument in argument context.)
pub(super) fn build_expand_last_obj_array_doc(
    printer: &Printer<'_>,
    opener: ArgOpener,
    obj_reuse: Option<(u32, DocId)>,
    head_parts: &[DocId],
    all_args_broken: DocId,
    build: impl FnOnce() -> DocId,
) -> DocId {
    let d = printer.d();
    if let Some(bail) = opener.expand_all_if_head_breaks(d, head_parts, all_args_broken) {
        return bail;
    }
    let expanded =
        printer.with_arrow_body_inject(obj_reuse, || build_expand_last_arg_doc(printer, build));
    opener.inline_hug_or_expand_all(d, head_parts, expanded, all_args_broken)
}

/// A block-bodied callback argument's body, built for every printing of the callback to
/// share ([`ShareTag::AssumedHugBody`] across a member chain's candidates): the arrow's own
/// body build ([`Printer::arrow_block_body_doc`]) or the `function`'s block.
fn shared_callback_body_doc(
    printer: &Printer<'_>,
    arg: &ast::Expression<'_>,
    body: &ast::BlockStatement<'_>,
    is_arrow: bool,
) -> DocId {
    let key = printer.chain_share_key(arg, ShareTag::AssumedHugBody);
    printer.chain_shared_doc(key, || {
        if is_arrow {
            printer.arrow_block_body_doc(body)
        } else {
            printer.build_block_statement_doc(body)
        }
    })
}

/// The block body of a callback argument — an arrow's or a `function`'s — built once and
/// returned as the body injection ([`Printer::inject_arrow_body`]) its printings are built
/// under, so a layout that prints the callback twice costs a second signature and not a
/// second subtree. `None` for any other argument.
fn prebuild_callback_block_body(
    printer: &Printer<'_>,
    arg: &ast::Expression<'_>,
) -> Option<(u32, DocId)> {
    let (body, is_arrow) = match &arg.kind {
        ast::ExpressionKind::ArrowFunctionExpression(arrow) => match &arrow.body {
            ast::ArrowFunctionBody::BlockStatement(block) => (block, true),
            ast::ArrowFunctionBody::Expression(_) => return None,
        },
        ast::ExpressionKind::FunctionExpression(func) => (&func.body, false),
        _ => return None,
    };
    Some((
        body.span.start,
        shared_callback_body_doc(printer, arg, body, is_arrow),
    ))
}

/// Prettier's expand-FIRST layout: a block-function first argument hugs and the tail stays
/// inline past its `}` (`setTimeout(() => { tick(); }, 100)`).
///
/// The layout twin of [`should_expand_first_arg`], which gates it. The **predicate** was
/// already shared and the layout was not, which is precisely how one copy gets a fix and the
/// other doesn't — the `new` spelling reached neither the argument-gap seam nor the last
/// argument's gap and dropped a comment in each, the chain's asked its break question of the
/// argument doc alone, and the two disagreed on both the argument builder and whether the
/// layout had a fallback at all. One body now for all three, through [`ArgOpener`] — the
/// chain's `&'static str` opener and hand-rolled hardline form are gone, as they are for the
/// expand-last ladder above.
///
/// The tail is written as **everything after the first argument** — prettier's
/// `printedArguments.slice(1)` — though `should_expand_first_arg` admits exactly two, so it
/// is one argument plus the two gaps around it: the inter-argument seam
/// ([`build_arg_gap_docs`]) and the last argument's gap to the `)`
/// ([`emit_last_arg_trailing_comments`]).
///
/// The three states are [`ArgOpener::inline_hug_first_or_expand_all`]; without the last one
/// a first argument with nothing of its own to break held an over-width line forever.
///
/// Returns the fully-expanded form OUTRIGHT when the tail will break — prettier's
/// `if (tailArgs.some(willBreak)) return allArgsBrokenOut()`, which precedes the ladder
/// rather than being one of its states. An inline tail cannot carry an
/// argument that breaks: without it the first argument hugs and a broken tail trails it
/// (`new A(() => {⏎…⏎}, fn(⏎// c⏎b))`), a form prettier never emits. `tailArgs` is the printed
/// argument **plus the comments printed with it**, which is why the question is asked of the
/// whole `tail_parts` and not of the argument's own doc — ownership decides what that doc
/// carries (`docs/comments.md` hazard 2).
pub(super) fn build_expand_first_arg_doc(
    printer: &Printer<'_>,
    opener: ArgOpener,
    arguments: &[&ast::Expression<'_>],
    paren_open: u32,
    call_end: u32,
) -> DocId {
    let d = printer.d();
    // The hug states read prettier's `expandFirstArg` printing of the first argument: an
    // arrow's parameters flat (`removeLines`), so a head too wide for the callee's line falls
    // through to the broken-out state rather than shattering its parameters inside the hug.
    // That state rebuilds the argument as `printedArguments` prints it, with its parameters
    // breakable on a line of their own. A `function` first argument takes no flattening —
    // `printFunction` flattens under `expandLastArg` alone — so its parameters still break
    // inside the hug.
    let build_first = || {
        if matches!(
            arguments[0].kind,
            ast::ExpressionKind::ArrowFunctionExpression(_)
        ) {
            build_flat_params_arg_doc(printer, || printer.build_arg_expression_doc(arguments[0]))
        } else {
            printer.build_arg_expression_doc(arguments[0])
        }
    };
    let assumed = AssumedParamHug::new(printer, arguments[0]);
    // The callback's body is built ONCE and injected into both of its printings — the hug's
    // and the broken-out state's, which differ in the parameters alone. Built in each, a
    // callback first argument nested in its own body (`f(() => { f(() => { … }, a) }, a)`)
    // doubled per level. The state above already shares it where it has one.
    let inject = assumed.as_ref().map_or_else(
        || prebuild_callback_block_body(printer, arguments[0]),
        |hug| Some(hug.body_inject()),
    );
    let first_arg_doc = printer.with_arrow_body_inject(inject, build_first);

    let mut tail_parts = DocBuf::new();
    let mut prev_end = arguments[0].span().end;
    for arg in arguments.iter().skip(1) {
        let (through_comma, leading) = build_arg_gap_docs(printer, prev_end, arg.span().start);
        tail_parts.extend(through_comma);
        tail_parts.push(d.text(" "));
        tail_parts.extend(leading);
        tail_parts.push(printer.build_arg_expression_doc(arg));
        prev_end = arg.span().end;
    }
    if let Some(last_arg) = arguments.last() {
        // No argument here is frozen: this layout builds each through
        // `build_arg_expression_doc`, so the interior is the argument's own.
        emit_last_arg_trailing_comments(
            printer,
            &mut tail_parts,
            last_arg,
            last_arg.paren_interior(),
            call_end,
        );
    }

    if tail_parts.iter().any(|&id| d.will_break(id)) {
        return printer.with_arrow_body_inject(inject, || {
            build_call_args_expanded(printer, opener, arguments, paren_open, call_end)
        });
    }

    // The first argument can expand internally; the tail stays on its closing line — and
    // when even that does not fit, every argument breaks out.
    let all_args_broken = printer.with_arrow_body_inject(inject, || {
        build_call_args_expanded(printer, opener.args_only(), arguments, paren_open, call_end)
    });
    if let Some(first) = assumed
        .as_ref()
        .and_then(|hug| hug.take_forced(printer, AssumedHugKind::ParamLiteral, build_first))
    {
        return opener.after_callee(
            d,
            opener.expand_first_state(d, d.group_break(first), d.concat(&tail_parts)),
        );
    }
    let assumed = assumed.and_then(|hug| hug.build(printer, false, build_first).literal);
    opener.inline_hug_first_or_expand_all(
        d,
        first_arg_doc,
        d.concat(&tail_parts),
        assumed,
        all_args_broken,
    )
}

/// Check if the last two arguments have the same outer AST type.
/// Prettier disables expand-last-arg hug state when `penultimateArg.type === lastArg.type`
/// (`couldExpandArg`, `print/call-arguments.js`). This covers both arrays, both objects, and also both TSAsExpression,
/// both TSSatisfiesExpression, etc.
pub(super) fn last_two_args_same_type(args: &[&ast::Expression<'_>]) -> bool {
    let last = args[args.len() - 1];
    let penultimate = args[args.len() - 2];
    std::mem::discriminant(&last.kind) == std::mem::discriminant(&penultimate.kind)
}

/// Build the "break body" state for expand-last-arg with an expression arrow.
///
/// Layout: `opener + head_parts + sig => \n  body\n)` — the head arguments stay inline and
/// only the last argument's body breaks after its `=>`.
///
/// `conditional_body` keeps the space after `=>` that a ternary body's break leaves at the
/// line end, which the renderer trims but a `fits` measurement counts: prettier prints such a
/// body as `" ", group([ifBreak("", "("), indent([softline, body]), …])`
/// (`printArrowFunctionBody`'s `shouldAddParensIfNotBreak` arm), so a `…, (a) =>` line of
/// exactly the print width does not fit there. Every other body breaks with no space
/// (`indent([line, body])`).
#[inline]
fn build_break_body_state(
    d: &DocArena,
    opener: ArgOpener,
    head_parts: &[DocId],
    sig_doc: DocId,
    body_doc: DocId,
    conditional_body: bool,
) -> DocId {
    d.concat(&[
        opener.open,
        d.concat(head_parts),
        sig_doc,
        d.text(arrow_before_broken_body(conditional_body)),
        d.indent_hardline(body_doc),
        d.hardline(),
        d.text(")"),
    ])
}

/// The `=>` an arrow argument prints ahead of a body that breaks onto the next line: with the
/// trailing space for a ternary body ([`build_break_body_state`]), which the renderer trims at
/// the break but a `fits` measurement of the `=>` line counts.
#[inline]
const fn arrow_before_broken_body(conditional_body: bool) -> &'static str {
    if conditional_body { " => " } else { " =>" }
}

/// The expand-last ladder for a last argument that is an expression arrow whose body BREAKS
/// after the `=>` — a call (through a trailing `!`) or a ternary: inline → break body →
/// expand all (`fn({ a: 1 }, (x) =>⏎\tcall(x, …)⏎)`).
///
/// One body for the plain-call / `new` printer and the member-chain one, which had drifted
/// into two copies of the same ladder. The pieces a caller genuinely owns are its `opener`,
/// its already-built signature and body docs, and the two argument docs the states read; the
/// rest is stated once.
///
/// The flat state is dropped when the last argument's own doc will break — prettier's rule,
/// and not an optimization: `fits()` would select it (its first line is short) but it prints
/// the wrong closing brackets (`}));` instead of `}),⏎)`).
///
/// `sig_doc` is [`build_hugged_arrow_sig_doc`]'s: `None` for a signature that refuses the
/// hug, which leaves only the all-broken-out state, prettier's `ArgExpansionBailout`.
/// `conditional_body` is whether the body is a ternary ([`build_break_body_state`]).
#[expect(
    clippy::too_many_arguments,
    reason = "the ladder's pieces are each the caller's own; a struct would only rename them"
)]
pub(super) fn build_break_body_ladder(
    d: &DocArena,
    opener: ArgOpener,
    head_parts: &[DocId],
    sig_doc: Option<DocId>,
    body_doc: DocId,
    conditional_body: bool,
    last_arg_doc: DocId,
    all_args_broken: DocId,
) -> DocId {
    let Some(sig_doc) = sig_doc else {
        return opener.expand_all(d, all_args_broken);
    };
    let state_expand_all = opener.expand_all_state(d, all_args_broken);
    if d.will_break(last_arg_doc) {
        // A break the body does not carry is the signature's own — a comment in an empty
        // parameter list, the one forced break a hugged signature keeps
        // ([`build_arrow_sig_doc`]). Prettier hugs the argument broken open
        // (`group(lastArg, { shouldBreak: true })`), so the comment keeps its line and the
        // body decides its own break below it; the break-body state would force one.
        let state_hug = if d.will_break(body_doc) {
            build_break_body_state(d, opener, head_parts, sig_doc, body_doc, conditional_body)
        } else {
            opener.hug(d, head_parts, last_arg_doc)
        };
        return opener.ladder(d, &[state_hug, state_expand_all]);
    }
    let state_break_body =
        build_break_body_state(d, opener, head_parts, sig_doc, body_doc, conditional_body);
    opener.ladder(
        d,
        &[
            opener.inline(d, head_parts, last_arg_doc),
            state_break_body,
            state_expand_all,
        ],
    )
}

/// The three-state ladder a **lone** arrow argument with a TERNARY body selects among:
/// flat with the grammar-clarifying parens (`map((x) => (x ? y : z))`), the body broken after
/// `=>` with the parens dropped, then the argument on its own line, printed whole as
/// prettier's broken-out argument prints it (`map(⏎\t(x) => (x ? y : z)⏎)`, the parameters
/// breaking first when the `=>` line overflows: `(⏎\t\tx…⏎\t) => (x ? y : z)`). The break
/// state's `=>` line is measured with the space a ternary body leaves where prettier leaves
/// one (`flat_pair`, below; [`arrow_before_broken_body`]), so a `map((x) =>` line of exactly
/// the print width breaks the argument out as prettier does.
///
/// The third parallel copy of this sat one per printer. What a caller owns is its `opener`,
/// its signature and body docs, and `printed`, the whole argument as its argument builder
/// prints it. A forced break in the BODY drops the flat state: `fits()` would otherwise pick
/// it on a truncated measurement and then print the parens around an already-broken body.
///
/// The question is asked of the body, never the whole arrow: a break the signature carries is
/// not one the break state answers. Read off the whole arrow it selected the break state on a
/// break the state then collapsed, and the collapsed output took the flat state on the next
/// pass. Every caller refuses a signature that will break ([`arrow_signature_refuses_hug`]),
/// so the call breaks every argument out with the argument printed whole — all but the one
/// break a hugged signature keeps, a comment in an empty parameter list
/// ([`build_arrow_sig_doc`]), which the flat state prints on its own line as prettier's hug
/// does (`fn((⏎\t// c⏎) => (a ? b : c))`).
///
/// `flat_pair` is whether the flat state takes those parens — and whether the break state's
/// `=>` line counts the space prettier leaves before them: not for a body whose leftmost token
/// takes its own pair (`({}).x ? b : c`,
/// [`has_leftmost_arrow_body_parens`](crate::printer::expressions::functions::has_leftmost_arrow_body_parens))
/// or one a comma-list region leaves bare ([`Printer::arrow_body_takes_ternary_pair`]). Nor
/// is the space counted when a comment in the `=>`→body gap ends its line
/// ([`arrow_body_has_leading_own_line_comment`]): prettier then breaks the body below `=>`
/// with none.
pub(super) fn build_ternary_arrow_hug_ladder(
    printer: &Printer<'_>,
    opener: ArgOpener,
    arrow: &ast::ArrowFunctionExpression<'_>,
    body_expr: &ast::Expression<'_>,
    sig_doc: DocId,
    body_doc: DocId,
    printed: impl FnOnce() -> DocId,
) -> DocId {
    let d = printer.d();
    let flat_pair = printer.arrow_body_takes_ternary_pair(body_expr);
    let prefix = opener.open;
    let body_start = body_expr.span().start;
    // All broken: the argument drops to its own line, printed WHOLE as prettier's
    // broken-out argument prints it (`printedArguments`) — its own printer decides whether
    // the parameters break, whether the body stays on the `=>` line in its parens or breaks
    // below it, and where a comment run in the `=>`→body gap goes. The state an enclosing
    // break mode falls through to. The body is the one already built for the states above,
    // INJECTED into the whole-arrow build rather than built again
    // ([`Printer::with_arrow_body_inject`], as [`build_arrow_call_body_states`] does): a
    // ternary branch holding another lone ternary-body arrow argument would otherwise build
    // its body twice per level, O(2^depth).
    let whole = printer.with_arrow_body_inject(Some((body_start, body_doc)), printed);
    let state_all_broken = d.concat(&[
        prefix,
        d.indent(d.concat(&[d.hardline(), whole])),
        d.hardline(),
        d.text(")"),
    ]);
    // The `=>`→body gap's comments, composed per state: glued where the body stays on the
    // `=>` line, a run the author broke after keeping its line where the body sits below
    // ([`prepend_arrow_body_comments`]).
    let body_below = prepend_arrow_body_comments(printer, arrow, body_start, body_doc);
    let body_doc = prepend_hugged_arrow_body_comments(printer, arrow, body_start, body_doc);
    // The arrow with its body broken after `=>`, whose measured `=>` line keeps the space a
    // ternary body's break leaves ([`arrow_before_broken_body`]).
    let arrow_before = arrow_before_broken_body(
        flat_pair && !arrow_body_has_leading_own_line_comment(printer, arrow, body_expr),
    );
    let arrow_broken = d.concat(&[sig_doc, d.text(arrow_before), d.indent_hardline(body_below)]);
    // Break: the first hardline breaks after `=>`, the second drops `)` to its own line.
    // No trailing comma (trailingComma: 'none').
    let state_break = d.concat(&[prefix, arrow_broken, d.hardline(), d.text(")")]);
    if d.will_break(body_doc) {
        // No flat state, but the break state still falls through to the argument on its own
        // line: a `callee((sig) =>` line too wide to hug breaks the argument out, as prettier's
        // `conditionalGroup` does past its hugged state.
        return opener.ladder(d, &[state_break, state_all_broken]);
    }
    // The arrow on one line: the parens the grammar wants around a ternary body.
    let arrow_flat = if flat_pair {
        d.concat(&[sig_doc, d.text(" => ("), body_doc, d.text(")")])
    } else {
        d.concat(&[sig_doc, d.text(" => "), body_doc])
    };
    // Flat: the arrow, then the call's own `)`.
    let state_flat = d.concat(&[prefix, arrow_flat, d.text(")")]);
    opener.ladder(d, &[state_flat, state_break, state_all_broken])
}

/// The ladder for a **lone** arrow argument whose expression body is a call (through a
/// trailing `!`): the plain-call and `new` spellings of the member chain's call-body arm.
///
/// Prettier's expand-last ladder for one argument: flat, the signature hugged with the body
/// broken after `=>` (`callee((sig) =>⏎\tbody⏎)`), then `allArgsBrokenOut()` — the argument
/// on its own line, which is what a signature too wide for the `callee(` line falls through
/// to. One ladder with the call's other expand-last spellings ([`build_break_body_ladder`]),
/// which also drops the flat state when the argument will break.
///
/// Two printings, as prettier's: the hug states reassemble the signature and the body, and
/// the broken-out state prints the whole argument. The body is built ONCE and injected into
/// the whole-argument build ([`prebuild_expand_last_break_body`]): building it in both
/// recursed into itself for a call-bodied arrow whose body is another such call
/// (`a(x => b(y => …))`), making the doc-node count O(2^depth). A commented argument is
/// injected too — one comment in the innermost body puts every enclosing argument's span on
/// page, so skipping it there rebuilt every level twice; the comment positions that need a
/// different composition never reach this arm (`arrow_hug_refused_by_comments`).
///
/// `body_reuse` is that prebuild, which the caller takes as the last test of its gate — the
/// prebuild takes every call body, so it never declines there. `printed`
/// builds the whole argument as the caller's argument builder prints it.
pub(super) fn build_arrow_call_body_states(
    printer: &Printer<'_>,
    opener: ArgOpener,
    arg: &ast::Expression<'_>,
    arrow: &ast::ArrowFunctionExpression<'_>,
    body_expr: &ast::Expression<'_>,
    body_reuse: (u32, DocId),
    printed: impl FnOnce() -> DocId,
) -> DocId {
    let d = printer.d();
    let arrow_doc = printer.with_arrow_body_inject(Some(body_reuse), printed);
    let (_, body_doc) = body_reuse;
    let body_doc = prepend_arrow_body_comments(printer, arrow, body_expr.span().start, body_doc);
    let sig_doc = build_hugged_arrow_sig_doc(printer, arrow, arg.span);
    build_break_body_ladder(
        d,
        opener,
        &[],
        sig_doc,
        body_doc,
        false,
        arrow_doc,
        arrow_doc,
    )
}

/// Build argument docs joined with breaks, preserving inter-argument comments.
///
/// Like `join_doc(args, separator)` but handles leading/trailing comments
/// between arguments. Used by expansion paths (all-arrows, function composition)
/// that would otherwise lose comments with simple `join_doc`.
///
/// `join` picks the separator style and says whether the `(`→first-argument gap is this
/// builder's to emit. `paren_close` bounds the
/// LAST argument's trailing gap, and `paren_line` receives the `(`-line comment run (see
/// [`emit_first_arg_leading_comments`]) — a non-empty run obliges the caller to wrap with
/// [`wrap_call_with_hard_breaks_paren_line`].
pub(super) fn build_args_joined_with_comments(
    printer: &Printer<'_>,
    arguments: &[&ast::Expression<'_>],
    paren_open: u32,
    paren_close: u32,
    join: ArgsJoin,
    paren_line: &mut DocBuf,
) -> DocId {
    let d = printer.d();
    let mut parts = DocBuf::new();
    let use_hardline = join.use_hardline();

    // Leading comments before first arg (e.g., `fn(/* c */ arg)`)
    let first_arg_start = arguments[0].span().start;
    if !matches!(join, ArgsJoin::HardlineLeadingGapEmitted) {
        emit_first_arg_leading_comments(
            printer,
            paren_line,
            &mut parts,
            paren_open,
            first_arg_start,
        );
    }

    let no_comment_sep = if use_hardline {
        d.comma_hardline()
    } else {
        d.comma_line()
    };

    // Whether the gap just closed — between `arguments[i - 1]` and `arguments[i]` — carries an
    // author blank line. Computed once at the bottom of the previous iteration (the no-comment
    // branch below) and reused here, since the top of this iteration and that bottom look at
    // the same gap under the same no-comment guard. Stays `false` for every join but
    // [`ArgsJoin::HardlinePreserveBlanks`].
    let mut prev_gap_has_blank = false;
    for (i, arg) in arguments.iter().enumerate() {
        // The preserved blank is emitted at the TOP of the next iteration, not at the bottom of
        // the previous one, so it lands after the comma rather than before it. Nothing to emit
        // in the gap, but a comment can still physically *be* there — an owned annotation
        // leading this argument. `prev_gap_has_blank` was measured with `blank_scan_end`, so
        // the annotation's own newlines don't read as a blank line yet an authored blank
        // *before* it is kept.
        if prev_gap_has_blank
            && i > 0
            && !printer.has_comments_to_emit_between(arguments[i - 1].span().end, arg.span().start)
        {
            parts.push(d.literalline());
            parts.push(d.hardline());
        }

        parts.push(build_joined_argument_doc(printer, paren_open, arguments, i));

        if i < arguments.len() - 1 {
            let next_arg_start = arguments[i + 1].span().start;

            if printer.inter_arg_gap_has_comments(arg, next_arg_start) {
                let interior = printer.arg_paren_interior(paren_open, arguments, i);
                let gap = printer.open_inter_arg_gap(&mut parts, arg, interior, next_arg_start);
                // The gap's `forces_expansion` obligation is the callers': the soft-join
                // callers are unreachable when any spread interior forces expansion —
                // their earlier trailing-comment arms, keyed on
                // `any_paren_interior_forces_expansion`, return first — and every
                // other join is hardline.
                debug_assert!(!gap.forces_expansion || use_hardline);
                // A commented gap's blank is comment-aware (routed, so a comment's own
                // newlines don't read as one) and rides here rather than at the next
                // iteration's top, whose guard skips a gap that holds a comment.
                // Behind a spread's stripped-paren interior the run past the `)` follows the
                // interior on lines of its own, where an author blank below it survives in
                // every layout, as it does when the run is a leading one.
                if (join.preserves_blanks() || gap.comments.is_behind_interior())
                    && gap.comments.has_blank_line_in_gap(printer)
                {
                    parts.push(d.literalline());
                }
                // A line comment runs to EOL → hard-break; otherwise honor the caller's style.
                parts.push(if gap.comments.has_trailing_line() || use_hardline {
                    d.hardline()
                } else {
                    d.line()
                });
                // hugging after-comma + own-line comments lead the next arg (`C`).
                gap.comments
                    .emit_leading_comments_inline_aware(&mut parts, printer);
                prev_gap_has_blank = false;
            } else if join.preserves_blanks() {
                // Split from `no_comment_sep`: the comma is emitted now and the break is
                // DEFERRED to the next iteration's top, which owns the blank's own pair.
                parts.push(d.text(","));
                let arg_end = arg.span().end;
                prev_gap_has_blank = printer
                    .is_next_line_empty(arg_end, printer.blank_scan_end(arg_end, next_arg_start));
                if !prev_gap_has_blank {
                    parts.push(d.hardline());
                }
            } else {
                parts.push(no_comment_sep);
            }
        } else {
            // The LAST argument's gap to `)` is this builder's too — the counterpart to
            // `open_inter_arg_gap` above. These layouts are reached by force-expansion
            // triggers (function composition, the expand-first fallback) that preempt the
            // callers' own comment-aware paths, so nothing else emits it and the loss is
            // total.
            let interior = printer.arg_paren_interior(paren_open, arguments, i);
            emit_last_arg_trailing_comments(printer, &mut parts, arg, interior, paren_close);
        }
    }

    d.concat(&parts)
}

/// The forced-expansion argument layout the call and `new` hardline arms share (function
/// composition, all-arrows, the expand-first fallback, and every ladder's all-args-broken
/// state): one argument per line with the gap comments preserved, wrapped in the call's
/// hard-broken parens.
pub(super) fn build_call_args_expanded(
    printer: &Printer<'_>,
    opener: ArgOpener,
    arguments: &[&ast::Expression<'_>],
    paren_open: u32,
    paren_close: u32,
) -> DocId {
    let mut paren_line = DocBuf::new();
    let args = build_args_joined_with_comments(
        printer,
        arguments,
        paren_open,
        paren_close,
        ArgsJoin::Hardline,
        &mut paren_line,
    );
    opener.wrap_hard_broken(printer.d(), &paren_line, args)
}

/// How [`build_args_joined_with_comments`] separates arguments, and who owns the
/// `(`→first-argument gap.
#[derive(Clone, Copy)]
pub(super) enum ArgsJoin {
    /// Hardline separators — forced expansion, one argument per line.
    Hardline,
    /// [`Self::Hardline`], but the caller has already emitted the `(`→first-argument gap
    /// itself (the `new` paren-line-prefix path does), so this builder must not print it a
    /// second time. A variant rather than a narrowed `paren_open`, because `paren_open`
    /// also anchors the first argument's FREEZE window: a directive alone on its line
    /// inside an already-emitted gap still freezes the first argument, and passing the
    /// first argument's own start as `paren_open` would leave that window empty.
    HardlineLeadingGapEmitted,
    /// [`Self::Hardline`], but an author blank line in a gap is PRESERVED rather than
    /// collapsed — prettier's `anyArgEmptyLine` layout, reached through
    /// [`build_call_args_with_blank_lines`]. A separate variant rather than a `bool`
    /// parameter because it is the same axis as the other three: how the arguments separate.
    HardlinePreserveBlanks,
    /// Soft-line separators — break only when the enclosing group breaks. A trailing line
    /// comment in a gap still forces a hardline.
    SoftLine,
}

impl ArgsJoin {
    fn use_hardline(self) -> bool {
        !matches!(self, Self::SoftLine)
    }

    /// Whether an author blank line in a gap survives. Only [`Self::HardlinePreserveBlanks`]
    /// keeps it; every other layout is reached by a force-expansion trigger that has no
    /// author blank to preserve in the first place.
    fn preserves_blanks(self) -> bool {
        matches!(self, Self::HardlinePreserveBlanks)
    }
}

/// The doc for argument `i` of a joined-argument layout: the verbatim slice when an
/// own-line format-ignore directive in its gap freezes it (Rule A), else the ordinary
/// argument-context builder — so a binary/logical chain (or conditional) keeps its
/// continuation indent and an assignment gets clarity parens.
///
/// Every layout in this family builds its arguments the same way, expand-first's
/// broken-out fallback included; a plain-builder spelling there was how it and the chain's
/// twin came to disagree on a breaking binary tail's continuation indent.
///
/// Every layout that reaches here is a **broken-out** one — nothing in this family
/// hugs an argument onto the callee's line — so the build goes through
/// [`build_printed_argument_doc`]; a curried chain that skipped the progressive layout
/// here would keep its heads welded to the first one. Rule A still wins inside it:
/// a frozen argument is a verbatim source slice, which no layout context can reach,
/// so the wrapper is inert on that arm.
pub(super) fn build_joined_argument_doc(
    printer: &Printer<'_>,
    paren_open: u32,
    args: &[&ast::Expression<'_>],
    i: usize,
) -> DocId {
    build_printed_argument_doc(printer, args[i], || {
        printer.build_arg_item_doc(paren_open, args, i)
    })
}

/// Check if a call/new should use the "expand first arg" pattern.
///
/// This matches prettier's behavior for calls like `setTimeout(() => {...}, 100)`:
/// - First arg is function/arrow with block body
/// - Remaining args are "hopefully short" (simple values)
/// - Result: first arg expands, tail args stay inline after closing `}`
pub(super) fn should_expand_first_arg(
    printer: &Printer<'_>,
    args: &[&ast::Expression<'_>],
) -> bool {
    // Need exactly 2 args (first is function, second is short)
    if args.len() != 2 {
        return false;
    }

    // First arg must be a function with block body
    if !is_block_function(args[0]) {
        return false;
    }

    // Second arg must be short/simple. `couldExpandArg`'s `hasComment(node)` needs to see
    // the argument's leading gap, so the previous argument's end goes with it — a comment
    // there attaches to a BARE collection (blocking the hug) but to the CAST of a wrapped
    // one (`/* c */ {} as T` still hugs); the predicate owns that asymmetry. The expand-first
    // path then carries the inter-arg leading comment through the shared argument-gap seam
    // (`build_arg_gap_docs`).
    //
    // **on page**: `hasComment` is a pure layout question, so an owned annotation blocks the
    // hug just like any other comment.
    is_short_second_arg_for_expand_first(args[1], args[0].span().end, |start, end| {
        printer.has_comments_on_page_between(start, end)
    })
}

/// Whether the first argument's own signature refuses the expand-FIRST hug — prettier's
/// `ArgExpansionBailout` reached through `expandFirstArg` instead of `expandLastArg`.
///
/// The expand-first hug renders the first argument's signature head on the callee's line,
/// so a signature that will break cannot be honored there, exactly as at the expand-last
/// states ([`arrow_signature_refuses_hug`], which this delegates to so the two directions ask
/// one question — its sole-parameter carve-out included, since an expand-first argument is
/// always block-bodied). Every argument breaks out instead, where the signature may break on
/// a line of its own.
///
/// A comment in an EMPTY parameter list refuses too, though no bailout reads it (the list
/// returns ahead of the throw): it is a comment on the argument itself, and
/// `shouldExpandFirstArg` opens with `!hasComment(firstArg)`. The expand-last direction has
/// no such gate, so there the argument hugs with the comment on its own line.
///
/// **Arrow only, deliberately.** `printArrowFunctionSignature` takes
/// `shouldExpandParameters = expandLastArg || expandFirstArg`, so an arrow bails out in both
/// directions; `printFunction` gates its own on `args.expandLastArg` **alone**, so a
/// `function` first argument never bails out and keeps the hug with its signature broken —
/// in every callee spelling (plain call, `new`, member chain). Reaching for the kind-agnostic
/// [`callback_signature_refuses_hug`](crate::printer::expressions::functions::callback_signature_refuses_hug)
/// here would expand a list prettier hugs.
pub(super) fn first_arg_signature_refuses_expand_first(
    printer: &Printer<'_>,
    args: &[&ast::Expression<'_>],
) -> bool {
    matches!(
        args.first(),
        Some(ast::Expression { span, kind: ast::ExpressionKind::ArrowFunctionExpression(arrow) })
            if arrow_signature_refuses_hug(printer, arrow, *span)
                || printer.arrow_empty_params_hold_comments(arrow)
    )
}

/// Append type arguments (`fn<T>`, `new Foo<K, V>`) to a callee doc, preserving
/// comments in the gap between the callee and `<`.
///
/// Uses `build_name_to_type_params_comments` for safe line comment handling.
pub(super) fn append_type_args_with_gap_comments(
    printer: &Printer<'_>,
    callee: DocId,
    callee_end: u32,
    type_arguments: Option<&ast::TSTypeParameterInstantiation<'_>>,
) -> DocId {
    let d = printer.d();
    match type_arguments {
        Some(ta) => {
            let ta_doc = printer.build_type_parameter_instantiation_doc(ta);
            match printer.build_name_to_type_params_comments_opt(
                callee_end,
                ta.span.start,
                CommentSpacing::Trailing,
            ) {
                Some(comments_doc) => d.concat(&[callee, comments_doc, ta_doc]),
                None => d.concat(&[callee, ta_doc]),
            }
        }
        None => callee,
    }
}

/// Build the doc for a call/new with no arguments (`fn()`, `new Foo<K, V>()`),
/// preserving dangling comments before the `(` and inside the empty parens.
///
/// `after_type_args` is the position after the type arguments (or the callee
/// when there are none); the actual `(` is located to separate pre-paren
/// comments from inside-paren comments, e.g. `fn<string> /* c */()`. `optional`
/// fuses `?.` into the list's opening (`call /* c */?.()`), so a gap comment
/// stays before it — the same `?.(` prefix the member-chain path passes.
pub(super) fn build_empty_args_doc(
    printer: &Printer<'_>,
    callee: DocId,
    after_type_args: u32,
    paren_close: u32,
    optional: bool,
) -> DocId {
    let d = printer.d();
    let prefix = if optional { d.text("?.(") } else { d.text("(") };
    d.concat(&[
        callee,
        build_empty_args_parens_doc(printer, after_type_args, paren_close, prefix),
    ])
}

/// Whether the sole-multiline-template hug applies — [`try_hug_multiline_template_arg`]'s
/// whole guard set, named because the call DISPATCHER has to ask the same question one level
/// up (a memberish callee is routed into the chain printer before any argument layout is
/// consulted, and this rule outranks that routing in prettier).
///
/// `gap_start` is where the `(` follows, per each family's convention — see
/// [`try_hug_multiline_template_arg`].
pub(super) fn multiline_template_hug_applies(
    printer: &Printer<'_>,
    args: &[&ast::Expression<'_>],
    gap_start: u32,
) -> bool {
    is_sole_multiline_template_arg(args)
        // Prettier's `isTemplateOnItsOwnLine`: a template the author put on a line of its
        // own declines, and the caller falls through to the expanded layout.
        && !sole_template_on_its_own_line(
            printer,
            gap_start,
            args[0].span().start,
        )
        // An honored directive in this gap DECLINES the hug — the parameter list's answer to
        // the same question (`docs/conformance_prettier_ignore.md` §On parameter lists: "a
        // lone huggable parameter expands rather than hugging, because a hug would pull the
        // directive off its own line and make it inert"). The hug is a flat concat with no
        // line of its own to put the run on, so the directive would land on the `(`'s —
        // inert — and the freeze this very gap grants would be gone on pass 2. Prettier hugs
        // and stays frozen (its directive lookup is attachment-based, not line-based), so
        // the expanded form is a cataloged divergence rather than a match.
        //
        // Not implied by the newline test above: the directive's own trailing newline puts
        // the template on a fresh line only when nothing else follows it, and
        // `` fn(⏎// prettier-ignore⏎/* x */ `a⏎b`) `` glues a SECOND comment to the backtick.
        && printer.args_frozen_span(gap_start, args, 0).is_none()
}

/// Prettier's `isTemplateOnItsOwnLine` for a sole multiline template argument, read at the
/// rule's FIXED POINT: whether the template starts a line of its own, asked of what the
/// chosen layout PRINTS ahead of the backtick rather than of the source alone.
///
/// Prettier asks the source — a newline between the backtick and the last thing before it
/// — and with no comment in the `(`-line gap that is a fixed point both ways: the hug
/// prints the backtick behind the `(`, the expanded list on a line of its own. A comment
/// run in the gap breaks that, because what precedes the backtick in the OUTPUT is the
/// run's last separator, which the source test never reads:
///
/// - hugged behind a run that ends its line (a `//`, an own-line block — often one the
///   printer relocated from the callee→`(` gap or out of a stripped shell:
///   `` x()⏎/* c */⏎(`⏎`) ``, `` a(// c⏎(`⏎`)) ``), the backtick lands on a line of its own
///   and the next pass expands;
/// - expanded behind a run that glues to the backtick (a block glued to a stripped `(`
///   whose newline the source test read: `` a(/* c */ (⏎`a⏎b`)) ``), the backtick follows
///   the comment and the next pass hugs.
///
/// Prettier takes those two passes; tsv answers with the layout whose output reads as
/// itself. The authored answer stands when its own layout is stable — or when neither is,
/// where no answer converges. Each layout's last separator is its emitter's own decision
/// ([`Printer::rhs_comments_glued_tail_separator`] for the hug,
/// [`Printer::leading_run_tail_separator`] in the argument list's stripped-paren mode for
/// the expanded list), and a comment OWNED by the template is glued to it in both.
///
/// The one definition, asked by every spelling of the hug: the plain call and `new`
/// ([`multiline_template_hug_applies`]), the member chain's redirect preempt and the chain's
/// own link, whose hug prints the plain call's run.
pub(in crate::printer) fn sole_template_on_its_own_line(
    printer: &Printer<'_>,
    gap_start: u32,
    template_start: u32,
) -> bool {
    let authored = has_newline_before_position(printer.source, template_start);
    if !printer.has_comments_on_page_between(gap_start, template_start) {
        return authored;
    }
    sole_template_on_its_own_line_commented(printer, gap_start, template_start, authored)
}

/// [`sole_template_on_its_own_line`] past its comment gate.
#[inline(never)]
fn sole_template_on_its_own_line_commented(
    printer: &Printer<'_>,
    gap_start: u32,
    template_start: u32,
    authored: bool,
) -> bool {
    use crate::printer::comments::{LeadingGlue, LeadingSeparator};
    let owned_last = printer
        .comments_on_page_between(gap_start, template_start)
        .last()
        .is_some_and(|c| c.owned_by_node);
    let (hug_glued, list_glued) = if owned_last {
        (true, true)
    } else {
        let list_glued = printer
            .leading_run_tail_separator(
                printer.comments_to_emit_between(gap_start, template_start),
                template_start,
                LeadingGlue::AdjacentStrippedParen,
            )
            .is_some_and(|(separator, _)| separator == LeadingSeparator::Space);
        let hug_glued = printer.rhs_comments_glued_tail_separator(gap_start, template_start)
            == Some(LeadingSeparator::Space);
        (hug_glued, list_glued)
    };
    if authored {
        // Expanded unless its output glues the backtick back and the hug's keeps it glued.
        !(list_glued && hug_glued)
    } else {
        // Hugged unless its output strands the backtick and the expanded list's keeps it so.
        !hug_glued && !list_glued
    }
}

/// The shape half of [`multiline_template_hug_applies`] — the argument list is one
/// multiline template — which reads the arguments alone.
///
/// Separate so a caller that would have to *derive* the gap position before it could ask
/// the whole question (the call dispatcher resolves a callee gap for it) can ask this
/// first: nearly no call has the shape, and a call without it is answered here.
#[inline]
pub(super) fn is_sole_multiline_template_arg(args: &[&ast::Expression<'_>]) -> bool {
    matches!(args, [arg] if is_multiline_template_expression(arg))
}

/// Single multiline-template argument on the same line as `(` — hug it,
/// keeping trailing comments as a line suffix.
///
/// Prettier has source-position-dependent behavior (isTemplateOnItsOwnLine):
/// - Hugged: `` fn(`line1\nline2`) `` → keep inline (no groups)
/// - Expanded: template on its own line → returns None so the caller falls
///   through to the ordinary argument layouts, which break on the template's own
///   newline through their `will_break` guards.
///
/// `gap_start` opens the `(`-line gap — the position after the callee and its type
/// arguments, i.e. the same `paren_open` every other argument seam scans from. The gap
/// spans the `(` itself, so a comment the author wrote on either side of it
/// (`` fn/* c */(`…`) ``, `` fn(/* c */ `…`) ``) lands in the one region and takes the one
/// emitter; prettier reaches the same place from the other side, attaching both spellings
/// as the argument's own leading comments.
///
/// ⚠️ A `//` CAN be in that gap, and the tempting argument that it cannot is one comment
/// short: a line comment ends its line, but the hug's own test asks only what precedes the
/// BACKTICK, so `` fn(// c⏎/* x */ `a⏎b`) `` glues a second comment to the template and the
/// hug still fires. The leading-run emitter handles it (`//` takes a hardline separator, so
/// it keeps its line and swallows nothing) — but a caller must not assume this gap is
/// block-only. It is the same near-miss the honored-directive conjunct in
/// [`multiline_template_hug_applies`] documents, and the same one that makes a chain callee's
/// DEFERRED `//` weld here (see `calls/mod.rs`'s bypass, which declines for it).
pub(super) fn try_hug_multiline_template_arg(
    printer: &Printer<'_>,
    callee: DocId,
    args: &[&ast::Expression<'_>],
    gap_start: u32,
    paren_close: u32,
) -> Option<DocId> {
    if !multiline_template_hug_applies(printer, args, gap_start) {
        return None;
    }
    let template_start = args[0].span().start;
    let d = printer.d();
    // Argument context, matching what the member chain's twin arm splices — a no-op for a
    // template either way, and the shape the rule is stated in.
    let arg_doc = printer.build_arg_expression_doc(args[0]);
    let mut parts: DocBuf = smallvec![callee, d.text("(")];
    // The `(`-line gap. Emitted here or DROPPED — this builder reassembles the call from
    // two texts plus the argument's doc, so nothing outside can see in (hazard 4 in
    // docs/comments.md). An OWNED comment (one the author glued to the backtick) rides
    // `arg_doc` instead and the emitter returns `None` for it, which is what keeps the two
    // from double-printing the same bytes.
    if let Some(leading) = printer.build_rhs_comments_glued_opt(gap_start, template_start) {
        parts.push(leading);
    }
    parts.push(arg_doc);
    // The template→`)` gap, per the canonical trailing-run rule: a block inline before
    // the `)`, a `//` deferred past it (the hug keeps the call flat, so the suffix
    // flushes at the statement's own break — `` fn(`…`); // c ``, matching prettier),
    // an own-line comment keeping its own line inside the suffix. Must-break ignored:
    // the hug is the point of this path.
    printer.push_trailing_comments_in_range(&mut parts, args[0].span().end, paren_close);
    parts.push(d.text(")"));
    Some(d.concat(&parts))
}

/// Build a call/new whose arguments have blank lines between them (hardline expansion,
/// preserving at most one blank line per gap): `callee(\n\targ1,\n\n\targ2\n)` — prettier's
/// `allArgsBrokenOut()` under `anyArgEmptyLine`.
///
/// The exact shape of [`build_call_args_expanded`], differing only in the two arguments that
/// say so: [`ArgsJoin::HardlinePreserveBlanks`] instead of [`ArgsJoin::Hardline`]. It was a
/// hand-rolled second copy of [`build_args_joined_with_comments`]'s per-argument loop until
/// the blank seams moved into that loop — which is where they belong, because the loop owns
/// all four gap questions (the `(`→first-argument run, each inter-argument gap's comments,
/// that gap's blank, and the last argument's run to `)`), and a second copy meant each new
/// answer had to be written twice or silently diverge. It still owns the WRAP for its own
/// reason: the `(`-line comment run must not escape as an out-param a caller could forget to
/// inject.
///
/// Both edge gaps are the shared loop's — emitted there or DROPPED, the hazard-4 shape in
/// docs/comments.md. Reachable from the `new` cascade, whose comment paths do not preempt
/// this one the way the plain call's do; `blanks:audit` found the `(`→first-argument half by
/// injecting a blank line beside a leading comment.
pub(super) fn build_call_args_with_blank_lines(
    printer: &Printer<'_>,
    callee: DocId,
    args: &[&ast::Expression<'_>],
    paren_open: u32,
    paren_close: u32,
) -> DocId {
    let mut paren_line = DocBuf::new();
    let args_doc = build_args_joined_with_comments(
        printer,
        args,
        paren_open,
        paren_close,
        ArgsJoin::HardlinePreserveBlanks,
        &mut paren_line,
    );
    wrap_call_with_hard_breaks_paren_line(printer.d(), callee, &paren_line, args_doc)
}
