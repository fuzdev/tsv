// Chain analysis for TypeScript member chain formatting
//
// This module handles the analysis phase of chain formatting:
// - Linearization: Flatten nested AST into a flat list of ChainNodes
// - Grouping: Group nodes by natural break points
// - Merge decisions: Determine if first groups should be merged
// - SymbolLookup trait for identifier resolution

use super::printing::ChainPrinter;
use super::types::{ChainGroup, ChainGroupVec, ChainNode, ChainNodeVec};
use crate::ast::internal::{self, Expression};
use crate::printer::{ParenContext, needs_parens};
use string_interner::DefaultSymbol;
use tsv_lang::TAB_WIDTH;

//
// Symbol Lookup Trait
//

/// Trait for looking up symbols (abstraction over interner)
pub trait SymbolLookup {
    /// Resolve `symbol` and apply `f` to the name without materializing a `String`.
    ///
    /// Callback style keeps the interner borrow inside the call, so implementations
    /// backed by `Rc<RefCell<…>>` don't need an owned copy to outlive the borrow.
    /// Returns `None` when the symbol is unknown to the interner.
    fn with_name<R>(&self, symbol: DefaultSymbol, f: impl FnOnce(&str) -> R) -> Option<R>;
}

//
// Linearization
//

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
fn linearize_chain<'a>(expr: &'a Expression<'_>) -> ChainNodeVec<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_recursive(expr, &mut nodes, &mut paren_gaps);
    finalize_chain_nodes(&mut nodes, &paren_gaps);
    nodes
}

/// Linearize starting from a CallExpression (avoids cloning to wrap in Expression)
pub fn linearize_chain_from_call<'a>(call: &'a internal::CallExpression<'_>) -> ChainNodeVec<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_call_callee(call, &mut nodes, &mut paren_gaps);
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
) -> ChainNodeVec<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_member_object(member, &mut nodes, &mut paren_gaps);
    linearize_member_node(member, &mut nodes, &mut paren_gaps);
    finalize_chain_nodes(&mut nodes, &paren_gaps);
    nodes
}

/// Linearize starting from a TSNonNullExpression (avoids cloning to wrap in Expression)
pub fn linearize_chain_from_non_null<'a>(
    non_null: &'a internal::TSNonNullExpression<'_>,
) -> ChainNodeVec<'a> {
    let mut nodes = ChainNodeVec::new();
    let mut paren_gaps = Vec::new();
    linearize_recursive(non_null.expression, &mut nodes, &mut paren_gaps);
    nodes.push(ChainNode::non_null());
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
        *np = needs_parens(expr, ParenContext::Callee);
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
fn child_stops_optional_chain(
    parent_start: u32,
    parent_optional: bool,
    child: &Expression<'_>,
) -> bool {
    !parent_optional && parent_start < child.span().start && child.has_optional_in_chain()
}

/// Push a sealed parenthesized-optional-chain object/callee as a base node.
///
/// When the sealed child is a non-null assertion wrapping the chain (`(a?.b!).c`,
/// `!` inside the parens), lift the `!` out: emit the bare chain as the
/// parenthesized base, then a separate `NonNull` node. That renders `(a?.b)!.c` —
/// prettier's canonical form, identical to the `!`-outside source `(a?.b)!.c`. The
/// `!` is a type-only assertion, so its position relative to the grouping parens
/// carries no runtime meaning, and both formatters normalize to the outside form.
/// Any other sealed child (a bare optional chain, `(a?.b).c`) stays a single
/// parenthesized base.
fn push_sealed_chain_base<'a>(child: &'a Expression<'_>, nodes: &mut ChainNodeVec<'a>) {
    if let Expression::TSNonNullExpression(non_null) = child {
        nodes.push(ChainNode::base(non_null.expression, true));
        nodes.push(ChainNode::non_null());
    } else {
        nodes.push(ChainNode::base(child, true));
    }
}

