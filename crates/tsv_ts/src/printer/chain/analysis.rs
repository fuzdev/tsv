// Chain analysis for TypeScript member chain formatting
//
// This module handles the analysis phase of chain formatting:
// - Linearization: Flatten nested AST into a flat list of ChainNodes
// - Grouping: Group nodes by natural break points
// - Merge decisions: Determine if first groups should be merged

use super::builder::{build_chain_doc, mark_long_curried_calls};
use super::printing::chain_gap_share_any;
use super::types::{ChainGroup, ChainGroupVec, ChainNode, ChainNodeVec};
use crate::ast::internal::{self, Expression, ExpressionKind, IdentName};
use crate::printer::calls::is_memberish;
use crate::printer::comments::{paren_pair_keeps_leading_run, paren_shell_close_after};
use crate::printer::{
    ParenContext, Printer, SecondTypeArgs, SpanMarks, instantiation_keeps_pair_before_type_args,
    is_called_function, is_multiline_template_expression, needs_parens,
};
use smallvec::SmallVec;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::has_newline_before_position;
use tsv_lang::{Comment, Span, TAB_WIDTH, has_line_spanning_comments_to_emit_in_range};

//
// Linearization
//

/// What the linearizer reads beyond the AST it walks: the printer — its source (to locate the
/// `)` of a pair a base owns), its comment table (for the gates asking whether a gap holds a
/// `//`), its withheld clarity pairs (for a base that is a called function expression, whose
/// pair is one — `Printer::withheld_clarity_pair_targets`), and where a member's gap opens
/// ([`Printer::member_gap_start`]) — and the heads that keep an authored shell behind
/// `typeof` or a unary `-` in a comma-list region (`Printer::region_kept_shell_targets`): a
/// base, a member object, a callee or a non-null operand, each printed as one parenthesized
/// base.
///
/// A struct rather than separate parameters because every step of the walk needs the
/// printer and none is ever passed alone — the same reason `ParenLeadingValue` travels as
/// one. Two words: it is copied down every step of the walk.
#[derive(Clone, Copy)]
pub struct LinearizeInput<'i> {
    pub(crate) printer: &'i Printer<'i>,
    /// The heads that keep the author's shell behind a `typeof` or a unary `-` in a
    /// comma-list region (`Printer::region_kept_shell_targets`), `None` in the document
    /// with no region marks, so an ordinary chain probes no set.
    pub(crate) region_kept_shells: Option<&'i SpanMarks>,
}

/// Linearize a chain expression into a flat list of nodes
///
/// Walks the AST bottom-up (like prettier's `rec()` function) to flatten
/// nested member/call chains into execution order.
///
/// Example: `a().b().c!.d` produces:
/// [Base(a), Call(), Member(.b), Call(), NonNull(!), Member(.d)]
///
/// With stripped grouping parens, extends member comment ranges to cover paren gaps
/// where block comments may live (mid-chain comment placement), where prettier's member
/// chain reaches the member and the order allows it ([`apply_paren_gaps`]) — elsewhere
/// prettier keeps comments at the chain start.
/// General-purpose entry point (used by tests; production code uses typed entry points)
#[cfg(test)]
fn linearize_chain<'a>(expr: &'a Expression<'_>, input: LinearizeInput<'_>) -> ChainNodeVec<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_spine(expr, input, &mut nodes, &mut paren_gaps);
    let _ = finalize_chain_nodes(&mut nodes, &paren_gaps, input);
    nodes
}

// The three typed entry points below fill a caller-owned `ChainNodeVec` — the `&mut` the
// walkers already take — rather than returning one. A `ChainNodeVec` is an inline
// array of eight 56-byte `ChainNode`s and a chain holds ~3 (p50 = 3, mean 2.82 over 40,239
// chains a fuz_app pass — `buffer_sizes`), so a by-value return copies the WHOLE array,
// length-independently, into a second slot the caller reserves to receive it; the caller's
// local is where the buffer has to live either way. That slot lands in
// `build_expression_doc_full_dispatch`, on the expression recursion cycle, where every level
// pays.
//
// ⚠️ Each one FILLS the buffer; none of them appends to it. The buffer arriving EMPTY is a
// precondition, because `finalize_chain_nodes` is three WHOLE-SLICE passes over one chain:
// `fix_callee_base_parens` matches `[Base, Call, ..]` from index 0, and `mark_own_call_layout`
// walks in reverse carrying `entered` across every node it meets. A stale prefix would defeat
// the base-paren match and silently re-mark an earlier chain's calls — a mis-LAYOUT, not a
// crash. Today every caller constructs its buffer immediately above the call, so it is
// unreachable; the natural next step from this shape (one warm buffer reused across chains)
// is what would reach it.
//
// ⚠️ It is a `debug_assert!`, NOT the release-unreachable spelling `OutputBuffer::
// as_empty_render_target` uses for the same class of hazard, and the difference is measured,
// not stylistic: a `nodes.clear()` at these three entry points costs `instructions:u`
// **+0.018% / +0.022%** (fuz_app / gro, means of 12 runs a side, per-side spread ≤0.013%).
// `SmallVec::truncate` has to re-derive the inline-vs-spilled triple before it can store a
// length, and this runs 40,239 times a pass. The debug assert is free in release and fires
// on every chain the test suite builds.
//
// ⚠️ Those two numbers replace a **+0.08…+0.32%** first reading, and the correction is
// worth more than the datum: at the time, the format board carried a ~0.4% per-exec
// lottery from `DocArena`'s static-node cache (see `tsv_lang::doc::arena`'s
// `STATIC_CACHE_SLOTS`), so this edit's real 0.02% sat two decades under the noise and
// the binary's collision draw was read as its cost. The doctrine that came out of it —
// make the hazard unreachable *where that is free*, not everywhere — survives the
// correction; the magnitude behind it did not.

/// Linearize starting from a CallExpression (avoids cloning to wrap in Expression)
///
/// Returns whether the chain holds a long-curried pair ([`finalize_chain_nodes`]).
pub fn linearize_chain_from_call_into<'a>(
    call: &'a internal::CallExpression<'_>,
    span: Span,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
) -> bool {
    debug_assert!(
        nodes.is_empty(),
        "the linearizer fills its buffer, never appends"
    );
    let mut paren_gaps = Vec::new();
    linearize_call_callee(call, span, input, nodes, &mut paren_gaps);
    nodes.push(ChainNode::call(call, span));
    finalize_chain_nodes(nodes, &paren_gaps, input)
}

/// Linearize starting from a MemberExpression (avoids cloning to wrap in Expression)
///
/// Returns whether the chain holds a long-curried pair ([`finalize_chain_nodes`]).
pub fn linearize_chain_from_member_into<'a>(
    member: &'a internal::MemberExpression<'_>,
    span: Span,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
) -> bool {
    debug_assert!(
        nodes.is_empty(),
        "the linearizer fills its buffer, never appends"
    );
    let mut paren_gaps = Vec::new();
    linearize_member_object(member, span, input, nodes, &mut paren_gaps);
    linearize_member_node(member, span, input, nodes, &mut paren_gaps);
    finalize_chain_nodes(nodes, &paren_gaps, input)
}

/// Linearize starting from a TSNonNullExpression (avoids cloning to wrap in Expression)
///
/// Returns whether the chain holds a long-curried pair ([`finalize_chain_nodes`]).
///
/// The terminal `NonNull` push takes no comment gate: the OUTERMOST operand→`!`
/// gap's comments are handled by `build_ts_non_null_doc` before this entry is
/// reached (a `//` there makes `needs_parens` true, a block comment takes its
/// trailing-comment branch), so only the comment-free case arrives here. Nested
/// non-nulls go through the walk's own step ([`non_null_step`]), which does gate.
pub fn linearize_chain_from_non_null_into<'a>(
    non_null: &'a internal::TSNonNullExpression<'_>,
    span: Span,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
) -> bool {
    debug_assert!(
        nodes.is_empty(),
        "the linearizer fills its buffer, never appends"
    );
    let mut paren_gaps = Vec::new();
    linearize_spine(non_null.expression, input, nodes, &mut paren_gaps);
    nodes.push(ChainNode::non_null(non_null, span));
    finalize_chain_nodes(nodes, &paren_gaps, input)
}

