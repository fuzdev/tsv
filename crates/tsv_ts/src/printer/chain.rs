// Member chain formatting for TypeScript
//
// This module implements prettier-compatible member chain formatting following
// prettier's linearize→group→conditionalGroup model from member-chain.js.
//
// ## Architecture
//
// 1. **Linearization**: Flatten nested AST into a flat list of `ChainNode`s
//    `a().b().c!.d` → [Base(a), Call(), Member(.b), Call(), NonNull(!), Member(.d)]
//
// 2. **Grouping**: Group nodes by natural break points
//    - First group: base + calls + non-null + numeric accessors + consecutive members
//    - Remaining groups: members* + calls*, break at memberish after call
//
// 3. **Doc building**: Use conditionalGroup with oneLine/expanded alternatives
//
// ## References
// - prettier/src/language-js/print/member-chain.js

use super::utils::is_simple_call_argument;
use crate::ast::internal::{self, Expression, LiteralValue};
use crate::printer::{ParenContext, needs_parens};
use string_interner::DefaultSymbol;
use tsv_lang::Span;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::has_blank_line_between;

// =============================================================================
// Data Structures
// =============================================================================

/// A node in a linearized chain
///
/// Each variant contains exactly the data it needs - no optional fields.
/// This makes invalid states unrepresentable.
#[derive(Debug, Clone, Copy)]
pub enum ChainNode<'a> {
    /// Base expression: identifier, literal, complex expr in parens
    Base {
        expr: &'a Expression,
        needs_parens: bool,
    },
    /// Call expression: ()
    Call {
        expr: &'a Expression,
        optional: bool,
    },
    /// Member access: .prop
    /// `object_end` is where the object expression ends
    /// `property_start` is where the property identifier starts (for comment detection)
    Member {
        property: DefaultSymbol,
        optional: bool,
        object_end: u32,
        property_start: u32,
    },
    /// Private member access: .#prop
    PrivateMember {
        property: DefaultSymbol,
        optional: bool,
        object_end: u32,
        property_start: u32,
    },
    /// Computed member access: [expr]
    ComputedMember {
        expr: &'a Expression,
        optional: bool,
        object_end: u32,
    },
    /// Non-null assertion: !
    NonNull,
}

impl<'a> ChainNode<'a> {
    /// Create a new base node
    pub fn base(expr: &'a Expression, needs_parens: bool) -> Self {
        Self::Base { expr, needs_parens }
    }

    /// Create a new call node
    pub fn call(expr: &'a Expression) -> Self {
        Self::Call {
            expr,
            optional: false,
        }
    }

    /// Create a new call node with optional chaining
    pub fn call_optional(expr: &'a Expression) -> Self {
        Self::Call {
            expr,
            optional: true,
        }
    }

    /// Create a new member node
    pub fn member(
        property: DefaultSymbol,
        optional: bool,
        object_end: u32,
        property_start: u32,
    ) -> Self {
        Self::Member {
            property,
            optional,
            object_end,
            property_start,
        }
    }

    /// Create a new private member node: .#prop
    pub fn private_member(
        property: DefaultSymbol,
        optional: bool,
        object_end: u32,
        property_start: u32,
    ) -> Self {
        Self::PrivateMember {
            property,
            optional,
            object_end,
            property_start,
        }
    }

    /// Create a new computed member node
    pub fn computed_member(expr: &'a Expression, optional: bool, object_end: u32) -> Self {
        Self::ComputedMember {
            expr,
            optional,
            object_end,
        }
    }

    /// Create a new non-null node
    pub fn non_null() -> Self {
        Self::NonNull
    }

    /// Check if this is a call node
    pub const fn is_call(&self) -> bool {
        matches!(self, Self::Call { .. })
    }

    /// Check if this is a member node (including computed)
    pub const fn is_member(&self) -> bool {
        matches!(
            self,
            Self::Member { .. } | Self::PrivateMember { .. } | Self::ComputedMember { .. }
        )
    }

    /// Get the comment range for this node (object_end, property_start)
    /// Returns None for nodes that don't have inter-element comment regions
    pub fn comment_range(&self) -> Option<(u32, u32)> {
        match self {
            Self::Member {
                object_end,
                property_start,
                ..
            }
            | Self::PrivateMember {
                object_end,
                property_start,
                ..
            } => Some((*object_end, *property_start)),
            Self::ComputedMember {
                object_end, expr, ..
            } => Some((*object_end, expr.span().start)),
            Self::Base { .. } | Self::Call { .. } | Self::NonNull => None,
        }
    }

