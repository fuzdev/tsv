// Type alias, function, and class declaration conversions

use super::super::{internal, public};
use super::{
    convert_block_statement, convert_expression, convert_type, convert_type_annotation,
    create_location,
};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};

pub(in crate::ast) fn convert_type_alias_declaration(
    type_alias: &internal::TSTypeAliasDeclaration,
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
        },
        type_annotation: convert_type(&type_alias.type_annotation, loc, offset),
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
        }),
        expression: false,
        generator: func_decl.generator,
        is_async: func_decl.r#async,
        params: func_decl
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner, offset))
            .collect(),
        return_type: func_decl
            .return_type
            .as_ref()
            .map(|rt| convert_type_annotation(rt, loc, offset)),
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
        id: class_decl.id.as_ref().map(|id| public::Identifier {
            node_type: "Identifier".to_string(),
            start: id.span.start,
            end: id.span.end,
            loc: create_location(id.span, loc, offset),
            name: interner.resolve_infallible(id.name).to_string(),
            optional: false,
            type_annotation: None,
        }),
        super_class: class_decl
            .super_class
            .as_ref()
            .map(|e| Box::new(convert_expression(e, source, loc, interner, offset))),
        body: convert_class_body(&class_decl.body, source, loc, interner, offset),
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
    };

    public::MethodDefinition {
        node_type: "MethodDefinition".to_string(),
        start: method.span.start,
        end: method.span.end,
        loc: create_location(method.span, loc, offset),
        is_static: method.is_static,
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
        is_static: prop.is_static,
        computed: prop.computed,
        key: Box::new(convert_expression(&prop.key, source, loc, interner, offset)),
        type_annotation: prop
            .type_annotation
            .as_ref()
            .map(|ta| convert_type_annotation(ta, loc, offset)),
        value: prop
            .value
            .as_ref()
            .map(|v| Box::new(convert_expression(v, source, loc, interner, offset))),
    }
}