/// Apply deferred paren gap extensions to member nodes.
///
/// Only extends a range where prettier prints the shell's comment MID-CHAIN. Prettier
/// attaches a comment written just inside a stripped `(` to the object the parens held,
/// and prints a chain node's comments at its own lookup — but only inside
/// `printMemberChain`, which reaches a node only from a call rooting it
/// ([`in_member_chain_scope`]). An object outside that walk is printed by the member
/// printer, which prints its comments around the whole object, ahead of the chain:
/// `((/* a */ (x) /* b */ ().h)[x])` keeps `/* a */ x /* b */().h[x]` (no call follows
/// the `.h` that holds the shell), and so does `((/* a */ (x).h).t!())` (the call's callee
/// is a `!`, so no member chain starts there). Widening such a gap printed the shell's
/// comment after every comment of the object it leads, out of source order
/// (`x /* b */() /* a */.h[x]`), and where an enclosing construct claimed the same run
/// from the left spine (`typeof ((/* a */ (x).h).t!())`), twice. Left unwidened, the
/// stretch is the head's ([`HeadWindow::outer`]).
///
/// The widened range runs from the stripped `(` all the way to this node's property,
/// which sweeps up the object subtree sitting in between — whose own member accesses
/// already claim their comments. So the node also records that subtree as the region
/// its EMITTERS must skip ([`ChainNode::paren_gap_skip`]); the widened range itself
/// stays whole for the layout predicates.
fn apply_paren_gaps(nodes: &mut [ChainNode<'_>], paren_gaps: &[ParenGap], printer: &Printer<'_>) {
    for &(node_index, outer_index, gap_start, object_start) in paren_gaps {
        // The object the shell held is the node just ahead of its parent member.
        if !in_member_chain_scope(nodes, outer_index - 1) {
            continue;
        }
        // The relocation is taken only while it keeps the authored ORDER: it declines
        // where a comment in the skipped object subtree or in the member's own gap, up to
        // the lookup's opening token, would print between the shell's run and the member.
        // Everything in the object prints ahead of this member, so a comment there would
        // print before the shell's run, which the author wrote first
        // (`((/* a */ (/* b */ q).p)[x]).p()` printing `/* b */ q /* a */.p`, as prettier
        // does); and the member's gap buckets its comments by kind rather than by source
        // position (`(// a⏎x /* b */.p).m()` printing `x /* b */ // a`). A comment inside a
        // computed lookup's brackets prints after the run either way, so it does not count
        // (`(/* a */ (x)[/* b */ i]).m()` keeps `x /* a */[/* b */ i]`). Left unwidened, the
        // shell's run is the head's, ahead of the whole chain and so in source order, where
        // the spelling without the shell prints it (`// a⏎x /* b */.p.m()`).
        //
        // TODO: a comment in the run that the object OWNS (glued to its first token) still
        // prints at the head while the rest of the run widens, out of order
        // (`(/* a */ /* b */ x.y.p).q()` prints `/* b */ x.y /* a */.p`). Declining the
        // relocation there leaves an owned multi-line block printed inside the chain's first
        // group, which force-expands the chain on the first pass only
        // (`y = (// c⏎/*⏎ */ a.b).m(1).n(2).o(3)`, the same instability as
        // `y = (// c⏎(/*⏎ */ a).b.m(1).n(2).o(3))`), so it waits on that fix.
        let Some((natural_object_end, property_start)) = nodes[node_index].comment_range() else {
            continue;
        };
        let check_end = if nodes[node_index].is_computed() {
            super::printing::find_computed_bracket(
                printer.source,
                natural_object_end,
                property_start,
            )
            .open
        } else {
            property_start
        };
        if printer.has_comments_on_page_between(object_start, check_end) {
            continue;
        }
        if let Some(
            ChainNode::Member {
                object_end,
                paren_gap_skip,
                ..
            }
            | ChainNode::PrivateMember {
                object_end,
                paren_gap_skip,
                ..
            }
            | ChainNode::ComputedMember {
                object_end,
                paren_gap_skip,
                ..
            },
        ) = nodes.get_mut(node_index)
        {
            // `object_end` is still the natural one: the linearizer pushes a fresh
            // member node per call, so each index is targeted at most once.
            *paren_gap_skip = Some(Span::new(object_start, *object_end));
            *object_end = gap_start;
        }
    }
}

/// Whether prettier's `printMemberChain` prints the chain node at `index` as one of its
/// LINKS — the walk inside which a node's comments print at the node's own lookup rather
/// than ahead of the chain.
///
/// The walk starts at a call whose callee is a member (`printCallExpression`'s
/// `isMemberish(node.callee)`; the root's own comments print outside it), and descends
/// through members, `!`s, and calls whose callee is a member or a call, stopping at
/// anything else — the base, or a call whose callee is a `!`, which it prints whole. So a
/// node is a link when some call to its right roots a walk that reaches it.
fn in_member_chain_scope(nodes: &[ChainNode<'_>], index: usize) -> bool {
    // A call whose callee is a member or a call keeps a walk going through it.
    let continues =
        |call: usize| call > 0 && (nodes[call - 1].is_member() || nodes[call - 1].is_call());
    let mut active = false;
    for j in (index..nodes.len()).rev() {
        let node = &nodes[j];
        if active {
            active = match node {
                ChainNode::Member { .. }
                | ChainNode::PrivateMember { .. }
                | ChainNode::ComputedMember { .. }
                | ChainNode::NonNull { .. } => true,
                ChainNode::Call { .. } => continues(j),
                ChainNode::Base { .. } => false,
            };
            if j == index {
                return active;
            }
        }
        // A call with a member callee roots a walk of its own (below it, not at it).
        if !active && node.is_call() && j > 0 && nodes[j - 1].is_member() {
            active = true;
        }
    }
    false
}

/// A deferred paren gap extension: (node_index, outer_index, gap_start, object_start) —
/// the member to widen, the member whose object the parser stripped the `(` from (the
/// shell's parent, which prettier must reach inside `printMemberChain` for the shell's
/// comment to print mid-chain), the stripped `(`, and that object's start.
type ParenGap = (usize, usize, u32, u32);

/// The left-spine steps of the chain rooted at `root` whose stripped grouping shell the
/// chain prints MID-CHAIN: each a `(member start, object start)` pair — the member whose
/// object the parser stripped a `(` from, and that object — whose comments
/// [`apply_paren_gaps`] hands to an earlier member's widened gap
/// ([`ChainNode::paren_gap_skip`]) rather than to the head. Empty for a root that is not
/// a member or a call, and for a chain that widens nothing.
///
/// The answer IS the linearizer's: the chain is linearized here exactly as its printer
/// will linearize it. Asked by a walk that must not hand such a comment to a site above
/// the chain ([`crate::printer::Printer::left_spine_run_claim`]), where it would print
/// twice.
pub(crate) fn chain_mid_chain_paren_gaps(
    root: &Expression<'_>,
    input: LinearizeInput<'_>,
) -> SmallVec<[(u32, u32); 2]> {
    let mut nodes = ChainNodeVec::new();
    match &root.kind {
        ExpressionKind::CallExpression(call) => {
            let _ = linearize_chain_from_call_into(call, root.span, input, &mut nodes);
        }
        ExpressionKind::MemberExpression(member) => {
            let _ = linearize_chain_from_member_into(member, root.span, input, &mut nodes);
        }
        _ => return SmallVec::new(),
    }
    // A widened node's `object_end` is the stripped `(` (the member's start), and its skip
    // opens at that member's object.
    nodes
        .iter()
        .filter_map(|node| Some((node.comment_range()?.0, node.paren_gap_skip()?.start)))
        .collect()
}

/// Finalize a freshly-linearized chain: apply deferred comment paren-gap
/// extensions, then re-evaluate the base node's parens for the callee case.
/// Shared by every linearization entry point so the two post-passes never
/// drift apart.
///
/// Returns whether the chain holds a long-curried pair ([`mark_own_call_layout`]), which
/// every entry point hands back for [`build_linearized_chain_doc`].
fn finalize_chain_nodes(
    nodes: &mut [ChainNode<'_>],
    paren_gaps: &[ParenGap],
    input: LinearizeInput<'_>,
) -> bool {
    apply_paren_gaps(nodes, paren_gaps, input.printer);
    fix_callee_base_parens(nodes, input);
    let long_curried = mark_own_call_layout(nodes, input.printer.source);
    #[cfg(feature = "buffer_stats")]
    crate::printer::buffer_stats::record_chain_nodes(nodes.len());
    long_curried
}

/// Decide, per call node, whether prettier would print it through `printCallExpression` —
/// where the call-level layout rules live — or as a link its `printMemberChain` swallowed
/// ([`super::types::ChainCall::own_call_layout`]).
///
/// tsv routes a whole callee spine into ONE chain; prettier splits the same spine into
/// several `printCallExpression` invocations, and only the calls under an entered
/// `printMemberChain` lose those rules. Reconstructing that split is a single outer→inner
/// walk, mirroring `printCallExpression` + `printMemberChain`'s `rec` step for step:
///
/// - **Not yet entered.** The call is printed by `printCallExpression`, so it keeps its own
///   layout. It then enters the chain iff its callee is memberish — unless its own
///   sole-multiline-template argument PREEMPTS the redirect, which sits below that `if`
///   (`a.b(\`x⏎y\`).c(\`p⏎q\`)` — both hug, because the outer never reaches the redirect).
/// - **Entered.** `rec` swallows a call whose callee is memberish *or itself a call*; that
///   call's arguments go to `printCallArguments` and lose the rules. A call whose callee is
///   NEITHER is where `rec` stops and falls back to `print()` — prettier's chain BASE, which
///   re-enters `printCallExpression` and keeps them (`a(\`x⏎y\`).b()` hugs where
///   `a.b(\`x⏎y\`).c()` expands, and that is the whole difference between them).
///
/// ⚠️ Only the template preempt is modelled, and the other three in prettier's `if` are
/// **unreachable** here rather than omitted: a test call's callee must match a dotted
/// identifier path (`describe.only`), and `require`/`define`/`import()` are a bare
/// identifier or their own node — none of which can have a call beneath them, so none can
/// ever be the ancestor that swallows one.
///
/// Returns whether the walk met a long-curried pair: a call directly followed by a call that
/// takes at least one argument and fewer than it — exactly the pairs
/// [`mark_long_curried_calls`] decides, so a chain without one skips that pass.
fn mark_own_call_layout(nodes: &mut [ChainNode<'_>], source: &str) -> bool {
    let mut entered = false;
    let mut long_curried = false;
    // The argument count of the node after this one when that node is a call.
    let mut next_call_args = None;
    for node in nodes.iter_mut().rev() {
        let ChainNode::Call { call, facts, .. } = node else {
            next_call_args = None;
            continue;
        };
        long_curried |=
            next_call_args.is_some_and(|outer| outer > 0 && call.arguments.len() > outer);
        next_call_args = Some(call.arguments.len());
        if entered {
            // `rec` swallows a memberish-or-call callee and stops at anything else, printing
            // that one with `print()` — a fresh `printCallExpression` context for everything
            // below it.
            let swallowed = is_memberish(call.callee)
                || matches!(call.callee.kind, ExpressionKind::CallExpression(_));
            facts.own_call_layout = !swallowed;
            entered = swallowed;
        } else {
            facts.own_call_layout = true;
            entered = is_memberish(call.callee) && !template_preempts_chain_redirect(call, source);
        }
    }
    long_curried
}

/// Prettier's `isTemplateLiteralSingleArg` — a sole multiline template the author did NOT put
/// on a line of its own, the one condition in `printCallExpression`'s top `if` that a call
/// with a memberish callee can meet, and so the only thing that stops the
/// `printMemberChain` redirect below it.
fn template_preempts_chain_redirect(call: &internal::CallExpression<'_>, source: &str) -> bool {
    call.arguments.len() == 1
        && is_multiline_template_expression(call.arguments[0])
        && !has_newline_before_position(source, call.arguments[0].span().start)
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
///
/// The one `Callee` pair that is not owed is a function expression's, and the printer
/// withholds it behind a `>` that may close a type-argument region
/// (`Printer::withhold_clarity_pairs_behind_region_close`) — read here off the same
/// marks the bare-callee path reads, so the two paths print one callee one way.
fn fix_callee_base_parens(nodes: &mut [ChainNode<'_>], input: LinearizeInput<'_>) {
    let source = input.printer.source;
    if let [
        ChainNode::Base {
            expr,
            needs_parens: np,
            ..
        },
        ChainNode::Call { call, .. },
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
        // for-init `in` rule never changes the verdict here — pass `false`. An
        // instantiation callee ahead of the call's own type arguments keeps the pair the
        // bare-callee path keeps ([`instantiation_keeps_pair_before_type_args`]).
        let second_list = call.type_arguments.is_some()
            && instantiation_keeps_pair_before_type_args(source, expr, SecondTypeArgs::Expression);
        let callee_pair = needs_parens(expr, ParenContext::Callee, false)
            && !(is_called_function(expr)
                && input
                    .printer
                    .withheld_clarity_pair_targets
                    .is_marked(expr.span()));
        *np = callee_pair
            || second_list
            || input
                .region_kept_shells
                .is_some_and(|marks| marks.is_marked(expr.span()));
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
    match &expr.kind {
        ExpressionKind::MemberExpression(member) => {
            member_object_paren_leading_start(member, expr.span)
                .map(|start| (start, member.object.span().start))
        }
        ExpressionKind::CallExpression(call) => call_callee_paren_leading_start(call, expr.span)
            .map(|start| (start, call.callee.span().start)),
        ExpressionKind::TSNonNullExpression(non_null) => {
            non_null_operand_paren_leading_start(non_null, expr.span, comments)
                .map(|start| (start, non_null.expression.span().start))
        }
        _ => None,
    }
}

/// A member's object keeps the pair — and with it the leading run — when the object
/// is a parenthesized optional chain this access seals (`( // c⏎a?.b).ddd`).
pub(crate) fn member_object_paren_leading_start(
    member: &internal::MemberExpression<'_>,
    span: Span,
) -> Option<u32> {
    child_stops_optional_chain(span.start, member.optional, member.object).then_some(span.start)
}

/// A callee keeps the pair two ways: the sealed optional chain (`( // c⏎a?.b)()`),
/// and the IIFE — a function or arrow, the one callee kind prettier prints the run
/// inside the parens for ([`paren_pair_keeps_leading_run`]).
pub(crate) fn call_callee_paren_leading_start(
    call: &internal::CallExpression<'_>,
    span: Span,
) -> Option<u32> {
    (child_stops_optional_chain(span.start, call.optional, call.callee)
        || (span.start < call.callee.span().start && paren_pair_keeps_leading_run(call.callee)))
    .then_some(span.start)
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
    span: Span,
) -> Option<u32> {
    (child_stops_optional_chain(span.start, false, tagged.tag)
        || (span.start < tagged.tag.span().start && paren_pair_keeps_leading_run(tagged.tag)))
    .then_some(span.start)
}

/// The two authorings that keep a non-null's operand a parenthesized base: a sealed
/// optional chain, and a shell RETAINED for a `//` in the operand→`!` gap — see the
/// linearizer's arm, whose condition this is.
fn non_null_operand_paren_leading_start(
    non_null: &internal::TSNonNullExpression<'_>,
    span: Span,
    comments: &[Comment],
) -> Option<u32> {
    (non_null.seals_optional_chain(span)
        || has_line_spanning_comments_to_emit_in_range(
            comments,
            non_null.expression.span().end,
            span.end,
        ))
    .then_some(span.start)
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
/// [`non_null_step`] instead (it never reaches here), so it stays as written too.
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

/// One link of the chain spine [`linearize_spine_loop`] has descended through and not yet
/// pushed: the step that runs once everything below the link is linearized.
#[derive(Clone, Copy)]
enum PendingLink<'a> {
    /// A call: push its node, after handing an IIFE callee base its leading `(` where the
    /// walk descended into the callee ([`finish_call_callee`]).
    Call {
        call: &'a internal::CallExpression<'a>,
        span: Span,
        /// Whether the walk descended into the callee — `false` where the callee stopped it as
        /// a base of its own ([`call_callee_step`]).
        descended: bool,
    },
    /// A member: push its lookup node ([`linearize_member_node`]).
    Member {
        member: &'a internal::MemberExpression<'a>,
        span: Span,
    },
    /// A non-null assertion whose operand did not keep the pair the `!` closes
    /// ([`non_null_step`]): push its `!` ([`finish_non_null`]).
    NonNull {
        non_null: &'a internal::TSNonNullExpression<'a>,
        span: Span,
    },
}

/// The links [`linearize_spine_loop`] holds: a chain's rest past [`LINK_FRAMES`] links, the
/// first eight of it without spilling to the heap.
type PendingLinks<'a> = SmallVec<[PendingLink<'a>; 8]>;

/// The link frames [`linearize_spine`] recurses through before it walks the rest of a chain
/// in [`linearize_spine_loop`]: the common chain is a few links long and its recursion is
/// the cheaper walk, while a chain past them costs the stack of this many frames whatever
/// its length.
const LINK_FRAMES: u32 = 16;

/// Linearize the chain rooted at `expr` onto `nodes`, base first: a member, call or
/// non-null link pushes its node after everything below it, so the nodes come out in
/// execution order.
///
/// Each link runs in two halves: on the way down, its descent half pushes what stops the
/// walk there (a base of its own) or hands back the operand to walk; once everything below
/// it is linearized, its return half pushes the link's node, reading `nodes` as it then
/// stands (the last node, the last member node). The first [`LINK_FRAMES`] links recurse;
/// the rest of a longer chain is walked by [`linearize_spine_loop`], which runs the same
/// halves in the same order without a frame per link.
fn linearize_spine<'a>(
    expr: &'a Expression<'_>,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    linearize_link(expr, input, nodes, paren_gaps, LINK_FRAMES);
}

/// [`linearize_spine`]'s recursion: `expr`'s link, with `frames` link frames left before
/// the rest of the chain goes to [`linearize_spine_loop`].
fn linearize_link<'a>(
    expr: &'a Expression<'_>,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
    frames: u32,
) {
    let Some(frames) = frames.checked_sub(1) else {
        linearize_spine_loop(expr, input, nodes, paren_gaps);
        return;
    };
    match &expr.kind {
        ExpressionKind::CallExpression(call) => {
            if let Some(callee) = call_callee_step(call, expr.span, input, nodes) {
                linearize_link(callee, input, nodes, paren_gaps, frames);
                finish_call_callee(call, expr.span, nodes);
            }
            nodes.push(ChainNode::call(call, expr.span));
        }
        ExpressionKind::MemberExpression(member) => {
            if let Some(object) = member_object_step(member, expr.span, input, nodes) {
                linearize_link(object, input, nodes, paren_gaps, frames);
            }
            linearize_member_node(member, expr.span, input, nodes, paren_gaps);
        }
        ExpressionKind::TSNonNullExpression(non_null) => {
            match non_null_step(non_null, expr.span, input, nodes) {
                NonNullStep::Done => {}
                NonNullStep::Finish => finish_non_null(non_null, expr.span, nodes),
                NonNullStep::Descend(inner) => {
                    linearize_link(inner, input, nodes, paren_gaps, frames);
                    finish_non_null(non_null, expr.span, nodes);
                }
            }
        }
        // Base case: expression that's not part of the chain structure. A
        // `TSInstantiationExpression` lands here too — as a member object (`(A<T>).x`)
        // or a `!` operand (`(A<T>)!.x`) it is a parenthesized base by the `ChainBase`
        // rule, type args kept; only as a CALL callee is it transparent, and
        // `call_callee_step` unwraps it before descending.
        _ => push_chain_base(expr, input, nodes),
    }
}

/// The rest of a chain past [`LINK_FRAMES`] links, walked as a loop: it descends through the
/// links running each descent half and holding each return half ([`PendingLink`]) until a
/// base ends the descent, then runs the held halves innermost first — the order
/// [`linearize_link`]'s recursion returns in, so each half reads the `nodes` it would read
/// there. A link costs a held entry rather than a native stack frame.
#[inline(never)]
fn linearize_spine_loop<'a>(
    expr: &'a Expression<'_>,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    let mut pending = PendingLinks::new();
    let mut current = expr;
    loop {
        match &current.kind {
            ExpressionKind::CallExpression(call) => {
                let callee = call_callee_step(call, current.span, input, nodes);
                pending.push(PendingLink::Call {
                    call,
                    span: current.span,
                    descended: callee.is_some(),
                });
                match callee {
                    Some(callee) => current = callee,
                    None => break,
                }
            }
            ExpressionKind::MemberExpression(member) => {
                let object = member_object_step(member, current.span, input, nodes);
                pending.push(PendingLink::Member {
                    member,
                    span: current.span,
                });
                match object {
                    Some(object) => current = object,
                    None => break,
                }
            }
            ExpressionKind::TSNonNullExpression(non_null) => {
                match non_null_step(non_null, current.span, input, nodes) {
                    NonNullStep::Done => break,
                    NonNullStep::Finish => {
                        pending.push(PendingLink::NonNull {
                            non_null,
                            span: current.span,
                        });
                        break;
                    }
                    NonNullStep::Descend(inner) => {
                        pending.push(PendingLink::NonNull {
                            non_null,
                            span: current.span,
                        });
                        current = inner;
                    }
                }
            }
            // the base case, as in `linearize_link`
            _ => {
                push_chain_base(current, input, nodes);
                break;
            }
        }
    }
    while let Some(link) = pending.pop() {
        match link {
            PendingLink::Call {
                call,
                span,
                descended,
            } => {
                if descended {
                    finish_call_callee(call, span, nodes);
                }
                nodes.push(ChainNode::call(call, span));
            }
            PendingLink::Member { member, span } => {
                linearize_member_node(member, span, input, nodes, paren_gaps);
            }
            PendingLink::NonNull { non_null, span } => finish_non_null(non_null, span, nodes),
        }
    }
}

/// Push `expr`, which ends the walk, as the chain's base.
#[expect(clippy::inline_always)] // a half of both walks: each keeps its own copy
#[inline(always)]
fn push_chain_base<'a>(
    expr: &'a Expression<'_>,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
) {
    // ChainBase always parenthesizes a binary base for precedence, so
    // the for-init `in` rule never changes the verdict here — pass `false`.
    let needs_parens = needs_parens(expr, ParenContext::ChainBase, false)
        || input
            .region_kept_shells
            .is_some_and(|marks| marks.is_marked(expr.span()));
    nodes.push(ChainNode::base(expr, needs_parens));
}

/// What a non-null link does with its operand ([`non_null_step`]).
enum NonNullStep<'a> {
    /// The operand kept the pair the `!` closes: both nodes are pushed, nothing is left.
    Done,
    /// The operand is a base of its own, already pushed: the `!` is left
    /// ([`finish_non_null`]).
    Finish,
    /// The operand continues the chain: walk it, then push the `!`.
    Descend(&'a Expression<'a>),
}

/// The descent half of a non-null link: push what stops the walk at the operand, or hand
/// the operand back to walk.
#[expect(clippy::inline_always)] // a half of both walks: each keeps its own copy
#[inline(always)]
fn non_null_step<'a>(
    non_null: &'a internal::TSNonNullExpression<'_>,
    span: Span,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
) -> NonNullStep<'a> {
    // Two authorings keep the whole operand a parenthesized base + `!` instead
    // of flattening it into the chain:
    // - a sealed parenthesized optional chain (`(a?.b)!.c`): the trailing
    //   access reached via this node's parent must not be absorbed, so it
    //   renders `(a?.b)!.c`, not `a?.b!.c`;
    // - a comment that SPANS A LINE in the operand→`!` gap (`(aaa // c⏎)!.bbb`,
    //   `(aaa /* a⏎b */)!.bbb`), which can only come from a grouping shell —
    //   written bare, the `//` would swallow the `!` and the multi-line block
    //   would put a line break before it (`[no LineTerminator here]`). The shell
    //   is RETAINED for the comment's sake, emitted inside the parens
    //   (`build_paren_operand_comment_doc`'s expanded layout), the same answer
    //   the standalone non-null gives this gap. Flattening instead hands the
    //   region to `NonNullGap::Bang`, whose emitter is block-only — the `//`
    //   would be dropped and the block inlined into a dead output, with nothing
    //   left to parenthesize at print time.
    let inner = non_null.expression;
    if let Some(start) =
        non_null_operand_paren_leading_start(non_null, span, input.printer.comments)
    {
        nodes.push(ChainNode::paren_base_before_non_null(
            inner, start, span.end,
        ));
        nodes.push(ChainNode::non_null_after_paren_operand());
        NonNullStep::Done
    } else if input
        .region_kept_shells
        .is_some_and(|marks| marks.is_marked(inner.span()))
    {
        // A kept shell around a longer head (`typeof (c.d)!.e`), as at a
        // member object.
        nodes.push(ChainNode::base(inner, true));
        NonNullStep::Finish
    } else {
        NonNullStep::Descend(inner)
    }
}