    /// Check if this is a non-null node
    pub const fn is_non_null(&self) -> bool {
        matches!(self, Self::NonNull)
    }

    /// Check if this is a numeric computed accessor like [0], [1]
    pub fn is_numeric_accessor(&self) -> bool {
        if let Self::ComputedMember { expr, .. } = self
            && let Expression::Literal(lit) = expr
        {
            return matches!(lit.value, LiteralValue::Number(_));
        }
        false
    }

    /// Check if this is a computed member access
    pub const fn is_computed(&self) -> bool {
        matches!(self, Self::ComputedMember { .. })
    }

    /// Get property symbol for Member nodes
    pub const fn property(&self) -> Option<DefaultSymbol> {
        match self {
            Self::Member { property, .. } => Some(*property),
            _ => None,
        }
    }
}

/// A group of chain nodes that stay on the same line
#[derive(Debug, Clone)]
pub struct ChainGroup<'a> {
    pub nodes: Vec<ChainNode<'a>>,
}

impl<'a> ChainGroup<'a> {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn push(&mut self, node: ChainNode<'a>) {
        self.nodes.push(node);
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get the comment range of the first node in this group
    /// Returns (object_end, property_start) if the first node is a member type
    pub fn first_member_range(&self) -> Option<(u32, u32)> {
        self.nodes.first()?.comment_range()
    }
}

impl<'a> Default for ChainGroup<'a> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Linearization
// =============================================================================

/// Linearize a chain expression into a flat list of nodes
///
/// Walks the AST bottom-up (like prettier's `rec()` function) to flatten
/// nested member/call chains into execution order.
///
/// Example: `a().b().c!.d` produces:
/// [Base(a), Call(), Member(.b), Call(), NonNull(!), Member(.d)]
pub fn linearize_chain<'a>(expr: &'a Expression) -> Vec<ChainNode<'a>> {
    let mut nodes = Vec::new();
    linearize_recursive(expr, &mut nodes);
    nodes
}

fn linearize_recursive<'a>(expr: &'a Expression, nodes: &mut Vec<ChainNode<'a>>) {
    match expr {
        // CallExpression: recurse into callee, then add Call node
        Expression::CallExpression(call) => {
            linearize_recursive(&call.callee, nodes);
            if call.optional {
                nodes.push(ChainNode::call_optional(expr));
            } else {
                nodes.push(ChainNode::call(expr));
            }
        }

        // MemberExpression: recurse into object, then add Member node
        Expression::MemberExpression(member) => {
            linearize_recursive(&member.object, nodes);

            let object_end = member.object.span().end;
            let property_start = member.property.span().start;
            if member.computed {
                nodes.push(ChainNode::computed_member(
                    &member.property,
                    member.optional,
                    object_end,
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
                ));
            }
        }

        // TSNonNullExpression: recurse into expression, then add NonNull node
        Expression::TSNonNullExpression(non_null) => {
            linearize_recursive(&non_null.expression, nodes);
            nodes.push(ChainNode::non_null());
        }

        // Base case: expression that's not part of the chain structure
        _ => {
            let needs_parens = needs_parens(expr, ParenContext::ChainBase);
            nodes.push(ChainNode::base(expr, needs_parens));
        }
    }
}

// =============================================================================
// Grouping
// =============================================================================

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

// =============================================================================
// Merge Logic
// =============================================================================

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

/// Check if an identifier name is a factory pattern (starts with capital letter)
///
/// Factory names like `Object`, `React`, `Observable` get merged with their first call.
/// Note: `$` and `_` prefixes are NOT treated as factory patterns by Prettier.
fn is_factory_name(symbol: DefaultSymbol, interner: &impl SymbolLookup) -> bool {
    let Some(name) = interner.lookup(symbol) else {
        return false;
    };
    let Some(first_char) = name.chars().next() else {
        return false;
    };
    first_char.is_uppercase()
}

/// Trait for looking up symbols (abstraction over interner)
pub trait SymbolLookup {
    fn lookup(&self, symbol: DefaultSymbol) -> Option<String>;
}

// =============================================================================
// Doc Building
// =============================================================================

