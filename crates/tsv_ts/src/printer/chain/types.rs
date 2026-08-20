// Chain data structures for TypeScript member chain formatting
//
// This module defines the core data types used throughout chain formatting:
// - ChainNode: Individual elements in a linearized chain
// - ChainNodeVec: Stack-friendly buffer for a linearized chain
// - ChainGroup: Groups of nodes that stay together on the same line

use crate::ast::internal::{self, IdentName, LiteralValue};
use smallvec::SmallVec;
use tsv_lang::Span;

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

/// The facts about one call LINK that only the chain can supply — the link's own printer
/// has no parent pointer and cannot see either of them.
///
/// `own_call_layout` is the one prettier states as a routing decision: `printCallExpression`
/// answers the call-level layout rules (the sole-multiline-template hug, the test-call flat
/// form) at its TOP, above the `printMemberChain` redirect, so a call that reaches that
/// function keeps them — but a call the redirect **swallowed** is printed by
/// `printMemberChain`'s own `rec`, which goes straight to `printCallArguments` and never asks
/// them. tsv linearizes more finely than `rec` does, folding calls into one chain that
/// prettier prints as separate `printCallExpression` invocations, so the two answers have to
/// be reconstructed rather than read off the chain's existence. See `mark_own_call_layout`.
///
/// The link's `?.` is deliberately NOT here: it is `call.optional`, readable from the node's
/// own `call`, and a second copy is a flag with two producers that can only ever disagree.
#[derive(Debug, Clone, Copy)]
pub struct ChainCall {
    /// This call keeps `printCallExpression`'s own layout rules — it is not a link the
    /// member-chain redirect swallowed. Set by `mark_own_call_layout` after linearization;
    /// `new` seeds it `true`, the value for a chain nothing entered.
    pub own_call_layout: bool,
}

impl ChainCall {
    /// A link's facts before the chain-wide pass decides `own_call_layout`.
    pub fn new() -> Self {
        Self {
            own_call_layout: true,
        }
    }
}

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
        /// Where the pair this base prints OPENS, when the base owns its own
        /// leading gap — the `(`→base run is this node's to emit rather than the
        /// enclosing chain's (`prepend_removed_paren_comments`, which would hoist it
        /// out in front of a pair that survives). `Some` only where the pair is
        /// REQUIRED *and* prettier keeps the run inside it: a sealed optional chain
        /// (`( // c⏎a?.b)!.ccc`) and a function/arrow IIFE callee
        /// (`( // c⏎() => {})().p`). Every other required pair here — a cast, a
        /// sequence, a ternary, an instantiation — hoists the run in both formatters
        /// and stays `None`.
        paren_leading_start: Option<u32>,
        paren_comment_end: Option<u32>,
        /// A non-null `!` applies to this base before the chain continues, so the base
        /// expression's immediate parent is that `!` and not the member access after it.
        /// Kept separate from `paren_comment_end` — which happens to be `Some` in the
        /// same shape — because the two answer different questions: that one bounds a
        /// COMMENT scan, this one decides a LAYOUT (prettier's `breakClosingParen` fires
        /// on a member parent, so a `!` in between suppresses it).
        followed_by_non_null: bool,
    },
    /// Call expression: ()
    Call {
        call: &'a internal::CallExpression<'a>,
        /// The facts about this call that only the CHAIN knows — see [`ChainCall`].
        facts: ChainCall,
    },
    /// Member access: .prop
    /// `object_end` is where the object expression ends
    /// `property_start` is where the property identifier starts (for comment
    /// detection; also the name's span start for span-identity resolution)
    Member {
        property: IdentName<'a>,
        optional: bool,
        object_end: u32,
        property_start: u32,
        /// The object subtree this node's widened gap must NOT claim — see
        /// [`ChainNode::paren_gap_skip`].
        paren_gap_skip: Option<Span>,
    },
    /// Private member access: .#prop
    /// `property_start` is the `#` (comment detection); `name_start` is the
    /// name token after it (span-identity resolution).
    PrivateMember {
        property: IdentName<'a>,
        optional: bool,
        object_end: u32,
        property_start: u32,
        name_start: u32,
        /// See [`ChainNode::paren_gap_skip`].
        paren_gap_skip: Option<Span>,
    },
    /// Computed member access: `[expr]`
    /// `bracket_end` is the position just before the closing `]` (for trailing comment detection)
    ComputedMember {
        expr: &'a internal::Expression<'a>,
        optional: bool,
        object_end: u32,
        bracket_end: u32,
        /// See [`ChainNode::paren_gap_skip`].
        paren_gap_skip: Option<Span>,
    },
    /// Non-null assertion: `!`
    ///
    /// `gap` names which rule the operand→`!` region takes — see [`NonNullGap`].
    NonNull { gap: NonNullGap },
}

