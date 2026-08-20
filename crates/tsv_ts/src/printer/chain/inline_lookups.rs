// Which of a member chain's `.prop` lookups carry a break point — prettier's
// `printMemberExpression` `shouldInline` (member.js), the half of it a chain cannot
// answer for itself.
//
// `shouldInline` is one disjunction over six clauses. Two are answerable from the chain's
// own linearized shape and are settled where the lookup is built — `node.computed`
// (`starts_segment` in `member_only.rs`, the paren-base arm in `builder/mod.rs`) and the
// lone `a.prop` off a bare base (`lone_lookup_off_bare_base`). A third, `BindExpression`,
// is a stage-0 proposal tsv does not parse, so it can never fire.
//
// The remaining three key on the chain's PARENT, which the chain has no pointer to —
// prettier reads them off `path.ancestors`. tsv supplies them the other way round: the
// parent marks the span it is about to build, and the chain root reads the mark once,
// before any node doc is built. TWO marks carry the three, because they say different
// things about which lookups are affected:
//
// - [`Printer::mark_inline_every_member_lookup`] — the assignment-TARGET clause and the
//   `new`-CALLEE clause. Both make the whole chain one unbreakable unit, so the width
//   falls to whatever the chain is built from (a call's arguments, a computed index's
//   brackets, a parenthesized base's parens) or overflows.
// - [`Printer::mark_member_call_tail_operand`] — the call-object clause, which reaches
//   only a chain sitting DIRECTLY under an assignment or declarator, and there inlines
//   only the last lookup, and only off a call with arguments.
//
// Both are keyed by span and not consumed: a chain rebuilt across `conditional_group`
// candidates must answer the same way every time, and a same-shaped chain nested deeper
// has a different span and keeps its width-driven break points.

use super::types::{ChainGroup, ChainNode};
use crate::ast::internal::Expression;
use crate::printer::Printer;
use std::cell::Cell;
use tsv_lang::Span;

/// The `shouldInline` answers a chain's PARENT supplies, resolved once at the chain root
/// ([`resolve_inline_lookups`]) and threaded down to the sites that place break points.
///
/// A struct rather than an enum because the two fields answer to different prettier
/// clauses and are read at different sites — `every` at the three lookup emitters,
/// `call_tail` at the peel. They only ever agree by coincidence, and folding them would
/// make a site read the clause it does not implement.
#[derive(Debug, Clone, Copy, Default)]
pub struct InlineLookups {
    /// EVERY `.prop` lookup in this chain carries no break point.
    pub every: bool,
    /// A lone `.prop` tail off a call WITH ARGUMENTS carries no break point.
    pub call_tail: bool,
}

impl InlineLookups {
    /// The answer for a chain no parent marked — and for every peeled SUB-chain, which is
    /// not the span either mark named.
    pub const NONE: Self = Self {
        every: false,
        call_tail: false,
    };
}

