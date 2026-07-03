// Statement dispatcher and simple statements.

use super::super::super::internal;
use super::super::Schema;
use super::control_flow::{
    write_break_statement, write_continue_statement, write_do_while_statement,
    write_for_in_statement, write_for_of_statement, write_for_statement, write_if_statement,
    write_labeled_statement, write_switch_statement, write_throw_statement, write_try_statement,
    write_while_statement,
};
use super::declarations::{
    write_class_declaration, write_function_declaration, write_type_alias_declaration,
};
use super::expressions::write_expression;
use super::modules::{
    write_export_default_value, write_export_specifier, write_import_attribute,
    write_import_specifier, write_module_export_name,
};
use super::types::{
    write_declare_function, write_entity_name, write_enum_declaration, write_interface_declaration,
};
use super::{
    Ctx, JsonWriter, close_node, kind_token, node_header, write_array, write_bare_node,
    write_identifier_plain, write_literal, write_or_null,
};
use tsv_lang::Span;

/// The `export` keyword's start, skipping comments so an `export` inside one
/// (`@dec /* export */ export …`) isn't mistaken for the keyword (which would
/// mislocate the node's start).
fn find_export_start(source: &str, span_start: u32) -> u32 {
    tsv_lang::source_scan::find_keyword(
        source.as_bytes(),
        span_start as usize,
        source.len(),
        b"export",
        tsv_lang::source_scan::TriviaProfile::JS,
    )
    .map_or(span_start, |pos| pos as u32)
}

/// The export node's start: a decorated exported class starts at `export`,
/// not at the decorator (shared by both export arms).
fn export_start(source: &str, span_start: u32, class_is_decorated: bool) -> u32 {
    if class_is_decorated {
        find_export_start(source, span_start)
    } else {
        span_start
    }
}