fn linearize_recursive<'a>(
    expr: &'a Expression<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    match expr {
        // CallExpression: recurse into callee, then add Call node
        Expression::CallExpression(call) => {
            linearize_call_callee(call, nodes, paren_gaps);
            if call.optional {
                nodes.push(ChainNode::call_optional(call));
            } else {
                nodes.push(ChainNode::call(call));
            }
        }

        // MemberExpression: recurse into object, then add Member node
        Expression::MemberExpression(member) => {
            linearize_member_object(member, nodes, paren_gaps);
            linearize_member_node(member, nodes, paren_gaps);
        }

        // TSNonNullExpression: recurse into expression, then add NonNull node
        // TODO: a TSInstantiationExpression operand here (`(A<T>)!.x`) is recursed
        // transparently and loses its type args (no Call node recovers them, unlike
        // the call-callee path). Same root cause as the member-object case fixed via
        // linearize_member_object. Untested because prettier's parser rejects the
        // syntax, so there's no canonical source for a fixture.
        Expression::TSNonNullExpression(non_null) => {
            // A parenthesized optional chain sealed inside the non-null assertion
            // (`(a?.b)!.c`) must keep its parens — the trailing access reached via this
            // node's parent must not be absorbed. Emit the bare chain as a
            // parenthesized base + `!` so it renders `(a?.b)!.c`, not `a?.b!.c`.
            let inner = &non_null.expression;
            if non_null.seals_optional_chain() {
                nodes.push(ChainNode::base(inner, true));
            } else {
                linearize_recursive(inner, nodes, paren_gaps);
            }
            nodes.push(ChainNode::non_null());
        }

        // TSInstantiationExpression as a call callee (`expr<T>(args)`): transparent.
        // The Call node recovers the type args via get_call_type_arguments() in
        // chain_args.rs, so the instantiation itself emits nothing here. Member
        // objects (`(A<T>).x`) take the `linearize_member_object` path instead,
        // which keeps the type args and parens.
        Expression::TSInstantiationExpression(inst) => {
            linearize_recursive(inst.expression, nodes, paren_gaps);
        }

        // Base case: expression that's not part of the chain structure
        _ => {
            let needs_parens = needs_parens(expr, ParenContext::ChainBase);
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
///   `push_sealed_chain_base` (which lifts an inner `!` out of the parens).
/// - A `TSInstantiationExpression` must keep its type args and be parenthesized:
///   `(A<T>).x`, not `A.x` (data loss) or `A<T>.x` (ambiguous). Prettier
///   parenthesizes an instantiation only when it is the object of a member
///   access, and no Call node follows here to recover dropped type args.
///
/// All other objects recurse normally.
fn linearize_member_object<'a>(
    member: &'a internal::MemberExpression<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    let object: &Expression<'_> = member.object;
    if child_stops_optional_chain(member.span.start, member.optional, object) {
        push_sealed_chain_base(object, nodes);
    } else if matches!(object, Expression::TSInstantiationExpression(_)) {
        nodes.push(ChainNode::base(object, true));
    } else {
        linearize_recursive(object, nodes, paren_gaps);
    }
}

/// Linearize a CallExpression's callee.
///
/// A parenthesized optional chain callee (`(a?.b)()`) terminates the chain and
/// must stay a parenthesized base node — see `child_stops_optional_chain`. All
/// other callees recurse normally.
fn linearize_call_callee<'a>(
    call: &'a internal::CallExpression<'_>,
    nodes: &mut ChainNodeVec<'a>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    if child_stops_optional_chain(call.span.start, call.optional, call.callee) {
        push_sealed_chain_base(call.callee, nodes);
    } else {
        linearize_recursive(call.callee, nodes, paren_gaps);
    }
}

