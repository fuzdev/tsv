// Import and export specifier writers.

use super::super::super::internal;
use super::super::Schema;
use super::declarations::{write_class_declaration, write_function_declaration};
use super::expressions::write_expression;
use super::types::{write_declare_function, write_interface_declaration};
use super::{
    Ctx, JsonWriter, close_node, kind_token, node_header, write_identifier_plain, write_literal,
};

/// Emit an import specifier (`ImportDefaultSpecifier` / `ImportSpecifier` /
/// `ImportNamespaceSpecifier`).
pub(super) fn write_import_specifier(
    w: &mut JsonWriter,
    spec: &internal::ImportSpecifier<'_>,
    ctx: &Ctx<'_>,
    schema: Schema,
) {
    match spec {
        internal::ImportSpecifier::Default(default_spec) => {
            node_header(w, "ImportDefaultSpecifier", default_spec.span, ctx);
            w.raw(",\"local\":");
            write_identifier_plain(w, &default_spec.local, ctx);
            close_node(w, "ImportDefaultSpecifier", default_spec.span, ctx);
        }
        internal::ImportSpecifier::Named(named_spec) => {
            let import_kind = kind_token(
                matches!(named_spec.import_kind, internal::ImportKind::Type),
                schema,
            );
            node_header(w, "ImportSpecifier", named_spec.span, ctx);
            w.raw(",\"imported\":");
            write_module_export_name(w, &named_spec.imported, ctx);
            w.raw(",\"local\":");
            write_identifier_plain(w, &named_spec.local, ctx);
            if let Some(kind) = import_kind {
                w.raw(",\"importKind\":");
                w.token(kind);
            }
            close_node(w, "ImportSpecifier", named_spec.span, ctx);
        }
        internal::ImportSpecifier::Namespace(ns_spec) => {
            node_header(w, "ImportNamespaceSpecifier", ns_spec.span, ctx);
            w.raw(",\"local\":");
            write_identifier_plain(w, &ns_spec.local, ctx);
            close_node(w, "ImportNamespaceSpecifier", ns_spec.span, ctx);
        }
    }
}

/// Emits an `ImportAttribute` node.
pub(super) fn write_import_attribute(
    w: &mut JsonWriter,
    attr: &internal::ImportAttribute<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ImportAttribute", attr.span, ctx);
    w.raw(",\"key\":");
    match &attr.key {
        internal::ImportAttributeKey::Identifier(id) => write_identifier_plain(w, id, ctx),
        internal::ImportAttributeKey::Literal(lit) => write_literal(w, lit, ctx),
    }
    w.raw(",\"value\":");
    write_literal(w, &attr.value, ctx);
    close_node(w, "ImportAttribute", attr.span, ctx);
}

/// Emits an `ExportSpecifier` node. Field order: `local`, `exported`,
/// `exportKind?`.
pub(super) fn write_export_specifier(
    w: &mut JsonWriter,
    spec: &internal::ExportSpecifier<'_>,
    ctx: &Ctx<'_>,
    schema: Schema,
) {
    let export_kind = kind_token(
        matches!(spec.export_kind, internal::ExportKind::Type),
        schema,
    );
    node_header(w, "ExportSpecifier", spec.span, ctx);
    w.raw(",\"local\":");
    write_module_export_name(w, &spec.local, ctx);
    w.raw(",\"exported\":");
    write_module_export_name(w, &spec.exported, ctx);
    if let Some(kind) = export_kind {
        w.raw(",\"exportKind\":");
        w.token(kind);
    }
    close_node(w, "ExportSpecifier", spec.span, ctx);
}

/// Emit a module export name: an identifier emits an `Identifier` node, a
/// string a `Literal` node.
pub(super) fn write_module_export_name(
    w: &mut JsonWriter,
    name: &internal::ModuleExportName<'_>,
    ctx: &Ctx<'_>,
) {
    match name {
        internal::ModuleExportName::Identifier(id) => write_identifier_plain(w, id, ctx),
        internal::ModuleExportName::Literal(lit) => write_literal(w, lit, ctx),
    }
}

/// Emit an `export default` value (an expression or a function/class/interface
/// declaration).
pub(super) fn write_export_default_value(
    w: &mut JsonWriter,
    value: &internal::ExportDefaultValue<'_>,
    ctx: &Ctx<'_>,
) {
    match value {
        internal::ExportDefaultValue::Expression(expr) => write_expression(w, expr, ctx),
        internal::ExportDefaultValue::FunctionDeclaration(func) => {
            write_function_declaration(w, func, ctx);
        }
        internal::ExportDefaultValue::TSDeclareFunction(func) => {
            // `declare` can't syntactically appear in an `export default`, so
            // the `exported` declare-position flag is inert here.
            write_declare_function(w, func, ctx, true);
        }
        internal::ExportDefaultValue::ClassDeclaration(class) => {
            write_class_declaration(w, class, ctx, true);
        }
        internal::ExportDefaultValue::TSInterfaceDeclaration(iface) => {
            write_interface_declaration(w, iface, ctx, true);
        }
    }
}