/// The return half of a non-null link, once its operand is linearized: push its `!`.
#[expect(clippy::inline_always)] // a half of both walks: each keeps its own copy
#[inline(always)]
fn finish_non_null(
    non_null: &internal::TSNonNullExpression<'_>,
    span: Span,
    nodes: &mut ChainNodeVec<'_>,
) {
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
        *paren_comment_end = Some(span.end);
        *followed_by_non_null = true;
        nodes.push(ChainNode::non_null_after_paren_operand());
    } else {
        nodes.push(ChainNode::non_null(non_null, span));
    }
}

/// Linearize a MemberExpression's object.
///
/// A parenthesized optional chain (`(a?.b).c`, `(a?.b!).c`) terminates the chain —
/// see `child_stops_optional_chain` — and stays a sealed base node built via
/// `push_sealed_chain_base` (which keeps the whole sealed child, `!` included, inside
/// the parens, preserving the author's form). Every other object is walked on;
/// an instantiation object (`(A<T>).x`) becomes a parenthesized base there by the
/// `ChainBase` rule, its type args kept.
fn linearize_member_object<'a>(
    member: &'a internal::MemberExpression<'_>,
    span: Span,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    if let Some(object) = member_object_step(member, span, input, nodes) {
        linearize_spine(object, input, nodes, paren_gaps);
    }
}

