// Conversion from internal AST to public AST

use super::internal;
use super::public;
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationTracker, Span};

/// Convert tsv_lang::SourceLocation to public::SourceLocation
///
/// Converts from the generic location type to the TypeScript-specific public type
/// with serde derives.
#[inline]
fn to_public_location(loc: tsv_lang::SourceLocation) -> public::SourceLocation {
    public::SourceLocation {
        start: public::Position {
            line: loc.start.line,
            column: loc.start.column,
        },
        end: public::Position {
            line: loc.end.line,
            column: loc.end.column,
        },
    }
}

/// Create source location, automatically handling offset if needed
///
/// Unified helper that eliminates repetitive if/else checks throughout conversion.
/// When offset is 0, uses fast path directly. When offset is non-zero, adjusts span accordingly.
#[inline]
fn create_location(span: Span, tracker: &LocationTracker, offset: usize) -> public::SourceLocation {
    let loc = if offset == 0 {
        tracker.span_to_location(span)
    } else {
        tracker.span_to_location_with_offset(span, offset)
    };
    to_public_location(loc)
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
                internal::LiteralValue::Boolean(b) => serde_json::Value::Bool(*b),
                internal::LiteralValue::Null => serde_json::Value::Null,
            };
            // Extract raw from source using span
            let raw = lit.span.extract(source);
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
                    .map(|p| convert_object_property(p, source, loc, interner, offset))
                    .collect(),
            })
        }
        internal::Expression::ArrayExpression(arr) => {
            public::Expression::ArrayExpression(public::ArrayExpression {
                node_type: "ArrayExpression".to_string(),
                start: arr.span.start,
                end: arr.span.end,
                loc: create_location(arr.span, loc, offset),
                elements: arr
                    .elements
                    .iter()
                    .map(|e| {
                        e.as_ref()
                            .map(|expr| convert_expression(expr, source, loc, interner, offset))
                    })
                    .collect(),
            })
        }
        internal::Expression::UnaryExpression(unary) => {
            public::Expression::UnaryExpression(public::UnaryExpression {
                node_type: "UnaryExpression".to_string(),
                start: unary.span.start,
                end: unary.span.end,
                loc: create_location(unary.span, loc, offset),
                operator: unary.operator.as_str().to_string(),
                prefix: unary.prefix,
                argument: Box::new(convert_expression(
                    &unary.argument,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::BinaryExpression(binary) => {
            // Determine node type: LogicalExpression for &&, ||, ?? - otherwise BinaryExpression
            let node_type = match binary.operator {
                internal::BinaryOperator::AmpersandAmpersand
                | internal::BinaryOperator::PipePipe
                | internal::BinaryOperator::QuestionQuestion => "LogicalExpression",
                _ => "BinaryExpression",
            };

            public::Expression::BinaryExpression(public::BinaryExpression {
                node_type: node_type.to_string(),
                start: binary.span.start,
                end: binary.span.end,
                loc: create_location(binary.span, loc, offset),
                left: Box::new(convert_expression(
                    &binary.left,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                operator: binary.operator.as_str().to_string(),
                right: Box::new(convert_expression(
                    &binary.right,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::ArrowFunctionExpression(arrow) => {
            let body = match &arrow.body {
                internal::ArrowFunctionBody::Expression(expr) => {
                    public::ArrowFunctionBody::Expression(Box::new(convert_expression(
                        expr, source, loc, interner, offset,
                    )))
                }
                internal::ArrowFunctionBody::BlockStatement { span } => {
                    public::ArrowFunctionBody::BlockStatement(public::BlockStatement {
                        node_type: "BlockStatement".to_string(),
                        start: span.start,
                        end: span.end,
                        loc: create_location(*span, loc, offset),
                        body: vec![], // TODO: Parse block body statements
                    })
                }
            };
            public::Expression::ArrowFunctionExpression(public::ArrowFunctionExpression {
                node_type: "ArrowFunctionExpression".to_string(),
                start: arrow.span.start,
                end: arrow.span.end,
                loc: create_location(arrow.span, loc, offset),
                id: None,
                expression: arrow.expression,
                generator: false,
                is_async: false,
                params: arrow
                    .params
                    .iter()
                    .map(|p| public::Identifier {
                        node_type: "Identifier".to_string(),
                        start: p.span.start,
                        end: p.span.end,
                        loc: create_location(p.span, loc, offset),
                        name: interner.resolve(p.name).unwrap().to_string(),
                        type_annotation: None,
                    })
                    .collect(),
                body,
            })
        }
        internal::Expression::SpreadElement(spread) => {
            public::Expression::SpreadElement(public::SpreadElement {
                node_type: "SpreadElement".to_string(),
                start: spread.span.start,
                end: spread.span.end,
                loc: create_location(spread.span, loc, offset),
                argument: Box::new(convert_expression(
                    &spread.argument,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::CallExpression(call) => {
            public::Expression::CallExpression(public::CallExpression {
                node_type: "CallExpression".to_string(),
                start: call.span.start,
                end: call.span.end,
                loc: create_location(call.span, loc, offset),
                callee: Box::new(convert_expression(
                    &call.callee,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                arguments: call
                    .arguments
                    .iter()
                    .map(|arg| convert_expression(arg, source, loc, interner, offset))
                    .collect(),
                optional: call.optional,
            })
        }
        internal::Expression::MemberExpression(member) => {
            public::Expression::MemberExpression(public::MemberExpression {
                node_type: "MemberExpression".to_string(),
                start: member.span.start,
                end: member.span.end,
                loc: create_location(member.span, loc, offset),
                object: Box::new(convert_expression(
                    &member.object,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                property: Box::new(convert_expression(
                    &member.property,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                computed: member.computed,
                optional: member.optional,
            })
        }
        internal::Expression::ConditionalExpression(cond) => {
            public::Expression::ConditionalExpression(public::ConditionalExpression {
                node_type: "ConditionalExpression".to_string(),
                start: cond.span.start,
                end: cond.span.end,
                loc: create_location(cond.span, loc, offset),
                test: Box::new(convert_expression(
                    &cond.test, source, loc, interner, offset,
                )),
                consequent: Box::new(convert_expression(
                    &cond.consequent,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                alternate: Box::new(convert_expression(
                    &cond.alternate,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
    }
}

fn convert_object_property(
    prop: &internal::ObjectProperty,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ObjectProperty {
    match prop {
        internal::ObjectProperty::Property(p) => {
            public::ObjectProperty::Property(convert_property(p, source, loc, interner, offset))
        }
        internal::ObjectProperty::SpreadElement(s) => {
            public::ObjectProperty::SpreadElement(public::SpreadElement {
                node_type: "SpreadElement".to_string(),
                start: s.span.start,
                end: s.span.end,
                loc: create_location(s.span, loc, offset),
                argument: Box::new(convert_expression(
                    &s.argument,
                    source,
                    loc,
                    interner,
                    offset,
                )),
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
        value: Box::new(convert_expression(
            &prop.value,
            source,
            loc,
            interner,
            offset,
        )),
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