/// Emit the `attributes` field with its skip rule: a `with` clause emits its
/// attributes (possibly `[]`); no clause emits `[]` under the Svelte schema and
/// omits the field under acorn.
fn write_attributes_field(
    w: &mut JsonWriter,
    attributes: Option<&[internal::ImportAttribute<'_>]>,
    ctx: &Ctx<'_>,
    schema: Schema,
) {
    match attributes {
        Some(attrs) => {
            w.raw(",\"attributes\":");
            write_array(w, attrs, |w, a| write_import_attribute(w, a, ctx));
        }
        None => {
            if schema.is_svelte_script() {
                w.raw(",\"attributes\":[]");
            }
        }
    }
}

/// Emit a `Statement`, dispatching on its variant.
pub(super) fn write_statement(
    w: &mut JsonWriter,
    stmt: &internal::Statement<'_>,
    ctx: &Ctx<'_>,
    schema: Schema,
) {
    match stmt {
        internal::Statement::ExpressionStatement(expr_stmt) => {
            node_header(w, "ExpressionStatement", expr_stmt.span, ctx);
            w.raw(",\"expression\":");
            write_expression(w, &expr_stmt.expression, ctx);
            if expr_stmt.is_directive {
                // acorn stores the raw string contents without quotes. A
                // directive is a string-literal expression, so its slice
                // includes both quotes (≥2 bytes).
                let raw = expr_stmt.expression.span().extract(ctx.source);
                w.raw(",\"directive\":");
                w.string(&raw[1..raw.len() - 1]);
            }
            close_node(w, "ExpressionStatement", expr_stmt.span, ctx);
        }
        internal::Statement::VariableDeclaration(var_decl) => {
            write_variable_declaration(w, var_decl, ctx);
        }
        internal::Statement::TSTypeAliasDeclaration(type_alias) => {
            write_type_alias_declaration(w, type_alias, ctx);
        }
        internal::Statement::ReturnStatement(ret) => {
            node_header(w, "ReturnStatement", ret.span, ctx);
            w.raw(",\"argument\":");
            write_or_null(w, ret.argument.as_ref(), |w, e| write_expression(w, e, ctx));
            close_node(w, "ReturnStatement", ret.span, ctx);
        }
        internal::Statement::BlockStatement(block) => {
            write_block_statement(w, block, ctx);
        }
        internal::Statement::FunctionDeclaration(func_decl) => {
            write_function_declaration(w, func_decl, ctx);
        }
        internal::Statement::ClassDeclaration(class_decl) => {
            write_class_declaration(w, class_decl, ctx);
        }
        internal::Statement::ExportNamedDeclaration(export_decl) => {
            let export_kind = kind_token(
                matches!(export_decl.export_kind, internal::ExportKind::Type),
                schema,
            );
            let start = export_start(
                ctx.source,
                export_decl.span.start,
                matches!(export_decl.declaration, Some(internal::Statement::ClassDeclaration(class))
                    if class.decorators.is_some()),
            );
            let export_span = Span::new(start, export_decl.span.end);
            node_header(w, "ExportNamedDeclaration", export_span, ctx);
            if let Some(kind) = export_kind {
                w.raw(",\"exportKind\":");
                w.token(kind);
            }
            w.raw(",\"declaration\":");
            write_or_null(w, export_decl.declaration.as_ref(), |w, d| {
                write_statement(w, d, ctx, schema);
            });
            w.raw(",\"specifiers\":");
            write_array(w, export_decl.specifiers, |w, s| {
                write_export_specifier(w, s, ctx, schema);
            });
            w.raw(",\"source\":");
            write_or_null(w, export_decl.source.as_ref(), |w, s| {
                write_literal(w, s, ctx);
            });
            write_attributes_field(w, export_decl.attributes, ctx, schema);
            close_node(w, "ExportNamedDeclaration", export_span, ctx);
        }
        internal::Statement::ExportDefaultDeclaration(export_decl) => {
            let export_kind = if schema.is_svelte_script() {
                None
            } else {
                Some("value")
            };
            let start = export_start(
                ctx.source,
                export_decl.span.start,
                matches!(&export_decl.declaration, internal::ExportDefaultValue::ClassDeclaration(class)
                    if class.decorators.is_some()),
            );
            let export_span = Span::new(start, export_decl.span.end);
            node_header(w, "ExportDefaultDeclaration", export_span, ctx);
            if let Some(kind) = export_kind {
                w.raw(",\"exportKind\":");
                w.token(kind);
            }
            w.raw(",\"declaration\":");
            write_export_default_value(w, &export_decl.declaration, ctx);
            close_node(w, "ExportDefaultDeclaration", export_span, ctx);
        }
        internal::Statement::ExportAllDeclaration(export_decl) => {
            let export_kind = kind_token(
                matches!(export_decl.export_kind, internal::ExportKind::Type),
                schema,
            );
            node_header(w, "ExportAllDeclaration", export_decl.span, ctx);
            if let Some(kind) = export_kind {
                w.raw(",\"exportKind\":");
                w.token(kind);
            }
            w.raw(",\"exported\":");
            write_or_null(w, export_decl.exported.as_ref(), |w, name| {
                write_module_export_name(w, name, ctx);
            });
            w.raw(",\"source\":");
            write_literal(w, &export_decl.source, ctx);
            write_attributes_field(w, export_decl.attributes, ctx, schema);
            close_node(w, "ExportAllDeclaration", export_decl.span, ctx);
        }
        internal::Statement::TSExportAssignment(export_assign) => {
            node_header(w, "TSExportAssignment", export_assign.span, ctx);
            w.raw(",\"expression\":");
            write_expression(w, &export_assign.expression, ctx);
            close_node(w, "TSExportAssignment", export_assign.span, ctx);
        }
        internal::Statement::TSNamespaceExportDeclaration(ns_export) => {
            node_header(w, "TSNamespaceExportDeclaration", ns_export.span, ctx);
            w.raw(",\"id\":");
            write_identifier_plain(w, &ns_export.id, ctx);
            close_node(w, "TSNamespaceExportDeclaration", ns_export.span, ctx);
        }
        internal::Statement::ImportDeclaration(import_decl) => {
            let import_kind = kind_token(
                matches!(import_decl.import_kind, internal::ImportKind::Type),
                schema,
            );
            node_header(w, "ImportDeclaration", import_decl.span, ctx);
            if let Some(kind) = import_kind {
                w.raw(",\"importKind\":");
                w.token(kind);
            }
            if let Some(phase) = import_decl.phase.as_str() {
                w.raw(",\"phase\":");
                w.token(phase);
            }
            w.raw(",\"specifiers\":");
            write_array(w, import_decl.specifiers, |w, s| {
                write_import_specifier(w, s, ctx, schema);
            });
            w.raw(",\"source\":");
            write_literal(w, &import_decl.source, ctx);
            write_attributes_field(w, import_decl.attributes, ctx, schema);
            close_node(w, "ImportDeclaration", import_decl.span, ctx);
        }
        internal::Statement::TSImportEqualsDeclaration(import_eq) => {
            node_header(w, "TSImportEqualsDeclaration", import_eq.span, ctx);
            w.raw(",\"importKind\":");
            w.token(match import_eq.import_kind {
                internal::ImportKind::Value => "value",
                internal::ImportKind::Type => "type",
            });
            w.raw(",\"isExport\":");
            w.bool(import_eq.is_export);
            w.raw(",\"id\":");
            write_identifier_plain(w, &import_eq.id, ctx);
            w.raw(",\"moduleReference\":");
            match &import_eq.module_reference {
                internal::TSModuleReference::ExternalModuleReference(ext_ref) => {
                    node_header(w, "TSExternalModuleReference", ext_ref.span, ctx);
                    w.raw(",\"expression\":");
                    write_literal(w, &ext_ref.expression, ctx);
                    close_node(w, "TSExternalModuleReference", ext_ref.span, ctx);
                }
                internal::TSModuleReference::EntityName(entity_name) => {
                    write_entity_name(w, entity_name, ctx);
                }
            }
            close_node(w, "TSImportEqualsDeclaration", import_eq.span, ctx);
        }
        // Control flow statements
        internal::Statement::IfStatement(if_stmt) => write_if_statement(w, if_stmt, ctx),
        internal::Statement::ForStatement(for_stmt) => write_for_statement(w, for_stmt, ctx),
        internal::Statement::ForInStatement(for_in) => write_for_in_statement(w, for_in, ctx),
        internal::Statement::ForOfStatement(for_of) => write_for_of_statement(w, for_of, ctx),
        internal::Statement::WhileStatement(while_stmt) => {
            write_while_statement(w, while_stmt, ctx);
        }
        internal::Statement::DoWhileStatement(do_while) => {
            write_do_while_statement(w, do_while, ctx);
        }
        internal::Statement::SwitchStatement(switch_stmt) => {
            write_switch_statement(w, switch_stmt, ctx);
        }
        internal::Statement::TryStatement(try_stmt) => write_try_statement(w, try_stmt, ctx),
        internal::Statement::ThrowStatement(throw_stmt) => {
            write_throw_statement(w, throw_stmt, ctx);
        }
        internal::Statement::BreakStatement(break_stmt) => {
            write_break_statement(w, break_stmt, ctx);
        }
        internal::Statement::ContinueStatement(continue_stmt) => {
            write_continue_statement(w, continue_stmt, ctx);
        }
        internal::Statement::LabeledStatement(labeled) => {
            write_labeled_statement(w, labeled, ctx);
        }
        internal::Statement::EmptyStatement(empty) => {
            write_bare_node(w, "EmptyStatement", empty.span, ctx);
        }
        internal::Statement::DebuggerStatement(dbg) => {
            write_bare_node(w, "DebuggerStatement", dbg.span, ctx);
        }
        internal::Statement::TSInterfaceDeclaration(iface) => {
            write_interface_declaration(w, iface, ctx);
        }
        internal::Statement::TSDeclareFunction(func) => write_declare_function(w, func, ctx),
        internal::Statement::TSEnumDeclaration(enum_decl) => {
            write_enum_declaration(w, enum_decl, ctx);
        }
        internal::Statement::TSModuleDeclaration(module_decl) => {
            write_module_declaration(w, module_decl, ctx);
        }
    }
}

/// Emits a `TSModuleDeclaration` node. Field order: `global` (only when
/// true), `id`, `body?` (omitted for shorthand ambient modules), `declare`
/// (only when true).
pub(super) fn write_module_declaration(
    w: &mut JsonWriter,
    decl: &internal::TSModuleDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSModuleDeclaration", decl.span, ctx);
    if decl.global {
        w.raw(",\"global\":true");
    }
    w.raw(",\"id\":");
    match &decl.id {
        internal::TSModuleName::Identifier(id) => write_identifier_plain(w, id, ctx),
        internal::TSModuleName::Literal(lit) => write_literal(w, lit, ctx),
    }
    if let Some(body) = &decl.body {
        w.raw(",\"body\":");
        match body {
            internal::TSModuleDeclarationBody::TSModuleBlock(block) => {
                // Always TypeScript context (declare namespace/module).
                node_header(w, "TSModuleBlock", block.span, ctx);
                w.raw(",\"body\":");
                write_array(w, block.body, |w, s| {
                    write_statement(w, s, ctx, Schema::Acorn);
                });
                close_node(w, "TSModuleBlock", block.span, ctx);
            }
            internal::TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
                write_module_declaration(w, nested, ctx);
            }
        }
    }
    if decl.declare {
        w.raw(",\"declare\":true");
    }
    close_node(w, "TSModuleDeclaration", decl.span, ctx);
}