/// Read both marks for the chain at `chain_span`.
///
/// Called at the chain root, **before any node doc is built**: a nested assignment inside
/// the chain (a computed index, `a[x = 1].b`) marks its own operands and would clear
/// these first.
///
/// ⚠️ An **optional chain** declines `every`. In the public AST an optional chain is
/// wrapped in a `ChainExpression`, and neither ancestor walk that feeds `every` passes
/// through one — `shouldInlineNewExpressionCallee` steps only through member `object` and
/// `TSNonNullExpression` links, and `firstNonMemberParent` stops at the first node that is
/// neither — so both clauses stop there and the lookups stay breakable. tsv has no
/// `ChainExpression` node: the fact lives in the chain's own links, one of which is
/// `optional`. A **sealed** optional chain (`(a?.b).c`) is not one of them — the
/// linearizer keeps the sealed part as a `Base`, which is exactly the nesting the public
/// AST gives it, so the outer lookups inline in both formatters.
///
/// `call_tail` does NOT decline: its `firstNonChainElementWrapperParent` walk skips
/// `ChainExpression` by construction, so prettier's answer there is unchanged by the `?.`.
/// The two walks reading the same wrapper oppositely is prettier's, not a tsv choice.
pub fn resolve_inline_lookups(
    chain_span: Span,
    groups: &[ChainGroup<'_>],
    printer: &Printer<'_>,
) -> InlineLookups {
    let marked = |mark: &Cell<Option<Span>>| mark.get() == Some(chain_span);
    InlineLookups {
        every: marked(&printer.inline_every_member_lookup)
            && !groups
                .iter()
                .any(|g| g.nodes.iter().any(ChainNode::is_optional_link)),
        call_tail: marked(&printer.inline_member_call_tail),
    }
}

impl<'a> Printer<'a> {
    /// Record that the member chain at `left` is about to be built as an assignment
    /// **target**, whose `.prop` lookups therefore carry no break point.
    ///
    /// Prettier's clause is `firstNonMemberParent.type === "AssignmentExpression" &&
    /// firstNonMemberParent.left.type !== "Identifier"`, so `a = <chain>` keeps the chain
    /// breakable while `a.b = …` does not — which is why a plain identifier target CLEARS
    /// the mark rather than leaving a stale one.
    ///
    /// The assignment **expression** is the only caller, because it is the only shape the
    /// clause names. A `for (a.b.c of …)` head is an assignment target in the grammar's
    /// sense but not in `shouldInline`'s: its `firstNonMemberParent` is the
    /// `ForOfStatement`, so prettier keeps those lookups breakable, and so does tsv. A
    /// destructuring target (`[a.b.c] = v`) declines through the span key rather than this
    /// match: the mark records the *pattern's* span, which no chain inside it shares.
    /// Prettier declines it too, one step earlier — the member's `firstNonMemberParent` is
    /// the pattern, not the assignment.
    pub(in crate::printer) fn mark_assignment_target_member_lookups(&self, left: &Expression<'_>) {
        self.mark_inline_every_member_lookup(match left {
            Expression::Identifier(_) => None,
            _ => Some(left.span()),
        });
    }

    /// Record that the member chain at `callee` is about to be built as a `new`
    /// expression's **callee**, whose `.prop` lookups therefore carry no break point.
    ///
    /// Prettier's clause is `shouldInlineNewExpressionCallee`, a walk up through member
    /// `object` and `TSNonNullExpression` links to a `NewExpression` whose `callee` is the
    /// chain — so the constructor name prints as one unbreakable unit and an over-width
    /// `new` sheds its width into the argument list, or overflows when there are none.
    ///
    /// Marked unconditionally, so no stale mark survives: a callee that is not a chain
    /// (`new Foo()`) never reaches a read site, and one the walk would decline — a chain
    /// inside a computed index (`new a[b.c.d]()`), or one under a cast (`new (a.b as T)()`)
    /// — declines through the span key, since neither is the callee's own span. The
    /// remaining decline, an optional-chain callee, is the reader's
    /// ([`resolve_inline_lookups`]).
    pub(in crate::printer) fn mark_new_callee_member_lookups(&self, callee: &Expression<'_>) {
        self.mark_inline_every_member_lookup(Some(callee.span()));
    }

    /// Record that the member chain at `operand` sits DIRECTLY under an assignment or a
    /// variable declarator, the position prettier's call-object clause is gated on
    /// (`firstNonChainElementWrapperParent.type === "AssignmentExpression" ||
    /// "VariableDeclarator"`).
    ///
    /// Only the assignment's **value** and the declarator's **initializer** call this. The
    /// clause names no side, but an assignment's left is already covered by the strictly
    /// stronger target clause above — a member-chain left is never an `Identifier`, so it
    /// inlines *every* lookup, of which this clause's last one is a subset.
    ///
    /// Marked unconditionally for the same reason the callee mark is. The shape half of
    /// the clause — the object being a call with arguments, and the tail being that one
    /// lookup — is the chain's own to check, at the peel ([`super::builder`]).
    pub(in crate::printer) fn mark_member_call_tail_operand(&self, operand: &Expression<'_>) {
        self.inline_member_call_tail.set(Some(operand.span()));
    }

    /// The shared setter behind the two `every` producers, so neither can forget that a
    /// declined chain must CLEAR the mark rather than leave the previous one standing.
    fn mark_inline_every_member_lookup(&self, chain: Option<Span>) {
        self.inline_every_member_lookup.set(chain);
    }
}
