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

mod build;
mod flags;
mod graph;
mod product;
#[cfg(test)]
mod tests;

pub use build::build_flow;
pub use flags::FlowFlags;
pub use graph::{FlowGraph, FlowReduceLabel, FlowSwitchClause};
pub use product::{FlowProduct, FlowStats, render_flow_dot};

use crate::ids::{FlowNodeId, NodeId};
