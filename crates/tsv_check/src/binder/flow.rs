//! The flow-graph walk — a per-file control-flow graph in struct-of-arrays form.
//!
//! This is the **third walk** of the binder (after the SoA node-identity walk
//! and the symbol bind). It ports tsgo's binder flow construction (`bind` /
//! `bindContainer` / `bindChildren` + the per-statement flow shapers) onto the
//! tsv AST, resolving each attachment's [`NodeId`] through the F0 address map's
//! **strict** [`BoundFile::require_node_id`] (a miss aborts — a flow graph must
//! never silently splice onto the wrong node).
//!
//! **F1a scope: the LINEAR + unreachable core.** Real branching topology
//! (if / loops / conditions / switch / try / labeled / break / continue) is
//! **F1b** — those constructs are handled here by a linear placeholder that
//! threads `current_flow` through their children (marked `// F1b: real
//! topology`), so every contained node still gets a flow attachment and the
//! walk never panics. What *is* real here: the flow substrate + constructors,
//! container save/restore (fresh `Start`, constructor/static-block return
//! targets), linear statement threading, the assertion-`Call` and
//! variable-`Assignment` mutations, and `return`/`throw` unreachable
//! propagation.
//!
//! **Deliberate scoping deviations (F1a; documented for F1b):**
//! - **Types are not descended.** The walk visits value positions only; pure
//!   type nodes (annotations, type arguments, type-parameter constraints,
//!   heritage type args, interface/type-alias bodies, enum bodies) are skipped.
//!   tsgo stamps `currentFlow` on every identifier *including* type positions
//!   (binder.go:602). For **pure** type positions those stamps are inert (the
//!   checker runs no CFA there — the same soundness that lets lib files skip
//!   flow). The **exception is `typeof` queries**: `typeof x` / `typeof x.y` in a
//!   type position *is* flow-narrowed by the checker, which is exactly why tsgo
//!   gates the `QualifiedName` stamp on `IsPartOfTypeQuery` (binder.go:611). So
//!   the omitted type-position identifier stamp (for `typeof x`) and the
//!   `QualifiedName`-inside-`typeof` stamp are **not** dead weight — they are a
//!   **P3 prerequisite** for typeof-query narrowing (ledgered as such), not
//!   inert. Nothing before P3 reads them, so deferring is safe now.
//! - **No `Start` region for the bodiless signature/type function-likes**
//!   (`TSFunctionType` / `TSConstructorType` / method-/call-/construct-signature)
//!   — a corollary of not descending types.
//! - **Binding-element flow.** tsv has no distinct binding-element node (patterns
//!   are pattern-shaped `Expression`s), so a destructuring `let {a} = e` emits a
//!   single `Assignment` per *declarator* (subject = the declarator) rather than
//!   one per element (binder.go:2329). Exact for the identifier case; the
//!   contained identifiers still get their leaf stamps.
//! - **IIFE inlining dropped** (binder.go:1525-1528). `is_flow_transparent` is
//!   narrowed to `ClassStaticBlockDeclaration` only; ordinary
//!   function-expression IIFEs stay flow-isolated (a safe F1 approximation;
//!   true inlining is F2).
//
// tsgo: internal/binder/binder.go bind / bindContainer / bindChildren
//       (+ the newFlowNode* / createFlow* / finishFlowLabel / addAntecedent
//        constructor family and the per-statement flow shapers)

use crate::binder::{BoundFile, addr_of};
use crate::ids::{FlowNodeId, NodeId};
use smallvec::SmallVec;
use tsv_lang::Span;
use tsv_ts::ast::Program;
use tsv_ts::ast::internal::{
    ArrowFunctionBody, BinaryOperator, ClassDeclaration, ClassExpression, ClassMember, Decorator,
    Expression, ForInit, FunctionDeclaration, FunctionExpression, Identifier, LiteralValue,
    MethodDefinition, MethodKind, ObjectPatternProperty, ObjectProperty, Property, Statement,
    TSModuleDeclarationBody, VariableDeclarator,
};

// --- FlowFlags -------------------------------------------------------------

/// The flow-node flag bits — a `u16` newtype over tsgo's 13 `FlowFlags`
/// (flow.go:5-23; the max bit is `Shared`, `1 << 12`, so a `u16` fits). All 13
/// bits are defined for shape; the F2-only bits (`SwitchClause`,
/// `ArrayMutation`, `ReduceLabel`) are never *set* in F1a.
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
    /// Switch-statement clause (F2 — never set in F1a).
    pub const SWITCH_CLAUSE: FlowFlags = FlowFlags(1 << 7);
    /// Potential array mutation (F2 — never set in F1a).
    pub const ARRAY_MUTATION: FlowFlags = FlowFlags(1 << 8);
    /// Potential assertion call.
    pub const CALL: FlowFlags = FlowFlags(1 << 9);
    /// Temporarily reduce antecedents of a label (F2 — never set in F1a).
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
    fn insert(&mut self, other: FlowFlags) {
        self.0 |= other.0;
    }

    /// Whether this is a label node (`BranchLabel` or `LoopLabel`).
    #[inline]
    #[must_use]
    pub const fn is_label(self) -> bool {
        self.intersects(FlowFlags::LABEL)
    }

    /// The raw bits (for the DOT renderer's header labels).
    #[inline]
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }
}

