// Argument classification and wrapping utilities for call expressions
//
// Handles:
// - Argument classification for chain contexts
// - Call expression wrapping with soft/hard breaks
// - Building argument lists split into head/last patterns

use super::super::{
    ArrowChainContext, CommentFilter, CommentSpacing, Printer, has_newline_before_position,
    is_curried_arrow_chain, is_multiline_template_expression,
};
use super::arg_comments::{
    emit_first_arg_leading_comments, find_comma_pos, has_blank_line_between_args,
    is_inline_block_after_comma, is_inline_block_before_comma,
};
use super::arg_predicates::{is_block_function, is_short_second_arg_for_expand_first};
use crate::ast::internal;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};

/// Build an inline arrow function signature without break points.
///
/// Used when we want the signature to stay on one line (e.g., `(x) =>`).
/// Does NOT include the ` =>` - caller adds that.
/// Only handles untyped arrows (no type params, no return type, no param types).
pub(crate) fn build_arrow_inline_signature(
    printer: &Printer<'_>,
    arrow: &internal::ArrowFunctionExpression,
) -> DocId {
    let d = printer.d();
    let mut sig_parts = DocBuf::new();
    if arrow.r#async {
        sig_parts.push(d.text("async "));
    }
    if arrow.params.is_empty() {
        if let Some(open) = arrow.params_start
            && let Some(close_after) = printer.find_closing_paren(open, arrow.body.span().start)
            && let Some(comment_doc) = printer
                .build_inline_comments_between_doc_no_leading_space_opt(open + 1, close_after - 1)
        {
            sig_parts.push(d.text("("));
            sig_parts.push(comment_doc);
            sig_parts.push(d.text(")"));
        } else {
            sig_parts.push(d.text("()"));
        }
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
    arrow: &internal::ArrowFunctionExpression,
) -> DocId {
    if arrow_has_type_annotations(arrow) {
        printer.d().group(printer.build_arrow_signature_doc(arrow))
    } else {
        build_arrow_inline_signature(printer, arrow)
    }
}

