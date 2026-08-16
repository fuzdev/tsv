// Argument classification and wrapping utilities for call expressions
//
// Handles:
// - Argument classification for chain contexts
// - Call expression wrapping with soft/hard breaks
// - Building argument lists split into head/last patterns

use super::super::{
    ArrowChainContext, CommentSpacing, Printer, is_curried_arrow_chain,
    is_multiline_template_expression,
};
use super::arg_comments::{
    any_arg_gap_has_comment_on_page, build_arg_gap_docs, emit_first_arg_leading_comments,
    emit_last_arg_trailing_comments, push_empty_args,
};
use super::arg_predicates::{
    arrow_body_is_call_through_non_null, is_block_function, is_react_hook_call_with_deps_array,
    is_short_second_arg_for_expand_first,
};
use crate::ast::internal;
use crate::printer::expressions::functions::{arrow_token_end, has_leftmost_object_expression};
use smallvec::{SmallVec, smallvec};
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
/// Prettier's other printing — the `expandLastArg` `lastArg` its hug states read — has
/// exactly one caller left, [`Printer::skip_arrow_chain`]'s, and only for a chain
/// `arrow_chain_should_break` forces open: see `call_formatting.rs`'s
/// `build_block_arrow_hug_states`, which states why every other chain needs no second build
/// (and why paying for one would be 2^depth).
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
/// descent: the hug of a chain `arrow_chain_should_break` forces open (the one position that
/// still sets the flag) suppressed the progressive layout of an entirely different chain
/// nested in its body
/// (`fn(({ a }) => ({ b }) => { return g((x, …) => (y) => z); })`). Every argument of every
/// nested call routes through here, which is exactly the seam where the flag stops applying.
pub(crate) fn build_printed_argument_doc(
    printer: &Printer<'_>,
    arg: &internal::Expression<'_>,
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
pub(crate) fn build_arrow_sig_doc(
    printer: &Printer<'_>,
    arrow: &internal::ArrowFunctionExpression<'_>,
) -> DocId {
    let d = printer.d();
    // The signature→`=>` gap rides inside the group, exactly as the plain arrow path
    // composes it. Without it this reassembly drops every comment there — the states
    // below emit `" =>"` themselves, so no other emitter can reach the gap.
    let sig = printer.append_pre_arrow_comments(arrow, printer.build_arrow_signature_doc(arrow));
    // ⚠️ The two arms are NOT interchangeable, and a green fixture suite does not say they
    // are: `remove_lines` and `group` differ inside an outer FLAT `fits` walk, which no
    // fixture here reaches (the TODO records two such "equivalent" deletions that had to be
    // reversed). This keeps each arm's historical layout exactly — the fix above is the gap
    // claim, not the break policy.
    let sig = if arrow_has_return_or_type_params(arrow) {
        d.group(sig)
    } else {
        d.remove_lines(sig)
    };
    // Every call-argument state that reassembles an arrow from signature + body starts
    // here, and none of them route the arrow through `build_expression_doc` — so this is
    // the only place its owned leading comment can be claimed. An owned comment nothing
    // prints is a *dropped* comment (`f(/** @param {any} n */ (n) => g(n))`), so the
    // claim must live on the same seam the reassembly does. See `comments/owned.rs`.
    printer.prepend_owned_leading_comment_at(arrow.span.start, sig)
}

/// Prepend any comments between arrow `=>` and body expression to `body_doc`.
///
/// When call argument paths build `sig_doc` and `body_doc` separately
/// (for break states like `(sig =>\n  body,\n)`), comments between `=>`
/// and the body are not part of either doc. This finds them and prepends
/// to `body_doc`, returning it unchanged if none exist.
pub(crate) fn prepend_arrow_body_comments(
    printer: &Printer<'_>,
    arrow: &internal::ArrowFunctionExpression<'_>,
    body_start: u32,
    body_doc: DocId,
) -> DocId {
    let arrow_end = arrow_token_end(arrow);

    // Prepend inline comments between `=>` and body. Glued: a single-line block
    // hugged to `=>` stays with the body across a source newline, matching the main
    // arrow-body path (`has_own_line_post_arrow_comment`) and prettier.
    if let Some(lc) = printer.build_rhs_comments_glued_opt(arrow_end, body_start) {
        printer.d().concat(&[lc, body_doc])
    } else {
        body_doc
    }
}

/// Break style for call expression wrapping
pub(super) enum CallBreakStyle {
    /// Soft breaks (can collapse to single line if it fits)
    Soft,
    /// Hard breaks (always multiline)
    Hard,
}

/// Wrap arguments in a call expression: `callee(args)`
///
/// With `Soft` breaks: `callee(args)` can collapse to a single line if it fits
/// With `Hard` breaks: Always uses multiline layout `callee(\n\targs,\n)`
///
/// IMPORTANT: The group only wraps the arguments, NOT the callee. This ensures
/// that if the callee contains hardlines (e.g., multiline array), they don't
/// force the arguments to break. The args make their own flat/break decision.
///
/// No trailing comma is emitted (trailingComma: 'none').
#[inline]
fn wrap_call(d: &DocArena, callee: DocId, args: DocId, style: CallBreakStyle) -> DocId {
    match style {
        CallBreakStyle::Soft => d.concat(&[
            callee,
            d.group(d.concat(&[
                d.text("("),
                d.indent_softline(args),
                d.softline(),
                d.text(")"),
            ])),
        ]),
        CallBreakStyle::Hard => d.concat(&[
            callee,
            d.text("("),
            d.indent_hardline(args),
            d.hardline(),
            d.text(")"),
        ]),
    }
}

/// Wrap arguments in a groupable call expression: `callee(args)`
/// Uses soft breaks so the call can collapse to a single line if it fits
#[inline]
pub(crate) fn wrap_call_with_soft_breaks(d: &DocArena, callee: DocId, args: DocId) -> DocId {
    wrap_call(d, callee, args, CallBreakStyle::Soft)
}

/// Wrap arguments in an expanded call expression: `callee(\n\targs,\n)`
/// Uses hard breaks to force multi-line layout.
///
/// Private: every caller outside this module goes through
/// [`wrap_call_with_hard_breaks_paren_line`], since a `(`-line comment run is always a
/// possibility there and dropping it on the floor is content loss.
#[inline]
fn wrap_call_with_hard_breaks(d: &DocArena, callee: DocId, args: DocId) -> DocId {
    wrap_call(d, callee, args, CallBreakStyle::Hard)
}

/// [`wrap_call_with_hard_breaks`] with the `(`→first-argument gap's same-line comment
/// run injected on the `(` line: `callee( // c⏎\targs⏎)`.
///
/// The counterpart to [`emit_first_arg_leading_comments`]'s `paren_line` output, and the
/// reason that run obliges a hard break: it ends in a `//` whose `line_suffix` needs a
/// following break to flush against, and any argument printed onto that line would be
/// swallowed by the comment. An empty run is just [`wrap_call_with_hard_breaks`].
#[inline]
pub(crate) fn wrap_call_with_hard_breaks_paren_line(
    d: &DocArena,
    callee: DocId,
    paren_line: &[DocId],
    args: DocId,
) -> DocId {
    if paren_line.is_empty() {
        return wrap_call_with_hard_breaks(d, callee, args);
    }
    d.concat(&[
        callee,
        d.text("("),
        d.concat(paren_line),
        d.indent_hardline(args),
        d.hardline(),
        d.text(")"),
    ])
}

/// Wrap arguments with a `will_break` guard: if any arg contains hardlines
/// (e.g., multi-line arrow bodies, block functions), force the group to break
/// so args expand onto separate lines. Otherwise use soft breaks.
///
/// Matches Prettier's `group(contents, { shouldBreak: printedArguments.some(willBreak) })`.
#[inline]
pub(crate) fn wrap_call_with_will_break_guard(d: &DocArena, callee: DocId, args: DocId) -> DocId {
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
pub(super) fn arg_needs_soft_wrap(arg: &internal::Expression<'_>) -> bool {
    matches!(
        arg,
        internal::Expression::CallExpression(_)
            | internal::Expression::MemberExpression(_)
            | internal::Expression::NewExpression(_)
            | internal::Expression::Identifier(_)
            | internal::Expression::ThisExpression(_)
            | internal::Expression::ConditionalExpression(_)
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
/// Only block-like expressions (objects, arrays, functions, classes) hug naturally,
/// including when wrapped in TS type assertions (`{...} as T`, `[...] satisfies T`).
/// Arrow functions are classified by their body type.
pub(super) fn classify_chain_arg(arg: &internal::Expression<'_>) -> ChainArgKind {
    match arg {
        // Block-like expressions hug the call parens naturally
        internal::Expression::ObjectExpression(_)
        | internal::Expression::ArrayExpression(_)
        | internal::Expression::FunctionExpression(_)
        | internal::Expression::ClassExpression(_) => ChainArgKind::HugsNaturally,
        // TS cast wrappers: classify based on the inner expression
        // e.g., `{...} as any` hugs, `longExpr as T` soft-wraps. Mirrors prettier's
        // couldExpandArg, which looks through `as`/`satisfies`/`<T>` but NOT a
        // non-null assertion, so `{...}!` / `[...]!` soft-wraps rather than hugging.
        internal::Expression::TSAsExpression(e) => classify_chain_arg(e.expression),
        internal::Expression::TSSatisfiesExpression(e) => classify_chain_arg(e.expression),
        internal::Expression::TSTypeAssertion(e) => classify_chain_arg(e.expression),
        // Arrow functions: prettier's couldExpandArg keys on the body type and
        // looks through the return-type annotation, so arrows are classified by
        // their body regardless of any return type.
        internal::Expression::ArrowFunctionExpression(arrow) => classify_arrow_body(arrow),
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
pub(crate) fn arrow_has_return_or_type_params(
    arrow: &internal::ArrowFunctionExpression<'_>,
) -> bool {
    arrow.return_type.is_some() || arrow.type_parameters.is_some()
}

/// Classify how an arrow function body should be formatted in chain context.
fn classify_arrow_body(arrow: &internal::ArrowFunctionExpression<'_>) -> ChainArgKind {
    match &arrow.body {
        internal::ArrowFunctionBody::BlockStatement(_) => ChainArgKind::HugsNaturally,
        internal::ArrowFunctionBody::Expression(expr) => classify_expression_body(expr),
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
pub(crate) fn could_expand_arrow_chain(arrow: &internal::ArrowFunctionExpression<'_>) -> bool {
    match &arrow.body {
        internal::ArrowFunctionBody::BlockStatement(_) => true,
        internal::ArrowFunctionBody::Expression(expr) => match &**expr {
            internal::Expression::ObjectExpression(_)
            | internal::Expression::ArrayExpression(_) => true,
            internal::Expression::ArrowFunctionExpression(inner) => could_expand_arrow_chain(inner),
            _ => false,
        },
    }
}

/// Classify how an expression body should be formatted.
///
/// Note: for arrows without TSTypeReference return types, object/array bodies
/// are caught earlier by the conditional_group path in chain_args.rs. This
/// HugsNaturally classification is primarily reached for arrows with
/// TSTypeReference returns (which bypass that path) and nested arrow chains.
fn classify_expression_body(expr: &internal::Expression<'_>) -> ChainArgKind {
    match expr {
        // Objects and arrays hug naturally (reached mainly for typed-return arrows)
        internal::Expression::ObjectExpression(_) | internal::Expression::ArrayExpression(_) => {
            ChainArgKind::HugsNaturally
        }
        // Ternaries hug but need trailing comma wrapper
        internal::Expression::ConditionalExpression(_) => ChainArgKind::NeedsWrapper,
        // Nested arrows inherit their body's classification
        internal::Expression::ArrowFunctionExpression(inner) => classify_arrow_body(inner),
        // Everything else needs soft wrap
        _ => ChainArgKind::NeedsSoftWrap,
    }
}

/// Wrap arguments with soft breaks (no callee, just prefix like "(" or "?.(")
///
/// Used in chain context where the callee is handled separately.
/// Structure: `prefix + softline + args + softline + ")"` (no trailing comma).
#[inline]
pub(super) fn wrap_args_with_soft_breaks(d: &DocArena, prefix: &'static str, args: DocId) -> DocId {
    d.group(d.concat(&[
        d.text(prefix),
        d.indent_softline(args),
        d.softline(),
        d.text(")"),
    ]))
}

/// Wrap a single huggable argument - hugs opening paren and breaks the closing
/// paren onto its own line when the content breaks internally.
///
/// Used for expressions with natural break points (objects, arrays, ternaries)
/// that should hug the opening paren. Under tsv's hardcoded `trailingComma: 'none'`
/// no trailing comma is added; the close still drops to its own line when broken.
/// Structure: `prefix + arg + softline + ")"`
#[inline]
pub(super) fn wrap_huggable_arg(d: &DocArena, prefix: &'static str, arg: DocId) -> DocId {
    d.group(d.concat(&[d.text(prefix), arg, d.softline(), d.text(")")]))
}

/// Build an arrow's expression body the same way the whole arrow's own body build does
/// (`build_arrow_doc_wrapping` clears `arrow_chain_context` before building the body — a
/// nested curried arrow in the body must not inherit the outer chain context), so the
/// pre-built DocId is byte-identical to what the arrow would build.
fn build_arrow_body_like_arrow(
    printer: &Printer<'_>,
    body_expr: &internal::Expression<'_>,
) -> DocId {
    let prev = printer.arrow_chain_context.replace(ArrowChainContext::None);
    let doc = printer.build_expression_doc(body_expr);
    printer.arrow_chain_context.set(prev);
    doc
}

/// Pre-build an expand-last-arg arrow's **break-body-state** body **once** so the whole-arrow
/// argument doc and the break-body state can share it, keeping the doc-node count linear
/// instead of O(2^depth) (see the `arrow_body_inject` field on `Printer`).
///
/// Returns `(body-expr span start, body DocId)` when `last_arg` is an arrow whose expression
/// body routes through `build_break_body_state` — a **call** (through a trailing `!`) or a
/// **conditional** (ternary). The caller injects it via `Printer::inject_arrow_body` before
/// `build_args_split_last`; the whole arrow reuses it (a call body via `build_arrow_body_doc`,
/// a conditional body via the conditional arm of `build_arrow_expression_body`), and the
/// break-body state reuses the same DocId. Leftmost-object conditionals are excluded — the
/// whole arrow routes those through `build_arrow_body_doc`'s object-parens arm, not the
/// conditional arm, so the injected raw wouldn't match. Returns `None` (unchanged behavior)
/// when the last arg isn't such an arrow, or when the call carries any comment (the commented
/// last-arg path composes the body differently; the exponential shapes are comment-free).
pub(crate) fn prebuild_expand_last_break_body(
    printer: &Printer<'_>,
    last_arg: Option<&internal::Expression<'_>>,
    call_has_comments: bool,
) -> Option<(u32, DocId)> {
    if call_has_comments {
        return None;
    }
    if let Some(internal::Expression::ArrowFunctionExpression(arrow)) = last_arg
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
        && (arrow_body_is_call_through_non_null(body_expr)
            || (matches!(&**body_expr, internal::Expression::ConditionalExpression(_))
                && !has_leftmost_object_expression(body_expr)))
    {
        let body_doc = build_arrow_body_like_arrow(printer, body_expr);
        return Some((body_expr.span().start, body_doc));
    }
    None
}

/// Pre-build an expand-last-arg arrow's **object/array** body once (the sibling of
/// `prebuild_expand_last_break_body` for the object/array hug path). Returns
/// `(body span, inject doc, hug body doc)`:
/// - `inject doc` is what the whole arrow's `build_arrow_body_doc` produces for this body —
///   `d.parens(obj)` for an object (the leftmost-object parens: `build_arrow_body_doc` wraps
///   the whole-body object in `d.parens` exactly as this does), or the bare array doc for an
///   array — and is injected so the whole-arrow arg doc reuses it;
/// - `hug body doc` is `d.parens(body)`, matching the previous inline
///   `d.parens(build_expression_doc(body))` the hug state wraps in `group_break`.
///
/// Both share the single body build, so `f(lead, x => ({{ k: f(lead, y => …) }}))` stays
/// linear. Returns `None` (unchanged) when the last arg isn't an object/array-body arrow or
/// the call carries comments.
pub(crate) fn prebuild_expand_last_obj_array_body(
    printer: &Printer<'_>,
    last_arg: Option<&internal::Expression<'_>>,
    call_has_comments: bool,
) -> Option<(u32, DocId, DocId)> {
    if call_has_comments {
        return None;
    }
    let d = printer.d();
    if let Some(internal::Expression::ArrowFunctionExpression(arrow)) = last_arg
        && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
    {
        match &**body_expr {
            internal::Expression::ObjectExpression(_) => {
                let raw = build_arrow_body_like_arrow(printer, body_expr);
                let parens = d.parens(raw);
                Some((body_expr.span().start, parens, parens))
            }
            internal::Expression::ArrayExpression(_) => {
                let raw = build_arrow_body_like_arrow(printer, body_expr);
                Some((body_expr.span().start, raw, d.parens(raw)))
            }
            _ => None,
        }
    } else {
        None
    }
}

/// Build argument docs split into head parts (with commas), last arg, and broken form
///
/// Used for patterns that keep short args inline with the last arg.
/// Returns (head_parts, last_arg_doc, all_args_broken) where:
/// - head_parts: all but last arg with ", " separators (includes inline block comments)
/// - last_arg_doc: the last argument doc
/// - all_args_broken: all args joined with comma_line() for fallback (includes inline block comments)
pub(crate) fn build_args_split_last(
    arguments: &[internal::Expression<'_>],
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
        .map(|(i, _)| ArgItem::ArgContext.build(printer, paren_open, arguments, i))
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

/// Build the "expand all args" doc structure: `callee(\n\tall_args,\n)`
///
/// Used when all arguments must be expanded to separate lines.
/// Wraps the args in `group_break` to force break mode, matching Prettier's
/// `allArgsBrokenOut()` which uses `group(contents, { shouldBreak: true })`.
/// Without the group, `line()` nodes would inherit the parent's mode and
/// render as spaces when the parent is in flat mode.
#[inline]
pub(crate) fn build_expand_all_args(d: &DocArena, callee: DocId, all_args_broken: DocId) -> DocId {
    d.concat(&[
        callee,
        d.group_break(d.concat(&[
            d.text("("),
            d.indent(d.concat(&[d.line(), all_args_broken])),
            d.line(),
            d.text(")"),
        ])),
    ])
}

/// Build the "expand all args" doc for chain context: `prefix\n\tall_args,\n)`
///
/// Like `build_expand_all_args` but takes a string prefix (e.g., `"("` or `"?.("`)
/// instead of a callee DocId, since chain contexts handle the callee separately.
///
/// Wraps in `group_break` to match Prettier's `allArgsBrokenOut()` which uses
/// `group({shouldBreak: true})`. This ensures the `line()` docs render as newlines
/// even when the parent context evaluates them in Flat mode (e.g., short chains
/// inside assignment layout's `fits()` check).
#[inline]
pub(super) fn build_chain_expand_all_args(
    d: &DocArena,
    prefix: &'static str,
    all_args_broken: DocId,
) -> DocId {
    d.group_break(d.concat(&[
        d.text(prefix),
        d.indent(d.concat(&[d.line(), all_args_broken])),
        d.line(),
        d.text(")"),
    ]))
}

/// Build the "inline" doc structure: `callee(head_parts + last_arg)`
///
/// Used as the first state in conditional groups where we try to fit everything inline.
#[inline]
pub(crate) fn build_inline_args(
    d: &DocArena,
    callee: DocId,
    head_parts: &[DocId],
    last_arg_doc: DocId,
) -> DocId {
    d.concat(&[
        callee,
        d.text("("),
        d.concat(head_parts),
        last_arg_doc,
        d.text(")"),
    ])
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
pub(crate) fn try_hook_deps_args_doc(
    printer: &Printer<'_>,
    args: &[internal::Expression<'_>],
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
    args: &[internal::Expression<'_>],
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
        parts.push(ArgItem::ArgContext.build(printer, paren_open, args, i));
    }
    parts.push(d.text(")"));
    d.concat(&parts)
}

/// Does the last argument's arrow write an own-line comment between its `=>` and its body?
///
/// Prettier decides this inside the arrow printer, above every argument layout:
/// `shouldPutBodyOnSameLine` opens with `!hasLeadingOwnLineComment(text, functionBody)`, so
/// such a comment drops the body below `=>` — and the branch that takes it appends
/// `trailingComma + trailingSpace`, where `trailingSpace` is a **softline under
/// `expandLastArg`** (`print/arrow-function.js`, `printArrowFunctionBody`). That softline is
/// the only thing that lands the call's `)` on its own line, and it is appended for **every
/// body kind** — block, object, array, arrow chain alike — which is why this question is
/// asked of the gap and not of the body's type.
///
/// **The chain walks to the TERMINAL arrow.** `expandLastArg` turns off
/// `shouldPrintAsChain` (`!args.expandLastArg && body is Arrow`), so a curried argument is
/// printed as nested arrows and the softline is appended by the innermost one — the gap that
/// carries the comment in `fn(() => () =>⏎\t// c⏎\t({ a: 1 }))`.
pub(crate) fn last_arg_has_own_line_post_arrow_comment(
    printer: &Printer<'_>,
    last_arg: &internal::Expression<'_>,
) -> bool {
    let internal::Expression::ArrowFunctionExpression(arrow) = last_arg else {
        return false;
    };
    let mut arrow = arrow;
    loop {
        let body_start = match &arrow.body {
            internal::ArrowFunctionBody::BlockStatement(block) => block.span.start,
            internal::ArrowFunctionBody::Expression(expr) => expr.span().start,
        };
        if let internal::ArrowFunctionBody::Expression(expr) = &arrow.body
            && let internal::Expression::ArrowFunctionExpression(inner) = &**expr
        {
            arrow = inner;
            continue;
        }
        return printer.has_own_line_post_arrow_comment(arrow_token_end(arrow), body_start);
    }
}

/// Assemble the single `expandLastArg` state
/// [`last_arg_has_own_line_post_arrow_comment`] selects: the head arguments stay inline, the
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
/// this kind of arm before. `head_parts` is empty at the single-argument sites; `prefix`
/// carries `(` for a call or `new` (whose callee sits outside the returned group) and the
/// chain's own `(` / `?.(`.
pub(crate) fn build_own_line_post_arrow_state(
    d: &DocArena,
    prefix: DocId,
    head_parts: &[DocId],
    last_arg_doc: DocId,
) -> DocId {
    d.group_break(d.concat(&[
        prefix,
        d.concat(head_parts),
        d.group_break(last_arg_doc),
        d.softline(),
        d.text(")"),
    ]))
}

/// Build a conditional group that tries inline first, then expands all args.
///
/// This is Prettier's "expand last arg" pattern for arrays/objects when there are
/// 2+ arguments and the last two are different types.
///
/// State 1: Try all args inline
/// State 2: Expand all args to separate lines
///
/// Note: Arrays/objects with the nested heuristic use group_break() (shouldBreak on the group)
/// rather than break_parent(). This keeps the break local to the array/object group,
/// allowing state 1 to work when head args fit inline and only the last arg needs to break.
pub(crate) fn build_inline_or_expand_all(
    d: &DocArena,
    callee: DocId,
    head_parts: &[DocId],
    last_arg_doc: DocId,
    all_args_broken: DocId,
) -> DocId {
    d.conditional_group(&[
        build_inline_args(d, callee, head_parts, last_arg_doc),
        build_expand_all_args(d, callee, all_args_broken),
    ])
}

/// Build the three-state conditional group for a **different-type** expand-last argument:
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
/// bracket fit. A last argument carrying its own forced break — a hardline from an interior
/// comment, a source-multiline `group_break` — simply falls out of state 0 onto the hug, so
/// no pre-check screens for one; the *same*-type path, which has no hug state, needs that
/// check and does it at its own call site.
///
/// Shared by the plain-call and `new` argument printers, which answer this identically;
/// the member-chain twin (`chain_args.rs`) builds the same ladder over a `&'static str`
/// opener rather than a callee doc, so it keeps its own spelling — see
/// [`build_chain_expand_all_args`].
pub(crate) fn build_inline_hug_or_expand_all(
    d: &DocArena,
    callee: DocId,
    head_parts: &[DocId],
    last_arg_doc: DocId,
    all_args_broken: DocId,
) -> DocId {
    let state_inline = build_inline_args(d, callee, head_parts, last_arg_doc);
    let state_hug = d.concat(&[
        callee,
        d.text("("),
        d.concat(head_parts),
        d.group_break(last_arg_doc),
        d.text(")"),
    ]);
    let state_expand_all = build_expand_all_args(d, callee, all_args_broken);
    d.conditional_group(&[state_inline, state_hug, state_expand_all])
}

/// Prettier's expand-FIRST layout: a block-function first argument hugs and the tail stays
/// inline past its `}` (`setTimeout(() => { tick(); }, 100)`).
///
/// The layout twin of [`should_expand_first_arg`], which gates it. The **predicate** was
/// already shared and the layout was not, which is precisely how one copy gets a fix and the
/// other doesn't — the `new` spelling reached neither the argument-gap seam nor the last
/// argument's gap and dropped a comment in each, while the chain's asked its break question
/// of the argument doc alone. One body now, for the plain call and `new` alike; the chain
/// keeps its own spelling (a `&'static str` opener and a hardline-built broken form rather
/// than [`build_call_args_expanded`]), as it does for the expand-last ladder above.
///
/// The tail is written as **everything after the first argument** — prettier's
/// `printedArguments.slice(1)` — though `should_expand_first_arg` admits exactly two, so it
/// is one argument plus the two gaps around it: the inter-argument seam
/// ([`build_arg_gap_docs`]) and the last argument's gap to the `)`
/// ([`emit_last_arg_trailing_comments`]).
///
/// Returns the fully-expanded form when the tail will break — prettier's
/// `if (tailArgs.some(willBreak)) return allArgsBrokenOut()`. An inline tail cannot carry an
/// argument that breaks: without it the first argument hugs and a broken tail trails it
/// (`new A(() => {⏎…⏎}, fn(⏎// c⏎b))`), a form prettier never emits. `tailArgs` is the printed
/// argument **plus the comments printed with it**, which is why the question is asked of the
/// whole `tail_parts` and not of the argument's own doc — ownership decides what that doc
/// carries (`docs/comments.md` hazard 2).
pub(super) fn build_expand_first_arg_doc(
    printer: &Printer<'_>,
    callee: DocId,
    arguments: &[internal::Expression<'_>],
    paren_open: u32,
    call_end: u32,
) -> DocId {
    let d = printer.d();
    let first_arg_doc = printer.build_expression_doc(&arguments[0]);

    let mut tail_parts = DocBuf::new();
    let mut prev_end = arguments[0].span().end;
    for arg in arguments.iter().skip(1) {
        let (through_comma, leading) = build_arg_gap_docs(printer, prev_end, arg.span().start);
        tail_parts.extend(through_comma);
        tail_parts.push(d.text(" "));
        tail_parts.extend(leading);
        tail_parts.push(printer.build_expression_doc(arg));
        prev_end = arg.span().end;
    }
    if let Some(last_arg) = arguments.last() {
        emit_last_arg_trailing_comments(printer, &mut tail_parts, last_arg, call_end);
    }

    if tail_parts.iter().any(|&id| d.will_break(id)) {
        return build_call_args_expanded(
            printer,
            callee,
            arguments,
            paren_open,
            call_end,
            ArgItem::Plain,
        );
    }

    // The first argument can expand internally; the tail stays on its closing line.
    d.concat(&[
        callee,
        d.text("("),
        first_arg_doc,
        d.concat(&tail_parts),
        d.text(")"),
    ])
}

/// Check if the last two arguments have the same outer AST type.
/// Prettier disables expand-last-arg hug state when `penultimateArg.type === lastArg.type`
/// (call-arguments.js:258). This covers both arrays, both objects, and also both TSAsExpression,
/// both TSSatisfiesExpression, etc.
pub(crate) fn last_two_args_same_type(args: &[internal::Expression<'_>]) -> bool {
    let last = &args[args.len() - 1];
    let penultimate = &args[args.len() - 2];
    std::mem::discriminant(last) == std::mem::discriminant(penultimate)
}

/// Build the "break body" state for expand-last-arg with an expression arrow.
///
/// Layout: `prefix + head_parts + sig => \n  body,\n)`
///
/// `prefix_doc` should include the callee and opening paren (e.g., `callee + "("` or `"("`).
#[inline]
pub(crate) fn build_break_body_state(
    d: &DocArena,
    prefix_doc: DocId,
    head_parts: &[DocId],
    sig_doc: DocId,
    body_doc: DocId,
) -> DocId {
    d.concat(&[
        prefix_doc,
        d.concat(head_parts),
        sig_doc,
        d.text(" =>"),
        d.indent_hardline(body_doc),
        d.hardline(),
        d.text(")"),
    ])
}

/// Build doc for arrow functions with call expression bodies.
///
/// Used when an arrow's body is a call expression (simple or with complex args).
///
/// When the body has hardlines (e.g., comments forcing multi-line args), uses a
/// group-based approach with softline to separate the outer closing paren. Without
/// this, the conditional_group's flat state would be selected by `fits()` (the first
/// line fits) but merge the inner and outer closing parens as `))`.
///
/// When the body fits on one line, creates a conditional group with two states:
/// - State 0 (flat): `callee((params) => body)`
/// - State 1 (break): `callee((params) =>\n  body,\n)`
///
/// Both states compose the same `sig_doc`/`body_doc` (the body is built ONCE by the
/// caller) — the flat state is `sig => body`, so the caller does NOT build a separate
/// whole-arrow doc. Building the whole arrow *and* the body was a redundant double-build
/// that recursed into itself for a call-bodied arrow whose body is another such call
/// (`a(x => b(y => …))`), making the doc-node count O(2^depth). See the build-fanout audit.
///
/// Parameters:
/// - `callee`: The call expression's callee doc
/// - `sig_doc`: The arrow's signature doc (`(params)`)
/// - `body_doc`: The arrow body expression doc
#[inline]
pub(crate) fn build_arrow_call_body_states(
    d: &DocArena,
    callee: DocId,
    sig_doc: DocId,
    body_doc: DocId,
) -> DocId {
    // Body has hardlines (comments, nested callbacks): softline separates outer )
    if d.will_break(body_doc) {
        return d.group(d.concat(&[
            callee,
            d.text("("),
            d.concat(&[sig_doc, d.text(" =>")]),
            d.group(d.indent_line(body_doc)),
            d.softline(),
            d.text(")"),
        ]));
    }

    d.conditional_group(&[
        // Flat: callee((params) => body)
        d.concat(&[
            callee,
            d.text("("),
            sig_doc,
            d.text(" => "),
            body_doc,
            d.text(")"),
        ]),
        // Break: callee((params) =>\n  body,\n)
        d.concat(&[
            callee,
            d.text("("),
            sig_doc,
            d.text(" =>"),
            d.indent_hardline(body_doc),
            d.hardline(),
            d.text(")"),
        ]),
    ])
}

/// Build argument docs joined with breaks, preserving inter-argument comments.
///
/// Like `join_doc(args, separator)` but handles leading/trailing comments
/// between arguments. Used by expansion paths (all-arrows, function composition)
/// that would otherwise lose comments with simple `join_doc`.
///
/// `join` picks the separator style and says whether the `(`→first-argument gap is this
/// builder's to emit; `item` picks the per-argument builder. `paren_close` bounds the
/// LAST argument's trailing gap, and `paren_line` receives the `(`-line comment run (see
/// [`emit_first_arg_leading_comments`]) — a non-empty run obliges the caller to wrap with
/// [`wrap_call_with_hard_breaks_paren_line`].
pub(crate) fn build_args_joined_with_comments(
    printer: &Printer<'_>,
    arguments: &[internal::Expression<'_>],
    paren_open: u32,
    paren_close: u32,
    join: ArgsJoin,
    item: ArgItem,
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

        parts.push(item.build(printer, paren_open, arguments, i));

        if i < arguments.len() - 1 {
            let next_arg_start = arguments[i + 1].span().start;

            if printer.inter_arg_gap_has_comments(arg, next_arg_start) {
                let gap = printer.open_inter_arg_gap(&mut parts, arg, next_arg_start);
                // The gap's `forces_expansion` obligation is the callers': the soft-join
                // callers are unreachable when any spread interior forces expansion —
                // their earlier trailing-comment arms, keyed on
                // `any_spread_paren_comment_forces_expansion`, return first — and every
                // other join is hardline.
                debug_assert!(!gap.forces_expansion || use_hardline);
                // A commented gap's blank is comment-aware (routed, so a comment's own
                // newlines don't read as one) and rides here rather than at the next
                // iteration's top, whose guard skips a gap that holds a comment.
                if join.preserves_blanks() && gap.comments.has_blank_line_in_gap(printer) {
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
            // triggers (multiline content, function composition, the expand-first
            // fallback) that preempt the callers' own comment-aware paths, so nothing
            // else emits it and the loss is total.
            emit_last_arg_trailing_comments(printer, &mut parts, arg, paren_close);
        }
    }

    d.concat(&parts)
}

/// The forced-expansion argument layout the call and `new` hardline arms share (multiline
/// content, function composition, all-arrows, the expand-first fallback): one argument per
/// line with the gap comments preserved, wrapped in the call's hard-broken parens.
pub(crate) fn build_call_args_expanded(
    printer: &Printer<'_>,
    callee: DocId,
    arguments: &[internal::Expression<'_>],
    paren_open: u32,
    paren_close: u32,
    item: ArgItem,
) -> DocId {
    let mut paren_line = DocBuf::new();
    let args = build_args_joined_with_comments(
        printer,
        arguments,
        paren_open,
        paren_close,
        ArgsJoin::Hardline,
        item,
        &mut paren_line,
    );
    wrap_call_with_hard_breaks_paren_line(printer.d(), callee, &paren_line, args)
}

/// How [`build_args_joined_with_comments`] separates arguments, and who owns the
/// `(`→first-argument gap.
#[derive(Clone, Copy)]
pub(crate) enum ArgsJoin {
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

/// Which ordinary builder a joined-argument layout uses for an argument Rule A hasn't
/// frozen. An enum rather than a closure parameter: the family wants exactly these two,
/// and a closure here trips the HRTB lifetime check at every call site.
#[derive(Clone, Copy)]
pub(crate) enum ArgItem {
    /// `build_arg_expression_doc` — argument context, so a binary/logical chain (or
    /// conditional) keeps its continuation indent and an assignment gets clarity parens.
    ArgContext,
    /// `build_expression_doc` — the plain builder, for the arms that print their
    /// arguments without that context (all-arrows, expand-first's broken-out fallback).
    Plain,
}

impl ArgItem {
    /// The doc for argument `i`: the verbatim slice when an own-line format-ignore
    /// directive in its gap freezes it (Rule A), else this variant's ordinary builder.
    /// The frozen slice is the same whichever builder the variant names, so the dispatch
    /// lives here rather than at each layout arm.
    ///
    /// Every layout that reaches here is a **broken-out** one — nothing in this family
    /// hugs an argument onto the callee's line — so the build goes through
    /// [`build_printed_argument_doc`]; a curried chain that skipped the progressive layout
    /// here would keep its heads welded to the first one. Rule A still wins inside it:
    /// a frozen argument is a verbatim source slice, which no layout context can reach,
    /// so the wrapper is inert on that arm.
    pub(crate) fn build(
        self,
        printer: &Printer<'_>,
        paren_open: u32,
        args: &[internal::Expression<'_>],
        i: usize,
    ) -> DocId {
        build_printed_argument_doc(printer, &args[i], || match self {
            Self::ArgContext => printer.build_arg_item_doc(paren_open, args, i),
            Self::Plain => printer.args_frozen_span(paren_open, args, i).map_or_else(
                || printer.build_expression_doc(&args[i]),
                |frozen| printer.build_frozen_arg_doc(&args[i], frozen),
            ),
        })
    }
}

/// Check if a call/new should use the "expand first arg" pattern.
///
/// This matches prettier's behavior for calls like `setTimeout(() => {...}, 100)`:
/// - First arg is function/arrow with block body
/// - Remaining args are "hopefully short" (simple values)
/// - Result: first arg expands, tail args stay inline after closing `}`
pub(super) fn should_expand_first_arg(
    printer: &Printer<'_>,
    args: &[internal::Expression<'_>],
) -> bool {
    // Need exactly 2 args (first is function, second is short)
    if args.len() != 2 {
        return false;
    }

    // First arg must be a function with block body
    if !is_block_function(&args[0]) {
        return false;
    }

    // Prettier's couldExpandArg returns true for a bare object/array with a leading
    // comment (`hasComment(node)`), so `!couldExpandArg(secondArg)` is false and it
    // breaks all args. tsv matches by blocking expand-first here. A cast-wrapped
    // collection (`/* c */ {} as T`) is deliberately NOT blocked — prettier's comment
    // attaches to the cast, `couldExpandArg` stays false, and it expand-firsts; the
    // expand-first path carries the inter-arg leading comment through the shared
    // argument-gap seam (`build_arg_gap_docs`).
    //
    // A JSDoc cast never reaches this gate, and must not be added to it: prettier keeps
    // the cast's parens, so its `couldExpandArg` sees an opaque paren node rather than the
    // collection inside, and it expands-first even for a non-empty one. The transparency a
    // cast does get is in `is_hopefully_short_arg`, not here — pinned by
    // `calls/expand_first_jsdoc_cast_second_arg`.
    //
    // **on page** (both probes): prettier's `couldExpandArg` asks `hasComment(node)`, a
    // pure layout question — an owned annotation is on the page and blocks the hug just
    // like any other comment. Kept in lockstep with the twin guard in
    // `chain_args::should_expand_first_arg_for_chain`.
    if matches!(
        &args[1],
        internal::Expression::ObjectExpression(_) | internal::Expression::ArrayExpression(_)
    ) && printer.has_comments_on_page_between(args[0].span().end, args[1].span().start)
    {
        return false;
    }

    // Second arg must be short/simple
    is_short_second_arg_for_expand_first(&args[1], |start, end| {
        printer.has_comments_on_page_between(start, end)
    })
}

/// Append type arguments (`fn<T>`, `new Foo<K, V>`) to a callee doc, preserving
/// comments in the gap between the callee and `<`.
///
/// Uses `build_name_to_type_params_comments` for safe line comment handling.
pub(super) fn append_type_args_with_gap_comments(
    printer: &Printer<'_>,
    callee: DocId,
    callee_end: u32,
    type_arguments: Option<&internal::TSTypeParameterInstantiation<'_>>,
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
    let prefix = if optional { "?.(" } else { "(" };
    let mut parts: DocBuf = smallvec![callee];
    push_empty_args(printer, &mut parts, after_type_args, paren_close, prefix);
    printer.d().concat(&parts)
}

/// Single multiline-template argument on the same line as `(` — hug it,
/// keeping trailing comments as a line suffix.
///
/// Prettier has source-position-dependent behavior (isTemplateOnItsOwnLine):
/// - Hugged: `` fn(`line1\nline2`) `` → keep inline (no groups)
/// - Expanded: template on its own line → returns None so the caller falls
///   through to the has_multiline_content path (hardline expansion).
pub(super) fn try_hug_multiline_template_arg(
    printer: &Printer<'_>,
    callee: DocId,
    args: &[internal::Expression<'_>],
    paren_close: u32,
) -> Option<DocId> {
    if args.len() != 1 || !is_multiline_template_expression(&args[0]) {
        return None;
    }
    let template_start = args[0].span().start;
    if has_newline_before_position(printer.source, template_start) {
        return None;
    }
    let d = printer.d();
    let arg_doc = printer.build_expression_doc(&args[0]);
    let mut parts: DocBuf = smallvec![callee, d.text("("), arg_doc, d.text(")")];
    if let Some(suffix) =
        printer.build_trailing_comments_line_suffix(args[0].span().end, paren_close)
    {
        parts.push(suffix);
    }
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
    args: &[internal::Expression<'_>],
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
        ArgItem::ArgContext,
        &mut paren_line,
    );
    wrap_call_with_hard_breaks_paren_line(printer.d(), callee, &paren_line, args_doc)
}
