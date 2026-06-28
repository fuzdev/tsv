// Type alias, function, and class declaration conversions

use super::super::{internal, public};
use super::types::convert_type_annotation as convert_type_annotation_from_types;
use super::{
    Schema, convert_block_statement, convert_expression, convert_identifier, convert_statement,
    convert_type, convert_type_annotation, create_location,
};
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationTracker, Span};

/// Convert a decorator from internal to public AST
pub(super) fn convert_decorator<'src>(
    decorator: &internal::Decorator<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Decorator<'src> {
    let mut expression = convert_expression(&decorator.expression, source, loc, interner, offset);
    // acorn-typescript omits `optional` on the call/member spine of an
    // unparenthesized decorator (its restricted ident/member/call parse never sets
    // it); a parenthesized `@(expr)` rides the full expression parser, which does.
    // Parens are stripped from the expression, so the only signal is the span gap —
    // the decorator span covers the closing `)` past the expression's end.
    let parenthesized = decorator.span.end > decorator.expression.span().end;
    if !parenthesized {
        strip_decorator_spine_optional(&mut expression);
    }
    public::Decorator {
        node_type: "Decorator",
        start: decorator.span.start,
        end: decorator.span.end,
        loc: create_location(decorator.span, loc, offset),
        expression,
    }
}

/// Remove `optional` along an unparenthesized decorator's call/member spine
/// (`@a.b.c()` — the top call and every member down to the base identifier);
/// call arguments are parsed by the full grammar and keep theirs.
fn strip_decorator_spine_optional(expression: &mut public::Expression<'_>) {
    let mut node = expression;
    loop {
        match node {
            public::Expression::CallExpression(call) => {
                call.optional = None;
                node = &mut call.callee;
            }
            public::Expression::MemberExpression(member) => {
                member.optional = None;
                node = &mut member.object;
            }
            _ => break,
        }
    }
}

pub(in crate::ast) fn convert_type_alias_declaration<'src>(
    type_alias: &internal::TSTypeAliasDeclaration<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeAliasDeclaration<'src> {
    public::TSTypeAliasDeclaration {
        node_type: "TSTypeAliasDeclaration",
        start: type_alias.span.start,
        end: type_alias.span.end,
        loc: create_location(type_alias.span, loc, offset),
        id: convert_identifier(&type_alias.id, source, loc, interner, offset),
        type_parameters: type_alias
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner, offset)),
        type_annotation: convert_type(&type_alias.type_annotation, source, loc, interner, offset),
        declare: type_alias.declare,
    }
}

pub(in crate::ast) fn convert_function_declaration<'src>(
    func_decl: &internal::FunctionDeclaration<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::FunctionDeclaration<'src> {
    public::FunctionDeclaration {
        node_type: "FunctionDeclaration",
        start: func_decl.span.start,
        end: func_decl.span.end,
        loc: create_location(func_decl.span, loc, offset),
        id: func_decl
            .id
            .as_ref()
            .map(|id| convert_identifier(id, source, loc, interner, offset)),
        expression: false,
        generator: func_decl.generator,
        is_async: func_decl.r#async,
        type_parameters: func_decl
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner, offset)),
        params: func_decl
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner, offset))
            .collect(),
        return_type: func_decl
            .return_type
            .as_ref()
            .map(|rt| convert_type_annotation(rt, source, loc, interner, offset)),
        body: convert_block_statement(&func_decl.body, source, loc, interner, offset),
    }
}