/// Check if there are blank lines BETWEEN methods (not just before the first method)
///
/// Prettier's blank line rules:
/// - Blank line before first method ONLY (no other blank lines) → try to fit inline
/// - Blank lines BETWEEN methods (groups[2+]) → force expand
///
/// Returns true only if there are blank lines after the first method (groups index >= 2),
/// which is when we should force the expanded layout.
fn has_blank_lines_between_methods<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> bool {
    let source = printer.get_source();
    // Skip groups[0] (base) and groups[1] (first method) - only check groups[2+]
    groups.iter().skip(2).any(|group| {
        group
            .first_member_range()
            .is_some_and(|(obj_end, prop_start)| {
                has_blank_line_between(source, obj_end, prop_start)
            })
    })
}

/// Build a doc for a chain from grouped nodes
///
/// Implements prettier's chain doc building logic:
/// - Member-only chains: use fill() for greedy packing
/// - Chains with calls: use group-based breaking
/// - Short chains (≤cutoff groups): simple group with softlines
/// - Longer chains: conditionalGroup([oneLine, expanded])
/// - 3+ calls with complex args: force expanded (no width-based decision)
pub fn build_chain_doc<'a, P: ChainPrinter>(groups: &[ChainGroup<'a>], printer: &P) -> Doc {
    if groups.is_empty() {
        return doc::text("");
    }

    // Single group: just print it
    if groups.len() == 1 {
        return print_group(&groups[0], printer);
    }

    // Check if first group ends with "(complex)!" pattern
    // This takes priority over member-only vs call chain distinction because
    // the parenthesized head needs special break handling regardless of chain type.
    if is_parenthesized_non_null_head(groups) {
        return build_parenthesized_non_null_chain_doc(groups, printer);
    }

    // Collect all call nodes in the chain for the 3+ calls rule
    let call_nodes: Vec<&ChainNode<'a>> = groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .filter(|n| n.is_call())
        .collect();

    // Check if this is a member-only chain (no calls)
    let has_calls = !call_nodes.is_empty();

    // Prettier's logic (member-chain.js:351-359):
    // If groups.length <= cutoff && !nodeHasComment:
    //   return group(oneLine)  // Simple group, NO fill()
    // Else:
    //   return conditionalGroup([oneLine, expanded with hardline breaks])
    //
    // We match this: short member-only chains use simple group(), not fill()
    let should_merge = should_merge_first_groups(groups, printer);
    let cutoff = if should_merge { 3 } else { 2 };

    if !has_calls {
        // Member-only chain: use fill for greedy packing
        return build_member_only_chain_doc(groups, printer);
    }

    // Prettier's 3+ calls rule (member-chain.js:400-408):
    // If there are more than 2 call expressions AND at least one has non-simple arguments
    // (arrow functions, function expressions), force the expanded layout.
    // Blank lines BETWEEN methods (not just before first) also force expansion.
    // Additionally, force expand when inside template expressions with original breaks.
    let has_blank_lines_between = has_blank_lines_between_methods(groups, printer);
    let force_expand = has_blank_lines_between
        || (call_nodes.len() > 2 && call_nodes.iter().any(|n| call_has_complex_args(n, printer)))
        || printer.should_force_expand();

    // Split groups into first (merged) and rest based on should_merge
    let split_at = if should_merge { 2 } else { 1 }.min(groups.len());
    let (first_groups, rest_groups) = groups.split_at(split_at);

    // Build doc for first group(s) - merged when should_merge
    let first_doc = {
        let first_docs: Vec<Doc> = first_groups
            .iter()
            .map(|g| print_group(g, printer))
            .collect();
        doc::concat(first_docs)
    };

    // Check if first groups will break (e.g., multiline object/array arg)
    // Note: This is currently unused in the short chain path but kept for potential future use.
    let _first_will_break = doc::will_break(&first_doc);

    // Chains with calls use group-based breaking
    // Short chains: use group with softlines so it can break if needed
    if groups.len() <= cutoff && !force_expand {
        if rest_groups.is_empty() {
            return doc::group(first_doc);
        }

        // Note: We do NOT check first_will_break here. Prettier's short chain path
        // (groups.length <= cutoff) just uses group(oneLine) without checking for breaks.
        // The breaking check only applies in the longer chain path (groups.length > cutoff).

        // Check if first groups contain calls with multiple args that might need expansion
        let first_has_multiarg_calls = first_groups.iter().flat_map(|g| g.nodes.iter()).any(|n| {
            matches!(
                n,
                ChainNode::Call { expr: Expression::CallExpression(call), .. }
                if call.arguments.len() > 1
            )
        });

        // For short chains, prettier just concatenates groups directly WITHOUT softlines.
        // This ensures hardlines inside groups don't cause breaks between groups.
        let rest_docs: Vec<Doc> = rest_groups
            .iter()
            .map(|g| print_group(g, printer))
            .collect();
        let on_line = doc::concat(
            std::iter::once(first_doc)
                .chain(rest_docs.clone())
                .collect(),
        );

        // If first groups have multi-arg calls, use 3-state conditionalGroup
        // to try breaking args before breaking chain
        if first_has_multiarg_calls {
            // State 1: Args expanded, chain inline (args forced to break, chain stays)
            let first_expanded: Vec<Doc> = first_groups
                .iter()
                .map(|g| print_group_expanded(g, printer))
                .collect();
            let first_expanded_doc = doc::concat(first_expanded);

            let state_1 = doc::concat(
                std::iter::once(first_expanded_doc.clone())
                    .chain(rest_docs)
                    .collect(),
            );

            // State 2: Everything expanded (args broken, chain broken)
            let rest_parts_hard = build_rest_parts_with_comments(rest_groups, printer, true, true);
            let state_2 = doc::concat(vec![
                first_expanded_doc,
                doc::indent(doc::concat(rest_parts_hard)),
            ]);

            return doc::conditional_group(vec![on_line, state_1, state_2]);
        }

        return doc::group(on_line);
    }

    // Check if any group except the last will break
    // This matches prettier's `printedGroups.slice(0, -1).some(willBreak)` check
    let any_non_last_breaks = groups[..groups.len() - 1].iter().any(|g| {
        let doc = print_group(g, printer);
        doc::will_break(&doc)
    });

    // For longer chains (>cutoff), force expanded if any non-last group breaks
    let force_expand_from_breaking = any_non_last_breaks;

    // Build expanded variant
    let expanded = build_expanded_doc(groups, should_merge, printer);

    // If force_expand (3+ calls with arrow/function args) or first groups break
    // with 2+ trailing, force the expanded layout
    if force_expand || force_expand_from_breaking {
        return expanded;
    }

    // Print all groups inline (for oneLine variant)
    let on_line: Vec<Doc> = groups.iter().map(|g| print_group(g, printer)).collect();
    let on_line_doc = doc::concat(on_line);

    // Use conditionalGroup to let printer decide
    doc::conditional_group(vec![on_line_doc, expanded])
}

