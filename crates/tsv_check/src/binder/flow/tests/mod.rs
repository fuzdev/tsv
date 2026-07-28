//! Shared test fixtures for the flow-graph walk, plus the flow-node minting,
//! container, and label-pool tests — the graph-construction machinery that
//! `build/mod.rs` itself owns, as opposed to any one statement or expression
//! shape. The statement-shaped, expression-shaped, and predicate tests live
//! in the `statements`, `expressions`, and `predicates` submodules,
//! mirroring `build/statements.rs`, `build/expressions.rs`, and
//! `build/predicates.rs`.

mod expressions;
mod predicates;
mod statements;

use super::build::FlowBuilder;
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
fn find_flow(product: &FlowProduct, pred: impl Fn(&FlowGraph, FlowNodeId) -> bool) -> FlowNodeId {
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
fn antecedent_iter_forms_agree_with_collected_forms() {
    // A branching + loop + try/finally program exercises single-slot nodes,
    // multi-antecedent labels, and a ReduceLabel; the zero-alloc iterators
    // must agree with the collected forms on every node, and the non-label
    // single-slot read must agree with the general form.
    let product = build(
        "function f(a: boolean) { try { while (a) { if (a) break; a = !a; } } finally { g(); } }",
    );
    let g = &product.graph;
    for raw in 1..=g.node_count() {
        let id = FlowNodeId::from_raw(raw).unwrap();
        let collected = g.antecedents(id);
        assert_eq!(g.antecedents_iter(id).collect::<Vec<_>>(), collected);
        if !g.flags(id).is_label() {
            assert_eq!(g.single_antecedent(id), collected.first().copied());
            assert!(collected.len() <= 1);
        }
        if g.flags(id).contains(FlowFlags::REDUCE_LABEL) {
            assert_eq!(
                g.reduce_label_antecedents_iter(id).collect::<Vec<_>>(),
                g.reduce_label_antecedents(id)
            );
        }
    }
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
