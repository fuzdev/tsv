// Chain expansion analysis helpers
//
// Pure functions for determining when chains should force expansion:
// - Blank line detection between methods
// - Comment-forced expansion
// - Complex argument detection
// - Callback analysis

use crate::ast::internal::{ArrowFunctionBody, Expression};
use crate::printer::calls::arg_predicates::is_simple_call_argument;

use super::super::printing::ChainPrinter;
use super::super::types::{ChainGroup, ChainNode};
use tsv_lang::printing::{self, has_blank_line_between_fast};

/// Check if a function parameter has a type annotation.
pub(super) fn has_param_type_annotation(param: &Expression) -> bool {
    match param {
        Expression::Identifier(id) => id.type_annotation.is_some(),
        Expression::ArrayPattern(arr) => arr.type_annotation.is_some(),
        Expression::ObjectPattern(obj) => obj.type_annotation.is_some(),
        Expression::AssignmentPattern(assign) => match assign.left.as_ref() {
            Expression::Identifier(id) => id.type_annotation.is_some(),
            Expression::ArrayPattern(arr) => arr.type_annotation.is_some(),
            Expression::ObjectPattern(obj) => obj.type_annotation.is_some(),
            _ => false,
        },
        _ => false,
    }
}

/// Check if there are blank lines BETWEEN methods (not just before the first method)
///
/// Prettier's blank line rules:
/// - Blank line before first method ONLY (no other blank lines) → try to fit inline
/// - Blank lines BETWEEN methods (groups[2+]) → force expand
///
/// Returns true only if there are blank lines after the first method (groups index >= 2),
/// which is when we should force the expanded layout.
pub(super) fn has_blank_lines_between_methods<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> bool {
    let line_breaks = printer.get_line_breaks();
    // Skip groups[0] (base) and groups[1] (first method) - only check groups[2+]
    groups.iter().skip(2).any(|group| {
        group
            .first_member_range()
            .is_some_and(|(obj_end, prop_start)| {
                has_blank_line_between_fast(line_breaks, obj_end, prop_start)
            })
    })
}

/// Check if any chain segment has comments that force expansion.
///
/// Comments between chain segments generally force the chain to expand, EXCEPT
/// for comments before the trailing member/computed member (last member-like node
/// in the chain). Those comments are handled inline via line_suffix in print_node.
///
/// Returns true if comments exist that should force expansion.
pub(super) fn has_comments_forcing_expansion<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> bool {
    for (group_idx, group) in groups.iter().enumerate() {
        let is_last_group = group_idx == groups.len() - 1;

        for (node_idx, node) in group.nodes.iter().enumerate() {
            // Skip the last member node in the last group - its comments are
            // handled inline via line_suffix, not by forcing expansion
            let is_last_node_in_last_group =
                is_last_group && node_idx == group.nodes.len() - 1 && node.is_member();
            if is_last_node_in_last_group {
                continue;
            }

            if let Some((obj_end, prop_start)) = node.comment_range()
                && printer.has_comments_between(obj_end, prop_start)
            {
                return true;
            }
        }
    }
    false
}

/// Check if a call node has complex (non-simple) arguments
///
/// Uses Prettier's `isSimpleCallArgument` logic (inverted) to determine
/// if a 3+ call chain should force break.
pub(super) fn call_has_complex_args<'a>(node: &ChainNode<'a>) -> bool {
    let Some(call) = node.as_call_expression() else {
        return false;
    };
    // Check if any argument is NOT simple (using Prettier's depth-limited check)
    call.arguments
        .iter()
        .any(|arg| !is_simple_call_argument(arg, 2))
}

/// Status of callback arguments in a call node
#[derive(Default)]
pub(super) struct CallbackStatus {
    /// Whether the call has any callback argument (arrow/function)
    pub has_callback: bool,
    /// Whether any callback will break (multiline body)
    pub will_break: bool,
}

/// Analyze callback status for a call node in a single pass
pub(super) fn call_callback_status<'a>(
    node: &ChainNode<'a>,
    line_breaks: &[u32],
) -> CallbackStatus {
    let Some(call) = node.as_call_expression() else {
        return CallbackStatus::default();
    };

    let mut has_callback = false;
    let mut will_break = false;

    for arg in &call.arguments {
        match arg {
            Expression::ArrowFunctionExpression(arrow) => {
                has_callback = true;
                if !will_break {
                    will_break = match &arrow.body {
                        // Block body breaks if it has statements or contains comments
                        // (comment-only blocks emit hardlines via comment printing)
                        ArrowFunctionBody::BlockStatement(block) => {
                            !block.body.is_empty()
                                || printing::has_newline_between_fast(
                                    line_breaks,
                                    block.span.start,
                                    block.span.end,
                                )
                        }
                        // Expression body - check if it's multiline (O(log n))
                        ArrowFunctionBody::Expression(expr) => {
                            let span = expr.span();
                            printing::has_newline_between_fast(line_breaks, span.start, span.end)
                        }
                    };
                }
            }
            Expression::FunctionExpression(func) => {
                // Function expressions break if body has statements or contains comments
                has_callback = true;
                if !will_break {
                    will_break = !func.body.body.is_empty()
                        || printing::has_newline_between_fast(
                            line_breaks,
                            func.body.span.start,
                            func.body.span.end,
                        );
                }
            }
            _ => {}
        }
        // Early exit if we've found everything
        if has_callback && will_break {
            break;
        }
    }

    CallbackStatus {
        has_callback,
        will_break,
    }
}

/// Check if chain ends with member access (not a call)
///
/// Used to enable the intermediate state where callback args expand but chain stays inline.
/// Skips trailing NonNull assertions - `.length!` counts as ending with member.
pub(super) fn ends_with_member<'a>(
    rest_groups: &[ChainGroup<'a>],
    first_groups: &[ChainGroup<'a>],
) -> bool {
    rest_groups
        .last()
        .or_else(|| first_groups.last())
        .is_some_and(|g| {
            g.nodes
                .iter()
                .rev()
                .find(|n| !n.is_non_null())
                .is_some_and(ChainNode::is_member)
        })
}
