// helper fns here aren't `#[test]`, so clippy.toml's allow-panic-in-tests doesn't reach them
#![expect(clippy::panic)]

//! `tsv_ts::__private`'s write surface, driven from outside the crate the way a host that
//! builds TypeScript around a parsed tree drives it (the Svelte compiler's output program):
//! a host module is parsed, a program is minted around its ids — an arrow over a host
//! expression, a call to a synthetic name, a rebuilt argument list, a string literal whose
//! cooked value lives in the pool, the host's comments as the program's own run — and the
//! canonical reprint prints it. Only the public and `__private` surface is used, so the test
//! also pins what a crate outside `tsv_ts` can name: every row type, the generic functions,
//! and inference of the sealed traits from the arguments alone.

use tsv_ts::__private::{
    extend_from_run, flush, format_canonical, open, parse_into, parsed_ref, push,
    push_escaped_name, push_item, push_list, push_text,
};
use tsv_ts::ast::{
    ArrowFunctionBody, ArrowFunctionExpression, AstStores, CallExpression, ExprId, Expression,
    ExpressionKind, ExpressionStatement, IdentName, Identifier, Literal, LiteralValue, Program,
    Span, Statement, StatementKind, StringCooked,
};

/// The host's one call, `g(1, 2)`, as its statement, call row and call.
fn host_call(stores: &AstStores, program: &Program) -> (Statement, Expression, CallExpression) {
    let view = stores.view();
    let statement = *view.stmt(view.stmt_list(program.body)[0]);
    let StatementKind::ExpressionStatement(expression) = statement.kind else {
        panic!("an expression statement");
    };
    let row = *view.expr(expression.expression);
    let ExpressionKind::CallExpression(call) = row.kind else {
        panic!("a call");
    };
    (statement, row, call)
}

#[test]
fn a_program_minted_around_host_ids_prints_canonically() {
    // the host module, then the appendix a builder writes its minted literals' text into
    let host = "// keep\ng(1, 2);\n";
    let source = format!("{host}'q'");
    let mut stores = AstStores::new();
    let program = parse_into(host, &mut stores).expect("parses");
    let (statement, call_row, call) = host_call(&stores, &program);
    let host_args: Vec<ExprId> = stores.view().expr_list(call.arguments).to_vec();
    let end = host.len() as u32;

    // `() => g(1, 2)`: an arrow payload over the host's call (a nested push goes into a local
    // first: the outer push already holds the stores)
    let params = push_list::<Expression>(&mut stores, &[]);
    let body = push(&mut stores, call_row);
    let arrow = push(
        &mut stores,
        ArrowFunctionExpression {
            type_parameters: None,
            params,
            body: ArrowFunctionBody::Expression(body),
            return_type: None,
            r#async: false,
            params_start: Some(end),
            arrow_token: end,
        },
    );
    let arrow = push(
        &mut stores,
        Expression {
            span: Span::new(end, end),
            kind: ExpressionKind::ArrowFunctionExpression(arrow),
        },
    );

    // `'q'`: a string literal over the appendix, its cooked value pooled
    let quoted = Span::new(end, end + 3);
    let cooked = push_text(&mut stores, "q");
    let literal = push(
        &mut stores,
        Expression {
            span: quoted,
            kind: ExpressionKind::Literal(Literal {
                value: LiteralValue::String(StringCooked::Decoded(cooked)),
                span: quoted,
            }),
        },
    );

    // the host's arguments rebuilt: the first kept, the minted literal, then the arrow
    let mark = open(&mut stores);
    extend_from_run(&mut stores, call.arguments, 0..1);
    push_item(&mut stores, literal);
    push_item(&mut stores, arrow);
    let arguments = flush(&mut stores, mark);
    assert_eq!(stores.view().expr_list(arguments)[0], host_args[0]);

    // `h(…)`: a call to a synthetic name no span spells
    let name = IdentName {
        escaped: Some(push_escaped_name(&mut stores, "h")),
        raw_len: 0,
        plain_ascii: false,
    };
    let callee = push(
        &mut stores,
        Expression::from_identifier(Identifier::simple(name, Span::new(end, end))),
    );
    let minted_call = push(
        &mut stores,
        Expression {
            span: Span::new(end, end + 3),
            kind: ExpressionKind::CallExpression(CallExpression {
                callee,
                arguments,
                ..call
            }),
        },
    );
    let StatementKind::ExpressionStatement(host_statement) = statement.kind else {
        panic!("an expression statement");
    };
    let minted_statement = push(
        &mut stores,
        Statement {
            span: Span::new(end, end + 3),
            kind: StatementKind::ExpressionStatement(ExpressionStatement {
                expression: minted_call,
                ..host_statement
            }),
        },
    );

    let host_body = stores.view().stmt_list(program.body).to_vec();
    let mut body = host_body;
    body.push(minted_statement);
    let comments = stores.view().comments(program.comments).to_vec();
    let output = Program {
        body: push_list(&mut stores, &body),
        comments: push_list(&mut stores, &comments),
        ..program
    };
    assert_eq!(
        format_canonical(parsed_ref(&output, &stores), &source),
        "// keep\ng(1, 2);\nh(1, 'q', () => g(1, 2));\n"
    );
}
