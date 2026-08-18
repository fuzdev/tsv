// Chain analysis for TypeScript member chain formatting
//
// This module handles the analysis phase of chain formatting:
// - Linearization: Flatten nested AST into a flat list of ChainNodes
// - Grouping: Group nodes by natural break points
// - Merge decisions: Determine if first groups should be merged

use super::types::{ChainGroup, ChainGroupVec, ChainNode, ChainNodeVec};
use crate::ast::internal::{self, Expression, IdentName};
use crate::printer::comments::{paren_pair_keeps_leading_run, paren_shell_close_after};
use crate::printer::{ParenContext, Printer, needs_parens};
use tsv_lang::{Comment, TAB_WIDTH, has_line_comments_in_range};

//
// Linearization
//

/// What the linearizer reads off the INPUT, beyond the AST it walks: the source (to
/// locate the `)` of a pair a base owns) and the comment table (for the gates asking
/// whether a gap holds a `//`).
///
/// A struct rather than two parameters because every recursive step needs both and
/// neither is ever passed alone — the same reason `ParenLeadingValue` travels as one.
#[derive(Clone, Copy)]
pub struct LinearizeInput<'i> {
    pub source: &'i str,
    pub comments: &'i [Comment],
}

/// Linearize a chain expression into a flat list of nodes
///
/// Walks the AST bottom-up (like prettier's `rec()` function) to flatten
/// nested member/call chains into execution order.
///
/// Example: `a().b().c!.d` produces:
/// [Base(a), Call(), Member(.b), Call(), NonNull(!), Member(.d)]
///
/// For call chains with stripped grouping parens, extends member comment ranges
/// to cover paren gaps where block comments may live (mid-chain comment placement).
/// This only applies to call chains — prettier keeps comments at the chain start
/// for member-only chains.
/// General-purpose entry point (used by tests; production code uses typed entry points)
#[cfg(test)]
fn linearize_chain<'a>(expr: &'a Expression<'_>, input: LinearizeInput<'_>) -> ChainNodeVec<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_recursive(expr, input, &mut nodes, &mut paren_gaps);
    finalize_chain_nodes(&mut nodes, &paren_gaps);
    nodes
}

/// Linearize starting from a CallExpression (avoids cloning to wrap in Expression)
pub fn linearize_chain_from_call<'a>(
    call: &'a internal::CallExpression<'_>,
    input: LinearizeInput<'_>,
) -> ChainNodeVec<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_call_callee(call, input, &mut nodes, &mut paren_gaps);
    if call.optional {
        nodes.push(ChainNode::call_optional(call));
    } else {
        nodes.push(ChainNode::call(call));
    }
    finalize_chain_nodes(&mut nodes, &paren_gaps);
    nodes
}

/// Linearize starting from a MemberExpression (avoids cloning to wrap in Expression)
pub fn linearize_chain_from_member<'a>(
    member: &'a internal::MemberExpression<'_>,
    input: LinearizeInput<'_>,
) -> ChainNodeVec<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_member_object(member, input, &mut nodes, &mut paren_gaps);
    linearize_member_node(member, input.source, &mut nodes, &mut paren_gaps);
    finalize_chain_nodes(&mut nodes, &paren_gaps);
    nodes
}

/// Linearize starting from a TSNonNullExpression (avoids cloning to wrap in Expression)
///
/// The terminal `NonNull` push takes no comment gate: the OUTERMOST operand→`!`
/// gap's comments are handled by `build_ts_non_null_doc` before this entry is
/// reached (a `//` there makes `needs_parens` true, a block comment takes its
/// trailing-comment branch), so only the comment-free case arrives here. Nested
/// non-nulls go through `linearize_recursive`'s own arm, which does gate.
pub fn linearize_chain_from_non_null<'a>(
    non_null: &'a internal::TSNonNullExpression<'_>,
    input: LinearizeInput<'_>,
) -> ChainNodeVec<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_recursive(non_null.expression, input, &mut nodes, &mut paren_gaps);
    nodes.push(ChainNode::non_null(non_null));
    finalize_chain_nodes(&mut nodes, &paren_gaps);
    nodes
}

