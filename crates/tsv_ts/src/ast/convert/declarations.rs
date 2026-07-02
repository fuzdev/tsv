// Type alias, function, and class declaration conversions

use super::super::{internal, public};
use super::types::convert_type_annotation as convert_type_annotation_from_types;
use super::{
    Schema, convert_block_statement, convert_expression, convert_identifier, convert_statement,
    convert_type, convert_type_annotation, create_location,
};
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationMapper, Span};

/// Convert a decorator from internal to public AST
pub(super) fn convert_decorator<'src>(
    decorator: &internal::Decorator<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::Decorator<'src> {
    let mut expression = convert_expression(&decorator.expression, source, loc, interner);
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
        start: loc.pos(decorator.span.start),
        end: loc.pos(decorator.span.end),
        loc: create_location(decorator.span, loc),
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
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSTypeAliasDeclaration<'src> {
    public::TSTypeAliasDeclaration {
        node_type: "TSTypeAliasDeclaration",
        start: loc.pos(type_alias.span.start),
        end: loc.pos(type_alias.span.end),
        loc: create_location(type_alias.span, loc),
        id: convert_identifier(&type_alias.id, source, loc, interner),
        type_parameters: type_alias
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner)),
        type_annotation: convert_type(&type_alias.type_annotation, source, loc, interner),
        declare: type_alias.declare,
    }
}

pub(in crate::ast) fn convert_function_declaration<'src>(
    func_decl: &internal::FunctionDeclaration<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::FunctionDeclaration<'src> {
    public::FunctionDeclaration {
        node_type: "FunctionDeclaration",
        start: loc.pos(func_decl.span.start),
        end: loc.pos(func_decl.span.end),
        loc: create_location(func_decl.span, loc),
        id: func_decl
            .id
            .as_ref()
            .map(|id| convert_identifier(id, source, loc, interner)),
        expression: false,
        generator: func_decl.generator,
        is_async: func_decl.r#async,
        type_parameters: func_decl
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner)),
        params: func_decl
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner))
            .collect(),
        return_type: func_decl
            .return_type
            .as_ref()
            .map(|rt| convert_type_annotation(rt, source, loc, interner)),
        body: convert_block_statement(&func_decl.body, source, loc, interner),
    }
}

pub(in crate::ast) fn convert_class_declaration<'src>(
    class_decl: &internal::ClassDeclaration<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ClassDeclaration<'src> {
    let mut super_class = class_decl
        .super_class
        .as_ref()
        .map(|e| Box::new(convert_expression(e, source, loc, interner)));
    let mut super_type_parameters = class_decl
        .super_type_parameters
        .as_ref()
        .map(|tp| convert_type_parameter_instantiation(tp, source, loc, interner));
    maybe_wrap_super_class(
        &mut super_class,
        &mut super_type_parameters,
        class_decl.type_parameters.as_ref().map(|tp| tp.span),
        class_decl.super_class.as_ref().map(|e| converted_start(e)),
        class_decl
            .super_type_parameters
            .as_ref()
            .map(|tp| tp.span.end),
        source,
        loc,
    );

    public::ClassDeclaration {
        node_type: "ClassDeclaration",
        start: loc.pos(class_decl.span.start),
        end: loc.pos(class_decl.span.end),
        loc: create_location(class_decl.span, loc),
        decorators: class_decl.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner))
                .collect()
        }),
        declare: class_decl.declare.then_some(true),
        abstract_: class_decl.r#abstract.then_some(true),
        id: class_decl
            .id
            .as_ref()
            .map(|id| convert_identifier(id, source, loc, interner)),
        type_parameters: class_decl
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner)),
        super_class,
        super_type_parameters,
        implements: if class_decl.implements.is_empty() {
            None
        } else {
            Some(
                class_decl
                    .implements
                    .iter()
                    .map(|h| convert_expression_with_type_arguments(h, source, loc, interner))
                    .collect(),
            )
        },
        body: convert_class_body(&class_decl.body, source, loc, interner),
    }
}

