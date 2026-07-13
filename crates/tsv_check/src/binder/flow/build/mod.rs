//! The flow-graph construction walk — `FlowBuilder` and the `build_flow` entry.
//!
//! Split by the AST shape each visitor descends: this parent module owns the
//! `FlowBuilder` struct (with `SavedFlow` / `ActiveLabelEntry`), the
//! flow-node-minting constructors (`newFlowNode*` / `createFlow*` /
//! `finishFlowLabel` / `addAntecedent` family), and the container / statement-list
//! traversal driver (`enter_container` / `exit_container` / `run` /
//! `visit_statement_list`). The per-node visitors live in the submodules — each
//! contributes its own `impl FlowBuilder` block: `statements` (statement dispatch,
//! the per-statement flow shapers, and the declaration-container descents),
//! `expressions` (`visit_expression`, the `bindCondition` machinery, the
//! function/class-expression containers, and the pattern visitors) — and the pure
//! AST predicates the walk dispatches on live in `predicates`. Purely a locality
//! split — no behavior distinction between the files.

mod expressions;
mod predicates;
mod statements;

use super::*;
use crate::binder::{BoundFile, NodeKind, addr_of};
use predicates::{is_false_keyword, is_true_keyword};
use smallvec::SmallVec;
use tsv_ts::ast::Program;
use tsv_ts::ast::internal::{Expression, Statement};

#[cfg(test)]
pub(super) use predicates::is_narrowable_reference;

/// Build the flow product for one parsed file, from its `Program` and the F0
/// [`BoundFile`] (the node-identity source). Invoked from `bind_program`'s
/// per-unit loop for parsed non-lib units (lib files skip flow construction —
/// no consumer reads lib flow and ambient files have no executable code).
#[must_use]
pub fn build_flow<'a>(program: &Program<'_>, source: &'a str, bound: &'a BoundFile) -> FlowProduct {
    let mut b = FlowBuilder::new(bound, source);
    b.run(program);
    b.finish()
}

// --- FlowBuilder -----------------------------------------------------------

/// Saved control-flow state restored at a flow-container boundary
/// (binder.go:1517-1524, the F1b subset — `activeLabelList` / `seenThisKeyword`
/// stay F2/unported). The true/false targets are **not** in tsgo's container
/// save set; F1b adds them (see the module header — the pointer-free
/// `isTopLevelLogicalExpression` heuristic needs a container to reset the
/// condition context).
struct SavedFlow {
    current_flow: FlowNodeId,
    current_return_target: Option<FlowNodeId>,
    current_exception_target: Option<FlowNodeId>,
    current_break_target: Option<FlowNodeId>,
    current_continue_target: Option<FlowNodeId>,
    current_true_target: Option<FlowNodeId>,
    current_false_target: Option<FlowNodeId>,
    has_explicit_return: bool,
    /// `saveActiveLabelList` (binder.go:1522) — the active labeled statements,
    /// cleared at every control-flow container (a label can't be jumped to from a
    /// nested function, even a flow-transparent IIFE) and restored on exit.
    active_label_list: Vec<ActiveLabelEntry>,
}

/// An entry in the active-label stack (`ActiveLabel`, binder.go:85-94), used LIFO
/// (innermost last). The name is recovered on demand from the label identifier's
/// span (`spans[label_node_id]`) rather than stored owned.
struct ActiveLabelEntry {
    /// The label's post-statement break target (`postStatementLabel`).
    break_target: FlowNodeId,
    /// The continue target, set by `set_continue_target` when the label directly
    /// encloses a loop (`None` for a label on a non-loop statement).
    continue_target: Option<FlowNodeId>,
    /// Whether a labeled `break`/`continue` resolved to this label — an
    /// unreferenced label's identifier gets the `Unreachable` stamp (the TS7028
    /// signal, binder.go:2167).
    referenced: bool,
    /// The label identifier's `NodeId` (the `Unreachable`-stamp target + the
    /// name-lookup key).
    label_node_id: NodeId,
}