pub(in crate::ast) fn convert_class_declaration<'src>(
    class_decl: &internal::ClassDeclaration<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassDeclaration<'src> {
    let mut super_class = class_decl
        .super_class
        .as_ref()
        .map(|e| Box::new(convert_expression(e, source, loc, interner, offset)));
    let mut super_type_parameters = class_decl
        .super_type_parameters
        .as_ref()
        .map(|tp| convert_type_parameter_instantiation(tp, source, loc, interner, offset));
    maybe_wrap_super_class(
        &mut super_class,
        &mut super_type_parameters,
        class_decl.type_parameters.as_ref().map(|tp| tp.span),
        source,
        loc,
        offset,
    );

    public::ClassDeclaration {
        node_type: "ClassDeclaration",
        start: class_decl.span.start,
        end: class_decl.span.end,
        loc: create_location(class_decl.span, loc, offset),
        decorators: class_decl.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner, offset))
                .collect()
        }),
        declare: class_decl.declare.then_some(true),
        abstract_: class_decl.r#abstract.then_some(true),
        id: class_decl
            .id
            .as_ref()
            .map(|id| convert_identifier(id, source, loc, interner, offset)),
        type_parameters: class_decl
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner, offset)),
        super_class,
        super_type_parameters,
        implements: if class_decl.implements.is_empty() {
            None
        } else {
            Some(
                class_decl
                    .implements
                    .iter()
                    .map(|h| {
                        convert_expression_with_type_arguments(h, source, loc, interner, offset)
                    })
                    .collect(),
            )
        },
        body: convert_class_body(&class_decl.body, source, loc, interner, offset),
    }
}

pub(in crate::ast) fn convert_class_expression<'src>(
    class_expr: &internal::ClassExpression<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassExpression<'src> {
    let mut super_class = class_expr
        .super_class
        .as_ref()
        .map(|e| Box::new(convert_expression(e, source, loc, interner, offset)));
    let mut super_type_parameters = class_expr
        .super_type_parameters
        .as_ref()
        .map(|tp| convert_type_parameter_instantiation(tp, source, loc, interner, offset));
    maybe_wrap_super_class(
        &mut super_class,
        &mut super_type_parameters,
        class_expr.type_parameters.as_ref().map(|tp| tp.span),
        source,
        loc,
        offset,
    );

    public::ClassExpression {
        node_type: "ClassExpression",
        start: class_expr.span.start,
        end: class_expr.span.end,
        loc: create_location(class_expr.span, loc, offset),
        decorators: class_expr.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner, offset))
                .collect()
        }),
        abstract_: class_expr.r#abstract.then_some(true),
        id: class_expr
            .id
            .as_ref()
            .map(|id| convert_identifier(id, source, loc, interner, offset)),
        type_parameters: class_expr
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner, offset)),
        super_class,
        super_type_parameters,
        implements: if class_expr.implements.is_empty() {
            None
        } else {
            Some(
                class_expr
                    .implements
                    .iter()
                    .map(|h| {
                        convert_expression_with_type_arguments(h, source, loc, interner, offset)
                    })
                    .collect(),
            )
        },
        body: convert_class_body(&class_expr.body, source, loc, interner, offset),
    }
}

/// acorn-typescript quirk: when `extends Base<T>` is on a different line from the
/// closing `>` of type parameters, superClass becomes a TSInstantiationExpression
/// wrapping the expression and type arguments. When on the same line, superClass
/// is the bare expression (Identifier, MemberExpression, etc.) with a separate
/// superTypeParameters field.
fn maybe_wrap_super_class<'src>(
    super_class: &mut Option<Box<public::Expression<'src>>>,
    super_type_parameters: &mut Option<public::TSTypeParameterInstantiation<'src>>,
    type_params_span: Option<Span>,
    source: &str,
    loc: &LocationTracker,
    offset: usize,
) {
    let Some(tp_span) = type_params_span else {
        return;
    };
    let (Some(sc), Some(stp)) = (super_class.as_ref(), super_type_parameters.as_ref()) else {
        return;
    };
    let sc_start = sc.start();

    // Only wrap when extends is on a different line from >
    if tsv_lang::printing::is_same_line(source, tp_span.end, sc_start) {
        return;
    }

    // Wrap: superClass becomes TSInstantiationExpression, superTypeParameters is consumed
    let combined_span = Span::new(sc_start, stp.end);
    let Some(inner) = super_class.take() else {
        return;
    };
    let Some(type_arguments) = super_type_parameters.take() else {
        return;
    };
    *super_class = Some(Box::new(public::Expression::TSInstantiationExpression(
        public::TSInstantiationExpression {
            node_type: "TSInstantiationExpression",
            start: combined_span.start,
            end: combined_span.end,
            loc: create_location(combined_span, loc, offset),
            expression: inner,
            type_arguments,
        },
    )));
}

