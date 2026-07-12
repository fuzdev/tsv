use super::*;
use crate::binder::{BoundFile, NodeKind, addr_of, expression_addr_kind, statement_kind};
use smallvec::SmallVec;
use tsv_ts::ast::Program;
use tsv_ts::ast::internal::{
    ArrowFunctionBody, AssignmentOperator, BinaryExpression, BinaryOperator, BreakStatement,
    ClassDeclaration, ClassExpression, ClassMember, ConditionalExpression, ContinueStatement,
    Decorator, DoWhileStatement, Expression, ForInOfLeft, ForInit, ForStatement,
    FunctionDeclaration, FunctionExpression, Identifier, IfStatement, LabeledStatement,
    LiteralValue, MethodDefinition, MethodKind, ObjectPatternProperty, ObjectProperty, Property,
    Statement, SwitchCase, SwitchStatement, TSModuleDeclarationBody, TryStatement, UnaryOperator,
    VariableDeclarator, WhileStatement,
};

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

    // --- statements -------------------------------------------------------

    fn visit_statement(&mut self, stmt: &Statement<'_>) {
        let id = self.require(addr_of(stmt), statement_kind(stmt));
        if self.current_unreachable() {
            // bindChildren dead path (binder.go:1651): the non-leaf statement's
            // flow attachment is nil (already `None`); mark potentially-
            // executable nodes; then descend generically (no flow shaping).
            if is_potentially_executable(stmt) {
                self.node_flags[id.index()] |= crate::binder::NODE_FLAGS_UNREACHABLE;
            }
            self.descend_children_generic(stmt);
            return;
        }
        // Reachable: statement-range nodes capture the entry flow before the
        // construct dispatches (binder.go:1663).
        if is_statement_range(stmt) {
            self.flow_of_node[id.index()] = Some(self.current_flow);
        }
        match stmt {
            Statement::ExpressionStatement(s) => {
                self.visit_expression(&s.expression);
                self.maybe_bind_expression_flow_if_call(&s.expression);
            }
            Statement::VariableDeclaration(d) => {
                for decl in d.declarations {
                    self.bind_variable_declaration_flow(decl);
                }
            }
            Statement::ReturnStatement(s) => {
                // `bindReturnStatement` (binder.go:1939).
                if let Some(a) = &s.argument {
                    self.visit_expression(a);
                }
                if let Some(rt) = self.current_return_target {
                    self.add_antecedent(rt, self.current_flow);
                }
                self.current_flow = self.unreachable_flow;
                self.has_explicit_return = true;
                self.has_flow_effects = true;
            }
            Statement::ThrowStatement(s) => {
                // `bindThrowStatement` (binder.go:1949).
                self.visit_expression(&s.argument);
                self.current_flow = self.unreachable_flow;
                self.has_flow_effects = true;
            }
            // --- F1b: branching control-flow topology ---------------------
            Statement::IfStatement(s) => self.bind_if_statement(s),
            Statement::WhileStatement(s) => self.bind_while_statement(id, s),
            Statement::DoWhileStatement(s) => self.bind_do_statement(id, s),
            Statement::ForStatement(s) => self.bind_for_statement(id, s),
            Statement::ForInStatement(s) => {
                self.bind_for_in_or_of(id, &s.left, &s.right, s.body);
            }
            Statement::ForOfStatement(s) => {
                self.bind_for_in_or_of(id, &s.left, &s.right, s.body);
            }
            Statement::BreakStatement(s) => self.bind_break_statement(s),
            Statement::ContinueStatement(s) => self.bind_continue_statement(s),
            Statement::SwitchStatement(s) => self.bind_switch_statement(id, s),
            Statement::TryStatement(s) => self.bind_try_statement(s),
            Statement::LabeledStatement(s) => self.bind_labeled_statement(s),
            // Everything else (declarations, blocks, exports, modules) threads
            // flow linearly through its children.
            _ => self.descend_children_generic(stmt),
        }
    }

    /// Descend a statement's value children threading `current_flow` linearly,
    /// with **no** flow shaping — the `bindEachChild` analog. Used by the
    /// **dead-code path** (where linear descent is correct — nothing is
    /// reachable) for every statement kind, and by the reachable `_` arm for the
    /// kinds without their own shaper (declarations, blocks, and the F2
    /// sequential placeholders — labeled / try / exports / modules). The
    /// branching arms below (`if` / the loops / `switch` / `break` / `continue`)
    /// are therefore reached **only in dead code**; the reachable topology lives
    /// in `visit_statement`. Containers nested here still open their own `Start`
    /// regions, so a function body stays reachable even in dead code.
    fn descend_children_generic(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::ExpressionStatement(s) => self.visit_expression(&s.expression),
            Statement::VariableDeclaration(d) => {
                for decl in d.declarations {
                    self.visit_expression(&decl.id);
                    if let Some(init) = &decl.init {
                        self.visit_expression(init);
                    }
                }
            }
            Statement::FunctionDeclaration(f) => {
                let id = self.require(addr_of(stmt), statement_kind(stmt));
                self.visit_function_declaration(f, id);
            }
            Statement::ClassDeclaration(c) => self.visit_class_decl(c),
            Statement::ReturnStatement(s) => {
                if let Some(a) = &s.argument {
                    self.visit_expression(a);
                }
            }
            Statement::ThrowStatement(s) => self.visit_expression(&s.argument),
            Statement::BlockStatement(b) => self.visit_statement_list(b.body),
            // --- dead-path linear descent for the branching kinds (their real
            //     topology lives in `visit_statement`; reached only when dead) ---
            Statement::IfStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.consequent);
                if let Some(alt) = s.alternate {
                    self.visit_statement(alt);
                }
            }
            Statement::ForStatement(s) => {
                match &s.init {
                    Some(ForInit::VariableDeclaration(d)) => {
                        for decl in d.declarations {
                            self.visit_expression(&decl.id);
                            if let Some(init) = &decl.init {
                                self.visit_expression(init);
                            }
                        }
                    }
                    Some(ForInit::Expression(e)) => self.visit_expression(e),
                    None => {}
                }
                if let Some(t) = &s.test {
                    self.visit_expression(t);
                }
                if let Some(u) = &s.update {
                    self.visit_expression(u);
                }
                self.visit_statement(s.body);
            }
            Statement::ForInStatement(s) => {
                self.visit_for_left(&s.left);
                self.visit_expression(&s.right);
                self.visit_statement(s.body);
            }
            Statement::ForOfStatement(s) => {
                self.visit_for_left(&s.left);
                self.visit_expression(&s.right);
                self.visit_statement(s.body);
            }
            Statement::WhileStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.body);
            }
            Statement::DoWhileStatement(s) => {
                self.visit_statement(s.body);
                self.visit_expression(&s.test);
            }
            Statement::SwitchStatement(s) => {
                self.visit_expression(&s.discriminant);
                for case in s.cases {
                    if let Some(t) = &case.test {
                        self.visit_expression(t);
                    }
                    self.visit_statement_list(case.consequent);
                }
            }
            Statement::TryStatement(s) => {
                self.visit_statement_list(s.block.body);
                if let Some(handler) = &s.handler {
                    if let Some(param) = &handler.param {
                        self.visit_expression(param);
                    }
                    self.visit_statement_list(handler.body.body);
                }
                if let Some(finalizer) = &s.finalizer {
                    self.visit_statement_list(finalizer.body);
                }
            }
            Statement::LabeledStatement(s) => {
                // Dead-path fallback; the reachable topology lives in
                // `bind_labeled_statement`.
                self.visit_identifier(&s.label);
                self.visit_statement(s.body);
            }
            Statement::BreakStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label);
                }
            }
            Statement::ContinueStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label);
                }
            }
            Statement::ExportNamedDeclaration(e) => {
                if let Some(inner) = e.declaration {
                    self.visit_statement(inner);
                }
                // export specifiers / source are non-value (skipped).
            }
            Statement::ExportDefaultDeclaration(e) => self.visit_export_default(e),
            Statement::TSExportAssignment(ea) => self.visit_expression(&ea.expression),
            Statement::TSModuleDeclaration(m) => self.visit_module(m),
            // No value content (types / imports / enum bodies / empty): skipped,
            // per the "types are not descended" scope note. See module docs.
            Statement::TSTypeAliasDeclaration(_)
            | Statement::TSInterfaceDeclaration(_)
            | Statement::TSDeclareFunction(_)
            | Statement::TSEnumDeclaration(_)
            | Statement::ImportDeclaration(_)
            | Statement::TSImportEqualsDeclaration(_)
            | Statement::ExportAllDeclaration(_)
            | Statement::TSNamespaceExportDeclaration(_)
            | Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_) => {}
        }
    }

    fn visit_for_left(&mut self, left: &ForInOfLeft<'_>) {
        use ForInOfLeft as L;
        match left {
            L::VariableDeclaration(d) => {
                for decl in d.declarations {
                    self.visit_expression(&decl.id);
                    if let Some(init) = &decl.init {
                        self.visit_expression(init);
                    }
                }
            }
            L::Pattern(e) => self.visit_expression(e),
        }
    }

    // --- statement flow shapers -------------------------------------------

    /// `maybeBindExpressionFlowIfCall` (binder.go:2143): a top-level dotted-name
    /// (non-`super`) call is a potential assertion → `createFlowCall`.
    fn maybe_bind_expression_flow_if_call(&mut self, expr: &Expression<'_>) {
        if let Expression::CallExpression(c) = expr
            && !matches!(c.callee, Expression::Super(_))
            && is_dotted_name(c.callee)
        {
            let call_id = self.require(addr_of(c), NodeKind::CallExpression);
            self.current_flow = self.create_flow_call(self.current_flow, call_id);
        }
    }

    /// `bindVariableDeclarationFlow` + `bindInitializedVariableFlow`
    /// (binder.go:2314) — a `var/let/const x = e` with an initializer emits one
    /// unconditional `Assignment`. The name binds as a **binding target**
    /// (`bind_binding_target`), so a destructuring default (`let {a = e} = …`)
    /// forks per `bindInitializer`. A destructuring pattern emits one
    /// `Assignment` per declarator (tsv has no binding-element node — see the
    /// module scope note).
    fn bind_variable_declaration_flow(&mut self, decl: &VariableDeclarator<'_>) {
        self.bind_binding_target(&decl.id);
        if let Some(init) = &decl.init {
            self.visit_expression(init);
        }
        if decl.init.is_some() {
            let decl_id = self.require(addr_of(decl), NodeKind::VariableDeclarator);
            self.current_flow =
                self.create_flow_mutation(FlowFlags::ASSIGNMENT, self.current_flow, decl_id);
        }
    }

    // --- branching statement flow shapers ---------------------------------

    /// `bindIfStatement` (binder.go:1924) — then/else branch labels merge at
    /// `postIf`; each branch binds against the condition-split flow.
    fn bind_if_statement(&mut self, s: &IfStatement<'_>) {
        let then_label = self.create_branch_label();
        let else_label = self.create_branch_label();
        let post_if = self.create_branch_label();
        self.bind_condition(Some(&s.test), then_label, else_label, false);
        self.current_flow = self.finish_flow_label(then_label);
        self.visit_statement(s.consequent);
        self.add_antecedent(post_if, self.current_flow);
        self.current_flow = self.finish_flow_label(else_label);
        if let Some(alt) = s.alternate {
            self.visit_statement(alt);
        }
        self.add_antecedent(post_if, self.current_flow);
        self.current_flow = self.finish_flow_label(post_if);
    }

    /// `bindWhileStatement` (binder.go:1857) — the entry edge is added to the
    /// loop label **before** it becomes `current_flow`; the back edge **after**
    /// the body.
    fn bind_while_statement(&mut self, stmt_id: NodeId, s: &WhileStatement<'_>) {
        let loop_label = self.create_loop_label();
        let pre_while = self.set_continue_target(stmt_id, loop_label);
        let pre_body = self.create_branch_label();
        let post_while = self.create_branch_label();
        self.add_antecedent(pre_while, self.current_flow); // entry edge (first)
        self.current_flow = pre_while;
        self.bind_condition(Some(&s.test), pre_body, post_while, false);
        self.current_flow = self.finish_flow_label(pre_body);
        self.bind_iterative_statement(s.body, post_while, pre_while);
        self.add_antecedent(pre_while, self.current_flow); // back edge (after)
        self.current_flow = self.finish_flow_label(post_while);
    }

    /// `bindDoStatement` (binder.go:1871) — the body runs from the loop label
    /// first; the continue target is a **pre-condition** branch label (not the
    /// loop label), and the condition loops back to the loop label.
    fn bind_do_statement(&mut self, stmt_id: NodeId, s: &DoWhileStatement<'_>) {
        let pre_do = self.create_loop_label();
        let condition_label = self.create_branch_label();
        let pre_condition = self.set_continue_target(stmt_id, condition_label);
        let post_do = self.create_branch_label();
        self.add_antecedent(pre_do, self.current_flow);
        self.current_flow = pre_do;
        self.bind_iterative_statement(s.body, post_do, pre_condition);
        self.add_antecedent(pre_condition, self.current_flow);
        self.current_flow = self.finish_flow_label(pre_condition);
        self.bind_condition(Some(&s.test), pre_do, post_do, false);
        self.current_flow = self.finish_flow_label(post_do);
    }

    /// `bindForStatement` (binder.go:1885) — init → loop label → condition →
    /// body (continue = the increment label) → incrementor → back edge.
    fn bind_for_statement(&mut self, stmt_id: NodeId, s: &ForStatement<'_>) {
        let loop_label = self.create_loop_label();
        let pre_loop = self.set_continue_target(stmt_id, loop_label);
        let pre_body = self.create_branch_label();
        let pre_increment = self.create_branch_label();
        let post_loop = self.create_branch_label();
        match &s.init {
            Some(ForInit::VariableDeclaration(d)) => {
                for decl in d.declarations {
                    self.bind_variable_declaration_flow(decl);
                }
            }
            Some(ForInit::Expression(e)) => self.visit_expression(e),
            None => {}
        }
        self.add_antecedent(pre_loop, self.current_flow);
        self.current_flow = pre_loop;
        // A nil condition is a true passthrough / false-unreachable, handled by
        // `create_flow_condition`'s nil-expression arm.
        self.bind_condition(s.test.as_ref(), pre_body, post_loop, false);
        self.current_flow = self.finish_flow_label(pre_body);
        self.bind_iterative_statement(s.body, post_loop, pre_increment);
        self.add_antecedent(pre_increment, self.current_flow);
        self.current_flow = self.finish_flow_label(pre_increment);
        if let Some(u) = &s.update {
            self.visit_expression(u);
        }
        self.add_antecedent(pre_loop, self.current_flow); // back edge
        self.current_flow = self.finish_flow_label(post_loop);
    }

    /// `bindForInOrOfStatement` (binder.go:1904). The exit edge is
    /// **unconditional** (a for-in/of can exit after zero iterations); continue
    /// targets the loop label. Shared by `for-in` and `for-of` (the for-of
    /// `await` modifier is a `bool` in tsv — no node to bind, no fork).
    fn bind_for_in_or_of(
        &mut self,
        stmt_id: NodeId,
        left: &ForInOfLeft<'_>,
        right: &Expression<'_>,
        body: &Statement<'_>,
    ) {
        let loop_label = self.create_loop_label();
        let pre_loop = self.set_continue_target(stmt_id, loop_label);
        let post_loop = self.create_branch_label();
        self.visit_expression(right);
        self.add_antecedent(pre_loop, self.current_flow);
        self.current_flow = pre_loop;
        self.add_antecedent(post_loop, self.current_flow); // unconditional exit
        // Bind the initializer (binder.go:1915-1918). A declaration-list variable
        // is assigned each iteration (`bindVariableDeclarationFlow`'s for-in/of
        // guard, binder.go:2316 — the `Assignment` mutation even with no
        // initializer); a pattern initializer runs `bindAssignmentTargetFlow`.
        match left {
            ForInOfLeft::VariableDeclaration(d) => {
                for decl in d.declarations {
                    self.bind_binding_target(&decl.id);
                    if let Some(init) = &decl.init {
                        self.visit_expression(init);
                    }
                    let decl_id = self.require(addr_of(decl), NodeKind::VariableDeclarator);
                    self.current_flow = self.create_flow_mutation(
                        FlowFlags::ASSIGNMENT,
                        self.current_flow,
                        decl_id,
                    );
                }
            }
            ForInOfLeft::Pattern(p) => {
                self.visit_expression(p);
                self.bind_assignment_target_flow(p);
            }
        }
        self.bind_iterative_statement(body, post_loop, pre_loop);
        self.add_antecedent(pre_loop, self.current_flow); // back edge
        self.current_flow = self.finish_flow_label(post_loop);
    }

    /// `setContinueTarget` (binder.go:1779) — walk the parent chain up from a
    /// loop while each parent is a `LabeledStatement`, assigning that label's
    /// continue target (so `continue L` on a labeled loop lands on the loop's
    /// continue point), in lockstep with the active-label stack from its top. No
    /// enclosing labeled statements → a no-op returning `target` unchanged.
    fn set_continue_target(&mut self, loop_node: NodeId, target: FlowNodeId) -> FlowNodeId {
        let mut node = loop_node;
        let mut i = self.active_label_list.len();
        loop {
            let Some(parent) = self.bound.parents[node.index()] else {
                break;
            };
            if self.bound.kinds[parent.index()] != NodeKind::LabeledStatement || i == 0 {
                break;
            }
            i -= 1;
            self.active_label_list[i].continue_target = Some(target);
            node = parent;
        }
        target
    }

    /// `bindIterativeStatement` (binder.go:1807) — bind a loop body with its
    /// break/continue targets installed, restored on exit.
    fn bind_iterative_statement(
        &mut self,
        body: &Statement<'_>,
        break_target: FlowNodeId,
        continue_target: FlowNodeId,
    ) {
        let save_break = self.current_break_target;
        let save_continue = self.current_continue_target;
        self.current_break_target = Some(break_target);
        self.current_continue_target = Some(continue_target);
        self.visit_statement(body);
        self.current_break_target = save_break;
        self.current_continue_target = save_continue;
    }

    /// `bindBreakStatement` (binder.go:1955) — a labeled `break L` resolves to
    /// `L`'s **break** target (`findActiveLabel`, marking it referenced); an
    /// unlabeled `break` uses `current_break_target`. An unresolved label is a
    /// no-op (deferred diagnostic).
    fn bind_break_statement(&mut self, s: &BreakStatement<'_>) {
        match &s.label {
            None => {
                let target = self.current_break_target;
                self.bind_break_or_continue_flow(target);
            }
            Some(label) => {
                self.visit_identifier(label);
                let name = self.label_text(label);
                if let Some(i) = self.find_active_label(name) {
                    self.active_label_list[i].referenced = true;
                    let target = Some(self.active_label_list[i].break_target);
                    self.bind_break_or_continue_flow(target);
                }
            }
        }
    }

    /// `bindContinueStatement` (binder.go:1959) — a labeled `continue L` resolves
    /// to `L`'s **continue** target; an unlabeled `continue` uses
    /// `current_continue_target`. A missing/`None` target is a no-op.
    fn bind_continue_statement(&mut self, s: &ContinueStatement<'_>) {
        match &s.label {
            None => {
                let target = self.current_continue_target;
                self.bind_break_or_continue_flow(target);
            }
            Some(label) => {
                self.visit_identifier(label);
                let name = self.label_text(label);
                if let Some(i) = self.find_active_label(name) {
                    self.active_label_list[i].referenced = true;
                    let target = self.active_label_list[i].continue_target;
                    self.bind_break_or_continue_flow(target);
                }
            }
        }
    }

    /// `bindBreakOrContinueFlow` (binder.go:1985) — route to the target and go
    /// unreachable; a `None` target (break/continue outside any loop/switch) is a
    /// no-op (the parser accepts it; the illegal-jump diagnostic is F3+).
    fn bind_break_or_continue_flow(&mut self, target: Option<FlowNodeId>) {
        if let Some(t) = target {
            self.add_antecedent(t, self.current_flow);
            self.current_flow = self.unreachable_flow;
            self.has_flow_effects = true;
        }
    }

    /// `bindSwitchStatement` (binder.go:2074) — a `switch` with a **local**
    /// post-switch break target (so a contained `break` resolves here, not at an
    /// enclosing loop) and the real clause topology (`bind_case_block`). When no
    /// clause is a `default`, an **unconditional** `(0, 0)` `SwitchClause`
    /// exhaustiveness sentinel — "no clause matched" — feeds the post-switch
    /// label alongside the case-block exit. `preSwitchCaseFlow` is captured
    /// **after** the discriminant is bound (the flow every clause forks from) and
    /// saved/restored here, as in tsgo (it is not in the container save set).
    fn bind_switch_statement(&mut self, switch_id: NodeId, s: &SwitchStatement<'_>) {
        let post_switch = self.create_branch_label();
        self.visit_expression(&s.discriminant);
        let save_break = self.current_break_target;
        let save_pre_switch = self.pre_switch_case_flow;
        self.current_break_target = Some(post_switch);
        self.pre_switch_case_flow = Some(self.current_flow);
        self.bind_case_block(switch_id, s);
        self.add_antecedent(post_switch, self.current_flow);
        let has_default = s.cases.iter().any(|c| c.test.is_none());
        if !has_default {
            // The "no clause matched" fall-off: reachable from the switch head
            // regardless of narrowing (an empty `(0, 0)` range is the sentinel).
            let pre_switch = self.pre_switch_case_flow.unwrap_or(self.unreachable_flow);
            let sentinel = self.create_flow_switch_clause(pre_switch, switch_id, 0, 0);
            self.add_antecedent(post_switch, sentinel);
        }
        self.current_break_target = save_break;
        self.pre_switch_case_flow = save_pre_switch;
        self.current_flow = self.finish_flow_label(post_switch);
    }

    /// `bindCaseBlock` (binder.go:2095) — thread the clauses. Each clause's
    /// `preCase` label is fed **from the switch head** (`preSwitchCaseFlow`,
    /// unconditionally — a narrowing switch wraps it in a per-clause
    /// `SwitchClause` node) plus the prior clause's fallthrough edge, so a clause
    /// reached only after a prior `break`/`return` stays reachable (the F2a
    /// reachability fix). An empty-clause run (`case a: case b:` with no
    /// statements) re-points to the head only when nothing live falls into it.
    fn bind_case_block(&mut self, switch_id: NodeId, s: &SwitchStatement<'_>) {
        let clauses = s.cases;
        let is_narrowing_switch =
            is_true_keyword(&s.discriminant) || is_narrowing_expression(&s.discriminant);
        let last = clauses.len().wrapping_sub(1);
        let mut fallthrough_flow = self.unreachable_flow;
        let mut i = 0;
        while i < clauses.len() {
            let clause_start = i as u32;
            // Empty-clause run: advance past clauses with no statements (bar the
            // last), re-pointing to the head only when nothing live falls in.
            while clauses[i].consequent.is_empty() && i + 1 < clauses.len() {
                if fallthrough_flow == self.unreachable_flow {
                    self.current_flow = self.pre_switch_case_flow.unwrap_or(self.unreachable_flow);
                }
                self.bind_case_or_default_clause(&clauses[i]);
                i += 1;
            }
            let pre_case = self.create_branch_label();
            let pre_switch = self.pre_switch_case_flow.unwrap_or(self.unreachable_flow);
            let pre_case_flow = if is_narrowing_switch {
                self.create_flow_switch_clause(pre_switch, switch_id, clause_start, i as u32 + 1)
            } else {
                pre_switch
            };
            self.add_antecedent(pre_case, pre_case_flow); // head edge (reachability fix)
            self.add_antecedent(pre_case, fallthrough_flow); // fallthrough (unreachable = no-op)
            self.current_flow = self.finish_flow_label(pre_case);
            self.bind_case_or_default_clause(&clauses[i]);
            fallthrough_flow = self.current_flow;
            if !self.current_unreachable() && i != last {
                let clause_id = self.require(addr_of(&clauses[i]), NodeKind::SwitchCase);
                self.fallthrough_flow.push((clause_id, self.current_flow));
            }
            i += 1;
        }
    }

    /// `bindCaseOrDefaultClause` (binder.go:2126) — the clause's test expression
    /// binds under the switch head (`preSwitchCaseFlow`, saved/restored), its
    /// statements under the current (post-`preCase`) flow.
    fn bind_case_or_default_clause(&mut self, case: &SwitchCase<'_>) {
        if let Some(test) = &case.test {
            let saved = self.current_flow;
            self.current_flow = self.pre_switch_case_flow.unwrap_or(self.unreachable_flow);
            self.visit_expression(test);
            self.current_flow = saved;
        }
        self.visit_statement_list(case.consequent);
    }

    // --- try / catch / finally --------------------------------------------

    /// A snapshot of a label's pending antecedent list (`label.Antecedents`) —
    /// the try/finally combine reads three of these directly (the pointer-free
    /// `combineFlowLists` analog).
    fn scratch_snapshot(&self, label: FlowNodeId) -> SmallVec<[FlowNodeId; 4]> {
        self.label_scratch.get(&label).cloned().unwrap_or_default()
    }

    /// `bindTryStatement` (binder.go:1993). Three fresh labels — `normalExit`,
    /// `returnLabel`, `exceptionLabel` — thread the "any instruction can throw"
    /// edge (`exceptionLabel` seeded from `current_flow` **before** the try
    /// block, `current_exception_target` repointed so `create_flow_mutation`'s
    /// fan-out comes alive). A catch is bound as a **second try** (a fresh
    /// `exceptionLabel` seeded from the first one's finish). With a finally, the
    /// finally label's antecedents = `normal ++ exception ++ return`
    /// (`combineFlowLists`), it becomes `current_flow`, and up to three
    /// `ReduceLabel`s route the finally's completion back through the return /
    /// outer-exception / normal-exit subsets (binder.go:2052-2067).
    fn bind_try_statement(&mut self, s: &TryStatement<'_>) {
        let save_return_target = self.current_return_target;
        let save_exception_target = self.current_exception_target;
        let normal_exit = self.create_branch_label();
        let return_label = self.create_branch_label();
        let mut exception_label = self.create_branch_label();
        if s.finalizer.is_some() {
            self.current_return_target = Some(return_label);
        }
        // The exception edge for exceptions before any mutation.
        self.add_antecedent(exception_label, self.current_flow);
        self.current_exception_target = Some(exception_label);
        self.visit_statement_list(s.block.body);
        self.add_antecedent(normal_exit, self.current_flow);
        if let Some(handler) = &s.handler {
            // The catch is the target of exceptions from the try block; its own
            // exceptions flow to a fresh label (catch = a second try).
            self.current_flow = self.finish_flow_label(exception_label);
            exception_label = self.create_branch_label();
            self.add_antecedent(exception_label, self.current_flow);
            self.current_exception_target = Some(exception_label);
            if let Some(param) = &handler.param {
                // The catch variable is a binding position (tsgo reaches it via
                // bindBindingElementFlow → bindInitializer), so a flow-changing
                // destructuring default forks — bind_binding_target, not the plain
                // value walk. Equivalent for a non-defaulted param.
                self.bind_binding_target(param);
            }
            self.visit_statement_list(handler.body.body);
            self.add_antecedent(normal_exit, self.current_flow);
        }
        // Restore BEFORE the finally — the finally isn't inside its own try.
        self.current_return_target = save_return_target;
        self.current_exception_target = save_exception_target;
        if let Some(finalizer) = &s.finalizer {
            let normal_list = self.scratch_snapshot(normal_exit);
            let exception_list = self.scratch_snapshot(exception_label);
            let return_list = self.scratch_snapshot(return_label);
            let finally_label = self.create_branch_label();
            // finallyLabel.Antecedents = normal ++ exception ++ return
            // (combineFlowLists, no dedup — faithful to binder.go:2043).
            let mut combined: SmallVec<[FlowNodeId; 4]> = SmallVec::new();
            combined.extend(normal_list.iter().copied());
            combined.extend(exception_list.iter().copied());
            combined.extend(return_list.iter().copied());
            self.label_scratch.insert(finally_label, combined);
            self.current_flow = finally_label;
            self.visit_statement_list(finalizer.body);
            if self.current_unreachable() {
                // An unreachable end-of-finally makes the whole try unreachable.
                self.current_flow = self.unreachable_flow;
            } else {
                // Route the finally's completion back through the return-only
                // subset (IIFE/constructor return target), then the outer
                // exception-only subset, then continue via the normal subset.
                if let Some(rt) = self.current_return_target
                    && !return_list.is_empty()
                {
                    let rl =
                        self.create_reduce_label(finally_label, &return_list, self.current_flow);
                    self.add_antecedent(rt, rl);
                }
                if let Some(et) = self.current_exception_target
                    && !exception_list.is_empty()
                {
                    let el =
                        self.create_reduce_label(finally_label, &exception_list, self.current_flow);
                    self.add_antecedent(et, el);
                }
                if normal_list.is_empty() {
                    self.current_flow = self.unreachable_flow;
                } else {
                    self.current_flow =
                        self.create_reduce_label(finally_label, &normal_list, self.current_flow);
                }
            }
        } else {
            self.current_flow = self.finish_flow_label(normal_exit);
        }
    }

    // --- labeled statements -----------------------------------------------

    /// `bindLabeledStatement` (binder.go:2153). Push an active-label entry
    /// (break target = `postStatementLabel`, continue target set later by a
    /// directly-enclosed loop's `set_continue_target`), bind the label + body,
    /// then pop; an **unreferenced** label gets the `Unreachable` stamp on its
    /// identifier (the TS7028 signal, binder.go:2167). The post label merges the
    /// body's exit.
    fn bind_labeled_statement(&mut self, s: &LabeledStatement<'_>) {
        let post = self.create_branch_label();
        let label_id = self.require(addr_of(&s.label), NodeKind::Identifier);
        self.active_label_list.push(ActiveLabelEntry {
            break_target: post,
            continue_target: None,
            referenced: false,
            label_node_id: label_id,
        });
        self.visit_identifier(&s.label);
        self.visit_statement(s.body);
        // Balanced with the push above (pop is always `Some`). An unreferenced
        // label's identifier gets the `Unreachable` stamp (the TS7028 signal).
        if let Some(entry) = self.active_label_list.pop()
            && !entry.referenced
        {
            self.node_flags[entry.label_node_id.index()] |= crate::binder::NODE_FLAGS_UNREACHABLE;
        }
        self.add_antecedent(post, self.current_flow);
        self.current_flow = self.finish_flow_label(post);
    }

    /// `findActiveLabel` (binder.go:1976) — innermost-first (the stack top is the
    /// last element, so scan from the end). Returns the stack index.
    fn find_active_label(&self, name: &str) -> Option<usize> {
        self.active_label_list
            .iter()
            .rposition(|e| self.bound.spans[e.label_node_id.index()].extract(self.source) == name)
    }

    /// The source text of a label identifier (the break/continue label name).
    fn label_text(&self, ident: &Identifier<'_>) -> &'a str {
        let id = self.require(addr_of(ident), NodeKind::Identifier);
        self.bound.spans[id.index()].extract(self.source)
    }

    // --- condition binding (the bindCondition machinery) ------------------

    /// `doWithConditionalBranches` (binder.go:1789) — bind `value` with the given
    /// true/false targets installed, restored on exit. A `None` value is the
    /// nil-node no-op (`for (;;)`).
    fn do_with_conditional_branches(
        &mut self,
        value: Option<&Expression<'_>>,
        true_target: FlowNodeId,
        false_target: FlowNodeId,
    ) {
        let saved_true = self.current_true_target;
        let saved_false = self.current_false_target;
        self.current_true_target = Some(true_target);
        self.current_false_target = Some(false_target);
        if let Some(v) = value {
            self.visit_expression(v);
        }
        self.current_true_target = saved_true;
        self.current_false_target = saved_false;
    }

    /// `bindCondition` (binder.go:1799) — bind the condition through the
    /// true/false targets, then, **only for an atomic condition** (not a logical
    /// `&&`/`||`/`??` or logical compound-assignment, whose sub-binder already
    /// wired the targets), add the true/false condition edges. `parent_is_nullish`
    /// is the caller-supplied `IsNullishCoalesce(parent)` guard (the operands of a
    /// `??`); `is_optional_chain_root` is always `false` here (optional chains are
    /// atomic — their dedicated short-circuit machinery is F2).
    fn bind_condition(
        &mut self,
        node: Option<&Expression<'_>>,
        true_target: FlowNodeId,
        false_target: FlowNodeId,
        parent_is_nullish: bool,
    ) {
        self.do_with_conditional_branches(node, true_target, false_target);
        if node.is_none_or(|e| !is_logical_condition(e)) {
            let with_id = node.map(|e| (e, self.expr_id(e)));
            let is_narrowing = node.is_some_and(is_narrowing_expression);
            let tc = self.create_flow_condition(
                FlowFlags::TRUE_CONDITION,
                self.current_flow,
                with_id,
                is_narrowing,
                false,
                parent_is_nullish,
            );
            self.add_antecedent(true_target, tc);
            let fc = self.create_flow_condition(
                FlowFlags::FALSE_CONDITION,
                self.current_flow,
                with_id,
                is_narrowing,
                false,
                parent_is_nullish,
            );
            self.add_antecedent(false_target, fc);
        }
    }

    /// `bindBinaryExpressionFlow`'s logical branch (binder.go:2219) — decides
    /// value-context (top-level → a `hasFlowEffects` post-label materializes only
    /// if the subtree had flow effects) vs condition-context (nested → the
    /// enclosing true/false targets). The pointer-free
    /// `isTopLevelLogicalExpression` test is `current_true_target.is_none()` (see
    /// the module header). `assign_target` is the LHS for a logical
    /// compound-assignment (`&&=`/`||=`/`??=`), else `None`.
    fn bind_binary_expression_flow(
        &mut self,
        node: &Expression<'_>,
        left: &Expression<'_>,
        right: &Expression<'_>,
        is_and: bool,
        is_nullish: bool,
        assign_target: Option<&Expression<'_>>,
    ) {
        if self.current_true_target.is_none() {
            let post = self.create_branch_label();
            let saved_flow = self.current_flow;
            let saved_effects = self.has_flow_effects;
            self.has_flow_effects = false;
            self.bind_logical_like_expression(
                node,
                left,
                right,
                is_and,
                is_nullish,
                assign_target,
                post,
                post,
            );
            self.current_flow = if self.has_flow_effects {
                self.finish_flow_label(post)
            } else {
                saved_flow
            };
            self.has_flow_effects = self.has_flow_effects || saved_effects;
        } else {
            let t = self.current_true_target.unwrap_or(self.unreachable_flow);
            let f = self.current_false_target.unwrap_or(self.unreachable_flow);
            self.bind_logical_like_expression(
                node,
                left,
                right,
                is_and,
                is_nullish,
                assign_target,
                t,
                f,
            );
        }
    }

    /// `bindLogicalLikeExpression` (binder.go:2261) — narrow the left operand
    /// against a fresh `preRight` label (vs the false target for `&&`/`&&=`, vs
    /// the true target otherwise), then the right against the original targets;
    /// a logical compound-assignment additionally mutates its target and tests
    /// the whole node.
    #[allow(clippy::too_many_arguments)] // faithful port of the tsgo signature
    fn bind_logical_like_expression(
        &mut self,
        node: &Expression<'_>,
        left: &Expression<'_>,
        right: &Expression<'_>,
        is_and: bool,
        is_nullish: bool,
        assign_target: Option<&Expression<'_>>,
        true_target: FlowNodeId,
        false_target: FlowNodeId,
    ) {
        let pre_right = self.create_branch_label();
        if is_and {
            self.bind_condition(Some(left), pre_right, false_target, is_nullish);
        } else {
            self.bind_condition(Some(left), true_target, pre_right, is_nullish);
        }
        self.current_flow = self.finish_flow_label(pre_right);
        if let Some(target) = assign_target {
            // Logical compound-assignment (binder.go:2271-2275): bind the RHS, mutate
            // the target, then test `node` (never a boolean keyword → the parent-nullish
            // guard is irrelevant). tsgo binds the RHS with `doWithConditionalBranches`
            // (targets = the outer true/false), but the value-vs-condition split is then
            // taken by `isTopLevelLogicalExpression(right)` on `right`'s PARENT — which
            // is this `&&=`/`||=`/`??=` node, not a logical operator — so `right` is
            // classified TOP-LEVEL and its internal conditions never thread into the
            // outer targets (only the whole-node truthiness below does). tsv emulates
            // `isTopLevelLogicalExpression` pointer-free as `current_true_target.is_none()`,
            // so binding the RHS with the targets SET would misclassify a logical `right`
            // (`a &&= x && y`) as nested. The faithful adaptation is to CLEAR the targets
            // (not set them) around the RHS bind: a logical `right` then sees itself
            // top-level (its own discarded post-label), and a non-logical `right` is
            // identical either way (the targets are only read by the logical branch).
            let saved_true = self.current_true_target.take();
            let saved_false = self.current_false_target.take();
            self.visit_expression(right);
            self.current_true_target = saved_true;
            self.current_false_target = saved_false;
            self.bind_assignment_target_flow(target);
            let node_id = self.expr_id(node);
            let is_narrowing = is_narrowing_expression(node);
            let tc = self.create_flow_condition(
                FlowFlags::TRUE_CONDITION,
                self.current_flow,
                Some((node, node_id)),
                is_narrowing,
                false,
                false,
            );
            self.add_antecedent(true_target, tc);
            let fc = self.create_flow_condition(
                FlowFlags::FALSE_CONDITION,
                self.current_flow,
                Some((node, node_id)),
                is_narrowing,
                false,
                false,
            );
            self.add_antecedent(false_target, fc);
        } else {
            self.bind_condition(Some(right), true_target, false_target, is_nullish);
        }
    }

    /// `bindConditionalExpressionFlow` (binder.go:2289) — a `?:` as a value: the
    /// condition splits to true/false labels feeding the two arms, which merge at
    /// a `hasFlowEffects`-gated post label.
    fn bind_conditional_expression_flow(&mut self, c: &ConditionalExpression<'_>) {
        let true_label = self.create_branch_label();
        let false_label = self.create_branch_label();
        let post = self.create_branch_label();
        let saved_flow = self.current_flow;
        let saved_effects = self.has_flow_effects;
        self.has_flow_effects = false;
        self.bind_condition(Some(c.test), true_label, false_label, false);
        self.current_flow = self.finish_flow_label(true_label);
        self.visit_expression(c.consequent);
        self.add_antecedent(post, self.current_flow);
        self.current_flow = self.finish_flow_label(false_label);
        self.visit_expression(c.alternate);
        self.add_antecedent(post, self.current_flow);
        self.current_flow = if self.has_flow_effects {
            self.finish_flow_label(post)
        } else {
            saved_flow
        };
        self.has_flow_effects = self.has_flow_effects || saved_effects;
    }

    /// `bindAssignmentTargetFlow` (binder.go:1821), **default branch only**: a
    /// narrowable-reference target gets an `Assignment` mutation. The
    /// array/object-literal destructuring recursion (the `inAssignmentPattern`
    /// per-element machinery) is F2, alongside parameter-default forks — a
    /// destructuring target is not a narrowable reference, so it mints no mutation
    /// here, and its sub-references were already visited.
    fn bind_assignment_target_flow(&mut self, target: &Expression<'_>) {
        if is_narrowable_reference(target) {
            let id = self.expr_id(target);
            self.current_flow =
                self.create_flow_mutation(FlowFlags::ASSIGNMENT, self.current_flow, id);
        }
    }

    /// The F0 [`NodeId`] of an expression node — its variant payload's
    /// `(address, kind)` in the address map, via the shared
    /// [`expression_addr_kind`] mapping (the same one `visit_expression`'s
    /// lockstep guard pins in `binder/mod.rs`). Condition / mutation subjects are
    /// always value expressions F0 lowered, so this never misses.
    fn expr_id(&self, e: &Expression<'_>) -> NodeId {
        let (addr, kind) = expression_addr_kind(e);
        self.require(addr, kind)
    }

    // --- containers -------------------------------------------------------

    fn visit_function_declaration(&mut self, f: &FunctionDeclaration<'_>, anchor: NodeId) {
        let saved = self.enter_container(None, false, false);
        self.bind_params(f.params);
        self.visit_statement_list(f.body.body);
        self.exit_container(saved, false, true, true, anchor, false);
    }

    /// A function expression. `is_iife` marks a call callee (an IIFE): the
    /// container is entered **transparently** (no fresh `Start`, `current_flow`
    /// not restored on exit) with its own return target, so the body joins the
    /// containing control flow (binder.go:1525-1544). The return-flow anchor
    /// stays off (`is_ctor_or_static = false`) — tsgo writes it only for
    /// constructors / static blocks, never a plain IIFE.
    fn visit_function_expression(
        &mut self,
        f: &FunctionExpression<'_>,
        node_id: NodeId,
        is_iife: bool,
    ) {
        // The function-expression flow write is captured at the OUTER flow,
        // before the body's Start (binder.go:915). Unconditional: the container
        // path does not nil it in dead code.
        self.set_flow_leaf(node_id);
        let saved = self.enter_container(Some(node_id), is_iife, is_iife);
        self.bind_params(f.params);
        self.visit_statement_list(f.body.body);
        self.exit_container(saved, is_iife, true, true, node_id, false);
    }

    fn visit_arrow(
        &mut self,
        a: &tsv_ts::ast::internal::ArrowFunctionExpression<'_>,
        node_id: NodeId,
        is_iife: bool,
    ) {
        self.set_flow_leaf(node_id); // binder.go:915 (arrows dispatch here too)
        let saved = self.enter_container(Some(node_id), is_iife, is_iife);
        self.bind_params(a.params);
        match &a.body {
            ArrowFunctionBody::Expression(e) => self.visit_expression(e),
            ArrowFunctionBody::BlockStatement(block) => self.visit_statement_list(block.body),
        }
        self.exit_container(saved, is_iife, true, true, node_id, false);
    }

    fn bind_params(&mut self, params: &[Expression<'_>]) {
        for param in params {
            self.bind_binding_target(param);
        }
    }

    /// `bindInitializer` (binder.go:2474) — bind a parameter / binding-element
    /// **default** and fork `current_flow` around it, but **only** when binding
    /// the default actually changed the flow (a `BindingElement`/`Parameter` has
    /// no side effects when its initializer isn't evaluated — GH#49759). The
    /// entry/exit pointer-equality guard is exact: a literal default (`= 1`)
    /// leaves `current_flow` untouched and mints no label.
    fn bind_initializer(&mut self, initializer: &Expression<'_>) {
        let entry = self.current_flow;
        self.visit_expression(initializer);
        if entry == self.unreachable_flow || entry == self.current_flow {
            return;
        }
        let exit = self.create_branch_label();
        self.add_antecedent(exit, entry);
        self.add_antecedent(exit, self.current_flow);
        self.current_flow = self.finish_flow_label(exit);
    }

    /// Bind a **binding target** (declaration / parameter position):
    /// `bindParameterFlow` / `bindBindingElementFlow` (binder.go:2463/2450). A
    /// defaulted element's initializer is bound **before** the name (TC39 order,
    /// via `bind_initializer`, which forks only when the default changed the
    /// flow). Distinct from the value traversal (`visit_expression`) so the
    /// assignment-target destructuring recursion — a separate deferred item —
    /// stays untouched; for a non-defaulted target the two are equivalent.
    fn bind_binding_target(&mut self, node: &Expression<'_>) {
        use Expression as E;
        match node {
            E::AssignmentPattern(a) => {
                self.visit_decorators(a.decorators);
                self.bind_initializer(a.right);
                self.bind_binding_target(a.left);
            }
            E::ObjectPattern(op) => {
                self.visit_decorators(op.decorators);
                for prop in op.properties {
                    match prop {
                        ObjectPatternProperty::Property(pr) => {
                            self.visit_expression(&pr.key);
                            self.bind_binding_target(&pr.value);
                        }
                        ObjectPatternProperty::RestElement(r) => {
                            self.bind_binding_target(r.argument);
                        }
                    }
                }
            }
            E::ArrayPattern(ap) => {
                self.visit_decorators(ap.decorators);
                for el in ap.elements.iter().flatten() {
                    self.bind_binding_target(el);
                }
            }
            E::RestElement(r) => self.bind_binding_target(r.argument),
            E::TSParameterProperty(pp) => self.bind_binding_target(pp.parameter),
            // A plain identifier / other leaf binding: the ordinary traversal.
            _ => self.visit_expression(node),
        }
    }

    fn visit_class_decl(&mut self, c: &ClassDeclaration<'_>) {
        self.visit_class_common(
            c.id.as_ref(),
            c.decorators,
            c.super_class,
            c.body.body,
            false,
        );
    }

    fn visit_class_expr(&mut self, c: &ClassExpression<'_>) {
        self.visit_class_common(
            c.id.as_ref(),
            c.decorators,
            c.super_class,
            c.body.body,
            true,
        );
    }

    /// The value-flow class descent shared by the declaration and expression forms
    /// (distinct types with the same field shape): the name binding, decorators, and
    /// the `extends` expression, then each member. Type positions (type params /
    /// super type args / `implements`) are skipped. `is_class_expression` threads
    /// the parent-kind half of tsgo's
    /// `IsObjectLiteralOrClassExpressionMethodOrAccessor` gate (utilities.go:566)
    /// down to `visit_method` — tsv expressions carry no parent pointer.
    fn visit_class_common(
        &mut self,
        name: Option<&Identifier<'_>>,
        decorators: Option<&[Decorator<'_>]>,
        super_class: Option<&Expression<'_>>,
        members: &[ClassMember<'_>],
        is_class_expression: bool,
    ) {
        if let Some(name) = name {
            self.visit_identifier(name);
        }
        self.visit_decorators(decorators);
        if let Some(sc) = super_class {
            self.visit_expression(sc);
        }
        for member in members {
            self.visit_class_member(member, is_class_expression);
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember<'_>, is_class_expression: bool) {
        match member {
            ClassMember::MethodDefinition(m) => self.visit_method(m, is_class_expression),
            ClassMember::PropertyDefinition(p) => {
                self.visit_decorators(p.decorators);
                self.visit_expression(&p.key);
                // property type annotation is a type position (skip).
                if let Some(value) = &p.value {
                    // A property-with-initializer is a control-flow container
                    // (binder.go:2584): fresh Start around the initializer.
                    let p_id = self.require(addr_of(p), NodeKind::PropertyDefinition);
                    let saved = self.enter_container(None, false, false);
                    self.visit_expression(value);
                    self.exit_container(saved, false, false, false, p_id, false);
                }
            }
            ClassMember::StaticBlock(s) => {
                // A class static block is flow-transparent (binder.go:1525-1528)
                // with its own return target; `return_flow` anchors on it.
                let s_id = self.require(addr_of(s), NodeKind::StaticBlock);
                let saved = self.enter_container(None, true, true);
                self.visit_statement_list(s.body);
                self.exit_container(saved, true, true, true, s_id, true);
            }
            // index signatures are type-only (skip).
            ClassMember::IndexSignature(_) => {}
        }
    }

    fn visit_method(&mut self, m: &MethodDefinition<'_>, is_class_expression: bool) {
        self.visit_decorators(m.decorators);
        let is_ctor = m.kind == MethodKind::Constructor;
        self.visit_expression(&m.key);
        // The method body lives in `value` (a FunctionExpression); the method is
        // a control-flow container anchored on that FunctionExpression — the
        // body-bearing node (tsv wraps a method body in a FunctionExpression,
        // where tsc's method node holds the body directly). The `MethodDefinition`
        // and its inline `value` share an address (a repr reorder puts `value` at
        // offset 0), so the address map keys on `(address, NodeKind)`; anchoring
        // here resolves the FunctionExpression id via its kind, and the method
        // itself resolves separately by `NodeKind::MethodDefinition`.
        let anchor = self.require(addr_of(&m.value), NodeKind::FunctionExpression);
        // A **class-expression** method/accessor (never a constructor, never a
        // class-declaration member) gets the outer-flow write on the METHOD node
        // (bindPropertyOrMethodOrAccessor, binder.go:981) and becomes the body
        // Start's subject (binder.go:1534) — the P3 narrowing hint
        // (`IsObjectLiteralOrClassExpressionMethodOrAccessor`, utilities.go:566;
        // the object-literal half lives in `visit_object_expr_property`).
        let start_subject = if is_class_expression && !is_ctor {
            let method_id = self.require(addr_of(m), NodeKind::MethodDefinition);
            self.set_flow_leaf(method_id);
            Some(method_id)
        } else {
            None
        };
        let saved = self.enter_container(start_subject, false, is_ctor);
        self.bind_params(m.value.params);
        self.visit_statement_list(m.value.body.body);
        self.exit_container(saved, false, true, true, anchor, is_ctor);
    }

    fn visit_module(&mut self, m: &tsv_ts::ast::internal::TSModuleDeclaration<'_>) {
        use tsv_ts::ast::internal::TSModuleName;
        if let TSModuleName::Identifier(name) = &m.id {
            self.visit_identifier(name);
        }
        match &m.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                // A ModuleBlock is a control-flow container (binder.go:2582) —
                // fresh Start, no return target, not function-like.
                let block_id = self.require(addr_of(block), NodeKind::TSModuleBlock);
                let saved = self.enter_container(None, false, false);
                self.visit_statement_list(block.body);
                self.exit_container(saved, false, false, false, block_id, false);
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                self.visit_module(nested);
            }
            None => {}
        }
    }

    fn visit_export_default(&mut self, e: &tsv_ts::ast::internal::ExportDefaultDeclaration<'_>) {
        use tsv_ts::ast::internal::ExportDefaultValue as V;
        match &e.declaration {
            V::Expression(expr) => self.visit_expression(expr),
            V::FunctionDeclaration(f) => {
                let id = self.require(addr_of(f), NodeKind::FunctionDeclaration);
                self.visit_function_declaration(f, id);
            }
            V::ClassDeclaration(c) => self.visit_class_decl(c),
            // A declare function / interface has no value body (skip).
            V::TSDeclareFunction(_) | V::TSInterfaceDeclaration(_) => {}
        }
    }

    // --- expressions ------------------------------------------------------

    fn visit_expression(&mut self, expr: &Expression<'_>) {
        use Expression as E;
        // A **value** sub-position resets the condition targets, so a logical
        // expression nested inside one (`if (f(x && y))`, `if (c ? x && y : z)`,
        // `if (g([x && y]))`) is classified top-level — a value with its own
        // post-label — not a sub-condition of the enclosing `bind_condition`. This
        // is the pointer-free `isTopLevelLogicalExpression` (binder.go:2782): only
        // the *threading* variants (`!`, `&&`/`||`/`??`, logical-assignment, parens)
        // propagate the targets into their operands; every other expression is a
        // value boundary. Without the reset, `current_true_target.is_none()` stays
        // false through the whole condition subtree and mis-wires nested logicals.
        let restore = if is_condition_threading(expr) {
            None
        } else {
            Some((
                self.current_true_target.take(),
                self.current_false_target.take(),
            ))
        };
        match expr {
            E::Identifier(idn) => self.visit_identifier(idn),
            E::ThisExpression(t) => {
                let id = self.require(addr_of(t), NodeKind::ThisExpression);
                self.set_flow_leaf(id);
            }
            E::Super(s) => {
                let id = self.require(addr_of(s), NodeKind::Super);
                self.set_flow_leaf(id);
            }
            E::MetaProperty(m) => {
                // Non-leaf write (nil'd in dead code). tsv models `import`/`new`
                // and `meta`/`target` as identifiers; they are keyword-ish, not
                // references, so only the MetaProperty node is stamped.
                let id = self.require(addr_of(m), NodeKind::MetaProperty);
                self.set_flow_nonleaf(id);
            }
            E::MemberExpression(m) => {
                // The access flow write (binder.go:618): non-leaf, reachable-
                // only, gated on `isNarrowableReference`.
                if is_narrowable_reference(expr) {
                    let id = self.require(addr_of(m), NodeKind::MemberExpression);
                    self.set_flow_nonleaf(id);
                }
                self.visit_expression(m.object);
                self.visit_expression(m.property);
            }
            E::Literal(_) | E::PrivateIdentifier(_) | E::RegexLiteral(_) => {}
            E::ObjectExpression(o) => {
                for prop in o.properties {
                    self.visit_object_property(prop);
                }
            }
            E::ArrayExpression(a) => {
                for el in a.elements.iter().flatten() {
                    self.visit_expression(el);
                }
            }
            E::UnaryExpression(u) if u.operator == UnaryOperator::Bang => {
                // `bindPrefixUnaryExpressionFlow` (binder.go:2174): swap the
                // condition targets around the operand so `!x` narrows inversely.
                // The pre/post swaps are symmetric — any sub-binder restores the
                // targets to their entry value (via `do_with_conditional_branches`
                // / the `!`-swap), so the second swap is a faithful restore.
                std::mem::swap(
                    &mut self.current_true_target,
                    &mut self.current_false_target,
                );
                self.visit_expression(u.argument);
                std::mem::swap(
                    &mut self.current_true_target,
                    &mut self.current_false_target,
                );
            }
            E::UnaryExpression(u) => self.visit_expression(u.argument),
            E::UpdateExpression(u) => self.visit_expression(u.argument),
            E::BinaryExpression(b) if b.operator.is_logical() => {
                // `bindBinaryExpressionFlow` logical branch (binder.go:2219).
                let is_and = b.operator == BinaryOperator::AmpersandAmpersand;
                let is_nullish = b.operator == BinaryOperator::QuestionQuestion;
                self.bind_binary_expression_flow(expr, b.left, b.right, is_and, is_nullish, None);
            }
            E::BinaryExpression(b) => {
                self.visit_expression(b.left);
                self.visit_expression(b.right);
            }
            E::CallExpression(c) => {
                // IIFE detection (`GetImmediatelyInvokedFunctionExpression`,
                // utilities.go:1834; `bindCallExpressionFlow`, binder.go:2419):
                // a non-async (non-generator) function/arrow callee — through any
                // grouping parens — is inlined into the containing flow. Its
                // arguments bind FIRST so the callee's flow write captures the
                // post-argument flow.
                let mut unwrapped = c.callee;
                while let E::ParenthesizedExpression(p) = unwrapped {
                    unwrapped = p.expression;
                }
                match unwrapped {
                    E::ArrowFunctionExpression(a) if !a.r#async => {
                        for arg in c.arguments {
                            self.visit_expression(arg);
                        }
                        let id = self.require(addr_of(a), NodeKind::ArrowFunctionExpression);
                        self.visit_arrow(a, id, true);
                    }
                    E::FunctionExpression(f) if !f.r#async && !f.generator => {
                        for arg in c.arguments {
                            self.visit_expression(arg);
                        }
                        let id = self.require(addr_of(f), NodeKind::FunctionExpression);
                        self.visit_function_expression(f, id, true);
                    }
                    _ => {
                        self.visit_expression(c.callee);
                        for arg in c.arguments {
                            self.visit_expression(arg);
                        }
                    }
                }
            }
            E::NewExpression(n) => {
                self.visit_expression(n.callee);
                for a in n.arguments {
                    self.visit_expression(a);
                }
            }
            E::ConditionalExpression(c) => self.bind_conditional_expression_flow(c),
            E::ArrowFunctionExpression(a) => {
                let id = self.require(addr_of(a), NodeKind::ArrowFunctionExpression);
                self.visit_arrow(a, id, false);
            }
            E::FunctionExpression(f) => {
                let id = self.require(addr_of(f), NodeKind::FunctionExpression);
                self.visit_function_expression(f, id, false);
            }
            E::ClassExpression(c) => self.visit_class_expr(c),
            E::SpreadElement(s) => self.visit_expression(s.argument),
            E::TemplateLiteral(t) => {
                for e in t.expressions {
                    self.visit_expression(e);
                }
            }
            E::TaggedTemplateExpression(t) => {
                self.visit_expression(t.tag);
                for e in t.quasi.expressions {
                    self.visit_expression(e);
                }
            }
            E::AwaitExpression(a) => self.visit_expression(a.argument),
            E::YieldExpression(y) => {
                if let Some(a) = y.argument {
                    self.visit_expression(a);
                }
            }
            E::SequenceExpression(s) => {
                // `bindBinaryExpressionFlow` comma branch: each operand's value
                // is discarded (statement-like), so a top-level dotted-name call
                // is a potential assertion — apply maybe-call per operand
                // (visit-then-maybe, like `ExpressionStatement`). tsgo nests
                // comma as left-assoc `BinaryExpression`s applying maybe-call to
                // both `Left`/`Right` at each level; the flattened form applies
                // it once per leaf operand (intermediate comma nodes are no-op
                // non-calls), so the effect matches.
                // tsgo: binder.go bindBinaryExpressionFlow (comma branch)
                for e in s.expressions {
                    self.visit_expression(e);
                    self.maybe_bind_expression_flow_if_call(e);
                }
            }
            E::AssignmentExpression(a) if is_logical_assign_op(a.operator) => {
                // `bindBinaryExpressionFlow` logical compound-assignment branch.
                let is_and = a.operator == AssignmentOperator::LogicalAndAssign;
                let is_nullish = a.operator == AssignmentOperator::NullishAssign;
                self.bind_binary_expression_flow(
                    expr,
                    a.left,
                    a.right,
                    is_and,
                    is_nullish,
                    Some(a.left),
                );
            }
            E::AssignmentExpression(a) => {
                // `bindBinaryExpressionFlow` assignment branch (binder.go:2249) —
                // bind operands, then the target's `Assignment` mutation.
                self.visit_expression(a.left);
                self.visit_expression(a.right);
                self.bind_assignment_target_flow(a.left);
            }
            E::ObjectPattern(op) => {
                self.visit_decorators(op.decorators);
                for prop in op.properties {
                    self.visit_object_pattern_property(prop);
                }
            }
            E::ArrayPattern(ap) => {
                self.visit_decorators(ap.decorators);
                for el in ap.elements.iter().flatten() {
                    self.visit_expression(el);
                }
            }
            E::AssignmentPattern(a) => {
                self.visit_decorators(a.decorators);
                self.visit_expression(a.left);
                self.visit_expression(a.right);
            }
            E::RestElement(r) => self.visit_expression(r.argument),
            E::TSTypeAssertion(t) => self.visit_expression(t.expression),
            E::TSAsExpression(t) => self.visit_expression(t.expression),
            E::TSSatisfiesExpression(t) => self.visit_expression(t.expression),
            E::TSInstantiationExpression(t) => self.visit_expression(t.expression),
            E::TSNonNullExpression(t) => self.visit_expression(t.expression),
            E::TSParameterProperty(pp) => self.visit_expression(pp.parameter),
            E::ImportExpression(i) => {
                self.visit_expression(i.source);
                if let Some(o) = i.options {
                    self.visit_expression(o);
                }
            }
            E::JsdocCast(c) => self.visit_expression(c.inner),
            E::ParenthesizedExpression(p) => self.visit_expression(p.expression),
        }
        if let Some((t, f)) = restore {
            self.current_true_target = t;
            self.current_false_target = f;
        }
    }

    fn visit_identifier(&mut self, ident: &Identifier<'_>) {
        // Identifier flow write (binder.go:602): a leaf — unconditional, so a
        // dead identifier keeps `Some(unreachable)`. Its decorators (parameter
        // decorators) are value expressions; its type annotation is a type
        // position (skipped).
        let id = self.require(addr_of(ident), NodeKind::Identifier);
        self.set_flow_leaf(id);
        self.visit_decorators(ident.decorators());
    }

    fn visit_decorators(&mut self, decorators: Option<&[Decorator<'_>]>) {
        if let Some(decs) = decorators {
            for d in decs {
                self.visit_expression(&d.expression);
            }
        }
    }

    fn visit_object_property(&mut self, prop: &ObjectProperty<'_>) {
        match prop {
            ObjectProperty::Property(pr) => self.visit_object_expr_property(pr),
            ObjectProperty::SpreadElement(s) => self.visit_expression(s.argument),
        }
    }

    fn visit_object_expr_property(&mut self, pr: &Property<'_>) {
        let is_method_or_accessor =
            pr.method || pr.kind != tsv_ts::ast::internal::PropertyKind::Init;
        if let (true, Expression::FunctionExpression(f)) = (is_method_or_accessor, &pr.value) {
            // An object-literal method/accessor is a control-flow container
            // anchored on its value FunctionExpression — the body-bearing node
            // (unlike `MethodDefinition`, a `Property` does NOT share its value's
            // address, so this is a consistency choice with `visit_method`, not a
            // collision workaround). The PROPERTY node — tsv's analog of tsgo's
            // object-literal MethodDeclaration — gets the outer-flow write
            // (bindPropertyOrMethodOrAccessor, binder.go:981) and becomes the
            // body Start's subject (binder.go:1534) — the P3 narrowing hint
            // (`IsObjectLiteralOrClassExpressionMethodOrAccessor`,
            // utilities.go:566; the class-expression half lives in `visit_method`).
            self.visit_expression(&pr.key);
            let anchor = self.require(addr_of(f), NodeKind::FunctionExpression);
            let prop_id = self.require(addr_of(pr), NodeKind::Property);
            self.set_flow_leaf(prop_id);
            let saved = self.enter_container(Some(prop_id), false, false);
            self.bind_params(f.params);
            self.visit_statement_list(f.body.body);
            self.exit_container(saved, false, true, true, anchor, false);
        } else {
            self.visit_expression(&pr.key);
            self.visit_expression(&pr.value);
        }
    }

    fn visit_object_pattern_property(&mut self, prop: &ObjectPatternProperty<'_>) {
        match prop {
            ObjectPatternProperty::Property(pr) => {
                self.visit_expression(&pr.key);
                self.visit_expression(&pr.value);
            }
            ObjectPatternProperty::RestElement(r) => self.visit_expression(r.argument),
        }
    }
}

// --- pure AST predicates (binder.go / utilities.go ports) ------------------

/// `is_potentially_executable` (utilities.go:4210) — the statement range (minus
/// `Block`/`Empty`, which are below the range), with `VariableStatement` gated
/// on block-scoping or an initializer, plus class/enum/module declarations.
fn is_potentially_executable(stmt: &Statement<'_>) -> bool {
    use Statement as S;
    match stmt {
        S::ExpressionStatement(_)
        | S::IfStatement(_)
        | S::DoWhileStatement(_)
        | S::WhileStatement(_)
        | S::ForStatement(_)
        | S::ForInStatement(_)
        | S::ForOfStatement(_)
        | S::ContinueStatement(_)
        | S::BreakStatement(_)
        | S::ReturnStatement(_)
        | S::SwitchStatement(_)
        | S::LabeledStatement(_)
        | S::ThrowStatement(_)
        | S::TryStatement(_)
        | S::DebuggerStatement(_) => true,
        S::VariableDeclaration(d) => {
            use tsv_ts::ast::internal::VariableDeclarationKind as K;
            d.kind != K::Var || d.declarations.iter().any(|decl| decl.init.is_some())
        }
        S::ClassDeclaration(_) | S::TSEnumDeclaration(_) | S::TSModuleDeclaration(_) => true,
        _ => false,
    }
}

/// Whether a statement kind is in tsc's `[FirstStatement, LastStatement]` range
/// (binder.go:1663) — the entry-flow write set. Excludes `Block`/`Empty` (below
/// the range) and every declaration kind (above it).
fn is_statement_range(stmt: &Statement<'_>) -> bool {
    use Statement as S;
    matches!(
        stmt,
        S::ExpressionStatement(_)
            | S::VariableDeclaration(_)
            | S::IfStatement(_)
            | S::DoWhileStatement(_)
            | S::WhileStatement(_)
            | S::ForStatement(_)
            | S::ForInStatement(_)
            | S::ForOfStatement(_)
            | S::ContinueStatement(_)
            | S::BreakStatement(_)
            | S::ReturnStatement(_)
            | S::SwitchStatement(_)
            | S::LabeledStatement(_)
            | S::ThrowStatement(_)
            | S::TryStatement(_)
            | S::DebuggerStatement(_)
    )
}

/// `IsDottedName` (utilities.go:1613).
fn is_dotted_name(expr: &Expression<'_>) -> bool {
    use Expression as E;
    match expr {
        E::Identifier(_) | E::ThisExpression(_) | E::Super(_) | E::MetaProperty(_) => true,
        E::MemberExpression(m) if !m.computed => is_dotted_name(m.object),
        E::ParenthesizedExpression(p) => is_dotted_name(p.expression),
        _ => false,
    }
}

/// `isNarrowableReference` (binder.go:2633) — the access flow-write gate.
/// Adapted to tsv's AST (tsc's comma/assignment `BinaryExpression` cases are
/// tsv's `SequenceExpression` / `AssignmentExpression`).
pub(super) fn is_narrowable_reference(node: &Expression<'_>) -> bool {
    use Expression as E;
    match node {
        E::Identifier(_) | E::ThisExpression(_) | E::Super(_) | E::MetaProperty(_) => true,
        E::MemberExpression(m) if !m.computed => is_narrowable_reference(m.object),
        E::ParenthesizedExpression(p) => is_narrowable_reference(p.expression),
        E::TSNonNullExpression(t) => is_narrowable_reference(t.expression),
        E::MemberExpression(m) => {
            // computed element access
            is_string_or_numeric_literal_like(m.property)
                || (is_entity_name_expression(m.property) && is_narrowable_reference(m.object))
        }
        E::AssignmentExpression(a) => is_left_hand_side_expression(a.left),
        E::SequenceExpression(s) => s.expressions.last().is_some_and(is_narrowable_reference),
        _ => false,
    }
}

fn is_string_or_numeric_literal_like(node: &Expression<'_>) -> bool {
    matches!(
        node,
        Expression::Literal(l) if matches!(l.value, LiteralValue::String(_) | LiteralValue::Number(_))
    )
}

/// `IsEntityNameExpression` (utilities.go:1595) — an identifier or a dotted
/// property-access chain bottoming in one.
fn is_entity_name_expression(node: &Expression<'_>) -> bool {
    use Expression as E;
    match node {
        E::Identifier(_) => true,
        E::MemberExpression(m) if !m.computed => {
            matches!(m.property, E::Identifier(_)) && is_entity_name_expression(m.object)
        }
        _ => false,
    }
}

/// `isLeftHandSideExpressionKind` (utilities.go:396) — the postfix/primary
/// expression forms. Reached only via the rare `(x = y).z` narrowable case.
fn is_left_hand_side_expression(node: &Expression<'_>) -> bool {
    use Expression as E;
    matches!(
        node,
        E::MemberExpression(_)
            | E::NewExpression(_)
            | E::CallExpression(_)
            | E::TaggedTemplateExpression(_)
            | E::ArrayExpression(_)
            | E::ParenthesizedExpression(_)
            | E::ObjectExpression(_)
            | E::ClassExpression(_)
            | E::FunctionExpression(_)
            | E::Identifier(_)
            | E::PrivateIdentifier(_)
            | E::RegexLiteral(_)
            | E::Literal(_)
            | E::TemplateLiteral(_)
            | E::ThisExpression(_)
            | E::Super(_)
            | E::TSNonNullExpression(_)
            | E::MetaProperty(_)
            | E::ImportExpression(_)
    )
}

fn is_true_keyword(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Literal(l) if matches!(l.value, LiteralValue::Boolean(true)))
}