/// Check if a call node has complex (non-simple) arguments
///
/// Uses Prettier's `isSimpleCallArgument` logic (inverted) to determine
/// if a 3+ call chain should force break.
fn call_has_complex_args<'a, P: ChainPrinter>(node: &ChainNode<'a>, _printer: &P) -> bool {
    let ChainNode::Call { expr, .. } = node else {
        return false;
    };
    let Expression::CallExpression(call) = expr else {
        return false;
    };

    // Check if any argument is NOT simple (using Prettier's depth-limited check)
    call.arguments
        .iter()
        .any(|arg| !is_simple_call_argument(arg, 2))
}

/// Check if chain starts with "(complex)!" pattern that needs chain-preferring breaks
///
/// Returns true if:
/// - First group is [Base(needs_parens=true), NonNull]
/// - The base expression is one that we want to keep flat (await, ternary, type assertion)
///
/// For binary expressions, we return false because they have natural internal break
/// points (at operators) and should break there rather than at the chain.
fn is_parenthesized_non_null_head(groups: &[ChainGroup]) -> bool {
    if groups.len() < 2 {
        return false;
    }
    let first = &groups[0];
    if first.nodes.len() != 2 {
        return false;
    }

    // First node should be Base with needs_parens, second should be NonNull
    let ChainNode::Base {
        expr,
        needs_parens: true,
    } = &first.nodes[0]
    else {
        return false;
    };
    if !first.nodes[1].is_non_null() {
        return false;
    }

    // Check if the base expression is one we want to keep flat
    // Binary expressions have natural break points (at operators), so exclude them
    matches!(
        expr,
        Expression::AwaitExpression(_)
            | Expression::ConditionalExpression(_)
            | Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSTypeAssertion(_)
    )
}

