use super::build::{FlowBuilder, is_narrowable_reference};
use super::*;
use crate::binder::{BoundFile, NodeKind, addr_of, bind_file};
use crate::ids::FileId;
use bumpalo::Bump;
use tsv_ts::ast::internal::{Expression, Statement};

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

/// Find the first node of `kind`, with its body-`Start` flow node (the START
/// whose subject is that node), if any.
fn start_subject_of(
    product: &FlowProduct,
    bound: &BoundFile,
    kind: NodeKind,
) -> (NodeId, Option<FlowNodeId>) {
    let node = NodeId::from_index(
        bound
            .kinds
            .iter()
            .position(|&k| k == kind)
            .expect("node of kind"),
    );
    let g = &product.graph;
    let start = (1..=g.node_count())
        .filter_map(FlowNodeId::from_raw)
        .find(|&f| g.flags(f).contains(FlowFlags::START) && g.subject(f) == Some(node));
    (node, start)
}

#[test]
fn class_expression_method_gets_flow_write_and_start_subject() {
    // tsgo binder.go:981 (outer-flow write on the method node) + :1534
    // (Start.Node = the method) — class-EXPRESSION methods only.
    let (product, bound) =
        build_with_bound("const C = class { m() { return 1; } get g() { return 2; } };");
    let (method, start) = start_subject_of(&product, &bound, NodeKind::MethodDefinition);
    assert!(
        start.is_some(),
        "class-expression method Start carries the method subject"
    );
    assert!(
        product.flow_of_node[method.index()].is_some(),
        "class-expression method node gets the outer-flow write"
    );
}

#[test]
fn class_declaration_method_stays_unstamped() {
    // The Parent.Kind gate (utilities.go:566): a class-DECLARATION method gets
    // neither the flow write nor a Start subject.
    let (product, bound) = build_with_bound("class D { m() { return 1; } }");
    let (method, start) = start_subject_of(&product, &bound, NodeKind::MethodDefinition);
    assert!(
        start.is_none(),
        "class-declaration method Start has no subject"
    );
    assert!(product.flow_of_node[method.index()].is_none());
}

#[test]
fn class_expression_constructor_excluded_from_method_gate() {
    // A constructor is not a MethodDeclaration/accessor kind — excluded even
    // inside a class expression.
    let (product, bound) = build_with_bound("const C = class { constructor() { this.x = 1; } };");
    let (ctor, start) = start_subject_of(&product, &bound, NodeKind::MethodDefinition);
    assert!(start.is_none(), "constructor Start has no subject");
    assert!(product.flow_of_node[ctor.index()].is_none());
}

#[test]
fn object_literal_method_gets_flow_write_and_start_subject() {
    // The object-literal half of the gate: the Property node (tsv's analog of
    // tsgo's object-literal MethodDeclaration) is stamped and made the subject.
    let (product, bound) = build_with_bound("const o = { m() { return 1; } };");
    let (prop, start) = start_subject_of(&product, &bound, NodeKind::Property);
    assert!(
        start.is_some(),
        "object-literal method Start carries the Property subject"
    );
    assert!(product.flow_of_node[prop.index()].is_some());
}

#[test]
fn object_literal_plain_property_stays_unstamped() {
    // A function-VALUED plain property is not a method: the FunctionExpression
    // itself is the Start subject (the fn-expr rule), the Property is not.
    let (product, bound) = build_with_bound("const o = { m: function () { return 1; } };");
    let (prop, prop_start) = start_subject_of(&product, &bound, NodeKind::Property);
    assert!(
        prop_start.is_none(),
        "plain property Start has no Property subject"
    );
    assert!(product.flow_of_node[prop.index()].is_none());
    let (_f, f_start) = start_subject_of(&product, &bound, NodeKind::FunctionExpression);
    assert!(
        f_start.is_some(),
        "the function expression keeps its own subject"
    );
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
