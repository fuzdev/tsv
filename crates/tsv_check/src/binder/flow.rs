//! The flow-graph walk — a per-file control-flow graph in struct-of-arrays form.
//!
//! This is the **third walk** of the binder (after the SoA node-identity walk
//! and the symbol bind). It ports tsgo's binder flow construction (`bind` /
//! `bindContainer` / `bindChildren` + the per-statement flow shapers) onto the
//! tsv AST, resolving each attachment's [`NodeId`] through the F0 address map's
//! **strict** [`BoundFile::require_node_id`] (a miss aborts — a flow graph must
//! never silently splice onto the wrong node).
//!
//! **F1b scope: the branching control-flow constructs.** On top of F1a's linear
//! substrate this slice builds faithful topology for **conditions** (the
//! `bindCondition` machinery — `&&`/`||`/`??`/`?:`/`!`/parenthesized + the
//! `hasFlowEffects` save/restore family), **`if`/`else`**, the five loops
//! (**`while`**, **`do…while`**, **`for`**, **`for-in`**, **`for-of`**), and
//! **unlabeled `break`/`continue`**.
//!
//! **F2a scope: switch-statement flow topology.** On top of F1b's local
//! post-switch break target this slice builds the real clause topology
//! (`bindSwitchStatement` / `bindCaseBlock` / `bindCaseOrDefaultClause`): every
//! clause's `preCase` label is fed **from the switch head unconditionally**
//! (`preSwitchCaseFlow`) in addition to the prior clause's fallthrough edge, so a
//! clause reached only after a prior `break`/`return` stays reachable — F1b's
//! linear stub wrongly marked it `Unreachable`. A narrowing switch
//! (`switch (true)` or a narrowing discriminant) additionally mints a
//! `SwitchClause` flow node per clause carrying the matched half-open
//! `[start, end)` clause range, and a switch with no `default` clause adds a
//! `(0, 0)` "no clause matched" `SwitchClause` exhaustiveness sentinel to the
//! post-switch label. Post-exhaustive-switch reachability (code after an
//! exhaustive no-`default` switch) is type-dependent
//! (`isExhaustiveSwitchStatement`) and stays deferred.
//!
//! **F2b scope: the four remaining flow landmines.** On top of F2a this slice
//! builds **try/catch/finally** topology (`bindTryStatement` — the
//! exception/return/normal-exit labels, the catch-as-second-try re-point, and
//! the `ReduceLabel` finally-completion routing back through the return /
//! outer-exception / normal antecedent subsets), **IIFE inlining**
//! (`GetImmediatelyInvokedFunctionExpression` + the `bindContainer`
//! `!isImmediatelyInvoked` gate — a non-async, non-generator function/arrow
//! callee of a call is bound *transparently* into the containing flow, with its
//! own return target merged at exit but no fresh `Start` and no `current_flow`
//! restore), **initializer forks** (`bindInitializer` — a parameter /
//! binding-element default that actually changes `current_flow` forks around
//! it), and **labeled statements** (`bindLabeledStatement` + the
//! `activeLabelList` — labeled `break`/`continue` resolution, per-label
//! continue-target propagation, and the unreferenced-label `Unreachable` stamp).
//! Flow stays **dark** — nothing consumes it until F3, so this slice emits no
//! diagnostics.
//!
//! **`isTopLevelLogicalExpression` without parent pointers.** tsgo's
//! `bindBinaryExpressionFlow` walks the parent chain to decide whether a logical
//! expression is evaluated for its value (top-level → `hasFlowEffects` post-label
//! wrap) or as a condition (nested → wired to the enclosing true/false targets).
//! tsv's `Expression` has no parent pointer, so the walk is replaced by keeping
//! the true/false targets `Some` **only** while binding an actual sub-condition —
//! they are set by `do_with_conditional_branches` / the `!`-swap, and reset to
//! `None` at three boundaries so they never leak into a non-condition: (1) at every
//! **value sub-position** — `visit_expression` resets them for every non-threading
//! expression, so a logical nested in a call argument / `?:` arm / array element
//! (`if (f(x && y))`) is classified top-level (a value), not a sub-condition;
//! (2) at every **flow container** — one can be entered mid-condition
//! (`if (arr.some(x => x && y))`), which would otherwise leak the outer targets
//! into the callback body; and (3) around the **logical-compound-assignment RHS** —
//! the RHS of `&&=`/`||=`/`??=` is reached through a *threading* node (the
//! compound-assign itself), so the `visit_expression` auto-reset never fires;
//! `bind_logical_like_expression` clears the targets explicitly so a logical RHS
//! (`a &&= x && y`) is classified top-level, matching tsgo's
//! `isTopLevelLogicalExpression` verdict on the RHS's parent (the compound-assign,
//! not a logical binary; see that site for detail). With these resets a logical
//! expression is top-level iff `current_true_target` is `None`. All are deliberate
//! departures from tsgo (which never saves the targets, relying on the parent walk)
//! required by the pointer-free heuristic.
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
//!   contained identifiers still get their leaf stamps. Binding-element and
//!   parameter **defaults** fork per `bindInitializer` (F2b) when the default
//!   changes the flow.
//
// tsgo: internal/binder/binder.go bind / bindContainer / bindChildren
//       (+ the newFlowNode* / createFlow* / finishFlowLabel / addAntecedent
//        constructor family and the per-statement flow shapers)