/// Apply deferred paren gap extensions to member nodes.
///
/// Only extends ranges for call chains — prettier places comments mid-chain
/// only when the chain contains calls.
fn apply_paren_gaps(nodes: &mut [ChainNode<'_>], paren_gaps: &[ParenGap]) {
    if !paren_gaps.is_empty() && nodes.iter().any(ChainNode::is_call) {
        for &(node_index, gap_start) in paren_gaps {
            if let Some(
                ChainNode::Member { object_end, .. }
                | ChainNode::PrivateMember { object_end, .. }
                | ChainNode::ComputedMember { object_end, .. },
            ) = nodes.get_mut(node_index)
            {
                *object_end = gap_start;
            }
        }
    }
}

/// A deferred paren gap extension: (node_index, gap_start)
type ParenGap = (usize, u32);

/// Finalize a freshly-linearized chain: apply deferred comment paren-gap
/// extensions, then re-evaluate the base node's parens for the callee case.
/// Shared by every linearization entry point so the two post-passes never
/// drift apart.
fn finalize_chain_nodes(nodes: &mut [ChainNode<'_>], paren_gaps: &[ParenGap]) {
    apply_paren_gaps(nodes, paren_gaps);
    fix_callee_base_parens(nodes);
    #[cfg(feature = "buffer_stats")]
    crate::printer::buffer_stats::record_chain_nodes(nodes.len());
}

/// Re-evaluate the base node's parens under `Callee` context when it is the
/// direct callee of the chain's first call.
///
/// The base node's parens were computed with `ChainBase` (member-object) rules
/// during linearization. A base that is *immediately* followed by a `Call`
/// node is actually that call's callee — e.g. `(() => 1)()` linearizes to
/// `[Base, Call]`, whereas `a.b()` has a `Member` between the base and the call
/// (`[Base, Member, Call]`), so its base stays a member object. A callee needs
/// the `Callee` rules so a function/arrow IIFE keeps its parens when the result
/// is member-accessed (`(function () {})().p`, `(() => 1)().p`), matching
/// prettier and the bare-callee path in `call_formatting.rs`.
fn fix_callee_base_parens(nodes: &mut [ChainNode<'_>]) {
    if let [
        ChainNode::Base {
            expr,
            needs_parens: np,
            ..
        },
        ChainNode::Call { .. },
        ..,
    ] = nodes
    {
        // A parenthesized optional-chain callee (`(a?.b)()`, `(a?.())()`) keeps its
        // parens — they terminate the chain so the call isn't absorbed into it.
        // The `Callee` rules don't model that boundary (it depends on the stripped
        // grouping parens, only knowable from the span gap during linearization),
        // so preserve the linearizer's decision instead of downgrading it. Such a
        // base is only ever produced by `linearize_call_callee`'s boundary check.
        if expr.has_optional_in_chain() {
            return;
        }
        // Callee always parenthesizes a binary operand for precedence, so the
        // for-init `in` rule never changes the verdict here — pass `false`.
        *np = needs_parens(expr, ParenContext::Callee, false);
    }
}

/// True when `child` (a member's object or a call's callee) is an optional chain
/// that source parens terminated, *and* the access applied to it is non-optional
/// (`(a?.b).c`, `(a?.b)()`). The grouping parens are stripped, so the only signal
/// is the span gap: the parent's span starts before the child's (it covers the
/// `(`). Such a child must stay a parenthesized base node — flattening it into the
/// chain would absorb the trailing access into the chain, dropping the
/// semantically-required parens and moving the short-circuit boundary (`(a?.b).c`
/// throws if `a` is null; `a?.b.c` short-circuits).
///
/// When the applied access is itself optional (`(a?.b)?.c`), the parens are
/// redundant — both forms short-circuit identically — so prettier strips them and
/// we let the chain flatten (`parent_optional` skips the boundary). The public-AST
/// converter still preserves acorn's nested `ChainExpression` for that case; this
/// is a printer-only normalization.
pub(crate) fn child_stops_optional_chain(
    parent_start: u32,
    parent_optional: bool,
    child: &Expression<'_>,
) -> bool {
    !parent_optional && parent_start < child.span().start && child.has_optional_in_chain()
}

/// The `(`→base gap a chain-forming node's own REQUIRED pair keeps INSIDE it —
/// `[node.span.start, base.span().start)` — or `None` where the node prints no such
/// pair, or prints one whose leading run is hoisted out in front (a cast, a
/// sequence, a ternary, an instantiation: prettier hoists there and tsv matches).
///
/// **One spelling of the question, read by three consumers**, which is the point of
/// its existing at all: the linearizer, which records it on the base node
/// (`ChainNode::Base`'s `paren_leading_start`); the chain's head, whose
/// `prepend_removed_paren_comments` must NOT claim a gap the base emits; and the
/// assignment layout gate, which must know the chain breaks INTERNALLY at that run
/// rather than leading the whole value with it. The three disagreeing is how the run
/// gets dropped, doubled, or double-indented.
///
/// The same position answer decides the pair's **trailing** gap — the per-shape
/// predicates below (`member_object_paren_leading_start`,
/// `call_callee_paren_leading_start`, `non_null_operand_paren_leading_start`, and
/// [`tag_paren_leading_start`] for the tagged template, which never linearizes) feed
/// `Printer::owned_pair_trailing_gap`, and every window that opens past the pair's `)`
/// reads the same one. A pair whose doc emits a trailing gap the window does not skip
/// double-prints; the reverse DROPS. It answered differently with and without a `!`
/// for as long as the trailing half was keyed on the operand's KIND instead.
pub fn chain_paren_leading_gap(expr: &Expression<'_>, comments: &[Comment]) -> Option<(u32, u32)> {
    match expr {
        Expression::MemberExpression(member) => member_object_paren_leading_start(member)
            .map(|start| (start, member.object.span().start)),
        Expression::CallExpression(call) => {
            call_callee_paren_leading_start(call).map(|start| (start, call.callee.span().start))
        }
        Expression::TSNonNullExpression(non_null) => {
            non_null_operand_paren_leading_start(non_null, comments)
                .map(|start| (start, non_null.expression.span().start))
        }
        _ => None,
    }
}

/// A member's object keeps the pair — and with it the leading run — when the object
/// is a parenthesized optional chain this access seals (`( // c⏎a?.b).ddd`).
pub(crate) fn member_object_paren_leading_start(
    member: &internal::MemberExpression<'_>,
) -> Option<u32> {
    child_stops_optional_chain(member.span.start, member.optional, member.object)
        .then_some(member.span.start)
}

/// A callee keeps the pair two ways: the sealed optional chain (`( // c⏎a?.b)()`),
/// and the IIFE — a function or arrow, the one callee kind prettier prints the run
/// inside the parens for ([`paren_pair_keeps_leading_run`]).
pub(crate) fn call_callee_paren_leading_start(call: &internal::CallExpression<'_>) -> Option<u32> {
    (child_stops_optional_chain(call.span.start, call.optional, call.callee)
        || (call.span.start < call.callee.span().start
            && paren_pair_keeps_leading_run(call.callee)))
    .then_some(call.span.start)
}

/// The tagged-template analog of [`call_callee_paren_leading_start`]: a tag's REQUIRED
/// pair keeps its gaps inside it at the same two shapes — a function/arrow tag (the
/// template half of prettier's IIFE-callee-or-tag rule) and a sealed optional chain.
///
/// A tagged template never enters the linearizer, so nothing records the returned `(`
/// on a node; the tag's own printer consumes it as its leading gap's open. Spelled here
/// beside its siblings, and in their shape, because it is the same question and the
/// family answering it in one place is the point ([`chain_paren_leading_gap`]).
pub(crate) fn tag_paren_leading_start(
    tagged: &internal::TaggedTemplateExpression<'_>,
) -> Option<u32> {
    (child_stops_optional_chain(tagged.span.start, false, tagged.tag)
        || (tagged.span.start < tagged.tag.span().start
            && paren_pair_keeps_leading_run(tagged.tag)))
    .then_some(tagged.span.start)
}

/// The two authorings that keep a non-null's operand a parenthesized base: a sealed
/// optional chain, and a shell RETAINED for a `//` in the operand→`!` gap — see the
/// linearizer's arm, whose condition this is.
fn non_null_operand_paren_leading_start(
    non_null: &internal::TSNonNullExpression<'_>,
    comments: &[Comment],
) -> Option<u32> {
    (non_null.seals_optional_chain()
        || has_line_comments_in_range(comments, non_null.expression.span().end, non_null.span.end))
    .then_some(non_null.span.start)
}

/// Push a sealed parenthesized-optional-chain object/callee as a base node.
///
/// The whole sealed child becomes the parenthesized base, preserving the author's
/// `!` position **inside** the parens (`(a?.b!).c` stays `(a?.b!).c`, not the lifted
/// `(a?.b)!.c`). Prettier 3.9 ([#18661](https://github.com/prettier/prettier/pull/18661))
/// stopped lifting the `!` outside the grouping parens; the two forms are
/// semantically identical (the `!` is a type-only assertion and the parens seal the
/// chain at the same runtime point), and they have *different* ESTree ASTs
/// (`ChainExpression(TSNonNull(…))` vs `TSNonNull(ChainExpression(…))`), so
/// preserving the author's form keeps tsv's output AST-faithful to the input.
/// The `!`-outside source (`(a?.b)!.c`) takes the `seals_optional_chain` arm in
/// `linearize_recursive` instead (it never reaches here), so it stays as written too.
///
/// `parent_start` is the `(` this pair prints — the enclosing member/call node's own
/// start — so the base carries its leading gap and emits it inside the parens
/// ([`chain_paren_leading_gap`]).
fn push_sealed_chain_base<'a>(
    child: &'a Expression<'_>,
    parent_start: u32,
    nodes: &mut ChainNodeVec<'a>,
) {
    nodes.push(ChainNode::sealed_base(child, parent_start));
}

fn linearize_recursive<'a>(
    expr: &'a Expression<'_>,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    match expr {
        // CallExpression: recurse into callee, then add Call node
        Expression::CallExpression(call) => {
            linearize_call_callee(call, input, nodes, paren_gaps);
            if call.optional {
                nodes.push(ChainNode::call_optional(call));
            } else {
                nodes.push(ChainNode::call(call));
            }
        }

        // MemberExpression: recurse into object, then add Member node
        Expression::MemberExpression(member) => {
            linearize_member_object(member, input, nodes, paren_gaps);
            linearize_member_node(member, input.source, nodes, paren_gaps);
        }

        // TSNonNullExpression: recurse into expression, then add NonNull node
        // TODO: a TSInstantiationExpression operand here (`(A<T>)!.x`) is recursed
        // transparently and loses its type args (no Call node recovers them, unlike
        // the call-callee path). Same root cause as the member-object case fixed via
        // linearize_member_object. Untested because prettier's parser rejects the
        // syntax, so there's no canonical source for a fixture.
        Expression::TSNonNullExpression(non_null) => {
            // Two authorings keep the whole operand a parenthesized base + `!` instead
            // of flattening it into the chain:
            // - a sealed parenthesized optional chain (`(a?.b)!.c`): the trailing
            //   access reached via this node's parent must not be absorbed, so it
            //   renders `(a?.b)!.c`, not `a?.b!.c`;
            // - a `//` in the operand→`!` gap (`(aaa // c⏎)!.bbb`), which can only come
            //   from a grouping shell — written bare, the `//` would swallow the `!`
            //   (`[no LineTerminator here]`). The shell is RETAINED for the comment's
            //   sake, emitted inside the parens (`build_paren_operand_comment_doc`'s
            //   line-comment layout), the same answer the standalone non-null gives
            //   this gap. Flattening instead hands the region to `NonNullGap::Bang`,
            //   whose emitter is block-only — the `//` would be dropped, with nothing
            //   left to parenthesize at print time.
            let inner = &non_null.expression;
            if let Some(start) = non_null_operand_paren_leading_start(non_null, input.comments) {
                nodes.push(ChainNode::paren_base_before_non_null(
                    inner,
                    start,
                    non_null.span.end,
                ));
                nodes.push(ChainNode::non_null_after_paren_operand());
            } else {
                linearize_recursive(inner, input, nodes, paren_gaps);
                // A comment from the stripped grouping parens (`(x + y /* c */)!.foo`)
                // lives between the operand and the `!`. When the operand is a
                // parenthesized base, keep the comment INSIDE the parens, where the
                // author wrote it, rather than dropping it.
                if let Some(ChainNode::Base {
                    needs_parens: true,
                    paren_comment_end,
                    followed_by_non_null,
                    ..
                }) = nodes.last_mut()
                {
                    *paren_comment_end = Some(non_null.span.end);
                    *followed_by_non_null = true;
                    nodes.push(ChainNode::non_null_after_paren_operand());
                } else {
                    nodes.push(ChainNode::non_null(non_null));
                }
            }
        }

        // TSInstantiationExpression as a call callee (`expr<T>(args)`): transparent.
        // The Call node recovers the type args via get_call_type_arguments() in
        // chain_args.rs, so the instantiation itself emits nothing here. Member
        // objects (`(A<T>).x`) take the `linearize_member_object` path instead,
        // which keeps the type args and parens.
        Expression::TSInstantiationExpression(inst) => {
            linearize_recursive(inst.expression, input, nodes, paren_gaps);
        }

        // Base case: expression that's not part of the chain structure
        _ => {
            // ChainBase always parenthesizes a binary base for precedence, so
            // the for-init `in` rule never changes the verdict here — pass `false`.
            let needs_parens = needs_parens(expr, ParenContext::ChainBase, false);
            nodes.push(ChainNode::base(expr, needs_parens));
        }
    }
}

/// Linearize a MemberExpression's object.
///
/// Two objects must stay a parenthesized base node instead of recursing into the
/// chain:
/// - A parenthesized optional chain (`(a?.b).c`, `(a?.b!).c`) terminates the
///   chain — see `child_stops_optional_chain`; the base is built via
///   `push_sealed_chain_base` (which keeps the whole sealed child, `!` included,
///   inside the parens, preserving the author's form).
/// - A `TSInstantiationExpression` must keep its type args and be parenthesized:
///   `(A<T>).x`, not `A.x` (data loss) or `A<T>.x` (ambiguous). Prettier
///   parenthesizes an instantiation only when it is the object of a member
///   access, and no Call node follows here to recover dropped type args.
///
/// All other objects recurse normally.
fn linearize_member_object<'a>(
    member: &'a internal::MemberExpression<'_>,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    let object: &Expression<'_> = member.object;
    if let Some(start) = member_object_paren_leading_start(member) {
        push_sealed_chain_base(object, start, nodes);
    } else if matches!(object, Expression::TSInstantiationExpression(_)) {
        nodes.push(ChainNode::base(object, true));
    } else {
        linearize_recursive(object, input, nodes, paren_gaps);
    }
}

