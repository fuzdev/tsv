// Type alias, function, and class declaration conversions

use super::super::{internal, public};
use super::{
    convert_block_statement, convert_expression, convert_type, convert_type_annotation,
    create_location,
};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};

/// Convert a decorator from internal to public AST
pub(super) fn convert_decorator(
    decorator: &internal::Decorator,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Decorator {
    public::Decorator {
        node_type: "Decorator".to_string(),
        start: decorator.span.start,
        end: decorator.span.end,
        loc: create_location(decorator.span, loc, offset),
        expression: convert_expression(&decorator.expression, source, loc, interner, offset),
    }
}

pub(in crate::ast) fn convert_type_alias_declaration(
    type_alias: &internal::TSTypeAliasDeclaration,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeAliasDeclaration {
    public::TSTypeAliasDeclaration {
        node_type: "TSTypeAliasDeclaration".to_string(),
        start: type_alias.span.start,
        end: type_alias.span.end,
        loc: create_location(type_alias.span, loc, offset),
        id: public::Identifier {
            node_type: "Identifier".to_string(),
            start: type_alias.id.span.start,
            end: type_alias.id.span.end,
            loc: create_location(type_alias.id.span, loc, offset),
            name: interner.resolve_infallible(type_alias.id.name).to_string(),
            optional: false,
            type_annotation: None,
            decorators: Vec::new(),
        },
        type_annotation: convert_type(&type_alias.type_annotation, source, loc, interner, offset),
    }
}

pub(in crate::ast) fn convert_function_declaration(
    func_decl: &internal::FunctionDeclaration,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::FunctionDeclaration {
    public::FunctionDeclaration {
        node_type: "FunctionDeclaration".to_string(),
        start: func_decl.span.start,
        end: func_decl.span.end,
        loc: create_location(func_decl.span, loc, offset),
        id: func_decl.id.as_ref().map(|id| public::Identifier {
            node_type: "Identifier".to_string(),
            start: id.span.start,
            end: id.span.end,
            loc: create_location(id.span, loc, offset),
            name: interner.resolve_infallible(id.name).to_string(),
            optional: false,
            type_annotation: None,
            decorators: Vec::new(),
        }),
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

pub(in crate::ast) fn convert_class_declaration(
    class_decl: &internal::ClassDeclaration,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassDeclaration {
    public::ClassDeclaration {
        node_type: "ClassDeclaration".to_string(),
        start: class_decl.span.start,
        end: class_decl.span.end,
        loc: create_location(class_decl.span, loc, offset),
        decorators: class_decl.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner, offset))
                .collect()
        }),
        declare: if class_decl.declare { Some(true) } else { None },
        id: class_decl.id.as_ref().map(|id| public::Identifier {
            node_type: "Identifier".to_string(),
            start: id.span.start,
            end: id.span.end,
            loc: create_location(id.span, loc, offset),
            name: interner.resolve_infallible(id.name).to_string(),
            optional: false,
            type_annotation: None,
            decorators: Vec::new(),
        }),
        type_parameters: class_decl
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner, offset)),
        super_class: class_decl
            .super_class
            .as_ref()
            .map(|e| Box::new(convert_expression(e, source, loc, interner, offset))),
        super_type_parameters: class_decl
            .super_type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_instantiation(tp, source, loc, interner, offset)),
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

pub(in crate::ast) fn convert_class_expression(
    class_expr: &internal::ClassExpression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassExpression {
    public::ClassExpression {
        node_type: "ClassExpression".to_string(),
        start: class_expr.span.start,
        end: class_expr.span.end,
        loc: create_location(class_expr.span, loc, offset),
        decorators: class_expr.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner, offset))
                .collect()
        }),
        id: class_expr.id.as_ref().map(|id| public::Identifier {
            node_type: "Identifier".to_string(),
            start: id.span.start,
            end: id.span.end,
            loc: create_location(id.span, loc, offset),
            name: interner.resolve_infallible(id.name).to_string(),
            optional: false,
            type_annotation: None,
            decorators: Vec::new(),
        }),
        type_parameters: class_expr
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration(tp, source, loc, interner, offset)),
        super_class: class_expr
            .super_class
            .as_ref()
            .map(|e| Box::new(convert_expression(e, source, loc, interner, offset))),
        super_type_parameters: class_expr
            .super_type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_instantiation(tp, source, loc, interner, offset)),
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