/// The flow-graph construction walk.
pub(super) struct FlowBuilder<'a> {
    bound: &'a BoundFile,
    /// The host document — the label-name lookup extracts `spans[id]` slices.
    source: &'a str,

    // graph columns
    pub(super) flags: Vec<FlowFlags>,
    subject: Vec<u32>,
    antecedent: Vec<u32>,
    pool: Vec<u32>,

    /// Per-active-label scratch antecedent lists, keyed by the label's
    /// `FlowNodeId`, flushed to `pool` at `finish_flow_label`
    /// (the `newFlowList` cons-list analog).
    label_scratch: crate::hash::FxHashMap<FlowNodeId, SmallVec<[FlowNodeId; 4]>>,

    // products
    flow_of_node: Vec<Option<FlowNodeId>>,
    node_flags: Vec<u8>,
    end_flow: Vec<(NodeId, FlowNodeId)>,
    return_flow: Vec<(NodeId, FlowNodeId)>,
    /// Case-clause fallthrough anchors (`FallthroughFlowNode`, binder.go:2121),
    /// sorted by `NodeId` in `finish()` like `end_flow`/`return_flow`.
    fallthrough_flow: Vec<(NodeId, FlowNodeId)>,
    /// Switch-clause payloads (`createFlowSwitchClause`); a `SwitchClause` node's
    /// `subject` slot is a 1-based index into this.
    switch_payloads: Vec<FlowSwitchClause>,
    /// Reduce-label payloads (`createReduceLabel`, try/finally); a `ReduceLabel`
    /// node's `subject` slot is a 1-based index into this.
    reduce_payloads: Vec<FlowReduceLabel>,
    /// The active labeled-statement stack (`activeLabelList`), used LIFO —
    /// innermost is the last element. Saved/cleared/restored at every container.
    active_label_list: Vec<ActiveLabelEntry>,

    // construction state (the F1b subset of the container-boundary set)
    current_flow: FlowNodeId,
    pub(super) unreachable_flow: FlowNodeId,
    current_return_target: Option<FlowNodeId>,
    /// Always `None` in F1b (`createFlowMutation` reads it; only try/finally sets
    /// it, which is F2), but ported so the exception hook is faithful.
    current_exception_target: Option<FlowNodeId>,
    /// Unlabeled-`break` / `continue` targets (binder.go:1546-1547) — set by the
    /// loop/switch binders, `None` outside a loop/switch, reset at a container.
    current_break_target: Option<FlowNodeId>,
    current_continue_target: Option<FlowNodeId>,
    /// `preSwitchCaseFlow` (binder.go:67) — the switch-head flow every clause
    /// forks from. Set by `bind_switch_statement` after the discriminant is
    /// bound, saved/restored there (not in the container set — it is only live
    /// while binding a switch's case block), `None` otherwise.
    pre_switch_case_flow: Option<FlowNodeId>,
    /// The condition-branch targets (binder.go:1790-1793). Set only inside
    /// `do_with_conditional_branches` and swapped by the `!`-prefix; their
    /// `Some`-ness is the pointer-free `isTopLevelLogicalExpression` signal (see
    /// the module header). Reset at a container so a nested function body binds
    /// its own logicals as top-level.
    current_true_target: Option<FlowNodeId>,
    current_false_target: Option<FlowNodeId>,
    /// `hasExplicitReturn` (binder.go:1549) — set by `return`, saved+reset at a
    /// container. Dark plumbing in F1b (the `HasExplicitReturn` node-flag write is
    /// F3-consumed reachability), ported for the faithful container-boundary set.
    has_explicit_return: bool,
    /// `hasFlowEffects` (binder.go:501/516) — set by `createFlowMutation` /
    /// `createFlowCall` / `return` / `throw` / `break` / `continue`; read by the
    /// logical/conditional post-label save/restore family to decide whether a
    /// post-expression label materializes. Not saved at a container (the family
    /// wrappers always reset-then-`OR`, isolating each subtree).
    has_flow_effects: bool,

    // stats
    branch_labels: u32,
    dead_labels: u32,
}