// --- F2 payload shapes (defined for the SoA shape; not populated in F1a) ----

/// A switch-clause payload (F2). Defined for the settled [`FlowGraph`] shape;
/// populated by the switch flow builder in a later slice.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // F2: written by the switch flow builder (binder.go:2087-2108)
pub struct FlowSwitchClause {
    /// The switch statement node.
    pub switch: NodeId,
    /// Inclusive clause-range start index.
    pub clause_start: u32,
    /// Exclusive clause-range end index.
    pub clause_end: u32,
}

/// A reduce-label payload (F2). Defined for the settled [`FlowGraph`] shape;
/// populated by the try/finally flow builder in a later slice.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // F2: written by the try/finally flow builder (binder.go:2042-2045)
pub struct FlowReduceLabel {
    /// The target label.
    pub target: FlowNodeId,
    /// Pool-run index of the temporary antecedent list.
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
    flags: Vec<FlowFlags>,
    /// Kind-discriminated by `flags`: a `NodeId` (raw, 1-based) | payload index
    /// | 0 = none. In F1a it is always a `NodeId` or 0.
    subject: Vec<u32>,
    /// Non-label: the single antecedent's raw `FlowNodeId` (0 = none).
    /// Label: a 1-based pool-run index (0 = collapsed / unfinalized).
    antecedent: Vec<u32>,
    /// Length-prefixed antecedent runs for labels (`[len, e0, e1, …]`).
    pool: Vec<u32>,
    /// F2 — switch-clause payloads (empty in F1a; kept for shape).
    #[allow(dead_code)] // F2: switch flow
    switch_payloads: Vec<FlowSwitchClause>,
    /// F2 — reduce-label payloads (empty in F1a; kept for shape).
    #[allow(dead_code)] // F2: try/finally flow
    reduce_payloads: Vec<FlowReduceLabel>,
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
    #[inline]
    #[must_use]
    pub fn subject(&self, id: FlowNodeId) -> Option<NodeId> {
        NodeId::from_raw_opt(self.subject[id.index()])
    }

    /// The antecedents of a flow node, in append order.
    ///
    /// Non-label nodes have 0 or 1 antecedent (the single-antecedent slot);
    /// label nodes decode their length-prefixed pool run.
    #[must_use]
    pub fn antecedents(&self, id: FlowNodeId) -> Vec<FlowNodeId> {
        let flags = self.flags[id.index()];
        let slot = self.antecedent[id.index()];
        if flags.is_label() {
            if slot == 0 {
                return Vec::new();
            }
            let off = (slot - 1) as usize;
            let len = self.pool[off] as usize;
            self.pool[off + 1..off + 1 + len]
                .iter()
                .filter_map(|&raw| FlowNodeId::from_raw(raw))
                .collect()
        } else {
            FlowNodeId::from_raw(slot).into_iter().collect()
        }
    }
}

// --- FlowProduct -----------------------------------------------------------

/// Small construction counters, surfaced for the density / dead-label-row
/// perf report (they are not consumed by any checker phase).
#[derive(Clone, Copy, Debug, Default)]
pub struct FlowStats {
    /// Branch labels created (`createBranchLabel`).
    pub branch_labels: u32,
    /// Branch labels that collapsed at `finishFlowLabel` (0 or 1 antecedent),
    /// leaving a dead row — the fraction to watch (INTERNALS §Flow graph).
    pub dead_labels: u32,
}

/// The owned, arena-free, file-local flow product carried **dark** in a
/// `BoundUnit` (nothing consumes it until F3; F1a builds it and `--dump-flow`
/// renders it). C15-relocatable by construction.
pub struct FlowProduct {
    /// The flow graph.
    pub graph: FlowGraph,
    /// Per-`NodeId` flow attachment (`None` where tsgo attaches nil — including
    /// non-leaf nodes cleared in dead code; a dead *leaf* keeps
    /// `Some(unreachable)`).
    pub flow_of_node: Vec<Option<FlowNodeId>>,
    /// The F0 `node_flags` column with the `Unreachable` bit set during the
    /// dead-code walk (`NODE_FLAGS_UNREACHABLE`).
    pub node_flags: Vec<u8>,
    /// Function-body + `SourceFile` end-of-flow anchors (binder.go:1561,1569),
    /// sorted by `NodeId`.
    pub end_flow: Vec<(NodeId, FlowNodeId)>,
    /// Constructor + class-static-block return-flow anchors ONLY
    /// (binder.go:1575), sorted by `NodeId`. Every other tsgo `ReturnFlowNode`
    /// write/read is dead plumbing and is not ported.
    pub return_flow: Vec<(NodeId, FlowNodeId)>,
    /// F2 — case-clause fallthrough anchors (empty in F1a; kept for shape).
    pub fallthrough_flow: Vec<(NodeId, FlowNodeId)>,
    /// Construction counters.
    pub stats: FlowStats,
}

impl FlowProduct {
    /// The `end_flow` anchor for a node, if any (small sorted anchor list).
    #[must_use]
    pub fn end_flow_of(&self, node: NodeId) -> Option<FlowNodeId> {
        self.end_flow
            .binary_search_by_key(&node, |&(n, _)| n)
            .ok()
            .map(|i| self.end_flow[i].1)
    }

