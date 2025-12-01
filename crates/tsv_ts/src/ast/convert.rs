// Conversion from internal AST to public AST

use super::internal;
use super::public;
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker, Span};

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
        internal::Statement::TSTypeAliasDeclaration(type_alias) => {
            public::Statement::TSTypeAliasDeclaration(public::TSTypeAliasDeclaration {
                node_type: "TSTypeAliasDeclaration".to_string(),
                start: type_alias.span.start,
                end: type_alias.span.end,
                loc: create_location(type_alias.span, loc, offset),
                id: public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: type_alias.id.span.start,
                    end: type_alias.id.span.end,
                    loc: create_location(type_alias.id.span, loc, offset),
                    name: interner.must_resolve(type_alias.id.name).to_string(),
                    type_annotation: None,
                },
                type_annotation: convert_type(&type_alias.type_annotation, loc, offset),
            })
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
            public::Statement::FunctionDeclaration(public::FunctionDeclaration {
                node_type: "FunctionDeclaration".to_string(),
                start: func_decl.span.start,
                end: func_decl.span.end,
                loc: create_location(func_decl.span, loc, offset),
                id: public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: func_decl.id.span.start,
                    end: func_decl.id.span.end,
                    loc: create_location(func_decl.id.span, loc, offset),
                    name: interner.must_resolve(func_decl.id.name).to_string(),
                    type_annotation: None,
                },
                expression: false,
                generator: func_decl.generator,
                is_async: func_decl.r#async,
                params: func_decl
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner, offset))
                    .collect(),
                body: convert_block_statement(&func_decl.body, source, loc, interner, offset),
            })
        }
        internal::Statement::ClassDeclaration(class_decl) => {
            public::Statement::ClassDeclaration(public::ClassDeclaration {
                node_type: "ClassDeclaration".to_string(),
                start: class_decl.span.start,
                end: class_decl.span.end,
                loc: create_location(class_decl.span, loc, offset),
                id: public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: class_decl.id.span.start,
                    end: class_decl.id.span.end,
                    loc: create_location(class_decl.id.span, loc, offset),
                    name: interner.must_resolve(class_decl.id.name).to_string(),
                    type_annotation: None,
                },
                super_class: class_decl
                    .super_class
                    .as_ref()
                    .map(|e| Box::new(convert_expression(e, source, loc, interner, offset))),
                body: convert_class_body(&class_decl.body, source, loc, interner, offset),
            })
        }
    }
}

fn convert_class_body(
    body: &internal::ClassBody,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ClassBody {
    public::ClassBody {
        node_type: "ClassBody".to_string(),
        start: body.span.start,
        end: body.span.end,
        loc: create_location(body.span, loc, offset),
        body: body
            .body
            .iter()
            .map(|m| convert_method_definition(m, source, loc, interner, offset))
            .collect(),
    }
}

fn convert_method_definition(
    method: &internal::MethodDefinition,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::MethodDefinition {
    // Convert the FunctionExpression value for the method
    let func = &method.value;
    let value = public::FunctionExpression {
        node_type: "FunctionExpression".to_string(),
        start: func.span.start,
        end: func.span.end,
        loc: create_location(func.span, loc, offset),
        id: func.id.as_ref().map(|id| public::Identifier {
            node_type: "Identifier".to_string(),
            start: id.span.start,
            end: id.span.end,
            loc: create_location(id.span, loc, offset),
            name: interner.must_resolve(id.name).to_string(),
            type_annotation: None,
        }),
        expression: false,
        generator: false,
        is_async: false,
        params: func
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner, offset))
            .collect(),
        body: convert_block_statement(&func.body, source, loc, interner, offset),
    };

    public::MethodDefinition {
        node_type: "MethodDefinition".to_string(),
        start: method.span.start,
        end: method.span.end,
        loc: create_location(method.span, loc, offset),
        is_static: method.is_static,
        computed: method.computed,
        key: Box::new(convert_expression(
            &method.key,
            source,
            loc,
            interner,
            offset,
        )),
        kind: method.kind.as_str().to_string(),
        value,
    }
}

