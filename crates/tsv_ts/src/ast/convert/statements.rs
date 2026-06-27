// Statement conversion dispatcher and simple statements

use super::super::{internal, public};
use super::{
    Schema, convert_break_statement, convert_class_declaration, convert_continue_statement,
    convert_do_while_statement, convert_export_default_value, convert_export_specifier,
    convert_expression, convert_for_in_statement, convert_for_of_statement, convert_for_statement,
    convert_function_declaration, convert_if_statement, convert_import_attribute,
    convert_import_specifier, convert_labeled_statement, convert_literal,
    convert_module_export_name, convert_switch_statement, convert_throw_statement,
    convert_try_statement, convert_type_alias_declaration, convert_while_statement,
    create_location, types::convert_entity_name,
};
use std::borrow::Cow;
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationTracker, Span};

/// Convert an import/export declaration's import attributes to the public shape.
///
/// Internal `None` = no `with` clause; `Some(_)` = a clause (possibly empty
/// `with {}`). Svelte's non-`lang="ts"` schema always emits the `attributes`
/// array (even with no clause); acorn-typescript omits it only when there is no
/// clause, but emits `[]` for an empty `with {}`. Shared by the import and the
/// two re-export hosts.
fn convert_attributes<'src>(
    attributes: Option<&[internal::ImportAttribute<'_>]>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
    schema: Schema,
) -> Option<Vec<public::ImportAttribute<'src>>> {
    match attributes {
        Some(attrs) => Some(
            attrs
                .iter()
                .map(|a| convert_import_attribute(a, source, loc, interner, offset))
                .collect(),
        ),
        None => schema.is_svelte_script().then(Vec::new),
    }
}

/// Find the `export` keyword position in source, scanning past decorators.
/// Used when a decorated class is exported — acorn's export node starts at
/// `export`, not at the decorator.
fn find_export_start(source: &str, span_start: u32, offset: usize) -> u32 {
    let src_start = span_start as usize - offset;
    // Skip comments so an `export` inside one (`@dec /* export */ export …`)
    // isn't mistaken for the keyword, which would mislocate the node's start.
    tsv_lang::source_scan::find_keyword(
        source.as_bytes(),
        src_start,
        source.len(),
        b"export",
        tsv_lang::source_scan::TriviaProfile::JS,
    )
    .map_or(span_start, |pos| (pos + offset) as u32)
}