/// Prepend any comments between arrow `=>` and body expression to `body_doc`.
///
/// When call argument paths build `sig_doc` and `body_doc` separately
/// (for break states like `(sig =>\n  body,\n)`), comments between `=>`
/// and the body are not part of either doc. This finds them and prepends
/// to `body_doc`, returning it unchanged if none exist.
pub(crate) fn prepend_arrow_body_comments(
    printer: &Printer<'_>,
    arrow: &internal::ArrowFunctionExpression,
    body_start: u32,
    body_doc: DocId,
) -> DocId {
    let arrow_end = printer.find_arrow_token_for(arrow) + "=>".len() as u32;

    // Prepend inline comments between `=>` and body
    if let Some(lc) = printer.build_rhs_comments_opt(arrow_end, body_start) {
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
/// `post_comma` is emitted immediately after the last arg (before the closing `)`), so
/// an after-comma trailing comment stays past where the comma was; pass `d.empty()` when
/// there is none. No trailing comma is emitted (trailingComma: 'none').
#[inline]
fn wrap_call(
    d: &DocArena,
    callee: DocId,
    args: DocId,
    post_comma: DocId,
    style: CallBreakStyle,
) -> DocId {
    match style {
        CallBreakStyle::Soft => d.concat(&[
            callee,
            d.group(d.concat(&[
                d.text("("),
                d.indent_softline(d.concat(&[args, post_comma])),
                d.softline(),
                d.text(")"),
            ])),
        ]),
        CallBreakStyle::Hard => d.concat(&[
            callee,
            d.text("("),
            d.indent(d.concat(&[d.hardline(), args, post_comma])),
            d.hardline(),
            d.text(")"),
        ]),
    }
}

/// Wrap arguments in a groupable call expression: `callee(args)`
/// Uses soft breaks so the call can collapse to a single line if it fits
#[inline]
pub(crate) fn wrap_call_with_soft_breaks(d: &DocArena, callee: DocId, args: DocId) -> DocId {
    wrap_call(d, callee, args, d.empty(), CallBreakStyle::Soft)
}

/// Wrap arguments in an expanded call expression: `callee(\n\targs,\n)`
/// Uses hard breaks to force multi-line layout
#[inline]
pub(crate) fn wrap_call_with_hard_breaks(d: &DocArena, callee: DocId, args: DocId) -> DocId {
    wrap_call(d, callee, args, d.empty(), CallBreakStyle::Hard)
}

/// Like [`wrap_call_with_soft_breaks`], but emits `post_comma` after the last arg so an
/// after-comma trailing comment is preserved past where the comma was (`b /* c */`; no
/// trailing comma, trailingComma: 'none').
#[inline]
pub(super) fn wrap_call_with_soft_breaks_suffix(
    d: &DocArena,
    callee: DocId,
    args: DocId,
    post_comma: DocId,
) -> DocId {
    wrap_call(d, callee, args, post_comma, CallBreakStyle::Soft)
}

/// Like [`wrap_call_with_hard_breaks`], but emits `post_comma` after the last arg (no
/// trailing comma; trailingComma: 'none').
#[inline]
pub(super) fn wrap_call_with_hard_breaks_suffix(
    d: &DocArena,
    callee: DocId,
    args: DocId,
    post_comma: DocId,
) -> DocId {
    wrap_call(d, callee, args, post_comma, CallBreakStyle::Hard)
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
pub(super) fn arg_needs_soft_wrap(arg: &internal::Expression) -> bool {
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
pub(super) fn classify_chain_arg(arg: &internal::Expression) -> ChainArgKind {
    match arg {
        // Block-like expressions hug the call parens naturally
        internal::Expression::ObjectExpression(_)
        | internal::Expression::ArrayExpression(_)
        | internal::Expression::FunctionExpression(_)
        | internal::Expression::ClassExpression(_) => ChainArgKind::HugsNaturally,
        // TS type wrappers: classify based on the inner expression
        // e.g., `{...} as any` hugs, `longExpr as T` soft-wraps
        internal::Expression::TSAsExpression(e) => classify_chain_arg(&e.expression),
        internal::Expression::TSSatisfiesExpression(e) => classify_chain_arg(&e.expression),
        internal::Expression::TSTypeAssertion(e) => classify_chain_arg(&e.expression),
        internal::Expression::TSNonNullExpression(e) => classify_chain_arg(&e.expression),
        // Arrow functions: arrows with TSTypeReference return types are NOT expandable
        // per prettier's couldExpandArg, so they need soft wrapping (default behavior).
        // Other arrows are classified by their body.
        internal::Expression::ArrowFunctionExpression(arrow) => {
            if arrow_has_type_reference_return(arrow) {
                ChainArgKind::NeedsSoftWrap
            } else {
                classify_arrow_body(arrow)
            }
        }
        // Everything else needs soft wrapping so the call can break
        // before the argument, giving the argument a fresh line to fit on
        _ => ChainArgKind::NeedsSoftWrap,
    }
}

/// Check if an arrow function has any type annotations (return type, type params, or param types).
///
/// Used to determine formatting behavior - arrows with type annotations often need
/// different breaking strategies than untyped arrows.
pub(crate) fn arrow_has_type_annotations(arrow: &internal::ArrowFunctionExpression) -> bool {
    arrow.return_type.is_some()
        || arrow.type_parameters.is_some()
        || arrow.params.iter().any(param_has_type_annotation)
}

/// Check if an arrow function has a TSTypeReference return type.
///
/// Matches prettier's couldExpandArg check: arrows with TSTypeReference return
/// types (e.g., `Promise<any>`, `Array<T>`) and non-block bodies are NOT expandable.
/// Other return types (TSTypePredicate, keyword types, unions) are fine.
///
/// Prettier ref: call-arguments.js couldExpandArg (lines 222-226)
pub(crate) fn arrow_has_type_reference_return(arrow: &internal::ArrowFunctionExpression) -> bool {
    if let Some(rt) = &arrow.return_type {
        matches!(*rt.type_annotation, internal::TSType::TypeReference(_))
    } else {
        false
    }
}

/// Check if a function parameter has a type annotation.
///
/// Handles all parameter patterns: Identifier, ArrayPattern, ObjectPattern, AssignmentPattern.
fn param_has_type_annotation(param: &internal::Expression) -> bool {
    match param {
        internal::Expression::Identifier(id) => id.type_annotation.is_some(),
        internal::Expression::ArrayPattern(arr) => arr.type_annotation.is_some(),
        internal::Expression::ObjectPattern(obj) => obj.type_annotation.is_some(),
        internal::Expression::AssignmentPattern(assign) => {
            // Assignment patterns wrap another pattern/identifier
            match assign.left.as_ref() {
                internal::Expression::Identifier(id) => id.type_annotation.is_some(),
                internal::Expression::ArrayPattern(arr) => arr.type_annotation.is_some(),
                internal::Expression::ObjectPattern(obj) => obj.type_annotation.is_some(),
                _ => false,
            }
        }
        _ => false,
    }
}

/// Classify how an arrow function body should be formatted in chain context.
fn classify_arrow_body(arrow: &internal::ArrowFunctionExpression) -> ChainArgKind {
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
pub(crate) fn could_expand_arrow_chain(arrow: &internal::ArrowFunctionExpression) -> bool {
    match &arrow.body {
        internal::ArrowFunctionBody::BlockStatement(_) => true,
        internal::ArrowFunctionBody::Expression(expr) => match &**expr {
            internal::Expression::ObjectExpression(_)
            | internal::Expression::ArrayExpression(_) => true,
            internal::Expression::ArrowFunctionExpression(inner) => {
                !arrow_has_type_reference_return(inner) && could_expand_arrow_chain(inner)
            }
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
fn classify_expression_body(expr: &internal::Expression) -> ChainArgKind {
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

/// Build argument docs split into head parts (with commas), last arg, and broken form
///
/// Used for patterns that keep short args inline with the last arg.
/// Returns (head_parts, last_arg_doc, all_args_broken) where:
/// - head_parts: all but last arg with ", " separators (includes inline block comments)
/// - last_arg_doc: the last argument doc
/// - all_args_broken: all args joined with comma_line() for fallback (includes inline block comments)
pub(crate) fn build_args_split_last(
    arguments: &[internal::Expression],
    printer: &Printer<'_>,
    paren_open: u32,
) -> (DocBuf, DocId, DocId) {
    let d = printer.d();
    // Build all args (using build_huggable_expression_doc for proper parens on assignments
    // and isolated_group wrapping for templates).
    //
    // A curried arrow-chain argument (`fn(x, (a) => (b) => …)`) routes through the
    // progressive call-arg chain layout: set the context so the outermost chain
    // arrow flattens its heads (`should_use_arrow_chain_layout` still gates on
    // untyped / comment-free, and `skip_arrow_chain` keeps the expand-last-arg hug
    // states on the default path). Mirrors prettier's `isCallLikeExpression(parent)`
    // reaching `printArrowFunctionSignatures`.
    let arg_docs: Vec<_> = arguments
        .iter()
        .map(|arg| {
            if is_curried_arrow_chain(arg) {
                printer
                    .build_with_arrow_chain_context(ArrowChainContext::CallArgOrBinaryish, || {
                        printer.build_huggable_expression_doc(arg)
                    })
            } else {
                printer.build_huggable_expression_doc(arg)
            }
        })
        .collect();

    // Leading comments between `(` and the first argument (e.g., /** @type {T} */).
    // Not handled by per-arg building — prepended to both head_parts and all_args_broken.
    let leading_comment_doc = printer.build_rhs_comments_opt(paren_open, arguments[0].span().start);

    // Build head docs (all but last) with commas and inline block comments
    // Comments are placed relative to the comma based on their source position
    let mut head_parts = DocBuf::new();
    if let Some(lc) = leading_comment_doc {
        head_parts.push(lc);
    }

    for (i, doc) in arg_docs.iter().take(arg_docs.len() - 1).enumerate() {
        head_parts.push(*doc);

        let arg_end = arguments[i].span().end;
        let next_arg_start = arguments[i + 1].span().start;
        let comma_pos = find_comma_pos(printer.source, arg_end, next_arg_start);

        // Add inline block comments around comma
        if let Some(cpos) = comma_pos {
            for comment in tsv_lang::comments_in_range(printer.comments, arg_end, next_arg_start) {
                if is_inline_block_before_comma(comment, cpos, printer.line_breaks, arg_end) {
                    head_parts.push(d.text(" "));
                    head_parts.push(printer.build_comment_doc(comment));
                }
            }
        }

        head_parts.push(d.text(", "));

        if let Some(cpos) = comma_pos {
            for comment in tsv_lang::comments_in_range(printer.comments, arg_end, next_arg_start) {
                if is_inline_block_after_comma(comment, cpos, printer.line_breaks, arg_end) {
                    head_parts.push(printer.build_comment_doc(comment));
                    head_parts.push(d.text(" "));
                }
            }
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

        // Add trailing inline block comments (except after last arg)
        if i < arguments.len() - 1 {
            let arg_end = arguments[i].span().end;
            let next_arg_start = arguments[i + 1].span().start;
            let comma_pos = find_comma_pos(printer.source, arg_end, next_arg_start);

            // Only add inline block comments that are BEFORE the comma
            if let Some(cpos) = comma_pos {
                for comment in
                    tsv_lang::comments_in_range(printer.comments, arg_end, next_arg_start)
                {
                    if is_inline_block_before_comma(comment, cpos, printer.line_breaks, arg_end) {
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
    head_parts: DocBuf,
    last_arg_doc: DocId,
) -> DocId {
    d.concat(&[
        callee,
        d.text("("),
        d.concat(&head_parts),
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
    head_parts: DocBuf,
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
pub(crate) fn last_two_args_same_type(args: &[internal::Expression]) -> bool {
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
/// Parameters:
/// - `callee`: The call expression's callee doc
/// - `arrow_doc`: The full arrow expression doc (for flat state)
/// - `sig_doc`: The arrow's signature doc (for break state)
/// - `body_doc`: The arrow body expression doc
#[inline]
pub(crate) fn build_arrow_call_body_states(
    d: &DocArena,
    callee: DocId,
    arrow_doc: DocId,
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
        d.concat(&[callee, d.text("("), arrow_doc, d.text(")")]),
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
    arguments: &[internal::Expression],
    paren_open: u32,
    use_hardline: bool,
    build_arg: impl Fn(&Printer<'_>, &internal::Expression) -> DocId,
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

            if printer.has_comments_between(arg_end, next_arg_start) {
                let pc = printer.open_inter_arg_gap(&mut parts, arg_end, next_arg_start);
                // A line comment runs to EOL → hard-break; otherwise honor the caller's style.
                parts.push(if pc.has_trailing_line() || use_hardline {
                    d.hardline()
                } else {
                    d.line()
                });
                // hugging after-comma + own-line comments lead the next arg (`C`).
                pc.emit_leading_comments_inline_aware(&mut parts, printer, next_arg_start);
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
    args: &[internal::Expression],
) -> bool {
    // Need exactly 2 args (first is function, second is short)
    if args.len() != 2 {
        return false;
    }

    // First arg must be a function with block body
    if !is_block_function(&args[0]) {
        return false;
    }

    // Prettier's couldExpandArg returns true for objects/arrays with hasComment(node),
    // which includes leading comments. This makes !couldExpandArg(secondArg) = false,
    // blocking shouldExpandFirstArg. Without this check, the expand-first path also
    // drops the leading comment (SAFETY).
    if matches!(
        &args[1],
        internal::Expression::ObjectExpression(_) | internal::Expression::ArrayExpression(_)
    ) && printer.has_comments_between(args[0].span().end, args[1].span().start)
    {
        return false;
    }

    // Second arg must be short/simple
    is_short_second_arg_for_expand_first(&args[1], |start, end| {
        printer.has_comments_between(start, end)
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
    type_arguments: Option<&internal::TSTypeParameterInstantiation>,
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
    let d = printer.d();
    let mut parts = vec![callee];
    if let Some(paren_pos) = printer.find_char_outside_comments(after_type_args, paren_close, b'(')
    {
        let pre_paren_comments = printer.build_comments_between_filtered_opt(
            after_type_args,
            paren_pos,
            CommentSpacing::Leading,
            CommentFilter::All,
        );
        let inside_paren_comments = printer
            .build_inline_comments_between_doc_no_leading_space_opt(paren_pos + 1, paren_close);
        if let Some(pre) = pre_paren_comments {
            parts.push(pre);
        }
        match inside_paren_comments {
            Some(inner) => {
                parts.push(d.text("("));
                parts.push(inner);
                parts.push(d.text(")"));
            }
            None => parts.push(d.text("()")),
        }
    } else {
        // Fallback: no `(` found (shouldn't happen for valid code)
        parts.push(d.text("()"));
    }
    d.concat(&parts)
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
    args: &[internal::Expression],
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
    let mut parts = vec![callee, d.text("("), arg_doc, d.text(")")];
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
    args: &[internal::Expression],
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
            if !printer.has_comments_between(prev_end, curr_start)
                && has_blank_line_between_args(
                    printer.source,
                    printer.line_breaks,
                    prev_end,
                    curr_start,
                )
            {
                arg_parts.push(d.literalline());
                arg_parts.push(d.hardline());
            }
        }

        arg_parts.push(printer.build_expression_doc(arg));

        if i < args.len() - 1 {
            let arg_end = arg.span().end;
            let next_start = args[i + 1].span().start;

            if printer.has_comments_between(arg_end, next_start) {
                let pc = printer.open_inter_arg_gap(&mut arg_parts, arg_end, next_start);

                let next_has_blank = pc.has_blank_line_in_gap(
                    printer.source,
                    printer.line_breaks,
                    arg_end,
                    next_start,
                );
                if next_has_blank {
                    arg_parts.push(d.literalline());
                }
                arg_parts.push(d.hardline());
                // hugging after-comma + own-line comments lead the next arg (`C`).
                pc.emit_leading_comments_inline_aware(&mut arg_parts, printer, next_start);
            } else {
                arg_parts.push(d.text(","));
                // Skip hardline if next arg has blank line
                // (handled at top of next iteration)
                let next_has_blank = has_blank_line_between_args(
                    printer.source,
                    printer.line_breaks,
                    arg_end,
                    next_start,
                );
                if !next_has_blank {
                    arg_parts.push(d.hardline());
                }
            }
        }
    }
    d.concat(&arg_parts)
}