/// Build doc for "(complex)!.method()" chains
///
/// Uses conditional_group to prefer breaking at the chain point rather than
/// inside the parenthesized expression. This matches prettier's behavior:
/// - First tries to fit everything on one line
/// - If that doesn't fit, breaks at the chain point (after `!)`, keeping inner expression flat
///
/// Example:
/// ```text
/// // Fits on one line:
/// (await func(args))!.property;
///
/// // Doesn't fit - breaks at chain:
/// (await func(args))!
///     .property;
///
/// // NOT (breaking inside parens):
/// (await func(
///     args,
/// ))!.property;
/// ```
fn build_parenthesized_non_null_chain_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> Doc {
    // First group: (complex)!
    let first_group_doc = print_group(&groups[0], printer);

    if groups.len() == 1 {
        return first_group_doc;
    }

    // Rest: .method().prop etc
    let rest_docs: Vec<Doc> = groups[1..]
        .iter()
        .map(|g| print_group(g, printer))
        .collect();

    // oneLine: everything concatenated flat
    let on_line = doc::concat(
        std::iter::once(first_group_doc.clone())
            .chain(rest_docs.clone())
            .collect(),
    );

    // If the first group will break internally (e.g., binary || expression),
    // just use group(oneLine) and let the inner expression break.
    // Don't force chain breaks in this case.
    //
    // For expressions that DON'T break internally (await, ternary, type assertion),
    // we want conditionalGroup to prefer chain breaks over inner group breaks.
    if doc::will_break(&first_group_doc) {
        // Inner expression will break - just wrap in group and let it break naturally
        return doc::group(on_line);
    }

    // expanded: first group + hardline + indent + rest groups
    // This breaks at the chain point, keeping inner expression flat
    let mut rest_parts = Vec::new();
    for rest_doc in rest_docs {
        rest_parts.push(doc::hardline());
        rest_parts.push(rest_doc);
    }
    let expanded = doc::concat(vec![first_group_doc, doc::indent(doc::concat(rest_parts))]);

    doc::conditional_group(vec![on_line, expanded])
}

/// Build doc for member-only chains using fill for greedy packing
///
/// Break points are ONLY at member access (`.foo`), not at non-null (`!`).
/// This ensures `.foo!` stays together as a unit.
///
/// Example: `a!.b!.c!` breaks as:
/// ```text
/// a!.b!
///     .c!
/// ```
/// NOT as:
/// ```text
/// a!
///     .b!
///     .c!
/// ```
fn build_member_only_chain_doc<'a, P: ChainPrinter>(groups: &[ChainGroup<'a>], printer: &P) -> Doc {
    // Flatten all nodes into individual segments
    let all_nodes: Vec<&ChainNode<'a>> = groups.iter().flat_map(|g| g.nodes.iter()).collect();

    if all_nodes.is_empty() {
        return doc::text("");
    }

    // Note: We intentionally do NOT check for blank lines here.
    // Blank lines in member-only chains are normalized (removed) - they don't
    // affect the formatting output. The fill-based approach below handles
    // line breaking based on width, which is the correct behavior.

    // For member-only chains, build first_doc from just the base identifier
    // and any immediately following non-null assertions (not the entire first group).
    // This ensures all member accesses become fill segments that can be wrapped.
    //
    // The grouping logic puts almost all members in the first group (for the
    // "short chain fits on one line" case), but for fill-based breaking we need
    // each member access to be a separate segment.
    let mut first_doc_end = 0;
    for (i, node) in all_nodes.iter().enumerate() {
        if node.is_member() {
            // Stop at first member - that starts the fill segments
            break;
        }
        first_doc_end = i + 1;
    }

    // Build first_doc from base + any trailing non-null assertions
    let first_doc_nodes: Vec<Doc> = all_nodes
        .iter()
        .take(first_doc_end)
        .map(|n| print_node(n, printer))
        .collect();
    let first_doc = if first_doc_nodes.is_empty() {
        doc::text("")
    } else {
        doc::concat(first_doc_nodes)
    };

    // If no remaining nodes after first_doc, just return it
    if first_doc_end >= all_nodes.len() {
        return first_doc;
    }

    // Build segments where each segment ENDS with a member access
    // Break points are BEFORE each member access (softlines in fill)
    //
    // Example: `a!.b!.c!` with nodes [Base(a), NonNull, Member(.b), NonNull, Member(.c), NonNull]
    //   first_doc = "a!"
    //   remaining nodes: [Member(.b), NonNull, Member(.c), NonNull]
    //   segments = [".b!", ".c!"]
    //   result = group(first + indent(fill([.b!, softline, .c!])))
    //
    // Fill packs segments greedily - as many as fit on each line.

    // Build segments by collecting nodes until we see the NEXT member
    // Each segment includes everything up to and including a member (+ trailing non-null)
    let remaining_nodes = &all_nodes[first_doc_end..];
    let mut segments: Vec<Doc> = Vec::new();
    let mut current_segment: Vec<Doc> = Vec::new();
    let mut seen_member = false;

    for (i, node) in remaining_nodes.iter().enumerate() {
        // Check if this is a member and we already have content that includes a member
        // If so, flush before adding this member
        if node.is_member() && seen_member {
            segments.push(doc::concat(std::mem::take(&mut current_segment)));
            seen_member = false;
        }

        current_segment.push(print_node(node, printer));

        if node.is_member() {
            seen_member = true;
        }

        // If this is the last node, flush
        if i == remaining_nodes.len() - 1 && !current_segment.is_empty() {
            segments.push(doc::concat(std::mem::take(&mut current_segment)));
        }
    }

    // If no segments, just return the first doc
    if segments.is_empty() {
        return first_doc;
    }

    // If only one segment, attach it directly (e.g., `a!` or `a!.b!`)
    if segments.len() == 1 {
        let Some(segment) = segments.pop() else {
            return first_doc;
        };
        return doc::concat(vec![first_doc, segment]);
    }

    // For 2+ segments: use conditional_group for proper break decisions
    // fill() handles greedy packing - it prints as many items as fit on
    // the current line, then breaks and continues.
    //
    // conditional_group([oneLine, expanded]) where:
    // - oneLine: all segments concatenated flat (no breaks)
    // - expanded: first_doc + indent(fill(segments with softlines))
    //
    // The fill starts at the position after first_doc and greedily packs.
    // Since fill is inside indent, overflow lines get the extra indent.

    // Build on_line: everything concatenated flat
    // Note: on_line does NOT need trailing_reserve because fits_with_lookahead
    // already sees trailing content (comma, etc.) in rest_commands with the
    // correct mode (Break → "," is counted).
    let mut on_line_parts = vec![first_doc.clone()];
    for segment in &segments {
        on_line_parts.push(segment.clone());
    }
    let on_line = doc::concat(on_line_parts);

    // Build fill_parts with softlines between segments
    let mut fill_parts = Vec::new();
    for segment in &segments {
        if !fill_parts.is_empty() {
            fill_parts.push(doc::softline());
        }
        fill_parts.push(segment.clone());
    }

    // Expanded: first_doc + indent(fill(segments))
    // Fill packs greedily until overflow, then breaks with indent.
    // trailing_reserve: 1 accounts for trailing punctuation (comma, semicolon).
    let expanded = doc::concat(vec![
        first_doc,
        doc::indent(doc::with_context(
            doc::fill(fill_parts),
            doc::DocContext {
                trailing_reserve: 1,
                base_indent_override: None,
            },
        )),
    ]);

    doc::conditional_group(vec![on_line, expanded])
}