/// The descent half of [`linearize_member_object`]: push the object where it stops the
/// walk as a base of its own, or hand it back to walk.
#[expect(clippy::inline_always)] // a half of both walks: each keeps its own copy
#[inline(always)]
fn member_object_step<'a>(
    member: &'a internal::MemberExpression<'_>,
    span: Span,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
) -> Option<&'a Expression<'a>> {
    let object: &Expression<'_> = member.object;
    if let Some(start) = member_object_paren_leading_start(member, span) {
        push_sealed_chain_base(object, start, nodes);
        None
    } else if input
        .region_kept_shells
        .is_some_and(|marks| marks.is_marked(object.span()))
    {
        // A kept shell around a longer head (`typeof (c.d).e`) stays one parenthesized
        // base: flattened, the pair would have no node to print on.
        nodes.push(ChainNode::base(object, true));
        None
    } else {
        Some(object)
    }
}

/// The instantiation a chain call's callee is, when the chain reads it as TRANSPARENT: the
/// call's node prints the instantiation's type arguments and the walk continues into
/// `inst.expression`, so the instantiation itself emits nothing.
///
/// `expr<T>(args)` parses to a call whose callee is the instantiation, while the canonical
/// tree puts `<T>` on the call. This is the one position where an instantiation is
/// transparent — reached as a member object or a `!` operand it is a parenthesized base.
/// A call with a list of its OWN (`f<T><U>(x).y`) has two, and the Call node prints only
/// its own, so there the instantiation stays the base and prints the first.
///
/// ⚠️ One predicate for every reader — the linearizer, the call's head doc (which prints
/// the list, and ahead of an optional call's `?.` when it comes from here) and
/// [`get_chain_base_start`]. A reader that answered on its own would print the list in the
/// wrong place or lose the comments between the expression and the `<`.
pub(crate) fn transparent_instantiation_callee<'a>(
    call: &'a internal::CallExpression<'a>,
) -> Option<&'a internal::TSInstantiationExpression<'a>> {
    match &call.callee.kind {
        ExpressionKind::TSInstantiationExpression(inst) if call.type_arguments.is_none() => {
            Some(inst)
        }
        _ => None,
    }
}

