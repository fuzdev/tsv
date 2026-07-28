//! Tests for the expression-shaped flow topology — logical / compound-assign
//! conditions, class-expression / object-literal method start-subjects, and
//! IIFE / function-expression isolation — the counterpart of
//! `build/expressions.rs`.

use super::super::*;
use super::{build, build_with_bound, condition_of, find_flow, flow_of_node, ident, nodes_of_kind};
use crate::binder::{BoundFile, NodeKind};

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