pub(in crate::ast) fn convert_class_expression<'src>(
    class_expr: &internal::ClassExpression<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ClassExpression<'src> {
    let mut super_class = class_expr
        .super_class
        .as_ref()
        .map(|e| Box::new(convert_expression(e, source, loc, interner)));
    let mut super_type_parameters = class_expr
        .super_type_parameters
        .as_ref()
        .map(|tp| convert_type_parameter_instantiation(tp, source, loc, interner));
    maybe_wrap_super_class(
        &mut super_class,
        &mut super_type_parameters,
        class_expr.type_parameters.as_ref().map(|tp| tp.span),
        class_expr.super_class.as_ref().map(|e| converted_start(e)),
        class_expr
            .super_type_parameters
            .as_ref()
            .map(|tp| tp.span.end),
        source,
        loc,
    );

    public::ClassExpression {
        node_type: "ClassExpression",
        start: loc.pos(class_expr.span.start),
        end: loc.pos(class_expr.span.end),
        loc: create_location(class_expr.span, loc),
        decorators: class_expr.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner))
                .collect()
        }),
        abstract_: class_expr.r#abstract.then_some(true),
        id: class_expr
            .id
            .as_ref()
            .map(|id| convert_identifier(id, source, loc, interner)),
        type_parameters: class_expr
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner)),
        super_class,
        super_type_parameters,
        implements: if class_expr.implements.is_empty() {
            None
        } else {
            Some(
                class_expr
                    .implements
                    .iter()
                    .map(|h| convert_expression_with_type_arguments(h, source, loc, interner))
                    .collect(),
            )
        },
        body: convert_class_body(&class_expr.body, source, loc, interner),
    }
}

/// Byte start of the public node `convert_expression` will emit for `e`.
///
/// A `JsdocCast` is internal-only and converts to its inner expression, so the
/// wrapper's own span never reaches the public AST — unwrap to match.
fn converted_start(e: &internal::Expression<'_>) -> u32 {
    let mut e = e;
    while let internal::Expression::JsdocCast(cast) = e {
        e = cast.inner;
    }
    e.span().start
}

/// acorn-typescript quirk: when `extends Base<T>` is on a different line from the
/// closing `>` of type parameters, superClass becomes a TSInstantiationExpression
/// wrapping the expression and type arguments. When on the same line, superClass
/// is the bare expression (Identifier, MemberExpression, etc.) with a separate
/// superTypeParameters field.
///
/// `super_class_start` / `super_type_parameters_end` are *byte* offsets taken
/// from the internal AST — the converted nodes may already be in char space
/// under a fused mapper, and the same-line scan byte-indexes `source`.
fn maybe_wrap_super_class<'src>(
    super_class: &mut Option<Box<public::Expression<'src>>>,
    super_type_parameters: &mut Option<public::TSTypeParameterInstantiation<'src>>,
    type_params_span: Option<Span>,
    super_class_start: Option<u32>,
    super_type_parameters_end: Option<u32>,
    source: &str,
    loc: LocationMapper<'_>,
) {
    let Some(tp_span) = type_params_span else {
        return;
    };
    let (Some(sc_start), Some(stp_end)) = (super_class_start, super_type_parameters_end) else {
        return;
    };

    // Only wrap when extends is on a different line from >
    if tsv_lang::printing::is_same_line(source, tp_span.end, sc_start) {
        return;
    }

    // Wrap: superClass becomes TSInstantiationExpression, superTypeParameters is consumed
    let combined_span = Span::new(sc_start, stp_end);
    let Some(inner) = super_class.take() else {
        return;
    };
    let Some(type_arguments) = super_type_parameters.take() else {
        return;
    };
    *super_class = Some(Box::new(public::Expression::TSInstantiationExpression(
        public::TSInstantiationExpression {
            node_type: "TSInstantiationExpression",
            start: loc.pos(combined_span.start),
            end: loc.pos(combined_span.end),
            loc: create_location(combined_span, loc),
            expression: inner,
            type_arguments,
        },
    )));
}

pub(in crate::ast) fn convert_class_body<'src>(
    body: &internal::ClassBody<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ClassBody<'src> {
    public::ClassBody {
        node_type: "ClassBody",
        start: loc.pos(body.span.start),
        end: loc.pos(body.span.end),
        loc: create_location(body.span, loc),
        body: body
            .body
            .iter()
            .map(|m| convert_class_member(m, source, loc, interner))
            .collect(),
    }
}

fn convert_class_member<'src>(
    member: &internal::ClassMember<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ClassMember<'src> {
    match member {
        internal::ClassMember::MethodDefinition(method) => public::ClassMember::MethodDefinition(
            convert_method_definition(method, source, loc, interner),
        ),
        internal::ClassMember::PropertyDefinition(prop) => public::ClassMember::PropertyDefinition(
            convert_property_definition(prop, source, loc, interner),
        ),
        internal::ClassMember::StaticBlock(block) => {
            public::ClassMember::StaticBlock(convert_static_block(block, source, loc, interner))
        }
        internal::ClassMember::IndexSignature(sig) => public::ClassMember::TSIndexSignature(
            convert_index_signature(sig, source, loc, interner),
        ),
    }
}

