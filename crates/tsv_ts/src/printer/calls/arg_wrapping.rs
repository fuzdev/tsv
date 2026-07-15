// Argument classification and wrapping utilities for call expressions
//
// Handles:
// - Argument classification for chain contexts
// - Call expression wrapping with soft/hard breaks
// - Building argument lists split into head/last patterns

use super::super::{
    ArrowChainContext, CommentSpacing, Printer, has_newline_before_position,
    is_curried_arrow_chain, is_multiline_template_expression,
};
use super::arg_comments::{
    emit_first_arg_leading_comments, find_comma_pos, has_blank_line_between_args,
    is_inline_block_after_comma, is_inline_block_before_comma, push_empty_args,
};
use super::arg_predicates::{
    arrow_body_is_call_through_non_null, is_block_function, is_short_second_arg_for_expand_first,
};
use crate::ast::internal;
use crate::printer::expressions::functions::has_leftmost_object_expression;
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};

/// Build an inline arrow function signature without break points.
///
/// Used when we want the signature to stay on one line (e.g., `(x) =>`).
/// Does NOT include the ` =>` - caller adds that.
/// Handles arrows with no type parameters and no return type; param-level type
/// annotations are fine (emitted via `build_function_parameter_doc`).
pub(crate) fn build_arrow_inline_signature(
    printer: &Printer<'_>,
    arrow: &internal::ArrowFunctionExpression<'_>,
) -> DocId {
    let d = printer.d();
    let mut sig_parts = DocBuf::new();
    if arrow.r#async {
        sig_parts.push(d.text("async "));
    }
    if arrow.params.is_empty() {
        sig_parts.push(
            printer
                .build_empty_params_with_comments_doc(arrow.params_start, arrow.body.span().start),
        );
    } else {
        sig_parts.push(d.text("("));
        sig_parts.push(
            d.join(
                arrow
                    .params
                    .iter()
                    .map(|p| printer.build_function_parameter_doc(p)),
                ", ",
            ),
        );
        sig_parts.push(d.text(")"));
    }
    d.concat(&sig_parts)
}

