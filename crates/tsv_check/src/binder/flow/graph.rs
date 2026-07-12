use super::*;

// --- F2 payload shapes (defined for the SoA shape; not populated in F1a) ----

/// A switch-clause payload: the switch it belongs to and the half-open
/// `[clause_start, clause_end)` clause range it matched. Written by the switch
/// flow builder (binder.go:2087-2108) and read back through
/// [`FlowGraph::switch_clause_data`].
#[derive(Clone, Copy, Debug)]
pub struct FlowSwitchClause {
    /// The switch statement node.
    pub switch: NodeId,
    /// Inclusive clause-range start index.
    pub clause_start: u32,
    /// Exclusive clause-range end index.
    pub clause_end: u32,
}

/// A reduce-label payload — the try/finally "temporarily reduce a label's
/// antecedents" node (`createReduceLabel`, binder.go:475/2042-2045). Written by
/// the try/finally flow builder and read back through
/// [`FlowGraph::reduce_label_data`].
#[derive(Clone, Copy, Debug)]
pub struct FlowReduceLabel {
    /// The label whose antecedent set is temporarily reduced.
    pub target: FlowNodeId,
    /// **1-based** pool-run index of the reduced antecedent list (the same
    /// length-prefixed pool convention the label pool uses).
    pub antecedents: u32,
}

// --- FlowGraph -------------------------------------------------------------

/// A per-file control-flow graph in struct-of-arrays form (per
/// `TODO_TYPECHECKER_INTERNALS.md` §Flow graph). Backward edges only (the
/// checker walks use→def).
///
/// Columns are indexed by `FlowNodeId::index()`. Antecedents are
/// kind-discriminated via `flags`: a non-label node's `antecedent` slot is the
/// single antecedent's raw id (0 = none); a label's slot is a **1-based
/// pool-run index** (0 = the label collapsed / was never finalized). The pool
/// stores length-prefixed runs (`[len, edge0, edge1, …]`); the entry edge is
/// appended first and order is preserved (load-bearing for P3), never sorted.
pub struct FlowGraph {
    pub(super) flags: Vec<FlowFlags>,
    /// Kind-discriminated by `flags`: a `NodeId` (raw, 1-based) | payload index
    /// | 0 = none. In F1a it is always a `NodeId` or 0.
    pub(super) subject: Vec<u32>,
    /// Non-label: the single antecedent's raw `FlowNodeId` (0 = none).
    /// Label: a 1-based pool-run index (0 = collapsed / unfinalized).
    pub(super) antecedent: Vec<u32>,
    /// Length-prefixed antecedent runs for labels (`[len, e0, e1, …]`).
    pub(super) pool: Vec<u32>,
    /// Switch-clause payloads, addressed by a `SwitchClause` node's 1-based
    /// `subject` slot (read via [`FlowGraph::switch_clause_data`]).
    pub(super) switch_payloads: Vec<FlowSwitchClause>,
    /// Reduce-label payloads (try/finally), addressed by a `ReduceLabel` node's
    /// 1-based `subject` slot (read via [`FlowGraph::reduce_label_data`]).
    pub(super) reduce_payloads: Vec<FlowReduceLabel>,
}

impl FlowGraph {
    /// The number of flow nodes in the graph (id 1 is `unreachableFlow`).
    #[inline]
    #[must_use]
    pub fn node_count(&self) -> u32 {
        self.flags.len() as u32
    }

    /// The flags of a flow node.
    #[inline]
    #[must_use]
    pub fn flags(&self, id: FlowNodeId) -> FlowFlags {
        self.flags[id.index()]
    }

    /// The subject `NodeId` of a flow node, if any (labels have none).
    ///
    /// **Not** valid on a `SwitchClause` node: there the `subject` slot holds a
    /// 1-based payload index, not a raw `NodeId` — use
    /// [`FlowGraph::switch_clause_data`] instead.
    #[inline]
    #[must_use]
    pub fn subject(&self, id: FlowNodeId) -> Option<NodeId> {
        NodeId::from_raw_opt(self.subject[id.index()])
    }