fn convert_index_signature<'src>(
    sig: &internal::TSIndexSignature<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSIndexSignature<'src> {
    public::TSIndexSignature {
        node_type: "TSIndexSignature",
        start: loc.pos(sig.span.start),
        end: loc.pos(sig.span.end),
        loc: create_location(sig.span, loc),
        parameters: sig
            .parameters
            .iter()
            .map(|p| public::Identifier {
                node_type: "Identifier",
                start: loc.pos(p.span.start),
                end: loc.pos(p.span.end),
                loc: create_location(p.span, loc),
                name: public::name_cow(p.span, source, p.name, interner),
                optional: p.optional,
                type_annotation: p
                    .type_annotation()
                    .map(|ta| convert_type_annotation_from_types(ta, source, loc, interner)),
                decorators: Vec::new(),
            })
            .collect(),
        type_annotation: convert_type_annotation_from_types(
            &sig.type_annotation,
            source,
            loc,
            interner,
        ),
        is_static: sig.is_static,
        readonly: sig.readonly,
    }
}

fn convert_static_block<'src>(
    block: &internal::StaticBlock<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::StaticBlock<'src> {
    public::StaticBlock {
        node_type: "StaticBlock",
        start: loc.pos(block.span.start),
        end: loc.pos(block.span.end),
        loc: create_location(block.span, loc),
        body: block
            .body
            .iter()
            // StaticBlock is always in TypeScript class context
            .map(|s| convert_statement(s, source, loc, interner, Schema::Acorn))
            .collect(),
    }
}

fn convert_method_definition<'src>(
    method: &internal::MethodDefinition<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::MethodDefinition<'src> {
    // Convert the FunctionExpression/TSDeclareMethod value for the method
    // Note: typeParameters is placed on MethodDefinition, not FunctionExpression (acorn convention)
    let func = &method.value;
    let type_parameters = func
        .type_parameters
        .as_ref()
        .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner));

    let params: Vec<_> = func
        .params
        .iter()
        .map(|p| convert_expression(p, source, loc, interner))
        .collect();
    let return_type = func
        .return_type
        .as_ref()
        .map(|rt| convert_type_annotation(rt, source, loc, interner));
    let id = func.id.as_ref().map(|id| public::Identifier {
        node_type: "Identifier",
        start: loc.pos(id.span.start),
        end: loc.pos(id.span.end),
        loc: create_location(id.span, loc),
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
            start: loc.pos(func.params_start),
            end: loc.pos(func.span.end),
            loc: create_location(value_span, loc),
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
            start: loc.pos(func.params_start),
            end: loc.pos(func.span.end),
            loc: create_location(value_span, loc),
            id,
            expression: false,
            generator: func.generator,
            is_async: func.r#async,
            type_parameters: None, // Moved to MethodDefinition
            params,
            return_type,
            body: convert_block_statement(&func.body, source, loc, interner),
        })
    };

    public::MethodDefinition {
        node_type: "MethodDefinition",
        start: loc.pos(method.span.start),
        end: loc.pos(method.span.end),
        loc: create_location(method.span, loc),
        decorators: method.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner))
                .collect()
        }),
        accessibility: method.accessibility.map(internal::Accessibility::as_str),
        is_abstract: method.r#abstract.then_some(true),
        is_static: method.is_static,
        is_override: method.r#override,
        optional: method.optional.then_some(true),
        computed: method.computed,
        key: Box::new(convert_expression(&method.key, source, loc, interner)),
        kind: method.kind.as_str(),
        type_parameters,
        value,
    }
}

fn convert_property_definition<'src>(
    prop: &internal::PropertyDefinition<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::PropertyDefinition<'src> {
    public::PropertyDefinition {
        node_type: "PropertyDefinition",
        start: loc.pos(prop.span.start),
        end: loc.pos(prop.span.end),
        loc: create_location(prop.span, loc),
        decorators: prop.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner))
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
        key: Box::new(convert_expression(&prop.key, source, loc, interner)),
        optional: matches!(prop.modifier, internal::PropertyModifier::Optional).then_some(true),
        definite: matches!(prop.modifier, internal::PropertyModifier::Definite).then_some(true),
        type_annotation: prop
            .type_annotation
            .as_ref()
            .map(|ta| convert_type_annotation(ta, source, loc, interner)),
        value: prop
            .value
            .as_ref()
            .map(|v| Box::new(convert_expression(v, source, loc, interner))),
    }
}

