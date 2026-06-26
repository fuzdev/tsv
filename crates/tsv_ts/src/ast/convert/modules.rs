// Import and export specifier conversions

use super::super::{internal, public};
use super::types::convert_declare_function;
use super::{
    Schema, bigint_to_decimal, convert_block_statement, convert_class_declaration,
    convert_expression, convert_identifier, convert_type_annotation,
    convert_type_parameter_declaration, create_location, json_number_from_f64,
};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationTracker;

pub(in crate::ast) fn convert_import_specifier(
    spec: &internal::ImportSpecifier<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
    schema: Schema,
) -> public::ImportSpecifier {
    match spec {
        internal::ImportSpecifier::Default(default_spec) => {
            public::ImportSpecifier::Default(public::ImportDefaultSpecifier {
                node_type: "ImportDefaultSpecifier",
                start: default_spec.span.start,
                end: default_spec.span.end,
                loc: create_location(default_spec.span, loc, offset),
                local: convert_identifier(&default_spec.local, loc, interner, offset),
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
                node_type: "ImportSpecifier",
                start: named_spec.span.start,
                end: named_spec.span.end,
                loc: create_location(named_spec.span, loc, offset),
                imported: convert_module_export_name(
                    &named_spec.imported,
                    source,
                    loc,
                    interner,
                    offset,
                ),
                local: convert_identifier(&named_spec.local, loc, interner, offset),
                import_kind,
            })
        }
        internal::ImportSpecifier::Namespace(ns_spec) => {
            public::ImportSpecifier::Namespace(public::ImportNamespaceSpecifier {
                node_type: "ImportNamespaceSpecifier",
                start: ns_spec.span.start,
                end: ns_spec.span.end,
                loc: create_location(ns_spec.span, loc, offset),
                local: convert_identifier(&ns_spec.local, loc, interner, offset),
            })
        }
    }
}

pub(in crate::ast) fn convert_import_attribute(
    attr: &internal::ImportAttribute<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ImportAttribute {
    let key = match &attr.key {
        internal::ImportAttributeKey::Identifier(id) => {
            public::ImportAttributeKey::Identifier(convert_identifier(id, loc, interner, offset))
        }
        internal::ImportAttributeKey::Literal(lit) => {
            public::ImportAttributeKey::Literal(convert_literal(lit, source, loc, offset))
        }
    };
    public::ImportAttribute {
        node_type: "ImportAttribute",
        start: attr.span.start,
        end: attr.span.end,
        loc: create_location(attr.span, loc, offset),
        key,
        value: convert_literal(&attr.value, source, loc, offset),
    }
}

pub(in crate::ast) fn convert_export_specifier(
    spec: &internal::ExportSpecifier<'_>,
    source: &str,
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
        node_type: "ExportSpecifier",
        start: spec.span.start,
        end: spec.span.end,
        loc: create_location(spec.span, loc, offset),
        local: convert_module_export_name(&spec.local, source, loc, interner, offset),
        exported: convert_module_export_name(&spec.exported, source, loc, interner, offset),
        export_kind,
    }
}

/// Convert a `ModuleExportName` (import/export specifier name, or `export * as`
/// namespace name): an identifier emits an `Identifier` node, a string a
/// `Literal` node — mirroring acorn (`ModuleExportName : IdentifierName | StringLiteral`).
pub(in crate::ast) fn convert_module_export_name(
    name: &internal::ModuleExportName<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ModuleExportName {
    match name {
        internal::ModuleExportName::Identifier(id) => {
            public::ModuleExportName::Identifier(convert_identifier(id, loc, interner, offset))
        }
        internal::ModuleExportName::Literal(lit) => {
            public::ModuleExportName::Literal(convert_literal(lit, source, loc, offset))
        }
    }
}

pub(in crate::ast) fn convert_export_default_value(
    value: &internal::ExportDefaultValue<'_>,
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
    lit: &internal::Literal<'_>,
    source: &str,
    loc: &LocationTracker,
    offset: usize,
) -> public::Literal {
    let (value, bigint) = match &lit.value {
        internal::LiteralValue::Number(n) => {
            (serde_json::Value::Number(json_number_from_f64(*n)), None)
        }
        internal::LiteralValue::String(cooked) => (
            serde_json::Value::String(cooked.resolve(lit.span, source).to_string()),
            None,
        ),
        internal::LiteralValue::BigInt => {
            let decimal = bigint_to_decimal(lit.bigint_digits(source));
            (serde_json::Value::String(decimal.clone()), Some(decimal))
        }
        internal::LiteralValue::Boolean(b) => (serde_json::Value::Bool(*b), None),
        internal::LiteralValue::Null => (serde_json::Value::Null, None),
        internal::LiteralValue::Undefined => (serde_json::Value::Null, None),
    };
    let raw = lit.span.extract(source);
    public::Literal {
        node_type: "Literal",
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
    func: &internal::FunctionDeclaration<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::FunctionDeclaration {
    public::FunctionDeclaration {
        node_type: "FunctionDeclaration",
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
    class: &internal::ClassDeclaration<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassDeclaration {
    // Delegate to the main converter in declarations.rs
    convert_class_declaration(class, source, loc, interner, offset)
}