/// Linearize a CallExpression's callee.
///
/// A parenthesized optional chain callee (`(a?.b)()`) terminates the chain and
/// must stay a parenthesized base node — see `child_stops_optional_chain`. All
/// other callees recurse normally.
fn linearize_call_callee<'a>(
    call: &'a internal::CallExpression<'_>,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    if child_stops_optional_chain(call.span.start, call.optional, call.callee) {
        push_sealed_chain_base(call.callee, call.span.start, nodes);
        return;
    }
    linearize_recursive(call.callee, input, nodes, paren_gaps);
    // An IIFE callee reached through the chain (`( // c⏎() => {})().p`) owns its own
    // leading gap, exactly as the bare-callee path does: the pair is required and
    // prettier keeps the run inside it. A function or arrow is never itself a chain,
    // so `linearize_recursive` pushed exactly one node for it and that node is the
    // base this call's `(` belongs to.
    if let Some(start) = call_callee_paren_leading_start(call)
        && let Some(ChainNode::Base {
            paren_leading_start,
            ..
        }) = nodes.last_mut()
    {
        *paren_leading_start = Some(start);
    }
}

/// Process a MemberExpression node: handle paren gaps and push the appropriate ChainNode.
///
/// Extracted from `linearize_recursive` so it can be shared with `linearize_chain_from_member`.
fn linearize_member_node<'a>(
    member: &'a internal::MemberExpression<'_>,
    source: &str,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    // When grouping parens are stripped (e.g., `/* comment */ (a).b` → `/* comment */ a.b`),
    // the MemberExpression span extends earlier than its object span, creating a gap
    // where comments from the stripped parens live. Record the gap so we can extend
    // the last member node's comment range (only applied for call chains).
    let member_start = member.span.start;
    let object_start = member.object.span().start;
    if member_start < object_start {
        // Find the last member node in the sub-chain
        for i in (0..nodes.len()).rev() {
            match &nodes[i] {
                ChainNode::Member { .. }
                | ChainNode::PrivateMember { .. }
                | ChainNode::ComputedMember { .. } => {
                    paren_gaps.push((i, member_start));
                    break;
                }
                ChainNode::Base { .. } => break,
                _ => continue,
            }
        }
    }

    // The object's own pair emits its trailing gap where the object is a sealed
    // optional chain (`(a?.b /* t */).c`) — the same position that keeps the pair's
    // LEADING run inside it, asked through the same predicate. This seam then opens past
    // that `)`, so the comment is not emitted a second time outside the parens
    // (`docs/comments.md` hazard 3), while a comment the author wrote AFTER the `)`
    // (`(a?.b) /* t */.c`) still belongs here.
    let operand_end = member.object.span().end;
    let object_end = Printer::gap_start_after_owned_pair(
        operand_end,
        member_object_paren_leading_start(member)
            .and_then(|_| paren_shell_close_after(source, operand_end))
            .map(|close| (operand_end, close)),
    );
    let property_start = member.property.span().start;
    if member.computed {
        nodes.push(ChainNode::computed_member(
            member.property,
            member.optional,
            object_end,
            member.span.end,
        ));
    } else if let Expression::Identifier(id) = member.property {
        nodes.push(ChainNode::member(
            id.ident_name(),
            member.optional,
            object_end,
            property_start,
        ));
    } else if let Expression::PrivateIdentifier(pid) = member.property {
        nodes.push(ChainNode::private_member(
            pid.name,
            member.optional,
            object_end,
            property_start,
            pid.name_span().start,
        ));
    } else {
        // Non-identifier property (shouldn't happen for non-computed)
        nodes.push(ChainNode::computed_member(
            member.property,
            member.optional,
            object_end,
            member.span.end,
        ));
    }
}

