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

use crate::ast::internal::{self, Expression, LiteralValue};
use string_interner::DefaultSymbol;
use tsv_lang::Span;
use tsv_lang::doc::{self, Doc};

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
    pub fn is_call(&self) -> bool {
        matches!(self, Self::Call { .. })
    }

    /// Check if this is a member node (including computed)
    pub fn is_member(&self) -> bool {
        matches!(
            self,
            Self::Member { .. } | Self::PrivateMember { .. } | Self::ComputedMember { .. }
        )
    }

    /// Check if this is a non-null node
    pub fn is_non_null(&self) -> bool {
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
    pub fn is_computed(&self) -> bool {
        matches!(self, Self::ComputedMember { .. })
    }

    /// Get property symbol for Member nodes
    pub fn property(&self) -> Option<DefaultSymbol> {
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
            let needs_parens = base_needs_parens(expr);
            nodes.push(ChainNode::base(expr, needs_parens));
        }
    }
}

/// Check if a base expression needs parentheses when used in a chain
///
/// Expressions with lower precedence than member access need parens:
/// - Binary expressions: `(a + b).method()`
/// - Conditional expressions: `(a ? b : c).method()`
/// - Assignment expressions: `(a = b).method()`
/// - Await expressions: `(await promise).method()`
/// - Sequence expressions (already have parens from parsing)
fn base_needs_parens(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::BinaryExpression(_)
            | Expression::ConditionalExpression(_)
            | Expression::AssignmentExpression(_)
            | Expression::AwaitExpression(_)
            | Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSTypeAssertion(_)
    )
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
pub fn should_merge_first_groups<'a>(
    groups: &[ChainGroup<'a>],
    interner: &impl SymbolLookup,
) -> bool {
    if groups.len() < 2 {
        return false;
    }

    // Don't merge if second group's first node has comments (not implemented yet)
    // if has_comment(&groups[1].nodes[0]) { return false; }

    should_not_wrap(groups, interner)
}

/// Check if chain should NOT wrap between first and second groups
///
/// Corresponds to prettier's `shouldNotWrap` logic:
/// - Single base that's `this`, factory identifier, or short name
/// - Multiple nodes where last is member with factory property
pub fn should_not_wrap<'a>(groups: &[ChainGroup<'a>], interner: &impl SymbolLookup) -> bool {
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
            // this.method() → merge
            Expression::Super(_) => true,

            // Object.keys() → merge (capital letter = factory)
            // d3.scale() → merge (short name in expression statement context)
            Expression::Identifier(id) => is_factory_name(id.name, interner) || has_computed,

            _ => has_computed,
        }
    } else {
        // Multiple nodes in first group: check if last is member with factory property
        if let Some(prop) = first.nodes.last().and_then(ChainNode::property) {
            return is_factory_name(prop, interner) || has_computed;
        }
        false
    }
}

/// Check if an identifier name is a factory pattern (capital letter or $ / _)
///
/// Factory names like `Object`, `React`, `Observable` get merged with their first call.
fn is_factory_name(symbol: DefaultSymbol, interner: &impl SymbolLookup) -> bool {
    let Some(name) = interner.lookup(symbol) else {
        return false;
    };
    let Some(first_char) = name.chars().next() else {
        return false;
    };
    // Capital letter or starts with $ or _
    first_char.is_uppercase() || first_char == '$' || first_char == '_'
}

/// Trait for looking up symbols (abstraction over interner)
pub trait SymbolLookup {
    fn lookup(&self, symbol: DefaultSymbol) -> Option<String>;
}

// =============================================================================
// Doc Building
// =============================================================================

/// Build a doc for a chain from grouped nodes
///
/// Implements prettier's chain doc building logic:
/// - Member-only chains: use fill() for greedy packing
/// - Chains with calls: use group-based breaking
/// - Short chains (≤cutoff groups): simple group with softlines
/// - Longer chains: conditionalGroup([oneLine, expanded])
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

    // Check if this is a member-only chain (no calls)
    let has_calls = groups
        .iter()
        .any(|g| g.nodes.iter().any(ChainNode::is_call));

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

    // Chains with calls use group-based breaking
    // Short chains: use group with softlines so it can break if needed
    if groups.len() <= cutoff {
        // Build: first_group + indent(softline + second_group + ...)
        let first_group_doc = print_group(&groups[0], printer);

        if groups.len() == 1 {
            return first_group_doc;
        }

        // Build rest with softlines (breaks become newlines when group breaks)
        let mut rest_parts = Vec::new();
        for group in &groups[1..] {
            rest_parts.push(doc::softline());
            rest_parts.push(print_group(group, printer));
        }

        return doc::group(doc::concat(vec![
            first_group_doc,
            doc::indent(doc::concat(rest_parts)),
        ]));
    }

    // Print all groups inline (for oneLine variant)
    let on_line: Vec<Doc> = groups.iter().map(|g| print_group(g, printer)).collect();
    let on_line_doc = doc::concat(on_line);

    // Build expanded variant
    let expanded = build_expanded_doc(groups, should_merge, printer);

    // Use conditionalGroup to let printer decide
    doc::conditional_group(vec![on_line_doc, expanded])
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

    // Build context for both variants to ensure symmetric constraint checking.
    // Reserve 1 char to prevent packing to exactly printWidth, which could be
    // exceeded by trailing content (commas, semicolons, brackets).
    let context = doc::DocContext {
        trailing_reserve: 1,
    };

    // Build on_line: everything concatenated flat, wrapped with context
    let mut on_line_parts = vec![first_doc.clone()];
    for segment in &segments {
        on_line_parts.push(segment.clone());
    }
    let on_line = doc::with_context(doc::concat(on_line_parts), context.clone());

    // Build fill_parts with softlines between segments
    let mut fill_parts = Vec::new();
    for segment in &segments {
        if !fill_parts.is_empty() {
            fill_parts.push(doc::softline());
        }
        fill_parts.push(segment.clone());
    }

    // Expanded: first_doc + indent(fill(segments)), with same context
    // The fill starts right after first_doc at current position.
    // It packs greedily until overflow, then breaks with indent.
    let expanded = doc::concat(vec![
        first_doc,
        doc::indent(doc::with_context(doc::fill(fill_parts), context)),
    ]);

    doc::conditional_group(vec![on_line, expanded])
}