/// Convert type parameter declaration: `<T extends U = V>`
pub(in crate::ast) fn convert_type_parameter_declaration<'src>(
    params: &internal::TSTypeParameterDeclaration<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSTypeParameterDeclaration<'src> {
    public::TSTypeParameterDeclaration {
        node_type: "TSTypeParameterDeclaration",
        start: loc.pos(params.span.start),
        end: loc.pos(params.span.end),
        loc: create_location(params.span, loc),
        params: params
            .params
            .iter()
            .map(|p| convert_type_parameter(p, source, loc, interner))
            .collect(),
        extra: params
            .trailing_comma
            .map(|pos| public::TSTypeParameterExtra {
                // Emitted like `start`/`end`: acorn reports it in UTF-16 units.
                trailing_comma: loc.pos(pos),
            }),
    }
}

/// Convert single type parameter: `T extends U = V`
pub(in crate::ast) fn convert_type_parameter<'src>(
    param: &internal::TSTypeParameter<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSTypeParameter<'src> {
    public::TSTypeParameter {
        node_type: "TSTypeParameter",
        start: loc.pos(param.span.start),
        end: loc.pos(param.span.end),
        loc: create_location(param.span, loc),
        is_const: param.is_const,
        is_in: param.is_in,
        is_out: param.is_out,
        name: public::name_cow(param.name.span, source, param.name.name, interner),
        constraint: param
            .constraint
            .as_ref()
            .map(|c| Box::new(convert_type(c, source, loc, interner))),
        default: param
            .default
            .as_ref()
            .map(|d| Box::new(convert_type(d, source, loc, interner))),
    }
}

/// Convert type parameter instantiation: `<T, U>`
pub(in crate::ast) fn convert_type_parameter_instantiation<'src>(
    params: &internal::TSTypeParameterInstantiation<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSTypeParameterInstantiation<'src> {
    public::TSTypeParameterInstantiation {
        node_type: "TSTypeParameterInstantiation",
        start: loc.pos(params.span.start),
        end: loc.pos(params.span.end),
        loc: create_location(params.span, loc),
        params: params
            .params
            .iter()
            .map(|p| convert_type(p, source, loc, interner))
            .collect(),
    }
}

/// Convert TSInterfaceHeritage to TSExpressionWithTypeArguments (for implements clause)
fn convert_expression_with_type_arguments<'src>(
    heritage: &internal::TSInterfaceHeritage<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSExpressionWithTypeArguments<'src> {
    // Convert TSEntityName to Expression (specifically Identifier)
    let expression = convert_entity_name_to_expression(&heritage.expression, source, loc, interner);

    public::TSExpressionWithTypeArguments {
        node_type: "TSExpressionWithTypeArguments",
        start: loc.pos(heritage.span.start),
        end: loc.pos(heritage.span.end),
        loc: create_location(heritage.span, loc),
        expression,
        type_parameters: heritage
            .type_arguments
            .as_ref()
            .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner)),
    }
}

/// Convert TSEntityName to Expression (Identifier or MemberExpression)
fn convert_entity_name_to_expression<'src>(
    entity: &internal::TSEntityName<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::Expression<'src> {
    match entity {
        internal::TSEntityName::Identifier(id) => {
            public::Expression::Identifier(public::Identifier {
                node_type: "Identifier",
                start: loc.pos(id.span.start),
                end: loc.pos(id.span.end),
                loc: create_location(id.span, loc),
                name: public::name_cow(id.span, source, id.name, interner),
                optional: id.optional,
                type_annotation: None,
                decorators: Vec::new(),
            })
        }
        internal::TSEntityName::QualifiedName(qn) => {
            // For qualified names like Foo.Bar, we convert to MemberExpression
            let object = convert_entity_name_to_expression(&qn.left, source, loc, interner);
            public::Expression::MemberExpression(public::MemberExpression {
                node_type: "MemberExpression",
                start: loc.pos(qn.span.start),
                end: loc.pos(qn.span.end),
                loc: create_location(qn.span, loc),
                object: Box::new(object),
                property: Box::new(public::Expression::Identifier(public::Identifier {
                    node_type: "Identifier",
                    start: loc.pos(qn.right.span.start),
                    end: loc.pos(qn.right.span.end),
                    loc: create_location(qn.right.span, loc),
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
