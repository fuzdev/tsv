// Core expression conversions and dispatcher

use super::super::{internal, public};
use super::{
    convert_arrow_function_expression, convert_await_expression, convert_call_expression,
    convert_class_expression, convert_conditional_expression, convert_function_expression,
    convert_member_expression, convert_new_expression, convert_object_pattern, convert_property,
    convert_template_literal, convert_type, convert_type_annotation,
    convert_type_parameter_instantiation, convert_yield_expression, create_location,
};
use std::borrow::Cow;
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationTracker, Span};

/// Main expression conversion dispatcher
pub fn convert_expression<'src>(
    expr: &internal::Expression<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Expression<'src> {
    convert_expression_inner(expr, source, loc, interner, offset, false)
}

/// Inner dispatcher with chain-awareness to prevent double-wrapping ChainExpression.
///
/// When `in_chain` is false and a MemberExpression/CallExpression/TSNonNullExpression
/// contains optional chaining (`?.`), it gets wrapped in ChainExpression. When `in_chain`
/// is true (we're already inside a chain), no wrapping occurs.
pub(in crate::ast::convert) fn convert_expression_inner<'src>(
    expr: &internal::Expression<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
    in_chain: bool,
) -> public::Expression<'src> {
    match expr {
        // A JSDoc cast is internal-only: unwrap to the inner expression so the
        // public AST stays paren-free (matching acorn/Svelte, which carry no
        // `ParenthesizedExpression`). `in_chain = false` because the cast's parens
        // seal any optional chain — the inner is a fresh chain root.
        internal::Expression::JsdocCast(cast) => {
            convert_expression_inner(cast.inner, source, loc, interner, offset, false)
        }
        internal::Expression::Literal(lit) => {
            public::Expression::Literal(super::convert_literal(lit, source, loc, offset))
        }
        internal::Expression::Identifier(id) => {
            public::Expression::Identifier(public::Identifier {
                node_type: "Identifier",
                start: id.span.start,
                end: id.span.end,
                loc: create_location(id.span, loc, offset),
                name: public::name_cow(id.span, source, id.name, interner),
                optional: id.optional,
                type_annotation: id
                    .type_annotation()
                    .map(|ta| convert_type_annotation(ta, source, loc, interner, offset)),
                decorators: id
                    .decorators()
                    .map(|decs| {
                        decs.iter()
                            .map(|d| super::convert_decorator(d, source, loc, interner, offset))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        }
        internal::Expression::PrivateIdentifier(pid) => {
            // The span includes the leading `#`, but the public `name` excludes
            // it — so borrow/resolve against the `#`-stripped span.
            let name_span = Span::new(pid.span.start + 1, pid.span.end);
            public::Expression::PrivateIdentifier(public::PrivateIdentifier {
                node_type: "PrivateIdentifier",
                start: pid.span.start,
                end: pid.span.end,
                loc: create_location(pid.span, loc, offset),
                name: public::name_cow(name_span, source, pid.name, interner),
            })
        }
        internal::Expression::ObjectExpression(obj) => {
            public::Expression::ObjectExpression(public::ObjectExpression {
                node_type: "ObjectExpression",
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
                node_type: "ArrayExpression",
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
                node_type: "UnaryExpression",
                start: unary.span.start,
                end: unary.span.end,
                loc: create_location(unary.span, loc, offset),
                operator: unary.operator.as_str(),
                prefix: unary.prefix,
                argument: Box::new(convert_expression(
                    unary.argument,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::UpdateExpression(update) => {
            public::Expression::UpdateExpression(public::UpdateExpression {
                node_type: "UpdateExpression",
                start: update.span.start,
                end: update.span.end,
                loc: create_location(update.span, loc, offset),
                operator: update.operator.as_str(),
                prefix: update.prefix,
                argument: Box::new(convert_expression(
                    update.argument,
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
                node_type,
                start: binary.span.start,
                end: binary.span.end,
                loc: create_location(binary.span, loc, offset),
                left: Box::new(convert_expression(
                    binary.left,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                operator: binary.operator.as_str(),
                right: Box::new(convert_expression(
                    binary.right,
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
                node_type: "SpreadElement",
                start: spread.span.start,
                end: spread.span.end,
                loc: create_location(spread.span, loc, offset),
                argument: Box::new(convert_expression(
                    spread.argument,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::CallExpression(call) => {
            let needs_chain = !in_chain && expr.has_optional_in_chain();
            let callee_in_chain = child_in_chain(
                call.span.start,
                call.callee.span().start,
                needs_chain,
                in_chain,
            );
            let converted =
                convert_call_expression(call, source, loc, interner, offset, callee_in_chain);
            maybe_wrap_chain(
                public::Expression::CallExpression(converted),
                call.span,
                loc,
                offset,
                needs_chain,
            )
        }
        internal::Expression::NewExpression(new_expr) => public::Expression::NewExpression(
            convert_new_expression(new_expr, source, loc, interner, offset),
        ),
        internal::Expression::MemberExpression(member) => {
            let needs_chain = !in_chain && expr.has_optional_in_chain();
            let object_in_chain = child_in_chain(
                member.span.start,
                member.object.span().start,
                needs_chain,
                in_chain,
            );
            let converted =
                convert_member_expression(member, source, loc, interner, offset, object_in_chain);
            maybe_wrap_chain(
                public::Expression::MemberExpression(converted),
                member.span,
                loc,
                offset,
                needs_chain,
            )
        }
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
                node_type: "TaggedTemplateExpression",
                start: tagged.span.start,
                end: tagged.span.end,
                loc: create_location(tagged.span, loc, offset),
                tag: Box::new(convert_expression(
                    tagged.tag, source, loc, interner, offset,
                )),
                quasi: convert_template_literal(&tagged.quasi, source, loc, interner, offset),
                type_arguments: tagged.type_arguments.as_ref().map(|ta| {
                    convert_type_parameter_instantiation(ta, source, loc, interner, offset)
                }),
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
                node_type: "SequenceExpression",
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
            // Reconstruct raw/pattern/flags as borrowed source slices (verbatim).
            public::Expression::RegexLiteral(public::RegexLiteral {
                node_type: "Literal", // Regex uses "Literal" type in acorn/Svelte AST
                start: regex.span.start,
                end: regex.span.end,
                loc: create_location(regex.span, loc, offset),
                value: serde_json::Value::Object(serde_json::Map::new()), // Empty object {}
                raw: Cow::Borrowed(regex.span.extract(source)),
                regex: public::RegexValue {
                    pattern: Cow::Borrowed(regex.pattern(source)),
                    flags: Cow::Borrowed(regex.flags(source)),
                },
            })
        }
        internal::Expression::ThisExpression(t) => {
            public::Expression::ThisExpression(public::ThisExpression {
                node_type: "ThisExpression",
                start: t.span.start,
                end: t.span.end,
                loc: create_location(t.span, loc, offset),
            })
        }
        internal::Expression::Super(s) => public::Expression::Super(public::Super {
            node_type: "Super",
            start: s.span.start,
            end: s.span.end,
            loc: create_location(s.span, loc, offset),
        }),
        internal::Expression::AssignmentExpression(assign) => {
            public::Expression::AssignmentExpression(public::AssignmentExpression {
                node_type: "AssignmentExpression",
                start: assign.span.start,
                end: assign.span.end,
                loc: create_location(assign.span, loc, offset),
                operator: assign.operator.as_str(),
                left: Box::new(convert_expression(
                    assign.left,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                right: Box::new(convert_expression(
                    assign.right,
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
                node_type: "ArrayPattern",
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
            let base_loc = create_location(pattern.span, loc, offset);
            public::Expression::AssignmentPattern(public::AssignmentPattern {
                node_type: "AssignmentPattern",
                start: pattern.span.start,
                end: pattern.span.end,
                loc: base_loc,
                left: Box::new(convert_expression(
                    pattern.left,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                right: Box::new(convert_expression(
                    pattern.right,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::RestElement(rest) => public::Expression::RestElement(
            super::convert_rest_element(rest, source, loc, interner, offset),
        ),
        internal::Expression::TSTypeAssertion(type_assert) => {
            public::Expression::TSTypeAssertion(public::TSTypeAssertion {
                node_type: "TSTypeAssertion",
                start: type_assert.span.start,
                end: type_assert.span.end,
                loc: create_location(type_assert.span, loc, offset),
                type_annotation: Box::new(convert_type(
                    type_assert.type_annotation,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                expression: Box::new(convert_expression(
                    type_assert.expression,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::TSAsExpression(as_expr) => {
            public::Expression::TSAsExpression(public::TSAsExpression {
                node_type: "TSAsExpression",
                start: as_expr.span.start,
                end: as_expr.span.end,
                loc: create_location(as_expr.span, loc, offset),
                expression: Box::new(convert_expression(
                    as_expr.expression,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                type_annotation: Box::new(convert_type(
                    as_expr.type_annotation,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::TSSatisfiesExpression(sat_expr) => {
            public::Expression::TSSatisfiesExpression(public::TSSatisfiesExpression {
                node_type: "TSSatisfiesExpression",
                start: sat_expr.span.start,
                end: sat_expr.span.end,
                loc: create_location(sat_expr.span, loc, offset),
                expression: Box::new(convert_expression(
                    sat_expr.expression,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                type_annotation: Box::new(convert_type(
                    sat_expr.type_annotation,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::Expression::TSInstantiationExpression(inst_expr) => {
            public::Expression::TSInstantiationExpression(public::TSInstantiationExpression {
                node_type: "TSInstantiationExpression",
                start: inst_expr.span.start,
                end: inst_expr.span.end,
                loc: create_location(inst_expr.span, loc, offset),
                expression: Box::new(convert_expression(
                    inst_expr.expression,
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
            let needs_chain = !in_chain && expr.has_optional_in_chain();
            // A parenthesized inner chain seals at the parens (`(a?.b())!?.()` —
            // the inner chain wraps itself inside the NonNull), so the paren-aware
            // child check applies here like it does for calls and members
            let inner_in_chain = child_in_chain(
                non_null_expr.span.start,
                non_null_expr.expression.span().start,
                needs_chain,
                in_chain,
            );
            let converted = public::TSNonNullExpression {
                node_type: "TSNonNullExpression",
                start: non_null_expr.span.start,
                end: non_null_expr.span.end,
                loc: create_location(non_null_expr.span, loc, offset),
                expression: Box::new(convert_expression_inner(
                    non_null_expr.expression,
                    source,
                    loc,
                    interner,
                    offset,
                    inner_in_chain,
                )),
            };
            maybe_wrap_chain(
                public::Expression::TSNonNullExpression(converted),
                non_null_expr.span,
                loc,
                offset,
                needs_chain,
            )
        }
        internal::Expression::ImportExpression(import_expr) => {
            public::Expression::ImportExpression(public::ImportExpression {
                node_type: "ImportExpression",
                start: import_expr.span.start,
                end: import_expr.span.end,
                loc: create_location(import_expr.span, loc, offset),
                source: Box::new(convert_expression(
                    import_expr.source,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                arguments: import_expr
                    .options
                    .as_ref()
                    .map(|opts| vec![convert_expression(opts, source, loc, interner, offset)])
                    .unwrap_or_default(),
            })
        }
        internal::Expression::MetaProperty(meta) => {
            public::Expression::MetaProperty(public::MetaProperty {
                node_type: "MetaProperty",
                start: meta.span.start,
                end: meta.span.end,
                loc: create_location(meta.span, loc, offset),
                meta: super::convert_identifier(&meta.meta, source, loc, interner, offset),
                property: super::convert_identifier(&meta.property, source, loc, interner, offset),
            })
        }
        internal::Expression::TSParameterProperty(param_prop) => {
            let mut parameter =
                convert_expression(param_prop.parameter, source, loc, interner, offset);
            // acorn quirk: when parameter is AssignmentPattern without type annotation,
            // the span/loc includes the accessibility modifier keyword
            if let public::Expression::AssignmentPattern(ref mut ap) = parameter {
                let has_type_ann = match ap.left.as_ref() {
                    public::Expression::Identifier(id) => id.type_annotation.is_some(),
                    public::Expression::ArrayPattern(arr) => arr.type_annotation.is_some(),
                    public::Expression::ObjectPattern(obj) => obj.type_annotation.is_some(),
                    _ => false,
                };
                if !has_type_ann {
                    ap.start = param_prop.span.start;
                    ap.end = param_prop.span.end;
                    ap.loc = create_location(param_prop.span, loc, offset);
                }
            }
            public::Expression::TSParameterProperty(public::TSParameterProperty {
                node_type: "TSParameterProperty",
                start: param_prop.span.start,
                end: param_prop.span.end,
                loc: create_location(param_prop.span, loc, offset),
                accessibility: param_prop
                    .accessibility
                    .map(internal::Accessibility::as_str),
                readonly: param_prop.readonly,
                r#override: param_prop.r#override,
                parameter: Box::new(parameter),
            })
        }
    }
}

/// Whether a member's object / a call's callee should convert as part of the
/// current optional chain.
///
/// A parenthesized object/callee seals its own chain (`(a?.b).c`, `(a?.b)()`):
/// the span gap — parent starts before child, covering the stripped `(` — means
/// the inner chain must convert as a fresh `in_chain = false` context so it gets
/// its own `ChainExpression` (matching acorn), even when this node is itself
/// inside an outer chain. Without parens the child stays in the enclosing chain.
fn child_in_chain(parent_start: u32, child_start: u32, needs_chain: bool, in_chain: bool) -> bool {
    let parenthesized = parent_start < child_start;
    !parenthesized && (needs_chain || in_chain)
}

/// Conditionally wrap an expression in ChainExpression.
/// Returns the expression as-is if `needs_chain` is false.
fn maybe_wrap_chain<'src>(
    inner: public::Expression<'src>,
    span: Span,
    loc: &LocationTracker,
    offset: usize,
    needs_chain: bool,
) -> public::Expression<'src> {
    if needs_chain {
        public::Expression::ChainExpression(public::ChainExpression {
            node_type: "ChainExpression",
            start: span.start,
            end: span.end,
            loc: create_location(span, loc, offset),
            expression: Box::new(inner),
        })
    } else {
        inner
    }
}

fn convert_object_property<'src>(
    prop: &internal::ObjectProperty<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ObjectProperty<'src> {
    match prop {
        internal::ObjectProperty::Property(p) => {
            public::ObjectProperty::Property(convert_property(p, source, loc, interner, offset))
        }
        internal::ObjectProperty::SpreadElement(s) => {
            public::ObjectProperty::SpreadElement(public::SpreadElement {
                node_type: "SpreadElement",
                start: s.span.start,
                end: s.span.end,
                loc: create_location(s.span, loc, offset),
                argument: Box::new(convert_expression(
                    s.argument, source, loc, interner, offset,
                )),
            })
        }
    }
}