/// The sealed optional chain a transparent instantiation callee instantiates
/// (`(a?.b)<T>?.(c).d`), with the `(` of its pair — the instantiation's own start.
///
/// The call is optional, so [`child_stops_optional_chain`] never fires on the callee
/// itself, and the callee is the instantiation rather than the chain: the pair that
/// seals the chain belongs to the instantiation's OPERAND. Its `(` is the
/// instantiation's start, never the call's — a redundant pair around the whole
/// instantiation (`((a?.b)<T>)?.(c).d`) opens the call's span earlier, and a comment
/// between the two `(` belongs to the head, not inside the operand's pair.
///
/// ⚠️ One predicate for the linearizer (which keeps the chain a sealed base, its leading
/// run inside the pair) and the call's head (whose expression→`<` window opens past the
/// pair's `)`). The call-level readers — [`call_callee_paren_leading_start`] and
/// [`chain_paren_leading_gap`] — deliberately do NOT read it: theirs is the CALLEE's
/// pair, the one between the call and the instantiation, which here is no pair the
/// printer keeps. Answering for the operand's pair there would tell the hoisting walk the
/// call's own shell survives, so a comment the author wrote ahead of the instantiation
/// (`x = ( // c⏎(a?.b)<T>)?.(d).e`) would be neither hoisted nor hung, and would move on the next
/// pass; and the callee gap reads that pair's trailing gap past the callee's end, which
/// is the `>`.
pub(crate) fn transparent_callee_sealed_chain<'a>(
    call: &'a internal::CallExpression<'a>,
) -> Option<(&'a Expression<'a>, u32)> {
    let inst = transparent_instantiation_callee(call)?;
    let open = call.callee.span().start;
    child_stops_optional_chain(open, false, inst.expression).then_some((inst.expression, open))
}

/// Linearize a CallExpression's callee.
///
/// A parenthesized optional chain callee (`(a?.b)()`) terminates the chain and
/// must stay a parenthesized base node — see `child_stops_optional_chain` — and so
/// does one instantiated by a transparent instantiation callee
/// ([`transparent_callee_sealed_chain`]). All other callees are walked on.
fn linearize_call_callee<'a>(
    call: &'a internal::CallExpression<'_>,
    span: Span,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    if let Some(callee) = call_callee_step(call, span, input, nodes) {
        linearize_spine(callee, input, nodes, paren_gaps);
        finish_call_callee(call, span, nodes);
    }
}

/// The descent half of [`linearize_call_callee`]: push the callee where it stops the walk
/// as a base of its own, or hand back the expression to walk.
#[expect(clippy::inline_always)] // a half of both walks: each keeps its own copy
#[inline(always)]
fn call_callee_step<'a>(
    call: &'a internal::CallExpression<'_>,
    span: Span,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
) -> Option<&'a Expression<'a>> {
    if child_stops_optional_chain(span.start, call.optional, call.callee) {
        push_sealed_chain_base(call.callee, span.start, nodes);
        return None;
    }
    if let Some((chain, open)) = transparent_callee_sealed_chain(call) {
        push_sealed_chain_base(chain, open, nodes);
        return None;
    }
    // A kept shell around a longer callee (`typeof (c.d)()`), as at a member object.
    if input
        .region_kept_shells
        .is_some_and(|marks| marks.is_marked(call.callee.span()))
    {
        nodes.push(ChainNode::base(call.callee, true));
        return None;
    }
    // A transparent instantiation callee ([`transparent_instantiation_callee`]) emits
    // nothing itself: the Call node prints its type arguments, so the walk goes on into the
    // instantiated expression.
    Some(transparent_instantiation_callee(call).map_or(call.callee, |inst| inst.expression))
}

/// The return half of [`linearize_call_callee`], once the callee the walk descended into
/// is linearized.
#[expect(clippy::inline_always)] // a half of both walks: each keeps its own copy
#[inline(always)]
fn finish_call_callee(
    call: &internal::CallExpression<'_>,
    span: Span,
    nodes: &mut ChainNodeVec<'_>,
) {
    // An IIFE callee reached through the chain (`( // c⏎() => {})().p`) owns its own
    // leading gap, exactly as the bare-callee path does: the pair is required and
    // prettier keeps the run inside it. A function or arrow is never itself a chain,
    // so the walk pushed exactly one node for it and that node is the
    // base this call's `(` belongs to.
    if let Some(start) = call_callee_paren_leading_start(call, span)
        && let Some(ChainNode::Base {
            paren_leading_start,
            ..
        }) = nodes.last_mut()
    {
        *paren_leading_start = Some(start);
    }
}

/// The chain node for `member`'s own lookup — `.prop`, `.#prop` or `[index]`, optional or
/// not — positioned for its object→property gap, for the member printer that prints one
/// lookup after an operand it does not linearize ([`super::build_lookup_after_operand_doc`]).
///
/// Built by the linearizer's own step, over an empty run: the stripped-paren gap it would
/// record names an earlier member node, and there is none.
pub(super) fn member_lookup_node<'a>(
    member: &'a internal::MemberExpression<'_>,
    member_span: Span,
    input: LinearizeInput<'_>,
) -> ChainNode<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_member_node(member, member_span, input, &mut nodes, &mut paren_gaps);
    debug_assert!(paren_gaps.is_empty() && nodes.len() == 1);
    #[expect(clippy::unreachable)] // `linearize_member_node` always pushes the member's node
    let Some(node) = nodes.pop() else {
        unreachable!("a member linearizes to its own lookup node");
    };
    node
}