pub(in crate::ast) fn convert_class_body<'src>(
    body: &internal::ClassBody<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassBody<'src> {
    public::ClassBody {
        node_type: "ClassBody",
        start: body.span.start,
        end: body.span.end,
        loc: create_location(body.span, loc, offset),
        body: body
            .body
            .iter()
            .map(|m| convert_class_member(m, source, loc, interner, offset))
            .collect(),
    }
}

fn convert_class_member<'src>(
    member: &internal::ClassMember<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassMember<'src> {
    match member {
        internal::ClassMember::MethodDefinition(method) => public::ClassMember::MethodDefinition(
            convert_method_definition(method, source, loc, interner, offset),
        ),
        internal::ClassMember::PropertyDefinition(prop) => public::ClassMember::PropertyDefinition(
            convert_property_definition(prop, source, loc, interner, offset),
        ),
        internal::ClassMember::StaticBlock(block) => public::ClassMember::StaticBlock(
            convert_static_block(block, source, loc, interner, offset),
        ),
        internal::ClassMember::IndexSignature(sig) => public::ClassMember::TSIndexSignature(
            convert_index_signature(sig, source, loc, interner, offset),
        ),
    }
}

fn convert_index_signature<'src>(
    sig: &internal::TSIndexSignature<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSIndexSignature<'src> {
    public::TSIndexSignature {
        node_type: "TSIndexSignature",
        start: sig.span.start,
        end: sig.span.end,
        loc: create_location(sig.span, loc, offset),
        parameters: sig
            .parameters
            .iter()
            .map(|p| public::Identifier {
                node_type: "Identifier",
                start: p.span.start,
                end: p.span.end,
                loc: create_location(p.span, loc, offset),
                name: public::name_cow(p.span, source, p.name, interner),
                optional: p.optional,
                type_annotation: p.type_annotation().map(|ta| {
                    convert_type_annotation_from_types(ta, source, loc, interner, offset)
                }),
                decorators: Vec::new(),
            })
            .collect(),
        type_annotation: convert_type_annotation_from_types(
            &sig.type_annotation,
            source,
            loc,
            interner,
            offset,
        ),
        is_static: sig.is_static,
        readonly: sig.readonly,
    }
}

fn convert_static_block<'src>(
    block: &internal::StaticBlock<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::StaticBlock<'src> {
    public::StaticBlock {
        node_type: "StaticBlock",
        start: block.span.start,
        end: block.span.end,
        loc: create_location(block.span, loc, offset),
        body: block
            .body
            .iter()
            // StaticBlock is always in TypeScript class context
            .map(|s| convert_statement(s, source, loc, interner, offset, Schema::Acorn))
            .collect(),
    }
}

