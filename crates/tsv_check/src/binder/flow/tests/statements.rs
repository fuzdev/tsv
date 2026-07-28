//! Tests for the statement-shaped flow topology — conditions, loops, switch,
//! try/catch/finally, initializer forks, and labeled statements — the
//! counterpart of `build/statements.rs`.

use super::super::*;
use super::{build, build_with_bound, condition_of, flow_of_node, ident, nodes_of_kind};
use crate::binder::NodeKind;

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
