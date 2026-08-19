// Chain expansion analysis helpers
//
// Pure functions for determining when chains should force expansion:
// - Blank line detection between methods
// - Comment-forced expansion
// - Complex argument detection
// - Callback analysis

use crate::ast::internal::{ArrowFunctionBody, Expression};
use crate::printer::calls::arg_predicates::is_simple_call_argument;

use super::super::printing::{chain_gap_any, node_comment_gap};
use super::super::types::{ChainGroup, ChainNode};
use crate::printer::Printer;
use tsv_lang::comments_on_page_in_range;
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
        let Some(node) = group.nodes.first() else {
            return false;
        };
        // Hole-honoring ([`chain_gap_any`]): a blank a widened range sweeps up sits INSIDE
        // the object — between two call arguments, say — and is not a blank between methods.
        node.comment_range().is_some_and(|gap| {
            chain_gap_any(gap, node.paren_gap_skip(), |obj_end, prop_start| {
                has_blank_line_between_fast(line_breaks, obj_end, prop_start)
            })
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
    chain_end: u32,
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
            //   (trailing_member_short_chain_line_comment). A SAME-LINE `//` joins it
            //   whenever something else would reach the line its deferral takes — a
            //   comment behind it in the same gap (`fn()// c⏎/* c1 */⏎.bar`, which the
            //   flat path would REORDER) or a trailer past the chain
            //   (`fn()// c⏎.bar; // c1`, which would WELD) — see
            //   `trailing_member_gap_line_comment`.
            let is_last_node_in_last_group =
                is_last_group && node_idx == group.nodes.len() - 1 && node.is_member();
            if is_last_node_in_last_group
                && !trailing_member_gap_line_comment(node, chain_end, printer)
            {
                continue;
            }

            // Hole-honoring ([`chain_gap_any`]): the comments a widened range sweeps up
            // belong to the object subtree's own printers or to the chain head, and
            // expanding this chain around one is a layout the un-widened spelling of the
            // same document does not get.
            if let Some(gap) = node.comment_range()
                && chain_gap_any(gap, node.paren_gap_skip(), |start, end| {
                    printer.has_comments_to_emit_between(start, end)
                })
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
/// - **computed** (`a.b()⏎// c⏎[0]`): ANY line comment forces — the trailing group
///   prints with no chain-level break of its own, so `print_node_inner` emits a
///   forced break for the gap, since a deferred `//` would swallow the `[i]` printed
///   after it; the chain must expand around that break (left flat, the hardline
///   lands in the one-line variant and renders unindented).
/// - **plain** (`fn()⏎// c⏎.bar`): an OWN-LINE line comment forces (own-line against
///   the gap's start — the object's printed end). A lone same-line `//` stays on the
///   flat path, whose `line_suffix` deferral past the member is the sanctioned
///   collapse (`fn().bar; // c`); an own-line `//` deferred the same way would lose
///   its authored line and weld behind a same-line one.
///
/// The collapse's licence is that the deferral is **lossless**, and it stops exactly
/// where that stops — so a SAME-LINE `//` forces too, in the two places something else
/// can reach the line it is about to take:
///
/// 1. **another comment in the same gap**, behind it. The flat path emits the follower
///    *inline* while the `//` is deferred, so the run comes out REORDERED and the
///    follower loses its authored line (`fn() // c1⏎/* c2 */⏎.bar` →
///    `fn() /* c2 */.bar; // c1`). Asked on the **on-page** axis: a follower glued to
///    the property is *owned* by it — printed by the member's own doc rather than by
///    this gap, which does not spare it from landing ahead of the deferred comment
///    (trailing_member_gap_comment_run).
/// 2. **a trailer past the chain**, read from the chain's source end through any
///    closing punctuation (`fn()// c⏎.bar⏎); // c1` — the expanded layout's own
///    reprint puts `);` on a later line, so a same-line read would collapse it back on
///    pass two), in-source axis. Both would flush at one line end, welding
///    (`fn().bar; // c // c1`, the second `//` becoming text of the first) or
///    reordering past a block (`fn().bar; /* c1 */ // c`). Conservative by design: a
///    layout break between the two only makes the expansion unneeded, never wrong
///    (trailing_member_gap_comment_statement_trailer).
fn trailing_member_gap_line_comment<'a>(
    node: &ChainNode<'a>,
    chain_end: u32,
    printer: &Printer<'_>,
) -> bool {
    let Some((start, end)) = node_comment_gap(node, printer) else {
        return false;
    };
    if matches!(node, ChainNode::ComputedMember { .. }) {
        return printer.classify_comments(start, end).has_line_comments();
    }
    // ON-PAGE, not to-emit: the question is what will OCCUPY the line, and an owned
    // block glued to the property (`fn() // c⏎/* c2 */.bar`) occupies it just as much
    // as an emitted one — it is merely printed by the member's own doc instead of by
    // this gap, which does not spare it from landing ahead of the deferred `//`.
    let mut deferred_line_seen = false;
    for c in comments_on_page_in_range(printer.comments, start, end) {
        // Anything at all BEHIND a same-line `//` in this gap: the flat path emits the
        // follower inline while the `//` is deferred to the line end, so the run comes
        // out REORDERED and the follower loses its authored line. The line-comment
        // spelling of this is the own-line arm below; a block falls through to here.
        if deferred_line_seen {
            return true;
        }
        if c.is_block {
            continue;
        }
        if printer.has_newline_between(start, c.span.start) {
            return true;
        }
        deferred_line_seen = true;
    }
    // The trailer read asks about the CHAIN's end, not the comment's, so it is
    // loop-invariant — and only a same-line `//` that survived the scan has anything to
    // ask it. Kept out of the loop, it runs at most once per chain build, and never at
    // all for the authorings the arms above already answer.
    deferred_line_seen && printer.trailer_follows_through_closers(chain_end)
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
