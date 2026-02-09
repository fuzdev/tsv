// Chain-specific argument building for call expressions
//
// Handles building call arguments in chain contexts where the callee
// is handled separately by the chain printer.

use super::super::Printer;
use super::super::utils::{
    arrow_has_trailing_param_comments, could_expand_arrow_body, is_block_function,
    is_short_second_arg_for_expand_first, last_arg_is_array_or_object, preceding_args_allow_hug,
};
use super::arg_comments::{
    PartitionedComments, any_comment_forces_expansion, find_comma_pos, has_inter_argument_comments,
    has_trailing_comments_on_args, is_comment_after_comma, is_comment_before_comma,
};
use super::arg_wrapping::{
    ChainArgKind, arrow_has_type_annotations, build_args_split_last, build_arrow_inline_signature,
    classify_chain_arg, wrap_args_with_soft_breaks, wrap_huggable_arg,
};
use crate::ast::internal;
use tsv_lang::doc::arena::DocId;

/// Build inline leading block comments for the first argument (non-expansion path).
///
/// Returns comments that should be emitted inline before the first arg:
/// - Block comments on the same line as paren_open (trailing_block)
/// - Block comments on the same line as the first arg (from leading)
fn build_inline_leading_comments(
    printer: &Printer,
    paren_open: u32,
    arg_start: u32,
) -> Option<DocId> {
    let d = printer.d();
    let pc = PartitionedComments::new(printer.comments, printer.line_breaks, paren_open, arg_start);

    let mut parts = Vec::new();

    // Block comments on same line as paren
    for comment in &pc.trailing_block {
        parts.push(printer.build_comment_doc(comment));
        parts.push(d.text(" "));
    }

    // Block comments on same line as first arg
    for comment in &pc.leading {
        if comment.is_block && printer.is_same_line(comment.span.start, arg_start) {
            parts.push(printer.build_comment_doc(comment));
            parts.push(d.text(" "));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(d.concat(&parts))
    }
}

/// Build inline trailing block comments for an argument (non-expansion path).
fn build_inline_trailing_comments(
    printer: &Printer,
    arg_end: u32,
    next_boundary: u32,
) -> Option<DocId> {
    let d = printer.d();
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
        parts.push(d.text(" "));
        parts.push(printer.build_comment_doc(comment));
    }
    Some(d.concat(&parts))
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
) -> DocId {
    build_call_args_doc_for_chain_impl(printer, call, optional, false)
}

/// Build a Doc for call arguments with forced expansion (hardlines instead of softlines)
///
/// Used for the "args expanded, chain inline" state in conditionalGroup.
pub(super) fn build_call_args_doc_for_chain_expanded(
    printer: &Printer,
    call: &internal::CallExpression,
    optional: bool,
) -> DocId {
    build_call_args_doc_for_chain_impl(printer, call, optional, true)
}

