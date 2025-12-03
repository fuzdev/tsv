// Function-related expression conversions

use super::super::{internal, public};
use super::{
    convert_block_statement, convert_expression, convert_type_annotation, create_location,
};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};

pub(in crate::ast) fn convert_arrow_function_expression(
    arrow: &internal::ArrowFunctionExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ArrowFunctionExpression {
    let body = match &arrow.body {
        internal::ArrowFunctionBody::Expression(expr) => public::ArrowFunctionBody::Expression(
            Box::new(convert_expression(expr, source, loc, interner, offset)),
        ),
        internal::ArrowFunctionBody::BlockStatement(block) => {
            public::ArrowFunctionBody::BlockStatement(convert_block_statement(
                block, source, loc, interner, offset,
            ))
        }
    };
    public::ArrowFunctionExpression {
        node_type: "ArrowFunctionExpression".to_string(),
        start: arrow.span.start,
        end: arrow.span.end,
        loc: create_location(arrow.span, loc, offset),
        id: None,
        expression: arrow.expression,
        generator: false,
        is_async: arrow.r#async,
        params: arrow
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner, offset))
            .collect(),
        body,
        return_type: arrow
            .return_type
            .as_ref()
            .map(|rt| convert_type_annotation(rt, loc, offset)),
    }
}

pub(in crate::ast) fn convert_function_expression(
    func: &internal::FunctionExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::FunctionExpression {
    public::FunctionExpression {
        node_type: "FunctionExpression".to_string(),
        start: func.span.start,
        end: func.span.end,
        loc: create_location(func.span, loc, offset),
        id: func.id.as_ref().map(|id| public::Identifier {
            node_type: "Identifier".to_string(),
            start: id.span.start,
            end: id.span.end,
            loc: create_location(id.span, loc, offset),
            name: interner.resolve_infallible(id.name).to_string(),
            optional: id.optional,
            type_annotation: None,
        }),
        expression: false,
        generator: func.generator,
        is_async: func.r#async,
        params: func
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner, offset))
            .collect(),
        return_type: func
            .return_type
            .as_ref()
            .map(|rt| convert_type_annotation(rt, loc, offset)),
        body: convert_block_statement(&func.body, source, loc, interner, offset),
    }
}

pub(in crate::ast) fn convert_call_expression(
    call: &internal::CallExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::CallExpression {
    public::CallExpression {
        node_type: "CallExpression".to_string(),
        start: call.span.start,
        end: call.span.end,
        loc: create_location(call.span, loc, offset),
        callee: Box::new(convert_expression(
            &call.callee,
            source,
            loc,
            interner,
            offset,
        )),
        arguments: call
            .arguments
            .iter()
            .map(|arg| convert_expression(arg, source, loc, interner, offset))
            .collect(),
        optional: call.optional,
    }
}

pub(in crate::ast) fn convert_new_expression(
    new_expr: &internal::NewExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::NewExpression {
    public::NewExpression {
        node_type: "NewExpression".to_string(),
        start: new_expr.span.start,
        end: new_expr.span.end,
        loc: create_location(new_expr.span, loc, offset),
        callee: Box::new(convert_expression(
            &new_expr.callee,
            source,
            loc,
            interner,
            offset,
        )),
        arguments: new_expr
            .arguments
            .iter()
            .map(|arg| convert_expression(arg, source, loc, interner, offset))
            .collect(),
    }
}

pub(in crate::ast) fn convert_member_expression(
    member: &internal::MemberExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::MemberExpression {
    public::MemberExpression {
        node_type: "MemberExpression".to_string(),
        start: member.span.start,
        end: member.span.end,
        loc: create_location(member.span, loc, offset),
        object: Box::new(convert_expression(
            &member.object,
            source,
            loc,
            interner,
            offset,
        )),
        property: Box::new(convert_expression(
            &member.property,
            source,
            loc,
            interner,
            offset,
        )),
        computed: member.computed,
        optional: member.optional,
    }
}

pub(in crate::ast) fn convert_conditional_expression(
    cond: &internal::ConditionalExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ConditionalExpression {
    public::ConditionalExpression {
        node_type: "ConditionalExpression".to_string(),
        start: cond.span.start,
        end: cond.span.end,
        loc: create_location(cond.span, loc, offset),
        test: Box::new(convert_expression(
            &cond.test, source, loc, interner, offset,
        )),
        consequent: Box::new(convert_expression(
            &cond.consequent,
            source,
            loc,
            interner,
            offset,
        )),
        alternate: Box::new(convert_expression(
            &cond.alternate,
            source,
            loc,
            interner,
            offset,
        )),
    }
}

pub(in crate::ast) fn convert_await_expression(
    await_expr: &internal::AwaitExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::AwaitExpression {
    public::AwaitExpression {
        node_type: "AwaitExpression".to_string(),
        start: await_expr.span.start,
        end: await_expr.span.end,
        loc: create_location(await_expr.span, loc, offset),
        argument: Box::new(convert_expression(
            &await_expr.argument,
            source,
            loc,
            interner,
            offset,
        )),
    }
}