/// Main statement conversion dispatcher
pub(in crate::ast) fn convert_statement<'src>(
    stmt: &internal::Statement<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
    schema: Schema,
) -> public::Statement<'src> {
    match stmt {
        internal::Statement::ExpressionStatement(expr_stmt) => {
            // Directive: acorn stores the raw string contents without quotes,
            // taken from the source of the directive literal expression.
            let directive = expr_stmt.is_directive.then(|| {
                let raw = expr_stmt.expression.span().extract(source);
                Cow::Borrowed(&raw[1..raw.len() - 1])
            });
            public::Statement::ExpressionStatement(public::ExpressionStatement {
                node_type: "ExpressionStatement",
                start: expr_stmt.span.start,
                end: expr_stmt.span.end,
                loc: create_location(expr_stmt.span, loc, offset),
                expression: convert_expression(
                    &expr_stmt.expression,
                    source,
                    loc,
                    interner,
                    offset,
                ),
                directive,
            })
        }
        internal::Statement::VariableDeclaration(var_decl) => {
            public::Statement::VariableDeclaration(convert_variable_declaration(
                var_decl, source, loc, interner, offset,
            ))
        }
        internal::Statement::TSTypeAliasDeclaration(type_alias) => {
            public::Statement::TSTypeAliasDeclaration(convert_type_alias_declaration(
                type_alias, source, loc, interner, offset,
            ))
        }
        internal::Statement::ReturnStatement(ret) => {
            public::Statement::ReturnStatement(public::ReturnStatement {
                node_type: "ReturnStatement",
                start: ret.span.start,
                end: ret.span.end,
                loc: create_location(ret.span, loc, offset),
                argument: ret
                    .argument
                    .as_ref()
                    .map(|expr| Box::new(convert_expression(expr, source, loc, interner, offset))),
            })
        }
        internal::Statement::BlockStatement(block) => public::Statement::BlockStatement(
            convert_block_statement(block, source, loc, interner, offset),
        ),
        internal::Statement::FunctionDeclaration(func_decl) => {
            public::Statement::FunctionDeclaration(convert_function_declaration(
                func_decl, source, loc, interner, offset,
            ))
        }
        internal::Statement::ClassDeclaration(class_decl) => public::Statement::ClassDeclaration(
            convert_class_declaration(class_decl, source, loc, interner, offset),
        ),
        internal::Statement::ExportNamedDeclaration(export_decl) => {
            let export_kind = match export_decl.export_kind {
                internal::ExportKind::Value => {
                    if schema.is_svelte_script() {
                        None
                    } else {
                        Some("value")
                    }
                }
                internal::ExportKind::Type => Some("type"),
            };
            // `attributes`: see `convert_attributes` (None = no `with` clause).
            let attributes = convert_attributes(
                export_decl.attributes,
                source,
                loc,
                interner,
                offset,
                schema,
            );
            let export_start = if let Some(internal::Statement::ClassDeclaration(class)) =
                export_decl.declaration
            {
                if class.decorators.is_some() {
                    find_export_start(source, export_decl.span.start, offset)
                } else {
                    export_decl.span.start
                }
            } else {
                export_decl.span.start
            };
            let export_span = Span::new(export_start, export_decl.span.end);
            public::Statement::ExportNamedDeclaration(public::ExportNamedDeclaration {
                node_type: "ExportNamedDeclaration",
                start: export_start,
                end: export_decl.span.end,
                loc: create_location(export_span, loc, offset),
                export_kind,
                declaration: export_decl
                    .declaration
                    .as_ref()
                    .map(|d| Box::new(convert_statement(d, source, loc, interner, offset, schema))),
                specifiers: export_decl
                    .specifiers
                    .iter()
                    .map(|s| convert_export_specifier(s, source, loc, interner, offset, schema))
                    .collect(),
                // TODO: Consider whether source should be stored differently
                // (e.g., just the module name string vs full Literal node)
                source: export_decl
                    .source
                    .as_ref()
                    .map(|s| convert_literal(s, source, loc, offset)),
                attributes,
            })
        }
        internal::Statement::ExportDefaultDeclaration(export_decl) => {
            let export_kind = if schema.is_svelte_script() {
                None
            } else {
                Some("value")
            };
            let export_start = if let internal::ExportDefaultValue::ClassDeclaration(class) =
                &export_decl.declaration
            {
                if class.decorators.is_some() {
                    find_export_start(source, export_decl.span.start, offset)
                } else {
                    export_decl.span.start
                }
            } else {
                export_decl.span.start
            };
            let export_span = Span::new(export_start, export_decl.span.end);
            public::Statement::ExportDefaultDeclaration(public::ExportDefaultDeclaration {
                node_type: "ExportDefaultDeclaration",
                start: export_start,
                end: export_decl.span.end,
                loc: create_location(export_span, loc, offset),
                export_kind,
                declaration: convert_export_default_value(
                    &export_decl.declaration,
                    source,
                    loc,
                    interner,
                    offset,
                ),
            })
        }
        internal::Statement::ExportAllDeclaration(export_decl) => {
            let export_kind = match export_decl.export_kind {
                internal::ExportKind::Value => {
                    if schema.is_svelte_script() {
                        None
                    } else {
                        Some("value")
                    }
                }
                internal::ExportKind::Type => Some("type"),
            };
            // `attributes`: see `convert_attributes` (None = no `with` clause).
            let attributes = convert_attributes(
                export_decl.attributes,
                source,
                loc,
                interner,
                offset,
                schema,
            );
            public::Statement::ExportAllDeclaration(public::ExportAllDeclaration {
                node_type: "ExportAllDeclaration",
                start: export_decl.span.start,
                end: export_decl.span.end,
                loc: create_location(export_decl.span, loc, offset),
                export_kind,
                exported: export_decl
                    .exported
                    .as_ref()
                    .map(|name| convert_module_export_name(name, source, loc, interner, offset)),
                source: convert_literal(&export_decl.source, source, loc, offset),
                attributes,
            })
        }
        internal::Statement::TSExportAssignment(export_assign) => {
            public::Statement::TSExportAssignment(public::TSExportAssignment {
                node_type: "TSExportAssignment",
                start: export_assign.span.start,
                end: export_assign.span.end,
                loc: create_location(export_assign.span, loc, offset),
                expression: convert_expression(
                    &export_assign.expression,
                    source,
                    loc,
                    interner,
                    offset,
                ),
            })
        }
        internal::Statement::ImportDeclaration(import_decl) => {
            let import_kind = match import_decl.import_kind {
                internal::ImportKind::Value => {
                    if schema.is_svelte_script() {
                        None
                    } else {
                        Some("value")
                    }
                }
                internal::ImportKind::Type => Some("type"),
            };
            let attributes = convert_attributes(
                import_decl.attributes,
                source,
                loc,
                interner,
                offset,
                schema,
            );
            public::Statement::ImportDeclaration(public::ImportDeclaration {
                node_type: "ImportDeclaration",
                start: import_decl.span.start,
                end: import_decl.span.end,
                loc: create_location(import_decl.span, loc, offset),
                import_kind,
                phase: import_decl.phase.as_str(),
                specifiers: import_decl
                    .specifiers
                    .iter()
                    .map(|s| convert_import_specifier(s, source, loc, interner, offset, schema))
                    .collect(),
                source: convert_literal(&import_decl.source, source, loc, offset),
                attributes,
            })
        }
        internal::Statement::TSImportEqualsDeclaration(import_eq) => {
            public::Statement::TSImportEqualsDeclaration(convert_import_equals_declaration(
                import_eq, source, loc, interner, offset,
            ))
        }
        // Control flow statements
        internal::Statement::IfStatement(if_stmt) => public::Statement::IfStatement(
            convert_if_statement(if_stmt, source, loc, interner, offset),
        ),
        internal::Statement::ForStatement(for_stmt) => public::Statement::ForStatement(
            convert_for_statement(for_stmt, source, loc, interner, offset),
        ),
        internal::Statement::ForInStatement(for_in) => public::Statement::ForInStatement(
            convert_for_in_statement(for_in, source, loc, interner, offset),
        ),
        internal::Statement::ForOfStatement(for_of) => public::Statement::ForOfStatement(
            convert_for_of_statement(for_of, source, loc, interner, offset),
        ),
        internal::Statement::WhileStatement(while_stmt) => public::Statement::WhileStatement(
            convert_while_statement(while_stmt, source, loc, interner, offset),
        ),
        internal::Statement::DoWhileStatement(do_while) => public::Statement::DoWhileStatement(
            convert_do_while_statement(do_while, source, loc, interner, offset),
        ),
        internal::Statement::SwitchStatement(switch_stmt) => public::Statement::SwitchStatement(
            convert_switch_statement(switch_stmt, source, loc, interner, offset),
        ),
        internal::Statement::TryStatement(try_stmt) => public::Statement::TryStatement(
            convert_try_statement(try_stmt, source, loc, interner, offset),
        ),
        internal::Statement::ThrowStatement(throw_stmt) => public::Statement::ThrowStatement(
            convert_throw_statement(throw_stmt, source, loc, interner, offset),
        ),
        internal::Statement::BreakStatement(break_stmt) => public::Statement::BreakStatement(
            convert_break_statement(break_stmt, source, loc, interner, offset),
        ),
        internal::Statement::ContinueStatement(continue_stmt) => {
            public::Statement::ContinueStatement(convert_continue_statement(
                continue_stmt,
                source,
                loc,
                interner,
                offset,
            ))
        }
        internal::Statement::LabeledStatement(labeled) => public::Statement::LabeledStatement(
            convert_labeled_statement(labeled, source, loc, interner, offset),
        ),
        internal::Statement::EmptyStatement(empty) => {
            public::Statement::EmptyStatement(public::EmptyStatement {
                node_type: "EmptyStatement",
                start: empty.span.start,
                end: empty.span.end,
                loc: create_location(empty.span, loc, offset),
            })
        }
        internal::Statement::DebuggerStatement(dbg) => {
            public::Statement::DebuggerStatement(public::DebuggerStatement {
                node_type: "DebuggerStatement",
                start: dbg.span.start,
                end: dbg.span.end,
                loc: create_location(dbg.span, loc, offset),
            })
        }
        internal::Statement::TSInterfaceDeclaration(iface) => {
            public::Statement::TSInterfaceDeclaration(super::convert_interface_declaration(
                iface, source, loc, interner, offset,
            ))
        }
        internal::Statement::TSDeclareFunction(func) => public::Statement::TSDeclareFunction(
            super::convert_declare_function(func, source, loc, interner, offset),
        ),
        internal::Statement::TSEnumDeclaration(enum_decl) => public::Statement::TSEnumDeclaration(
            super::convert_enum_declaration(enum_decl, source, loc, interner, offset),
        ),
        internal::Statement::TSModuleDeclaration(module_decl) => {
            public::Statement::TSModuleDeclaration(convert_module_declaration(
                module_decl,
                source,
                loc,
                interner,
                offset,
            ))
        }
    }
}