fn is_false_keyword(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Literal(l) if matches!(l.value, LiteralValue::Boolean(false)))
}

/// Whether a condition node is a logical `&&`/`||`/`??` or a logical
/// compound-assignment `&&=`/`||=`/`??=` — the `bindCondition` non-atomic test
/// (binder.go:1801, combining `IsLogicalExpression` + `isLogicalAssignment`).
/// Such a node's sub-binder already wired the true/false targets, so
/// `bindCondition` must NOT re-add the atomic true/false conditions.
fn is_logical_condition(e: &Expression<'_>) -> bool {
    match e {
        Expression::BinaryExpression(b) => b.operator.is_logical(),
        Expression::AssignmentExpression(a) => is_logical_assign_op(a.operator),
        _ => false,
    }
}

/// Whether an expression **threads** the enclosing condition targets into its
/// operands (vs being a value boundary that resets them). Mirrors the four
/// threading arms of `visit_expression`: `!`, `&&`/`||`/`??`, logical-assignment,
/// and parentheses — the same set tsgo's `isTopLevelLogicalExpression`
/// (binder.go:2782) ascends through. Every other expression is a value
/// sub-position (see the reset in `visit_expression`).
fn is_condition_threading(e: &Expression<'_>) -> bool {
    match e {
        Expression::UnaryExpression(u) => u.operator == UnaryOperator::Bang,
        Expression::ParenthesizedExpression(_) => true,
        _ => is_logical_condition(e),
    }
}