fn convert_block_statement(
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
        // id can be Identifier, ArrayPattern, or ObjectPattern
        id: convert_expression(&declarator.id, source, loc, interner, offset),
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
                // undefined is represented as a special identifier in most ASTs
                // but as a literal in ours - serialize as null for JSON compatibility
                internal::LiteralValue::Undefined => serde_json::Value::Null,
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
                name: interner.must_resolve(id.name).to_string(),
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
        internal::Expression::UpdateExpression(update) => {
            public::Expression::UpdateExpression(public::UpdateExpression {
                node_type: "UpdateExpression".to_string(),
                start: update.span.start,
                end: update.span.end,
                loc: create_location(update.span, loc, offset),
                operator: update.operator.as_str().to_string(),
                prefix: update.prefix,
                argument: Box::new(convert_expression(
                    &update.argument,
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
                    .map(|p| convert_expression(p, source, loc, interner, offset))
                    .collect(),
                body,
            })
        }
        internal::Expression::FunctionExpression(func) => {
            public::Expression::FunctionExpression(public::FunctionExpression {
                node_type: "FunctionExpression".to_string(),
                start: func.span.start,
                end: func.span.end,
                loc: create_location(func.span, loc, offset),
                id: func.id.as_ref().map(|id| public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: id.span.start,
                    end: id.span.end,
                    loc: create_location(id.span, loc, offset),
                    name: interner.must_resolve(id.name).to_string(),
                    type_annotation: None,
                }),
                expression: false,
                generator: false,
                is_async: false,
                params: func
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner, offset))
                    .collect(),
                body: convert_block_statement(&func.body, source, loc, interner, offset),
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
        internal::Expression::NewExpression(new_expr) => {
            public::Expression::NewExpression(public::NewExpression {
                node_type: "NewExpression".to_string(),
                start: new_expr.span.start,
                end: new_expr.span.end,
                loc: create_location(new_expr.span, loc, offset),
                callee: Box::new(convert_expression(
                    &new_expr.callee,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                arguments: new_expr
                    .arguments
                    .iter()
                    .map(|arg| convert_expression(arg, source, loc, interner, offset))
                    .collect(),
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
        internal::Expression::TemplateLiteral(template) => public::Expression::TemplateLiteral(
            convert_template_literal(template, source, loc, interner, offset),
        ),
        internal::Expression::TaggedTemplateExpression(tagged) => {
            public::Expression::TaggedTemplateExpression(public::TaggedTemplateExpression {
                node_type: "TaggedTemplateExpression".to_string(),
                start: tagged.span.start,
                end: tagged.span.end,
                loc: create_location(tagged.span, loc, offset),
                tag: Box::new(convert_expression(
                    &tagged.tag,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                quasi: convert_template_literal(&tagged.quasi, source, loc, interner, offset),
            })
        }
        internal::Expression::AwaitExpression(await_expr) => {
            public::Expression::AwaitExpression(public::AwaitExpression {
                node_type: "AwaitExpression".to_string(),
                start: await_expr.span.start,
                end: await_expr.span.end,
                loc: create_location(await_expr.span, loc, offset),
                argument: Box::new(convert_expression(
                    &await_expr.argument,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::SequenceExpression(seq) => {
            public::Expression::SequenceExpression(public::SequenceExpression {
                node_type: "SequenceExpression".to_string(),
                start: seq.span.start,
                end: seq.span.end,
                loc: create_location(seq.span, loc, offset),
                expressions: seq
                    .expressions
                    .iter()
                    .map(|e| convert_expression(e, source, loc, interner, offset))
                    .collect(),
            })
        }
        internal::Expression::RegexLiteral(regex) => {
            // Reconstruct raw from source: /pattern/flags
            let raw = regex.span.extract(source).to_string();
            public::Expression::RegexLiteral(public::RegexLiteral {
                node_type: "Literal".to_string(), // Regex uses "Literal" type in acorn/Svelte AST
                start: regex.span.start,
                end: regex.span.end,
                loc: create_location(regex.span, loc, offset),
                value: serde_json::Value::Object(serde_json::Map::new()), // Empty object {}
                raw,
                regex: public::RegexValue {
                    pattern: regex.pattern.clone(),
                    flags: regex.flags.clone(),
                },
            })
        }
        internal::Expression::Super(s) => public::Expression::Super(public::Super {
            node_type: "Super".to_string(),
            start: s.span.start,
            end: s.span.end,
            loc: create_location(s.span, loc, offset),
        }),
        internal::Expression::AssignmentExpression(assign) => {
            public::Expression::AssignmentExpression(public::AssignmentExpression {
                node_type: "AssignmentExpression".to_string(),
                start: assign.span.start,
                end: assign.span.end,
                loc: create_location(assign.span, loc, offset),
                operator: assign.operator.as_str().to_string(),
                left: Box::new(convert_expression(
                    &assign.left,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                right: Box::new(convert_expression(
                    &assign.right,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::ObjectPattern(obj) => {
            public::Expression::ObjectPattern(public::ObjectPattern {
                node_type: "ObjectPattern".to_string(),
                start: obj.span.start,
                end: obj.span.end,
                loc: create_location(obj.span, loc, offset),
                properties: obj
                    .properties
                    .iter()
                    .map(|p| convert_object_pattern_property(p, source, loc, interner, offset))
                    .collect(),
            })
        }
        internal::Expression::ArrayPattern(arr) => {
            public::Expression::ArrayPattern(public::ArrayPattern {
                node_type: "ArrayPattern".to_string(),
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
        internal::Expression::AssignmentPattern(pattern) => {
            public::Expression::AssignmentPattern(public::AssignmentPattern {
                node_type: "AssignmentPattern".to_string(),
                start: pattern.span.start,
                end: pattern.span.end,
                loc: create_location(pattern.span, loc, offset),
                left: Box::new(convert_expression(
                    &pattern.left,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                right: Box::new(convert_expression(
                    &pattern.right,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::RestElement(rest) => {
            public::Expression::RestElement(public::RestElement {
                node_type: "RestElement".to_string(),
                start: rest.span.start,
                end: rest.span.end,
                loc: create_location(rest.span, loc, offset),
                argument: Box::new(convert_expression(
                    &rest.argument,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
    }
}

fn convert_template_literal(
    template: &internal::TemplateLiteral,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TemplateLiteral {
    let _ = (source, interner); // Suppress unused warnings - available for future use
    public::TemplateLiteral {
        node_type: "TemplateLiteral".to_string(),
        start: template.span.start,
        end: template.span.end,
        loc: create_location(template.span, loc, offset),
        quasis: template
            .quasis
            .iter()
            .map(|q| convert_template_element(q, loc, offset))
            .collect(),
        expressions: template
            .expressions
            .iter()
            .map(|e| convert_expression(e, source, loc, interner, offset))
            .collect(),
    }
}

fn convert_template_element(
    element: &internal::TemplateElement,
    loc: &LocationTracker,
    offset: usize,
) -> public::TemplateElement {
    public::TemplateElement {
        node_type: "TemplateElement".to_string(),
        start: element.span.start,
        end: element.span.end,
        loc: create_location(element.span, loc, offset),
        value: public::TemplateElementValue {
            raw: element.raw.clone(),
            cooked: element.cooked.clone(),
        },
        tail: element.tail,
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

fn convert_object_pattern_property(
    prop: &internal::ObjectPatternProperty,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ObjectPatternProperty {
    match prop {
        internal::ObjectPatternProperty::Property(p) => public::ObjectPatternProperty::Property(
            convert_property(p, source, loc, interner, offset),
        ),
        internal::ObjectPatternProperty::RestElement(r) => {
            public::ObjectPatternProperty::RestElement(public::RestElement {
                node_type: "RestElement".to_string(),
                start: r.span.start,
                end: r.span.end,
                loc: create_location(r.span, loc, offset),
                argument: Box::new(convert_expression(
                    &r.argument,
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
        kind: prop.kind.as_str().to_string(),
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
        internal::TSType::Keyword(kw) => convert_keyword_type(kw, loc, offset),
        internal::TSType::Literal(lit) => convert_literal_type(lit, loc, offset),
    }
}

fn convert_literal_type(
    lit: &internal::TSLiteralType,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSType {
    match lit {
        internal::TSLiteralType::TemplateLiteral(template) => {
            public::TSType::TSLiteralType(public::TSLiteralType {
                node_type: "TSLiteralType".to_string(),
                start: template.span.start,
                end: template.span.end,
                loc: create_location(template.span, loc, offset),
                literal: public::TSLiteralTypeLiteral::TemplateLiteral(
                    convert_template_literal_type(template, loc, offset),
                ),
            })
        }
    }
}

fn convert_template_literal_type(
    template: &internal::TemplateLiteralType,
    loc: &LocationTracker,
    offset: usize,
) -> public::TemplateLiteralType {
    public::TemplateLiteralType {
        node_type: "TemplateLiteral".to_string(),
        start: template.span.start,
        end: template.span.end,
        loc: create_location(template.span, loc, offset),
        quasis: template
            .quasis
            .iter()
            .map(|q| convert_template_element_type(q, loc, offset))
            .collect(),
        expressions: template
            .types
            .iter()
            .map(|t| convert_type(t, loc, offset))
            .collect(),
    }
}

fn convert_template_element_type(
    elem: &internal::TemplateElement,
    loc: &LocationTracker,
    offset: usize,
) -> public::TemplateElement {
    public::TemplateElement {
        node_type: "TemplateElement".to_string(),
        start: elem.span.start,
        end: elem.span.end,
        loc: create_location(elem.span, loc, offset),
        value: public::TemplateElementValue {
            raw: elem.raw.clone(),
            cooked: elem.cooked.clone(),
        },
        tail: elem.tail,
    }
}

/// Convert internal TSKeywordType to the appropriate public type variant
fn convert_keyword_type(
    kw: &internal::TSKeywordType,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSType {
    use internal::TSKeywordKind;

    // Helper macro to reduce boilerplate - creates the public type struct
    macro_rules! make_public {
        ($variant:ident) => {{
            public::TSType::$variant(public::$variant {
                node_type: kw.kind.node_type_name().to_string(),
                start: kw.span.start,
                end: kw.span.end,
                loc: create_location(kw.span, loc, offset),
            })
        }};
    }

    match kw.kind {
        TSKeywordKind::Number => make_public!(TSNumberKeyword),
        TSKeywordKind::String => make_public!(TSStringKeyword),
        TSKeywordKind::Boolean => make_public!(TSBooleanKeyword),
        TSKeywordKind::Any => make_public!(TSAnyKeyword),
        TSKeywordKind::Void => make_public!(TSVoidKeyword),
        TSKeywordKind::Undefined => make_public!(TSUndefinedKeyword),
        TSKeywordKind::Null => make_public!(TSNullKeyword),
        TSKeywordKind::Never => make_public!(TSNeverKeyword),
        TSKeywordKind::Unknown => make_public!(TSUnknownKeyword),
        TSKeywordKind::Object => make_public!(TSObjectKeyword),
        TSKeywordKind::Symbol => make_public!(TSSymbolKeyword),
        TSKeywordKind::BigInt => make_public!(TSBigIntKeyword),
    }
}