/// Implementation for call args doc building
fn build_call_args_doc_for_chain_impl(
    printer: &Printer,
    call: &internal::CallExpression,
    optional: bool,
    force_expand: bool,
) -> DocId {
    let d = printer.d();
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

    // Multiple arrow function arguments: always expand to multiple lines
    // Prettier always expands 2+ arrow function arguments, regardless of source formatting.
    // This matches Prettier's behavior: fn(() => x, () => y) → fn(\n  () => x,\n  () => y,\n)
    let all_args_are_arrows = call.arguments.len() >= 2
        && call
            .arguments
            .iter()
            .all(|arg| matches!(arg, internal::Expression::ArrowFunctionExpression(_)));

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

    let force_expand =
        force_expand || has_blank_lines || comments_force_expansion || all_args_are_arrows;

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
                    inner_parts.push(d.text(" "));
                }
                inner_parts.push(printer.build_comment_doc(comment));
            }
            parts.push(d.text(prefix));
            parts.push(d.concat(&inner_parts));
            parts.push(d.text(")"));
        } else {
            parts.push(d.text_owned(format!("{prefix})")));
        }
        d.concat(&parts)
    } else if force_expand {
        // Special case: single object/array arg should hug the parens
        // and expand internally with hardlines, not softlines around it.
        // e.g., `.push({\n  ...\n})` not `.push(\n  {...},\n)`
        //
        // We use build_arg_expression_doc_expanded which produces hardlines,
        // allowing fits() to correctly measure the first line: `chain.call({`
        // and return true (since hardlines end the fits check).
        //
        // Exception: when there are trailing comments, we use the full expansion
        // path which produces the extra-indented style that Prettier uses:
        // `fn(\n  {...} /* comment */,\n)` not `fn({...} /* comment */)`
        if call.arguments.len() == 1 && !has_trailing_block_comments {
            let arg = &call.arguments[0];
            if matches!(
                arg,
                internal::Expression::ObjectExpression(_)
                    | internal::Expression::ArrayExpression(_)
            ) {
                // Build the object/array with forced internal expansion (hardlines)
                let arg_doc = printer.build_arg_expression_doc_expanded(arg);
                parts.push(d.text(prefix));
                parts.push(arg_doc);
                parts.push(d.text(")"));
                return d.concat(&parts);
            }
        }

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
                        arg_parts.push(d.text(" "));
                    } else {
                        // On own line
                        arg_parts.push(printer.build_comment_doc(comment));
                        arg_parts.push(d.hardline());
                    }
                }

                // Emit trailing comments from paren (inline block comments)
                for comment in &first_pc.trailing_block {
                    arg_parts.push(printer.build_comment_doc(comment));
                    arg_parts.push(d.text(" "));
                }
                for comment in &first_pc.trailing_line {
                    arg_parts.push(printer.build_comment_doc(comment));
                    arg_parts.push(d.hardline());
                }
            }

            // Check for blank line before this arg (from previous arg)
            // Only add blank line preservation when there are no comments between args,
            // since comments will be emitted with their own line breaks
            if i > 0 {
                let prev_end = call.arguments[i - 1].span().end;
                let has_comments_before = printer.has_comments_between(prev_end, arg_start);
                if !has_comments_before && printer.has_blank_line_between(prev_end, arg_start) {
                    arg_parts.push(d.literalline());
                    arg_parts.push(d.hardline());
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
                            arg_parts.push(d.text(" "));
                            arg_parts.push(printer.build_comment_doc(comment));
                        }
                    }
                }

                arg_parts.push(d.text(","));

                // Emit trailing line comments (always after comma)
                for comment in &pc.trailing_line {
                    arg_parts.push(d.text(" "));
                    arg_parts.push(printer.build_comment_doc(comment));
                }

                // Emit trailing block comments that are AFTER the comma (as leading on next arg)
                if let Some(cpos) = comma_pos {
                    for comment in &pc.trailing_block {
                        if is_comment_after_comma(comment, cpos) {
                            arg_parts.push(d.text(" "));
                            arg_parts.push(printer.build_comment_doc(comment));
                        }
                    }
                }

                // Skip hardline if next arg has blank line AND no comments between
                // (blank line preservation handles the line break)
                let has_comments_before_next =
                    printer.has_comments_between(arg_end, next_arg_start);
                let next_has_blank = !has_comments_before_next
                    && printer.has_blank_line_between(arg_end, next_arg_start);
                if !next_has_blank {
                    arg_parts.push(d.hardline());
                }
                pc.emit_leading_comments_inline_aware(&mut arg_parts, printer, next_arg_start);
            } else {
                // Last argument - check for trailing comments before closing paren
                if pc.has_trailing_line() || pc.has_trailing_block() {
                    arg_parts.push(d.text(","));
                    pc.emit_trailing_comments(&mut arg_parts, printer);
                    trailing_comma_already_added = true;
                }
            }
        }

        parts.push(d.text(prefix));
        let trailing = if trailing_comma_already_added {
            d.empty()
        } else {
            d.text(",")
        };
        parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&arg_parts), trailing])));
        parts.push(d.hardline());
        parts.push(d.text(")"));
        d.concat(&parts)
    } else {
        // Single argument handling
        if call.arguments.len() == 1 {
            let arg = &call.arguments[0];

            // Special case: arrow function with call expression body
            // Prettier keeps `(sig =>` hugged, breaking after `=>` to the body.
            // Structure: `(sig =>\n  body,\n)` instead of `(\n  sig =>\n    body,\n)`
            //
            // EXCEPTION: Typed arrows with complex call bodies (objects, non-empty arrays) expand
            // the call arguments, putting signature on its own line:
            // Structure: `(\n  sig =>\n    body,\n)`
            if let internal::Expression::ArrowFunctionExpression(arrow) = arg
                && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
                && matches!(&**body_expr, internal::Expression::CallExpression(_))
            {
                let has_typed_annotations = arrow_has_type_annotations(arrow);

                // Check if body call has complex arguments (objects, non-empty arrays, block functions)
                let is_complex_call_body =
                    if let internal::Expression::CallExpression(nested_call) = &**body_expr {
                        nested_call.arguments.iter().any(|arg| match arg {
                            internal::Expression::ArrowFunctionExpression(arr) => {
                                matches!(arr.body, internal::ArrowFunctionBody::BlockStatement(_))
                            }
                            internal::Expression::ObjectExpression(obj) => {
                                !obj.properties.is_empty()
                            }
                            internal::Expression::ArrayExpression(arr) => !arr.elements.is_empty(),
                            internal::Expression::FunctionExpression(_) => true,
                            _ => false,
                        })
                    } else {
                        false
                    };

                // Typed arrow with complex call body: force full expansion
                // Structure: `(\n  sig =>\n    body,\n)`
                if has_typed_annotations && is_complex_call_body {
                    let arrow_doc = printer.build_arg_expression_doc(arg);
                    let body_doc = printer.build_expression_doc(body_expr);
                    let sig_doc = d.group(printer.build_arrow_signature_doc(arrow));

                    // Expanded state with signature on its own line
                    let expanded_state = d.concat(&[
                        d.text(prefix),
                        d.indent(d.concat(&[
                            d.hardline(),
                            sig_doc,
                            d.text(" =>"),
                            d.indent(d.concat(&[d.hardline(), body_doc, d.text(",")])),
                        ])),
                        d.hardline(),
                        d.text(")"),
                    ]);

                    // If body will break, use expanded directly
                    if d.will_break(body_doc) {
                        parts.push(expanded_state);
                    } else {
                        parts.push(d.conditional_group(&[
                            // Flat: (arrow)
                            d.concat(&[d.text(prefix), arrow_doc, d.text(")")]),
                            // Expanded: (\n  sig =>\n    body,\n)
                            expanded_state,
                        ]));
                    }
                    return d.concat(&parts);
                }

                // Untyped arrow or typed without complex body: use hugged pattern
                let arrow_doc = printer.build_arg_expression_doc(arg);
                let body_doc = printer.build_expression_doc(body_expr);

                // For typed arrows, use full signature; for untyped, use inline signature
                // Wrap in group so the signature stays flat even when the outer group breaks
                let sig_doc = if has_typed_annotations {
                    d.group(printer.build_arrow_signature_doc(arrow))
                } else {
                    build_arrow_inline_signature(printer, arrow)
                };

                // Build the break state (always used when body has hardlines)
                let break_state = d.concat(&[
                    d.text(prefix),
                    sig_doc,
                    d.text(" =>"),
                    d.indent(d.concat(&[d.hardline(), body_doc, d.text(",")])),
                    d.hardline(),
                    d.text(")"),
                ]);

                // If body will break (multiline content), use break state directly
                // This ensures trailing comma is present when content is multiline
                if d.will_break(body_doc) {
                    parts.push(break_state);
                } else {
                    parts.push(d.conditional_group(&[
                        // Flat: (arrow)
                        d.concat(&[d.text(prefix), arrow_doc, d.text(")")]),
                        // Break: (sig =>\n  body,\n)
                        break_state,
                    ]));
                }
                return d.concat(&parts);
            }

            // Special case: arrow function with ternary body
            // Prettier uses conditional parens:
            // - Flat: `map((x) => (x ? y : z))` - with parens
            // - Break: `map((x) =>\n  x ? y : z,)` - no parens, body indented
            if let internal::Expression::ArrowFunctionExpression(arrow) = arg
                && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
                && could_expand_arrow_body(body_expr)
            {
                let arrow_doc = printer.build_arg_expression_doc(arg);
                let body_doc = printer.build_expression_doc(body_expr);
                let sig_doc = d.group(printer.build_arrow_signature_doc(arrow));

                // State 0: Flat - with parens around ternary
                let state_flat = d.concat(&[
                    d.text(prefix),
                    sig_doc,
                    d.text(" => ("),
                    body_doc,
                    d.text("))"),
                ]);

                // State 1: Break - no parens, body indented
                let state_break = d.concat(&[
                    d.text(prefix),
                    sig_doc,
                    d.text(" =>"),
                    d.indent(d.concat(&[d.hardline(), body_doc, d.text(",")])),
                    d.hardline(),
                    d.text(")"),
                ]);

                // State 2: All broken - signature and body both indented
                let state_all_broken = d.concat(&[
                    d.text(prefix),
                    d.indent(d.concat(&[
                        d.hardline(),
                        sig_doc,
                        d.text(" =>"),
                        d.indent(d.concat(&[d.hardline(), body_doc, d.trailing_comma()])),
                    ])),
                    d.hardline(),
                    d.text(")"),
                ]);

                // If arrow is already flat (no breaking content), try all states
                // If it has breaking content, use state_break directly
                if d.will_break(arrow_doc) {
                    parts.push(state_break);
                } else {
                    parts.push(d.conditional_group(&[state_flat, state_break, state_all_broken]));
                }
                return d.concat(&parts);
            }

            // Build arg doc, wrapping certain expressions in isolated_group to prevent
            // internal breaks from propagating to parent groups (enables call hugging)
            let arg_doc = printer.build_huggable_expression_doc(arg);
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
            let arg_with_comments = match (leading_comments_doc, trailing_comments_doc) {
                (Some(leading), Some(trailing)) => d.concat(&[leading, arg_doc, trailing]),
                (Some(leading), None) => d.concat(&[leading, arg_doc]),
                (None, Some(trailing)) => d.concat(&[arg_doc, trailing]),
                (None, None) => arg_doc,
            };

            // Check if it's a block arrow with trailing param comments
            // These need soft-break wrapping to expand the call
            let block_arrow_has_trailing_param_comments =
                if let internal::Expression::ArrowFunctionExpression(arrow) = arg
                    && !arrow.body.is_expression()
                {
                    arrow_has_trailing_param_comments(arrow, |start, end| {
                        printer.has_comments_between(start, end)
                    })
                } else {
                    false
                };

            if block_arrow_has_trailing_param_comments {
                // Block arrow with trailing param comments - force expansion
                parts.push(wrap_args_with_soft_breaks(d, prefix, arg_with_comments));
                return d.concat(&parts);
            }

            match classify_chain_arg(arg) {
                ChainArgKind::NeedsSoftWrap => {
                    // Needs soft-break wrapping - e.g., long strings
                    parts.push(wrap_args_with_soft_breaks(d, prefix, arg_with_comments));
                }
                ChainArgKind::NeedsWrapper => {
                    // Huggable with internal break points (ternary, etc.)
                    // Hugs opening paren but adds trailing comma when content breaks
                    parts.push(wrap_huggable_arg(d, prefix, arg_with_comments));
                }
                ChainArgKind::HugsNaturally => {
                    // Objects/arrays/blocks that hug naturally
                    parts.push(d.text(prefix));
                    parts.push(arg_with_comments);
                    parts.push(d.text(")"));
                }
            }
            return d.concat(&parts);
        }

        // Multiple arguments with callback hugging pattern:
        // When last arg is a block function and preceding args allow hugging,
        // keep args inline: `.method('arg', () => { ... })` instead of expanding
        if call.arguments.len() >= 2
            && call.arguments.last().is_some_and(is_block_function)
            && preceding_args_allow_hug(&call.arguments, printer.line_breaks)
            && !comments_force_expansion
        {
            let (head_parts, last_arg_doc, _) = build_args_split_last(&call.arguments, printer);
            let all_parts: Vec<_> = head_parts
                .into_iter()
                .chain(std::iter::once(last_arg_doc))
                .collect();
            let inner = d.concat(&all_parts);
            parts.push(d.text(prefix));
            parts.push(inner);
            parts.push(d.text(")"));
            return d.concat(&parts);
        }

        // Expression arrow with call expression body
        // Prettier keeps preceding args inline and breaks after =>
        // e.g., `a.b(c, (x) =>\n  fn(x, ...),\n);`
        if call.arguments.len() >= 2
            && preceding_args_allow_hug(&call.arguments, printer.line_breaks)
            && !comments_force_expansion
            && let Some(internal::Expression::ArrowFunctionExpression(arrow)) =
                call.arguments.last()
            && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
            && matches!(&**body_expr, internal::Expression::CallExpression(_))
            && !arrow_has_type_annotations(arrow)
        {
            let (head_parts, last_arg_doc, all_args_broken) =
                build_args_split_last(&call.arguments, printer);
            let inline_sig = build_arrow_inline_signature(printer, arrow);
            let body_doc = printer.build_expression_doc(body_expr);

            // State 0: all inline
            let state_inline = d.concat(&[
                d.text(prefix),
                d.concat(&head_parts),
                last_arg_doc,
                d.text(")"),
            ]);

            // State 1: hug - head inline, arrow body breaks after =>
            let state_break_body = d.concat(&[
                d.text(prefix),
                d.concat(&head_parts),
                inline_sig,
                d.text(" =>"),
                d.indent(d.concat(&[d.hardline(), body_doc, d.text(",")])),
                d.hardline(),
                d.text(")"),
            ]);

            // State 2: expand all args
            let state_expand_all = d.concat(&[
                d.text(prefix),
                d.indent(d.concat(&[d.line(), all_args_broken, d.text(",")])),
                d.line(),
                d.text(")"),
            ]);

            parts.push(d.conditional_group(&[state_inline, state_break_body, state_expand_all]));
            return d.concat(&parts);
        }

        // Expression arrow with object/array body
        // Prettier keeps preceding args inline and expands object/array internally
        // e.g., `a.b(c, (x) => ({\n  y: x,\n}));`
        if call.arguments.len() >= 2
            && preceding_args_allow_hug(&call.arguments, printer.line_breaks)
            && !comments_force_expansion
            && let Some(internal::Expression::ArrowFunctionExpression(arrow)) =
                call.arguments.last()
            && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
            && matches!(
                &**body_expr,
                internal::Expression::ObjectExpression(_)
                    | internal::Expression::ArrayExpression(_)
            )
            && !arrow_has_type_annotations(arrow)
        {
            let (head_parts, last_arg_doc, all_args_broken) =
                build_args_split_last(&call.arguments, printer);
            let inline_sig = build_arrow_inline_signature(printer, arrow);
            // Object/array in arrow body needs parens: (x) => ({ ... })
            let body_doc = d.parens(printer.build_expression_doc(body_expr));

            // State 0: all inline
            let state_inline = d.concat(&[
                d.text(prefix),
                d.concat(&head_parts),
                last_arg_doc,
                d.text(")"),
            ]);

            // State 1: hug - head inline, object/array expands internally
            let state_hug = d.concat(&[
                d.text(prefix),
                d.concat(&head_parts),
                inline_sig,
                d.text(" => "),
                d.group_break(body_doc),
                d.text(")"),
            ]);

            // State 2: expand all args
            let state_expand_all = d.concat(&[
                d.text(prefix),
                d.indent(d.concat(&[d.line(), all_args_broken, d.text(",")])),
                d.line(),
                d.text(")"),
            ]);

            parts.push(d.conditional_group(&[state_inline, state_hug, state_expand_all]));
            return d.concat(&parts);
        }

        // "Expand last arg" pattern for arrays/objects:
        // Keep preceding args inline, only expand the last array/object arg.
        // e.g., `assert.deepEqual(parse('/foo'), [{...}, {...}])` keeps parse('/foo') inline
        // Matches prettier's shouldExpandLastArg for array/object arguments.
        //
        // Skip when last two args are same type (both arrays or both objects) - use expand-all instead.
        if call.arguments.len() >= 2
            && last_arg_is_array_or_object(&call.arguments)
            && preceding_args_allow_hug(&call.arguments, printer.line_breaks)
            && !comments_force_expansion
        {
            // Check if last two args have the same "expandable" type (both arrays or both objects)
            // Prettier disables expand-last-arg in this case
            let last_two_same_type = {
                let last = &call.arguments[call.arguments.len() - 1];
                let penultimate = &call.arguments[call.arguments.len() - 2];
                matches!(
                    (last, penultimate),
                    (
                        internal::Expression::ArrayExpression(_),
                        internal::Expression::ArrayExpression(_)
                    ) | (
                        internal::Expression::ObjectExpression(_),
                        internal::Expression::ObjectExpression(_)
                    )
                )
            };

            if !last_two_same_type {
                let (head_parts, last_arg_doc, all_args_broken) =
                    build_args_split_last(&call.arguments, printer);

                // State 0: inline - all args on one line
                let state_inline = d.concat(&[
                    d.text(prefix),
                    d.concat(&head_parts),
                    last_arg_doc,
                    d.text(")"),
                ]);

                // State 1: hug - head inline, last expands with group_break
                // group_break forces the array/object to break internally
                let state_hug = d.concat(&[
                    d.text(prefix),
                    d.concat(&head_parts),
                    d.group_break(last_arg_doc),
                    d.text(")"),
                ]);

                // State 2: expand all - all args on separate lines
                let state_expand_all = d.concat(&[
                    d.text(prefix),
                    d.indent(d.concat(&[d.line(), all_args_broken, d.text(",")])),
                    d.line(),
                    d.text(")"),
                ]);

                parts.push(d.conditional_group(&[state_inline, state_hug, state_expand_all]));
                return d.concat(&parts);
            }
        }

        // "Expand first arg" pattern: first arg is block function, rest are short
        // e.g., `.reduce((acc, item) => { ... }, {})` - callback hugs, tail args stay inline
        // Matches prettier's shouldExpandFirstArg behavior
        if call.arguments.len() == 2
            && is_block_function(&call.arguments[0])
            && !comments_force_expansion
            && is_short_second_arg_for_expand_first(&call.arguments[1], |start, end| {
                printer.has_comments_between(start, end)
            })
        {
            // First arg (callback) expands, tail args stay inline
            let first_arg_doc = printer.build_arg_expression_doc(&call.arguments[0]);
            let second_arg_doc = printer.build_arg_expression_doc(&call.arguments[1]);

            parts.push(d.text(prefix));
            parts.push(first_arg_doc);
            parts.push(d.text(", "));
            parts.push(second_arg_doc);
            parts.push(d.text(")"));
            return d.concat(&parts);
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
                    d.concat(&[leading, arg_doc])
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
                    arg_docs_with_comments.push(d.concat(&[with_leading, trailing]));
                } else {
                    arg_docs_with_comments.push(with_leading);
                }
            } else {
                arg_docs_with_comments.push(with_leading);
            }
        }
        let arg_parts = d.join_doc(arg_docs_with_comments, d.comma_line());
        parts.push(wrap_args_with_soft_breaks(d, prefix, arg_parts));
        d.concat(&parts)
    }
}
