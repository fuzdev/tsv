// Statement dispatcher and simple statements.

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
    Ctx, Sink, close_node, node_header, write_array, write_bare_node, write_export_kind_field,
    write_identifier_plain, write_import_kind_field, write_literal, write_or_null,
};
use crate::ast;
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
fn write_attributes_field<const E: u16>(
    w: &mut Sink<'_>,
    attributes: Option<&[ast::ImportAttribute]>,
    ctx: &Ctx<'_>,
) {
    match attributes {
        Some(attrs) => {
            raw!(w, @E, ",\"attributes\":");
            write_array(w, attrs, |w, a| write_import_attribute(w, a, ctx));
        }
        None => {
            if ctx.vanilla_acorn {
                raw!(w, @E, ",\"attributes\":[]");
            }
        }
    }
}

/// Emit a `Statement`, dispatching on its variant.
pub(super) fn write_statement(w: &mut Sink<'_>, stmt: &ast::Statement, ctx: &Ctx<'_>) {
    match &stmt.kind {
        ast::StatementKind::ExpressionStatement(expr_stmt) => {
            node_header!(w, "ExpressionStatement", stmt.span, ctx);
            raw!(w, "ExpressionStatement", ",\"expression\":");
            write_expression(w, ctx.view.expr(expr_stmt.expression), ctx);
            if expr_stmt.is_directive {
                // acorn stores the raw string contents without quotes. A
                // directive is a string-literal expression, so its slice
                // includes both quotes (≥2 bytes).
                let raw = ctx
                    .view
                    .expr(expr_stmt.expression)
                    .span()
                    .extract(ctx.source);
                raw!(w, "ExpressionStatement", ",\"directive\":");
                w.string(&raw[1..raw.len() - 1]);
            }
            close_node(w, "ExpressionStatement", stmt.span, ctx);
        }
        ast::StatementKind::VariableDeclaration(var_decl) => {
            write_variable_declaration(w, var_decl, ctx);
        }
        &ast::StatementKind::TSTypeAliasDeclaration(type_alias) => {
            let type_alias = ctx.view.type_alias(type_alias);
            write_type_alias_declaration(w, type_alias, stmt.span, ctx);
        }
        ast::StatementKind::ReturnStatement(ret) => {
            node_header!(w, "ReturnStatement", stmt.span, ctx);
            raw!(w, "ReturnStatement", ",\"argument\":");
            write_or_null(w, ret.argument.as_ref(), |w, e| {
                write_expression(w, ctx.view.expr(*e), ctx);
            });
            close_node(w, "ReturnStatement", stmt.span, ctx);
        }
        ast::StatementKind::BlockStatement(block) => {
            write_block_statement(w, block, ctx);
        }
        &ast::StatementKind::FunctionDeclaration(func_decl) => {
            let func_decl = ctx.view.fn_decl(func_decl);
            write_function_declaration(w, func_decl, ctx);
        }
        &ast::StatementKind::ClassDeclaration(class_decl) => {
            let class_decl = ctx.view.class_decl(class_decl);
            write_class_declaration(w, class_decl, ctx);
        }
        &ast::StatementKind::ExportNamedDeclaration(export_decl) => {
            let export_decl = ctx.view.export_named(export_decl);
            let start = export_start(
                ctx.source,
                stmt.span.start,
                matches!(ctx.view.opt_stmt(export_decl.declaration), Some(&ast::Statement {
                    kind: ast::StatementKind::ClassDeclaration(class),
                    ..
                })
                    if ctx.view.class_decl(class).decorators.is_some()),
            );
            let export_span = Span::new(start, stmt.span.end);
            node_header!(w, "ExportNamedDeclaration", export_span, ctx);
            write_export_kind_field::<{ at!("ExportNamedDeclaration") }>(
                w,
                export_decl.export_kind,
                ctx,
            );
            raw!(w, "ExportNamedDeclaration", ",\"declaration\":");
            write_or_null(w, ctx.view.opt_stmt(export_decl.declaration), |w, d| {
                write_statement(w, d, ctx);
            });
            raw!(w, "ExportNamedDeclaration", ",\"specifiers\":");
            write_array(w, ctx.view.export_specs(export_decl.specifiers), |w, s| {
                write_export_specifier(w, s, ctx);
            });
            raw!(w, "ExportNamedDeclaration", ",\"source\":");
            write_or_null(w, export_decl.source.as_ref(), |w, s| {
                write_literal(w, s, ctx);
            });
            write_attributes_field::<{ at!("ExportNamedDeclaration") }>(
                w,
                export_decl.attributes.map(|a| ctx.view.import_attrs(a)),
                ctx,
            );
            close_node(w, "ExportNamedDeclaration", export_span, ctx);
        }
        &ast::StatementKind::ExportDefaultDeclaration(export_decl) => {
            let export_decl = ctx.view.export_default(export_decl);
            let start = export_start(
                ctx.source,
                stmt.span.start,
                matches!(&export_decl.declaration, ast::ExportDefaultValue::ClassDeclaration(class)
                    if class.decorators.is_some()),
            );
            let export_span = Span::new(start, stmt.span.end);
            node_header!(w, "ExportDefaultDeclaration", export_span, ctx);
            json_only! { write_export_kind_field::<0>(w, ast::ExportKind::Value, ctx); }
            // A default export is always a value export: its entry's `exportKind` vocabulary
            // holds that one word, so the tape copy writes `write_export_kind_field`'s `Value`
            // arms alone (its `Type` arm's literal fails that entry's build).
            tape! {
                if !ctx.vanilla_acorn {
                    raw!(w, "ExportDefaultDeclaration", ",\"exportKind\":\"value\"");
                }
            }
            raw!(w, "ExportDefaultDeclaration", ",\"declaration\":");
            write_export_default_value(w, &export_decl.declaration, ctx);
            close_node(w, "ExportDefaultDeclaration", export_span, ctx);
        }
        &ast::StatementKind::ExportAllDeclaration(export_decl) => {
            let export_decl = ctx.view.export_all(export_decl);
            node_header!(w, "ExportAllDeclaration", stmt.span, ctx);
            write_export_kind_field::<{ at!("ExportAllDeclaration") }>(
                w,
                export_decl.export_kind,
                ctx,
            );
            raw!(w, "ExportAllDeclaration", ",\"exported\":");
            write_or_null(w, export_decl.exported.as_ref(), |w, name| {
                write_module_export_name(w, name, ctx);
            });
            raw!(w, "ExportAllDeclaration", ",\"source\":");
            write_literal(w, &export_decl.source, ctx);
            write_attributes_field::<{ at!("ExportAllDeclaration") }>(
                w,
                export_decl.attributes.map(|a| ctx.view.import_attrs(a)),
                ctx,
            );
            close_node(w, "ExportAllDeclaration", stmt.span, ctx);
        }
        &ast::StatementKind::TSExportAssignment(export_assign) => {
            let export_assign = ctx.view.export_assign(export_assign);
            node_header!(w, "TSExportAssignment", stmt.span, ctx);
            raw!(w, "TSExportAssignment", ",\"expression\":");
            write_expression(w, ctx.view.expr(export_assign.expression), ctx);
            close_node(w, "TSExportAssignment", stmt.span, ctx);
        }
        ast::StatementKind::TSNamespaceExportDeclaration(ns_export) => {
            node_header!(w, "TSNamespaceExportDeclaration", stmt.span, ctx);
            raw!(w, "TSNamespaceExportDeclaration", ",\"id\":");
            write_identifier_plain(w, &ns_export.id, ctx);
            close_node(w, "TSNamespaceExportDeclaration", stmt.span, ctx);
        }
        &ast::StatementKind::ImportDeclaration(import_decl) => {
            let import_decl = ctx.view.import_decl(import_decl);
            node_header!(w, "ImportDeclaration", stmt.span, ctx);
            write_import_kind_field::<{ at!("ImportDeclaration") }>(
                w,
                import_decl.import_kind,
                ctx,
            );
            if let Some(phase) = import_decl.phase.as_str() {
                raw!(w, "ImportDeclaration", ",\"phase\":");
                json_only! { w.token(phase); }
                tape! {
                    let _ = phase;
                    phase_keyword!(w, import_decl.phase, "ImportDeclaration");
                }
            }
            raw!(w, "ImportDeclaration", ",\"specifiers\":");
            write_array(w, ctx.view.import_specs(import_decl.specifiers), |w, s| {
                write_import_specifier(w, s, ctx);
            });
            raw!(w, "ImportDeclaration", ",\"source\":");
            write_literal(w, &import_decl.source, ctx);
            write_attributes_field::<{ at!("ImportDeclaration") }>(
                w,
                import_decl.attributes.map(|a| ctx.view.import_attrs(a)),
                ctx,
            );
            close_node(w, "ImportDeclaration", stmt.span, ctx);
        }
        &ast::StatementKind::TSImportEqualsDeclaration(import_eq) => {
            let import_eq = ctx.view.import_equals(import_eq);
            node_header!(w, "TSImportEqualsDeclaration", stmt.span, ctx);
            raw!(w, "TSImportEqualsDeclaration", ",\"importKind\":");
            let import_kind = match import_eq.import_kind {
                ast::ImportKind::Value => "value",
                ast::ImportKind::Type => "type",
            };
            json_only! { w.token(import_kind); }
            tape! {
                const VOCAB: u16 = ::tsv_lang::tape::field_aux(
                    entry!("TSImportEqualsDeclaration"),
                    "importKind",
                );
                let index = by_variant!(import_eq.import_kind, ast::ImportKind => [Value, Type],
                    |k| -> u32 {
                        ::tsv_lang::tape::keyword_id(VOCAB, match k {
                            ast::ImportKind::Value => "value",
                            ast::ImportKind::Type => "type",
                        })
                    });
                let _ = import_kind;
                w.t_keyword_id(VOCAB, index);
            }
            raw!(w, "TSImportEqualsDeclaration", ",\"isExport\":");
            w.bool(import_eq.is_export);
            raw!(w, "TSImportEqualsDeclaration", ",\"id\":");
            write_identifier_plain(w, &import_eq.id, ctx);
            raw!(w, "TSImportEqualsDeclaration", ",\"moduleReference\":");
            match &import_eq.module_reference {
                ast::TSModuleReference::ExternalModuleReference(ext_ref) => {
                    node_header!(w, "TSExternalModuleReference", ext_ref.span, ctx);
                    raw!(w, "TSExternalModuleReference", ",\"expression\":");
                    write_literal(w, &ext_ref.expression, ctx);
                    close_node(w, "TSExternalModuleReference", ext_ref.span, ctx);
                }
                ast::TSModuleReference::EntityName(entity_name) => {
                    write_entity_name(w, entity_name, ctx);
                }
            }
            close_node(w, "TSImportEqualsDeclaration", stmt.span, ctx);
        }
        // Control flow statements
        ast::StatementKind::IfStatement(if_stmt) => {
            write_if_statement(w, if_stmt, stmt.span, ctx);
        }
        ast::StatementKind::ForStatement(for_stmt) => {
            write_for_statement(w, for_stmt, stmt.span, ctx);
        }
        ast::StatementKind::ForInStatement(for_in) => {
            write_for_in_statement(w, for_in, stmt.span, ctx);
        }
        ast::StatementKind::ForOfStatement(for_of) => {
            write_for_of_statement(w, for_of, stmt.span, ctx);
        }
        ast::StatementKind::WhileStatement(while_stmt) => {
            write_while_statement(w, *while_stmt, stmt.span, ctx);
        }
        ast::StatementKind::DoWhileStatement(do_while) => {
            write_do_while_statement(w, *do_while, stmt.span, ctx);
        }
        ast::StatementKind::WithStatement(with_stmt) => {
            write_with_statement(w, *with_stmt, stmt.span, ctx);
        }
        ast::StatementKind::SwitchStatement(switch_stmt) => {
            write_switch_statement(w, switch_stmt, stmt.span, ctx);
        }
        &ast::StatementKind::TryStatement(try_stmt) => {
            write_try_statement(w, ctx.view.try_stmt(try_stmt), stmt.span, ctx);
        }
        ast::StatementKind::ThrowStatement(throw_stmt) => {
            write_throw_statement(w, *throw_stmt, stmt.span, ctx);
        }
        ast::StatementKind::BreakStatement(break_stmt) => {
            write_break_statement(w, break_stmt, stmt.span, ctx);
        }
        ast::StatementKind::ContinueStatement(continue_stmt) => {
            write_continue_statement(w, continue_stmt, stmt.span, ctx);
        }
        &ast::StatementKind::LabeledStatement(labeled) => {
            write_labeled_statement(w, ctx.view.labeled_stmt(labeled), stmt.span, ctx);
        }
        ast::StatementKind::EmptyStatement(_) => {
            write_bare_node(w, "EmptyStatement", ent!("EmptyStatement"), stmt.span, ctx);
        }
        ast::StatementKind::DebuggerStatement(_) => {
            write_bare_node(
                w,
                "DebuggerStatement",
                ent!("DebuggerStatement"),
                stmt.span,
                ctx,
            );
        }
        &ast::StatementKind::TSInterfaceDeclaration(iface) => {
            let iface = ctx.view.interface_decl(iface);
            write_interface_declaration(w, iface, ctx);
        }
        &ast::StatementKind::TSDeclareFunction(func) => {
            let func = ctx.view.declare_fn(func);
            write_declare_function(w, func, ctx);
        }
        &ast::StatementKind::TSEnumDeclaration(enum_decl) => {
            let enum_decl = ctx.view.enum_decl(enum_decl);
            write_enum_declaration(w, enum_decl, stmt.span, ctx);
        }
        &ast::StatementKind::TSModuleDeclaration(module_decl) => {
            let module_decl = ctx.view.module_decl(module_decl);
            write_module_declaration(w, module_decl, ctx);
        }
    }
}