//
// Grouping
//

/// Group linearized chain nodes into logical groups
///
/// Follows prettier's grouping algorithm:
/// 1. First group: base + calls + non-null + numeric accessors + consecutive members
/// 2. Remaining groups: members* + calls*, break when seeing memberish after call
///
/// Plus prettier's comment rule (member-chain.js: a node with a trailing comment
/// closes its group), scoped to the comment kind that carries a correctness stake:
/// a member whose gap holds a **line comment** starts a new group — the first
/// group's "consecutive members" run stops ahead of it, a numeric accessor is not
/// glued into the base group past it, and phase 2 closes the current group before
/// it. What that buys is emitter routing, not layout: a member inside a group prints
/// through `print_member_access`, which can only DEFER a gap `//` to the line end
/// (`fn().bar.baz; // c` — where a second deferred `//` welds onto the first, and
/// an own-line comment loses its line), while a group's first member takes the
/// chain-level gap emitters, which break around the comment and keep it in place.
/// The trailing member's own deferral is a different, sanctioned matter
/// (`has_comments_forcing_expansion`).
pub fn group_chain_nodes<'a>(nodes: &[ChainNode<'a>], comments: &[Comment]) -> ChainGroupVec<'a> {
    if nodes.is_empty() {
        return ChainGroupVec::new();
    }

    // The grouped chain is built on the stack (`ChainGroupVec`): short chains —
    // the common case — never touch the heap; longer chains spill.
    let mut groups: ChainGroupVec<'a> = ChainGroupVec::new();
    let mut current = ChainGroup::new();
    let mut i = 0;

    // First node always goes into first group
    current.push(nodes[0]);
    i += 1;

    // Phase 1: Build first group
    // Add: calls, non-null, numeric accessors to first group
    while i < nodes.len() {
        let node = &nodes[i];
        if (node.is_call() || node.is_non_null() || node.is_numeric_accessor())
            && !gap_has_line_comment(node, comments)
        {
            current.push(nodes[i]);
            i += 1;
        } else {
            break;
        }
    }

    // If first node wasn't a call, add consecutive members
    // (but not the last one - that stays with subsequent calls)
    if !nodes[0].is_call() {
        while i + 1 < nodes.len()
            && nodes[i].is_member()
            && nodes[i + 1].is_member()
            && !gap_has_line_comment(&nodes[i], comments)
        {
            current.push(nodes[i]);
            i += 1;
        }
    }

    groups.push(current);
    current = ChainGroup::new();

    // Phase 2: Build remaining groups
    // Pattern: (members)* (calls)*, break at memberish after call — or at a member
    // whose gap holds a line comment (see above)
    let mut seen_call = false;

    while i < nodes.len() {
        let node = &nodes[i];

        // When we've seen a call and encounter a member, start a new group — or when
        // the member's gap holds a line comment (only a member has a gap)
        if (seen_call && node.is_member() && !node.is_numeric_accessor())
            || gap_has_line_comment(node, comments)
        {
            if !current.is_empty() {
                groups.push(current);
                current = ChainGroup::new();
            }
            seen_call = false;
        }

        // Track if we've seen a call
        if node.is_call() {
            seen_call = true;
        }

        current.push(nodes[i]);
        i += 1;
    }

    // Don't forget the last group
    if !current.is_empty() {
        groups.push(current);
    }

    #[cfg(feature = "buffer_stats")]
    {
        crate::printer::buffer_stats::record_chain_groups(groups.len());
        for group in &groups {
            crate::printer::buffer_stats::record_group_nodes(group.nodes.len());
        }
    }

    groups
}