/// Convert TypeScript module/namespace declaration
pub(in crate::ast) fn convert_module_declaration<'src>(
    decl: &internal::TSModuleDeclaration<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSModuleDeclaration<'src> {
    public::TSModuleDeclaration {
        node_type: "TSModuleDeclaration",
        start: decl.span.start,
        end: decl.span.end,
        loc: create_location(decl.span, loc, offset),
        id: convert_module_name(&decl.id, source, loc, interner, offset),
        body: decl
            .body
            .as_ref()
            .map(|b| convert_module_declaration_body(b, source, loc, interner, offset)),
        declare: decl.declare,
        global: decl.global,
    }
}

/// Convert module/namespace name (identifier or string literal)
fn convert_module_name<'src>(
    name: &internal::TSModuleName<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSModuleName<'src> {
    match name {
        internal::TSModuleName::Identifier(id) => {
            public::TSModuleName::Identifier(convert_identifier(id, source, loc, interner, offset))
        }
        internal::TSModuleName::Literal(lit) => {
            public::TSModuleName::Literal(convert_literal(lit, source, loc, offset))
        }
    }
}

/// Convert module declaration body
fn convert_module_declaration_body<'src>(
    body: &internal::TSModuleDeclarationBody<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSModuleDeclarationBody<'src> {
    match body {
        internal::TSModuleDeclarationBody::TSModuleBlock(block) => {
            public::TSModuleDeclarationBody::TSModuleBlock(convert_module_block(
                block, source, loc, interner, offset,
            ))
        }
        internal::TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            public::TSModuleDeclarationBody::TSModuleDeclaration(Box::new(
                convert_module_declaration(nested, source, loc, interner, offset),
            ))
        }
    }
}