    /// The switch-clause payload of a `SwitchClause` flow node.
    ///
    /// A `SwitchClause` node's `subject` slot stores a **1-based index** into
    /// `switch_payloads` (the same convention the label pool uses), not a
    /// [`NodeId`], so [`FlowGraph::subject`] must not be called on it — it would
    /// mis-decode the index as a node id. This is the only correct reader;
    /// callers gate on `flags(id).contains(SWITCH_CLAUSE)`.
    #[must_use]
    pub fn switch_clause_data(&self, id: FlowNodeId) -> &FlowSwitchClause {
        debug_assert!(self.flags(id).contains(FlowFlags::SWITCH_CLAUSE));
        let index = self.subject[id.index()] as usize; // 1-based
        &self.switch_payloads[index - 1]
    }

    /// The reduce-label payload of a `ReduceLabel` flow node.
    ///
    /// Like a `SwitchClause` node, a `ReduceLabel` node's `subject` slot stores a
    /// **1-based index** into `reduce_payloads`, not a [`NodeId`], so
    /// [`FlowGraph::subject`] must not be called on it. Callers gate on
    /// `flags(id).contains(REDUCE_LABEL)`. The payload's `antecedents` field is a
    /// 1-based pool-run index of the reduced antecedent list.
    #[must_use]
    pub fn reduce_label_data(&self, id: FlowNodeId) -> &FlowReduceLabel {
        debug_assert!(self.flags(id).contains(FlowFlags::REDUCE_LABEL));
        let index = self.subject[id.index()] as usize; // 1-based
        &self.reduce_payloads[index - 1]
    }

    /// A length-prefixed pool run as a raw slice (`slot` is 1-based; 0 = empty).
    #[inline]
    fn pool_run(&self, slot: u32) -> &[u32] {
        if slot == 0 {
            return &[];
        }
        let off = (slot - 1) as usize;
        let len = self.pool[off] as usize;
        &self.pool[off + 1..off + 1 + len]
    }

    /// The single antecedent of a **non-label** flow node (`None` for `Start` /
    /// `Unreachable`) — the O(1) slot read the CFA's linear-chain walk follows
    /// (tsgo's `flow.Antecedent` chase), no pool touch, no allocation.
    ///
    /// Not valid on a label node, whose slot holds a pool-run index — decode
    /// those via [`FlowGraph::antecedents_iter`].
    #[inline]
    #[must_use]
    pub fn single_antecedent(&self, id: FlowNodeId) -> Option<FlowNodeId> {
        debug_assert!(
            !self.flags[id.index()].is_label(),
            "a label's antecedent slot is a pool-run index — use antecedents_iter"
        );
        FlowNodeId::from_raw(self.antecedent[id.index()])
    }

    /// The antecedents of a flow node, in append order, as a **zero-alloc**
    /// borrowing iterator — the hot-path form for the CFA walkers (label
    /// recursion iterates this; linear chains take [`FlowGraph::single_antecedent`]).
    /// Labels decode their length-prefixed pool run; non-label nodes yield their
    /// 0-or-1 slot.
    pub fn antecedents_iter(&self, id: FlowNodeId) -> impl Iterator<Item = FlowNodeId> + '_ {
        let flags = self.flags[id.index()];
        let slot = self.antecedent[id.index()];
        let (run, single) = if flags.is_label() {
            (self.pool_run(slot), None)
        } else {
            (&[][..], FlowNodeId::from_raw(slot))
        };
        run.iter()
            .filter_map(|&raw| FlowNodeId::from_raw(raw))
            .chain(single)
    }

    /// The reduced antecedent list of a `ReduceLabel` node, in append order, as
    /// a **zero-alloc** borrowing iterator (the temporary antecedent subset the
    /// checker substitutes for `target` while it passes this node).
    pub fn reduce_label_antecedents_iter(
        &self,
        id: FlowNodeId,
    ) -> impl Iterator<Item = FlowNodeId> + '_ {
        let data = self.reduce_label_data(id);
        self.pool_run(data.antecedents)
            .iter()
            .filter_map(|&raw| FlowNodeId::from_raw(raw))
    }

    /// [`FlowGraph::reduce_label_antecedents_iter`], collected (the convenient
    /// form for tests and the DOT renderer; hot paths take the iterator).
    #[must_use]
    pub fn reduce_label_antecedents(&self, id: FlowNodeId) -> Vec<FlowNodeId> {
        self.reduce_label_antecedents_iter(id).collect()
    }

    /// [`FlowGraph::antecedents_iter`], collected (the convenient form for tests
    /// and the DOT renderer; hot paths take the iterator).
    #[must_use]
    pub fn antecedents(&self, id: FlowNodeId) -> Vec<FlowNodeId> {
        self.antecedents_iter(id).collect()
    }
}
