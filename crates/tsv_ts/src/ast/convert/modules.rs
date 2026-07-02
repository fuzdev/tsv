// Import and export specifier conversions

use super::super::{internal, public};
use super::types::convert_declare_function;
use super::{
    Schema, bigint_to_decimal, convert_class_declaration, convert_expression,
    convert_function_declaration, convert_identifier, convert_interface_declaration,
    create_location, json_number_from_f64,
};
use std::borrow::Cow;
use string_interner::DefaultStringInterner;
use tsv_lang::LocationMapper;

pub(in crate::ast) fn convert_import_specifier<'src>(
    spec: &internal::ImportSpecifier<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
    schema: Schema,
) -> public::ImportSpecifier<'src> {
    match spec {
        internal::ImportSpecifier::Default(default_spec) => {
            public::ImportSpecifier::Default(public::ImportDefaultSpecifier {
                node_type: "ImportDefaultSpecifier",
                start: loc.pos(default_spec.span.start),
                end: loc.pos(default_spec.span.end),
                loc: create_location(default_spec.span, loc),
                local: convert_identifier(&default_spec.local, source, loc, interner),
            })
        }
        internal::ImportSpecifier::Named(named_spec) => {
            let import_kind = match named_spec.import_kind {
                internal::ImportKind::Value => {
                    if schema.is_svelte_script() {
                        None
                    } else {
                        Some("value")
                    }
                }
                internal::ImportKind::Type => Some("type"),
            };
            public::ImportSpecifier::Named(public::ImportNamedSpecifier {
                node_type: "ImportSpecifier",
                start: loc.pos(named_spec.span.start),
                end: loc.pos(named_spec.span.end),
                loc: create_location(named_spec.span, loc),
                imported: convert_module_export_name(&named_spec.imported, source, loc, interner),
                local: convert_identifier(&named_spec.local, source, loc, interner),
                import_kind,
            })
        }
        internal::ImportSpecifier::Namespace(ns_spec) => {
            public::ImportSpecifier::Namespace(public::ImportNamespaceSpecifier {
                node_type: "ImportNamespaceSpecifier",
                start: loc.pos(ns_spec.span.start),
                end: loc.pos(ns_spec.span.end),
                loc: create_location(ns_spec.span, loc),
                local: convert_identifier(&ns_spec.local, source, loc, interner),
            })
        }
    }
}

pub(in crate::ast) fn convert_import_attribute<'src>(
    attr: &internal::ImportAttribute<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ImportAttribute<'src> {
    let key = match &attr.key {
        internal::ImportAttributeKey::Identifier(id) => {
            public::ImportAttributeKey::Identifier(convert_identifier(id, source, loc, interner))
        }
        internal::ImportAttributeKey::Literal(lit) => {
            public::ImportAttributeKey::Literal(convert_literal(lit, source, loc))
        }
    };
    public::ImportAttribute {
        node_type: "ImportAttribute",
        start: loc.pos(attr.span.start),
        end: loc.pos(attr.span.end),
        loc: create_location(attr.span, loc),
        key,
        value: convert_literal(&attr.value, source, loc),
    }
}

pub(in crate::ast) fn convert_export_specifier<'src>(
    spec: &internal::ExportSpecifier<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
    schema: Schema,
) -> public::ExportSpecifier<'src> {
    let export_kind = match spec.export_kind {
        internal::ExportKind::Value => {
            if schema.is_svelte_script() {
                None
            } else {
                Some("value")
            }
        }
        internal::ExportKind::Type => Some("type"),
    };
    public::ExportSpecifier {
        node_type: "ExportSpecifier",
        start: loc.pos(spec.span.start),
        end: loc.pos(spec.span.end),
        loc: create_location(spec.span, loc),
        local: convert_module_export_name(&spec.local, source, loc, interner),
        exported: convert_module_export_name(&spec.exported, source, loc, interner),
        export_kind,
    }
}

/// Convert a `ModuleExportName` (import/export specifier name, or `export * as`
/// namespace name): an identifier emits an `Identifier` node, a string a
/// `Literal` node — mirroring acorn (`ModuleExportName : IdentifierName | StringLiteral`).
pub(in crate::ast) fn convert_module_export_name<'src>(
    name: &internal::ModuleExportName<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ModuleExportName<'src> {
    match name {
        internal::ModuleExportName::Identifier(id) => {
            public::ModuleExportName::Identifier(convert_identifier(id, source, loc, interner))
        }
        internal::ModuleExportName::Literal(lit) => {
            public::ModuleExportName::Literal(convert_literal(lit, source, loc))
        }
    }
}

pub(in crate::ast) fn convert_export_default_value<'src>(
    value: &internal::ExportDefaultValue<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ExportDefaultValue<'src> {
    match value {
        internal::ExportDefaultValue::Expression(expr) => {
            public::ExportDefaultValue::Expression(convert_expression(expr, source, loc, interner))
        }
        internal::ExportDefaultValue::FunctionDeclaration(func) => {
            public::ExportDefaultValue::FunctionDeclaration(convert_function_declaration(
                func, source, loc, interner,
            ))
        }
        internal::ExportDefaultValue::TSDeclareFunction(func) => {
            public::ExportDefaultValue::TSDeclareFunction(convert_declare_function(
                func, source, loc, interner,
            ))
        }
        internal::ExportDefaultValue::ClassDeclaration(class) => {
            public::ExportDefaultValue::ClassDeclaration(convert_class_declaration(
                class, source, loc, interner,
            ))
        }
        internal::ExportDefaultValue::TSInterfaceDeclaration(iface) => {
            public::ExportDefaultValue::TSInterfaceDeclaration(convert_interface_declaration(
                iface, source, loc, interner,
            ))
        }
    }
}

// Helper to convert literal for import attributes and export sources
pub(in crate::ast) fn convert_literal<'src>(
    lit: &internal::Literal<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
) -> public::Literal<'src> {
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
    };
    let raw = lit.span.extract(source);
    public::Literal {
        node_type: "Literal",
        start: loc.pos(lit.span.start),
        end: loc.pos(lit.span.end),
        loc: create_location(lit.span, loc),
        value,
        raw: Cow::Borrowed(raw),
        bigint,
    }
}