/// Convert module block
fn convert_module_block<'src>(
    block: &internal::TSModuleBlock<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSModuleBlock<'src> {
    // TSModuleBlock is always in TypeScript context (declare namespace/module)
    public::TSModuleBlock {
        node_type: "TSModuleBlock",
        start: block.span.start,
        end: block.span.end,
        loc: create_location(block.span, loc, offset),
        body: block
            .body
            .iter()
            .map(|s| convert_statement(s, source, loc, interner, offset, Schema::Acorn))
            .collect(),
    }
}

pub(in crate::ast) fn convert_block_statement<'src>(
    block: &internal::BlockStatement<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::BlockStatement<'src> {
    // BlockStatement is always in TypeScript context (function bodies, etc.)
    public::BlockStatement {
        node_type: "BlockStatement",
        start: block.span.start,
        end: block.span.end,
        loc: create_location(block.span, loc, offset),
        body: block
            .body
            .iter()
            .map(|s| convert_statement(s, source, loc, interner, offset, Schema::Acorn))
            .collect(),
    }
}

pub fn convert_variable_declaration<'src>(
    var_decl: &internal::VariableDeclaration<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::VariableDeclaration<'src> {
    public::VariableDeclaration {
        node_type: "VariableDeclaration",
        start: var_decl.span.start,
        end: var_decl.span.end,
        loc: create_location(var_decl.span, loc, offset),
        declarations: var_decl
            .declarations
            .iter()
            .map(|d| convert_variable_declarator(d, source, loc, interner, offset))
            .collect(),
        kind: var_decl.kind.as_str(),
        declare: var_decl.declare,
    }
}