/// Whether an assignment operator is a logical compound-assignment
/// (`||=`/`&&=`/`??=`) — `IsLogicalOrCoalescingAssignmentOperator`.
fn is_logical_assign_op(op: AssignmentOperator) -> bool {
    matches!(
        op,
        AssignmentOperator::LogicalOrAssign
            | AssignmentOperator::LogicalAndAssign
            | AssignmentOperator::NullishAssign
    )
}

/// `isNarrowingExpression` (binder.go:2602) — the `createFlowCondition` gate.
/// Adapted to tsv's AST: comma / assignment are their own `SequenceExpression` /
/// `AssignmentExpression` nodes (tsc folds them into `BinaryExpression`), so their
/// `isNarrowingBinaryExpression` cases move here.
fn is_narrowing_expression(expr: &Expression<'_>) -> bool {
    use Expression as E;
    match expr {
        E::Identifier(_) | E::ThisExpression(_) => true,
        E::MemberExpression(_) => contains_narrowable_reference(expr),
        E::CallExpression(c) => {
            c.arguments.iter().any(contains_narrowable_reference)
                || matches!(c.callee, E::MemberExpression(m)
                    if !m.computed && contains_narrowable_reference(m.object))
        }
        E::ParenthesizedExpression(p) => is_narrowing_expression(p.expression),
        E::TSNonNullExpression(t) => is_narrowing_expression(t.expression),
        E::UnaryExpression(u)
            if u.operator == UnaryOperator::Typeof || u.operator == UnaryOperator::Bang =>
        {
            is_narrowing_expression(u.argument)
        }
        E::BinaryExpression(b) => is_narrowing_binary_expression(b),
        // The `isNarrowingBinaryExpression` assignment cases (`=`/`||=`/`&&=`/`??=`
        // → containsNarrowableReference(left)); other compound assignments are not
        // narrowing.
        E::AssignmentExpression(a) => {
            matches!(
                a.operator,
                AssignmentOperator::Assign
                    | AssignmentOperator::LogicalOrAssign
                    | AssignmentOperator::LogicalAndAssign
                    | AssignmentOperator::NullishAssign
            ) && contains_narrowable_reference(a.left)
        }
        // The `isNarrowingBinaryExpression` comma case (`isNarrowingExpression`
        // of the last operand).
        E::SequenceExpression(s) => s.expressions.last().is_some_and(is_narrowing_expression),
        _ => false,
    }
}

