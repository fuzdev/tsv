// --- FlowFlags -------------------------------------------------------------

/// The flow-node flag bits — a `u16` newtype over tsgo's 13 `FlowFlags`
/// (flow.go:5-23; the max bit is `Shared`, `1 << 12`, so a `u16` fits). All 13
/// bits are defined for shape; `SwitchClause` (F2a) and `ReduceLabel` (F2b) are
/// set by the flow builder, while `ArrayMutation` is never *set* (its two ordinary
/// mutation sites are deliberately skipped per the F2 census — a narrowing hint).
///
/// # tsgo
/// `internal/ast/flow.go` `FlowFlags`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FlowFlags(u16);

impl FlowFlags {
    /// Unreachable code.
    pub const UNREACHABLE: FlowFlags = FlowFlags(1 << 0);
    /// Start of the flow graph.
    pub const START: FlowFlags = FlowFlags(1 << 1);
    /// Non-looping junction.
    pub const BRANCH_LABEL: FlowFlags = FlowFlags(1 << 2);
    /// Looping junction.
    pub const LOOP_LABEL: FlowFlags = FlowFlags(1 << 3);
    /// Assignment.
    pub const ASSIGNMENT: FlowFlags = FlowFlags(1 << 4);
    /// Condition known to be true.
    pub const TRUE_CONDITION: FlowFlags = FlowFlags(1 << 5);
    /// Condition known to be false.
    pub const FALSE_CONDITION: FlowFlags = FlowFlags(1 << 6);
    /// Switch-statement clause (set by the switch flow builder, F2a).
    pub const SWITCH_CLAUSE: FlowFlags = FlowFlags(1 << 7);
    /// Potential array mutation — never set (its two ordinary mutation sites are
    /// deliberately skipped per the F2 census; a narrowing-only hint).
    pub const ARRAY_MUTATION: FlowFlags = FlowFlags(1 << 8);
    /// Potential assertion call.
    pub const CALL: FlowFlags = FlowFlags(1 << 9);
    /// Temporarily reduce antecedents of a label (set by try/finally, F2b).
    pub const REDUCE_LABEL: FlowFlags = FlowFlags(1 << 10);
    /// Referenced as an antecedent once.
    pub const REFERENCED: FlowFlags = FlowFlags(1 << 11);
    /// Referenced as an antecedent more than once.
    pub const SHARED: FlowFlags = FlowFlags(1 << 12);
    /// `BranchLabel | LoopLabel`.
    pub const LABEL: FlowFlags = FlowFlags((1 << 2) | (1 << 3));
    /// `TrueCondition | FalseCondition`.
    pub const CONDITION: FlowFlags = FlowFlags((1 << 5) | (1 << 6));

    /// Whether every bit of `other` is set.
    #[inline]
    #[must_use]
    pub const fn contains(self, other: FlowFlags) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any bit of `other` is set.
    #[inline]
    #[must_use]
    pub const fn intersects(self, other: FlowFlags) -> bool {
        self.0 & other.0 != 0
    }

    /// Set `other`'s bits.
    #[inline]
    pub(super) fn insert(&mut self, other: FlowFlags) {
        self.0 |= other.0;
    }

    /// Whether this is a label node (`BranchLabel` or `LoopLabel`).
    #[inline]
    #[must_use]
    pub const fn is_label(self) -> bool {
        self.intersects(FlowFlags::LABEL)
    }
}
