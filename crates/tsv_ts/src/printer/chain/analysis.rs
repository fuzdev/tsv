// Chain analysis for TypeScript member chain formatting
//
// This module handles the analysis phase of chain formatting:
// - Linearization: Flatten nested AST into a flat list of ChainNodes
// - Grouping: Group nodes by natural break points
// - Merge decisions: Determine if first groups should be merged
// - SymbolLookup trait for identifier resolution

use super::printing::ChainPrinter;
use super::types::{ChainGroup, ChainNode};
use crate::ast::internal::Expression;
use crate::printer::{ParenContext, needs_parens};
use string_interner::DefaultSymbol;

//
// Symbol Lookup Trait
//

/// Trait for looking up symbols (abstraction over interner)
pub trait SymbolLookup {
    fn lookup(&self, symbol: DefaultSymbol) -> Option<String>;
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
pub fn linearize_chain<'a>(expr: &'a Expression) -> Vec<ChainNode<'a>> {
    let mut nodes = Vec::new();
    let mut paren_gaps = Vec::new();
    linearize_recursive(expr, &mut nodes, &mut paren_gaps);

    // Only extend ranges for call chains — prettier places comments mid-chain
    // only when the chain contains calls
    if !paren_gaps.is_empty() && nodes.iter().any(ChainNode::is_call) {
        for (node_index, gap_start) in paren_gaps {
            if let Some(node) = nodes.get_mut(node_index) {
                match node {
                    ChainNode::Member { object_end, .. }
                    | ChainNode::PrivateMember { object_end, .. }
                    | ChainNode::ComputedMember { object_end, .. } => {
                        *object_end = gap_start;
                    }
                    _ => {}
                }
            }
        }
    }

    nodes
}

/// A deferred paren gap extension: (node_index, gap_start)
type ParenGap = (usize, u32);

