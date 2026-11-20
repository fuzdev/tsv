//! Tests for the pure AST predicates the flow walk dispatches on — the
//! counterpart of `build/predicates.rs`.

use super::super::build::{FlowBuilder, is_narrowable_reference};
use super::super::*;
use super::parse;
use crate::binder::{NodeKind, addr_of, bind_file};
use crate::ids::FileId;
use tsv_ts::ast::{Expression, ExpressionKind, StatementKind};

#[test]
fn create_flow_condition_ports_verbatim() {
    let src = "true; false; y;";
    let parsed = parse(src);
    let view = parsed.view();
    let bound = bind_file((&parsed).into(), src, FileId::ROOT);

    // Extract the top-level expressions + their node ids, read in place.
    let expr_at = |i: usize| -> (&Expression, NodeId) {
        let stmt = view.stmt(view.stmt_list(parsed.program.body)[i]);
        let StatementKind::ExpressionStatement(s) = &stmt.kind else {
            panic!("expression statement");
        };
        let expression = view.expr(s.expression);
        let id = match &expression.kind {
            ExpressionKind::Literal(l) => bound.require_node_id(addr_of(l), NodeKind::Literal),
            ExpressionKind::Identifier(idn) => {
                bound.require_node_id(addr_of(idn), NodeKind::Identifier)
            }
            _ => panic!("unexpected expression"),
        };
        (expression, id)
    };
    let true_lit = expr_at(0);
    let false_lit = expr_at(1);
    let y = expr_at(2);

    let mut b = FlowBuilder::new(view, &bound, src);
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
    let src = "a.b; a[0]; a?.b;";
    let parsed = parse(src);
    let view = parsed.view();
    for &stmt in view.stmt_list(parsed.program.body) {
        if let StatementKind::ExpressionStatement(s) = &view.stmt(stmt).kind {
            assert!(
                is_narrowable_reference(view, view.expr(s.expression)),
                "member/element access should be narrowable"
            );
        }
    }
}
