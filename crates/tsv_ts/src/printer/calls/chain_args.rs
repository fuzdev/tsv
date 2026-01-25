// Chain-specific argument building for call expressions
//
// Handles building call arguments in chain contexts where the callee
// is handled separately by the chain printer.

use super::super::Printer;
use super::arg_comments::{
    PartitionedComments, any_comment_forces_expansion, find_comma_pos, has_inter_argument_comments,
    has_trailing_comments_on_args, is_comment_after_comma, is_comment_before_comma,
};
use super::arg_wrapping::{
    ChainArgKind, arrow_has_type_annotations, build_arrow_inline_signature, classify_chain_arg,
    wrap_args_with_soft_breaks, wrap_huggable_arg,
};
use crate::ast::internal;
use tsv_lang::doc::{self, Doc};

/// Build inline leading block comments for the first argument (non-expansion path).
///
/// Returns comments that should be emitted inline before the first arg:
/// - Block comments on the same line as paren_open (trailing_block)
/// - Block comments on the same line as the first arg (from leading)
fn build_inline_leading_comments(
    printer: &Printer,
    paren_open: u32,
    arg_start: u32,
) -> Option<Doc> {
    let pc = PartitionedComments::new(printer.comments, printer.line_breaks, paren_open, arg_start);

    let mut parts = Vec::new();

    // Block comments on same line as paren
    for comment in &pc.trailing_block {
        parts.push(printer.build_comment_doc(comment));
        parts.push(doc::text(" "));
    }

    // Block comments on same line as first arg
    for comment in &pc.leading {
        if comment.is_block && printer.is_same_line(comment.span.start, arg_start) {
            parts.push(printer.build_comment_doc(comment));
            parts.push(doc::text(" "));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(doc::concat(parts))
    }
}

/// Build inline trailing block comments for an argument (non-expansion path).
fn build_inline_trailing_comments(
    printer: &Printer,
    arg_end: u32,
    next_boundary: u32,
) -> Option<Doc> {
    let pc = PartitionedComments::new(
        printer.comments,
        printer.line_breaks,
        arg_end,
        next_boundary,
    );

    if !pc.has_trailing_block() {
        return None;
    }

    let mut parts = Vec::new();
    for comment in &pc.trailing_block {
        parts.push(doc::text(" "));
        parts.push(printer.build_comment_doc(comment));
    }
    Some(doc::concat(parts))
}

/// Build a Doc for call arguments only (for chain printing)
///
/// Uses proper group wrapping so args can break independently from the chain.
/// This allows the chain's conditionalGroup to try:
/// 1. Everything inline
/// 2. Args broken but chain inline (if args are in their own group)
/// 3. Chain broken (if args broken still doesn't fit)
pub(super) fn build_call_args_doc_for_chain(
    printer: &Printer,
    call: &internal::CallExpression,
    optional: bool,
) -> Doc {
    build_call_args_doc_for_chain_impl(printer, call, optional, false)
}

/// Build a Doc for call arguments with forced expansion (hardlines instead of softlines)
///
/// Used for the "args expanded, chain inline" state in conditionalGroup.
pub(super) fn build_call_args_doc_for_chain_expanded(
    printer: &Printer,
    call: &internal::CallExpression,
    optional: bool,
) -> Doc {
    build_call_args_doc_for_chain_impl(printer, call, optional, true)
}

/// Implementation for call args doc building
fn build_call_args_doc_for_chain_impl(
    printer: &Printer,
    call: &internal::CallExpression,
    optional: bool,
    force_expand: bool,
) -> Doc {
    // Build type arguments if present: `<T, U>`
    let type_args_doc = call
        .type_arguments
        .as_ref()
        .map(|ta| printer.build_type_parameter_instantiation_doc(ta));

    // Check for blank lines between arguments (forces expansion)
    let has_blank_lines = call.arguments.windows(2).any(|window| {
        let prev_end = window[0].span().end;
        let curr_start = window[1].span().start;
        printer.has_blank_line_between(prev_end, curr_start)
    });

    // Get paren_open position (after type args if present, otherwise after callee)
    let paren_open = call
        .type_arguments
        .as_ref()
        .map_or_else(|| call.callee.span().end, |ta| ta.span.end);

    // Check for any comments in arguments (leading, inter-argument, or trailing)
    // Note: presence of comments doesn't necessarily mean expansion - only line comments
    // and block comments on their own line force expansion
    let has_leading_comments = !call.arguments.is_empty()
        && printer.has_comments_between(paren_open, call.arguments[0].span().start);
    let has_inter_arg_comments = has_inter_argument_comments(call, printer);
    let has_trailing_comments = has_trailing_comments_on_args(call, printer);
    // Also check for trailing block comments on last arg (for inline handling)
    let has_trailing_block_comments = call
        .arguments
        .last()
        .is_some_and(|last| printer.has_comments_between(last.span().end, call.span.end));
    let has_any_comments = has_leading_comments
        || has_inter_arg_comments
        || has_trailing_comments
        || has_trailing_block_comments;

    // Check if any comments require expansion (line comments or block comments on own line)
    // Inline block comments don't force expansion
    let comments_force_expansion = any_comment_forces_expansion(call, printer, paren_open);

    let force_expand = force_expand || has_blank_lines || comments_force_expansion;

    let prefix = if optional { "?.(" } else { "(" };

    let mut parts = Vec::new();
    if let Some(ta_doc) = type_args_doc {
        parts.push(ta_doc);
    }

    if call.arguments.is_empty() {
        // Check for comments inside empty parens
        let has_empty_paren_comments = printer.has_comments_between(paren_open, call.span.end);
        if has_empty_paren_comments {
            // Emit comments between parens
            let mut inner_parts = Vec::new();
            for comment in tsv_lang::comments_in_range(printer.comments, paren_open, call.span.end)
            {
                if !inner_parts.is_empty() {
                    inner_parts.push(doc::text(" "));
                }
                inner_parts.push(printer.build_comment_doc(comment));
            }
            parts.push(doc::text(prefix));
            parts.push(doc::concat(inner_parts));
            parts.push(doc::text(")"));
        } else {
            parts.push(doc::text_owned(format!("{prefix})")));
        }
        doc::concat(parts)
    } else if force_expand {
        // Forced expansion: use hardlines instead of softlines
        // Build arguments with blank line preservation and full comment handling
        let mut arg_parts = Vec::new();
        let mut trailing_comma_already_added = false;

        for (i, arg) in call.arguments.iter().enumerate() {
            let arg_start = arg.span().start;

            // Handle leading comments before first argument
            if i == 0 && has_leading_comments {
                let first_pc = PartitionedComments::new(
                    printer.comments,
                    printer.line_breaks,
                    paren_open,
                    arg_start,
                );

                // Emit leading comments
                // - Block comments on same line as first arg: emit inline
                // - Other comments: emit on own line
                for comment in &first_pc.leading {
                    if comment.is_block && printer.is_same_line(comment.span.start, arg_start) {
                        // Inline with first arg
                        arg_parts.push(printer.build_comment_doc(comment));
                        arg_parts.push(doc::text(" "));
                    } else {
                        // On own line
                        arg_parts.push(printer.build_comment_doc(comment));
                        arg_parts.push(doc::hardline());
                    }
                }

                // Emit trailing comments from paren (inline block comments)
                for comment in &first_pc.trailing_block {
                    arg_parts.push(printer.build_comment_doc(comment));
                    arg_parts.push(doc::text(" "));
                }
                for comment in &first_pc.trailing_line {
                    arg_parts.push(printer.build_comment_doc(comment));
                    arg_parts.push(doc::hardline());
                }
            }

            // Check for blank line before this arg (from previous arg)
            // Only add blank line preservation when there are no comments between args,
            // since comments will be emitted with their own line breaks
            if i > 0 {
                let prev_end = call.arguments[i - 1].span().end;
                let has_comments_before = printer.has_comments_between(prev_end, arg_start);
                if !has_comments_before && printer.has_blank_line_between(prev_end, arg_start) {
                    arg_parts.push(doc::literalline());
                    arg_parts.push(doc::hardline());
                }
            }

            arg_parts.push(printer.build_arg_expression_doc(arg));

            // Handle trailing comments and comma placement
            let arg_end = arg.span().end;
            let next_boundary = if i < call.arguments.len() - 1 {
                call.arguments[i + 1].span().start
            } else {
                call.span.end
            };

            let pc = PartitionedComments::new(
                printer.comments,
                printer.line_breaks,
                arg_end,
                next_boundary,
            );

            if i < call.arguments.len() - 1 {
                // Not the last argument
                let next_arg_start = call.arguments[i + 1].span().start;
                let comma_pos = find_comma_pos(printer.source, arg_end, next_arg_start);

                // Emit trailing block comments that are BEFORE the comma
                if let Some(cpos) = comma_pos {
                    for comment in &pc.trailing_block {
                        if is_comment_before_comma(comment, cpos) {
                            arg_parts.push(doc::text(" "));
                            arg_parts.push(printer.build_comment_doc(comment));
                        }
                    }
                }

                arg_parts.push(doc::text(","));

                // Emit trailing line comments (always after comma)
                for comment in &pc.trailing_line {
                    arg_parts.push(doc::text(" "));
                    arg_parts.push(printer.build_comment_doc(comment));
                }

                // Emit trailing block comments that are AFTER the comma (as leading on next arg)
                if let Some(cpos) = comma_pos {
                    for comment in &pc.trailing_block {
                        if is_comment_after_comma(comment, cpos) {
                            arg_parts.push(doc::text(" "));
                            arg_parts.push(printer.build_comment_doc(comment));
                        }
                    }
                }

                // Skip hardline if next arg has blank line AND no comments between
                // (blank line preservation handles the line break)
                let has_comments_before_next = printer.has_comments_between(arg_end, next_arg_start);
                let next_has_blank =
                    !has_comments_before_next && printer.has_blank_line_between(arg_end, next_arg_start);
                if !next_has_blank {
                    arg_parts.push(doc::hardline());
                }
                pc.emit_leading_comments_inline_aware(&mut arg_parts, printer, next_arg_start);
            } else {
                // Last argument - check for trailing comments before closing paren
                if pc.has_trailing_line() || pc.has_trailing_block() {
                    arg_parts.push(doc::text(","));
                    pc.emit_trailing_comments(&mut arg_parts, printer);
                    trailing_comma_already_added = true;
                }
            }
        }

        parts.push(doc::text(prefix));
        let trailing = if trailing_comma_already_added {
            doc::empty()
        } else {
            doc::text(",")
        };
        parts.push(doc::indent(doc::concat(vec![
            doc::hardline(),
            doc::concat(arg_parts),
            trailing,
        ])));
        parts.push(doc::hardline());
        parts.push(doc::text(")"));
        doc::concat(parts)
    } else {
        // Single argument handling
        if call.arguments.len() == 1 {
            let arg = &call.arguments[0];

            // Special case: arrow function with call expression body (no type annotations)
            // Prettier keeps `(sig =>` hugged, breaking after `=>` to the body.
            // Structure: `(sig =>\n  body,\n)` instead of `(\n  sig =>\n    body,\n)`
            if let internal::Expression::ArrowFunctionExpression(arrow) = arg
                && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
            {
                // Only apply when body is a call expression and no type annotations
                if matches!(&**body_expr, internal::Expression::CallExpression(_))
                    && !arrow_has_type_annotations(arrow)
                {
                    let arrow_doc = printer.build_arg_expression_doc(arg);
                    let body_doc = printer.build_expression_doc(body_expr);
                    let inline_sig = build_arrow_inline_signature(printer, arrow);

                    // Build the break state (always used when body has hardlines)
                    let break_state = doc::concat(vec![
                        doc::text(prefix),
                        inline_sig,
                        doc::text(" =>"),
                        doc::indent(doc::concat(vec![
                            doc::hardline(),
                            body_doc.clone(),
                            doc::text(","),
                        ])),
                        doc::hardline(),
                        doc::text(")"),
                    ]);

                    // If body will break (multiline content), use break state directly
                    // This ensures trailing comma is present when content is multiline
                    if doc::will_break(&body_doc) {
                        parts.push(break_state);
                    } else {
                        parts.push(doc::conditional_group(vec![
                            // Flat: (arrow)
                            doc::concat(vec![doc::text(prefix), arrow_doc, doc::text(")")]),
                            // Break: (sig =>\n  body,\n)
                            break_state,
                        ]));
                    }
                    return doc::concat(parts);
                }
            }

            let arg_doc = printer.build_arg_expression_doc(arg);
            let arg_start = arg.span().start;
            let arg_end = arg.span().end;

            // Check for leading inline block comments before the arg
            let leading_comments_doc = if has_leading_comments {
                build_inline_leading_comments(printer, paren_open, arg_start)
            } else {
                None
            };

            // Check for trailing inline block comments (don't force expansion)
            let trailing_comments_doc = if has_any_comments {
                build_inline_trailing_comments(printer, arg_end, call.span.end)
            } else {
                None
            };

            // Build combined arg doc with leading/trailing comments
            let arg_with_comments = match (&leading_comments_doc, &trailing_comments_doc) {
                (Some(leading), Some(trailing)) => {
                    doc::concat(vec![leading.clone(), arg_doc, trailing.clone()])
                }
                (Some(leading), None) => doc::concat(vec![leading.clone(), arg_doc]),
                (None, Some(trailing)) => doc::concat(vec![arg_doc, trailing.clone()]),
                (None, None) => arg_doc,
            };

            match classify_chain_arg(arg) {
                ChainArgKind::NeedsSoftWrap => {
                    // Needs soft-break wrapping - e.g., long strings
                    parts.push(wrap_args_with_soft_breaks(prefix, arg_with_comments));
                }
                ChainArgKind::NeedsWrapper => {
                    // Huggable with internal break points (ternary, etc.)
                    // Hugs opening paren but adds trailing comma when content breaks
                    parts.push(wrap_huggable_arg(prefix, arg_with_comments));
                }
                ChainArgKind::HugsNaturally => {
                    // Objects/arrays/blocks that hug naturally
                    parts.push(doc::text(prefix));
                    parts.push(arg_with_comments);
                    parts.push(doc::text(")"));
                }
            }
            return doc::concat(parts);
        }

        // Multiple arguments: wrap in group with softlines so they can break
        // Include inline block comments if present
        let mut arg_docs_with_comments = Vec::new();
        for (i, arg) in call.arguments.iter().enumerate() {
            let arg_doc = printer.build_arg_expression_doc(arg);
            let arg_start = arg.span().start;
            let arg_end = arg.span().end;

            // For first arg, check for leading inline block comments
            let with_leading = if i == 0 && has_leading_comments {
                if let Some(leading) = build_inline_leading_comments(printer, paren_open, arg_start)
                {
                    doc::concat(vec![leading, arg_doc])
                } else {
                    arg_doc
                }
            } else {
                arg_doc
            };

            // Check for trailing block comments
            let next_boundary = if i < call.arguments.len() - 1 {
                call.arguments[i + 1].span().start
            } else {
                call.span.end
            };

            if has_any_comments {
                if let Some(trailing) =
                    build_inline_trailing_comments(printer, arg_end, next_boundary)
                {
                    arg_docs_with_comments.push(doc::concat(vec![with_leading, trailing]));
                } else {
                    arg_docs_with_comments.push(with_leading);
                }
            } else {
                arg_docs_with_comments.push(with_leading);
            }
        }
        let arg_parts = doc::join_doc(arg_docs_with_comments, doc::comma_line());
        parts.push(wrap_args_with_soft_breaks(prefix, arg_parts));
        doc::concat(parts)
    }
}