/// `containsNarrowableReference` (binder.go:2620) — a narrowable reference, or an
/// optional-chain node whose object/callee contains one.
fn contains_narrowable_reference(expr: &Expression<'_>) -> bool {
    if is_narrowable_reference(expr) {
        return true;
    }
    match expr {
        Expression::MemberExpression(m) if expr.has_optional_in_chain() => {
            contains_narrowable_reference(m.object)
        }
        Expression::CallExpression(c) if expr.has_optional_in_chain() => {
            contains_narrowable_reference(c.callee)
        }
        Expression::TSNonNullExpression(n) if expr.has_optional_in_chain() => {
            contains_narrowable_reference(n.expression)
        }
        _ => false,
    }
}

/// `isNarrowingBinaryExpression` (binder.go:2666) for tsv's `BinaryExpression`
/// (which never carries the comma / assignment operators — those are separate
/// nodes, handled in `is_narrowing_expression`).
fn is_narrowing_binary_expression(b: &BinaryExpression<'_>) -> bool {
    use BinaryOperator as Op;
    match b.operator {
        Op::EqualsEquals | Op::BangEquals | Op::EqualsEqualsEquals | Op::BangEqualsEquals => {
            let left = skip_parens(b.left);
            let right = skip_parens(b.right);
            is_narrowable_operand(left)
                || is_narrowable_operand(right)
                || is_narrowing_typeof_operands(right, left)
                || is_narrowing_typeof_operands(left, right)
                || (is_boolean_literal(right) && is_narrowing_expression(left))
                || (is_boolean_literal(left) && is_narrowing_expression(right))
        }
        Op::Instanceof => is_narrowable_operand(b.left),
        Op::In => is_narrowing_expression(b.right),
        _ => false,
    }
}

