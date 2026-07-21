// Chain data structures for TypeScript member chain formatting
//
// This module defines the core data types used throughout chain formatting:
// - ChainNode: Individual elements in a linearized chain
// - ChainNodeVec: Stack-friendly buffer for a linearized chain
// - ChainGroup: Groups of nodes that stay together on the same line

use crate::ast::internal::{self, IdentName, LiteralValue};
use smallvec::SmallVec;

/// Buffer for a linearized chain — chains are measured-short, so small chains
/// (the common case) stay on the stack. `ChainNode` is `Copy` and ~24 bytes.
pub type ChainNodeVec<'a> = SmallVec<[ChainNode<'a>; 8]>;

/// Stack-friendly buffer for the grouped chain — `group_chain_nodes` builds this
/// once per chain. `ChainGroup` is ~112 bytes (it embeds an inline `ChainGroupNodesVec`),
/// so the inline capacity stays small at `4`: most chains are 1–2 groups, but a
/// 3–4-group chain (`a.b().c()` and friends) is common in real code — a two-call
/// chain is already 3 groups — so `4` keeps the common shapes on the stack while
/// the genuinely long chains, which break anyway, spill to the heap.
pub type ChainGroupVec<'a> = SmallVec<[ChainGroup<'a>; 4]>;

/// Stack-friendly buffer for one group's own nodes (the [`ChainGroup::nodes`]
/// field) — groups are measured-short, so up to `4` entries stay inline.
pub type ChainGroupNodesVec<'a> = SmallVec<[ChainNode<'a>; 4]>;

/// Stack-friendly buffer of chain-node references — for the member-only and
/// base-call flatten passes that collect `&ChainNode` before printing. `8` covers
/// the common short chain inline; longer chains spill. `'n` is the (short) borrow
/// of the `ChainGroup` slice; `'a` is the AST lifetime the nodes point into.
pub type ChainNodeRefVec<'n, 'a> = SmallVec<[&'n ChainNode<'a>; 8]>;

/// A node in a linearized chain
///
/// Each variant contains exactly the data it needs - no optional fields.
/// This makes invalid states unrepresentable.
#[derive(Debug, Clone, Copy)]
pub enum ChainNode<'a> {
    /// Base expression: identifier, literal, complex expr in parens
    ///
    /// `paren_comment_end`, when `Some`, marks the end of the region (just past the
    /// stripped grouping `)` / following `!`) in which a trailing comment from the
    /// parens should be emitted *inside* them, before the `)`. Used for a
    /// parenthesized operand of a non-null assertion (`(x + y /* c */)!.foo`) so the
    /// comment is preserved where the author wrote it rather than dropped.
    Base {
        expr: &'a internal::Expression<'a>,
        needs_parens: bool,
        paren_comment_end: Option<u32>,
    },
    /// Call expression: ()
    Call {
        call: &'a internal::CallExpression<'a>,
        optional: bool,
    },
    /// Member access: .prop
    /// `object_end` is where the object expression ends
    /// `property_start` is where the property identifier starts (for comment
    /// detection; also the name's span start for span-identity resolution)
    Member {
        property: IdentName,
        optional: bool,
        object_end: u32,
        property_start: u32,
    },
    /// Private member access: .#prop
    /// `property_start` is the `#` (comment detection); `name_start` is the
    /// name token after it (span-identity resolution).
    PrivateMember {
        property: IdentName,
        optional: bool,
        object_end: u32,
        property_start: u32,
        name_start: u32,
    },
    /// Computed member access: [expr]
    /// `bracket_end` is the position just before the closing `]` (for trailing comment detection)
    ComputedMember {
        expr: &'a internal::Expression<'a>,
        optional: bool,
        object_end: u32,
        bracket_end: u32,
    },
    /// Non-null assertion: !
    NonNull,
}

/// Whether a computed index is a numeric literal — Prettier's `isNumericLiteral` carve-out.
///
/// The same predicate drives both halves of prettier's computed-lookup handling, which is
/// why it lives here rather than in either consumer: `printMemberLookup` (member.js) keeps a
/// numeric lookup's brackets FLAT while every other index gets a breakable group
/// (`computed_lookup_doc`), and `printMemberChain` (member-chain.js) lets a numeric lookup
/// ride along in the current group where a non-numeric one opens a new one
/// (`is_numeric_accessor`, used by `group_chain_nodes`).
pub fn is_numeric_index(expr: &internal::Expression<'_>) -> bool {
    matches!(expr, internal::Expression::Literal(lit) if matches!(lit.value, LiteralValue::Number(_)))
}

impl<'a> ChainNode<'a> {
    /// Create a new base node
    pub fn base(expr: &'a internal::Expression<'a>, needs_parens: bool) -> Self {
        Self::Base {
            expr,
            needs_parens,
            paren_comment_end: None,
        }
    }

    /// Create a parenthesized base node that preserves a trailing comment from the
    /// stripped grouping parens, emitted inside them before `)`. `paren_comment_end`
    /// bounds the region to scan for that comment (e.g. the non-null assertion's
    /// span end).
    pub fn base_with_paren_comment(
        expr: &'a internal::Expression<'a>,
        paren_comment_end: u32,
    ) -> Self {
        Self::Base {
            expr,
            needs_parens: true,
            paren_comment_end: Some(paren_comment_end),
        }
    }

    /// Create a new call node
    pub fn call(call: &'a internal::CallExpression<'a>) -> Self {
        Self::Call {
            call,
            optional: false,
        }
    }

    /// Create a new call node with optional chaining
    pub fn call_optional(call: &'a internal::CallExpression<'a>) -> Self {
        Self::Call {
            call,
            optional: true,
        }
    }

    /// Create a new member node
    pub fn member(
        property: IdentName,
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
        property: IdentName,
        optional: bool,
        object_end: u32,
        property_start: u32,
        name_start: u32,
    ) -> Self {
        Self::PrivateMember {
            property,
            optional,
            object_end,
            property_start,
            name_start,
        }
    }

    /// Create a new computed member node
    pub fn computed_member(
        expr: &'a internal::Expression<'a>,
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
        matches!(self, Self::ComputedMember { expr, .. } if is_numeric_index(expr))
    }

    /// Check if this is a computed member access
    pub const fn is_computed(&self) -> bool {
        matches!(self, Self::ComputedMember { .. })
    }

    /// Get the property name channel (+ its span start) for Member nodes
    pub const fn property(&self) -> Option<(IdentName, u32)> {
        match self {
            Self::Member {
                property,
                property_start,
                ..
            } => Some((*property, *property_start)),
            _ => None,
        }
    }

    /// Get the CallExpression if this is a Call node
    pub fn as_call_expression(&self) -> Option<&'a internal::CallExpression<'a>> {
        if let Self::Call { call, .. } = self {
            Some(call)
        } else {
            None
        }
    }
}

/// A group of chain nodes that stay on the same line
///
/// Groups are measured-short, so the nodes buffer keeps the common shapes
/// inline (see [`ChainGroupNodesVec`]).
#[derive(Debug, Clone)]
pub struct ChainGroup<'a> {
    pub nodes: ChainGroupNodesVec<'a>,
}

impl<'a> ChainGroup<'a> {
    pub fn new() -> Self {
        Self {
            nodes: SmallVec::new(),
        }
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
