// Statement conversion dispatcher and simple statements

use super::super::{internal, public};
use super::{
    convert_break_statement, convert_class_declaration, convert_continue_statement,
    convert_do_while_statement, convert_export_default_value, convert_export_specifier,
    convert_expression, convert_for_in_statement, convert_for_of_statement, convert_for_statement,
    convert_function_declaration, convert_if_statement, convert_import_attribute,
    convert_import_specifier, convert_labeled_statement, convert_literal, convert_switch_statement,
    convert_throw_statement, convert_try_statement, convert_type_alias_declaration,
    convert_while_statement, create_location,
};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationTracker;

/// Main statement conversion dispatcher
pub(in crate::ast) fn convert_statement(
    stmt: &internal::Statement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Statement {
    match stmt {
        internal::Statement::ExpressionStatement(expr_stmt) => {
            public::Statement::ExpressionStatement(public::ExpressionStatement {
                node_type: "ExpressionStatement".to_string(),
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
            })
        }
        internal::Statement::VariableDeclaration(var_decl) => {
            public::Statement::VariableDeclaration(public::VariableDeclaration {
                node_type: "VariableDeclaration".to_string(),
                start: var_decl.span.start,
                end: var_decl.span.end,
                loc: create_location(var_decl.span, loc, offset),
                declarations: var_decl
                    .declarations
                    .iter()
                    .map(|d| convert_variable_declarator(d, source, loc, interner, offset))
                    .collect(),
                kind: var_decl.kind.as_str().to_string(),
            })
        }
        internal::Statement::TSTypeAliasDeclaration(type_alias) => {
            public::Statement::TSTypeAliasDeclaration(convert_type_alias_declaration(
                type_alias, loc, interner, offset,
            ))
        }
        internal::Statement::ReturnStatement(ret) => {
            public::Statement::ReturnStatement(public::ReturnStatement {
                node_type: "ReturnStatement".to_string(),
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
            public::Statement::ExportNamedDeclaration(public::ExportNamedDeclaration {
                node_type: "ExportNamedDeclaration".to_string(),
                start: export_decl.span.start,
                end: export_decl.span.end,
                loc: create_location(export_decl.span, loc, offset),
                // TODO: Support "type" for TypeScript `export type { T }`
                export_kind: "value".to_string(),
                declaration: export_decl
                    .declaration
                    .as_ref()
                    .map(|d| Box::new(convert_statement(d, source, loc, interner, offset))),
                specifiers: export_decl
                    .specifiers
                    .iter()
                    .map(|s| convert_export_specifier(s, loc, interner, offset))
                    .collect(),
                // TODO: Consider whether source should be stored differently
                // (e.g., just the module name string vs full Literal node)
                source: export_decl
                    .source
                    .as_ref()
                    .map(|s| convert_literal(s, source, loc, offset)),
            })
        }
        internal::Statement::ExportDefaultDeclaration(export_decl) => {
            public::Statement::ExportDefaultDeclaration(public::ExportDefaultDeclaration {
                node_type: "ExportDefaultDeclaration".to_string(),
                start: export_decl.span.start,
                end: export_decl.span.end,
                loc: create_location(export_decl.span, loc, offset),
                export_kind: "value".to_string(),
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
            public::Statement::ExportAllDeclaration(public::ExportAllDeclaration {
                node_type: "ExportAllDeclaration".to_string(),
                start: export_decl.span.start,
                end: export_decl.span.end,
                loc: create_location(export_decl.span, loc, offset),
                export_kind: "value".to_string(),
                exported: export_decl
                    .exported
                    .as_ref()
                    .map(|id| convert_identifier(id, loc, interner, offset)),
                source: convert_literal(&export_decl.source, source, loc, offset),
            })
        }
        internal::Statement::ImportDeclaration(import_decl) => {
            public::Statement::ImportDeclaration(public::ImportDeclaration {
                node_type: "ImportDeclaration".to_string(),
                start: import_decl.span.start,
                end: import_decl.span.end,
                loc: create_location(import_decl.span, loc, offset),
                import_kind: match import_decl.import_kind {
                    internal::ImportKind::Value => "value".to_string(),
                    internal::ImportKind::Type => "type".to_string(),
                },
                specifiers: import_decl
                    .specifiers
                    .iter()
                    .map(|s| convert_import_specifier(s, loc, interner, offset))
                    .collect(),
                source: convert_literal(&import_decl.source, source, loc, offset),
                attributes: import_decl
                    .attributes
                    .iter()
                    .map(|a| convert_import_attribute(a, source, loc, interner, offset))
                    .collect(),
            })
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
            convert_break_statement(break_stmt, loc, interner, offset),
        ),
        internal::Statement::ContinueStatement(continue_stmt) => {
            public::Statement::ContinueStatement(convert_continue_statement(
                continue_stmt,
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
                node_type: "EmptyStatement".to_string(),
                start: empty.span.start,
                end: empty.span.end,
                loc: create_location(empty.span, loc, offset),
            })
        }
    }
}

pub(in crate::ast) fn convert_block_statement(
    block: &internal::BlockStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::BlockStatement {
    public::BlockStatement {
        node_type: "BlockStatement".to_string(),
        start: block.span.start,
        end: block.span.end,
        loc: create_location(block.span, loc, offset),
        body: block
            .body
            .iter()
            .map(|s| convert_statement(s, source, loc, interner, offset))
            .collect(),
    }
}

pub(in crate::ast) fn convert_variable_declarator(
    declarator: &internal::VariableDeclarator,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::VariableDeclarator {
    public::VariableDeclarator {
        node_type: "VariableDeclarator".to_string(),
        start: declarator.span.start,
        end: declarator.span.end,
        loc: create_location(declarator.span, loc, offset),
        // id can be Identifier, ArrayPattern, or ObjectPattern
        id: convert_expression(&declarator.id, source, loc, interner, offset),
        init: declarator
            .init
            .as_ref()
            .map(|expr| convert_expression(expr, source, loc, interner, offset)),
    }
}

pub(in crate::ast) fn convert_identifier(
    id: &internal::Identifier,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Identifier {
    use tsv_lang::InfallibleResolve;

    public::Identifier {
        node_type: "Identifier".to_string(),
        start: id.span.start,
        end: id.span.end,
        loc: create_location(id.span, loc, offset),
        name: interner.resolve_infallible(id.name).to_string(),
        optional: false,
        type_annotation: None,
    }
}