impl<'a> FlowBuilder<'a> {
    pub(super) fn new(bound: &'a BoundFile, source: &'a str) -> FlowBuilder<'a> {
        let n = bound.node_count as usize;
        let mut b = FlowBuilder {
            bound,
            source,
            flags: Vec::new(),
            subject: Vec::new(),
            antecedent: Vec::new(),
            pool: Vec::new(),
            label_scratch: crate::hash::FxHashMap::default(),
            flow_of_node: vec![None; n],
            node_flags: vec![0u8; n],
            end_flow: Vec::new(),
            return_flow: Vec::new(),
            fallthrough_flow: Vec::new(),
            switch_payloads: Vec::new(),
            reduce_payloads: Vec::new(),
            active_label_list: Vec::new(),
            current_flow: FlowNodeId::UNREACHABLE,
            unreachable_flow: FlowNodeId::UNREACHABLE,
            current_return_target: None,
            current_exception_target: None,
            current_break_target: None,
            current_continue_target: None,
            pre_switch_case_flow: None,
            current_true_target: None,
            current_false_target: None,
            has_explicit_return: false,
            has_flow_effects: false,
            branch_labels: 0,
            dead_labels: 0,
        };
        // Mint the unreachableFlow singleton FIRST → id 1 by construction
        // (binder.go:126); tsgo's pointer-identity test becomes id equality.
        b.unreachable_flow = b.new_flow_node(FlowFlags::UNREACHABLE);
        debug_assert_eq!(b.unreachable_flow, FlowNodeId::UNREACHABLE);
        b.current_flow = b.unreachable_flow;
        b
    }

    pub(super) fn finish(mut self) -> FlowProduct {
        // Flush any label whose antecedents still live in scratch: the **loop
        // labels** (`preWhile`/`preDo`/`preLoop` — referenced via their condition
        // flow and a back/continue edge, but the loop binders never call
        // `finish_flow_label` on them since a back edge can be added after the label
        // is already used, so their entry + back edges never reach the pool via the
        // collapse path), AND the **un-finished value-context post labels** — a
        // top-level logical / conditional whose subtree had no flow effects keeps
        // `current_flow` at the saved pre-expression flow and never finishes its
        // `post` label, leaving a dead, unreferenced row (matching tsgo's
        // un-finished label object). Deterministic order (sort by id) so the pool
        // layout is reproducible; the per-label edge order is push-order.
        let mut pending: Vec<FlowNodeId> = self.label_scratch.keys().copied().collect();
        pending.sort_unstable();
        for label in pending {
            let list = self.label_scratch.remove(&label).unwrap_or_default();
            if list.is_empty() {
                continue;
            }
            let off = self.pool.len() as u32;
            self.pool.push(list.len() as u32);
            self.pool.extend(list.iter().map(|e| e.get()));
            self.antecedent[label.index()] = off + 1; // 1-based pool-run index
        }
        let mut end_flow = self.end_flow;
        let mut return_flow = self.return_flow;
        let mut fallthrough_flow = self.fallthrough_flow;
        end_flow.sort_unstable_by_key(|&(n, _)| n);
        return_flow.sort_unstable_by_key(|&(n, _)| n);
        fallthrough_flow.sort_unstable_by_key(|&(n, _)| n);
        FlowProduct {
            graph: FlowGraph {
                flags: self.flags,
                subject: self.subject,
                antecedent: self.antecedent,
                pool: self.pool,
                switch_payloads: self.switch_payloads,
                reduce_payloads: self.reduce_payloads,
            },
            flow_of_node: self.flow_of_node,
            node_flags: self.node_flags,
            end_flow,
            return_flow,
            fallthrough_flow,
            stats: FlowStats {
                branch_labels: self.branch_labels,
                dead_labels: self.dead_labels,
            },
        }
    }

    // --- flow node constructors (binder.go:454-575) -----------------------

    /// `newFlowNode` (binder.go:454) — a bare node with only flags.
    pub(super) fn new_flow_node(&mut self, flags: FlowFlags) -> FlowNodeId {
        let id = FlowNodeId::from_index(self.flags.len());
        self.flags.push(flags);
        self.subject.push(0);
        self.antecedent.push(0);
        id
    }

    /// `newFlowNodeEx` (binder.go:460) — a node with a subject + single
    /// antecedent.
    fn new_flow_node_ex(
        &mut self,
        flags: FlowFlags,
        subject: Option<NodeId>,
        antecedent: FlowNodeId,
    ) -> FlowNodeId {
        let id = self.new_flow_node(flags);
        self.subject[id.index()] = subject.map_or(0, NodeId::get);
        self.antecedent[id.index()] = antecedent.get();
        id
    }

    /// `createBranchLabel` (binder.go:471).
    pub(super) fn create_branch_label(&mut self) -> FlowNodeId {
        self.branch_labels += 1;
        self.new_flow_node(FlowFlags::BRANCH_LABEL)
    }

    /// `createLoopLabel` (binder.go:467).
    fn create_loop_label(&mut self) -> FlowNodeId {
        self.new_flow_node(FlowFlags::LOOP_LABEL)
    }

    /// `createFlowMutation` (binder.go:499). The `currentExceptionTarget` hook
    /// is a no-op in F1b (that field is always `None`; try/finally sets it, F2).
    /// Sets `hasFlowEffects` (binder.go:501) — the condition/logical post-label
    /// family reads it to decide whether a post-expression label materializes.
    fn create_flow_mutation(
        &mut self,
        flags: FlowFlags,
        antecedent: FlowNodeId,
        node: NodeId,
    ) -> FlowNodeId {
        self.set_flow_node_referenced(antecedent);
        self.has_flow_effects = true;
        let result = self.new_flow_node_ex(flags, Some(node), antecedent);
        if let Some(target) = self.current_exception_target {
            self.add_antecedent(target, result);
        }
        result
    }

    /// `createFlowCall` (binder.go:514). Sets `hasFlowEffects = true`
    /// (binder.go:516) — see `create_flow_mutation`.
    fn create_flow_call(&mut self, antecedent: FlowNodeId, node: NodeId) -> FlowNodeId {
        self.set_flow_node_referenced(antecedent);
        self.has_flow_effects = true;
        self.new_flow_node_ex(FlowFlags::CALL, Some(node), antecedent)
    }

    /// `createFlowSwitchClause` (binder.go:509) — a `SwitchClause` flow node
    /// carrying the switch node + the matched half-open `[clause_start,
    /// clause_end)` clause range as a `FlowSwitchClause` payload. The `subject`
    /// slot holds a **1-based index** into `switch_payloads` (not a `NodeId`) —
    /// read via [`FlowGraph::switch_clause_data`], never [`FlowGraph::subject`].
    /// Unlike the mutation/call constructors this does **not** set
    /// `hasFlowEffects` (a switch clause is a junction, not an effect).
    fn create_flow_switch_clause(
        &mut self,
        antecedent: FlowNodeId,
        switch: NodeId,
        clause_start: u32,
        clause_end: u32,
    ) -> FlowNodeId {
        self.set_flow_node_referenced(antecedent);
        self.switch_payloads.push(FlowSwitchClause {
            switch,
            clause_start,
            clause_end,
        });
        let payload_index = self.switch_payloads.len() as u32; // 1-based
        let id = self.new_flow_node(FlowFlags::SWITCH_CLAUSE);
        self.subject[id.index()] = payload_index;
        self.antecedent[id.index()] = antecedent.get();
        id
    }

    /// `createReduceLabel` (binder.go:475) — a `ReduceLabel` node carrying a
    /// `target` label + a snapshot of a **reduced** antecedent list (flushed to
    /// the pool as a length-prefixed run, like a label). Unlike every other flow
    /// constructor this does **not** `setFlowNodeReferenced` its antecedent (tsgo
    /// `newFlowNodeEx` without the reference bump). The `subject` slot holds a
    /// **1-based index** into `reduce_payloads` (not a `NodeId`) — read via
    /// [`FlowGraph::reduce_label_data`], never [`FlowGraph::subject`].
    fn create_reduce_label(
        &mut self,
        target: FlowNodeId,
        antecedents_snapshot: &[FlowNodeId],
        antecedent: FlowNodeId,
    ) -> FlowNodeId {
        // Flush the reduced antecedent snapshot as a length-prefixed pool run.
        let off = self.pool.len() as u32;
        self.pool.push(antecedents_snapshot.len() as u32);
        self.pool
            .extend(antecedents_snapshot.iter().map(|e| e.get()));
        self.reduce_payloads.push(FlowReduceLabel {
            target,
            antecedents: off + 1, // 1-based pool-run index
        });
        let payload_index = self.reduce_payloads.len() as u32; // 1-based
        let id = self.new_flow_node(FlowFlags::REDUCE_LABEL);
        self.subject[id.index()] = payload_index;
        self.antecedent[id.index()] = antecedent.get();
        id
    }

    /// `createFlowCondition` (binder.go:479) — the condition-binding constructor.
    /// The `expression.Parent` guards (optional-chain root / nullish coalesce) are
    /// supplied by the caller, which has the parent context tsv's AST does not
    /// carry on an `Expression`; `is_narrowing` is the caller's
    /// `is_narrowing_expression` verdict.
    pub(super) fn create_flow_condition(
        &mut self,
        flags: FlowFlags,
        antecedent: FlowNodeId,
        expression: Option<(&Expression<'_>, NodeId)>,
        is_narrowing: bool,
        is_optional_chain_root: bool,
        parent_is_nullish: bool,
    ) -> FlowNodeId {
        if self.flags[antecedent.index()].contains(FlowFlags::UNREACHABLE) {
            return antecedent;
        }
        let Some((expr, expr_id)) = expression else {
            return if flags.contains(FlowFlags::TRUE_CONDITION) {
                antecedent
            } else {
                self.unreachable_flow
            };
        };
        if (is_true_keyword(expr) && flags.contains(FlowFlags::FALSE_CONDITION)
            || is_false_keyword(expr) && flags.contains(FlowFlags::TRUE_CONDITION))
            && !is_optional_chain_root
            && !parent_is_nullish
        {
            return self.unreachable_flow;
        }
        if !is_narrowing {
            return antecedent;
        }
        self.set_flow_node_referenced(antecedent);
        self.new_flow_node_ex(flags, Some(expr_id), antecedent)
    }

    /// `setFlowNodeReferenced` (binder.go:538) — first reference sets
    /// `Referenced`, thereafter `Shared`.
    fn set_flow_node_referenced(&mut self, flow: FlowNodeId) {
        let f = &mut self.flags[flow.index()];
        if f.contains(FlowFlags::REFERENCED) {
            f.insert(FlowFlags::SHARED);
        } else {
            f.insert(FlowFlags::REFERENCED);
        }
    }

    /// `addAntecedent` (binder.go:547) — order-preserving, first-write-wins
    /// **id-equality** dedup append; unreachable edges are dropped;
    /// `setFlowNodeReferenced` fires only on a genuine append.
    pub(super) fn add_antecedent(&mut self, label: FlowNodeId, antecedent: FlowNodeId) {
        if self.flags[antecedent.index()].contains(FlowFlags::UNREACHABLE) {
            return;
        }
        let list = self.label_scratch.entry(label).or_default();
        if list.contains(&antecedent) {
            return;
        }
        list.push(antecedent);
        self.set_flow_node_referenced(antecedent);
    }

    /// `finishFlowLabel` (binder.go:567) — 0 antecedents → `unreachableFlow`
    /// (a dead label row), exactly 1 → the antecedent itself (the label never
    /// enters the graph, dead row), 2+ → flush the run to the pool and keep the
    /// label.
    pub(super) fn finish_flow_label(&mut self, label: FlowNodeId) -> FlowNodeId {
        let list = self.label_scratch.remove(&label).unwrap_or_default();
        match list.as_slice() {
            [] => {
                self.dead_labels += 1;
                self.unreachable_flow
            }
            [single] => {
                self.dead_labels += 1;
                *single
            }
            edges => {
                let off = self.pool.len() as u32;
                self.pool.push(edges.len() as u32);
                self.pool.extend(edges.iter().map(|e| e.get()));
                self.antecedent[label.index()] = off + 1; // 1-based pool-run index
                label
            }
        }
    }

    // --- helpers ----------------------------------------------------------

    #[inline]
    fn require(&self, address: usize, kind: NodeKind) -> NodeId {
        self.bound.require_node_id(address, kind)
    }

    #[inline]
    fn current_unreachable(&self) -> bool {
        self.current_flow == self.unreachable_flow
    }

    /// Stamp `flow_of_node[id] = current_flow` (a leaf write — unconditional,
    /// so a dead leaf keeps `Some(unreachable)`, matching tsgo's token nodes
    /// that bypass `bindChildren`).
    #[inline]
    fn set_flow_leaf(&mut self, id: NodeId) {
        self.flow_of_node[id.index()] = Some(self.current_flow);
    }

    /// Stamp `flow_of_node[id]` for a **non-leaf** node whose bind()-switch
    /// write is nil'd by `bindChildren` in dead code — so it lands only when
    /// reachable (dead → left `None`).
    #[inline]
    fn set_flow_nonleaf(&mut self, id: NodeId) {
        if !self.current_unreachable() {
            self.flow_of_node[id.index()] = Some(self.current_flow);
        }
    }

    // --- container save/restore (binder.go:1516-1591, F1 subset) ----------

    /// Enter a control-flow container: fresh `Start` (unless flow-transparent),
    /// optional return target, exception target reset.
    fn enter_container(
        &mut self,
        start_subject: Option<NodeId>,
        transparent: bool,
        wants_return_target: bool,
    ) -> SavedFlow {
        let saved = SavedFlow {
            current_flow: self.current_flow,
            current_return_target: self.current_return_target,
            current_exception_target: self.current_exception_target,
            current_break_target: self.current_break_target,
            current_continue_target: self.current_continue_target,
            current_true_target: self.current_true_target,
            current_false_target: self.current_false_target,
            has_explicit_return: self.has_explicit_return,
            // Cleared even for a flow-transparent IIFE: a label outside the
            // callee can't be `break`/`continue`-targeted from inside it.
            active_label_list: std::mem::take(&mut self.active_label_list),
        };
        if !transparent {
            let start = self.new_flow_node(FlowFlags::START);
            if let Some(s) = start_subject {
                self.subject[start.index()] = s.get();
            }
            self.current_flow = start;
        }
        self.current_return_target = if wants_return_target {
            Some(self.create_branch_label())
        } else {
            None
        };
        self.current_exception_target = None;
        self.current_break_target = None;
        self.current_continue_target = None;
        // Reset the condition context so a nested body binds its own logicals as
        // top-level (see the module header — the pointer-free
        // `isTopLevelLogicalExpression` heuristic). tsgo leaves these untouched.
        self.current_true_target = None;
        self.current_false_target = None;
        self.has_explicit_return = false;
        saved
    }

    /// Exit a control-flow container: the postlude (end-of-flow anchor, return
    /// target merge, restore). `is_ctor_or_static` gates the `return_flow`
    /// anchor; `function_like && body_present` gates `end_flow`.
    fn exit_container(
        &mut self,
        saved: SavedFlow,
        transparent: bool,
        function_like: bool,
        body_present: bool,
        anchor: NodeId,
        is_ctor_or_static: bool,
    ) {
        if !self.current_unreachable() && function_like && body_present {
            self.end_flow.push((anchor, self.current_flow));
        }
        if let Some(rt) = self.current_return_target {
            self.add_antecedent(rt, self.current_flow);
            self.current_flow = self.finish_flow_label(rt);
            if is_ctor_or_static {
                self.return_flow.push((anchor, self.current_flow));
            }
        }
        if !transparent {
            self.current_flow = saved.current_flow;
        }
        self.current_return_target = saved.current_return_target;
        self.current_exception_target = saved.current_exception_target;
        self.current_break_target = saved.current_break_target;
        self.current_continue_target = saved.current_continue_target;
        self.current_true_target = saved.current_true_target;
        self.current_false_target = saved.current_false_target;
        self.has_explicit_return = saved.has_explicit_return;
        self.active_label_list = saved.active_label_list;
    }

    // --- entry (SourceFile container) -------------------------------------

    fn run(&mut self, program: &Program<'_>) {
        // The SourceFile is a control-flow container: fresh Start (id 2), no
        // return target (not an IIFE/constructor), no Start subject.
        let root = self.require(addr_of(program), NodeKind::Program);
        let start = self.new_flow_node(FlowFlags::START);
        self.current_flow = start;
        self.current_return_target = None;
        self.current_exception_target = None;
        self.current_break_target = None;
        self.current_continue_target = None;
        self.current_true_target = None;
        self.current_false_target = None;
        self.has_explicit_return = false;
        self.visit_statement_list(program.body);
        // SourceFile end_flow is unconditional (binder.go:1567-1569).
        self.end_flow.push((root, self.current_flow));
    }

    // --- statement lists (functions-first, binder.go:1766) ----------------

    fn visit_statement_list(&mut self, stmts: &[Statement<'_>]) {
        for stmt in stmts {
            if matches!(stmt, Statement::FunctionDeclaration(_)) {
                self.visit_statement(stmt);
            }
        }
        for stmt in stmts {
            if !matches!(stmt, Statement::FunctionDeclaration(_)) {
                self.visit_statement(stmt);
            }
        }
    }
}
