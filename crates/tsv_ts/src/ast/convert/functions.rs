// Function-related expression conversions

use super::super::{internal, public};
use super::expressions::convert_expression_inner;
use super::{
    convert_block_statement, convert_expression, convert_type_annotation,
    convert_type_parameter_declaration, convert_type_parameter_instantiation, create_location,
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
        expression: arrow.body.is_expression(),
        generator: false,
        is_async: arrow.r#async,
        params: arrow
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner, offset))
            .collect(),
        body,
        type_parameters: arrow
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner, offset)),
        return_type: arrow
            .return_type
            .as_ref()
            .map(|rt| convert_type_annotation(rt, source, loc, interner, offset)),
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
            decorators: Vec::new(),
        }),
        expression: false,
        generator: func.generator,
        is_async: func.r#async,
        type_parameters: func
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner, offset)),
        params: func
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner, offset))
            .collect(),
        return_type: func
            .return_type
            .as_ref()
            .map(|rt| convert_type_annotation(rt, source, loc, interner, offset)),
        body: convert_block_statement(&func.body, source, loc, interner, offset),
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
        type_arguments: new_expr
            .type_arguments
            .as_ref()
            .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner, offset)),
        arguments: new_expr
            .arguments
            .iter()
            .map(|arg| convert_expression(arg, source, loc, interner, offset))
            .collect(),
    }
}

/// Chain-aware call expression conversion.
/// When `in_chain` is true, uses `convert_expression_inner` with `in_chain=true`
/// for the callee so nested chain expressions don't get double-wrapped.
pub(in crate::ast) fn convert_call_expression(
    call: &internal::CallExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
    in_chain: bool,
) -> public::CallExpression {
    let mut callee =
        convert_expression_inner(&call.callee, source, loc, interner, offset, in_chain);
    // acorn-typescript's `?.<T>(...)` path marks the callee node itself optional
    // (its parseSubscript sets `base.optional = true` before parsing the type args)
    if call.optional && call.type_arguments.is_some() {
        match &mut callee {
            public::Expression::Identifier(id) => id.optional = true,
            public::Expression::MemberExpression(member) => member.optional = Some(true),
            public::Expression::CallExpression(inner) => inner.optional = Some(true),
            _ => {}
        }
    }
    // acorn-typescript omits `optional` on a typeArguments call unless the call is
    // part of an optional chain (parseSubscript only sets it when `_optionalChained`);
    // the chain test is the call's own left segment, so a trailing `?.` after the
    // call (`a.fn<T>()?.b`) doesn't count, and parens seal the segment
    let in_optional_chain = call.optional
        || (call.span.start >= call.callee.span().start && call.callee.has_optional_in_chain());
    public::CallExpression {
        node_type: "CallExpression".to_string(),
        start: call.span.start,
        end: call.span.end,
        loc: create_location(call.span, loc, offset),
        callee: Box::new(callee),
        type_arguments: call
            .type_arguments
            .as_ref()
            .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner, offset)),
        arguments: call
            .arguments
            .iter()
            .map(|arg| convert_expression(arg, source, loc, interner, offset))
            .collect(),
        optional: if call.type_arguments.is_some() && !in_optional_chain {
            None
        } else {
            Some(call.optional)
        },
    }
}

/// Chain-aware member expression conversion.
/// When `in_chain` is true, uses `convert_expression_inner` with `in_chain=true`
/// for the object so nested chain expressions don't get double-wrapped.
pub(in crate::ast) fn convert_member_expression(
    member: &internal::MemberExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
    in_chain: bool,
) -> public::MemberExpression {
    public::MemberExpression {
        node_type: "MemberExpression".to_string(),
        start: member.span.start,
        end: member.span.end,
        loc: create_location(member.span, loc, offset),
        object: Box::new(convert_expression_inner(
            &member.object,
            source,
            loc,
            interner,
            offset,
            in_chain,
        )),
        property: Box::new(convert_expression(
            &member.property,
            source,
            loc,
            interner,
            offset,
        )),
        computed: member.computed,
        optional: Some(member.optional),
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

pub(in crate::ast) fn convert_yield_expression(
    yield_expr: &internal::YieldExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::YieldExpression {
    public::YieldExpression {
        node_type: "YieldExpression".to_string(),
        start: yield_expr.span.start,
        end: yield_expr.span.end,
        loc: create_location(yield_expr.span, loc, offset),
        argument: yield_expr
            .argument
            .as_ref()
            .map(|arg| Box::new(convert_expression(arg, source, loc, interner, offset))),
        delegate: yield_expr.delegate,
    }
}
