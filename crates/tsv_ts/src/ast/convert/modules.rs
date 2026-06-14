// Import and export specifier conversions

use super::super::{internal, public};
use super::types::convert_declare_function;
use super::{
    Schema, bigint_to_decimal, convert_block_statement, convert_class_declaration,
    convert_expression, convert_identifier, convert_type_annotation,
    convert_type_parameter_declaration, create_location, json_number_from_f64,
};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};

pub(in crate::ast) fn convert_import_specifier(
    spec: &internal::ImportSpecifier,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
    schema: Schema,
) -> public::ImportSpecifier {
    match spec {
        internal::ImportSpecifier::Default(default_spec) => {
            public::ImportSpecifier::Default(public::ImportDefaultSpecifier {
                node_type: "ImportDefaultSpecifier".to_string(),
                start: default_spec.span.start,
                end: default_spec.span.end,
                loc: create_location(default_spec.span, loc, offset),
                local: public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: default_spec.local.span.start,
                    end: default_spec.local.span.end,
                    loc: create_location(default_spec.local.span, loc, offset),
                    name: interner
                        .resolve_infallible(default_spec.local.name)
                        .to_string(),
                    optional: false,
                    type_annotation: None,
                    decorators: Vec::new(),
                },
            })
        }
        internal::ImportSpecifier::Named(named_spec) => {
            let import_kind = match named_spec.import_kind {
                internal::ImportKind::Value => {
                    if schema.is_svelte_script() {
                        None
                    } else {
                        Some("value".to_string())
                    }
                }
                internal::ImportKind::Type => Some("type".to_string()),
            };
            public::ImportSpecifier::Named(public::ImportNamedSpecifier {
                node_type: "ImportSpecifier".to_string(),
                start: named_spec.span.start,
                end: named_spec.span.end,
                loc: create_location(named_spec.span, loc, offset),
                imported: public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: named_spec.imported.span.start,
                    end: named_spec.imported.span.end,
                    loc: create_location(named_spec.imported.span, loc, offset),
                    name: interner
                        .resolve_infallible(named_spec.imported.name)
                        .to_string(),
                    optional: false,
                    type_annotation: None,
                    decorators: Vec::new(),
                },
                local: public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: named_spec.local.span.start,
                    end: named_spec.local.span.end,
                    loc: create_location(named_spec.local.span, loc, offset),
                    name: interner
                        .resolve_infallible(named_spec.local.name)
                        .to_string(),
                    optional: false,
                    type_annotation: None,
                    decorators: Vec::new(),
                },
                import_kind,
            })
        }
        internal::ImportSpecifier::Namespace(ns_spec) => {
            public::ImportSpecifier::Namespace(public::ImportNamespaceSpecifier {
                node_type: "ImportNamespaceSpecifier".to_string(),
                start: ns_spec.span.start,
                end: ns_spec.span.end,
                loc: create_location(ns_spec.span, loc, offset),
                local: public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: ns_spec.local.span.start,
                    end: ns_spec.local.span.end,
                    loc: create_location(ns_spec.local.span, loc, offset),
                    name: interner.resolve_infallible(ns_spec.local.name).to_string(),
                    optional: false,
                    type_annotation: None,
                    decorators: Vec::new(),
                },
            })
        }
    }
}