fn linearize_recursive<'a>(
    expr: &'a Expression,
    nodes: &mut Vec<ChainNode<'a>>,
    paren_gaps: &mut Vec<ParenGap>,
) {
    match expr {
        // CallExpression: recurse into callee, then add Call node
        Expression::CallExpression(call) => {
            linearize_recursive(&call.callee, nodes, paren_gaps);
            if call.optional {
                nodes.push(ChainNode::call_optional(expr));
            } else {
                nodes.push(ChainNode::call(expr));
            }
        }

        // MemberExpression: recurse into object, then add Member node
        Expression::MemberExpression(member) => {
            linearize_recursive(&member.object, nodes, paren_gaps);

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
                    &member.property,
                    member.optional,
                    object_end,
                    member.span.end,
                ));
            } else if let Expression::Identifier(id) = member.property.as_ref() {
                nodes.push(ChainNode::member(
                    id.name,
                    member.optional,
                    object_end,
                    property_start,
                ));
            } else if let Expression::PrivateIdentifier(pid) = member.property.as_ref() {
                nodes.push(ChainNode::private_member(
                    pid.name,
                    member.optional,
                    object_end,
                    property_start,
                ));
            } else {
                // Non-identifier property (shouldn't happen for non-computed)
                nodes.push(ChainNode::computed_member(
                    &member.property,
                    member.optional,
                    object_end,
                    member.span.end,
                ));
            }
        }

        // TSNonNullExpression: recurse into expression, then add NonNull node
        Expression::TSNonNullExpression(non_null) => {
            linearize_recursive(&non_null.expression, nodes, paren_gaps);
            nodes.push(ChainNode::non_null());
        }

        // TSInstantiationExpression: recurse into expression (transparent in chains)
        // Type args are recovered by get_call_type_arguments() in chain_args.rs.
        Expression::TSInstantiationExpression(inst) => {
            linearize_recursive(&inst.expression, nodes, paren_gaps);
        }

        // Base case: expression that's not part of the chain structure
        _ => {
            let needs_parens = needs_parens(expr, ParenContext::ChainBase);
            nodes.push(ChainNode::base(expr, needs_parens));
        }
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
pub fn group_chain_nodes<'a>(nodes: Vec<ChainNode<'a>>) -> Vec<ChainGroup<'a>> {
    if nodes.is_empty() {
        return vec![];
    }

    let mut groups: Vec<ChainGroup<'a>> = Vec::new();
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

            // this.method() → merge (this is parsed as Identifier in our AST)
            // Object.keys() → merge (capital letter = factory)
            // d3.scale() → merge (short name ≤ tabWidth in expression statement context only)
            Expression::Identifier(id) => {
                is_this_identifier(id.name, printer)
                    || is_factory_name(id.name, printer)
                    || has_computed
                    || (printer.is_expression_statement()
                        && is_short_name(id.name, printer, printer.get_tab_width()))
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
fn is_short_name(symbol: DefaultSymbol, interner: &impl SymbolLookup, tab_width: usize) -> bool {
    let Some(name) = interner.lookup(symbol) else {
        return false;
    };
    name.len() <= tab_width
}

/// Check if an identifier is `this`
///
/// In our AST, `this` is parsed as an Identifier with name "this" (not a separate ThisExpression).
/// `this.method()` chains should be merged (keep `this` on same line as first method call).
fn is_this_identifier(symbol: DefaultSymbol, interner: &impl SymbolLookup) -> bool {
    interner.lookup(symbol).is_some_and(|name| name == "this")
}

/// Check if an identifier name is a factory pattern.
///
/// Factory names get merged with their first call in chain formatting.
/// Matches Prettier's `isFactory`: `/^[A-Z]|^[$_]+$/u` (member-chain.js:273)
/// - Starts with uppercase: `Object`, `React`, `Observable`
/// - Pure `$`/`_` identifiers: `$`, `_`, `$_`, `$__` (lodash-style)
fn is_factory_name(symbol: DefaultSymbol, interner: &impl SymbolLookup) -> bool {
    let Some(name) = interner.lookup(symbol) else {
        return false;
    };
    name.chars().next().is_some_and(char::is_uppercase)
        || (!name.is_empty() && name.chars().all(|c| c == '$' || c == '_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::internal::{CallExpression, Identifier, MemberExpression};
    use string_interner::DefaultStringInterner;
    use tsv_lang::Span;

    /// Helper to create an identifier expression
    fn make_identifier(interner: &mut DefaultStringInterner, name: &str) -> Expression {
        let symbol = interner.get_or_intern(name);
        Expression::Identifier(Identifier {
            name: symbol,
            type_annotation: None,
            decorators: None,
            optional: false,
            span: Span::new(0, name.len() as u32),
        })
    }

    /// Helper to create a member expression: object.property
    fn make_member(
        interner: &mut DefaultStringInterner,
        object: Expression,
        property_name: &str,
        object_end: u32,
    ) -> Expression {
        let prop_symbol = interner.get_or_intern(property_name);
        let property_start = object_end + 1; // after the dot
        let span_end = property_start + property_name.len() as u32;
        Expression::MemberExpression(MemberExpression {
            object: Box::new(object),
            property: Box::new(Expression::Identifier(Identifier {
                name: prop_symbol,
                type_annotation: None,
                decorators: None,
                optional: false,
                span: Span::new(property_start, span_end),
            })),
            computed: false,
            optional: false,
            span: Span::new(0, span_end),
        })
    }

    /// Helper to create a call expression: callee()
    fn make_call(callee: Expression, callee_end: u32) -> Expression {
        Expression::CallExpression(CallExpression {
            callee: Box::new(callee),
            arguments: Vec::new(),
            type_arguments: None,
            optional: false,
            span: Span::new(0, callee_end + 2), // +2 for "()"
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
        let mut interner = DefaultStringInterner::new();
        // Build: a.b.c
        let a = make_identifier(&mut interner, "a");
        let ab = make_member(&mut interner, a, "b", 1);
        let abc = make_member(&mut interner, ab, "c", 3);

        let nodes = linearize_chain(&abc);

        // Should produce: [Base(a), Member(.b), Member(.c)]
        assert_eq!(nodes.len(), 3);
        assert!(matches!(nodes[0], ChainNode::Base { .. }));
        assert!(matches!(nodes[1], ChainNode::Member { .. }));
        assert!(matches!(nodes[2], ChainNode::Member { .. }));
    }

    #[test]
    fn test_linearize_call_chain() {
        let mut interner = DefaultStringInterner::new();
        // Build: a().b()
        let a = make_identifier(&mut interner, "a");
        let a_call = make_call(a, 1);
        let ab = make_member(&mut interner, a_call, "b", 3);
        let ab_call = make_call(ab, 5);

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
        let mut interner = DefaultStringInterner::new();
        // Build: a.b.c.d
        let a = make_identifier(&mut interner, "a");
        let ab = make_member(&mut interner, a, "b", 1);
        let abc = make_member(&mut interner, ab, "c", 3);
        let abcd = make_member(&mut interner, abc, "d", 5);

        let nodes = linearize_chain(&abcd);
        let groups = group_chain_nodes(nodes);

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
        let mut interner = DefaultStringInterner::new();
        // Build: a().b().c
        let a = make_identifier(&mut interner, "a");
        let a_call = make_call(a, 1);
        let ab = make_member(&mut interner, a_call, "b", 3);
        let ab_call = make_call(ab, 5);
        let abc = make_member(&mut interner, ab_call, "c", 7);

        let nodes = linearize_chain(&abc);
        let groups = group_chain_nodes(nodes);

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
        let nodes: Vec<ChainNode> = vec![];
        let groups = group_chain_nodes(nodes);
        assert!(groups.is_empty());
    }
}