fn convert_method_definition<'src>(
    method: &internal::MethodDefinition<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::MethodDefinition<'src> {
    // Convert the FunctionExpression/TSDeclareMethod value for the method
    // Note: typeParameters is placed on MethodDefinition, not FunctionExpression (acorn convention)
    let func = &method.value;
    let type_parameters = func
        .type_parameters
        .as_ref()
        .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner, offset));

    let params: Vec<_> = func
        .params
        .iter()
        .map(|p| convert_expression(p, source, loc, interner, offset))
        .collect();
    let return_type = func
        .return_type
        .as_ref()
        .map(|rt| convert_type_annotation(rt, source, loc, interner, offset));
    let id = func.id.as_ref().map(|id| public::Identifier {
        node_type: "Identifier",
        start: id.span.start,
        end: id.span.end,
        loc: create_location(id.span, loc, offset),
        name: public::name_cow(id.span, source, id.name, interner),
        optional: id.optional,
        type_annotation: None,
        decorators: Vec::new(),
    });

    // Abstract methods and overload signatures emit TSDeclareMethod (no body)
    // Detect by: abstract flag OR empty body with zero-width span (synthetic body)
    let is_bodyless = method.r#abstract
        || (func.body.body.is_empty() && func.body.span.start == func.body.span.end);
    // Value span starts at params_start (the `(`) not at the method keyword
    let value_span = Span::new(func.params_start, func.span.end);
    let value = if is_bodyless {
        public::MethodValue::TSDeclareMethod(public::TSDeclareMethod {
            node_type: "TSDeclareMethod",
            start: func.params_start,
            end: func.span.end,
            loc: create_location(value_span, loc, offset),
            id,
            expression: false,
            generator: func.generator,
            is_async: func.r#async,
            params,
            return_type,
        })
    } else {
        public::MethodValue::FunctionExpression(public::FunctionExpression {
            node_type: "FunctionExpression",
            start: func.params_start,
            end: func.span.end,
            loc: create_location(value_span, loc, offset),
            id,
            expression: false,
            generator: func.generator,
            is_async: func.r#async,
            type_parameters: None, // Moved to MethodDefinition
            params,
            return_type,
            body: convert_block_statement(&func.body, source, loc, interner, offset),
        })
    };

    public::MethodDefinition {
        node_type: "MethodDefinition",
        start: method.span.start,
        end: method.span.end,
        loc: create_location(method.span, loc, offset),
        decorators: method.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner, offset))
                .collect()
        }),
        accessibility: method.accessibility.map(internal::Accessibility::as_str),
        is_abstract: method.r#abstract.then_some(true),
        is_static: method.is_static,
        is_override: method.r#override,
        optional: method.optional.then_some(true),
        computed: method.computed,
        key: Box::new(convert_expression(
            &method.key,
            source,
            loc,
            interner,
            offset,
        )),
        kind: method.kind.as_str(),
        type_parameters,
        value,
    }
}

fn convert_property_definition<'src>(
    prop: &internal::PropertyDefinition<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::PropertyDefinition<'src> {
    public::PropertyDefinition {
        node_type: "PropertyDefinition",
        start: prop.span.start,
        end: prop.span.end,
        loc: create_location(prop.span, loc, offset),
        decorators: prop.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner, offset))
                .collect()
        }),
        is_abstract: prop.r#abstract.then_some(true),
        accessor: prop.accessor.then_some(true),
        accessibility: prop.accessibility.map(internal::Accessibility::as_str),
        readonly: prop.readonly.then_some(true),
        r#override: prop.r#override.then_some(true),
        declare: prop.declare.then_some(true),
        is_static: prop.is_static,
        computed: prop.computed,
        key: Box::new(convert_expression(&prop.key, source, loc, interner, offset)),
        optional: matches!(prop.modifier, internal::PropertyModifier::Optional).then_some(true),
        definite: matches!(prop.modifier, internal::PropertyModifier::Definite).then_some(true),
        type_annotation: prop
            .type_annotation
            .as_ref()
            .map(|ta| convert_type_annotation(ta, source, loc, interner, offset)),
        value: prop
            .value
            .as_ref()
            .map(|v| Box::new(convert_expression(v, source, loc, interner, offset))),
    }
}

/// Convert type parameter declaration: `<T extends U = V>`
pub(in crate::ast) fn convert_type_parameter_declaration<'src>(
    params: &internal::TSTypeParameterDeclaration<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeParameterDeclaration<'src> {
    public::TSTypeParameterDeclaration {
        node_type: "TSTypeParameterDeclaration",
        start: params.span.start,
        end: params.span.end,
        loc: create_location(params.span, loc, offset),
        params: params
            .params
            .iter()
            .map(|p| convert_type_parameter(p, source, loc, interner, offset))
            .collect(),
        extra: params
            .trailing_comma
            .map(|pos| public::TSTypeParameterExtra {
                trailing_comma: pos + offset as u32,
            }),
    }
}