/// Emits a `TSModuleDeclaration` node. Field order: `global` (only when
/// true), `id`, `body?` (omitted for shorthand ambient modules), `declare`
/// (only when true).
pub(super) fn write_module_declaration(
    w: &mut Sink<'_>,
    decl: &ast::TSModuleDeclaration,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "TSModuleDeclaration", decl.span, ctx);
    if decl.global {
        raw!(w, "TSModuleDeclaration", ",\"global\":true");
    }
    raw!(w, "TSModuleDeclaration", ",\"id\":");
    match &decl.id {
        ast::TSModuleName::Identifier(id) => write_identifier_plain(w, id, ctx),
        ast::TSModuleName::Literal(lit) => write_literal(w, lit, ctx),
    }
    if let Some(body) = &decl.body {
        raw!(w, "TSModuleDeclaration", ",\"body\":");
        match body {
            ast::TSModuleDeclarationBody::TSModuleBlock(block) => {
                node_header!(w, "TSModuleBlock", block.span, ctx);
                raw!(w, "TSModuleBlock", ",\"body\":");
                write_array(w, ctx.view.stmt_list(block.body), |w, s| {
                    write_statement(w, ctx.view.stmt(*s), ctx);
                });
                close_node(w, "TSModuleBlock", block.span, ctx);
            }
            &ast::TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
                let nested = ctx.view.module_decl(nested);
                write_module_declaration(w, nested, ctx);
            }
        }
    }
    if decl.declare {
        raw!(w, "TSModuleDeclaration", ",\"declare\":true");
    }
    close_node(w, "TSModuleDeclaration", decl.span, ctx);
}

