// Conversion from internal AST to public AST

use super::internal;
use super::public;
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationTracker, Span};

// Helper to create SourceLocation from Span
fn create_source_location(span: Span, tracker: &LocationTracker) -> public::SourceLocation {
    let (start_line, start_col) = tracker.get_line_column(span.start as usize);
    let (end_line, end_col) = tracker.get_line_column(span.end as usize);

    public::SourceLocation {
        start: public::Position {
            line: start_line,
            column: start_col,
        },
        end: public::Position {
            line: end_line,
            column: end_col,
        },
    }
}

// Helper to create SourceLocation with position offset
// Used for embedded content where AST has global positions but LocationTracker is from substring
fn create_source_location_with_offset(
    span: Span,
    tracker: &LocationTracker,
    offset: usize,
) -> public::SourceLocation {
    // Subtract offset to get positions relative to the tracker's source
    let adjusted_span = Span {
        start: span.start - offset as u32,
        end: span.end - offset as u32,
    };
    create_source_location(adjusted_span, tracker)
}

/// Create source location, automatically handling offset if needed
///
/// Unified helper that eliminates repetitive if/else checks throughout conversion.
/// When offset is 0, uses fast path directly. When offset is non-zero, adjusts span accordingly.
#[inline]
fn create_location(span: Span, tracker: &LocationTracker, offset: usize) -> public::SourceLocation {
    if offset == 0 {
        create_source_location(span, tracker)
    } else {
        create_source_location_with_offset(span, tracker, offset)
    }
}

pub fn convert_program(
    program: &internal::Program,
    source: &str,
    loc: &LocationTracker,
) -> public::Program {
    convert_program_with_offset(program, source, loc, 0)
}

// Convert Program with position offset for embedded content
pub fn convert_program_with_offset(
    program: &internal::Program,
    source: &str,
    loc: &LocationTracker,
    offset: usize,
) -> public::Program {
    let interner = program.interner.borrow();

    public::Program {
        node_type: "Program".to_string(),
        start: program.span.start,
        end: program.span.end,
        loc: create_location(program.span, loc, offset),
        body: program
            .body
            .iter()
            .map(|s| convert_statement(s, source, loc, &interner, offset))
            .collect(),
        source_type: "module".to_string(),
    }
}

fn convert_statement(
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
    }
}

fn convert_variable_declarator(
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
        id: public::Identifier {
            node_type: "Identifier".to_string(),
            start: declarator.id.span.start,
            end: declarator.id.span.end,
            loc: create_location(declarator.id.span, loc, offset),
            name: interner.resolve(declarator.id.name).unwrap().to_string(),
            type_annotation: declarator
                .id
                .type_annotation
                .as_ref()
                .map(|ta| convert_type_annotation(ta, loc, offset)),
        },
        init: declarator
            .init
            .as_ref()
            .map(|expr| convert_expression(expr, source, loc, interner, offset)),
    }
}

pub fn convert_expression(
    expr: &internal::Expression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Expression {
    match expr {
        internal::Expression::Literal(lit) => {
            let value = match &lit.value {
                internal::LiteralValue::Number(n) => serde_json::Value::Number(
                    serde_json::Number::from_f64(*n).unwrap_or_else(|| serde_json::Number::from(0)),
                ),
                internal::LiteralValue::String { content, .. } => {
                    serde_json::Value::String(content.clone())
                }
            };
            // Extract raw from source using span
            let raw = &source[lit.span.start as usize..lit.span.end as usize];
            public::Expression::Literal(public::Literal {
                node_type: "Literal".to_string(),
                start: lit.span.start,
                end: lit.span.end,
                loc: create_location(lit.span, loc, offset),
                value,
                raw: raw.to_string(),
            })
        }
        internal::Expression::Identifier(id) => {
            public::Expression::Identifier(public::Identifier {
                node_type: "Identifier".to_string(),
                start: id.span.start,
                end: id.span.end,
                loc: create_location(id.span, loc, offset),
                name: interner.resolve(id.name).unwrap().to_string(),
                type_annotation: id
                    .type_annotation
                    .as_ref()
                    .map(|ta| convert_type_annotation(ta, loc, offset)),
            })
        }
        internal::Expression::ObjectExpression(obj) => {
            public::Expression::ObjectExpression(public::ObjectExpression {
                node_type: "ObjectExpression".to_string(),
                start: obj.span.start,
                end: obj.span.end,
                loc: create_location(obj.span, loc, offset),
                properties: obj
                    .properties
                    .iter()
                    .map(|p| convert_property(p, source, loc, interner, offset))
                    .collect(),
            })
        }
    }
}

// TODO: Support property decorators in conversion
// Convert decorator AST nodes from internal to public format
// Needed when internal::Property gains decorators field
fn convert_property(
    prop: &internal::Property,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Property {
    // TODO: Handle PropertyKind enum when refactored
    // Currently: Direct field access (method, shorthand, computed)
    // After refactor: Match on PropertyKind to extract fields
    // Also needed: Support for Get/Set property kinds (change kind: String field)
    public::Property {
        node_type: "Property".to_string(),
        start: prop.span.start,
        end: prop.span.end,
        loc: create_location(prop.span, loc, offset),
        method: prop.method,
        shorthand: prop.shorthand,
        computed: prop.computed,
        key: Box::new(convert_expression(&prop.key, source, loc, interner, offset)),
        value: Box::new(convert_expression(&prop.value, source, loc, interner, offset)),
        kind: "init".to_string(),
    }
}

fn convert_type_annotation(
    type_annotation: &internal::TSTypeAnnotation,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSTypeAnnotation {
    public::TSTypeAnnotation {
        node_type: "TSTypeAnnotation".to_string(),
        start: type_annotation.span.start,
        end: type_annotation.span.end,
        loc: create_location(type_annotation.span, loc, offset),
        type_annotation: Box::new(convert_type(&type_annotation.type_annotation, loc, offset)),
    }
}

fn convert_type(
    ts_type: &internal::TSType,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSType {
    match ts_type {
        internal::TSType::TSNumberKeyword(node) => {
            public::TSType::TSNumberKeyword(public::TSNumberKeyword {
                node_type: "TSNumberKeyword".to_string(),
                start: node.span.start,
                end: node.span.end,
                loc: create_location(node.span, loc, offset),
            })
        }
    }
}
