// Core expression conversions and dispatcher

use super::super::{internal, public};
use super::{
    convert_arrow_function_expression, convert_await_expression, convert_call_expression,
    convert_class_expression, convert_conditional_expression, convert_function_expression,
    convert_member_expression, convert_new_expression, convert_object_pattern, convert_property,
    convert_template_literal, convert_type, convert_type_annotation,
    convert_type_parameter_instantiation, convert_yield_expression, create_location,
};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};

/// Main expression conversion dispatcher
pub fn convert_expression(
    expr: &internal::Expression,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Expression {
    match expr {
        internal::Expression::Literal(lit) => convert_literal_expression(lit, source, loc, offset),
        internal::Expression::Identifier(id) => {
            public::Expression::Identifier(public::Identifier {
                node_type: "Identifier".to_string(),
                start: id.span.start,
                end: id.span.end,
                loc: create_location(id.span, loc, offset),
                name: interner.resolve_infallible(id.name).to_string(),
                optional: id.optional,
                type_annotation: id
                    .type_annotation
                    .as_ref()
                    .map(|ta| convert_type_annotation(ta, source, loc, interner, offset)),
                decorators: id
                    .decorators
                    .as_ref()
                    .map(|decs| {
                        decs.iter()
                            .map(|d| super::convert_decorator(d, source, loc, interner, offset))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        }
        internal::Expression::PrivateIdentifier(pid) => {
            public::Expression::PrivateIdentifier(public::PrivateIdentifier {
                node_type: "PrivateIdentifier".to_string(),
                start: pid.span.start,
                end: pid.span.end,
                loc: create_location(pid.span, loc, offset),
                name: interner.resolve_infallible(pid.name).to_string(),
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
            public::Expression::ArrowFunctionExpression(convert_arrow_function_expression(
                arrow, source, loc, interner, offset,
            ))
        }
        internal::Expression::FunctionExpression(func) => public::Expression::FunctionExpression(
            convert_function_expression(func, source, loc, interner, offset),
        ),
        internal::Expression::ClassExpression(class_expr) => public::Expression::ClassExpression(
            convert_class_expression(class_expr, source, loc, interner, offset),
        ),
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
        internal::Expression::CallExpression(call) => public::Expression::CallExpression(
            convert_call_expression(call, source, loc, interner, offset),
        ),
        internal::Expression::NewExpression(new_expr) => public::Expression::NewExpression(
            convert_new_expression(new_expr, source, loc, interner, offset),
        ),
        internal::Expression::MemberExpression(member) => public::Expression::MemberExpression(
            convert_member_expression(member, source, loc, interner, offset),
        ),
        internal::Expression::ConditionalExpression(cond) => {
            public::Expression::ConditionalExpression(convert_conditional_expression(
                cond, source, loc, interner, offset,
            ))
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
        internal::Expression::AwaitExpression(await_expr) => public::Expression::AwaitExpression(
            convert_await_expression(await_expr, source, loc, interner, offset),
        ),
        internal::Expression::YieldExpression(yield_expr) => public::Expression::YieldExpression(
            convert_yield_expression(yield_expr, source, loc, interner, offset),
        ),
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
        internal::Expression::ObjectPattern(obj) => public::Expression::ObjectPattern(
            convert_object_pattern(obj, source, loc, interner, offset),
        ),
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
                type_annotation: arr
                    .type_annotation
                    .as_ref()
                    .map(|ta| convert_type_annotation(ta, source, loc, interner, offset)),
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
        internal::Expression::TSTypeAssertion(type_assert) => {
            public::Expression::TSTypeAssertion(public::TSTypeAssertion {
                node_type: "TSTypeAssertion".to_string(),
                start: type_assert.span.start,
                end: type_assert.span.end,
                loc: create_location(type_assert.span, loc, offset),
                type_annotation: Box::new(convert_type(
                    &type_assert.type_annotation,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                expression: Box::new(convert_expression(
                    &type_assert.expression,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::TSAsExpression(as_expr) => {
            public::Expression::TSAsExpression(public::TSAsExpression {
                node_type: "TSAsExpression".to_string(),
                start: as_expr.span.start,
                end: as_expr.span.end,
                loc: create_location(as_expr.span, loc, offset),
                expression: Box::new(convert_expression(
                    &as_expr.expression,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                type_annotation: Box::new(convert_type(
                    &as_expr.type_annotation,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::TSSatisfiesExpression(sat_expr) => {
            public::Expression::TSSatisfiesExpression(public::TSSatisfiesExpression {
                node_type: "TSSatisfiesExpression".to_string(),
                start: sat_expr.span.start,
                end: sat_expr.span.end,
                loc: create_location(sat_expr.span, loc, offset),
                expression: Box::new(convert_expression(
                    &sat_expr.expression,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                type_annotation: Box::new(convert_type(
                    &sat_expr.type_annotation,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::TSInstantiationExpression(inst_expr) => {
            public::Expression::TSInstantiationExpression(public::TSInstantiationExpression {
                node_type: "TSInstantiationExpression".to_string(),
                start: inst_expr.span.start,
                end: inst_expr.span.end,
                loc: create_location(inst_expr.span, loc, offset),
                expression: Box::new(convert_expression(
                    &inst_expr.expression,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                type_arguments: convert_type_parameter_instantiation(
                    &inst_expr.type_arguments,
                    source,
                    loc,
                    interner,
                    offset,
                ),
            })
        }
        internal::Expression::TSNonNullExpression(non_null_expr) => {
            public::Expression::TSNonNullExpression(public::TSNonNullExpression {
                node_type: "TSNonNullExpression".to_string(),
                start: non_null_expr.span.start,
                end: non_null_expr.span.end,
                loc: create_location(non_null_expr.span, loc, offset),
                expression: Box::new(convert_expression(
                    &non_null_expr.expression,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::ImportExpression(import_expr) => {
            public::Expression::ImportExpression(public::ImportExpression {
                node_type: "ImportExpression".to_string(),
                start: import_expr.span.start,
                end: import_expr.span.end,
                loc: create_location(import_expr.span, loc, offset),
                source: Box::new(convert_expression(
                    &import_expr.source,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                options: import_expr
                    .options
                    .as_ref()
                    .map(|opts| Box::new(convert_expression(opts, source, loc, interner, offset))),
            })
        }
        internal::Expression::MetaProperty(meta) => {
            public::Expression::MetaProperty(public::MetaProperty {
                node_type: "MetaProperty".to_string(),
                start: meta.span.start,
                end: meta.span.end,
                loc: create_location(meta.span, loc, offset),
                meta: public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: meta.meta.span.start,
                    end: meta.meta.span.end,
                    loc: create_location(meta.meta.span, loc, offset),
                    name: interner.resolve_infallible(meta.meta.name).to_string(),
                    optional: false,
                    type_annotation: None,
                    decorators: Vec::new(),
                },
                property: public::Identifier {
                    node_type: "Identifier".to_string(),
                    start: meta.property.span.start,
                    end: meta.property.span.end,
                    loc: create_location(meta.property.span, loc, offset),
                    name: interner.resolve_infallible(meta.property.name).to_string(),
                    optional: false,
                    type_annotation: None,
                    decorators: Vec::new(),
                },
            })
        }
        internal::Expression::TSParameterProperty(param_prop) => {
            public::Expression::TSParameterProperty(public::TSParameterProperty {
                node_type: "TSParameterProperty".to_string(),
                start: param_prop.span.start,
                end: param_prop.span.end,
                loc: create_location(param_prop.span, loc, offset),
                accessibility: param_prop.accessibility.map(|a| a.as_str().to_string()),
                readonly: param_prop.readonly,
                parameter: Box::new(convert_expression(
                    &param_prop.parameter,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
    }
}

fn convert_literal_expression(
    lit: &internal::Literal,
    source: &str,
    loc: &LocationTracker,
    offset: usize,
) -> public::Expression {
    let (value, bigint) = match &lit.value {
        internal::LiteralValue::Number(n) => (
            serde_json::Value::Number(
                serde_json::Number::from_f64(*n).unwrap_or_else(|| serde_json::Number::from(0)),
            ),
            None,
        ),
        internal::LiteralValue::String { content, .. } => {
            (serde_json::Value::String(content.clone()), None)
        }
        internal::LiteralValue::BigInt(val) => {
            // BigInt: value is the string, bigint field stores the same value
            (serde_json::Value::String(val.clone()), Some(val.clone()))
        }
        internal::LiteralValue::Boolean(b) => (serde_json::Value::Bool(*b), None),
        internal::LiteralValue::Null => (serde_json::Value::Null, None),
        // undefined is represented as a special identifier in most ASTs
        // but as a literal in ours - serialize as null for JSON compatibility
        internal::LiteralValue::Undefined => (serde_json::Value::Null, None),
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
        bigint,
    })
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