/// Which rule the operand→`!` region of a [`ChainNode::NonNull`] takes, keyed on
/// what sits to its left.
///
/// The two arms are a partition of one region, so exactly one emitter prints its
/// comments — deliberately not a "someone else claimed this" flag.
#[derive(Debug, Clone, Copy)]
pub enum NonNullGap {
    /// The region belongs to the `!`, which prints its comments just before
    /// itself: `aaa /* c */!.bbb`. `operand_end` is where the operand expression
    /// ends; `bang_end` is just past the `!`.
    ///
    /// The emitter is block-only, and that bound is sound because a `//` can never
    /// reach this arm: written bare, the `!` binds under `[no LineTerminator
    /// here]`, so only a single-line block comment can sit in the region — and the
    /// one authoring that puts a `//` there, a grouping shell (`(aaa // c⏎)!.bbb`),
    /// is caught by the linearizer's line-comment gate, which RETAINS the shell as
    /// a parenthesized base (this node then takes [`Self::InsideOperandParens`])
    /// rather than flattening the operand into a region with nothing left to
    /// parenthesize.
    Bang { operand_end: u32, bang_end: u32 },
    /// A parenthesized operand keeps the region *inside* its own parens, where the
    /// author wrote it (`(x + y /* c */)!.foo`), so the `!` prints nothing. Set by
    /// the linearizer's `paren_comment_end` arm on the preceding [`ChainNode::Base`].
    InsideOperandParens,
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
            paren_leading_start: None,
            paren_comment_end: None,
            followed_by_non_null: false,
        }
    }

    /// A base whose REQUIRED pair keeps its own leading run inside it — the sealed
    /// optional chain reached without a `!` (`( // c⏎a?.b).ddd`,
    /// `( // c⏎a?.b)()`). `paren_leading_start` is the `(`, i.e. the enclosing
    /// member/call node's own start.
    pub fn sealed_base(expr: &'a internal::Expression<'a>, paren_leading_start: u32) -> Self {
        Self::Base {
            expr,
            needs_parens: true,
            paren_leading_start: Some(paren_leading_start),
            paren_comment_end: None,
            followed_by_non_null: false,
        }
    }

    /// Create a parenthesized base node whose `!` follows as the next node —
    /// the linearizer's non-null arm builds both its authorings this way: a sealed
    /// optional chain (`(a?.b)!.c`, parens required) and a shell retained for a
    /// `//` in the operand→`!` gap (`(aaa // c⏎)!.bbb`). `paren_comment_end`
    /// bounds the region to scan for a trailing comment from the parens, emitted
    /// inside them before `)` — the scan may find nothing (a comment-free sealed
    /// chain), in which case the printer renders bare parens.
    pub fn paren_base_before_non_null(
        expr: &'a internal::Expression<'a>,
        paren_leading_start: u32,
        paren_comment_end: u32,
    ) -> Self {
        Self::Base {
            expr,
            needs_parens: true,
            paren_leading_start: Some(paren_leading_start),
            paren_comment_end: Some(paren_comment_end),
            followed_by_non_null: true,
        }
    }

    /// Create a new call node
    pub fn call(call: &'a internal::CallExpression<'a>) -> Self {
        Self::Call {
            call,
            facts: ChainCall::new(),
        }
    }

    /// Create a new member node
    pub fn member(
        property: IdentName<'a>,
        optional: bool,
        object_end: u32,
        property_start: u32,
    ) -> Self {
        Self::Member {
            property,
            optional,
            object_end,
            property_start,
            paren_gap_skip: None,
        }
    }

    /// Create a new private member node: .#prop
    pub fn private_member(
        property: IdentName<'a>,
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
            paren_gap_skip: None,
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
            paren_gap_skip: None,
        }
    }

    /// Create a new non-null node whose `!` owns the operand→`!` region
    ///
    /// The bounds are derived here rather than at the call sites, so the two
    /// linearization entry points cannot hand the node different ones.
    pub fn non_null(non_null: &internal::TSNonNullExpression<'_>) -> Self {
        Self::NonNull {
            gap: NonNullGap::Bang {
                operand_end: non_null.expression.span().end,
                bang_end: non_null.span.end,
            },
        }
    }

    /// Create a new non-null node whose region a parenthesized operand keeps
    pub fn non_null_after_paren_operand() -> Self {
        Self::NonNull {
            gap: NonNullGap::InsideOperandParens,
        }
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

    /// Get the **chain-level** comment range for this node (object_end, property_start)
    /// — the region whose comments the chain builder emits ahead of the line break it
    /// puts in front of the node.
    ///
    /// Returns None for nodes that get no such break. A [`Self::NonNull`] has a region
    /// of its own but is never one of them: the `!` binds under `[no LineTerminator
    /// here]`, so a break before it is a syntax error and the node prints that region
    /// itself (see [`NonNullGap`]).
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
            Self::Base { .. } | Self::Call { .. } | Self::NonNull { .. } => None,
        }
    }

    /// The region *inside* this node's widened comment gap that belongs to other
    /// emitters: its own object subtree.
    ///
    /// `Some` only where the linearizer widened [`Self::comment_range`] backward over a
    /// stripped grouping paren (`apply_paren_gaps`). That widening reaches a comment the
    /// author wrote just inside the `(`, which prettier relocates to just before this
    /// node; but it also sweeps the whole object subtree lying between the `(` and this
    /// node's own gap, and every member access in there already claims its own comments.
    /// The node's real claim is therefore two DISJOINT regions — the stripped-paren
    /// prefix and its own gap — with this subtree excluded between them; claiming the
    /// span whole printed every comment in the object twice
    /// (`docs/comments.md` hazard 3).
    ///
    /// Deliberately not applied to the *layout* predicates, which keep asking the whole
    /// widened range: a comment anywhere in it still occupies the page here.
    pub const fn paren_gap_skip(&self) -> Option<Span> {
        match self {
            Self::Member { paren_gap_skip, .. }
            | Self::PrivateMember { paren_gap_skip, .. }
            | Self::ComputedMember { paren_gap_skip, .. } => *paren_gap_skip,
            Self::Base { .. } | Self::Call { .. } | Self::NonNull { .. } => None,
        }
    }

    /// Check if this is a non-null node
    pub const fn is_non_null(&self) -> bool {
        matches!(self, Self::NonNull { .. })
    }

    /// Whether this link is written with `?.`, which makes the whole chain a
    /// `ChainExpression` in the public AST.
    ///
    /// The linearizer keeps a **sealed** optional chain (`(a?.b).c`) as a
    /// [`Self::Base`], so a chain answering `true` here is one whose own top level is
    /// optional — exactly the nesting the public AST wraps. Read by
    /// [`super::resolve_inline_lookups`], where that wrapper stops two of prettier's
    /// `shouldInline` ancestor walks.
    pub const fn is_optional_link(&self) -> bool {
        match self {
            Self::Member { optional, .. }
            | Self::PrivateMember { optional, .. }
            | Self::ComputedMember { optional, .. } => *optional,
            Self::Call { call, .. } => call.optional,
            Self::Base { .. } | Self::NonNull { .. } => false,
        }
    }

    /// Check if this is a numeric computed accessor like `[0]`, `[1]`
    pub fn is_numeric_accessor(&self) -> bool {
        matches!(self, Self::ComputedMember { expr, .. } if is_numeric_index(expr))
    }

    /// Check if this is a computed member access
    pub const fn is_computed(&self) -> bool {
        matches!(self, Self::ComputedMember { .. })
    }

    /// Get the property name channel (+ its span start) for Member nodes
    pub const fn property(&self) -> Option<(IdentName<'a>, u32)> {
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
}

impl<'a> Default for ChainGroup<'a> {
    fn default() -> Self {
        Self::new()
    }
}