    /// The `return_flow` anchor for a node, if any (constructor / static block).
    #[must_use]
    pub fn return_flow_of(&self, node: NodeId) -> Option<FlowNodeId> {
        self.return_flow
            .binary_search_by_key(&node, |&(n, _)| n)
            .ok()
            .map(|i| self.return_flow[i].1)
    }
}

/// Build the flow product for one parsed file, from its `Program` and the F0
/// [`BoundFile`] (the node-identity source). Invoked from `bind_program`'s
/// per-unit loop for parsed non-lib units (lib files skip flow construction —
/// no consumer reads lib flow and ambient files have no executable code).
#[must_use]
pub fn build_flow(program: &Program<'_>, _source: &str, bound: &BoundFile) -> FlowProduct {
    let mut b = FlowBuilder::new(bound);
    b.run(program);
    b.finish()
}

// --- FlowBuilder -----------------------------------------------------------

/// Saved control-flow state restored at a flow-container boundary
/// (binder.go:1517-1524, the F1 subset — the deferred break/continue/label
/// targets are always `None` in F1a and land with F1b's branching).
struct SavedFlow {
    current_flow: FlowNodeId,
    current_return_target: Option<FlowNodeId>,
    current_exception_target: Option<FlowNodeId>,
}

/// The flow-graph construction walk.
struct FlowBuilder<'a> {
    bound: &'a BoundFile,

    // graph columns
    flags: Vec<FlowFlags>,
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

    // construction state (the F1 subset of the container-boundary set)
    current_flow: FlowNodeId,
    unreachable_flow: FlowNodeId,
    current_return_target: Option<FlowNodeId>,
    /// Always `None` in F1 (`createFlowMutation` reads it; only try/finally sets
    /// it, which is F2), but ported so the exception hook is faithful.
    current_exception_target: Option<FlowNodeId>,

    // stats
    branch_labels: u32,
    dead_labels: u32,
}