/// Build an expanded chain doc with first group(s) inline and rest indented
///
/// Common pattern for expanded chains: first group(s) + hardline + indent(rest)
fn build_expanded_chain_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    split_at: usize,
    printer: &P,
) -> Doc {
    if groups.is_empty() {
        return doc::text("");
    }

    let (first_groups, rest) = groups.split_at(split_at.min(groups.len()));

    // Print first group(s) inline
    let first_docs: Vec<Doc> = first_groups
        .iter()
        .map(|g| print_group(g, printer))
        .collect();
    let first_doc = doc::concat(first_docs);

    if rest.is_empty() {
        return first_doc;
    }

    // Print rest with hardlines and indent (including trailing comments and blank line preservation)
    let rest_parts = build_rest_parts_with_comments(rest, printer, true, false);

    doc::concat(vec![first_doc, doc::indent(doc::concat(rest_parts))])
}

/// Build the expanded doc variant (first group(s) + indented rest)
fn build_expanded_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    should_merge: bool,
    printer: &P,
) -> Doc {
    let split_at = if should_merge { 2 } else { 1 };
    build_expanded_chain_doc(groups, split_at, printer)
}

/// Print a single chain group
fn print_group<'a, P: ChainPrinter>(group: &ChainGroup<'a>, printer: &P) -> Doc {
    let docs: Vec<Doc> = group.nodes.iter().map(|n| print_node(n, printer)).collect();
    doc::concat(docs)
}

/// Build a line break doc with optional blank line preservation
///
/// Returns:
/// - `softline` when `use_hardline` is false
/// - `hardline` when no blank line in source
/// - `literalline + hardline` when blank line should be preserved
fn build_chain_line_break<P: ChainPrinter>(
    printer: &P,
    object_end: u32,
    property_start: u32,
    use_hardline: bool,
) -> Doc {
    if !use_hardline {
        return doc::softline();
    }

    // Check for blank line preservation (only when no comments - comments handle their own spacing)
    // When there are comments between obj and property, the 2+ newlines (one before comment,
    // one after) should NOT be treated as a blank line.
    let source = printer.get_source();
    let has_comments = printer.has_comments_between(object_end, property_start);

    if !has_comments && has_blank_line_between(source, object_end, property_start) {
        // Preserve blank line: literalline (no indent) + hardline (with indent for next content)
        doc::concat(vec![doc::literalline(), doc::hardline()])
    } else {
        doc::hardline()
    }
}