use crate::binder::{BoundFile, NodeKind, addr_of, expression_addr_kind, statement_kind};
use crate::ids::{FlowNodeId, NodeId};
use smallvec::SmallVec;
use tsv_lang::Span;
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
    fn insert(&mut self, other: FlowFlags) {
        self.0 |= other.0;
    }

    /// Whether this is a label node (`BranchLabel` or `LoopLabel`).
    #[inline]
    #[must_use]
    pub const fn is_label(self) -> bool {
        self.intersects(FlowFlags::LABEL)
    }
}

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
    flags: Vec<FlowFlags>,
    /// Kind-discriminated by `flags`: a `NodeId` (raw, 1-based) | payload index
    /// | 0 = none. In F1a it is always a `NodeId` or 0.
    subject: Vec<u32>,
    /// Non-label: the single antecedent's raw `FlowNodeId` (0 = none).
    /// Label: a 1-based pool-run index (0 = collapsed / unfinalized).
    antecedent: Vec<u32>,
    /// Length-prefixed antecedent runs for labels (`[len, e0, e1, …]`).
    pool: Vec<u32>,
    /// Switch-clause payloads, addressed by a `SwitchClause` node's 1-based
    /// `subject` slot (read via [`FlowGraph::switch_clause_data`]).
    switch_payloads: Vec<FlowSwitchClause>,
    /// Reduce-label payloads (try/finally), addressed by a `ReduceLabel` node's
    /// 1-based `subject` slot (read via [`FlowGraph::reduce_label_data`]).
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

    /// The reduced antecedent list of a `ReduceLabel` node, in append order (the
    /// temporary antecedent subset the checker substitutes for `target` while it
    /// passes this node).
    #[must_use]
    pub fn reduce_label_antecedents(&self, id: FlowNodeId) -> Vec<FlowNodeId> {
        let data = self.reduce_label_data(id);
        let off = (data.antecedents - 1) as usize; // 1-based pool-run index
        let len = self.pool[off] as usize;
        self.pool[off + 1..off + 1 + len]
            .iter()
            .filter_map(|&raw| FlowNodeId::from_raw(raw))
            .collect()
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
    /// Per-node flag bytes, one per [`NodeId`] (minted zeroed here — the flow
    /// walk is the sole writer today), with the `Unreachable` bit set during the
    /// dead-code walk (`NODE_FLAGS_UNREACHABLE`).
    pub node_flags: Vec<u8>,
    /// Function-body + `SourceFile` end-of-flow anchors (binder.go:1561,1569),
    /// sorted by `NodeId`.
    pub end_flow: Vec<(NodeId, FlowNodeId)>,
    /// Constructor + class-static-block return-flow anchors ONLY
    /// (binder.go:1575), sorted by `NodeId`. Every other tsgo `ReturnFlowNode`
    /// write/read is dead plumbing and is not ported.
    pub return_flow: Vec<(NodeId, FlowNodeId)>,
    /// Case-clause fallthrough anchors: the reachable exit flow of each non-last
    /// clause (tsgo's `clause.AsCaseOrDefaultClause().FallthroughFlowNode`,
    /// binder.go:2121), sorted by `NodeId`.
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

    /// The `fallthrough_flow` anchor for a case clause, if any (the reachable
    /// exit flow of a non-last clause).
    #[must_use]
    pub fn fallthrough_flow_of(&self, node: NodeId) -> Option<FlowNodeId> {
        self.fallthrough_flow
            .binary_search_by_key(&node, |&(n, _)| n)
            .ok()
            .map(|i| self.fallthrough_flow[i].1)
    }
}

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
struct FlowBuilder<'a> {
    bound: &'a BoundFile,
    /// The host document — the label-name lookup extracts `spans[id]` slices.
    source: &'a str,

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
    unreachable_flow: FlowNodeId,
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
    fn new(bound: &'a BoundFile, source: &'a str) -> FlowBuilder<'a> {
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

    fn finish(mut self) -> FlowProduct {
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
        self.visit_class_common(c.id.as_ref(), c.decorators, c.super_class, c.body.body);
    }

    fn visit_class_expr(&mut self, c: &ClassExpression<'_>) {
        self.visit_class_common(c.id.as_ref(), c.decorators, c.super_class, c.body.body);
    }

    /// The value-flow class descent shared by the declaration and expression forms
    /// (distinct types with the same field shape): the name binding, decorators, and
    /// the `extends` expression, then each member. Type positions (type params /
    /// super type args / `implements`) are skipped.
    fn visit_class_common(
        &mut self,
        name: Option<&Identifier<'_>>,
        decorators: Option<&[Decorator<'_>]>,
        super_class: Option<&Expression<'_>>,
        members: &[ClassMember<'_>],
    ) {
        if let Some(name) = name {
            self.visit_identifier(name);
        }
        self.visit_decorators(decorators);
        if let Some(sc) = super_class {
            self.visit_expression(sc);
        }
        for member in members {
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

    fn visit_method(&mut self, m: &MethodDefinition<'_>) {
        self.visit_decorators(m.decorators);
        let is_ctor = m.kind == MethodKind::Constructor;
        self.visit_expression(&m.key);
        // The method body lives in `value` (a FunctionExpression); the method is
        // a control-flow container anchored on that FunctionExpression — the
        // body-bearing node (tsv wraps a method body in a FunctionExpression,
        // where tsc's method node holds the body directly). The `MethodDefinition`
        // and its inline `value` share an address (a repr reorder puts `value` at
        // offset 0), so the address map keys on `(address, NodeKind)`; anchoring
        // here resolves the FunctionExpression id via its kind (the method itself
        // is now separately resolvable by `NodeKind::MethodDefinition`). The
        // obj-literal/class-expression method flow-write + Start.Node subject
        // (binder.go:982, 1534) is a P3 narrowing hint, deferred to F1b.
        let anchor = self.require(addr_of(&m.value), NodeKind::FunctionExpression);
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
            // collision workaround). The obj-literal method flow-write
            // (binder.go:982) is a P3 narrowing hint, deferred.
            self.visit_expression(&pr.key);
            let anchor = self.require(addr_of(f), NodeKind::FunctionExpression);
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
    if flags.contains(FlowFlags::REDUCE_LABEL) {
        // The `subject` slot is a payload index, not a NodeId — read the target
        // through the payload, never subject().
        let data = g.reduce_label_data(id);
        return format!("#{} {}→N{}", id.get(), header, data.target.get());
    }
    if flags.contains(FlowFlags::SWITCH_CLAUSE) {
        // A SwitchClause node's `subject` slot is a payload index, not a NodeId —
        // read the switch text + clause range through the payload, never subject().
        let data = g.switch_clause_data(id);
        let span = node_spans[data.switch.index()];
        let text = span.extract(source);
        let text = text.split('\n').next().unwrap_or(text);
        let text = match text.char_indices().nth(24) {
            Some((idx, _)) => &text[..idx],
            None => text,
        };
        return format!(
            "#{} {}[{},{}): {}",
            id.get(),
            header,
            data.clause_start,
            data.clause_end,
            text
        );
    }
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
    } else if flags.contains(FlowFlags::SWITCH_CLAUSE) {
        "switch"
    } else if flags.contains(FlowFlags::REDUCE_LABEL) {
        "reduce"
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

    /// Build the flow product **and** keep the `BoundFile` (both owned) so a
    /// topology test can look up node ids by kind / text.
    fn build_with_bound(source: &str) -> (FlowProduct, BoundFile) {
        let arena = Bump::new();
        let program = tsv_ts::parse(source, &arena).expect("parse");
        let bound = bind_file(&program, source, FileId::ROOT);
        let product = build_flow(&program, source, &bound);
        (product, bound)
    }

    /// The flow node stamped on a node (panics if unattached).
    fn flow_of_node(product: &FlowProduct, id: NodeId) -> FlowNodeId {
        product.flow_of_node[id.index()].expect("flow attachment")
    }

    /// The single flow node matching `pred` (panics if none / used where unique).
    fn find_flow(
        product: &FlowProduct,
        pred: impl Fn(&FlowGraph, FlowNodeId) -> bool,
    ) -> FlowNodeId {
        (1..=product.graph.node_count())
            .filter_map(FlowNodeId::from_raw)
            .find(|&id| pred(&product.graph, id))
            .expect("matching flow node")
    }

    /// The condition node (`TrueCondition`/`FalseCondition`) whose subject is
    /// `subject`.
    fn condition_of(product: &FlowProduct, subject: NodeId, want_true: bool) -> FlowNodeId {
        let flag = if want_true {
            FlowFlags::TRUE_CONDITION
        } else {
            FlowFlags::FALSE_CONDITION
        };
        find_flow(product, |g, id| {
            g.flags(id).contains(flag) && g.subject(id) == Some(subject)
        })
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
    fn node_flags_column_is_minted_here_zeroed_and_sized() {
        // The per-node flag column lives on the flow product (its sole writer);
        // reachable-only code leaves every byte zero.
        let (product, bound) = build_with_bound("const x = 1; function f<T>(a: T) { return a; }");
        assert_eq!(product.node_flags.len(), bound.node_count as usize);
        assert!(product.node_flags.iter().all(|&b| b == 0));
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

    /// Count `CALL` flow nodes in the whole graph.
    fn call_node_count(product: &FlowProduct) -> usize {
        (1..=product.graph.node_count())
            .filter_map(FlowNodeId::from_raw)
            .filter(|&id| product.graph.flags(id).contains(FlowFlags::CALL))
            .count()
    }

    #[test]
    fn comma_operands_each_mint_a_call_flow_node() {
        // `bindBinaryExpressionFlow` comma branch applies `maybeBindExpressionFlowIfCall`
        // to every operand — each discarded (statement-like) dotted-name call is a
        // potential assertion, so a two-operand comma mints one CALL per operand.
        let two = build("function f() { m1(), m2(); }");
        assert_eq!(
            call_node_count(&two),
            2,
            "each comma operand's dotted-name call should mint a CALL flow node"
        );
        // Control: a bare expression statement mints exactly one (the established path).
        let one = build("function f() { m1(); }");
        assert_eq!(call_node_count(&one), 1);
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
        // The symmetric accessor resolves the anchor to the same return flow.
        assert_eq!(product.return_flow_of(anchor_node), Some(rf));
        assert!(product.stats.branch_labels >= 1);
        assert!(product.stats.dead_labels >= 1);
    }

    #[test]
    fn finish_flow_label_pool_run_preserves_order_and_dedups() {
        let src = "const x = 1;";
        let arena = Bump::new();
        let program = tsv_ts::parse(src, &arena).expect("parse");
        let bound = bind_file(&program, src, FileId::ROOT);
        let mut b = FlowBuilder::new(&bound, src);
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
                Expression::Literal(l) => bound.require_node_id(addr_of(l), NodeKind::Literal),
                Expression::Identifier(idn) => {
                    bound.require_node_id(addr_of(idn), NodeKind::Identifier)
                }
                _ => panic!("unexpected expression"),
            };
            (&s.expression, id)
        };
        let true_lit = expr_at(0);
        let false_lit = expr_at(1);
        let y = expr_at(2);

        let mut b = FlowBuilder::new(&bound, src);
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

    // --- F1b branching topology (hand-traced graphs) ----------------------

    #[test]
    fn if_else_two_arm_merge() {
        // `if (x) a; else b;` — C1=TrueCond(x,F0), C2=FalseCond(x,F0); a.flow=C1,
        // b.flow=C2; both merge at a materialized BranchLabel [C1,C2]; F0 Shared.
        let src = "function f() { if (x) a; else b; }";
        let (product, bound) = build_with_bound(src);
        let x = ident(&bound, src, "x");
        let a = ident(&bound, src, "a");
        let b = ident(&bound, src, "b");

        let f0 = flow_of_node(&product, x);
        assert!(product.graph.flags(f0).contains(FlowFlags::START));

        let c1 = condition_of(&product, x, true);
        let c2 = condition_of(&product, x, false);
        assert_eq!(product.graph.antecedents(c1), vec![f0]);
        assert_eq!(product.graph.antecedents(c2), vec![f0]);
        assert_eq!(flow_of_node(&product, a), c1);
        assert_eq!(flow_of_node(&product, b), c2);

        // F0 is referenced by both conditions → Shared.
        assert!(product.graph.flags(f0).contains(FlowFlags::SHARED));

        // The if merges at postIf (a materialized 2-antecedent BranchLabel) — the
        // function's end-of-flow.
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        let exit = product.end_flow_of(f).expect("f end_flow");
        assert!(product.graph.flags(exit).contains(FlowFlags::BRANCH_LABEL));
        assert_eq!(product.graph.antecedents(exit), vec![c1, c2]);
    }

    #[test]
    fn reachable_after_if_merge() {
        // `if (x) a; b;` — with no else, `b` (the statement after the if) binds at
        // the postIf merge label.
        let src = "function f() { if (x) a; b; }";
        let (product, bound) = build_with_bound(src);
        let x = ident(&bound, src, "x");
        let b = ident(&bound, src, "b");
        let c1 = condition_of(&product, x, true);
        let c2 = condition_of(&product, x, false);
        let b_flow = flow_of_node(&product, b);
        // b's entry flow is the postIf label carrying the then-branch (C1) and the
        // empty-else branch (C2).
        assert!(
            product
                .graph
                .flags(b_flow)
                .contains(FlowFlags::BRANCH_LABEL)
        );
        assert_eq!(product.graph.antecedents(b_flow), vec![c1, c2]);
    }

    #[test]
    fn logical_in_condition_value_subposition_is_top_level() {
        // `if (f(x && y)) a; else b;` — the `x && y` sits in a VALUE sub-position
        // (a call argument) of the if condition, so it is top-level (a value with
        // its own post-label), NOT a sub-condition of the if. tsgo classifies this
        // via a parent walk (`isTopLevelLogicalExpression`); tsv resets the
        // condition targets at the value boundary in `visit_expression`. The if's
        // actual condition `f(x && y)` is non-narrowing with no flow effects, so
        // BOTH arms enter from the function Start — the distinguishing property:
        // the bug wired x/y's conditions into the if's then/else, making
        // a.flow != b.flow. (`if (c ? x && y : z)` and `if (g([x && y]))` are the
        // same class — value sub-positions.)
        let src = "function w() { if (f(x && y)) a; else b; }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        let b = ident(&bound, src, "b");
        let a_flow = flow_of_node(&product, a);
        let b_flow = flow_of_node(&product, b);
        assert_eq!(
            a_flow, b_flow,
            "a non-narrowing if-condition merges both arms; x && y must not wire into them"
        );
        assert!(product.graph.flags(a_flow).contains(FlowFlags::START));
        // `x && y` is still narrowed as a value — its own condition nodes exist,
        // but they feed x && y's post-label, not the if arms.
        let x = ident(&bound, src, "x");
        let xc = condition_of(&product, x, true);
        assert_ne!(a_flow, xc);
    }

    #[test]
    fn logical_compound_assign_rhs_is_top_level_value() {
        // `a &&= x && y;` as a STATEMENT — the RHS `x && y` binds as a top-level
        // VALUE. tsgo classifies it via `isTopLevelLogicalExpression` (binder.go:2782)
        // on `right`'s PARENT, which is the `&&=` node (not a logical operator), so
        // the RHS is top-level: its own true/false conditions are self-contained in a
        // throwaway post-label and discarded (effect-free identifiers), NOT threaded
        // into the outer `&&=` post-label. tsgo wires only FALSE(a) + the whole-node
        // truthiness — 3 antecedents. The bug (threading the RHS) leaked x/y's four
        // conditions, giving 6: [FALSE(a), FALSE(x), TRUE(y), FALSE(y), TRUE(whole),
        // FALSE(whole)].
        let src = "function f() { a &&= x && y; }";
        let (product, bound) = build_with_bound(src);
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        // The `&&=` has flow effects (the Assignment mutation), so its post-label is
        // materialized and becomes the function's end-of-flow.
        let post = product.end_flow_of(f).expect("f end_flow");
        assert!(product.graph.flags(post).contains(FlowFlags::BRANCH_LABEL));

        let a = ident(&bound, src, "a");
        let whole = nodes_of_kind(&bound, NodeKind::AssignmentExpression)[0];
        let false_a = condition_of(&product, a, false);
        let true_whole = condition_of(&product, whole, true);
        let false_whole = condition_of(&product, whole, false);
        // Exact shape (and order): FALSE(a), then the whole-node TRUE/FALSE — no x/y.
        assert_eq!(
            product.graph.antecedents(post),
            vec![false_a, true_whole, false_whole],
            "the &&= post-label carries FALSE(a) + TRUE/FALSE(whole) only — x/y stay top-level"
        );
    }

    #[test]
    fn logical_compound_assign_still_threads_whole_node_in_condition() {
        // `if (a &&= x && y) d;` — the `&&=` node itself is a CONDITION (its parent
        // is the if), so its whole-node truthiness threads into then/else, while its
        // RHS `x && y` is still top-level (self-contained, discarded). Post-fix:
        //   - the then-branch enters from the whole-node TRUE condition ALONE
        //     (d.flow == TRUE(whole)) — x/y's TRUE(y) does not merge in;
        //   - the else branch carries exactly FALSE(a) + FALSE(whole) — x/y's
        //     FALSE(x)/FALSE(y) do not leak in.
        // The bug merged TRUE(y) into the then-branch and FALSE(x)/FALSE(y) into the
        // else-branch.
        let src = "function f() { if (a &&= x && y) d; }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        let d = ident(&bound, src, "d");
        let whole = nodes_of_kind(&bound, NodeKind::AssignmentExpression)[0];
        let false_a = condition_of(&product, a, false);
        let true_whole = condition_of(&product, whole, true);
        let false_whole = condition_of(&product, whole, false);

        // then-branch = the whole-node TRUE condition alone (single antecedent
        // collapses the then-label to the condition itself).
        assert_eq!(
            flow_of_node(&product, d),
            true_whole,
            "the then-branch enters from the &&= whole-node truthiness alone — TRUE(y) must not merge in"
        );

        // postIf merges the then-exit (TRUE(whole)) and the else-branch label.
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        let post_if = product.end_flow_of(f).expect("f end_flow");
        let ants = product.graph.antecedents(post_if);
        assert_eq!(
            ants.len(),
            2,
            "postIf merges the then-exit and the else-branch"
        );
        assert_eq!(
            ants[0], true_whole,
            "then-exit is the whole-node TRUE condition"
        );
        let else_label = ants[1];
        assert_eq!(
            product.graph.antecedents(else_label),
            vec![false_a, false_whole],
            "the else branch carries only FALSE(a) + FALSE(whole) — x/y stay top-level"
        );
    }

    #[test]
    fn coalescing_compound_assign_rhs_is_top_level_value() {
        // `a ??= x || y;` as a STATEMENT — the shared logical-compound-assign branch
        // walked with `is_and=false, is_nullish=true` (the `??=` path, distinct from
        // `&&=`). Like `&&=`, the RHS `x || y` is a top-level VALUE: tsgo's
        // `isTopLevelLogicalExpression(right)` (binder.go:2782) inspects `right`'s
        // PARENT — the `??=` node, which is a compound-assignment operator, not a
        // logical binary (`IsLogicalExpression` unwraps parens/`!` then requires a
        // `&&`/`||`/`??` *binary*), so `right` is top-level. Its own true/false
        // conditions are self-contained in a throwaway post-label and discarded
        // (effect-free identifiers), NOT threaded into the outer `??=` post-label.
        // The `??=`/`||` mirror of `bindLogicalLikeExpression` (binder.go:2266-2268,
        // the non-`&&` branch) wires the LEFT's TRUE condition (not FALSE, as `&&=`
        // does) into the post: the outer post carries TRUE(a) + the whole-node
        // truthiness — 3 antecedents, no x/y. The bug (threading the RHS) would leak
        // x/y's four conditions.
        let src = "function f() { a ??= x || y; }";
        let (product, bound) = build_with_bound(src);
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        // The `??=` mutates `a` (a flow effect), so its post-label is materialized and
        // becomes the function's end-of-flow.
        let post = product.end_flow_of(f).expect("f end_flow");
        assert!(product.graph.flags(post).contains(FlowFlags::BRANCH_LABEL));

        let a = ident(&bound, src, "a");
        let x = ident(&bound, src, "x");
        let whole = nodes_of_kind(&bound, NodeKind::AssignmentExpression)[0];
        let true_a = condition_of(&product, a, true);
        let true_whole = condition_of(&product, whole, true);
        let false_whole = condition_of(&product, whole, false);
        // Exact shape (and order): TRUE(a) (the `??=`/`||` mirror of the `&&=` test's
        // FALSE(a)), then the whole-node TRUE/FALSE — no x/y.
        assert_eq!(
            product.graph.antecedents(post),
            vec![true_a, true_whole, false_whole],
            "the ??= post-label carries TRUE(a) + TRUE/FALSE(whole) only — x || y stays top-level"
        );
        // `x || y` is still narrowed as a value — its TRUE(x) condition exists and
        // feeds its OWN (discarded, effect-free) post-label, distinct from the ??= post.
        let true_x = condition_of(&product, x, true);
        let x_post = find_flow(&product, |g, id| {
            g.flags(id).is_label() && g.antecedents(id).contains(&true_x)
        });
        assert_ne!(
            x_post, post,
            "x || y feeds its own post-label, not the ??= post"
        );
        assert!(!product.graph.antecedents(post).contains(&true_x));
    }

    #[test]
    fn nested_logical_compound_assign_rhs_gets_own_post_label() {
        // `a &&= b ||= c;` — the RHS `b ||= c` is ITSELF a logical compound-assignment.
        // Its parent is the outer `&&=` node (an assignment operator, not a logical
        // binary), so tsgo `isTopLevelLogicalExpression(b ||= c)` is true: it is bound
        // top-level with its OWN post-label, NOT threaded into the outer `&&=` targets.
        // Because `b ||= c` has a flow effect (it mutates `b`), its post-label is
        // materialized and the outer `a`-mutation flows THROUGH it — distinct from the
        // effect-free logical-RHS case (`a ??= x || y`) where the RHS post is discarded.
        let src = "function f() { a &&= b ||= c; }";
        let (product, bound) = build_with_bound(src);
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        let post = product.end_flow_of(f).expect("f end_flow");
        assert!(product.graph.flags(post).contains(FlowFlags::BRANCH_LABEL));

        let a = ident(&bound, src, "a");
        let b = ident(&bound, src, "b");
        // Two AssignmentExpressions: the outer `a &&= b ||= c` (whole statement) and
        // the inner RHS `b ||= c`. Disambiguate by span length (outer encloses inner).
        let assigns = nodes_of_kind(&bound, NodeKind::AssignmentExpression);
        assert_eq!(assigns.len(), 2);
        let span_len = |id: NodeId| bound.spans[id.index()].end - bound.spans[id.index()].start;
        let outer = assigns
            .iter()
            .copied()
            .max_by_key(|&id| span_len(id))
            .unwrap();
        let inner = assigns
            .iter()
            .copied()
            .min_by_key(|&id| span_len(id))
            .unwrap();

        let false_a = condition_of(&product, a, false);
        let true_outer = condition_of(&product, outer, true);
        let false_outer = condition_of(&product, outer, false);
        // The outer `&&=` post carries FALSE(a) + the outer whole-node TRUE/FALSE only
        // (the `&&=` mirror) — the inner `b ||= c`'s conditions do NOT leak in.
        assert_eq!(
            product.graph.antecedents(post),
            vec![false_a, true_outer, false_outer],
            "the &&= post carries FALSE(a) + TRUE/FALSE(outer) only — b ||= c stays top-level"
        );

        // The inner `b ||= c` has its OWN materialized post-label (it mutates `b`),
        // carrying its own [TRUE(b), TRUE(inner), FALSE(inner)] — the `||=` mirror,
        // self-contained exactly as the whole `??=` RHS was, one level down.
        let true_b = condition_of(&product, b, true);
        let true_inner = condition_of(&product, inner, true);
        let false_inner = condition_of(&product, inner, false);
        let inner_post = find_flow(&product, |g, id| {
            g.flags(id).is_label() && g.antecedents(id).contains(&true_inner)
        });
        assert_ne!(
            inner_post, post,
            "b ||= c feeds its own post-label, not the &&= post"
        );
        assert_eq!(
            product.graph.antecedents(inner_post),
            vec![true_b, true_inner, false_inner],
            "b ||= c's own post carries TRUE(b) + its whole-node TRUE/FALSE"
        );
        // The outer `a`-mutation's antecedent is that inner post (b ||= c had flow
        // effects), so the nested compound-assign threads through as a top-level value.
        let a_assign = find_flow(&product, |g, id| {
            g.flags(id).contains(FlowFlags::ASSIGNMENT) && g.subject(id) == Some(a)
        });
        assert_eq!(
            product.graph.antecedents(a_assign),
            vec![inner_post],
            "the outer a-mutation's antecedent is b ||= c's materialized post"
        );
    }

    #[test]
    fn while_loop_topology() {
        // `while (x) a;` — L1=LoopLabel; entry F0 added first, back edge (C1)
        // after the body → L1.antecedents=[F0,C1]; x.flow=L1; a.flow=C1; exit=C2.
        let src = "function f() { while (x) a; }";
        let (product, bound) = build_with_bound(src);
        let x = ident(&bound, src, "x");
        let a = ident(&bound, src, "a");
        let while_stmt = nodes_of_kind(&bound, NodeKind::WhileStatement)[0];
        let f0 = flow_of_node(&product, while_stmt); // the while's entry flow (f's Start)

        let l1 = flow_of_node(&product, x);
        assert!(product.graph.flags(l1).contains(FlowFlags::LOOP_LABEL));
        let c1 = condition_of(&product, x, true);
        let c2 = condition_of(&product, x, false);
        assert_eq!(product.graph.antecedents(l1), vec![f0, c1]);
        assert_eq!(flow_of_node(&product, a), c1);

        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        assert_eq!(product.end_flow_of(f), Some(c2));
    }

    #[test]
    fn do_while_loop_topology() {
        // `do a; while (x);` — L1=LoopLabel[F0]; a.flow=L1; x.flow=L1; the
        // true-condition loops back → L1.antecedents=[F0,C1]; exit=C2.
        let src = "function f() { do a; while (x); }";
        let (product, bound) = build_with_bound(src);
        let x = ident(&bound, src, "x");
        let a = ident(&bound, src, "a");
        let do_stmt = nodes_of_kind(&bound, NodeKind::DoWhileStatement)[0];
        let f0 = flow_of_node(&product, do_stmt);

        let l1 = flow_of_node(&product, a);
        assert!(product.graph.flags(l1).contains(FlowFlags::LOOP_LABEL));
        assert_eq!(flow_of_node(&product, x), l1); // condition binds from the loop label
        let c1 = condition_of(&product, x, true);
        let c2 = condition_of(&product, x, false);
        assert_eq!(product.graph.antecedents(l1), vec![f0, c1]);

        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        assert_eq!(product.end_flow_of(f), Some(c2));
    }

    #[test]
    fn for_infinite_self_loop() {
        // `for (;;) a;` — nil condition: True→L1 passthrough, False→unreachable
        // (dropped). a.flow=L1; the back edge self-loops → L1.antecedents=[F0,L1];
        // postLoop stays empty so the function exits unreachable (no end_flow).
        let src = "function f() { for (;;) a; }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        let for_stmt = nodes_of_kind(&bound, NodeKind::ForStatement)[0];
        let f0 = flow_of_node(&product, for_stmt);

        let l1 = flow_of_node(&product, a);
        assert!(product.graph.flags(l1).contains(FlowFlags::LOOP_LABEL));
        // Self-loop: L1 is its own back-edge antecedent (guarded by vec equality).
        assert_eq!(product.graph.antecedents(l1), vec![f0, l1]);

        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        assert_eq!(product.end_flow_of(f), None); // unreachable exit
    }

    #[test]
    fn unlabeled_continue_targets_loop_label() {
        // `while (x) continue;` — the continue routes back to the loop label,
        // so L1.antecedents=[F0, C1]; the normal exit is the false condition.
        let src = "function f() { while (x) continue; }";
        let (product, bound) = build_with_bound(src);
        let x = ident(&bound, src, "x");
        let l1 = flow_of_node(&product, x);
        let c1 = condition_of(&product, x, true);
        let antes = product.graph.antecedents(l1);
        assert!(
            antes.contains(&c1),
            "continue back-edge lands on the loop label"
        );
        assert_eq!(antes.len(), 2); // [entry F0, continue C1]

        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        let c2 = condition_of(&product, x, false);
        assert_eq!(product.end_flow_of(f), Some(c2));
    }

    #[test]
    fn unlabeled_break_targets_post_loop() {
        // `while (x) break;` — the break routes to the post-loop label (the
        // function exit), which also carries the false-condition edge; the break
        // makes the back edge unreachable, so the loop label keeps only its entry.
        let src = "function f() { while (x) break; }";
        let (product, bound) = build_with_bound(src);
        let x = ident(&bound, src, "x");
        let c1 = condition_of(&product, x, true);
        let c2 = condition_of(&product, x, false);

        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        let exit = product.end_flow_of(f).expect("f end_flow");
        let antes = product.graph.antecedents(exit);
        assert!(antes.contains(&c1), "break edge to the post-loop label");
        assert!(antes.contains(&c2), "false-condition exit edge");

        // The loop label kept only the entry edge (the back edge was unreachable).
        let l1 = flow_of_node(&product, x);
        assert_eq!(product.graph.antecedents(l1).len(), 1);
    }

    #[test]
    fn referenced_shared_recompute_parity() {
        // Recompute the live-graph in-degree and check it against the Referenced /
        // Shared bits. `setFlowNodeReferenced` marks a node on EVERY antecedent
        // add at construction (matching tsgo), including adds into a branch label
        // that later COLLAPSES to a dead row — and tsv's SoA drops a collapsed
        // label's edges (slot 0, no pool run). So the live in-degree is a **lower
        // bound** on the referenced-count, and the sound, one-directional
        // invariant is: every live antecedent edge is reflected in the bits (they
        // never under-mark). The fn Start (shared by both condition nodes) gives a
        // genuine live in-degree ≥ 2 → Shared.
        let src = "function f() { if (x) a; else b; }";
        let product = build(src);
        let g = &product.graph;
        let n = g.node_count();
        let mut indeg = vec![0u32; (n + 1) as usize];
        for id in (1..=n).filter_map(FlowNodeId::from_raw) {
            for ante in g.antecedents(id) {
                indeg[ante.get() as usize] += 1;
            }
        }
        let mut saw_shared = false;
        for id in (1..=n).filter_map(FlowNodeId::from_raw) {
            let d = indeg[id.get() as usize];
            let flags = g.flags(id);
            if d >= 1 {
                assert!(
                    flags.contains(FlowFlags::REFERENCED),
                    "in-degree ≥ 1 ⟹ Referenced at node {}",
                    id.get()
                );
            }
            if d >= 2 {
                assert!(
                    flags.contains(FlowFlags::SHARED),
                    "in-degree ≥ 2 ⟹ Shared at node {}",
                    id.get()
                );
                saw_shared = true;
            }
        }
        assert!(saw_shared, "the fn Start is shared by both condition nodes");
    }

    // --- F2a switch topology (hand-traced graphs) -------------------------

    /// Every `SwitchClause` flow node, in id order.
    fn switch_clauses(product: &FlowProduct) -> Vec<FlowNodeId> {
        (1..=product.graph.node_count())
            .filter_map(FlowNodeId::from_raw)
            .filter(|&id| product.graph.flags(id).contains(FlowFlags::SWITCH_CLAUSE))
            .collect()
    }

    #[test]
    fn switch_no_default_has_exhaustive_sentinel() {
        // `switch (x) { case 1: a; }` — no default clause, so postSwitch gets the
        // clause-1 exit AND a `(0, 0)` "no clause matched" SwitchClause sentinel.
        let src = "function f() { switch (x) { case 1: a; } }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        // The clause body is reachable (fed from the switch head).
        assert_ne!(flow_of_node(&product, a), FlowNodeId::UNREACHABLE);

        // The `(0, 0)` sentinel exists and feeds postSwitch (the function exit).
        let sentinel = switch_clauses(&product)
            .into_iter()
            .find(|&id| {
                let d = product.graph.switch_clause_data(id);
                d.clause_start == 0 && d.clause_end == 0
            })
            .expect("no-default (0,0) sentinel");
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        let exit = product.end_flow_of(f).expect("f end_flow");
        assert!(
            product.graph.antecedents(exit).contains(&sentinel),
            "the (0,0) sentinel feeds postSwitch"
        );
    }

    #[test]
    fn switch_break_then_clause_stays_reachable() {
        // THE F2a PROOF. `switch (x) { case 1: break; case 2: a; }` — case 1
        // breaks, so nothing falls through into case 2; but case 2 is reachable
        // FROM THE SWITCH HEAD, so `a` must be reachable. F1b's linear stub
        // threaded current_flow (= unreachable after the break) into case 2 and
        // wrongly marked it Unreachable — this test fails on that stub.
        let src = "function f() { switch (x) { case 1: break; case 2: a; } }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        let a_flow = flow_of_node(&product, a);
        assert_ne!(
            a_flow,
            FlowNodeId::UNREACHABLE,
            "case 2 is reachable from the switch head despite case 1's break"
        );
        // `a`'s entry is the clause's SwitchClause node covering range [1, 2).
        assert!(
            product
                .graph
                .flags(a_flow)
                .contains(FlowFlags::SWITCH_CLAUSE)
        );
        assert_eq!(
            {
                let d = product.graph.switch_clause_data(a_flow);
                (d.clause_start, d.clause_end)
            },
            (1, 2)
        );
        // The `a;` statement is reachable: Some entry flow, no Unreachable flag.
        let a_stmt = nodes_of_kind(&bound, NodeKind::ExpressionStatement)[0];
        assert!(product.flow_of_node[a_stmt.index()].is_some());
        assert_eq!(
            product.node_flags[a_stmt.index()] & crate::binder::NODE_FLAGS_UNREACHABLE,
            0
        );
    }

    #[test]
    fn switch_fallthrough_feeds_next_clause() {
        // `switch (x) { case 1: a; case 2: b; }` — case 1 falls through to case 2,
        // so case 2's preCase merges its switch-head edge (a SwitchClause[1,2)) and
        // case 1's fallthrough edge; case 1 records a fallthrough anchor.
        let src = "function f() { switch (x) { case 1: a; case 2: b; } }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        let b = ident(&bound, src, "b");
        let a_flow = flow_of_node(&product, a);
        let b_flow = flow_of_node(&product, b);

        // case 2 binds at a materialized 2-antecedent branch label.
        assert!(
            product
                .graph
                .flags(b_flow)
                .contains(FlowFlags::BRANCH_LABEL)
        );
        let antes = product.graph.antecedents(b_flow);
        assert_eq!(antes.len(), 2);
        // One antecedent is case 1's exit (the fallthrough).
        assert!(antes.contains(&a_flow), "fallthrough edge from case 1");
        // The other is case 2's switch-head SwitchClause with range [1, 2).
        let head = antes
            .iter()
            .copied()
            .find(|&x| x != a_flow)
            .expect("head edge");
        assert!(product.graph.flags(head).contains(FlowFlags::SWITCH_CLAUSE));
        assert_eq!(
            {
                let d = product.graph.switch_clause_data(head);
                (d.clause_start, d.clause_end)
            },
            (1, 2)
        );
        // case 1 (the first SwitchCase node) recorded its reachable exit anchor.
        let case1 = nodes_of_kind(&bound, NodeKind::SwitchCase)[0];
        assert_eq!(product.fallthrough_flow_of(case1), Some(a_flow));
    }

    #[test]
    fn switch_empty_clause_run_reachable() {
        // `switch (x) { case 1: case 2: a; }` — the empty `case 1` shares the run
        // with `case 2`; `a` is reachable, fed from the head via one SwitchClause
        // whose range spans the merged run [0, 2).
        let src = "function f() { switch (x) { case 1: case 2: a; } }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        let a_flow = flow_of_node(&product, a);
        assert_ne!(a_flow, FlowNodeId::UNREACHABLE);
        assert!(
            product
                .graph
                .flags(a_flow)
                .contains(FlowFlags::SWITCH_CLAUSE)
        );
        assert_eq!(
            {
                let d = product.graph.switch_clause_data(a_flow);
                (d.clause_start, d.clause_end)
            },
            (0, 2)
        );
    }

    #[test]
    fn switch_true_narrows_with_real_range() {
        // `switch (true) { case y: a; }` — a narrowing switch, so the clause gets
        // a real SwitchClause node carrying its [0, 1) range, fed from the head.
        let src = "function f() { switch (true) { case y: a; } }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        let a_flow = flow_of_node(&product, a);
        assert!(
            product
                .graph
                .flags(a_flow)
                .contains(FlowFlags::SWITCH_CLAUSE)
        );
        assert_eq!(
            {
                let d = product.graph.switch_clause_data(a_flow);
                (d.clause_start, d.clause_end)
            },
            (0, 1)
        );
        // The SwitchClause node's single antecedent is the switch head (fn Start).
        let head = product.graph.antecedents(a_flow);
        assert_eq!(head.len(), 1);
        assert!(product.graph.flags(head[0]).contains(FlowFlags::START));
    }

    #[test]
    fn switch_non_narrowing_clauses_have_no_payload() {
        // `switch (f()) { case 1: a; case 2: b; }` — a call discriminant is NOT
        // narrowing, so each clause is fed from the bare switch head (no per-clause
        // `SwitchClause` payload node). Clauses stay reachable; the only SwitchClause
        // in the graph is the no-default `(0,0)` sentinel. (Guards the `is_narrowing_switch`
        // false branch — a regression that always minted SwitchClause nodes would
        // pass every narrowing test.)
        let src = "function f() { switch (f()) { case 1: a; case 2: b; } }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        let b = ident(&bound, src, "b");
        assert_ne!(flow_of_node(&product, a), FlowNodeId::UNREACHABLE);
        assert_ne!(flow_of_node(&product, b), FlowNodeId::UNREACHABLE);
        // Neither clause body's entry flow is a SwitchClause node.
        assert!(
            !product
                .graph
                .flags(flow_of_node(&product, a))
                .contains(FlowFlags::SWITCH_CLAUSE)
        );
        assert!(
            !product
                .graph
                .flags(flow_of_node(&product, b))
                .contains(FlowFlags::SWITCH_CLAUSE)
        );
        // The only SwitchClause node is the `(0,0)` sentinel (no default clause).
        let clauses = switch_clauses(&product);
        assert_eq!(clauses.len(), 1);
        let d = product.graph.switch_clause_data(clauses[0]);
        assert_eq!((d.clause_start, d.clause_end), (0, 0));
    }

    #[test]
    fn switch_with_default_has_no_sentinel() {
        // `switch (x) { case 1: a; default: b; }` — a `default` clause makes the
        // switch exhaustive, so NO `(0,0)` sentinel is emitted. (Narrowing, so the
        // clauses still get real SwitchClause payloads.) Guards the `has_default`
        // path — a regression that always emitted the sentinel would pass every
        // no-default test.
        let src = "function f() { switch (x) { case 1: a; default: b; } }";
        let (product, bound) = build_with_bound(src);
        let a = ident(&bound, src, "a");
        let b = ident(&bound, src, "b");
        assert_ne!(flow_of_node(&product, a), FlowNodeId::UNREACHABLE);
        assert_ne!(flow_of_node(&product, b), FlowNodeId::UNREACHABLE);
        // No SwitchClause node carries the `(0,0)` sentinel range.
        assert!(
            switch_clauses(&product).into_iter().all(|id| {
                let d = product.graph.switch_clause_data(id);
                (d.clause_start, d.clause_end) != (0, 0)
            }),
            "a default-present switch emits no (0,0) exhaustiveness sentinel"
        );
    }

    // --- F2b: the four remaining flow landmines (hand-traced graphs) -------

    /// Every `ReduceLabel` flow node, in id order.
    fn reduce_labels(product: &FlowProduct) -> Vec<FlowNodeId> {
        (1..=product.graph.node_count())
            .filter_map(FlowNodeId::from_raw)
            .filter(|&id| product.graph.flags(id).contains(FlowFlags::REDUCE_LABEL))
            .collect()
    }

    #[test]
    fn try_finally_reduce_label_and_merge() {
        // `try { a; } finally { b; }` — b binds at the finally label (a branch
        // label merging the try-normal and exception antecedents); the try exits
        // through a REDUCE_LABEL (the finally's normal-completion routing) whose
        // target is that finally label.
        let src = "function f() { try { a; } finally { b; } }";
        let (product, bound) = build_with_bound(src);
        let b = ident(&bound, src, "b");
        let b_flow = flow_of_node(&product, b);
        assert!(
            product
                .graph
                .flags(b_flow)
                .contains(FlowFlags::BRANCH_LABEL)
        );

        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        let exit = product.end_flow_of(f).expect("f end_flow");
        assert!(product.graph.flags(exit).contains(FlowFlags::REDUCE_LABEL));
        assert_eq!(product.graph.reduce_label_data(exit).target, b_flow);
        // The reduced antecedent list is the try block's normal exit (f's Start).
        let reduced = product.graph.reduce_label_antecedents(exit);
        assert_eq!(reduced.len(), 1);
        assert!(product.graph.flags(reduced[0]).contains(FlowFlags::START));
    }

    #[test]
    fn try_catch_finally_exception_edges() {
        // Catch = a second try. `try { x = 1; } catch { b; } finally { c; }` —
        // the catch binds at the try's exception label, fed by BOTH the
        // "any instruction can throw" edge (the entry Start) AND the mutation's
        // exception fan-out (createFlowMutation → currentExceptionTarget).
        let src = "function f() { try { x = 1; } catch { b; } finally { c; } }";
        let (product, bound) = build_with_bound(src);
        let b = ident(&bound, src, "b");
        let b_flow = flow_of_node(&product, b);
        assert!(
            product
                .graph
                .flags(b_flow)
                .contains(FlowFlags::BRANCH_LABEL)
        );
        let antes = product.graph.antecedents(b_flow);
        assert!(
            antes
                .iter()
                .any(|&a| product.graph.flags(a).contains(FlowFlags::START)),
            "the pre-mutation throw edge"
        );
        assert!(
            antes
                .iter()
                .any(|&a| product.graph.flags(a).contains(FlowFlags::ASSIGNMENT)),
            "the mutation's exception fan-out"
        );
        // The finally still routes normal completion through a REDUCE_LABEL.
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        let exit = product.end_flow_of(f).expect("f end_flow");
        assert!(product.graph.flags(exit).contains(FlowFlags::REDUCE_LABEL));
    }

    #[test]
    fn try_finally_return_routes_through_reduce_label() {
        // An IIFE gives the try a real (non-None) return target, so a `return`
        // inside a try-with-finally materializes a return-only ReduceLabel that
        // feeds that target (and collapses onto it as the function exit).
        let src = "function f() { (function() { try { return 1; } finally { g(); } })(); }";
        let (product, bound) = build_with_bound(src);
        let reduces = reduce_labels(&product);
        assert_eq!(
            reduces.len(),
            1,
            "one ReduceLabel: the return-only finally routing"
        );
        let rl = reduces[0];
        let reduced = product.graph.reduce_label_antecedents(rl);
        assert_eq!(reduced.len(), 1, "the single return path");
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        assert_eq!(product.end_flow_of(f), Some(rl));
    }

    #[test]
    fn iife_body_is_inlined_into_containing_flow() {
        // THE IIFE PROOF. `(function(){ g(); })(); h();` — the IIFE body is NOT
        // flow-isolated: `h` continues from the IIFE body's exit (the g() call),
        // and `g` binds under the ambient flow (no fresh Start).
        let src = "function f() { (function(){ g(); })(); h(); }";
        let (product, bound) = build_with_bound(src);
        let g = ident(&bound, src, "g");
        let h = ident(&bound, src, "h");
        assert!(
            product
                .graph
                .flags(flow_of_node(&product, g))
                .contains(FlowFlags::START),
            "g binds under the ambient (transparent) flow"
        );
        assert!(
            product
                .graph
                .flags(flow_of_node(&product, h))
                .contains(FlowFlags::CALL),
            "h continues from the IIFE body's g() call, not a restored/fresh flow"
        );
    }

    #[test]
    fn non_invoked_function_expression_is_flow_isolated() {
        // Contrast: a non-invoked function expression IS isolated — `h` is
        // unaffected (binds at the `const x = …` mutation), and `g` binds under
        // the function's own fresh Start.
        let src = "function f() { const x = function(){ g(); }; h(); }";
        let (product, bound) = build_with_bound(src);
        let g = ident(&bound, src, "g");
        let h = ident(&bound, src, "h");
        assert!(
            product
                .graph
                .flags(flow_of_node(&product, g))
                .contains(FlowFlags::START)
        );
        assert!(
            product
                .graph
                .flags(flow_of_node(&product, h))
                .contains(FlowFlags::ASSIGNMENT),
            "h binds at the const-x assignment, not the isolated g() call"
        );
    }

    #[test]
    fn async_iife_stays_isolated() {
        // Guards the `!async` gate: an async IIFE is NOT inlined, so `h` binds
        // under the outer function's own flow (Start), not continued from the
        // async body's g() call. A regression dropping the async check would make
        // `h`'s flow the inlined CALL (as in the sync-IIFE proof).
        let src = "function f() { (async function(){ g(); })(); h(); }";
        let (product, bound) = build_with_bound(src);
        let h = ident(&bound, src, "h");
        let h_flow = flow_of_node(&product, h);
        assert!(
            product.graph.flags(h_flow).contains(FlowFlags::START),
            "h binds under the outer Start — the async IIFE body is flow-isolated"
        );
        assert!(!product.graph.flags(h_flow).contains(FlowFlags::CALL));
    }

    #[test]
    fn try_return_finally_leaves_post_try_unreachable_in_plain_function() {
        // Guards the normal-list-empty → unreachable branch: in a PLAIN function
        // (no return target), `try { return; } finally {}` leaves the code after
        // the try unreachable — the try's only exit was via `return` (to the
        // return label), so the finally's normal-exit list is empty. The existing
        // return-reduce test uses an IIFE (non-None return target), so this
        // plain-function branch was uncovered.
        let src = "function f() { try { return; } finally {} g(); }";
        let (product, bound) = build_with_bound(src);
        let g = ident(&bound, src, "g");
        // `g` (a leaf in dead code) keeps `Some(unreachable)`; the `g();` statement
        // is unreachable.
        assert_eq!(flow_of_node(&product, g), FlowNodeId::UNREACHABLE);
    }

    #[test]
    fn parameter_default_that_changes_flow_forks() {
        // A parameter default containing a flow-changing expression (an
        // assignment mutation) forks current_flow around the initializer
        // (bindInitializer). The only branch label is the fork's exit.
        let src = "function f(a = (b = c)) {}";
        let (product, bound) = build_with_bound(src);
        assert_eq!(product.stats.branch_labels, 1);
        let a = ident(&bound, src, "a");
        let a_flow = flow_of_node(&product, a);
        assert!(
            product
                .graph
                .flags(a_flow)
                .contains(FlowFlags::BRANCH_LABEL)
        );
        assert_eq!(
            product.graph.antecedents(a_flow).len(),
            2,
            "the no-default entry + the post-initializer flow merge"
        );
    }

    #[test]
    fn parameter_default_without_flow_change_does_not_fork() {
        // A literal default doesn't change current_flow → no fork, no label.
        let src = "function f(a = 1) {}";
        let product = build(src);
        assert_eq!(product.stats.branch_labels, 0);
    }

    #[test]
    fn labeled_continue_resolves_to_loop_continue_target() {
        // `outer: while (x) { continue outer; }` — continue outer routes to the
        // while's continue target (the loop label), and `outer` is referenced so
        // its label identifier carries NO Unreachable bit.
        let src = "function f() { outer: while (x) { continue outer; } }";
        let (product, bound) = build_with_bound(src);
        let x = ident(&bound, src, "x");
        let l1 = flow_of_node(&product, x);
        assert!(product.graph.flags(l1).contains(FlowFlags::LOOP_LABEL));
        let c1 = condition_of(&product, x, true);
        let antes = product.graph.antecedents(l1);
        assert!(
            antes.contains(&c1),
            "continue outer lands on the loop label (like an unlabeled continue)"
        );
        assert_eq!(antes.len(), 2); // [entry, continue-outer back edge]

        let outer = ident(&bound, src, "outer");
        assert_eq!(
            product.node_flags[outer.index()] & crate::binder::NODE_FLAGS_UNREACHABLE,
            0,
            "outer is referenced → no Unreachable stamp"
        );
    }

    #[test]
    fn unreferenced_label_gets_unreachable_stamp() {
        // `unused: a;` — the label is never targeted, so its identifier gets the
        // Unreachable bit (the TS7028 signal).
        let src = "function f() { unused: a; }";
        let (product, bound) = build_with_bound(src);
        let unused = ident(&bound, src, "unused");
        assert_ne!(
            product.node_flags[unused.index()] & crate::binder::NODE_FLAGS_UNREACHABLE,
            0,
            "an unreferenced label identifier carries the Unreachable bit"
        );
    }

    #[test]
    fn labeled_break_targets_outer_post_label() {
        // `outer: inner: while (x) { break outer; }` — break outer targets
        // outer's post-statement label (the function exit, merging the break edge
        // and the loop's normal false-condition exit). `outer` is referenced,
        // `inner` is not.
        let src = "function f() { outer: inner: while (x) { break outer; } }";
        let (product, bound) = build_with_bound(src);
        let outer = ident(&bound, src, "outer");
        let inner = ident(&bound, src, "inner");
        assert_eq!(
            product.node_flags[outer.index()] & crate::binder::NODE_FLAGS_UNREACHABLE,
            0,
            "outer is referenced by break outer"
        );
        assert_ne!(
            product.node_flags[inner.index()] & crate::binder::NODE_FLAGS_UNREACHABLE,
            0,
            "inner is unused"
        );

        let x = ident(&bound, src, "x");
        let c1 = condition_of(&product, x, true);
        let c2 = condition_of(&product, x, false);
        let f = nodes_of_kind(&bound, NodeKind::FunctionDeclaration)[0];
        let exit = product.end_flow_of(f).expect("f end_flow");
        assert!(product.graph.flags(exit).contains(FlowFlags::BRANCH_LABEL));
        let antes = product.graph.antecedents(exit);
        assert!(
            antes.contains(&c1),
            "the break-outer edge (from inside the loop body)"
        );
        assert!(
            antes.contains(&c2),
            "the loop's normal false-condition exit"
        );
    }
}