/// Build an arrow function signature doc, choosing inline or full based on type annotations.
///
/// Untyped arrows use the inline signature (no break points).
/// Typed arrows use the full signature wrapped in a group (can break internally).
/// Does NOT include the ` =>` - caller adds that.
pub(crate) fn build_arrow_sig_doc(
    printer: &Printer<'_>,
    arrow: &internal::ArrowFunctionExpression<'_>,
) -> DocId {
    let sig = if arrow_has_return_or_type_params(arrow) {
        printer.d().group(printer.build_arrow_signature_doc(arrow))
    } else {
        build_arrow_inline_signature(printer, arrow)
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
    let arrow_end = arrow.arrow_token + "=>".len() as u32;

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
            d.indent(d.concat(&[d.hardline(), args])),
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
/// Uses hard breaks to force multi-line layout
#[inline]
pub(crate) fn wrap_call_with_hard_breaks(d: &DocArena, callee: DocId, args: DocId) -> DocId {
    wrap_call(d, callee, args, CallBreakStyle::Hard)
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
/// These are the parts `build_arrow_inline_signature` can't render, so an arrow
/// carrying either needs the full grouped signature. A param-level type annotation
/// does NOT need it — the inline signature emits param types too (via
/// `build_function_parameter_doc`), so a params-only-typed arrow renders identically
/// either way.
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
    // progressive call-arg chain layout: set the context so the outermost chain
    // arrow flattens its heads (`should_use_arrow_chain_layout` still gates on
    // untyped / comment-free, and `skip_arrow_chain` keeps the expand-last-arg hug
    // states on the default path). Mirrors prettier's `isCallLikeExpression(parent)`
    // reaching `printArrowFunctionSignatures`.
    let arg_docs: DocBuf = arguments
        .iter()
        .map(|arg| {
            if is_curried_arrow_chain(arg) {
                printer
                    .build_with_arrow_chain_context(ArrowChainContext::CallArgOrBinaryish, || {
                        printer.build_arg_expression_doc(arg)
                    })
            } else {
                printer.build_arg_expression_doc(arg)
            }
        })
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

    for (i, doc) in arg_docs.iter().take(arg_docs.len() - 1).enumerate() {
        head_parts.push(*doc);

        // Only the `, ` separator is structural; the comma scan and the two inline
        // block-comment lookups are pure comment placement, so gate them.
        if has_comments {
            let arg_end = arguments[i].span().end;
            let next_arg_start = arguments[i + 1].span().start;
            let comma_pos = find_comma_pos(printer.source, arg_end, next_arg_start);

            // Add inline block comments around comma
            if let Some(cpos) = comma_pos {
                for comment in comments_to_emit_in_range(printer.comments, arg_end, next_arg_start)
                {
                    if is_inline_block_before_comma(
                        comment,
                        cpos,
                        printer.comment_line_breaks,
                        arg_end,
                    ) {
                        head_parts.push(d.text(" "));
                        head_parts.push(printer.build_comment_doc(comment));
                    }
                }
            }

            head_parts.push(d.text(", "));

            if let Some(cpos) = comma_pos {
                for comment in comments_to_emit_in_range(printer.comments, arg_end, next_arg_start)
                {
                    if is_inline_block_after_comma(
                        comment,
                        cpos,
                        printer.comment_line_breaks,
                        arg_end,
                    ) {
                        head_parts.push(printer.build_comment_doc(comment));
                        head_parts.push(d.text(" "));
                    }
                }
            }
        } else {
            head_parts.push(d.text(", "));
        }
    }
    let last_arg_doc = arg_docs[arg_docs.len() - 1];

    // Build all_args_broken with inline block comments (same comma-aware logic)
    let mut all_args_parts = DocBuf::new();
    if let Some(lc) = leading_comment_doc {
        all_args_parts.push(lc);
    }

    for (i, doc) in arg_docs.iter().enumerate() {
        if i > 0 {
            all_args_parts.push(d.comma_line());
        }
        all_args_parts.push(*doc);

        // Add trailing inline block comments (except after last arg). Pure comment
        // placement — gated on the whole-call comment flag.
        if has_comments && i < arguments.len() - 1 {
            let arg_end = arguments[i].span().end;
            let next_arg_start = arguments[i + 1].span().start;
            let comma_pos = find_comma_pos(printer.source, arg_end, next_arg_start);

            // Only add inline block comments that are BEFORE the comma
            if let Some(cpos) = comma_pos {
                for comment in comments_to_emit_in_range(printer.comments, arg_end, next_arg_start)
                {
                    if is_inline_block_before_comma(
                        comment,
                        cpos,
                        printer.comment_line_breaks,
                        arg_end,
                    ) {
                        all_args_parts.push(d.text(" "));
                        all_args_parts.push(printer.build_comment_doc(comment));
                    }
                }
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
        d.indent(d.concat(&[d.hardline(), body_doc])),
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
            d.indent(d.concat(&[d.hardline(), body_doc])),
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
/// When `use_hardline` is true, separators are hardlines (forced expansion).
/// When false, separators are soft lines (break only when the group breaks).
/// Trailing line comments always force a hardline regardless of this setting.
pub(crate) fn build_args_joined_with_comments(
    printer: &Printer<'_>,
    arguments: &[internal::Expression<'_>],
    paren_open: u32,
    use_hardline: bool,
    build_arg: impl Fn(&Printer<'_>, &internal::Expression<'_>) -> DocId,
) -> DocId {
    let d = printer.d();
    let mut parts = DocBuf::new();

    // Leading comments before first arg (e.g., `fn(/* c */ arg)`)
    let first_arg_start = arguments[0].span().start;
    emit_first_arg_leading_comments(printer, &mut parts, paren_open, first_arg_start);

    let no_comment_sep = if use_hardline {
        d.comma_hardline()
    } else {
        d.comma_line()
    };

    for (i, arg) in arguments.iter().enumerate() {
        parts.push(build_arg(printer, arg));

        if i < arguments.len() - 1 {
            let arg_end = arg.span().end;
            let next_arg_start = arguments[i + 1].span().start;

            if printer.has_comments_to_emit_between(arg_end, next_arg_start) {
                let pc = printer.open_inter_arg_gap(&mut parts, arg_end, next_arg_start);
                // A line comment runs to EOL → hard-break; otherwise honor the caller's style.
                parts.push(if pc.has_trailing_line() || use_hardline {
                    d.hardline()
                } else {
                    d.line()
                });
                // hugging after-comma + own-line comments lead the next arg (`C`).
                pc.emit_leading_comments_inline_aware(&mut parts, printer);
            } else {
                parts.push(no_comment_sep);
            }
        }
    }

    d.concat(&parts)
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
    // expand-first path carries the inter-arg leading comment via
    // `build_after_comma_leading_comments`.
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
/// comments from inside-paren comments, e.g. `fn<string> /* c */()`.
pub(super) fn build_empty_args_doc(
    printer: &Printer<'_>,
    callee: DocId,
    after_type_args: u32,
    paren_close: u32,
) -> DocId {
    let mut parts: DocBuf = smallvec![callee];
    push_empty_args(printer, &mut parts, after_type_args, paren_close, "(", "()");
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

/// Build the argument list doc for a call/new whose arguments have blank lines
/// between them (hardline expansion, preserving at most one blank line per gap).
///
/// Handles comments in the gaps; a gap without comments preserves its blank
/// line at the top of the next iteration. The caller wraps the result with
/// `wrap_call_with_hard_breaks`.
pub(super) fn build_args_with_blank_lines(
    printer: &Printer<'_>,
    args: &[internal::Expression<'_>],
) -> DocId {
    let d = printer.d();
    let mut arg_parts = DocBuf::new();
    for (i, arg) in args.iter().enumerate() {
        // Check for blank line before this arg (no-comment case only).
        // When comments exist, blank lines are handled in the separator
        // logic of the previous iteration.
        if i > 0 {
            let prev_end = args[i - 1].span().end;
            let curr_start = arg.span().start;
            // Nothing to emit in the gap, but a comment can still physically *be* there —
            // an owned annotation leading this argument. The blank-line scan counts raw
            // newlines, so it must stop at the comment: `[prev_end, comment_start)` excludes
            // the annotation's own newlines yet keeps an authored blank line *before* it.
            if !printer.has_comments_to_emit_between(prev_end, curr_start)
                && has_blank_line_between_args(
                    printer.source,
                    printer.layout_line_breaks,
                    prev_end,
                    printer.blank_scan_end(prev_end, curr_start),
                )
            {
                arg_parts.push(d.literalline());
                arg_parts.push(d.hardline());
            }
        }

        // Argument-context builder so a binary/logical chain (or conditional) keeps
        // its continuation indent, and an assignment gets clarity parens — same as
        // the no-blank-line path; the blank-line forced expansion is just another
        // reason the args break.
        arg_parts.push(printer.build_arg_expression_doc(arg));

        if i < args.len() - 1 {
            let arg_end = arg.span().end;
            let next_start = args[i + 1].span().start;

            if printer.has_comments_to_emit_between(arg_end, next_start) {
                let pc = printer.open_inter_arg_gap(&mut arg_parts, arg_end, next_start);

                let next_has_blank =
                    pc.has_blank_line_in_gap(printer.source, printer.layout_line_breaks);
                if next_has_blank {
                    arg_parts.push(d.literalline());
                }
                arg_parts.push(d.hardline());
                // hugging after-comma + own-line comments lead the next arg (`C`).
                pc.emit_leading_comments_inline_aware(&mut arg_parts, printer);
            } else {
                arg_parts.push(d.text(","));
                // Skip hardline if next arg has blank line
                // (handled at top of next iteration — same physical scan window, so the
                // two agree even when an owned annotation sits in the gap).
                let next_has_blank = has_blank_line_between_args(
                    printer.source,
                    printer.layout_line_breaks,
                    arg_end,
                    printer.blank_scan_end(arg_end, next_start),
                );
                if !next_has_blank {
                    arg_parts.push(d.hardline());
                }
            }
        }
    }
    d.concat(&arg_parts)
}