/// Build the expanded doc variant (first group(s) + indented rest)
fn build_expanded_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    should_merge: bool,
    printer: &P,
) -> Doc {
    let split_at = if should_merge { 2 } else { 1 };
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

    // Print rest with hardlines and indent
    let mut rest_parts = Vec::new();
    for group in rest {
        rest_parts.push(doc::hardline());
        rest_parts.push(print_group(group, printer));
    }

    doc::concat(vec![first_doc, doc::indent(doc::concat(rest_parts))])
}

/// Print a single chain group
fn print_group<'a, P: ChainPrinter>(group: &ChainGroup<'a>, printer: &P) -> Doc {
    let docs: Vec<Doc> = group.nodes.iter().map(|n| print_node(n, printer)).collect();
    doc::concat(docs)
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
        } => {
            let prop_name = printer.lookup_symbol(*property);
            // Check for comments between object end and property start (e.g., `a /* c */ .b`)
            let comments_doc = printer.build_block_comments_doc(
                *object_end,
                *property_start,
                super::CommentSpacing::Leading,
            );
            if *optional {
                doc::concat(vec![
                    comments_doc,
                    doc::text_owned(format!("?.{prop_name}")),
                ])
            } else {
                doc::concat(vec![comments_doc, doc::text_owned(format!(".{prop_name}"))])
            }
        }

        ChainNode::PrivateMember {
            property,
            optional,
            object_end,
            property_start,
        } => {
            let prop_name = printer.lookup_symbol(*property);
            let comments_doc = printer.build_block_comments_doc(
                *object_end,
                *property_start,
                super::CommentSpacing::Leading,
            );
            if *optional {
                doc::concat(vec![
                    comments_doc,
                    doc::text_owned(format!("?.#{prop_name}")),
                ])
            } else {
                doc::concat(vec![
                    comments_doc,
                    doc::text_owned(format!(".#{prop_name}")),
                ])
            }
        }

        ChainNode::ComputedMember {
            expr,
            optional,
            object_end,
        } => {
            let inner = printer.print_expression(expr);
            let prop_start = printer.get_property_span(expr).start;
            // Comments go INSIDE the brackets for computed members: obj[/* c */ key]
            // Use trailing space (not leading) since we're right after `[`
            let comments_doc = printer.build_block_comments_doc(
                *object_end,
                prop_start,
                super::CommentSpacing::Trailing,
            );
            let inner_with_comments = doc::concat(vec![comments_doc, inner]);
            if *optional {
                doc::concat(vec![doc::text("?.["), inner_with_comments, doc::text("]")])
            } else {
                doc::brackets(inner_with_comments)
            }
        }

        ChainNode::NonNull => doc::text("!"),
    }
}

/// Trait for printing chain elements (abstraction over Printer)
pub trait ChainPrinter: SymbolLookup {
    /// Print an expression as a Doc
    fn print_expression(&self, expr: &Expression) -> Doc;

    /// Print a parenthesized base expression with indent-on-break behavior
    fn print_parenthesized_base(&self, expr: &Expression) -> Doc;

    /// Print call arguments: () or (arg1, arg2)
    fn print_call_args(&self, call: &internal::CallExpression, optional: bool) -> Doc;

    /// Look up a symbol to its string representation
    fn lookup_symbol(&self, symbol: DefaultSymbol) -> String;

    /// Build a doc for inline block comments between two positions
    /// Returns a doc with block comments using the specified spacing
    /// Note: Only block comments are included (line comments are filtered out)
    fn build_block_comments_doc(&self, start: u32, end: u32, spacing: super::CommentSpacing)
    -> Doc;

    /// Get the span for a given expression
    fn get_property_span(&self, expr: &Expression) -> Span;
}