/// Process a MemberExpression node: handle paren gaps and push the appropriate ChainNode.
///
/// Extracted from `linearize_recursive` so it can be shared with `linearize_chain_from_member`.
fn linearize_member_node<'a>(
    member: &'a internal::MemberExpression<'_>,
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

    let object_end = member.object.span().end;
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
            id.name,
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
pub fn group_chain_nodes<'a>(nodes: &[ChainNode<'a>]) -> ChainGroupVec<'a> {
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
        if node.is_call() || node.is_non_null() || node.is_numeric_accessor() {
            current.push(nodes[i]);
            i += 1;
        } else {
            break;
        }
    }

    // If first node wasn't a call, add consecutive members
    // (but not the last one - that stays with subsequent calls)
    if !nodes[0].is_call() {
        while i + 1 < nodes.len() && nodes[i].is_member() && nodes[i + 1].is_member() {
            current.push(nodes[i]);
            i += 1;
        }
    }

    groups.push(current);
    current = ChainGroup::new();

    // Phase 2: Build remaining groups
    // Pattern: (members)* (calls)*, break at memberish after call
    let mut seen_call = false;

    while i < nodes.len() {
        let node = &nodes[i];

        // When we've seen a call and encounter a member, start a new group
        if seen_call && node.is_member() && !node.is_numeric_accessor() {
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
pub fn should_merge_first_groups<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> bool {
    if groups.len() < 2 {
        return false;
    }

    // Don't merge if second group's first node has comments (not implemented yet)
    // if has_comment(&groups[1].nodes[0]) { return false; }

    should_not_wrap(groups, printer)
}

/// Check if chain should NOT wrap between first and second groups
///
/// Corresponds to prettier's `shouldNotWrap` logic:
/// - Single base that's `this`, factory identifier, or short name (in expression statement)
/// - Multiple nodes where last is member with factory property
pub fn should_not_wrap<'a, P: ChainPrinter>(groups: &[ChainGroup<'a>], printer: &P) -> bool {
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
                is_factory_name(id.name, printer)
                    || has_computed
                    || (printer.is_expression_statement() && is_short_name(id.name, printer))
            }

            _ => has_computed,
        }
    } else {
        // Multiple nodes in first group: check if last is member with factory property
        if let Some(prop) = first.nodes.last().and_then(ChainNode::property) {
            return is_factory_name(prop, printer) || has_computed;
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
fn is_short_name(symbol: DefaultSymbol, interner: &impl SymbolLookup) -> bool {
    interner
        .with_name(symbol, |name| name.len() <= TAB_WIDTH)
        .unwrap_or(false)
}

/// Check if an identifier name is a factory pattern.
///
/// Factory names get merged with their first call in chain formatting.
/// Matches Prettier's `isFactory`: `/^[A-Z]|^[$_]+$/u` (member-chain.js:273)
/// - Starts with uppercase: `Object`, `React`, `Observable`
/// - Pure `$`/`_` identifiers: `$`, `_`, `$_`, `$__` (lodash-style)
fn is_factory_name(symbol: DefaultSymbol, interner: &impl SymbolLookup) -> bool {
    interner
        .with_name(
            symbol,
            crate::printer::expressions::literals::is_factory_identifier_name,
        )
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::internal::{CallExpression, Identifier, MemberExpression};
    use bumpalo::Bump;
    use string_interner::DefaultStringInterner;
    use tsv_lang::Span;

    /// Helper to create an identifier expression
    fn make_identifier<'arena>(
        interner: &mut DefaultStringInterner,
        name: &str,
    ) -> Expression<'arena> {
        let symbol = interner.get_or_intern(name);
        Expression::Identifier(Identifier::simple(symbol, Span::new(0, name.len() as u32)))
    }

    /// Helper to create a member expression: object.property
    fn make_member<'arena>(
        arena: &'arena Bump,
        interner: &mut DefaultStringInterner,
        object: Expression<'arena>,
        property_name: &str,
        object_end: u32,
    ) -> Expression<'arena> {
        let prop_symbol = interner.get_or_intern(property_name);
        let property_start = object_end + 1; // after the dot
        let span_end = property_start + property_name.len() as u32;
        Expression::MemberExpression(MemberExpression {
            object: arena.alloc(object),
            property: arena.alloc(Expression::Identifier(Identifier::simple(
                prop_symbol,
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
        let mut interner = DefaultStringInterner::new();
        let expr = make_identifier(&mut interner, "foo");

        let nodes = linearize_chain(&expr);

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
        let mut interner = DefaultStringInterner::new();
        // Build: a.b.c
        let a = make_identifier(&mut interner, "a");
        let ab = make_member(&arena, &mut interner, a, "b", 1);
        let abc = make_member(&arena, &mut interner, ab, "c", 3);

        let nodes = linearize_chain(&abc);

        // Should produce: [Base(a), Member(.b), Member(.c)]
        assert_eq!(nodes.len(), 3);
        assert!(matches!(nodes[0], ChainNode::Base { .. }));
        assert!(matches!(nodes[1], ChainNode::Member { .. }));
        assert!(matches!(nodes[2], ChainNode::Member { .. }));
    }

    #[test]
    fn test_linearize_call_chain() {
        let arena = Bump::new();
        let mut interner = DefaultStringInterner::new();
        // Build: a().b()
        let a = make_identifier(&mut interner, "a");
        let a_call = make_call(&arena, a, 1);
        let ab = make_member(&arena, &mut interner, a_call, "b", 3);
        let ab_call = make_call(&arena, ab, 5);

        let nodes = linearize_chain(&ab_call);

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
        let mut interner = DefaultStringInterner::new();
        // Build: a.b.c.d
        let a = make_identifier(&mut interner, "a");
        let ab = make_member(&arena, &mut interner, a, "b", 1);
        let abc = make_member(&arena, &mut interner, ab, "c", 3);
        let abcd = make_member(&arena, &mut interner, abc, "d", 5);

        let nodes = linearize_chain(&abcd);
        let groups = group_chain_nodes(&nodes);

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
        let mut interner = DefaultStringInterner::new();
        // Build: a().b().c
        let a = make_identifier(&mut interner, "a");
        let a_call = make_call(&arena, a, 1);
        let ab = make_member(&arena, &mut interner, a_call, "b", 3);
        let ab_call = make_call(&arena, ab, 5);
        let abc = make_member(&arena, &mut interner, ab_call, "c", 7);

        let nodes = linearize_chain(&abc);
        let groups = group_chain_nodes(&nodes);

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
        let groups = group_chain_nodes(&[]);
        assert!(groups.is_empty());
    }
}
