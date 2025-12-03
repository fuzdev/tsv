// Import and export specifier conversions

use super::super::{internal, public};
use super::{convert_expression, convert_identifier, create_location};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};

pub(in crate::ast) fn convert_import_specifier(
    spec: &internal::ImportSpecifier,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
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
                },
            })
        }
        internal::ImportSpecifier::Named(named_spec) => {
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
                },
                import_kind: match named_spec.import_kind {
                    internal::ImportKind::Value => "value".to_string(),
                    internal::ImportKind::Type => "type".to_string(),
                },
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
        },
        value: convert_literal(&attr.value, source, loc, offset),
    }
}

pub(in crate::ast) fn convert_export_specifier(
    spec: &internal::ExportSpecifier,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ExportSpecifier {
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
        },
        exported: public::Identifier {
            node_type: "Identifier".to_string(),
            start: spec.exported.span.start,
            end: spec.exported.span.end,
            loc: create_location(spec.exported.span, loc, offset),
            name: interner.resolve_infallible(spec.exported.name).to_string(),
            optional: false,
            type_annotation: None,
        },
        // TODO: Support "type" for TypeScript `export type { T }`
        export_kind: "value".to_string(),
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
        internal::ExportDefaultValue::ClassDeclaration(class) => {
            public::ExportDefaultValue::ClassDeclaration(convert_class_declaration(
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
    let value = match &lit.value {
        internal::LiteralValue::Number(n) => serde_json::Value::Number(
            serde_json::Number::from_f64(*n).unwrap_or_else(|| serde_json::Number::from(0)),
        ),
        internal::LiteralValue::String { content, .. } => {
            serde_json::Value::String(content.clone())
        }
        internal::LiteralValue::Boolean(b) => serde_json::Value::Bool(*b),
        internal::LiteralValue::Null => serde_json::Value::Null,
        internal::LiteralValue::Undefined => serde_json::Value::Null,
    };
    let raw = lit.span.extract(source);
    public::Literal {
        node_type: "Literal".to_string(),
        start: lit.span.start,
        end: lit.span.end,
        loc: create_location(lit.span, loc, offset),
        value,
        raw: raw.to_string(),
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
    use super::{convert_block_statement, convert_type_annotation};

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

// Helper for export default class conversion
fn convert_class_declaration(
    class: &internal::ClassDeclaration,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassDeclaration {
    use super::convert_class_body;

    public::ClassDeclaration {
        node_type: "ClassDeclaration".to_string(),
        start: class.span.start,
        end: class.span.end,
        loc: create_location(class.span, loc, offset),
        id: class
            .id
            .as_ref()
            .map(|id| convert_identifier(id, loc, interner, offset)),
        super_class: class
            .super_class
            .as_ref()
            .map(|e| Box::new(convert_expression(e, source, loc, interner, offset))),
        body: convert_class_body(&class.body, source, loc, interner, offset),
    }
}