pub(in crate::ast) fn convert_variable_declarator<'src>(
    declarator: &internal::VariableDeclarator<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::VariableDeclarator<'src> {
    public::VariableDeclarator {
        node_type: "VariableDeclarator",
        start: declarator.span.start,
        end: declarator.span.end,
        loc: create_location(declarator.span, loc, offset),
        // id can be Identifier, ArrayPattern, or ObjectPattern
        id: convert_expression(&declarator.id, source, loc, interner, offset),
        definite: declarator.definite,
        init: declarator
            .init
            .as_ref()
            .map(|expr| convert_expression(expr, source, loc, interner, offset)),
    }
}

pub(in crate::ast) fn convert_identifier<'src>(
    id: &internal::Identifier<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Identifier<'src> {
    public::Identifier {
        node_type: "Identifier",
        start: id.span.start,
        end: id.span.end,
        loc: create_location(id.span, loc, offset),
        name: public::name_cow(id.span, source, id.name, interner),
        optional: false,
        type_annotation: None,
        decorators: Vec::new(),
    }
}

fn convert_import_equals_declaration<'src>(
    decl: &internal::TSImportEqualsDeclaration<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSImportEqualsDeclaration<'src> {
    public::TSImportEqualsDeclaration {
        node_type: "TSImportEqualsDeclaration",
        start: decl.span.start,
        end: decl.span.end,
        loc: create_location(decl.span, loc, offset),
        import_kind: match decl.import_kind {
            internal::ImportKind::Value => "value",
            internal::ImportKind::Type => "type",
        },
        is_export: decl.is_export,
        id: convert_identifier(&decl.id, source, loc, interner, offset),
        module_reference: convert_module_reference(
            &decl.module_reference,
            source,
            loc,
            interner,
            offset,
        ),
    }
}

fn convert_module_reference<'src>(
    module_ref: &internal::TSModuleReference<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSModuleReference<'src> {
    match module_ref {
        internal::TSModuleReference::ExternalModuleReference(ext_ref) => {
            public::TSModuleReference::ExternalModuleReference(public::TSExternalModuleReference {
                node_type: "TSExternalModuleReference",
                start: ext_ref.span.start,
                end: ext_ref.span.end,
                loc: create_location(ext_ref.span, loc, offset),
                expression: convert_literal(&ext_ref.expression, source, loc, offset),
            })
        }
        internal::TSModuleReference::EntityName(entity_name) => {
            public::TSModuleReference::EntityName(convert_entity_name(
                entity_name,
                source,
                loc,
                interner,
                offset,
            ))
        }
    }
}