//
// Merge Logic
//

/// Check if first two groups should be merged (factory pattern)
///
/// Corresponds to prettier's `shouldMerge` logic:
/// - `Object.keys(items).filter()` → merge "Object" + ".keys()" on first line
/// - `_.values(obj).map()` → merge "_" + ".values()" on first line
pub fn should_merge_first_groups<'a>(groups: &[ChainGroup<'a>], printer: &Printer<'_>) -> bool {
    if groups.len() < 2 {
        return false;
    }

    // Prettier refuses the merge when the second group's first node carries a
    // comment (`!hasComment(groups[1][0].node)`); tsv asks it of the kind that
    // matters — a merged member prints through `print_member_access`, which defers a
    // gap `//` to the line end (see `group_chain_nodes`), so a line comment there
    // needs the unmerged path's chain-level emitter.
    if gap_has_line_comment(&groups[1].nodes[0], printer.comments) {
        return false;
    }

    should_not_wrap(groups, printer)
}

/// Whether a chain node's comment gap (see [`ChainNode::comment_range`]) holds a
/// line comment — the grouping's comment question. Nodes without a gap (a base, a
/// call, a `!`) answer `false`.
///
/// ⚠️ This is the RAW range, not the printer's narrowed
/// [`node_comment_gap`](super::printing::node_comment_gap): for a computed member the
/// latter cuts the gap at the `[`, and it cannot be asked here — it reads
/// `Printer::chain_has_comments`, which `build_chain_doc` sets *after* grouping. So a
/// `//` written INSIDE the brackets (`arr.foo()[ // c⏎0]`) also starts a group. That
/// is the wanted answer rather than a tolerated one: the comment still occupies the
/// line the accessor would otherwise be glued onto, and the trailing group's own
/// pre-bracket break is what keeps it off the index (pinned by
/// `member/computed_leading_line_comment`). Take this asymmetry as deliberate before
/// "fixing" the two spellings into one.
fn gap_has_line_comment(node: &ChainNode<'_>, comments: &[Comment]) -> bool {
    node.comment_range()
        .is_some_and(|(start, end)| has_line_comments_in_range(comments, start, end))
}