/// Emits a `BlockStatement` node.
pub(super) fn write_block_statement(w: &mut Sink<'_>, block: &ast::BlockStatement, ctx: &Ctx<'_>) {
    node_header!(w, "BlockStatement", block.span, ctx);
    raw!(w, "BlockStatement", ",\"body\":");
    super::write_body_array(w, ctx.view.stmt_list(block.body), ctx, |w, s| {
        write_statement(w, ctx.view.stmt(*s), ctx);
    });
    close_node(w, "BlockStatement", block.span, ctx);
}

/// Emits a `VariableDeclaration` node (each declarator a `VariableDeclarator`).
/// Field order: `declare?`, `declarations` (each: `id`, `definite` only when true, `init`
/// nullable), `kind` — acorn-typescript's statement route (`tsTryParseDeclare` stamps
/// `declare` before the declaration parses). `export declare` stamps the finished node
/// instead (`parseExportDeclaration`); tsv's wire keeps the one order.
pub(super) fn write_variable_declaration(
    w: &mut Sink<'_>,
    var_decl: &ast::VariableDeclaration,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "VariableDeclaration", var_decl.span, ctx);
    if var_decl.declare {
        raw!(w, "VariableDeclaration", ",\"declare\":true");
    }
    raw!(w, "VariableDeclaration", ",\"declarations\":");
    write_array(w, ctx.view.declarators(var_decl.declarations), |w, d| {
        node_header!(w, "VariableDeclarator", d.span, ctx);
        raw!(w, "VariableDeclarator", ",\"id\":");
        write_expression(w, ctx.view.expr(d.id), ctx);
        if d.definite {
            raw!(w, "VariableDeclarator", ",\"definite\":true");
        }
        raw!(w, "VariableDeclarator", ",\"init\":");
        write_or_null(w, d.init.as_ref(), |w, e| {
            write_expression(w, ctx.view.expr(*e), ctx);
        });
        close_node(w, "VariableDeclarator", d.span, ctx);
    });
    // One literal per kind, so each appends the whole field at once rather
    // than a runtime-length copy of `as_str()`.
    match var_decl.kind {
        ast::VariableDeclarationKind::Const => {
            raw!(w, "VariableDeclaration", ",\"kind\":\"const\"");
        }
        ast::VariableDeclarationKind::Let => {
            raw!(w, "VariableDeclaration", ",\"kind\":\"let\"");
        }
        ast::VariableDeclarationKind::Var => {
            raw!(w, "VariableDeclaration", ",\"kind\":\"var\"");
        }
        ast::VariableDeclarationKind::Using => {
            raw!(w, "VariableDeclaration", ",\"kind\":\"using\"");
        }
        ast::VariableDeclarationKind::AwaitUsing => {
            raw!(w, "VariableDeclaration", ",\"kind\":\"await using\"");
        }
    }
    close_node(w, "VariableDeclaration", var_decl.span, ctx);
}
