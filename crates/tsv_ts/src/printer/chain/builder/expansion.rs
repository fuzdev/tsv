// Chain expansion analysis helpers
//
// Pure functions for determining when chains should force expansion:
// - Blank line detection between methods
// - Comment-forced expansion
// - Complex argument detection
// - Callback analysis

use crate::ast::internal::{ArrowFunctionBody, Expression};
use crate::printer::calls::arg_predicates::is_simple_call_argument;

use super::super::printing::node_comment_gap;
use super::super::types::{ChainGroup, ChainNode};
use crate::printer::Printer;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::printing::{self, has_blank_line_between_fast};

/// Check if there are blank lines BETWEEN methods (not just before the first method)
///
/// Prettier's blank line rules:
/// - Blank line before first method ONLY (no other blank lines) → try to fit inline
/// - Blank lines BETWEEN methods (groups[2+]) → force expand
///
/// Returns true only if there are blank lines after the first method (groups index >= 2),
/// which is when we should force the expanded layout.
pub(super) fn has_blank_lines_between_methods<'a>(
    groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> bool {
    let line_breaks = printer.get_layout_line_breaks();
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
pub(super) fn has_comments_forcing_expansion<'a>(
    groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> bool {
    for (group_idx, group) in groups.iter().enumerate() {
        let is_last_group = group_idx == groups.len() - 1;

        for (node_idx, node) in group.nodes.iter().enumerate() {
            // Skip the last member node in the last group - its comments are
            // handled inline via line_suffix, not by forcing expansion. That deferral
            // is the sanctioned collapse for a LONE same-line `//`
            // (`fn().bar; // c` — trailing_member_after_call_comment), so it must
            // survive here. Two exceptions, both "a `//` must end its line":
            //
            // - a computed member whose pre-bracket gap holds ANY line comment:
            //   `print_node_inner` emits a forced break in that gap rather than
            //   deferring (a deferred `//` would swallow the `[i]` printed after it —
            //   see its ComputedMember arm). The chain has to expand around that
            //   break — left flat, the hardline lands in the one-line variant and the
            //   whole chain renders unindented.
            // - a plain member whose gap holds an OWN-LINE line comment: deferring it
            //   discards the authored line and, behind a same-line `//`, welds the two
            //   into ONE comment (`fn().bar; // c3 // c4`), the second `//` becoming
            //   text inside the first. The chain expands and the gap emitters put each
            //   comment in place — the shape every longer chain already takes
            //   (trailing_member_short_chain_line_comment).
            let is_last_node_in_last_group =
                is_last_group && node_idx == group.nodes.len() - 1 && node.is_member();
            if is_last_node_in_last_group && !trailing_member_gap_line_comment(node, printer) {
                continue;
            }

            if let Some((obj_end, prop_start)) = node.comment_range()
                && printer.has_comments_to_emit_between(obj_end, prop_start)
            {
                return true;
            }
        }
    }
    false
}

/// Whether a trailing member's gap carries a line comment the chain must EXPAND around
/// — the exception to the last-member skip in [`has_comments_forcing_expansion`].
///
/// The bar differs by member kind, because what the flat layout does with the comment
/// differs:
///
/// - **computed** (`a.b()⏎// c⏎[0]`): ANY line comment forces — the gap is one a chain
///   builder never owns (a computed member with a numeric-literal index is glued into
///   the preceding call's group instead of starting one), and `print_node_inner` emits
///   a forced break for it, since a deferred `//` would swallow the `[i]` printed
///   after it.
/// - **plain** (`fn()⏎// c⏎.bar`): only an OWN-LINE line comment forces (own-line
///   against the gap's start — the object's printed end). A lone same-line `//` stays
///   on the flat path, whose `line_suffix` deferral past the member is the sanctioned
///   collapse (`fn().bar; // c`); an own-line `//` deferred the same way would lose
///   its authored line and weld behind a same-line one.
fn trailing_member_gap_line_comment<'a>(node: &ChainNode<'a>, printer: &Printer<'_>) -> bool {
    let Some((start, end)) = node_comment_gap(node, printer) else {
        return false;
    };
    if matches!(node, ChainNode::ComputedMember { .. }) {
        return printer.classify_comments(start, end).has_line_comments();
    }
    comments_to_emit_in_range(printer.comments, start, end)
        .any(|c| !c.is_block && printer.has_newline_between(start, c.span.start))
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

    for arg in call.arguments {
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