/// Check if chain should NOT wrap between first and second groups
///
/// Corresponds to prettier's `shouldNotWrap` logic:
/// - Single base that's `this`, factory identifier, or short name (in expression statement)
/// - Multiple nodes where last is member with factory property
pub fn should_not_wrap<'a>(groups: &[ChainGroup<'a>], printer: &Printer<'_>) -> bool {
    if groups.len() < 2 {
        return false;
    }

    let first = &groups[0];
    let has_computed = groups[1].nodes.first().is_some_and(ChainNode::is_computed);

    if first.nodes.len() == 1 {
        // Single node in first group - must be a Base
        let ChainNode::Base { expr, .. } = &first.nodes[0] else {
            return false;
        };

        match expr {
            // super.method() → merge
            Expression::Super(_) => true,

            // this.method() → merge
            Expression::ThisExpression(_) => true,

            // Object.keys() → merge (capital letter = factory)
            // d3.scale() → merge (short name ≤ tabWidth in expression statement context only)
            Expression::Identifier(id) => {
                is_factory_name(id.ident_name(), id.span.start, printer)
                    || has_computed
                    || (printer.is_expression_statement()
                        && is_short_name(id.ident_name(), id.span.start, printer))
            }

            _ => has_computed,
        }
    } else {
        // Multiple nodes in first group: check if last is member with factory property
        if let Some((prop, prop_start)) = first.nodes.last().and_then(ChainNode::property) {
            return is_factory_name(prop, prop_start, printer) || has_computed;
        }
        false
    }
}

