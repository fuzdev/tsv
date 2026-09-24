// Statement dispatcher and simple statements.

use super::super::super::internal;
use super::control_flow::{
    write_break_statement, write_continue_statement, write_do_while_statement,
    write_for_in_statement, write_for_of_statement, write_for_statement, write_if_statement,
    write_labeled_statement, write_switch_statement, write_throw_statement, write_try_statement,
    write_while_statement, write_with_statement,
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
    Ctx, JsonWriter, close_node, node_header, write_array, write_bare_node,
    write_export_kind_field, write_identifier_plain, write_import_kind_field, write_literal,
    write_or_null,
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
) {
    match attributes {
        Some(attrs) => {
            w.raw(",\"attributes\":");
            write_array(w, attrs, |w, a| write_import_attribute(w, a, ctx));
        }
        None => {
            if ctx.vanilla_acorn {
                w.raw(",\"attributes\":[]");
            }
        }
    }
}

/// Emit a `Statement`, dispatching on its variant.
pub(super) fn write_statement(w: &mut JsonWriter, stmt: &internal::Statement<'_>, ctx: &Ctx<'_>) {
    match &stmt.kind {
        internal::StatementKind::ExpressionStatement(expr_stmt) => {
            node_header(w, "ExpressionStatement", stmt.span, ctx);
            w.raw(",\"expression\":");
            write_expression(w, expr_stmt.expression, ctx);
            if expr_stmt.is_directive {
                // acorn stores the raw string contents without quotes. A
                // directive is a string-literal expression, so its slice
                // includes both quotes (≥2 bytes).
                let raw = expr_stmt.expression.span().extract(ctx.source);
                w.raw(",\"directive\":");
                w.string(&raw[1..raw.len() - 1]);
            }
            close_node(w, "ExpressionStatement", stmt.span, ctx);
        }
        internal::StatementKind::VariableDeclaration(var_decl) => {
            write_variable_declaration(w, var_decl, ctx, false);
        }
        internal::StatementKind::TSTypeAliasDeclaration(type_alias) => {
            write_type_alias_declaration(w, type_alias, stmt.span, ctx);
        }
        internal::StatementKind::ReturnStatement(ret) => {
            node_header(w, "ReturnStatement", stmt.span, ctx);
            w.raw(",\"argument\":");
            write_or_null(w, ret.argument.as_ref(), |w, e| write_expression(w, e, ctx));
            close_node(w, "ReturnStatement", stmt.span, ctx);
        }
        internal::StatementKind::BlockStatement(block) => {
            write_block_statement(w, block, ctx);
        }
        internal::StatementKind::FunctionDeclaration(func_decl) => {
            write_function_declaration(w, func_decl, ctx);
        }
        internal::StatementKind::ClassDeclaration(class_decl) => {
            write_class_declaration(w, class_decl, ctx, false);
        }
        internal::StatementKind::ExportNamedDeclaration(export_decl) => {
            let is_type_export = matches!(export_decl.export_kind, internal::ExportKind::Type);
            let start = export_start(
                ctx.source,
                stmt.span.start,
                matches!(export_decl.declaration, Some(internal::Statement {
                    kind: internal::StatementKind::ClassDeclaration(class),
                    ..
                })
                    if class.decorators.is_some()),
            );
            let export_span = Span::new(start, stmt.span.end);
            node_header(w, "ExportNamedDeclaration", export_span, ctx);
            write_export_kind_field(w, export_decl.export_kind, ctx);
            w.raw(",\"declaration\":");
            write_or_null(w, export_decl.declaration.as_ref(), |w, d| {
                write_exported_declaration(w, d, ctx, is_type_export);
            });
            w.raw(",\"specifiers\":");
            write_array(w, export_decl.specifiers, |w, s| {
                write_export_specifier(w, s, ctx);
            });
            w.raw(",\"source\":");
            write_or_null(w, export_decl.source.as_ref(), |w, s| {
                write_literal(w, s, ctx);
            });
            write_attributes_field(w, export_decl.attributes, ctx);
            close_node(w, "ExportNamedDeclaration", export_span, ctx);
        }
        internal::StatementKind::ExportDefaultDeclaration(export_decl) => {
            let start = export_start(
                ctx.source,
                stmt.span.start,
                matches!(&export_decl.declaration, internal::ExportDefaultValue::ClassDeclaration(class)
                    if class.decorators.is_some()),
            );
            let export_span = Span::new(start, stmt.span.end);
            node_header(w, "ExportDefaultDeclaration", export_span, ctx);
            write_export_kind_field(w, internal::ExportKind::Value, ctx);
            w.raw(",\"declaration\":");
            write_export_default_value(w, &export_decl.declaration, ctx);
            close_node(w, "ExportDefaultDeclaration", export_span, ctx);
        }
        internal::StatementKind::ExportAllDeclaration(export_decl) => {
            node_header(w, "ExportAllDeclaration", stmt.span, ctx);
            write_export_kind_field(w, export_decl.export_kind, ctx);
            w.raw(",\"exported\":");
            write_or_null(w, export_decl.exported.as_ref(), |w, name| {
                write_module_export_name(w, name, ctx);
            });
            w.raw(",\"source\":");
            write_literal(w, &export_decl.source, ctx);
            write_attributes_field(w, export_decl.attributes, ctx);
            close_node(w, "ExportAllDeclaration", stmt.span, ctx);
        }
        internal::StatementKind::TSExportAssignment(export_assign) => {
            node_header(w, "TSExportAssignment", stmt.span, ctx);
            w.raw(",\"expression\":");
            write_expression(w, &export_assign.expression, ctx);
            close_node(w, "TSExportAssignment", stmt.span, ctx);
        }
        internal::StatementKind::TSNamespaceExportDeclaration(ns_export) => {
            node_header(w, "TSNamespaceExportDeclaration", stmt.span, ctx);
            w.raw(",\"id\":");
            write_identifier_plain(w, &ns_export.id, ctx);
            close_node(w, "TSNamespaceExportDeclaration", stmt.span, ctx);
        }
        internal::StatementKind::ImportDeclaration(import_decl) => {
            node_header(w, "ImportDeclaration", stmt.span, ctx);
            write_import_kind_field(w, import_decl.import_kind, ctx);
            if let Some(phase) = import_decl.phase.as_str() {
                w.raw(",\"phase\":");
                w.token(phase);
            }
            w.raw(",\"specifiers\":");
            write_array(w, import_decl.specifiers, |w, s| {
                write_import_specifier(w, s, ctx);
            });
            w.raw(",\"source\":");
            write_literal(w, &import_decl.source, ctx);
            write_attributes_field(w, import_decl.attributes, ctx);
            close_node(w, "ImportDeclaration", stmt.span, ctx);
        }
        internal::StatementKind::TSImportEqualsDeclaration(import_eq) => {
            node_header(w, "TSImportEqualsDeclaration", stmt.span, ctx);
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
            close_node(w, "TSImportEqualsDeclaration", stmt.span, ctx);
        }
        // Control flow statements
        internal::StatementKind::IfStatement(if_stmt) => {
            write_if_statement(w, if_stmt, stmt.span, ctx);
        }
        internal::StatementKind::ForStatement(for_stmt) => {
            write_for_statement(w, for_stmt, stmt.span, ctx);
        }
        internal::StatementKind::ForInStatement(for_in) => {
            write_for_in_statement(w, for_in, stmt.span, ctx);
        }
        internal::StatementKind::ForOfStatement(for_of) => {
            write_for_of_statement(w, for_of, stmt.span, ctx);
        }
        internal::StatementKind::WhileStatement(while_stmt) => {
            write_while_statement(w, while_stmt, stmt.span, ctx);
        }
        internal::StatementKind::DoWhileStatement(do_while) => {
            write_do_while_statement(w, do_while, stmt.span, ctx);
        }
        internal::StatementKind::WithStatement(with_stmt) => {
            write_with_statement(w, with_stmt, stmt.span, ctx);
        }
        internal::StatementKind::SwitchStatement(switch_stmt) => {
            write_switch_statement(w, switch_stmt, stmt.span, ctx);
        }
        internal::StatementKind::TryStatement(try_stmt) => {
            write_try_statement(w, try_stmt, stmt.span, ctx);
        }
        internal::StatementKind::ThrowStatement(throw_stmt) => {
            write_throw_statement(w, throw_stmt, stmt.span, ctx);
        }
        internal::StatementKind::BreakStatement(break_stmt) => {
            write_break_statement(w, break_stmt, stmt.span, ctx);
        }
        internal::StatementKind::ContinueStatement(continue_stmt) => {
            write_continue_statement(w, continue_stmt, stmt.span, ctx);
        }
        internal::StatementKind::LabeledStatement(labeled) => {
            write_labeled_statement(w, labeled, stmt.span, ctx);
        }
        internal::StatementKind::EmptyStatement(_) => {
            write_bare_node(w, "EmptyStatement", stmt.span, ctx);
        }
        internal::StatementKind::DebuggerStatement(_) => {
            write_bare_node(w, "DebuggerStatement", stmt.span, ctx);
        }
        internal::StatementKind::TSInterfaceDeclaration(iface) => {
            write_interface_declaration(w, iface, ctx, false);
        }
        internal::StatementKind::TSDeclareFunction(func) => {
            write_declare_function(w, func, ctx, false);
        }
        internal::StatementKind::TSEnumDeclaration(enum_decl) => {
            write_enum_declaration(w, enum_decl, stmt.span, ctx);
        }
        internal::StatementKind::TSModuleDeclaration(module_decl) => {
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
                node_header(w, "TSModuleBlock", block.span, ctx);
                w.raw(",\"body\":");
                write_array(w, block.body, |w, s| write_statement(w, s, ctx));
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

/// Emits a `BlockStatement` node.
pub(super) fn write_block_statement(
    w: &mut JsonWriter,
    block: &internal::BlockStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "BlockStatement", block.span, ctx);
    w.raw(",\"body\":");
    super::write_body_array(w, block.body, ctx, |w, s| write_statement(w, s, ctx));
    close_node(w, "BlockStatement", block.span, ctx);
}

/// Emit an `export`ed declaration. Same dispatch as `write_statement`, but the
/// `declare`-carrying declarations get `exported = true`: acorn-typescript's
/// `parseExportDeclaration` stamps `declare` on the *finished* node (post-hoc),
/// unlike the statement-level `tsTryParseDeclare` prefix assignment — so
/// `export declare` serializes `declare` last.
///
/// A decorated class needs the sharper question, because a decorator between
/// `export` and `declare` sends the class back down the *statement* route: it
/// takes `is_type_export`, the enclosing node's `exportKind`, which acorn sets in
/// the very `if (isDeclare)` block that does the post-hoc stamp. For the other
/// three, `declare` can only have come from the export route, so the bit is `true`
/// by construction (and inert where there is no `declare` to place).
fn write_exported_declaration(
    w: &mut JsonWriter,
    stmt: &internal::Statement<'_>,
    ctx: &Ctx<'_>,
    is_type_export: bool,
) {
    match &stmt.kind {
        internal::StatementKind::VariableDeclaration(var_decl) => {
            write_variable_declaration(w, var_decl, ctx, true);
        }
        internal::StatementKind::ClassDeclaration(class_decl) => {
            write_class_declaration(w, class_decl, ctx, is_type_export);
        }
        internal::StatementKind::TSInterfaceDeclaration(iface) => {
            write_interface_declaration(w, iface, ctx, true);
        }
        internal::StatementKind::TSDeclareFunction(func) => {
            write_declare_function(w, func, ctx, true);
        }
        _ => write_statement(w, stmt, ctx),
    }
}

/// Emits a `VariableDeclaration` node (each declarator a `VariableDeclarator`).
/// Field order: `declare?` (statement position), `declarations` (each: `id`,
/// `definite` only when true, `init` nullable), `kind`, `declare?` (export
/// position) — acorn-typescript's statement-level `declare` is stamped before
/// the declaration parses (`tsTryParseDeclare`), while `export declare` stamps
/// the finished node (`parseExportDeclaration`), so the field's position
/// depends on `exported`.
pub(super) fn write_variable_declaration(
    w: &mut JsonWriter,
    var_decl: &internal::VariableDeclaration<'_>,
    ctx: &Ctx<'_>,
    exported: bool,
) {
    node_header(w, "VariableDeclaration", var_decl.span, ctx);
    if var_decl.declare && !exported {
        w.raw(",\"declare\":true");
    }
    w.raw(",\"declarations\":");
    write_array(w, var_decl.declarations, |w, d| {
        node_header(w, "VariableDeclarator", d.span, ctx);
        w.raw(",\"id\":");
        write_expression(w, d.id, ctx);
        if d.definite {
            w.raw(",\"definite\":true");
        }
        w.raw(",\"init\":");
        write_or_null(w, d.init.as_ref(), |w, e| write_expression(w, e, ctx));
        close_node(w, "VariableDeclarator", d.span, ctx);
    });
    // One literal per kind, so each appends the whole field at once rather
    // than a runtime-length copy of `as_str()`.
    match var_decl.kind {
        internal::VariableDeclarationKind::Const => w.raw(",\"kind\":\"const\""),
        internal::VariableDeclarationKind::Let => w.raw(",\"kind\":\"let\""),
        internal::VariableDeclarationKind::Var => w.raw(",\"kind\":\"var\""),
        internal::VariableDeclarationKind::Using => w.raw(",\"kind\":\"using\""),
        internal::VariableDeclarationKind::AwaitUsing => w.raw(",\"kind\":\"await using\""),
    }
    if var_decl.declare && exported {
        w.raw(",\"declare\":true");
    }
    close_node(w, "VariableDeclaration", var_decl.span, ctx);
}