impl<'a> FlowBuilder<'a> {
    fn new(bound: &'a BoundFile) -> FlowBuilder<'a> {
        let n = bound.node_count as usize;
        let mut b = FlowBuilder {
            bound,
            flags: Vec::new(),
            subject: Vec::new(),
            antecedent: Vec::new(),
            pool: Vec::new(),
            label_scratch: crate::hash::FxHashMap::default(),
            flow_of_node: vec![None; n],
            node_flags: bound.node_flags.clone(),
            end_flow: Vec::new(),
            return_flow: Vec::new(),
            current_flow: FlowNodeId::UNREACHABLE,
            unreachable_flow: FlowNodeId::UNREACHABLE,
            current_return_target: None,
            current_exception_target: None,
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

    fn finish(self) -> FlowProduct {
        let mut end_flow = self.end_flow;
        let mut return_flow = self.return_flow;
        end_flow.sort_unstable_by_key(|&(n, _)| n);
        return_flow.sort_unstable_by_key(|&(n, _)| n);
        FlowProduct {
            graph: FlowGraph {
                flags: self.flags,
                subject: self.subject,
                antecedent: self.antecedent,
                pool: self.pool,
                switch_payloads: Vec::new(),
                reduce_payloads: Vec::new(),
            },
            flow_of_node: self.flow_of_node,
            node_flags: self.node_flags,
            end_flow,
            return_flow,
            fallthrough_flow: Vec::new(),
            stats: FlowStats {
                branch_labels: self.branch_labels,
                dead_labels: self.dead_labels,
            },
        }
    }

    // --- flow node constructors (binder.go:454-575) -----------------------

    /// `newFlowNode` (binder.go:454) — a bare node with only flags.
    fn new_flow_node(&mut self, flags: FlowFlags) -> FlowNodeId {
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
    fn create_branch_label(&mut self) -> FlowNodeId {
        self.branch_labels += 1;
        self.new_flow_node(FlowFlags::BRANCH_LABEL)
    }

    /// `createLoopLabel` (binder.go:467). F1b (loops) exercises this.
    #[allow(dead_code)] // F1b: loop constructs
    fn create_loop_label(&mut self) -> FlowNodeId {
        self.new_flow_node(FlowFlags::LOOP_LABEL)
    }

    /// `createFlowMutation` (binder.go:499). The `currentExceptionTarget` hook
    /// is a no-op in F1 (that field is always `None`; try/finally sets it, F2).
    // F1b: tsgo also sets `hasFlowEffects = true` here (binder.go:501) — F1b's
    // condition/logical/for binding reads it to decide whether a post-expression
    // label materializes, so F1b must reintroduce it (omitting it over-produces
    // flow nodes: a sound superset, but diverges from tsgo's shape).
    fn create_flow_mutation(
        &mut self,
        flags: FlowFlags,
        antecedent: FlowNodeId,
        node: NodeId,
    ) -> FlowNodeId {
        self.set_flow_node_referenced(antecedent);
        let result = self.new_flow_node_ex(flags, Some(node), antecedent);
        if let Some(target) = self.current_exception_target {
            self.add_antecedent(target, result);
        }
        result
    }

    /// `createFlowCall` (binder.go:514). F1b: also sets `hasFlowEffects = true`
    /// (binder.go:516) — see `create_flow_mutation`.
    fn create_flow_call(&mut self, antecedent: FlowNodeId, node: NodeId) -> FlowNodeId {
        self.set_flow_node_referenced(antecedent);
        self.new_flow_node_ex(FlowFlags::CALL, Some(node), antecedent)
    }

    /// `createFlowCondition` (binder.go:479) — ported for F1b's condition
    /// binding. The `expression.Parent` guards (optional-chain root / nullish
    /// coalesce) are supplied by the caller, which has the parent context tsv's
    /// AST does not carry on an `Expression`; `is_narrowing` is the caller's
    /// `is_narrowing_expression` verdict (F1b). Unexercised by F1a's linear
    /// walk (conditions are F1b).
    #[allow(dead_code)] // F1b: condition binding (bindCondition)
    fn create_flow_condition(
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
    fn add_antecedent(&mut self, label: FlowNodeId, antecedent: FlowNodeId) {
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
    fn finish_flow_label(&mut self, label: FlowNodeId) -> FlowNodeId {
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
    fn require(&self, address: usize) -> NodeId {
        self.bound.require_node_id(address)
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
    }

    // --- entry (SourceFile container) -------------------------------------

    fn run(&mut self, program: &Program<'_>) {
        // The SourceFile is a control-flow container: fresh Start (id 2), no
        // return target (not an IIFE/constructor), no Start subject.
        let root = self.require(addr_of(program));
        let start = self.new_flow_node(FlowFlags::START);
        self.current_flow = start;
        self.current_return_target = None;
        self.current_exception_target = None;
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
        let id = self.require(addr_of(stmt));
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
                if let Some(a) = &s.argument {
                    self.visit_expression(a);
                }
                if let Some(rt) = self.current_return_target {
                    self.add_antecedent(rt, self.current_flow);
                }
                self.current_flow = self.unreachable_flow;
            }
            Statement::ThrowStatement(s) => {
                self.visit_expression(&s.argument);
                self.current_flow = self.unreachable_flow;
            }
            // Everything else (declarations, blocks, and the F1b branching
            // placeholders) threads flow linearly through its children.
            _ => self.descend_children_generic(stmt),
        }
    }

    /// Descend a statement's value children threading `current_flow` linearly,
    /// with **no** flow shaping — the `bindEachChild` analog. Shared by the
    /// dead-code path and the F1b branching placeholders (which build no real
    /// topology yet). Containers nested here still open their own `Start`
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
                let id = self.require(addr_of(stmt));
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
            // --- F1b: real topology (branching) — linear placeholder descent -
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
                self.visit_statement(s.body); // F1b: real topology
            }
            Statement::ForInStatement(s) => {
                self.visit_for_left(&s.left);
                self.visit_expression(&s.right);
                self.visit_statement(s.body); // F1b: real topology
            }
            Statement::ForOfStatement(s) => {
                self.visit_for_left(&s.left);
                self.visit_expression(&s.right);
                self.visit_statement(s.body); // F1b: real topology
            }
            Statement::WhileStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.body); // F1b: real topology
            }
            Statement::DoWhileStatement(s) => {
                self.visit_statement(s.body); // F1b: real topology
                self.visit_expression(&s.test);
            }
            Statement::SwitchStatement(s) => {
                self.visit_expression(&s.discriminant);
                for case in s.cases {
                    if let Some(t) = &case.test {
                        self.visit_expression(t);
                    }
                    self.visit_statement_list(case.consequent); // F1b: real topology
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
                self.visit_identifier(&s.label); // F1b: real label topology
                self.visit_statement(s.body);
            }
            Statement::BreakStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label); // F1b: real break/continue topology
                }
            }
            Statement::ContinueStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label); // F1b: real break/continue topology
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

    fn visit_for_left(&mut self, left: &tsv_ts::ast::internal::ForInOfLeft<'_>) {
        use tsv_ts::ast::internal::ForInOfLeft as L;
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
            let call_id = self.require(addr_of(c));
            self.current_flow = self.create_flow_call(self.current_flow, call_id);
        }
    }

    /// `bindVariableDeclarationFlow` + `bindInitializedVariableFlow`
    /// (binder.go:2314) — a `var/let/const x = e` with an initializer emits one
    /// unconditional `Assignment` (no default-value fork; that is F2). A
    /// destructuring pattern emits one `Assignment` per declarator (tsv has no
    /// binding-element node — see the module scope note).
    fn bind_variable_declaration_flow(&mut self, decl: &VariableDeclarator<'_>) {
        self.visit_expression(&decl.id);
        if let Some(init) = &decl.init {
            self.visit_expression(init);
        }
        if decl.init.is_some() {
            let decl_id = self.require(addr_of(decl));
            self.current_flow =
                self.create_flow_mutation(FlowFlags::ASSIGNMENT, self.current_flow, decl_id);
        }
    }

    // --- containers -------------------------------------------------------

    fn visit_function_declaration(&mut self, f: &FunctionDeclaration<'_>, anchor: NodeId) {
        let saved = self.enter_container(None, false, false);
        self.bind_params(f.params);
        self.visit_statement_list(f.body.body);
        self.exit_container(saved, false, true, true, anchor, false);
    }

    fn visit_function_expression(&mut self, f: &FunctionExpression<'_>, node_id: NodeId) {
        // The function-expression flow write is captured at the OUTER flow,
        // before the body's Start (binder.go:915). Unconditional: the container
        // path does not nil it in dead code.
        self.set_flow_leaf(node_id);
        let saved = self.enter_container(Some(node_id), false, false);
        self.bind_params(f.params);
        self.visit_statement_list(f.body.body);
        self.exit_container(saved, false, true, true, node_id, false);
    }

    fn visit_arrow(
        &mut self,
        a: &tsv_ts::ast::internal::ArrowFunctionExpression<'_>,
        node_id: NodeId,
    ) {
        self.set_flow_leaf(node_id); // binder.go:915 (arrows dispatch here too)
        let saved = self.enter_container(Some(node_id), false, false);
        self.bind_params(a.params);
        match &a.body {
            ArrowFunctionBody::Expression(e) => self.visit_expression(e),
            ArrowFunctionBody::BlockStatement(block) => self.visit_statement_list(block.body),
        }
        self.exit_container(saved, false, true, true, node_id, false);
    }

    fn bind_params(&mut self, params: &[Expression<'_>]) {
        for param in params {
            self.visit_expression(param);
        }
    }

    fn visit_class_decl(&mut self, c: &ClassDeclaration<'_>) {
        if let Some(name) = &c.id {
            self.visit_identifier(name);
        }
        self.visit_decorators(c.decorators);
        if let Some(sc) = c.super_class {
            self.visit_expression(sc);
        }
        // type params / super type args / implements are type positions (skip).
        for member in c.body.body {
            self.visit_class_member(member);
        }
    }

    fn visit_class_expr(&mut self, c: &ClassExpression<'_>) {
        if let Some(name) = &c.id {
            self.visit_identifier(name);
        }
        self.visit_decorators(c.decorators);
        if let Some(sc) = c.super_class {
            self.visit_expression(sc);
        }
        for member in c.body.body {
            self.visit_class_member(member);
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember<'_>) {
        match member {
            ClassMember::MethodDefinition(m) => self.visit_method(m),
            ClassMember::PropertyDefinition(p) => {
                self.visit_decorators(p.decorators);
                self.visit_expression(&p.key);
                // property type annotation is a type position (skip).
                if let Some(value) = &p.value {
                    // A property-with-initializer is a control-flow container
                    // (binder.go:2584): fresh Start around the initializer.
                    let p_id = self.require(addr_of(p));
                    let saved = self.enter_container(None, false, false);
                    self.visit_expression(value);
                    self.exit_container(saved, false, false, false, p_id, false);
                }
            }
            ClassMember::StaticBlock(s) => {
                // A class static block is flow-transparent (binder.go:1525-1528)
                // with its own return target; `return_flow` anchors on it.
                let s_id = self.require(addr_of(s));
                let saved = self.enter_container(None, true, true);
                self.visit_statement_list(s.body);
                self.exit_container(saved, true, true, true, s_id, true);
            }
            // index signatures are type-only (skip).
            ClassMember::IndexSignature(_) => {}
        }
    }

    fn visit_method(&mut self, m: &MethodDefinition<'_>) {
        self.visit_decorators(m.decorators);
        let is_ctor = m.kind == MethodKind::Constructor;
        self.visit_expression(&m.key);
        // The method body lives in `value` (a FunctionExpression); the method is
        // a control-flow container anchored on that FunctionExpression. tsv wraps
        // a method body in a FunctionExpression (tsc's method node holds the body
        // directly), and — F0 hazard — the address map collides the
        // MethodDefinition with its inline `value` (a repr reorder puts `value`
        // at offset 0, so F0's later insert overwrites the map slot). So the
        // value FunctionExpression is the reliably-addressable body-bearing node;
        // anchor there. The obj-literal/class-expression method flow-write +
        // Start.Node subject (binder.go:982, 1534) is a P3 narrowing hint,
        // deferred to F1b.
        let anchor = self.require(addr_of(&m.value));
        let saved = self.enter_container(None, false, is_ctor);
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
                let block_id = self.require(addr_of(block));
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
                let id = self.require(addr_of(f));
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
        match expr {
            E::Identifier(idn) => self.visit_identifier(idn),
            E::ThisExpression(t) => {
                let id = self.require(addr_of(t));
                self.set_flow_leaf(id);
            }
            E::Super(s) => {
                let id = self.require(addr_of(s));
                self.set_flow_leaf(id);
            }
            E::MetaProperty(m) => {
                // Non-leaf write (nil'd in dead code). tsv models `import`/`new`
                // and `meta`/`target` as identifiers; they are keyword-ish, not
                // references, so only the MetaProperty node is stamped.
                let id = self.require(addr_of(m));
                self.set_flow_nonleaf(id);
            }
            E::MemberExpression(m) => {
                // The access flow write (binder.go:618): non-leaf, reachable-
                // only, gated on `isNarrowableReference`.
                if is_narrowable_reference(expr) {
                    let id = self.require(addr_of(m));
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
            E::UnaryExpression(u) => self.visit_expression(u.argument),
            E::UpdateExpression(u) => self.visit_expression(u.argument),
            E::BinaryExpression(b) => {
                // F1b: logical short-circuit topology; F1a threads linearly.
                self.visit_expression(b.left);
                self.visit_expression(b.right);
            }
            E::CallExpression(c) => {
                self.visit_expression(c.callee);
                for a in c.arguments {
                    self.visit_expression(a);
                }
            }
            E::NewExpression(n) => {
                self.visit_expression(n.callee);
                for a in n.arguments {
                    self.visit_expression(a);
                }
            }
            E::ConditionalExpression(c) => {
                // F1b: conditional branch topology; F1a threads linearly.
                self.visit_expression(c.test);
                self.visit_expression(c.consequent);
                self.visit_expression(c.alternate);
            }
            E::ArrowFunctionExpression(a) => {
                let id = self.require(addr_of(a));
                self.visit_arrow(a, id);
            }
            E::FunctionExpression(f) => {
                let id = self.require(addr_of(f));
                self.visit_function_expression(f, id);
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
                for e in s.expressions {
                    self.visit_expression(e);
                }
            }
            E::AssignmentExpression(a) => {
                // F1b: assignment createFlowMutation; F1a descends only.
                self.visit_expression(a.left);
                self.visit_expression(a.right);
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
    }

    fn visit_identifier(&mut self, ident: &Identifier<'_>) {
        // Identifier flow write (binder.go:602): a leaf — unconditional, so a
        // dead identifier keeps `Some(unreachable)`. Its decorators (parameter
        // decorators) are value expressions; its type annotation is a type
        // position (skipped).
        let id = self.require(addr_of(ident));
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
            // collision workaround). The obj-literal method flow-write
            // (binder.go:982) is a P3 narrowing hint, deferred.
            self.visit_expression(&pr.key);
            let anchor = self.require(addr_of(f));
            let saved = self.enter_container(None, false, false);
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
fn is_narrowable_reference(node: &Expression<'_>) -> bool {
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

/// Whether a binary expression is a nullish-coalescing (`??`) — the
/// `IsNullishCoalesce` guard's operator test (utilities.go:387). Used by F1b's
/// condition binding to compute the `create_flow_condition` parent guard.
#[allow(dead_code)] // F1b: condition binding
fn is_nullish_coalesce(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::QuestionQuestion
    )
}

// --- DOT renderer (formatControlFlowGraph reference) -----------------------

/// Render one unit's flow graph to Graphviz DOT — the `--dump-flow` product.
/// Backward DFS from the `SourceFile`/function end-of-flow anchors (and return
/// anchors) with cycle detection, after Strada's `formatControlFlowGraph`
/// (flag→header label, subject-node source text, backward edges). `node_spans`
/// is the F0 `BoundFile::spans` column (subject text = `source[span]`).
#[must_use]
pub fn render_flow_dot(product: &FlowProduct, node_spans: &[Span], source: &str) -> String {
    use std::fmt::Write as _;
    let g = &product.graph;
    let mut out = String::new();
    out.push_str("digraph flow {\n");
    out.push_str("  rankdir=BT;\n");
    out.push_str("  node [shape=box, fontname=\"monospace\"];\n");

    let mut seen = vec![false; g.node_count() as usize + 1];
    let mut stack: Vec<FlowNodeId> = Vec::new();
    // Roots: every end_flow / return_flow anchor (the exits), plus id 1 so a
    // fully-unreachable graph still renders the singleton.
    for &(_, f) in product.end_flow.iter().chain(product.return_flow.iter()) {
        stack.push(f);
    }
    stack.push(FlowNodeId::UNREACHABLE);

    while let Some(id) = stack.pop() {
        if seen[id.index() + 1] {
            continue;
        }
        seen[id.index() + 1] = true;
        let label = flow_node_label(g, id, node_spans, source);
        let _ = writeln!(out, "  N{} [label=\"{}\"];", id.get(), escape_dot(&label));
        for ante in g.antecedents(id) {
            let _ = writeln!(out, "  N{} -> N{};", id.get(), ante.get());
            stack.push(ante); // cycle-guarded by `seen`
        }
    }

    // Anchor edges (dashed) so the exits are visible.
    for (node, f) in &product.end_flow {
        let _ = writeln!(
            out,
            "  END_{n} [shape=doublecircle, label=\"end#{n}\"];\n  END_{n} -> N{f} [style=dashed];",
            n = node.get(),
            f = f.get()
        );
    }
    out.push_str("}\n");
    out
}

fn flow_node_label(g: &FlowGraph, id: FlowNodeId, node_spans: &[Span], source: &str) -> String {
    let flags = g.flags(id);
    let header = flow_flag_header(flags);
    if let Some(node) = g.subject(id) {
        let span = node_spans[node.index()];
        let text = span.extract(source);
        let text = text.split('\n').next().unwrap_or(text);
        // Truncate on a char boundary (byte-slicing `&text[..32]` panics when a
        // multibyte char straddles byte 32).
        let text = match text.char_indices().nth(32) {
            Some((idx, _)) => &text[..idx],
            None => text,
        };
        format!("#{} {}: {}", id.get(), header, text)
    } else {
        format!("#{} {}", id.get(), header)
    }
}

/// The most salient flag as a short header label (label/condition/start/…).
fn flow_flag_header(flags: FlowFlags) -> &'static str {
    if flags.contains(FlowFlags::UNREACHABLE) {
        "unreachable"
    } else if flags.contains(FlowFlags::START) {
        "start"
    } else if flags.contains(FlowFlags::LOOP_LABEL) {
        "loop"
    } else if flags.contains(FlowFlags::BRANCH_LABEL) {
        "branch"
    } else if flags.contains(FlowFlags::ASSIGNMENT) {
        "assign"
    } else if flags.contains(FlowFlags::TRUE_CONDITION) {
        "true"
    } else if flags.contains(FlowFlags::FALSE_CONDITION) {
        "false"
    } else if flags.contains(FlowFlags::CALL) {
        "call"
    } else {
        "flow"
    }
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binder::{BoundFile, NodeKind, bind_file};
    use crate::ids::FileId;
    use bumpalo::Bump;

    /// Bind + build the flow product for a snippet (a fresh arena per call).
    fn flow_of(source: &str) -> (Bump, BoundFile) {
        let arena = Bump::new();
        let program = tsv_ts::parse(source, &arena).expect("parse");
        let bound = bind_file(&program, source, FileId::ROOT);
        (arena, bound)
    }

    fn build(source: &str) -> FlowProduct {
        let arena = Bump::new();
        let program = tsv_ts::parse(source, &arena).expect("parse");
        let bound = bind_file(&program, source, FileId::ROOT);
        build_flow(&program, source, &bound)
    }

    fn nodes_of_kind(bound: &BoundFile, kind: NodeKind) -> Vec<NodeId> {
        bound
            .kinds
            .iter()
            .enumerate()
            .filter(|(_, k)| **k == kind)
            .map(|(i, _)| NodeId::from_index(i))
            .collect()
    }

    /// The `NodeId` of the identifier whose source text is exactly `text`.
    fn ident(bound: &BoundFile, source: &str, text: &str) -> NodeId {
        for (i, k) in bound.kinds.iter().enumerate() {
            if *k == NodeKind::Identifier && bound.spans[i].extract(source) == text {
                return NodeId::from_index(i);
            }
        }
        panic!("identifier {text:?} not found");
    }

    #[test]
    fn unreachable_flow_is_id_1() {
        let product = build("const x = 1;");
        let uid = FlowNodeId::UNREACHABLE;
        assert_eq!(uid.get(), 1);
        assert!(product.graph.flags(uid).contains(FlowFlags::UNREACHABLE));
        // The SourceFile Start is id 2 (minted right after unreachable).
        assert!(product.graph.node_count() >= 2);
    }

    #[test]
    fn linear_two_statements_thread_one_start() {
        let src = "function f() { a; b; }";
        let (_arena, bound) = flow_of(src);
        let product = {
            let arena = Bump::new();
            let program = tsv_ts::parse(src, &arena).expect("parse");
            build_flow(&program, src, &bind_file(&program, src, FileId::ROOT))
        };
        // Both expression statements capture the same entry flow (f's Start), and
        // that Start is f's end-of-flow (reachable at exit).
        let stmts = nodes_of_kind(&bound, NodeKind::ExpressionStatement);
        assert_eq!(stmts.len(), 2);
        let flow_a = product.flow_of_node[stmts[0].index()].expect("a entry flow");
        let flow_b = product.flow_of_node[stmts[1].index()].expect("b entry flow");
        assert_eq!(flow_a, flow_b);
        assert!(product.graph.flags(flow_a).contains(FlowFlags::START));

        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        assert_eq!(product.end_flow_of(f), Some(flow_a));
    }

    #[test]
    fn linear_var_init_and_dotted_call() {
        let product = build("function f() { let x = 1; g(); }");
        // One Assignment mutation (`x = 1`) and one Call (`g()`).
        let has_assignment = (1..=product.graph.node_count())
            .filter_map(FlowNodeId::from_raw)
            .any(|id| product.graph.flags(id).contains(FlowFlags::ASSIGNMENT));
        let has_call = (1..=product.graph.node_count())
            .filter_map(FlowNodeId::from_raw)
            .any(|id| product.graph.flags(id).contains(FlowFlags::CALL));
        assert!(
            has_assignment,
            "expected a createFlowMutation(Assignment) node"
        );
        assert!(has_call, "expected a createFlowCall node");
    }

    #[test]
    fn unreachable_after_return_propagates() {
        let src = "function f() { return; a; }";
        let (_arena, bound) = flow_of(src);
        let product = {
            let arena = Bump::new();
            let program = tsv_ts::parse(src, &arena).expect("parse");
            build_flow(&program, src, &bind_file(&program, src, FileId::ROOT))
        };

        // The ReturnStatement's entry flow is f's Start.
        let ret = nodes_of_kind(&bound, NodeKind::ReturnStatement)[0];
        let ret_flow = product.flow_of_node[ret.index()].expect("return entry flow");
        assert!(product.graph.flags(ret_flow).contains(FlowFlags::START));

        // The dead `a;` ExpressionStatement: flow nil (None) + Unreachable bit.
        let a_stmt = nodes_of_kind(&bound, NodeKind::ExpressionStatement)[0];
        assert_eq!(product.flow_of_node[a_stmt.index()], None);
        assert_ne!(
            product.node_flags[a_stmt.index()] & crate::binder::NODE_FLAGS_UNREACHABLE,
            0
        );

        // The dead leaf identifier `a` keeps Some(unreachable = id 1).
        let a_id = ident(&bound, src, "a");
        assert_eq!(
            product.flow_of_node[a_id.index()],
            Some(FlowNodeId::UNREACHABLE)
        );

        // f gets NO end_flow (its exit is unreachable). The only end_flow is the
        // SourceFile root.
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        assert_eq!(product.end_flow_of(f), None);
        assert_eq!(product.end_flow.len(), 1); // SourceFile only
    }

    #[test]
    fn constructor_gets_a_return_flow_anchor() {
        let src = "class C { constructor() { return; } }";
        let (_arena, bound) = flow_of(src);
        let product = {
            let arena = Bump::new();
            let program = tsv_ts::parse(src, &arena).expect("parse");
            build_flow(&program, src, &bind_file(&program, src, FileId::ROOT))
        };
        // The constructor container carries exactly one return_flow anchor (keyed
        // on the value FunctionExpression — the reliably-addressable body-bearing
        // node; see the F0-collision note in `visit_method`). Its single-
        // antecedent return label collapsed to the `return`'s Start (a dead row).
        assert_eq!(product.return_flow.len(), 1);
        let rf = product.return_flow[0].1;
        assert!(product.graph.flags(rf).contains(FlowFlags::START));
        // The anchor is a FunctionExpression node (the method body).
        let anchor_node = product.return_flow[0].0;
        assert_eq!(
            bound.kinds[anchor_node.index()],
            NodeKind::FunctionExpression
        );
        assert!(product.stats.branch_labels >= 1);
        assert!(product.stats.dead_labels >= 1);
    }

    #[test]
    fn finish_flow_label_pool_run_preserves_order_and_dedups() {
        let src = "const x = 1;";
        let arena = Bump::new();
        let program = tsv_ts::parse(src, &arena).expect("parse");
        let bound = bind_file(&program, src, FileId::ROOT);
        let mut b = FlowBuilder::new(&bound);
        let a1 = b.new_flow_node(FlowFlags::START);
        let a2 = b.new_flow_node(FlowFlags::ASSIGNMENT);
        let label = b.create_branch_label();
        b.add_antecedent(label, a1);
        b.add_antecedent(label, a2);
        b.add_antecedent(label, a1); // id-equality dedup: ignored
        let finished = b.finish_flow_label(label);
        assert_eq!(finished, label); // 2+ antecedents → the label survives
        let product = b.finish();
        // Entry edge first, order preserved, no duplicate.
        assert_eq!(product.graph.antecedents(label), vec![a1, a2]);
        // Both antecedents were referenced; a1 twice would be Shared, but the dup
        // was a no-op, so a1 is Referenced-once here.
        assert!(product.graph.flags(a1).contains(FlowFlags::REFERENCED));
    }

    #[test]
    fn create_flow_condition_ports_verbatim() {
        let src = "true; false; y;";
        let arena = Bump::new();
        let program = tsv_ts::parse(src, &arena).expect("parse");
        let bound = bind_file(&program, src, FileId::ROOT);

        // Extract the top-level expressions + their node ids.
        let expr_at = |i: usize| -> (&Expression<'_>, NodeId) {
            let Statement::ExpressionStatement(s) = &program.body[i] else {
                panic!("expression statement");
            };
            let id = match &s.expression {
                Expression::Literal(l) => bound.require_node_id(addr_of(l)),
                Expression::Identifier(idn) => bound.require_node_id(addr_of(idn)),
                _ => panic!("unexpected expression"),
            };
            (&s.expression, id)
        };
        let true_lit = expr_at(0);
        let false_lit = expr_at(1);
        let y = expr_at(2);

        let mut b = FlowBuilder::new(&bound);
        let ante = b.new_flow_node(FlowFlags::START);

        // nil-expr True → passthrough; nil-expr False → unreachable.
        assert_eq!(
            b.create_flow_condition(FlowFlags::TRUE_CONDITION, ante, None, false, false, false),
            ante
        );
        assert_eq!(
            b.create_flow_condition(FlowFlags::FALSE_CONDITION, ante, None, false, false, false),
            b.unreachable_flow
        );

        // literal `true` under a FalseCondition (not in an optional-chain /
        // nullish context) short-circuits to unreachable; `false` under a
        // TrueCondition likewise.
        assert_eq!(
            b.create_flow_condition(
                FlowFlags::FALSE_CONDITION,
                ante,
                Some(true_lit),
                false,
                false,
                false
            ),
            b.unreachable_flow
        );
        assert_eq!(
            b.create_flow_condition(
                FlowFlags::TRUE_CONDITION,
                ante,
                Some(false_lit),
                false,
                false,
                false
            ),
            b.unreachable_flow
        );

        // A non-narrowing expression leaves the antecedent unchanged.
        assert_eq!(
            b.create_flow_condition(
                FlowFlags::TRUE_CONDITION,
                ante,
                Some(y),
                false,
                false,
                false
            ),
            ante
        );

        // A narrowing expression mints a new condition node carrying the flag.
        let cond =
            b.create_flow_condition(FlowFlags::TRUE_CONDITION, ante, Some(y), true, false, false);
        assert_ne!(cond, ante);
        assert!(b.flags[cond.index()].contains(FlowFlags::TRUE_CONDITION));
    }

    #[test]
    fn is_narrowable_reference_matches_tsgo_shape() {
        // Sanity for the live access-gate helper.
        let arena = Bump::new();
        let src = "a.b; a[0]; a?.b;";
        let program = tsv_ts::parse(src, &arena).expect("parse");
        for stmt in program.body {
            if let Statement::ExpressionStatement(s) = stmt {
                assert!(
                    is_narrowable_reference(&s.expression),
                    "member/element access should be narrowable"
                );
            }
        }
    }
}