pub(in crate::ast) fn convert_class_body(
    body: &internal::ClassBody,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassBody {
    public::ClassBody {
        node_type: "ClassBody".to_string(),
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

fn convert_class_member(
    member: &internal::ClassMember,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassMember {
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

fn convert_index_signature(
    sig: &internal::TSIndexSignature,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSIndexSignature {
    use super::types::convert_type_annotation;

    public::TSIndexSignature {
        node_type: "TSIndexSignature".to_string(),
        start: sig.span.start,
        end: sig.span.end,
        loc: create_location(sig.span, loc, offset),
        parameters: sig
            .parameters
            .iter()
            .map(|p| {
                let name = interner
                    .resolve(p.name)
                    .map_or_else(String::new, str::to_string);
                public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: p.span.start,
                    end: p.span.end,
                    loc: create_location(p.span, loc, offset),
                    name,
                    optional: p.optional,
                    type_annotation: p
                        .type_annotation
                        .as_ref()
                        .map(|ta| convert_type_annotation(ta, source, loc, interner, offset)),
                    decorators: Vec::new(),
                }
            })
            .collect(),
        type_annotation: convert_type_annotation(
            &sig.type_annotation,
            source,
            loc,
            interner,
            offset,
        ),
        readonly: sig.readonly,
    }
}

fn convert_static_block(
    block: &internal::StaticBlock,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::StaticBlock {
    use super::convert_statement;

    public::StaticBlock {
        node_type: "StaticBlock".to_string(),
        start: block.span.start,
        end: block.span.end,
        loc: create_location(block.span, loc, offset),
        body: block
            .body
            .iter()
            .map(|s| convert_statement(s, source, loc, interner, offset))
            .collect(),
    }
}

fn convert_method_definition(
    method: &internal::MethodDefinition,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::MethodDefinition {
    // Convert the FunctionExpression value for the method
    let func = &method.value;
    let value = public::FunctionExpression {
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
    };

    public::MethodDefinition {
        node_type: "MethodDefinition".to_string(),
        start: method.span.start,
        end: method.span.end,
        loc: create_location(method.span, loc, offset),
        decorators: method.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner, offset))
                .collect()
        }),
        accessibility: method.accessibility.map(|a| a.as_str().to_string()),
        is_static: method.is_static,
        is_override: method.r#override,
        computed: method.computed,
        key: Box::new(convert_expression(
            &method.key,
            source,
            loc,
            interner,
            offset,
        )),
        kind: method.kind.as_str().to_string(),
        value,
    }
}

fn convert_property_definition(
    prop: &internal::PropertyDefinition,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::PropertyDefinition {
    public::PropertyDefinition {
        node_type: "PropertyDefinition".to_string(),
        start: prop.span.start,
        end: prop.span.end,
        loc: create_location(prop.span, loc, offset),
        decorators: prop.decorators.as_ref().map(|decs| {
            decs.iter()
                .map(|d| convert_decorator(d, source, loc, interner, offset))
                .collect()
        }),
        accessor: if prop.accessor { Some(true) } else { None },
        accessibility: prop.accessibility.map(|a| a.as_str().to_string()),
        readonly: if prop.readonly { Some(true) } else { None },
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
pub(in crate::ast) fn convert_type_parameter_declaration(
    params: &internal::TSTypeParameterDeclaration,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeParameterDeclaration {
    public::TSTypeParameterDeclaration {
        node_type: "TSTypeParameterDeclaration".to_string(),
        start: params.span.start,
        end: params.span.end,
        loc: create_location(params.span, loc, offset),
        params: params
            .params
            .iter()
            .map(|p| convert_type_parameter(p, source, loc, interner, offset))
            .collect(),
    }
}

/// Convert single type parameter: `T extends U = V`
fn convert_type_parameter(
    param: &internal::TSTypeParameter,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeParameter {
    public::TSTypeParameter {
        node_type: "TSTypeParameter".to_string(),
        start: param.span.start,
        end: param.span.end,
        loc: create_location(param.span, loc, offset),
        is_const: param.is_const,
        is_in: param.is_in,
        is_out: param.is_out,
        name: public::Identifier {
            node_type: "Identifier".to_string(),
            start: param.name.span.start,
            end: param.name.span.end,
            loc: create_location(param.name.span, loc, offset),
            name: interner.resolve_infallible(param.name.name).to_string(),
            optional: false,
            type_annotation: None,
            decorators: Vec::new(),
        },
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
pub(in crate::ast) fn convert_type_parameter_instantiation(
    params: &internal::TSTypeParameterInstantiation,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeParameterInstantiation {
    public::TSTypeParameterInstantiation {
        node_type: "TSTypeParameterInstantiation".to_string(),
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
fn convert_expression_with_type_arguments(
    heritage: &internal::TSInterfaceHeritage,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSExpressionWithTypeArguments {
    // Convert TSEntityName to Expression (specifically Identifier)
    let expression = convert_entity_name_to_expression(&heritage.expression, loc, interner, offset);

    public::TSExpressionWithTypeArguments {
        node_type: "TSExpressionWithTypeArguments".to_string(),
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
fn convert_entity_name_to_expression(
    entity: &internal::TSEntityName,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Expression {
    match entity {
        internal::TSEntityName::Identifier(id) => {
            public::Expression::Identifier(public::Identifier {
                node_type: "Identifier".to_string(),
                start: id.span.start,
                end: id.span.end,
                loc: create_location(id.span, loc, offset),
                name: interner.resolve_infallible(id.name).to_string(),
                optional: id.optional,
                type_annotation: None,
                decorators: Vec::new(),
            })
        }
        internal::TSEntityName::QualifiedName(qn) => {
            // For qualified names like Foo.Bar, we convert to MemberExpression
            let object = convert_entity_name_to_expression(&qn.left, loc, interner, offset);
            public::Expression::MemberExpression(public::MemberExpression {
                node_type: "MemberExpression".to_string(),
                start: qn.span.start,
                end: qn.span.end,
                loc: create_location(qn.span, loc, offset),
                object: Box::new(object),
                property: Box::new(public::Expression::Identifier(public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: qn.right.span.start,
                    end: qn.right.span.end,
                    loc: create_location(qn.right.span, loc, offset),
                    name: interner.resolve_infallible(qn.right.name).to_string(),
                    optional: qn.right.optional,
                    type_annotation: None,
                    decorators: Vec::new(),
                })),
                computed: false,
                optional: false,
            })
        }
    }
}