/// Emits a `BlockStatement` node (always TypeScript context).
pub(super) fn write_block_statement(
    w: &mut JsonWriter,
    block: &internal::BlockStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "BlockStatement", block.span, ctx);
    w.raw(",\"body\":");
    write_array(w, block.body, |w, s| {
        write_statement(w, s, ctx, Schema::Acorn);
    });
    close_node(w, "BlockStatement", block.span, ctx);
}

/// Emits a `VariableDeclaration` node (each declarator a `VariableDeclarator`).
/// Field order: `declarations` (each: `id`, `definite` only when true, `init`
/// nullable), `kind`, `declare` (only when true).
pub(super) fn write_variable_declaration(
    w: &mut JsonWriter,
    var_decl: &internal::VariableDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "VariableDeclaration", var_decl.span, ctx);
    w.raw(",\"declarations\":");
    write_array(w, var_decl.declarations, |w, d| {
        node_header(w, "VariableDeclarator", d.span, ctx);
        w.raw(",\"id\":");
        write_expression(w, &d.id, ctx);
        if d.definite {
            w.raw(",\"definite\":true");
        }
        w.raw(",\"init\":");
        write_or_null(w, d.init.as_ref(), |w, e| write_expression(w, e, ctx));
        close_node(w, "VariableDeclarator", d.span, ctx);
    });
    w.raw(",\"kind\":");
    w.token(var_decl.kind.as_str());
    if var_decl.declare {
        w.raw(",\"declare\":true");
    }
    close_node(w, "VariableDeclaration", var_decl.span, ctx);
}