pub(in crate::ast) fn convert_import_attribute(
    attr: &internal::ImportAttribute,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ImportAttribute {
    public::ImportAttribute {
        node_type: "ImportAttribute".to_string(),
        start: attr.span.start,
        end: attr.span.end,
        loc: create_location(attr.span, loc, offset),
        key: public::Identifier {
            node_type: "Identifier".to_string(),
            start: attr.key.span.start,
            end: attr.key.span.end,
            loc: create_location(attr.key.span, loc, offset),
            name: interner.resolve_infallible(attr.key.name).to_string(),
            optional: false,
            type_annotation: None,
            decorators: Vec::new(),
        },
        value: convert_literal(&attr.value, source, loc, offset),
    }
}

pub(in crate::ast) fn convert_export_specifier(
    spec: &internal::ExportSpecifier,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
    schema: Schema,
) -> public::ExportSpecifier {
    let export_kind = match spec.export_kind {
        internal::ExportKind::Value => {
            if schema.is_svelte_script() {
                None
            } else {
                Some("value".to_string())
            }
        }
        internal::ExportKind::Type => Some("type".to_string()),
    };
    public::ExportSpecifier {
        node_type: "ExportSpecifier".to_string(),
        start: spec.span.start,
        end: spec.span.end,
        loc: create_location(spec.span, loc, offset),
        local: public::Identifier {
            node_type: "Identifier".to_string(),
            start: spec.local.span.start,
            end: spec.local.span.end,
            loc: create_location(spec.local.span, loc, offset),
            name: interner.resolve_infallible(spec.local.name).to_string(),
            optional: false,
            type_annotation: None,
            decorators: Vec::new(),
        },
        exported: public::Identifier {
            node_type: "Identifier".to_string(),
            start: spec.exported.span.start,
            end: spec.exported.span.end,
            loc: create_location(spec.exported.span, loc, offset),
            name: interner.resolve_infallible(spec.exported.name).to_string(),
            optional: false,
            type_annotation: None,
            decorators: Vec::new(),
        },
        export_kind,
    }
}

pub(in crate::ast) fn convert_export_default_value(
    value: &internal::ExportDefaultValue,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ExportDefaultValue {
    match value {
        internal::ExportDefaultValue::Expression(expr) => public::ExportDefaultValue::Expression(
            convert_expression(expr, source, loc, interner, offset),
        ),
        internal::ExportDefaultValue::FunctionDeclaration(func) => {
            public::ExportDefaultValue::FunctionDeclaration(convert_function_to_public(
                func, source, loc, interner, offset,
            ))
        }
        internal::ExportDefaultValue::TSDeclareFunction(func) => {
            public::ExportDefaultValue::TSDeclareFunction(convert_declare_function(
                func, source, loc, interner, offset,
            ))
        }
        internal::ExportDefaultValue::ClassDeclaration(class) => {
            public::ExportDefaultValue::ClassDeclaration(convert_class_declaration_local(
                class, source, loc, interner, offset,
            ))
        }
    }
}

// Helper to convert literal for import attributes and export sources
pub(in crate::ast) fn convert_literal(
    lit: &internal::Literal,
    source: &str,
    loc: &LocationTracker,
    offset: usize,
) -> public::Literal {
    let (value, bigint) = match &lit.value {
        internal::LiteralValue::Number(n) => {
            (serde_json::Value::Number(json_number_from_f64(*n)), None)
        }
        internal::LiteralValue::String { content, .. } => {
            (serde_json::Value::String(content.clone()), None)
        }
        internal::LiteralValue::BigInt(val) => {
            let decimal = bigint_to_decimal(val);
            (serde_json::Value::String(decimal.clone()), Some(decimal))
        }
        internal::LiteralValue::Boolean(b) => (serde_json::Value::Bool(*b), None),
        internal::LiteralValue::Null => (serde_json::Value::Null, None),
        internal::LiteralValue::Undefined => (serde_json::Value::Null, None),
    };
    let raw = lit.span.extract(source);
    public::Literal {
        node_type: "Literal".to_string(),
        start: lit.span.start,
        end: lit.span.end,
        loc: create_location(lit.span, loc, offset),
        value,
        raw: raw.to_string(),
        bigint,
    }
}

// Helper for export default value conversion
fn convert_function_to_public(
    func: &internal::FunctionDeclaration,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::FunctionDeclaration {
    public::FunctionDeclaration {
        node_type: "FunctionDeclaration".to_string(),
        start: func.span.start,
        end: func.span.end,
        loc: create_location(func.span, loc, offset),
        id: func
            .id
            .as_ref()
            .map(|id| convert_identifier(id, loc, interner, offset)),
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

// Helper for export default class conversion
fn convert_class_declaration_local(
    class: &internal::ClassDeclaration,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassDeclaration {
    // Delegate to the main converter in declarations.rs
    convert_class_declaration(class, source, loc, interner, offset)
}