/// Check if an identifier name is short (≤ tabWidth)
///
/// Short names like `a`, `b`, `fn` get merged with their first call.
/// Only applies in expression statement context (per Prettier's logic).
///
/// Prettier ref: `isShort` in print/member-chain.js:284
/// Uses `name.length <= options.tabWidth` (JS .length, ASCII-only in practice)
fn is_short_name(name: IdentName<'_>, name_start: u32, printer: &Printer<'_>) -> bool {
    printer.with_ident_name_at(name, name_start, |name| name.len() <= TAB_WIDTH)
}

/// Check if an identifier name is a factory pattern.
///
/// Factory names get merged with their first call in chain formatting.
/// Matches Prettier's `isFactory`: `/^[A-Z]|^[$_]+$/u` (member-chain.js:273)
/// - Starts with uppercase: `Object`, `React`, `Observable`
/// - Pure `$`/`_` identifiers: `$`, `_`, `$_`, `$__` (lodash-style)
fn is_factory_name(name: IdentName<'_>, name_start: u32, printer: &Printer<'_>) -> bool {
    printer.with_ident_name_at(
        name,
        name_start,
        crate::printer::expressions::literals::is_factory_identifier_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::internal::{CallExpression, Identifier, MemberExpression};
    use bumpalo::Bump;
    use tsv_lang::Span;

    /// Helper to create an identifier expression. Tests fabricate spans with no
    /// backing source, so the name rides the escaped channel (an `&'arena str`
    /// resolved directly, regardless of span).
    fn make_identifier<'arena>(name: &'arena str) -> Expression<'arena> {
        let len = name.len() as u32;
        let ident_name = IdentName {
            escaped: Some(name),
            raw_len: 0,
        };
        Expression::Identifier(Identifier::simple(ident_name, Span::new(0, len)))
    }

    /// Helper to create a member expression: object.property
    fn make_member<'arena>(
        arena: &'arena Bump,
        object: Expression<'arena>,
        property_name: &'arena str,
        object_end: u32,
    ) -> Expression<'arena> {
        let prop_name = IdentName {
            escaped: Some(property_name),
            raw_len: 0,
        };
        let property_start = object_end + 1; // after the dot
        let span_end = property_start + property_name.len() as u32;
        Expression::MemberExpression(MemberExpression {
            object: arena.alloc(object),
            property: arena.alloc(Expression::Identifier(Identifier::simple(
                prop_name,
                Span::new(property_start, span_end),
            ))),
            computed: false,
            optional: false,
            span: Span::new(0, span_end),
        })
    }

    /// Helper to create a call expression: callee()
    fn make_call<'arena>(
        arena: &'arena Bump,
        callee: Expression<'arena>,
        callee_end: u32,
    ) -> Expression<'arena> {
        Expression::CallExpression(CallExpression {
            callee: arena.alloc(callee),
            arguments: &[],
            type_arguments: None,
            optional: false,
            span: Span::new(0, callee_end + "()".len() as u32),
        })
    }

    #[test]
    fn test_linearize_simple_identifier() {
        let expr = make_identifier("foo");

        let nodes = linearize_chain(
            &expr,
            LinearizeInput {
                source: "",
                comments: &[],
            },
        );

        assert_eq!(nodes.len(), 1);
        assert!(matches!(
            nodes[0],
            ChainNode::Base {
                needs_parens: false,
                ..
            }
        ));
    }

    #[test]
    fn test_linearize_member_chain() {
        let arena = Bump::new();
        // Build: a.b.c
        let a = make_identifier("a");
        let ab = make_member(&arena, a, "b", 1);
        let abc = make_member(&arena, ab, "c", 3);

        let nodes = linearize_chain(
            &abc,
            LinearizeInput {
                source: "",
                comments: &[],
            },
        );

        // Should produce: [Base(a), Member(.b), Member(.c)]
        assert_eq!(nodes.len(), 3);
        assert!(matches!(nodes[0], ChainNode::Base { .. }));
        assert!(matches!(nodes[1], ChainNode::Member { .. }));
        assert!(matches!(nodes[2], ChainNode::Member { .. }));
    }

    #[test]
    fn test_linearize_call_chain() {
        let arena = Bump::new();
        // Build: a().b()
        let a = make_identifier("a");
        let a_call = make_call(&arena, a, 1);
        let ab = make_member(&arena, a_call, "b", 3);
        let ab_call = make_call(&arena, ab, 5);

        let nodes = linearize_chain(
            &ab_call,
            LinearizeInput {
                source: "",
                comments: &[],
            },
        );

        // Should produce: [Base(a), Call(), Member(.b), Call()]
        assert_eq!(nodes.len(), 4);
        assert!(matches!(nodes[0], ChainNode::Base { .. }));
        assert!(nodes[1].is_call());
        assert!(nodes[2].is_member());
        assert!(nodes[3].is_call());
    }

    #[test]
    fn test_group_member_only_chain() {
        let arena = Bump::new();
        // Build: a.b.c.d
        let a = make_identifier("a");
        let ab = make_member(&arena, a, "b", 1);
        let abc = make_member(&arena, ab, "c", 3);
        let abcd = make_member(&arena, abc, "d", 5);

        let nodes = linearize_chain(
            &abcd,
            LinearizeInput {
                source: "",
                comments: &[],
            },
        );
        let groups = group_chain_nodes(&nodes, &[]);

        // For member-only chains, Prettier puts almost everything in first group
        // (all consecutive members except the last one if followed by more members)
        // In this case: [a.b.c, .d] or similar grouping
        assert!(!groups.is_empty());
        // First group contains base
        assert!(
            groups[0]
                .nodes
                .iter()
                .any(|n| matches!(n, ChainNode::Base { .. }))
        );
    }

    #[test]
    fn test_group_call_chain_breaks_after_call() {
        let arena = Bump::new();
        // Build: a().b().c
        let a = make_identifier("a");
        let a_call = make_call(&arena, a, 1);
        let ab = make_member(&arena, a_call, "b", 3);
        let ab_call = make_call(&arena, ab, 5);
        let abc = make_member(&arena, ab_call, "c", 7);

        let nodes = linearize_chain(
            &abc,
            LinearizeInput {
                source: "",
                comments: &[],
            },
        );
        let groups = group_chain_nodes(&nodes, &[]);

        // Grouping should break at member after call
        // Expected: [Base(a), Call()] [Member(.b), Call()] [Member(.c)]
        assert!(groups.len() >= 2, "Should have at least 2 groups");

        // First group contains base and its call
        assert!(
            groups[0]
                .nodes
                .iter()
                .any(|n| matches!(n, ChainNode::Base { .. }))
        );
        assert!(groups[0].nodes.iter().any(ChainNode::is_call));
    }

    #[test]
    fn test_group_empty_input() {
        let groups = group_chain_nodes(&[], &[]);
        assert!(groups.is_empty());
    }

    /// A line comment in `[start, start + 4)` — wide enough to fit inside the gaps
    /// the helper below hands out.
    fn make_line_comment(start: u32) -> Comment {
        Comment {
            content_span: Span::new(start + 2, start + 4),
            is_block: false,
            multiline: false,
            span: Span::new(start, start + 4),
            emit_character_field: false,
            bump_pattern_columns: false,
            owned_by_node: false,
        }
    }

    fn make_member_node<'arena>(
        name: &'arena str,
        object_end: u32,
        property_start: u32,
    ) -> ChainNode<'arena> {
        let ident_name = IdentName {
            escaped: Some(name),
            raw_len: 0,
        };
        ChainNode::member(ident_name, false, object_end, property_start)
    }

    /// The invariant the whole comment rule rests on: a member whose gap holds a line
    /// comment is a group's FIRST node, never an interior one — only a group's first
    /// member reaches the chain-level gap emitters, and a member printed inside a
    /// group can merely defer the `//` to the line end, where two of them weld.
    ///
    /// Exercises all three places the grouping could otherwise absorb it: phase 1's
    /// call/non-null/numeric run, phase 1's consecutive-members run, and phase 2's
    /// members-after-a-call run.
    #[test]
    fn test_group_splits_at_a_member_whose_gap_holds_a_line_comment() {
        let arena = Bump::new();
        let base = make_identifier("a");
        let call = make_call(&arena, make_identifier("a"), 1);
        let Expression::CallExpression(call) = &call else {
            panic!("make_call builds a CallExpression")
        };

        // a.b.c.d.e — every gap 10 bytes wide, so a comment can sit in any one
        let members: Vec<ChainNode<'_>> = (0..4)
            .map(|i| make_member_node("m", 20 + i * 10, 26 + i * 10))
            .collect();

        for (label, nodes) in [
            ("member-only", {
                let mut v = vec![ChainNode::base(&base, false)];
                v.extend(members.iter().copied());
                v
            }),
            ("after a call", {
                let mut v = vec![ChainNode::base(&base, false), ChainNode::call(call)];
                v.extend(members.iter().copied());
                v
            }),
        ] {
            for (commented, member) in members.iter().enumerate() {
                let (gap_start, _) = member.comment_range().unwrap();
                let comments = [make_line_comment(gap_start + 1)];
                let groups = group_chain_nodes(&nodes, &comments);

                let mut found = false;
                for group in &groups {
                    for (idx, node) in group.nodes.iter().enumerate() {
                        if node.comment_range() != member.comment_range() {
                            continue;
                        }
                        found = true;
                        assert_eq!(
                            idx, 0,
                            "{label}: member {commented} with a `//` in its gap must \
                             start its group, not sit at index {idx}"
                        );
                    }
                }
                assert!(
                    found,
                    "{label}: member {commented} vanished from the grouping"
                );
            }
        }
    }
}