/// Builder for constructing chain parts with proper comment handling.
///
/// Encapsulates the logic for interleaving comments, line breaks, and groups
/// when building the rest of a chain (everything after the first group).
struct ChainPartsBuilder<'a, 'p, P: ChainPrinter> {
    parts: Vec<Doc>,
    printer: &'p P,
    use_hardline: bool,
    use_expanded: bool,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a, 'p, P: ChainPrinter> ChainPartsBuilder<'a, 'p, P> {
    fn new(printer: &'p P, use_hardline: bool, use_expanded: bool, group_count: usize) -> Self {
        Self {
            // Each group produces ~5 docs: trailing comments, line break, block comments,
            // leading comments, and the group doc itself
            parts: Vec::with_capacity(group_count * 5),
            printer,
            use_hardline,
            use_expanded,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add a group with its associated comments and line breaks
    fn add_group(&mut self, group: &ChainGroup<'a>) {
        self.add_comments_and_break(group);
        self.add_group_doc(group);
    }

    /// Add trailing comments, line break, and leading comments before a group
    fn add_comments_and_break(&mut self, group: &ChainGroup<'a>) {
        if let Some((object_end, property_start)) = group.first_member_range() {
            // Trailing line comments (on the same line as previous element)
            self.parts
                .push(self.printer.build_trailing_line_comments_doc(object_end, property_start));

            // Line break with blank line preservation
            self.parts.push(build_chain_line_break(
                self.printer,
                object_end,
                property_start,
                self.use_hardline,
            ));

            // Leading comments (between prev and this element, on their own lines)
            self.parts.push(self.printer.build_block_comments_doc(
                object_end,
                property_start,
                super::CommentSpacing::Leading,
            ));
            self.parts.push(
                self.printer
                    .build_leading_line_comments_doc(object_end, property_start),
            );
        } else {
            // No member range - just add line break
            self.parts.push(if self.use_hardline {
                doc::hardline()
            } else {
                doc::softline()
            });
        }
    }

    /// Add the group's doc (either expanded or normal)
    fn add_group_doc(&mut self, group: &ChainGroup<'a>) {
        self.parts.push(if self.use_expanded {
            print_group_expanded(group, self.printer)
        } else {
            print_group(group, self.printer)
        });
    }

    fn build(self) -> Vec<Doc> {
        self.parts
    }
}

/// Build rest parts with comments and blank line preservation
/// Handles both trailing line comments (same line) and leading line comments (own line)
/// Emits: [trailing_comments?, line_break, leading_comments?, group] for each rest group
fn build_rest_parts_with_comments<'a, P: ChainPrinter>(
    rest_groups: &[ChainGroup<'a>],
    printer: &P,
    use_hardline: bool,
    use_expanded: bool,
) -> Vec<Doc> {
    let mut builder = ChainPartsBuilder::new(printer, use_hardline, use_expanded, rest_groups.len());
    for group in rest_groups {
        builder.add_group(group);
    }
    builder.build()
}

/// Print a member access (shared logic for Member and PrivateMember)
fn print_member_access<P: ChainPrinter>(
    printer: &P,
    property: DefaultSymbol,
    optional: bool,
    object_end: u32,
    property_start: u32,
    is_private: bool,
) -> Doc {
    let prop_name = printer.lookup_symbol(property);
    let block_comments_doc = printer.build_block_comments_doc(
        object_end,
        property_start,
        super::CommentSpacing::Leading,
    );
    let hash = if is_private { "#" } else { "" };
    let dot = if optional { "?." } else { "." };
    doc::concat(vec![
        block_comments_doc,
        doc::text_owned(format!("{dot}{hash}{prop_name}")),
    ])
}

/// Print a single chain node
fn print_node<'a, P: ChainPrinter>(node: &ChainNode<'a>, printer: &P) -> Doc {
    match node {
        ChainNode::Base { expr, needs_parens } => {
            if *needs_parens {
                printer.print_parenthesized_base(expr)
            } else {
                printer.print_expression(expr)
            }
        }

        ChainNode::Call { expr, optional } => {
            if let Expression::CallExpression(call) = expr {
                printer.print_call_args(call, *optional)
            } else {
                doc::text("()")
            }
        }

        ChainNode::Member {
            property,
            optional,
            object_end,
            property_start,
        } => print_member_access(
            printer,
            *property,
            *optional,
            *object_end,
            *property_start,
            false,
        ),

        ChainNode::PrivateMember {
            property,
            optional,
            object_end,
            property_start,
        } => print_member_access(
            printer,
            *property,
            *optional,
            *object_end,
            *property_start,
            true,
        ),

        ChainNode::ComputedMember {
            expr,
            optional,
            object_end,
        } => {
            let inner = printer.print_expression(expr);
            let prop_start = printer.get_property_span(expr).start;
            // Block comments go INSIDE the brackets for computed members: obj[/* c */ key]
            // Use trailing space (not leading) since we're right after `[`
            let block_comments_doc = printer.build_block_comments_doc(
                *object_end,
                prop_start,
                super::CommentSpacing::Trailing,
            );
            let inner_with_comments = doc::concat(vec![block_comments_doc, inner]);
            if *optional {
                doc::concat(vec![doc::text("?.["), inner_with_comments, doc::text("]")])
            } else {
                doc::brackets(inner_with_comments)
            }
        }

        ChainNode::NonNull => doc::text("!"),
    }
}

/// Print a single chain node with forced call expansion
fn print_node_expanded<'a, P: ChainPrinter>(node: &ChainNode<'a>, printer: &P) -> Doc {
    match node {
        ChainNode::Call { expr, optional } => {
            if let Expression::CallExpression(call) = expr {
                printer.print_call_args_expanded(call, *optional)
            } else {
                doc::text("()")
            }
        }
        // All other nodes print the same way
        _ => print_node(node, printer),
    }
}

