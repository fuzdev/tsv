// Chain data structures for TypeScript member chain formatting
//
// This module defines the core data types used throughout chain formatting:
// - ChainNode: Individual elements in a linearized chain
// - ChainNodeVec: Stack-friendly buffer for a linearized chain
// - ChainGroup: Groups of nodes that stay together on the same line

use crate::ast::internal::{self, LiteralValue};
use smallvec::SmallVec;
use string_interner::DefaultSymbol;

/// Buffer for a linearized chain — chains are measured-short, so small chains
/// (the common case) stay on the stack. `ChainNode` is `Copy` and ~24 bytes.
pub type ChainNodeVec<'a> = SmallVec<[ChainNode<'a>; 8]>;

/// A node in a linearized chain
///
/// Each variant contains exactly the data it needs - no optional fields.
/// This makes invalid states unrepresentable.
#[derive(Debug, Clone, Copy)]
pub enum ChainNode<'a> {
    /// Base expression: identifier, literal, complex expr in parens
    Base {
        expr: &'a internal::Expression,
        needs_parens: bool,
    },
    /// Call expression: ()
    Call {
        call: &'a internal::CallExpression,
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
        expr: &'a internal::Expression,
        optional: bool,
        object_end: u32,
        bracket_end: u32,
    },
    /// Non-null assertion: !
    NonNull,
}

impl<'a> ChainNode<'a> {
    /// Create a new base node
    pub fn base(expr: &'a internal::Expression, needs_parens: bool) -> Self {
        Self::Base { expr, needs_parens }
    }

    /// Create a new call node
    pub fn call(call: &'a internal::CallExpression) -> Self {
        Self::Call {
            call,
            optional: false,
        }
    }

    /// Create a new call node with optional chaining
    pub fn call_optional(call: &'a internal::CallExpression) -> Self {
        Self::Call {
            call,
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
        expr: &'a internal::Expression,
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
            && let internal::Expression::Literal(lit) = expr
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
    pub fn as_call_expression(&self) -> Option<&'a internal::CallExpression> {
        if let Self::Call { call, .. } = self {
            Some(call)
        } else {
            None
        }
    }
}

/// A group of chain nodes that stay on the same line
///
/// Groups are measured-short, so the nodes buffer holds up to 4 entries inline
/// without heap allocation.
#[derive(Debug, Clone)]
pub struct ChainGroup<'a> {
    pub nodes: SmallVec<[ChainNode<'a>; 4]>,
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