/// Process a MemberExpression node: handle paren gaps and push the appropriate ChainNode.
///
/// A step of `linearize_spine`, shared with
/// `linearize_chain_from_member_into`.
fn linearize_member_node<'a>(
    member: &'a internal::MemberExpression<'_>,
    member_span: Span,
    input: LinearizeInput<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    let source = input.printer.source;
    // When grouping parens are stripped (e.g., `/* comment */ (a).b` → `/* comment */ a.b`),
    // the MemberExpression span extends earlier than its object span, creating a gap
    // where comments from the stripped parens live. Record the gap so we can extend
    // the last member node's comment range (where `apply_paren_gaps` allows it).
    let member_start = member_span.start;
    let object_start = member.object.span().start;
    if member_start < object_start {
        // Find the last member node in the sub-chain
        for i in (0..nodes.len()).rev() {
            match &nodes[i] {
                ChainNode::Member { .. }
                | ChainNode::PrivateMember { .. }
                | ChainNode::ComputedMember { .. } => {
                    // This member is pushed next, at `nodes.len()`: the shell's parent.
                    paren_gaps.push((i, nodes.len(), member_start, object_start));
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
        member_object_paren_leading_start(member, member_span)
            .and_then(|_| paren_shell_close_after(source, operand_end))
            .map(|close| (operand_end, close)),
    );
    // A gap the object's own pair does not end opens at the object's printed end, claiming
    // the run of stripped shells a binary on its right spine prints after its last operand
    // ([`Printer::member_gap_start`]).
    let object_end = if object_end == operand_end {
        input.printer.member_gap_start(member.object)
    } else {
        object_end
    };
    let property_start = member.property.span().start;
    if member.computed {
        nodes.push(ChainNode::computed_member(
            member.property,
            member.optional,
            object_end,
            member_span.end,
        ));
    } else if let ExpressionKind::Identifier(id) = &member.property.kind {
        nodes.push(ChainNode::member(
            id.ident_name(),
            member.optional,
            object_end,
            property_start,
        ));
    } else if let ExpressionKind::PrivateIdentifier(pid) = &member.property.kind {
        nodes.push(ChainNode::private_member(
            pid.name,
            member.optional,
            object_end,
            property_start,
            pid.name_span(member.property.span).start,
        ));
    } else {
        // Non-identifier property (shouldn't happen for non-computed)
        nodes.push(ChainNode::computed_member(
            member.property,
            member.optional,
            object_end,
            member_span.end,
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
///
/// The grouping is a PARTITION into consecutive runs — `i` only ever advances and
/// every node joins the run it is scanned into — so each group is recorded as the
/// index where it opens and emitted as `&nodes[open..close]`. Nothing is copied.
pub fn group_chain_nodes<'a>(
    nodes: &'a [ChainNode<'a>],
    printer: &Printer<'_>,
) -> ChainGroupVec<'a> {
    if nodes.is_empty() {
        return ChainGroupVec::new();
    }

    // The grouped chain is built on the stack (`ChainGroupVec`): short chains —
    // the common case — never touch the heap; longer chains spill.
    let mut groups: ChainGroupVec<'a> = ChainGroupVec::new();
    // Where the group being scanned opens. A group is closed by pushing
    // `&nodes[open..i]`, after which `open` moves to `i`.
    let mut open = 0;
    // First node always goes into first group
    let mut i = 1;

    // Phase 1: Build first group
    // Add: calls, non-null, numeric accessors to first group
    while i < nodes.len() {
        let node = &nodes[i];
        if (node.is_call() || node.is_non_null() || node.is_numeric_accessor())
            && !gap_has_line_comment(node, printer)
        {
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
            && !gap_has_line_comment(&nodes[i], printer)
        {
            i += 1;
        }
    }

    groups.push(ChainGroup::new(&nodes[open..i]));
    open = i;

    // Phase 2: Build remaining groups
    // Pattern: (members)* (calls)*, break at memberish after call — or at a member
    // whose gap holds a line comment (see above)
    let mut seen_call = false;

    while i < nodes.len() {
        let node = &nodes[i];

        // When we've seen a call and encounter a member, start a new group — or when
        // the member's gap holds a line comment (only a member has a gap)
        if (seen_call && node.is_member() && !node.is_numeric_accessor())
            || gap_has_line_comment(node, printer)
        {
            if open < i {
                groups.push(ChainGroup::new(&nodes[open..i]));
                open = i;
            }
            seen_call = false;
        }

        // Track if we've seen a call
        if node.is_call() {
            seen_call = true;
        }

        i += 1;
    }

    // Don't forget the last group
    if open < i {
        groups.push(ChainGroup::new(&nodes[open..i]));
    }

    // The groups must TILE `nodes`: consecutive, gap-free, and covering it
    // exactly. Every consumer reads a group's nodes as the chain's own nodes in
    // source order, so a gap silently drops a link from the printed chain and an
    // overlap prints one twice — and both are invisible to the fixture suite on
    // any chain shape the corpus happens not to spell. Checked by pointer
    // identity (no deref), which catches a mis-stepped `open` that a length sum
    // alone would let through.
    #[cfg(debug_assertions)]
    {
        let mut offset = 0;
        for group in &groups {
            debug_assert!(
                std::ptr::eq(group.nodes.as_ptr(), nodes[offset..].as_ptr()),
                "chain group at node {offset} is not the next run of the linearized chain"
            );
            offset += group.nodes.len();
        }
        debug_assert_eq!(
            offset,
            nodes.len(),
            "chain groups must cover the whole linearized chain"
        );
    }

    #[cfg(feature = "buffer_stats")]
    crate::printer::buffer_stats::record_chain_groups(groups.len());

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
///
/// `in_expression_statement` is prettier's `isExpressionStatement` — whether the chain's
/// outermost call is an expression statement's own expression, which is what admits the
/// short-name merge (`d3.scaleLinear()`). The caller states it: the chain builder reads the
/// statement printer's flag, and the assignment layout's label question
/// ([`super::builder::call_prints_as_member_chain`]) passes `false`, since prettier asks
/// the CALL's parent there and that parent is the assignment.
pub(super) fn should_merge_first_groups<'a>(
    groups: &[ChainGroup<'a>],
    in_expression_statement: bool,
    printer: &Printer<'_>,
) -> bool {
    if groups.len() < 2 {
        return false;
    }

    // Prettier refuses the merge when the second group's first node carries a
    // comment (`!hasComment(groups[1][0].node)`); tsv asks it of the kind that
    // matters — a merged member prints through `print_member_access`, which defers a
    // gap `//` to the line end (see `group_chain_nodes`), so a line comment there
    // needs the unmerged path's chain-level emitter.
    if gap_has_line_comment(&groups[1].nodes[0], printer) {
        return false;
    }

    should_not_wrap(groups, in_expression_statement, printer)
}

/// Whether a chain node's comment gap holds a line comment — the grouping's comment
/// question. Nodes without a gap (a base, a call, a `!`) answer `false`.
///
/// Asked of the CHAIN's share of the gap ([`chain_gap_share_any`]): a computed member's stops
/// at its `[`. A `//` inside the brackets is the lookup's own, printed by the bracket emitter
/// and the index's doc around the index, so the lookup stays in the group prettier puts it in
/// (`x[⏎// c⏎y.p⏎]()`, `arr.foo()[ // c⏎0⏎]` — the latter's `//` keeps the `[` line by the
/// delimiter rule, inside the brackets). A `//` between the object and the `[` does start a
/// group: a member inside a group can only defer its gap comment to the line end, and that one
/// would swallow the `[i]` printed after it.
///
/// Asked of every member node the grouping walks, and the gap is the `.` alone in ~98% of
/// those asks — so the head is inline: the node's range answers at the site, and the
/// printer's wrapper (`Printer::has_line_comments_between`) puts the narrow-gap test
/// inline ahead of its one outlined search, which reads the comment-free window before
/// it probes the array. Pinned `always`: under a plain hint the head stayed an outlined
/// symbol at its four sites, and every narrow ask paid its prologue and `ret` for a
/// two-compare answer.
#[expect(clippy::inline_always)]
#[inline(always)]
fn gap_has_line_comment(node: &ChainNode<'_>, printer: &Printer<'_>) -> bool {
    // Through the hole-honoring seam: the widened range's skipped middle holds comments
    // this node prints none of — an inner member's gap forces its OWN node's group split,
    // and the head's share is printed ahead of the whole chain. Asking the range whole
    // split the group for a comment that is not on this gap's line at all.
    chain_gap_share_any(node, printer.source, |start, end| {
        printer.has_line_comments_between(start, end)
    })
}

/// Check if chain should NOT wrap between first and second groups
///
/// Corresponds to prettier's `shouldNotWrap` logic:
/// - Single base that's `this`, factory identifier, or short name (in expression statement)
/// - Multiple nodes where last is member with factory property
fn should_not_wrap<'a>(
    groups: &[ChainGroup<'a>],
    in_expression_statement: bool,
    printer: &Printer<'_>,
) -> bool {
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

        match &expr.kind {
            // super.method() → merge
            ExpressionKind::Super(_) => true,

            // this.method() → merge
            ExpressionKind::ThisExpression(_) => true,

            // Object.keys() → merge (capital letter = factory)
            // d3.scale() → merge (short name ≤ tabWidth in expression statement context only)
            ExpressionKind::Identifier(id) => {
                is_factory_name(id.ident_name(), id.span.start, printer)
                    || has_computed
                    || (in_expression_statement
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
/// Prettier ref: `isShort` in `print/member-chain.js`
/// Uses `name.length <= options.tabWidth` (JS .length, ASCII-only in practice)
fn is_short_name(name: IdentName<'_>, name_start: u32, printer: &Printer<'_>) -> bool {
    printer.with_ident_name_at(name, name_start, |name| name.len() <= TAB_WIDTH)
}

/// Check if an identifier name is a factory pattern.
///
/// Factory names get merged with their first call in chain formatting.
/// Matches Prettier's `isFactory`: `/^[A-Z]|^[$_]+$/u` (`print/member-chain.js`)
/// - Starts with uppercase: `Object`, `React`, `Observable`
/// - Pure `$`/`_` identifiers: `$`, `_`, `$_`, `$__` (lodash-style)
fn is_factory_name(name: IdentName<'_>, name_start: u32, printer: &Printer<'_>) -> bool {
    printer.with_ident_name_at(
        name,
        name_start,
        crate::printer::expressions::literals::is_factory_identifier_name,
    )
}

/// Build the doc for an already-linearized chain, with the head claim
/// [`chain_head_comment_window`] describes.
///
/// **Every chain entry point goes through here**, and there are three: a call, a member,
/// and a `!` that seals a chain. Each differs only in which linearizer it ran and what it
/// hands over; the steps after that — the head window, the long-curried marks
/// ([`mark_long_curried_calls`]), the grouping, the doc, and the prepend — are one answer, so
/// they are written once. Reassembling them at a call site is
/// how the non-null door came to omit the prepend and drop every comment in its base's
/// stripped parens.
///
/// `chain_start` is the span start of the node whose children were LINEARIZED, which is not
/// always the node being printed: for the non-null door it is the OPERAND, because the
/// region outside the operand is that builder's own leading claim and the two windows must
/// partition the head rather than both taking it (`docs/comments.md` hazard 3). `span` is
/// the printed node's own span, which the chain doc needs for its source lookups.
/// `long_curried` is what the linearizer returned: whether the chain holds a long-curried
/// pair at all, without which the marks have nothing to decide.
pub fn build_linearized_chain_doc(
    nodes: &mut [ChainNode<'_>],
    long_curried: bool,
    head_expr: &Expression<'_>,
    chain_start: u32,
    span: Span,
    printer: &Printer<'_>,
) -> DocId {
    let HeadWindow {
        outer: head_outer,
        start: head_start,
        end: head_end,
    } = chain_head_comment_window(nodes, head_expr, chain_start);
    // The BASE's erased-paren freeze, over the very region the prepend below emits: an
    // alone-on-line format-ignore directive the author wrote inside grouping parens the
    // parser dropped ahead of the base (`(⏎// prettier-ignore⏎{b:  1}⏎).k`) sits INSIDE the
    // chain's span, so no enclosing head can see it and the chain owes the freeze. Armed as
    // a cell because the base's doc is built inside the layout selection below
    // ([`Printer::with_frozen_chain_base`]); the scope is that one operand, Rule A's child
    // scope, since a whole-chain slice would have to contain the `)` the strip removes.
    //
    // TODO: `nodes[0]` is the linearizer's LEAF, not the construct's immediate child, so an
    // author who parenthesized more than one chain level freezes the wrong unit — the
    // directive hoists out and the operand normalizes (`(⏎// prettier-ignore⏎a  .b⏎).k` prints
    // `a.b.k`, prettier keeping `a  .b.k`). The non-null door reaches the same seam and drops
    // the pair with it (`(⏎// prettier-ignore⏎a?.b⏎)!` prints an unindented `a?.b!` under a
    // glued directive, which is not a fixed point). Both predate the freeze's pair work and
    // want the shell the author wrote, not the leaf, as the freeze unit.
    let frozen_base = match nodes.first() {
        Some(ChainNode::Base { expr: base, .. }) => {
            printer.left_spine_operand_frozen_span(head_start, base)
        }
        _ => None,
    };
    let outer_ungroups = printer
        .chain_ungroups_args
        .replace(long_curried && mark_long_curried_calls(nodes, printer));
    let groups = group_chain_nodes(nodes, printer);
    // A comment in a shell below a pair the head's left spine prints — the base's own
    // pair or one deeper (`((/*c*/ (a + 2) * 2) * 2).b`) — leads the whole chain
    // (`Printer::left_spine_run_claim`). Only where the head takes the whole region down
    // to the base: a widened member's prefix is placed mid-chain, which is the
    // linearizer's answer, and a base that owns its leading gap keeps the run inside it.
    // So does a run the base's doc OPENS with on a line of its own, pair or no pair (the
    // `await` / `new` clause): a tagged-template base prints its tag's run inside its own
    // doc, which is the chain's first group, so the head claims it and prints it ahead of
    // the chain, where its hardline leaves the chain's layout alone (`(// c⏎a)`t`.g().h()`
    // stays flat, as the reparse with the comment ahead of the statement does).
    let claim = match nodes.first() {
        Some(ChainNode::Base {
            expr: base,
            needs_parens,
            paren_leading_start: None,
            ..
        }) if head_start == chain_start => {
            printer.left_spine_run_claim(span, chain_start, base, || {
                *needs_parens || printer.left_spine_shell_has_own_line_comment(base)
            })
        }
        _ => None,
    };
    // The first member gap's claim on the base's stripped trailing shells
    // ([`Printer::member_gap_start`]), which the base's own emitters decline.
    let right_claim = match &*nodes {
        [ChainNode::Base { expr: base, .. }, link, ..] => link
            .comment_range()
            .map(|(gap_start, _)| gap_start)
            .filter(|&gap_start| gap_start < base.span().end)
            .map(|gap_start| Span::new(gap_start, base.span().end)),
        _ => None,
    };
    let outer_right_claim = printer.claimed_right_spine_run.replace(right_claim);
    let doc = printer.with_left_spine_claim(claim, || {
        let chain_doc =
            printer.with_frozen_chain_base(frozen_base, || build_chain_doc(&groups, span, printer));
        printer.chain_ungroups_args.set(outer_ungroups);
        let chain_doc =
            printer.prepend_removed_paren_comments(span, head_start, head_end, chain_doc);
        head_outer
            .iter()
            .rev()
            .fold(chain_doc, |doc, &(start, end)| {
                printer.prepend_removed_paren_comments(span, start, end, doc)
            })
    });
    printer.claimed_right_spine_run.set(outer_right_claim);
    doc
}

/// The window `prepend_removed_paren_comments` claims at a chain's head: the leading
/// region the chain's own nodes do NOT claim.
///
/// The whole region is `[chain start, base start)` — the stripped grouping parens the
/// author wrapped the chain in, and any comment among them. It is claimed by two
/// emitters and they must PARTITION it (`docs/comments.md` hazard 3):
///
/// - A member whose gap linearization widened back over a stripped `(` claims that
///   paren's own prefix, `[member start, its object's start)` — prettier relocates a
///   comment written just inside such a `(` to just before that member.
/// - The head takes the REST: everything from the innermost widened paren's object
///   inward, plus any stretch outside the widened claims that none of them covers — a
///   stripped shell around a call's callee or a `!`'s operand, which widens nothing
///   ([`HeadWindow::outer`]).
///
/// So the window's `start` is the innermost widened claim's END (the largest
/// [`ChainNode::paren_gap_skip`] start, since the parens nest), not the chain's own
/// start — reading it as the chain's start hands the head a region a member already
/// claimed, and stopping the head at the widened claim's START (what this returned
/// before, as a single bound) handed it NOTHING while the member skipped the same
/// region: `((⟨⟩a).b).c(x)` had no emitter at all and DROPPED every comment there. The
/// same holds where the base prints a pair of its own and the window ends at that `(`:
/// the start is still the widened claim's end, or `(/* c */ (x?.y).p).t()` prints the
/// comment twice, once ahead of the chain and once ahead of `.p`. And the stretches
/// outside the claims are the head's as well, or `(/* c */ (x.y).p)()` drops it.
///
/// Only a member a call's member chain reaches widens at all, and only while the shell's
/// comment keeps its authored order ([`apply_paren_gaps`]) — so a member-only chain, and
/// every shell that does not widen, keeps its region at the head.
///
/// Not called directly: [`build_linearized_chain_doc`] is the seam, and it exists so this
/// claim cannot be forgotten at a fourth door. It was forgotten at the third —
/// `Printer::build_ts_non_null_doc`'s arm, where a `!` sealing a chain reaches the
/// linearizer — and the base's whole leading run was DROPPED (`const c = ( // x⏎e).m()!;`
/// printed `const c = e.m()!;`) while the two spellings either side of it, the same shell
/// with no `!` and one whose `!` is not the chain's outermost node, were correct all along.
fn chain_head_comment_window(
    nodes: &[ChainNode<'_>],
    expr: &Expression<'_>,
    chain_start: u32,
) -> HeadWindow {
    // The innermost widened claim's end. `paren_gap_skip` is `Some` exactly on a node
    // linearization widened, and its start is that node's object's own span start —
    // never past the base, nor past the `(` of a pair the base prints, since the object
    // contains both — so the max is a position in `[chain_start, end]` for either end
    // below and the window can't invert.
    let start = nodes
        .iter()
        .filter_map(ChainNode::paren_gap_skip)
        .map(|skip| skip.start)
        .max()
        .unwrap_or(chain_start);
    let end = match nodes.first() {
        // A base that OWNS its leading gap (a sealed optional chain, an IIFE callee —
        // `ChainNode::Base::paren_leading_start`) emits it INSIDE the pair it prints, so
        // the claim stops at that `(`. Claiming past it would hoist the run out in front
        // of a pair that survives.
        Some(ChainNode::Base {
            paren_leading_start: Some(pair_start),
            ..
        }) => *pair_start,
        // The linearizer has already walked to the innermost base: it is `nodes[0]`, and
        // its span starts where the walk over member / call / non-null spans would stop
        // (each of those spans opens at its leftmost child). Reading it there spares one
        // dispatch per chain level; the walk stays as the debug oracle.
        Some(ChainNode::Base { expr: base, .. }) => {
            let base_start = base.span().start;
            debug_assert_eq!(base_start, get_chain_base_start(expr));
            base_start
        }
        _ => get_chain_base_start(expr),
    };
    debug_assert!(start <= end, "a widened claim opens past the head's end");
    HeadWindow {
        outer: widened_claim_holes(nodes, chain_start, start),
        start,
        end,
    }
}

/// The head claim [`chain_head_comment_window`] returns: `[start, end)`, the region
/// inside every widened member claim, plus `outer`, the stretches of the leading region
/// OUTSIDE those claims that no member claims either ([`widened_claim_holes`]), in
/// source order.
struct HeadWindow {
    outer: SmallVec<[(u32, u32); 1]>,
    start: u32,
    end: u32,
}

/// The stretches of `[chain_start, inner_start)` that no widened member claims.
///
/// Each widened member claims `[its gap start, its skip start)` — one stripped shell's
/// prefix ([`apply_paren_gaps`]). The prefixes of member links tile the region, since
/// every outer member link finds a member to widen once an inner one has; but a stripped
/// shell around a CALL link's callee or a `!` link's operand widens nothing, so it is a
/// stretch of the region outside the claims, and the head is its emitter. The claims
/// are disjoint (each is one link's prefix), so a sweep in source order finds the gaps.
fn widened_claim_holes(
    nodes: &[ChainNode<'_>],
    chain_start: u32,
    inner_start: u32,
) -> SmallVec<[(u32, u32); 1]> {
    let mut holes = SmallVec::new();
    if inner_start == chain_start {
        return holes;
    }
    let mut claims: SmallVec<[(u32, u32); 4]> = nodes
        .iter()
        .filter_map(|node| Some((node.comment_range()?.0, node.paren_gap_skip()?.start)))
        .collect();
    claims.sort_unstable();
    let mut cursor = chain_start;
    for (claim_start, claim_end) in claims {
        if cursor < claim_start {
            holes.push((cursor, claim_start));
        }
        cursor = cursor.max(claim_end);
    }
    debug_assert_eq!(
        cursor, inner_start,
        "the innermost claim ends where the head resumes"
    );
    holes
}

/// Get the start position of the innermost base expression in a chain.
///
/// [`chain_head_comment_window`] reads this off the linearized base node instead and
/// keeps the walk as its debug oracle (and the fallback for a node list without one).
fn get_chain_base_start<'a>(mut expr: &'a Expression<'a>) -> u32 {
    loop {
        expr = match &expr.kind {
            ExpressionKind::MemberExpression(member) => member.object,
            ExpressionKind::CallExpression(call) => {
                transparent_instantiation_callee(call).map_or(call.callee, |inst| inst.expression)
            }
            ExpressionKind::TSNonNullExpression(non_null) => non_null.expression,
            // Note: TaggedTemplateExpression is NOT traversed here because its own
            // build_tagged_template_doc handles comments from removed parentheses
            _ => return expr.span().start,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrinterInputs;
    use crate::ast::internal::{CallExpression, Identifier, MemberExpression};
    use bumpalo::Bump;
    use tsv_lang::doc::arena::DocArena;
    use tsv_lang::printing::LineBreaks;
    use tsv_lang::{EmbedContext, Span};

    /// A printer over `comments` alone — what the grouping asks about a member's gap. The
    /// nodes and comments fabricate spans with no backing source, so the source is empty
    /// and the inputs are spelled out rather than derived (`for_document` would read each
    /// comment's text out of that source).
    fn make_printer<'a>(
        arena: &'a DocArena,
        lines: &'a LineBreaks<'a>,
        comments: &'a [Comment],
    ) -> Printer<'a> {
        let inputs = PrinterInputs {
            source: "",
            comments,
            line_table: lines.table(),
            has_owned_comments: false,
            has_format_ignore: false,
            comment_free_window: None,
        };
        // A test printer stands in for a whole document, so it owns its lines.
        Printer::with_context(arena, &inputs, EmbedContext::line_owning(), 0)
    }

    /// Helper to create an identifier expression. Tests fabricate spans with no
    /// backing source, so the name rides the escaped channel (an `&'arena str`
    /// resolved directly, regardless of span).
    fn make_identifier<'arena>(name: &'arena str) -> Expression<'arena> {
        let len = name.len() as u32;
        let ident_name = IdentName {
            escaped: Some(name),
            raw_len: 0,
            plain_ascii: false,
        };
        Expression::from_identifier(Identifier::simple(ident_name, Span::new(0, len)))
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
            plain_ascii: false,
        };
        let property_start = object_end + 1; // after the dot
        let span_end = property_start + property_name.len() as u32;
        Expression {
            span: Span::new(0, span_end),
            kind: ExpressionKind::MemberExpression(MemberExpression {
                object: arena.alloc(object),
                property: arena.alloc(Expression::from_identifier(Identifier::simple(
                    prop_name,
                    Span::new(property_start, span_end),
                ))),
                computed: false,
                optional: false,
            }),
        }
    }

    /// Helper to create a call expression: callee()
    fn make_call<'arena>(
        arena: &'arena Bump,
        callee: Expression<'arena>,
        callee_end: u32,
    ) -> Expression<'arena> {
        Expression {
            kind: ExpressionKind::CallExpression(CallExpression {
                callee: arena.alloc(callee),
                arguments: &[],
                type_arguments: None,
                optional: false,
            }),
            span: Span::new(0, callee_end + "()".len() as u32),
        }
    }

    #[test]
    fn test_linearize_simple_identifier() {
        let expr = make_identifier("foo");

        let docs = DocArena::new();
        let lines = LineBreaks::new("", Vec::new());
        let printer = make_printer(&docs, &lines, &[]);
        let nodes = linearize_chain(&expr, printer.linearize_input());

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

        let docs = DocArena::new();
        let lines = LineBreaks::new("", Vec::new());
        let printer = make_printer(&docs, &lines, &[]);
        let nodes = linearize_chain(&abc, printer.linearize_input());

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

        let docs = DocArena::new();
        let lines = LineBreaks::new("", Vec::new());
        let printer = make_printer(&docs, &lines, &[]);
        let nodes = linearize_chain(&ab_call, printer.linearize_input());

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

        let docs = DocArena::new();
        let lines = LineBreaks::new("", Vec::new());
        let printer = make_printer(&docs, &lines, &[]);
        let nodes = linearize_chain(&abcd, printer.linearize_input());
        let groups = group_chain_nodes(&nodes, &printer);

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

        let docs = DocArena::new();
        let lines = LineBreaks::new("", Vec::new());
        let printer = make_printer(&docs, &lines, &[]);
        let nodes = linearize_chain(&abc, printer.linearize_input());
        let groups = group_chain_nodes(&nodes, &printer);

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
        let arena_docs = DocArena::new();
        let lines = LineBreaks::new("", Vec::new());
        let printer = make_printer(&arena_docs, &lines, &[]);
        let groups = group_chain_nodes(&[], &printer);
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
            from_template_reader: false,
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
            plain_ascii: false,
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
        let call_expr = make_call(&arena, make_identifier("a"), 1);
        let ExpressionKind::CallExpression(call) = &call_expr.kind else {
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
                let mut v = vec![
                    ChainNode::base(&base, false),
                    ChainNode::call(call, call_expr.span),
                ];
                v.extend(members.iter().copied());
                v
            }),
        ] {
            for (commented, member) in members.iter().enumerate() {
                let (gap_start, _) = member.comment_range().unwrap();
                let comments = [make_line_comment(gap_start + 1)];
                let arena_docs = DocArena::new();
                let lines = LineBreaks::new("", Vec::new());
                let printer = make_printer(&arena_docs, &lines, &comments);
                let groups = group_chain_nodes(&nodes, &printer);

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

    /// The nodes and paren gaps of `root`'s chain, walked by [`linearize_link`] with `frames`
    /// link frames — or, for `None`, by [`linearize_spine_loop`] from the first link.
    fn linearized(root: &Expression<'_>, input: LinearizeInput<'_>, frames: Option<u32>) -> String {
        let (mut nodes, mut paren_gaps) = (ChainNodeVec::new(), Vec::new());
        match frames {
            Some(frames) => linearize_link(root, input, &mut nodes, &mut paren_gaps, frames),
            None => linearize_spine_loop(root, input, &mut nodes, &mut paren_gaps),
        }
        format!("{nodes:?} {paren_gaps:?}")
    }

    /// Every expression a chain test walks from: each link down the spine of `expr`, and
    /// the same for every call argument and every tagged template's tag on the way.
    fn chain_roots<'a>(expr: &'a Expression<'a>, out: &mut Vec<&'a Expression<'a>>) {
        out.push(expr);
        match &expr.kind {
            ExpressionKind::CallExpression(call) => {
                chain_roots(call.callee, out);
                for argument in call.arguments {
                    chain_roots(argument, out);
                }
            }
            ExpressionKind::MemberExpression(member) => chain_roots(member.object, out),
            ExpressionKind::TSNonNullExpression(non_null) => {
                chain_roots(non_null.expression, out);
            }
            ExpressionKind::TSInstantiationExpression(inst) => chain_roots(inst.expression, out),
            ExpressionKind::TaggedTemplateExpression(tagged) => chain_roots(tagged.tag, out),
            _ => {}
        }
    }

    /// The loop pushes the nodes and the paren gaps the recursion pushes, in the same order —
    /// the loop from the first link, the recursion all the way down, and the walk itself,
    /// which switches from one to the other past [`LINK_FRAMES`] links — over chains that
    /// reach every descent half and every return half: sealed optional chains, transparent
    /// instantiations, IIFE bases, retained `!` shells, stripped-paren gaps with comments,
    /// and the kept shells of a comma-list region (every link's span marked, every other
    /// one, none), each also behind a run of links long enough to reach the switch.
    #[test]
    fn linearize_spine_runs_the_halves_in_the_recursions_order() {
        let short = [
            "a.b.c.d;",
            "a().b().c!.d;",
            "a?.b.c?.().d;",
            "(a?.b).c.d();",
            "(a?.b)!.c();",
            "(a?.b!).c().d;",
            "(a?.b)()!.c;",
            "(a?.b)<T>?.(c).d;",
            "((a?.b)<T>)?.(c).d;",
            "f<T>(x).y<U>().z;",
            "f<T><U>(x).y;",
            "( // c\n() => {})().p.q();",
            "(function () {})().p;",
            "(/* c */ () => 1)()().p!;",
            "(aaa // c\n)!.bbb.ccc();",
            "(aaa /* a\nb */)!.bbb!.ccc;",
            "(x + y /* c */)!.foo().bar;",
            "/* c */ (a).b.c();",
            "(/* c */ (a).b).c().d;",
            "((a).b /* d */).c().d();",
            "(((a).b).c /* e */).d().e;",
            "a[0][1].b!()!.c;",
            "new a.b().c.d();",
            "a`t`.b.c();",
            "(a, b).c().d;",
            "(await x).y.z();",
            "a!.b!!.c!();",
            "a.b(c.d().e, (f?.g).h()).i;",
            "a.b`x${c.d()}`.e();",
            "x.y(() => z.w().v).u;",
        ];
        let long_tail = ".b!/* c */.c()?.d[0]".repeat(LINK_FRAMES as usize / 2);
        let sources: Vec<String> = short
            .iter()
            .map(|&source| source.to_owned())
            .chain(short.iter().map(|source| {
                let chain = source.trim_end_matches(";\n").trim_end_matches(';');
                format!("({chain}){long_tail}.e;")
            }))
            .collect();
        for source in &sources {
            let source = source.as_str();
            let arena = Bump::new();
            let program = crate::parse(source, &arena).expect("parses");
            let mut roots = Vec::new();
            for statement in program.body {
                if let internal::StatementKind::ExpressionStatement(statement) = &statement.kind {
                    chain_roots(statement.expression, &mut roots);
                }
            }
            let every = SpanMarks::default();
            let alternate = SpanMarks::default();
            for (i, root) in roots.iter().enumerate() {
                every.mark(root.span);
                if i % 2 == 1 {
                    alternate.mark(root.span);
                }
            }
            let docs = DocArena::new();
            let lines = LineBreaks::new(source, Vec::new());
            let inputs = PrinterInputs {
                source,
                comments: program.comments,
                line_table: lines.table(),
                has_owned_comments: false,
                has_format_ignore: false,
                comment_free_window: None,
            };
            let printer = Printer::with_context(&docs, &inputs, EmbedContext::line_owning(), 0);
            for marks in [None, Some(&every), Some(&alternate)] {
                let input = LinearizeInput {
                    printer: &printer,
                    region_kept_shells: marks,
                };
                for root in &roots {
                    let recursed = linearized(root, input, Some(u32::MAX));
                    let looped = linearized(root, input, None);
                    let switched = linearized(root, input, Some(LINK_FRAMES));
                    let at = &source[root.span.range()];
                    assert_eq!(looped, recursed, "{source:?} from {at:?}");
                    assert_eq!(switched, recursed, "{source:?} from {at:?}");
                }
            }
        }
    }
}
