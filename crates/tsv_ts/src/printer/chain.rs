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

use super::utils::{contains_call_expression, is_simple_call_argument};
use crate::ast::internal::{self, ArrowFunctionBody, Expression, LiteralValue};
use crate::printer::{ParenContext, needs_parens};
use string_interner::DefaultSymbol;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::{self, has_blank_line_between_fast};
use tsv_lang::{ClassifiedComments, Span, SymbolToU32};

/// Check if a function parameter has a type annotation.
fn has_param_type_annotation(param: &Expression) -> bool {
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

//
// Data Structures
//

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
    /// `bracket_end` is the position just before the closing `]` (for trailing comment detection)
    ComputedMember {
        expr: &'a Expression,
        optional: bool,
        object_end: u32,
        bracket_end: u32,
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
    pub fn computed_member(
        expr: &'a Expression,
        optional: bool,
        object_end: u32,
        bracket_end: u32,
    ) -> Self {
        Self::ComputedMember {
            expr,
            optional,
            object_end,
            bracket_end,
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

    /// Get the CallExpression if this is a Call node
    pub fn as_call_expression(&self) -> Option<&internal::CallExpression> {
        if let Self::Call { expr, .. } = self
            && let Expression::CallExpression(call) = expr
        {
            return Some(call);
        }
        None
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

//
// Doc Building
//

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
fn has_comments_forcing_expansion<'a, P: ChainPrinter>(
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
        return doc::empty();
    }

    // Single group: just print it
    if groups.len() == 1 {
        return print_group(&groups[0], printer);
    }

    // Check if first group is "(await ...)" pattern (without NonNull)
    // This takes priority because the parenthesized await needs special break handling.
    if is_parenthesized_await_head(groups) {
        return build_parenthesized_await_chain_doc(groups, printer);
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

    // Prettier's chain expansion rules (member-chain.js:400-408):
    // 1. Blank lines BETWEEN methods (not just before first) force expansion
    // 2. 3+ calls with complex args force expansion
    // 3. 2+ calls with callbacks, where any callback has a multiline body, force expansion
    // 4. Inside template expressions with original breaks force expansion
    let has_blank_lines_between = has_blank_lines_between_methods(groups, printer);

    // Single pass: count callbacks and check if any breaks
    let line_breaks = printer.get_line_breaks();
    let (calls_with_callbacks, any_callback_breaks) =
        call_nodes
            .iter()
            .fold((0usize, false), |(count, any_breaks), node| {
                let status = call_callback_status(node, line_breaks);
                (
                    count + usize::from(status.has_callback),
                    any_breaks || status.will_break,
                )
            });

    // Comments between chain segments force expansion, EXCEPT for comments before
    // trailing members (which are handled specially by add_group_no_break)
    let has_forcing_comments = has_comments_forcing_expansion(groups, printer);

    let force_expand = has_blank_lines_between
        || has_forcing_comments
        || (call_nodes.len() > 2 && call_nodes.iter().any(|n| call_has_complex_args(n)))
        || (calls_with_callbacks >= 2 && any_callback_breaks)
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
            std::iter::once(first_doc.clone())
                .chain(rest_docs.clone())
                .collect(),
        );

        // Check if first groups contain any calls (regardless of arg count)
        let first_has_calls = first_groups
            .iter()
            .flat_map(|g| g.nodes.iter())
            .any(ChainNode::is_call);

        // If first groups have multi-arg calls, use 4-state conditionalGroup.
        // Prettier tries these in order until one fits:
        // 0. Everything flat (on_line)
        // 1. First args inline, last call's args expanded (for short first args, long total)
        // 2. First args expanded, last call flexible (for long first args)
        // 3. Everything expanded (first args broken, chain broken)
        if first_has_multiarg_calls {
            // State 1: First args inline, rest groups with expanded call args
            // For `expect(short_args).toBe(false)` where total exceeds width but first fits
            let rest_expanded_inline: Vec<Doc> = rest_groups
                .iter()
                .map(|g| print_group_expanded(g, printer))
                .collect();
            let state_last_expanded = doc::concat(
                std::iter::once(first_doc)
                    .chain(rest_expanded_inline)
                    .collect(),
            );

            // State 2: First call's args expanded, rest groups flexible
            // For cases like `fn(long, args).method(true)` where first call needs to break
            let first_expanded: Vec<Doc> = first_groups
                .iter()
                .map(|g| print_group_expanded(g, printer))
                .collect();
            let first_expanded_doc = doc::concat(first_expanded);

            let state_first_expanded = doc::concat(
                std::iter::once(first_expanded_doc.clone())
                    .chain(rest_docs)
                    .collect(),
            );

            // State 3: Everything expanded (first args broken, chain broken)
            let rest_parts_hard = build_rest_parts_with_comments(rest_groups, printer, true, true);
            let state_all_expanded = doc::concat(vec![
                first_expanded_doc,
                doc::indent(doc::concat(rest_parts_hard)),
            ]);

            return doc::conditional_group(vec![
                on_line,
                state_last_expanded,
                state_first_expanded,
                state_all_expanded,
            ]);
        }

        // Single-arg first calls with NON-CALL args: use 2-state conditionalGroup.
        // Prettier prefers breaking at the LAST call when the first call has a simple arg.
        // 0. Everything flat (on_line)
        // 1. First call inline, last call's args expanded
        // This handles patterns like `fn(a.b()).call(longTemplate)` where Prettier
        // keeps `fn(a.b())` together and breaks at `.call(`.
        //
        // IMPORTANT: Only apply this when the first call's single arg does NOT contain
        // nested calls. When the first call's arg is itself a call (or contains calls),
        // that inner call may need to break, so let each group format independently -
        // the last call stays inline if it fits.
        //
        // Example where we DON'T want to force last-call expansion:
        //   fn(complex_call(long, args)).c.d(x)
        // → fn(
        //     complex_call(long, args),
        //   ).c.d(x);  // .c.d(x) stays inline because x fits
        let first_call_arg_contains_call = first_groups
            .iter()
            .flat_map(|g| g.nodes.iter())
            .filter_map(ChainNode::as_call_expression)
            .any(|call| call.arguments.iter().any(contains_call_expression));

        // Check if chain ends with member (for callback arg breaking preference)
        let chain_ends_with_member = ends_with_member(rest_groups, first_groups);

        // When chain ends with member and first groups have calls, prefer expanding
        // first groups' call args over breaking the chain.
        // e.g., `Object.keys(obj).filter(\n  cb,\n).length` not `Object.keys(\n  obj,\n).filter(...)`
        if first_has_calls && chain_ends_with_member {
            let first_expanded: Vec<Doc> = first_groups
                .iter()
                .map(|g| print_group_expanded(g, printer))
                .collect();
            let first_expanded_doc = doc::concat(first_expanded);
            let state_first_expanded = doc::concat(
                std::iter::once(first_expanded_doc)
                    .chain(rest_docs)
                    .collect(),
            );

            return doc::conditional_group(vec![on_line, state_first_expanded]);
        }

        // Prettier's short chain behavior (member-chain.js lines 351-360):
        // For chains with groups.length <= cutoff, just return group(oneLine).
        // The simple group lets the chain stay flat while internal call arguments
        // can break independently. The assignment's Fluid layout controls whether
        // to break after `=`.
        //
        // Our special handling below is for cases where first groups have calls
        // that may need expansion. When first groups have NO calls (just member
        // accesses), we use simple group to match prettier.
        if !first_has_calls {
            return doc::group(on_line);
        }

        if !first_call_arg_contains_call {
            // For factory patterns (shouldMerge), use simple group instead of conditional_group.
            // This lets the assignment decide the break point (break after `=`) rather than
            // having the chain expand call args.
            //
            // Example: `const A = Factory.method(x).build(y);`
            // - With conditional_group: tries expanding last call's args → `...build(\n  y,\n)`
            // - With simple group: lets assignment break → `const A =\n  Factory.method(x).build(y);`
            //
            // The latter matches Prettier's behavior for factory patterns.
            if should_merge {
                return doc::group(on_line);
            }

            let rest_expanded_inline: Vec<Doc> = rest_groups
                .iter()
                .map(|g| print_group_expanded(g, printer))
                .collect();
            let state_last_expanded = doc::concat(
                std::iter::once(first_doc)
                    .chain(rest_expanded_inline)
                    .collect(),
            );

            return doc::conditional_group(vec![on_line, state_last_expanded]);
        }

        // When first call's arg contains calls (e.g., `fn(a.b(long, args)).c.d(x)`),
        // we need to handle two cases:
        // 1. The last call needs to break → first groups inline, rest expands
        // 2. The inner call needs to break → first groups expand, rest stays inline
        //
        // Prettier prefers breaking at the END (last groups) before breaking at the
        // beginning (first groups), so we try state_last_expanded before state_first_expanded.
        //
        // Build both states to try in order:
        // State 1: first groups inline, rest expanded
        let rest_expanded_inline: Vec<Doc> = rest_groups
            .iter()
            .map(|g| print_group_expanded(g, printer))
            .collect();
        let state_last_expanded = doc::concat(
            std::iter::once(first_doc)
                .chain(rest_expanded_inline)
                .collect(),
        );

        // State 2: first groups expanded (inner call breaks), rest inline
        let first_expanded: Vec<Doc> = first_groups
            .iter()
            .map(|g| print_group_expanded(g, printer))
            .collect();
        let first_expanded_doc = doc::concat(first_expanded);
        let state_first_expanded = doc::concat(
            std::iter::once(first_expanded_doc)
                .chain(rest_docs)
                .collect(),
        );

        return doc::conditional_group(vec![on_line, state_last_expanded, state_first_expanded]);
    }

    // Check if any group except the last will break
    // This matches prettier's `printedGroups.slice(0, -1).some(willBreak)` check
    let any_non_last_breaks = groups[..groups.len() - 1].iter().any(|g| {
        let doc = print_group(g, printer);
        doc::will_break(&doc)
    });

    // Check if this chain ends with member access (not a call)
    // This enables the intermediate state: args expanded but chain inline
    // e.g., `items.filter(\n  (x) => ...,\n).length` instead of breaking the chain
    //
    // When the chain ends with a call (e.g., `items.map(...).join(...)`), we prefer
    // breaking the chain over expanding args - this is Prettier's behavior.
    let chain_ends_with_member = ends_with_member(rest_groups, first_groups);

    // Count calls in rest_groups (for chain_ends_with_member special case)
    let rest_call_count = rest_groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .filter(|n| n.is_call())
        .count();

    // For longer chains (>cutoff), force expanded if any non-last group breaks
    // EXCEPTION: When chain ends with member AND has exactly one call in rest,
    // we prefer keeping chain inline with args expanded (handled below)
    let force_expand_from_breaking =
        any_non_last_breaks && !(chain_ends_with_member && rest_call_count == 1);

    // Build expanded variant (needed for multiple code paths)
    let expanded = build_expanded_doc(groups, should_merge, printer);

    // If force_expand (3+ calls with arrow/function args) or first groups break
    // with 2+ trailing, force the expanded layout
    if force_expand || force_expand_from_breaking {
        return expanded;
    }

    // Print all groups inline (for oneLine variant)
    let on_line: Vec<Doc> = groups.iter().map(|g| print_group(g, printer)).collect();
    let on_line_doc = doc::concat(on_line);

    // Handle chains ending with member access (e.g., `.length`) with exactly one call in rest.
    // We already computed rest_call_count above.
    if chain_ends_with_member && rest_call_count == 1 {
        // Check if the call's single arg needs expansion. This includes:
        // 1. Direct object/array that will break (has source newlines)
        // 2. Arrow with type annotations AND breaking object/array body
        //
        // When will_break is true, conditional_group doesn't work correctly because
        // fits() fails immediately. We need to directly select args_expanded_doc.
        //
        // Note: Arrows WITHOUT type annotations hug the call and only their internal
        // object expands. But arrows WITH type annotations expand the call args.
        let rest_has_breaking_arg = rest_groups.iter().any(|g| {
            g.nodes.iter().any(|n| {
                let Some(call) = n.as_call_expression() else {
                    return false;
                };
                if call.arguments.len() != 1 {
                    return false;
                }

                match &call.arguments[0] {
                    // Direct object/array with breaking content
                    Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => {
                        let arg_doc = printer.print_expression(&call.arguments[0]);
                        doc::will_break(&arg_doc)
                    }
                    // Arrow with type annotations and breaking object/array body
                    Expression::ArrowFunctionExpression(arrow)
                        if arrow.return_type.is_some()
                            || arrow.type_parameters.is_some()
                            || arrow.params.iter().any(has_param_type_annotation) =>
                    {
                        if let ArrowFunctionBody::Expression(body) = &arrow.body
                            && matches!(
                                &**body,
                                Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
                            )
                        {
                            let body_doc = printer.print_expression(body);
                            doc::will_break(&body_doc)
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            })
        });

        // First groups stay flat
        let first_docs: Vec<Doc> = first_groups
            .iter()
            .map(|g| print_group(g, printer))
            .collect();
        // Rest groups have calls expanded (uses hardlines)
        let rest_expanded: Vec<Doc> = rest_groups
            .iter()
            .map(|g| print_group_expanded(g, printer))
            .collect();
        let args_expanded_doc = doc::concat(first_docs.into_iter().chain(rest_expanded).collect());

        // When the arg will break internally, directly use args_expanded_doc.
        // conditional_group doesn't work because will_break=true causes fits() to fail.
        if rest_has_breaking_arg {
            return args_expanded_doc;
        }

        // Try: 1. Everything inline, 2. Args expanded chain inline, 3. Chain expanded
        return doc::conditional_group(vec![on_line_doc, args_expanded_doc, expanded]);
    }

    // Check if the last call has a single object/array argument that will break.
    // When the last arg is an object/array that will break internally (has source newlines
    // or other breaking content), we want to try keeping the chain inline with the
    // object expanded before breaking the chain.
    // e.g., `aaaaaa.get(x).push({\n  prop,\n})` instead of breaking the chain.
    //
    // This intermediate state uses hardlines in the object (via print_group_expanded)
    // so fits() correctly measures just the first line: `aaaaaa.get(x).push({`
    //
    // IMPORTANT: Only use this when the object's normal doc will_break. For short objects
    // that fit inline, we should let the normal conditional_group logic decide between
    // on_line (everything flat) and expanded (chain broken).
    let last_group_will_break_object = rest_groups.last().is_some_and(|g| {
        g.nodes
            .iter()
            .rev()
            .find_map(ChainNode::as_call_expression)
            .is_some_and(|call| {
                if call.arguments.len() != 1 {
                    return false;
                }
                match &call.arguments[0] {
                    Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => {
                        // Check if the normal arg doc will break
                        // (has source newlines, line comments, or other breaking content)
                        let arg_doc = printer.print_expression(&call.arguments[0]);
                        doc::will_break(&arg_doc)
                    }
                    _ => false,
                }
            })
    });

    if last_group_will_break_object {
        // First groups stay flat
        let first_docs: Vec<Doc> = first_groups
            .iter()
            .map(|g| print_group(g, printer))
            .collect();
        // Rest groups: all but last stay flat, last is expanded
        let rest_len = rest_groups.len();
        let rest_docs: Vec<Doc> = rest_groups
            .iter()
            .enumerate()
            .map(|(i, g)| {
                if i == rest_len - 1 {
                    print_group_expanded(g, printer)
                } else {
                    print_group(g, printer)
                }
            })
            .collect();
        let args_expanded_doc = doc::concat(first_docs.into_iter().chain(rest_docs).collect());

        // Try: 1. Everything inline, 2. Args expanded chain inline, 3. Chain expanded
        return doc::conditional_group(vec![on_line_doc, args_expanded_doc, expanded]);
    }

    // Use conditionalGroup to let printer decide
    doc::conditional_group(vec![on_line_doc, expanded])
}

/// Check if a call node has complex (non-simple) arguments
///
/// Uses Prettier's `isSimpleCallArgument` logic (inverted) to determine
/// if a 3+ call chain should force break.
fn call_has_complex_args<'a>(node: &ChainNode<'a>) -> bool {
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
struct CallbackStatus {
    /// Whether the call has any callback argument (arrow/function)
    has_callback: bool,
    /// Whether any callback will break (multiline body)
    will_break: bool,
}

/// Analyze callback status for a call node in a single pass
fn call_callback_status<'a>(node: &ChainNode<'a>, line_breaks: &[u32]) -> CallbackStatus {
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
                        // Block body breaks only if it has statements (empty {} stays inline)
                        ArrowFunctionBody::BlockStatement(block) => !block.body.is_empty(),
                        // Expression body - check if it's multiline (O(log n))
                        ArrowFunctionBody::Expression(expr) => {
                            let span = expr.span();
                            printing::has_newline_between_fast(line_breaks, span.start, span.end)
                        }
                    };
                }
            }
            Expression::FunctionExpression(func) => {
                // Function expressions break only if body is non-empty
                has_callback = true;
                if !will_break {
                    will_break = !func.body.body.is_empty();
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
fn ends_with_member(rest_groups: &[ChainGroup], first_groups: &[ChainGroup]) -> bool {
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

/// Build a chain-break doc: first_doc followed by indented rest_docs on new lines
///
/// Result: `first_doc + indent(hardline + rest[0] + hardline + rest[1] + ...)`
fn build_chain_break_doc(first_doc: Doc, rest_docs: &[Doc]) -> Doc {
    let mut rest_parts = Vec::with_capacity(rest_docs.len() * 2);
    for rest_doc in rest_docs {
        rest_parts.push(doc::hardline());
        rest_parts.push(rest_doc.clone());
    }
    doc::concat(vec![first_doc, doc::indent(doc::concat(rest_parts))])
}

/// Check if chain starts with "(await ...)" pattern that needs chain-preferring breaks
///
/// Returns true if:
/// - First group is [Base(needs_parens=true)] (single node, no NonNull)
/// - The base expression is an await expression
///
/// This handles `(await fn(...)).member` patterns where we want to break at the
/// chain point (after `)`) rather than inside the await expression.
/// Check if chain starts with "(await ...)" pattern that needs chain-preferring breaks
///
/// Returns true if:
/// - First group's first node is Base(needs_parens=true) with an AwaitExpression
/// - First group does NOT start with [Base, NonNull] (that's handled by is_parenthesized_non_null_head)
///
/// The first group may contain additional nodes (consecutive members like `.prop`)
/// which are grouped together by Prettier's algorithm. These trailing members will
/// break together with the rest of the chain when needed.
///
/// Example: `(await fn({...})).prop?.method_long()` groups as:
/// - Group 0: [(Base await), (Member .prop)]
/// - Group 1: [(Member ?.method_long), (Call)]
///
/// When the chain exceeds print_width, we break at the group boundary (after `.prop`).
fn is_parenthesized_await_head(groups: &[ChainGroup]) -> bool {
    if groups.len() < 2 {
        return false;
    }
    let first = &groups[0];
    if first.nodes.is_empty() {
        return false;
    }

    // First node should be Base with needs_parens=true and an AwaitExpression
    let ChainNode::Base {
        expr,
        needs_parens: true,
    } = &first.nodes[0]
    else {
        return false;
    };

    // Check it's an await expression
    if !matches!(expr, Expression::AwaitExpression(_)) {
        return false;
    }

    // Reject if second node is NonNull - that's the (await)!.method() pattern
    // handled by is_parenthesized_non_null_head instead
    if first.nodes.len() >= 2 && first.nodes[1].is_non_null() {
        return false;
    }

    true
}

/// Build doc for "(await ...).method()" chains
///
/// Uses conditional_group to prefer breaking at the chain point rather than
/// inside the parenthesized await expression. This matches prettier's behavior:
/// - First tries to fit everything on one line
/// - If that doesn't fit, breaks at the chain point (after `)`), keeping await expression flat
/// - If the await expression itself exceeds print width, it expands internally
///
/// Example:
/// ```text
/// // Fits on one line (100 chars):
/// const a = (await fn({...o, a: bbb})).prop?.method();
///
/// // Doesn't fit (101 chars) - breaks at chain, await stays flat:
/// const b = (
///     await fn({...o, a: bbbb})
/// ).prop?.method();
///
/// // Inner await exceeds 100 - breaks at chain AND expands await:
/// const d = (
///     await fn({
///         ...o,
///         a: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
///     })
/// ).prop?.method();
/// ```
fn build_parenthesized_await_chain_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> Doc {
    // First group: (await ...)
    // print_group calls print_parenthesized_base which returns the indent-on-break structure
    let first_group_doc = print_group(&groups[0], printer);

    if groups.len() == 1 {
        return first_group_doc;
    }

    // Build first group with hardline-expanded parens for args_break state.
    let first_group_expanded = print_first_group_with_expanded_parens(&groups[0], printer);

    // Rest: .method().prop etc
    let rest_docs: Vec<Doc> = groups[1..]
        .iter()
        .map(|g| print_group(g, printer))
        .collect();

    // Check if first group will break (e.g., objects with group_break have should_break=true).
    // When will_break is true, conditional_group won't work correctly because fits() measures
    // flat content but the actual render will be expanded. Use direct fits() check instead.
    let first_will_break = doc::will_break(&first_group_doc);

    if first_will_break {
        // Count calls to determine threshold: multi-call breaks at >= print_width, single at >
        let call_count = groups[0]
            .nodes
            .iter()
            .skip(1)
            .chain(groups[1..].iter().flat_map(|g| g.nodes.iter()))
            .filter(|n| matches!(n, ChainNode::Call { .. }))
            .count();

        // Build chain tail doc: ")" + trailing nodes from first group + rest groups
        // This contains no should_break groups, so fits() measures it accurately.
        let first_group_trailing: Vec<Doc> = groups[0]
            .nodes
            .iter()
            .skip(1)
            .map(|n| print_node(n, printer))
            .collect();
        let chain_tail_doc = doc::concat(
            std::iter::once(doc::text(")"))
                .chain(first_group_trailing)
                .chain(rest_docs.iter().cloned())
                .collect(),
        );

        // Available width = print_width - base_indent - threshold_adjustment
        // base_indent: 2 tabs (function body + assignment) = 4 visual chars
        // threshold_adjustment: -2 for multi-call (>= behavior), -1 for single-call (> behavior)
        let base_indent = printer.get_tab_width() * 2;
        let print_width = printer.get_print_width();
        let threshold_adj = if call_count > 1 { 2 } else { 1 };
        let available = print_width.saturating_sub(base_indent + threshold_adj);

        // Choose between args_break (chain inline) and chain_break (chain on new lines)
        let args_break = doc::concat(
            std::iter::once(first_group_expanded.clone())
                .chain(rest_docs.clone())
                .collect(),
        );
        let chain_break = build_chain_break_doc(first_group_expanded, &rest_docs);

        return if printer.fits_chain_tail(&chain_tail_doc, available) {
            args_break
        } else {
            chain_break
        };
    }

    // First group doesn't break - use conditional_group with hardline-expanded args_break
    // so fits() can correctly measure the actual line widths.
    //
    // Build three states:
    // 1. on_line: everything flat (parens and chain inline)
    // 2. args_break: parens expanded with hardlines, chain inline
    // 3. chain_break: parens expanded, chain also breaks

    // State 1: Everything on one line
    let on_line = doc::concat(
        std::iter::once(first_group_doc)
            .chain(rest_docs.clone())
            .collect(),
    );

    // State 2: Parens expanded (hardlines), chain stays together
    let args_break = doc::concat(
        std::iter::once(first_group_expanded.clone())
            .chain(rest_docs.clone())
            .collect(),
    );

    // State 3: Chain breaks after first group
    let chain_break = build_chain_break_doc(first_group_expanded, &rest_docs);

    // Let conditional_group decide via fits()
    doc::conditional_group(vec![on_line, args_break, chain_break])
}

/// Print the first group with hardline-expanded parens for the base expression.
///
/// Used for `args_break` state so fits() can measure actual broken line widths.
fn print_first_group_with_expanded_parens<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> Doc {
    let mut docs = Vec::with_capacity(group.nodes.len());
    for (i, node) in group.nodes.iter().enumerate() {
        if i == 0 {
            // First node is the parenthesized base - use expanded version
            if let ChainNode::Base {
                expr,
                needs_parens: true,
            } = node
            {
                docs.push(printer.print_parenthesized_base_expanded(expr));
            } else {
                docs.push(print_node(node, printer));
            }
        } else {
            docs.push(print_node(node, printer));
        }
    }
    doc::concat(docs)
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
    // Need at least 2 nodes: Base and NonNull
    // Additional nodes (like consecutive members) are allowed
    if first.nodes.len() < 2 {
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
/// For await/yield expressions with non-null, Prettier prefers breaking inside
/// the call args rather than at the parens. So we use simple parens (not
/// indent-on-break) for these cases.
///
/// For other expressions (ternary, type assertions), use conditional_group to
/// prefer breaking at the chain point rather than inside the parenthesized expression.
fn build_parenthesized_non_null_chain_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> Doc {
    // Check if base is await/yield - these use simple parens and no chain breaking
    let is_await_yield = matches!(
        groups[0].nodes.first(),
        Some(ChainNode::Base { expr, .. })
        if matches!(expr, Expression::AwaitExpression(_) | Expression::YieldExpression(_))
    );

    // First group: (complex)! [possibly followed by consecutive members]
    let first_group_doc = if is_await_yield {
        // For await/yield, use simple parens so inner call args can expand
        let ChainNode::Base { expr, .. } = &groups[0].nodes[0] else {
            unreachable!()
        };
        let inner_doc = printer.print_expression(expr);
        let parens_doc = doc::parens(inner_doc);

        // Build the full first group: (expr)! + any trailing nodes (members, etc.)
        let mut parts = vec![parens_doc];
        for node in groups[0].nodes.iter().skip(1) {
            parts.push(print_node(node, printer));
        }
        doc::concat(parts)
    } else {
        print_group(&groups[0], printer)
    };

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
    // just use group(oneLine) and let the inner expression break naturally.
    // For expressions that don't break internally, use conditionalGroup to
    // prefer chain breaks over inner group breaks.
    if doc::will_break(&first_group_doc) {
        return doc::group(on_line);
    }

    // expanded: chain breaks after !, keeping inner expression flat
    let expanded = build_chain_break_doc(first_group_doc, &rest_docs);

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
    // NOTE: We intentionally do NOT add break_parent for line comments here.
    // The break_parent approach causes issues with line_suffix flushing order -
    // the suffix gets flushed at the wrong line break. Instead, line comments
    // in member-only chains are handled via line_suffix, and the assignment
    // layout naturally handles them (suffix appears at end of line).

    // Flatten all nodes into individual segments
    let all_nodes: Vec<&ChainNode<'a>> = groups.iter().flat_map(|g| g.nodes.iter()).collect();

    if all_nodes.is_empty() {
        return doc::empty();
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
        doc::empty()
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
    // Note: Block comments are handled by print_node for member nodes
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

        // print_node handles block comments for member nodes
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

    // Build fill with segments - this packs greedily at the current position.
    // Note: The fill now uses rest_commands for width calculation, so we don't
    // need to set a trailing_reserve here. The fill will see the actual trailing
    // content in the document tree.
    let fill_doc = doc::fill(fill_parts);

    // Use conditional_group with on_line (flat) and expanded (fill-based) variants.
    // The fill packs greedily, respecting print width including trailing content.
    let expanded = doc::concat(vec![first_doc, doc::indent(fill_doc)]);

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
        return doc::empty();
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
    let line_breaks = printer.get_line_breaks();
    let has_comments = printer.has_comments_between(object_end, property_start);

    if !has_comments && has_blank_line_between_fast(line_breaks, object_end, property_start) {
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

    /// Add a group without a preceding line break, but with trailing comments
    /// Used for trailing member accesses that should stay on same line as `})`
    fn add_group_no_break(&mut self, group: &ChainGroup<'a>) {
        self.add_trailing_comments_only(group);
        self.add_group_doc(group);
    }

    /// Add only trailing comments (no line break, no leading comments)
    /// Used when the next element should stay on the same line as the previous.
    ///
    /// Emits:
    /// 1. Trailing block comments (same line as previous element)
    /// 2. Trailing line comments (same line, via line_suffix)
    ///
    /// Skips leading comments and line breaks since we want the member to stay
    /// on the same line. Leading comments that appear on their own line before
    /// a trailing member are a complex case - Prettier moves them elsewhere
    /// (e.g., after `=`), which requires structural transformation beyond what
    /// this function handles.
    fn add_trailing_comments_only(&mut self, group: &ChainGroup<'a>) {
        if let Some((object_end, property_start)) = group.first_member_range() {
            let classified = self.printer.classify_comments(object_end, property_start);

            // Trailing block comments (same line as previous element)
            // e.g., `.map(x => x) /* comment */.length`
            self.parts.push(
                self.printer
                    .build_trailing_block_doc(&classified.trailing_block),
            );

            // Trailing line comments (same line as previous element)
            // e.g., `.map(x => x) // comment` - goes to end of line via line_suffix
            self.parts.push(
                self.printer
                    .build_trailing_line_doc(&classified.trailing_line),
            );

            // Note: Leading comments (on their own line before the member) are
            // intentionally not emitted here. They would need a line break, but
            // we're explicitly avoiding breaks to keep the member on the same line.
        }
    }

    /// Add trailing comments, line break, and leading comments before a group
    ///
    /// Emits comments in this order:
    /// 1. Trailing block comments (same line as previous element) - before line break
    /// 2. Trailing line comments (same line) - via line_suffix
    /// 3. Line break
    /// 4. Leading block comments (on their own line)
    /// 5. Leading line comments (on their own line)
    ///
    /// Uses single-pass comment classification (O(log n + k)) instead of 4 separate
    /// filter calls (O(4 log n + 4k)).
    fn add_comments_and_break(&mut self, group: &ChainGroup<'a>) {
        if let Some((object_end, property_start)) = group.first_member_range() {
            // Classify all comments in one pass (single binary search)
            let classified = self.printer.classify_comments(object_end, property_start);

            // Trailing block comments (same line as previous element)
            // Use Leading spacing (space before comment): `method() /* c */`
            self.parts.push(
                self.printer
                    .build_trailing_block_doc(&classified.trailing_block),
            );

            // Trailing line comments (same line as previous element)
            self.parts.push(
                self.printer
                    .build_trailing_line_doc(&classified.trailing_line),
            );

            // Line break with blank line preservation
            self.parts.push(build_chain_line_break(
                self.printer,
                object_end,
                property_start,
                self.use_hardline,
            ));

            // Leading block comments (on their own line)
            self.parts.push(
                self.printer
                    .build_leading_block_doc(&classified.leading_block),
            );

            // Leading line comments (on their own line)
            self.parts.push(
                self.printer
                    .build_leading_line_doc(&classified.leading_line),
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
    ///
    /// Skips block comments for the first member since `add_comments_and_break`
    /// already handles them (emitting before the line break).
    fn add_group_doc(&mut self, group: &ChainGroup<'a>) {
        self.parts.push(if self.use_expanded {
            print_group_expanded_skip_first_comments(group, self.printer)
        } else {
            print_group_skip_first_comments(group, self.printer)
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
    // Check if last group is a simple member (no calls) - it should stay on same line as `})`
    // e.g., `.filter().map({...})).length` - `.length` stays on same line as `})`
    let last_is_simple_member = rest_groups.last().is_some_and(|g| {
        g.nodes.len() == 1 && g.nodes.iter().all(|n| n.is_member() && !n.is_call())
    });

    // Check if last group has comments that force a line break.
    // Line comments (`// ...`) consume the rest of the line, so we can't emit them
    // and then print more code on the same line. Leading comments also need their
    // own line. Only trailing block comments can stay inline.
    let last_has_break_forcing_comments = last_is_simple_member
        && rest_groups.last().is_some_and(|g| {
            if let Some((object_end, property_start)) = g.first_member_range() {
                let classified = printer.classify_comments(object_end, property_start);
                // Any line comments or leading comments force a break
                !classified.trailing_line.is_empty()
                    || !classified.leading_block.is_empty()
                    || !classified.leading_line.is_empty()
            } else {
                false
            }
        });

    let mut builder =
        ChainPartsBuilder::new(printer, use_hardline, use_expanded, rest_groups.len());
    for (i, group) in rest_groups.iter().enumerate() {
        // Don't add hardline before last group if it's a simple member WITHOUT
        // comments that force a break
        let is_last = i == rest_groups.len() - 1;
        if is_last && last_is_simple_member && use_hardline && !last_has_break_forcing_comments {
            builder.add_group_no_break(group);
        } else {
            builder.add_group(group);
        }
    }
    builder.build()
}

/// Print a member access (shared logic for Member and PrivateMember)
///
/// Emits comments before the member access:
/// - Block comments inline (e.g., `a /* comment */.b`)
/// - Line comments via line_suffix (moved to end of line, matching Prettier)
///
/// The `skip_comments` flag is used by the expanded path where `add_comments_and_break`
/// already handles comments for the first member of rest groups.
fn print_member_access<P: ChainPrinter>(
    printer: &P,
    property: DefaultSymbol,
    optional: bool,
    object_end: u32,
    property_start: u32,
    is_private: bool,
    skip_comments: bool,
) -> Doc {
    // Build member doc without format! allocation - use doc::symbol for deferred resolution
    let prop_id = property.to_u32();
    let member_doc = match (optional, is_private) {
        (false, false) => doc::concat(vec![doc::text("."), doc::symbol(prop_id)]),
        (true, false) => doc::concat(vec![doc::text("?."), doc::symbol(prop_id)]),
        (false, true) => doc::concat(vec![doc::text(".#"), doc::symbol(prop_id)]),
        (true, true) => doc::concat(vec![doc::text("?.#"), doc::symbol(prop_id)]),
    };

    if skip_comments {
        return member_doc;
    }

    // Classify all comments in the range between object and property
    let classified = printer.classify_comments(object_end, property_start);

    // Trailing block comments: same line as previous element (e.g., `method() /* c */.prop`)
    let trailing_block = printer.build_trailing_block_doc(&classified.trailing_block);

    // Line comments (both trailing and leading) get moved to end of line via line_suffix.
    // This matches Prettier's behavior of hoisting mid-chain line comments.
    // NOTE: We use build_line_comments_no_boundary here (not build_trailing_line_doc) because
    // we don't want to flush the line_suffix immediately - it should stay deferred until
    // the actual end of line.
    let trailing_line = printer.build_line_comments_no_boundary(&classified.trailing_line);
    let leading_line = printer.build_line_comments_no_boundary(&classified.leading_line);

    // Leading block comments on their own line - emit inline (rare case)
    let leading_block = printer.build_trailing_block_doc(&classified.leading_block);

    doc::concat(vec![
        trailing_block,
        trailing_line,
        leading_block,
        leading_line,
        member_doc,
    ])
}

/// Print a single chain node
fn print_node<'a, P: ChainPrinter>(node: &ChainNode<'a>, printer: &P) -> Doc {
    print_node_impl(node, printer, false)
}

/// Print a single chain node, optionally skipping comments for first member
///
/// Used by `add_group_doc` in expanded path where `add_comments_and_break`
/// already handles comments for the first member.
fn print_node_impl<'a, P: ChainPrinter>(
    node: &ChainNode<'a>,
    printer: &P,
    skip_comments: bool,
) -> Doc {
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
            skip_comments,
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
            skip_comments,
        ),

        ChainNode::ComputedMember {
            expr,
            optional,
            object_end,
            bracket_end,
        } => {
            let inner = printer.print_expression(expr);
            let prop_span = printer.get_property_span(expr);

            // Line comments between object and `[` get moved to end of line via line_suffix.
            // Block comments are handled by build_block_comments_doc below (kept inline).
            let pre_bracket = printer.classify_comments(*object_end, prop_span.start);
            let pre_trailing_line =
                printer.build_line_comments_no_boundary(&pre_bracket.trailing_line);
            let pre_leading_line =
                printer.build_line_comments_no_boundary(&pre_bracket.leading_line);

            // Block comments: obj[/* c */ key] and obj[key /* c */]
            let leading_comments_doc = printer.build_block_comments_doc(
                *object_end,
                prop_span.start,
                super::CommentSpacing::Trailing,
            );
            let trailing_comments_doc = printer.build_block_comments_doc(
                prop_span.end,
                *bracket_end,
                super::CommentSpacing::Leading,
            );

            let inner_with_comments =
                doc::concat(vec![leading_comments_doc, inner, trailing_comments_doc]);
            let bracket_doc = if *optional {
                doc::concat(vec![doc::text("?.["), inner_with_comments, doc::text("]")])
            } else {
                doc::brackets(inner_with_comments)
            };

            // Emit line comments via line_suffix (moved to end of line)
            doc::concat(vec![pre_trailing_line, pre_leading_line, bracket_doc])
        }

        ChainNode::NonNull => doc::text("!"),
    }
}

/// Print a single chain node with forced call expansion
fn print_node_expanded<'a, P: ChainPrinter>(node: &ChainNode<'a>, printer: &P) -> Doc {
    print_node_expanded_impl(node, printer, false)
}

/// Print a single chain node with forced call expansion, optionally skipping comments
fn print_node_expanded_impl<'a, P: ChainPrinter>(
    node: &ChainNode<'a>,
    printer: &P,
    skip_comments: bool,
) -> Doc {
    match node {
        ChainNode::Call { expr, optional } => {
            if let Expression::CallExpression(call) = expr {
                printer.print_call_args_expanded(call, *optional)
            } else {
                doc::text("()")
            }
        }
        // All other nodes print the same way
        _ => print_node_impl(node, printer, skip_comments),
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

/// Print a chain group, skipping block comments for the first member node
///
/// Used by `add_group_doc` in expanded path where `add_comments_and_break`
/// already handles comments for the first member (emitting before the line break).
fn print_group_skip_first_comments<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> Doc {
    print_group_skip_first_comments_impl(group, printer, false)
}

/// Print a chain group with forced call expansion, skipping block comments for the first member
fn print_group_expanded_skip_first_comments<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> Doc {
    print_group_skip_first_comments_impl(group, printer, true)
}

/// Implementation for printing a chain group, optionally with forced call expansion
fn print_group_skip_first_comments_impl<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
    force_expand: bool,
) -> Doc {
    let docs: Vec<Doc> = group
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            // Skip comments only for the first member node
            let skip_comments = i == 0 && n.is_member();
            if force_expand {
                print_node_expanded_impl(n, printer, skip_comments)
            } else {
                print_node_impl(n, printer, skip_comments)
            }
        })
        .collect();
    doc::concat(docs)
}

/// Trait for printing chain elements (abstraction over Printer)
pub trait ChainPrinter: SymbolLookup {
    /// Print an expression as a Doc
    fn print_expression(&self, expr: &Expression) -> Doc;

    /// Print a parenthesized base expression with indent-on-break behavior
    fn print_parenthesized_base(&self, expr: &Expression) -> Doc;

    /// Print a parenthesized base expression with forced expansion (hardlines)
    /// Used for `args_break` state in conditional_group so fits() can measure correctly
    fn print_parenthesized_base_expanded(&self, expr: &Expression) -> Doc;

    /// Print call arguments: () or (arg1, arg2)
    fn print_call_args(&self, call: &internal::CallExpression, optional: bool) -> Doc;

    /// Print call arguments with forced expansion (hardlines)
    /// Used for the "args broken, chain inline" state in conditionalGroup
    fn print_call_args_expanded(&self, call: &internal::CallExpression, optional: bool) -> Doc;

    /// Build a doc for inline block comments between two positions
    /// Returns a doc with block comments using the specified spacing
    /// Note: Only block comments are included (line comments are filtered out)
    fn build_block_comments_doc(&self, start: u32, end: u32, spacing: super::CommentSpacing)
    -> Doc;

    /// Get the span for a given expression
    fn get_property_span(&self, expr: &Expression) -> Span;

    /// Check if the chain is the direct child of an ExpressionStatement
    ///
    /// Used to determine if short identifier names should be merged with their
    /// first call (e.g., `a.fn().b()` → merge `a` with `.fn()` only in statements).
    fn is_expression_statement(&self) -> bool;

    /// Get the precomputed line breaks table for O(log n) line boundary lookups
    fn get_line_breaks(&self) -> &[u32];

    /// Check if there are any comments between two positions
    fn has_comments_between(&self, start: u32, end: u32) -> bool;

    /// Classify all comments in a range by position and type in a single pass.
    ///
    /// Returns comments organized into 4 buckets (trailing_block, trailing_line,
    /// leading_block, leading_line) using a single binary search instead of 4
    /// separate filter calls.
    fn classify_comments(&self, start: u32, end: u32) -> ClassifiedComments<'_>;

    /// Build doc for trailing block comments from a pre-classified slice.
    /// Emits space before each comment: `method() /* c */`
    fn build_trailing_block_doc(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Build doc for trailing line comments from a pre-classified slice.
    /// Uses line_suffix to keep comments with preceding element.
    fn build_trailing_line_doc(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Build doc for leading block comments from a pre-classified slice.
    /// Comments are on their own lines, no surrounding spaces.
    fn build_leading_block_doc(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Build doc for leading line comments from a pre-classified slice.
    /// Emits hardline after each comment.
    fn build_leading_line_doc(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Build line_suffix docs for line comments WITHOUT a trailing boundary.
    ///
    /// Used for inline chain formatting where we want comments to defer to end
    /// of line without being flushed immediately. Unlike `build_trailing_line_doc`,
    /// this doesn't add a `line_suffix_boundary()` at the end.
    fn build_line_comments_no_boundary(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Get the tab width from config
    fn get_tab_width(&self) -> usize;

    /// Get the print width from config
    fn get_print_width(&self) -> usize;

    /// Check if chain expansion should be forced
    ///
    /// Used when inside template expressions with original breaks, where the
    /// expression is too long for the remaining print width.
    fn should_force_expand(&self) -> bool;

    /// Check if a doc fits in the available width (for chain break decisions)
    ///
    /// Used when deciding between args_break and chain_break states.
    /// Performs accurate width measurement via fits() with symbol resolution.
    fn fits_chain_tail(&self, doc: &Doc, available: usize) -> bool;
}
