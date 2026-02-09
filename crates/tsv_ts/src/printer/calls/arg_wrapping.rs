// Argument classification and wrapping utilities for call expressions
//
// Handles:
// - Argument classification for chain contexts
// - Call expression wrapping with soft/hard breaks
// - Building argument lists split into head/last patterns

use super::super::Printer;
use super::arg_comments::{
    find_comma_pos, is_inline_block_after_comma, is_inline_block_before_comma,
};
use crate::ast::internal;
use tsv_lang::doc::arena::{DocArena, DocId};

/// Build an inline arrow function signature without break points.
///
/// Used when we want the signature to stay on one line (e.g., `(x) =>`).
/// Does NOT include the ` =>` - caller adds that.
/// Only handles untyped arrows (no type params, no return type, no param types).
pub(super) fn build_arrow_inline_signature(
    printer: &Printer,
    arrow: &internal::ArrowFunctionExpression,
) -> DocId {
    let d = printer.d();
    let mut sig_parts = Vec::new();
    if arrow.r#async {
        sig_parts.push(d.text("async "));
    }
    if arrow.params.is_empty() {
        sig_parts.push(d.text("()"));
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
#[inline]
fn wrap_call(d: &DocArena, callee: DocId, args: DocId, style: CallBreakStyle) -> DocId {
    match style {
        CallBreakStyle::Soft => d.concat(&[
            callee,
            d.group(d.concat(&[
                d.text("("),
                d.indent_softline(d.concat(&[args, d.trailing_comma()])),
                d.softline(),
                d.text(")"),
            ])),
        ]),
        CallBreakStyle::Hard => d.concat(&[
            callee,
            d.text("("),
            d.indent(d.concat(&[d.hardline(), args, d.text(",")])),
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
/// Returns the `ChainArgKind` for arrow functions, or `NeedsSoftWrap` for
/// other expression types that need soft wrapping (calls, members, binaries).
pub(super) fn classify_chain_arg(arg: &internal::Expression) -> ChainArgKind {
    match arg {
        // These expression types need soft wrapping so the call can break
        // before the argument, giving the argument a fresh line to fit on
        internal::Expression::CallExpression(_)
        | internal::Expression::MemberExpression(_)
        | internal::Expression::NewExpression(_)
        | internal::Expression::Identifier(_)
        | internal::Expression::BinaryExpression(_)
        | internal::Expression::ConditionalExpression(_) => ChainArgKind::NeedsSoftWrap,
        // Template literals need soft wrap so long lines can break at the call's (
        internal::Expression::TemplateLiteral(_) => ChainArgKind::NeedsSoftWrap,
        // Tagged templates need soft wrap too
        internal::Expression::TaggedTemplateExpression(_) => ChainArgKind::NeedsSoftWrap,
        // Literals need soft wrapping so chains can break at the last call
        // This allows e.g. `fn1(fn2()).fn3('short')` to break at fn3 instead of fn2
        internal::Expression::Literal(_) => ChainArgKind::NeedsSoftWrap,
        // Arrow functions are classified by their body
        internal::Expression::ArrowFunctionExpression(arrow) => classify_arrow_body(arrow),
        // Everything else hugs naturally (objects, arrays as direct args)
        _ => ChainArgKind::HugsNaturally,
    }
}

/// Check if an arrow function has any type annotations (return type, type params, or param types).
///
/// Used to determine formatting behavior - arrows with type annotations often need
/// different breaking strategies than untyped arrows.
pub(super) fn arrow_has_type_annotations(arrow: &internal::ArrowFunctionExpression) -> bool {
    arrow.return_type.is_some()
        || arrow.type_parameters.is_some()
        || arrow.params.iter().any(param_has_type_annotation)
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

/// Classify how an expression body should be formatted.
fn classify_expression_body(expr: &internal::Expression) -> ChainArgKind {
    match expr {
        // Objects and arrays have their own trailing comma handling
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
/// Structure: `prefix + softline + args + trailing_comma + softline + ")"`
#[inline]
pub(super) fn wrap_args_with_soft_breaks(d: &DocArena, prefix: &'static str, args: DocId) -> DocId {
    d.group(d.concat(&[
        d.text(prefix),
        d.indent_softline(d.concat(&[args, d.trailing_comma()])),
        d.softline(),
        d.text(")"),
    ]))
}

/// Wrap a single huggable argument - hugs opening paren but adds trailing comma
/// when the content breaks internally.
///
/// Used for expressions with natural break points (objects, arrays, ternaries)
/// that should hug the opening paren but still get proper trailing comma handling.
/// Structure: `prefix + arg + if_break(",\n") + ")"`
#[inline]
pub(super) fn wrap_huggable_arg(d: &DocArena, prefix: &'static str, arg: DocId) -> DocId {
    d.group(d.concat(&[
        d.text(prefix),
        arg,
        d.if_break(d.concat(&[d.text(","), d.line()]), d.empty()),
        d.text(")"),
    ]))
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
    printer: &Printer,
) -> (Vec<DocId>, DocId, DocId) {
    let d = printer.d();
    // Build all args (using build_huggable_expression_doc for proper parens on assignments
    // and isolated_group wrapping for templates)
    let arg_docs: Vec<_> = arguments
        .iter()
        .map(|arg| printer.build_huggable_expression_doc(arg))
        .collect();

    // Build head docs (all but last) with commas and inline block comments
    // Comments are placed relative to the comma based on their source position
    let mut head_parts = Vec::new();
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
    let mut all_args_parts = Vec::new();
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
#[inline]
pub(super) fn build_expand_all_args(d: &DocArena, callee: DocId, all_args_broken: DocId) -> DocId {
    d.concat(&[
        callee,
        d.text("("),
        d.indent(d.concat(&[d.line(), all_args_broken, d.text(",")])),
        d.line(),
        d.text(")"),
    ])
}

/// Build the "inline" doc structure: `callee(head_parts + last_arg)`
///
/// Used as the first state in conditional groups where we try to fit everything inline.
#[inline]
pub(super) fn build_inline_args(
    d: &DocArena,
    callee: DocId,
    head_parts: Vec<DocId>,
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
pub(super) fn build_inline_or_expand_all(
    d: &DocArena,
    callee: DocId,
    head_parts: Vec<DocId>,
    last_arg_doc: DocId,
    all_args_broken: DocId,
) -> DocId {
    d.conditional_group(&[
        build_inline_args(d, callee, head_parts, last_arg_doc),
        build_expand_all_args(d, callee, all_args_broken),
    ])
}

/// Build a conditional group for arrow functions with call expression bodies.
///
/// Used when an arrow's body is a call expression (simple or with complex args).
/// Creates two states:
/// - State 0 (flat): `callee((params) => body)`
/// - State 1 (break): `callee((params) =>\n  body,\n)`
///
/// Parameters:
/// - `callee`: The call expression's callee doc
/// - `arrow_doc`: The full arrow expression doc (for flat state)
/// - `inline_sig`: The arrow's inline signature (for break state)
/// - `body_doc`: The arrow body expression doc
#[inline]
pub(super) fn build_arrow_call_body_states(
    d: &DocArena,
    callee: DocId,
    arrow_doc: DocId,
    inline_sig: DocId,
    body_doc: DocId,
) -> DocId {
    d.conditional_group(&[
        // Flat: callee((params) => body)
        d.concat(&[callee, d.text("("), arrow_doc, d.text(")")]),
        // Break: callee((params) =>\n  body,\n)
        d.concat(&[
            callee,
            d.text("("),
            inline_sig,
            d.text(" =>"),
            d.indent(d.concat(&[d.hardline(), body_doc, d.text(",")])),
            d.hardline(),
            d.text(")"),
        ]),
    ])
}