/// Convert single type parameter: `T extends U = V`
pub(in crate::ast) fn convert_type_parameter<'src>(
    param: &internal::TSTypeParameter<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeParameter<'src> {
    public::TSTypeParameter {
        node_type: "TSTypeParameter",
        start: param.span.start,
        end: param.span.end,
        loc: create_location(param.span, loc, offset),
        is_const: param.is_const,
        is_in: param.is_in,
        is_out: param.is_out,
        name: public::name_cow(param.name.span, source, param.name.name, interner),
        constraint: param
            .constraint
            .as_ref()
            .map(|c| Box::new(convert_type(c, source, loc, interner, offset))),
        default: param
            .default
            .as_ref()
            .map(|d| Box::new(convert_type(d, source, loc, interner, offset))),
    }
}

/// Convert type parameter instantiation: `<T, U>`
pub(in crate::ast) fn convert_type_parameter_instantiation<'src>(
    params: &internal::TSTypeParameterInstantiation<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeParameterInstantiation<'src> {
    public::TSTypeParameterInstantiation {
        node_type: "TSTypeParameterInstantiation",
        start: params.span.start,
        end: params.span.end,
        loc: create_location(params.span, loc, offset),
        params: params
            .params
            .iter()
            .map(|p| convert_type(p, source, loc, interner, offset))
            .collect(),
    }
}

/// Convert TSInterfaceHeritage to TSExpressionWithTypeArguments (for implements clause)
fn convert_expression_with_type_arguments<'src>(
    heritage: &internal::TSInterfaceHeritage<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSExpressionWithTypeArguments<'src> {
    // Convert TSEntityName to Expression (specifically Identifier)
    let expression =
        convert_entity_name_to_expression(&heritage.expression, source, loc, interner, offset);

    public::TSExpressionWithTypeArguments {
        node_type: "TSExpressionWithTypeArguments",
        start: heritage.span.start,
        end: heritage.span.end,
        loc: create_location(heritage.span, loc, offset),
        expression,
        type_parameters: heritage
            .type_arguments
            .as_ref()
            .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner, offset)),
    }
}

/// Convert TSEntityName to Expression (Identifier or MemberExpression)
fn convert_entity_name_to_expression<'src>(
    entity: &internal::TSEntityName<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Expression<'src> {
    match entity {
        internal::TSEntityName::Identifier(id) => {
            public::Expression::Identifier(public::Identifier {
                node_type: "Identifier",
                start: id.span.start,
                end: id.span.end,
                loc: create_location(id.span, loc, offset),
                name: public::name_cow(id.span, source, id.name, interner),
                optional: id.optional,
                type_annotation: None,
                decorators: Vec::new(),
            })
        }
        internal::TSEntityName::QualifiedName(qn) => {
            // For qualified names like Foo.Bar, we convert to MemberExpression
            let object = convert_entity_name_to_expression(&qn.left, source, loc, interner, offset);
            public::Expression::MemberExpression(public::MemberExpression {
                node_type: "MemberExpression",
                start: qn.span.start,
                end: qn.span.end,
                loc: create_location(qn.span, loc, offset),
                object: Box::new(object),
                property: Box::new(public::Expression::Identifier(public::Identifier {
                    node_type: "Identifier",
                    start: qn.right.span.start,
                    end: qn.right.span.end,
                    loc: create_location(qn.right.span, loc, offset),
                    name: public::name_cow(qn.right.span, source, qn.right.name, interner),
                    optional: qn.right.optional,
                    type_annotation: None,
                    decorators: Vec::new(),
                })),
                computed: false,
                optional: Some(false),
            })
        }
    }
}