/// Print a chain group with forced call expansion
fn print_group_expanded<'a, P: ChainPrinter>(group: &ChainGroup<'a>, printer: &P) -> Doc {
    let docs: Vec<Doc> = group
        .nodes
        .iter()
        .map(|n| print_node_expanded(n, printer))
        .collect();
    doc::concat(docs)
}

/// Trait for printing chain elements (abstraction over Printer)
pub trait ChainPrinter: SymbolLookup {
    /// Print an expression as a Doc
    fn print_expression(&self, expr: &Expression) -> Doc;

    /// Print a parenthesized base expression with indent-on-break behavior
    fn print_parenthesized_base(&self, expr: &Expression) -> Doc;

    /// Print call arguments: () or (arg1, arg2)
    fn print_call_args(&self, call: &internal::CallExpression, optional: bool) -> Doc;

    /// Print call arguments with forced expansion (hardlines)
    /// Used for the "args broken, chain inline" state in conditionalGroup
    fn print_call_args_expanded(&self, call: &internal::CallExpression, optional: bool) -> Doc;

    /// Look up a symbol to its string representation
    fn lookup_symbol(&self, symbol: DefaultSymbol) -> String;

    /// Build a doc for inline block comments between two positions
    /// Returns a doc with block comments using the specified spacing
    /// Note: Only block comments are included (line comments are filtered out)
    fn build_block_comments_doc(&self, start: u32, end: u32, spacing: super::CommentSpacing)
    -> Doc;

    /// Build a line_suffix doc for trailing line comments between two positions
    /// Returns empty doc if no line comments found
    fn build_trailing_line_comments_doc(&self, start: u32, end: u32) -> Doc;

    /// Build a doc for leading line comments (comments on their own line)
    /// Returns empty doc if no line comments found
    fn build_leading_line_comments_doc(&self, start: u32, end: u32) -> Doc;

    /// Get the span for a given expression
    fn get_property_span(&self, expr: &Expression) -> Span;

    /// Check if the chain is the direct child of an ExpressionStatement
    ///
    /// Used to determine if short identifier names should be merged with their
    /// first call (e.g., `a.fn().b()` → merge `a` with `.fn()` only in statements).
    fn is_expression_statement(&self) -> bool;

    /// Get the source code
    fn get_source(&self) -> &str;

    /// Check if there are any comments between two positions
    fn has_comments_between(&self, start: u32, end: u32) -> bool;

    /// Get the tab width from config
    fn get_tab_width(&self) -> usize;

    /// Check if chain expansion should be forced
    ///
    /// Used when inside template expressions with original breaks, where the
    /// expression is too long for the remaining print width.
    fn should_force_expand(&self) -> bool;
}