/// `isNarrowableOperand` (binder.go:2686).
fn is_narrowable_operand(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ParenthesizedExpression(p) => is_narrowable_operand(p.expression),
        Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
            is_narrowable_operand(a.left)
        }
        Expression::SequenceExpression(s) => {
            s.expressions.last().is_some_and(is_narrowable_operand)
        }
        _ => contains_narrowable_reference(expr),
    }
}

/// `isNarrowingTypeOfOperands` (binder.go:2702) — `typeof <operand> === <string>`.
fn is_narrowing_typeof_operands(expr1: &Expression<'_>, expr2: &Expression<'_>) -> bool {
    matches!(expr1, Expression::UnaryExpression(u)
        if u.operator == UnaryOperator::Typeof && is_narrowable_operand(u.argument))
        && is_string_literal_like(expr2)
}

/// `IsStringLiteralLike` — a string literal or a no-substitution template.
fn is_string_literal_like(e: &Expression<'_>) -> bool {
    match e {
        Expression::Literal(l) => matches!(l.value, LiteralValue::String(_)),
        Expression::TemplateLiteral(t) => t.expressions.is_empty(),
        _ => false,
    }
}

/// `IsBooleanLiteral` — a `true` / `false` keyword literal.
fn is_boolean_literal(e: &Expression<'_>) -> bool {
    matches!(e, Expression::Literal(l) if matches!(l.value, LiteralValue::Boolean(_)))
}

/// `SkipParentheses` — strip grouping `ParenthesizedExpression` wrappers (rare in
/// tsv, which discards grouping parens except under `preserve_parens`).
fn skip_parens<'a, 'arena>(e: &'a Expression<'arena>) -> &'a Expression<'arena> {
    let mut e = e;
    while let Expression::ParenthesizedExpression(p) = e {
        e = p.expression;
    }
    e
}
