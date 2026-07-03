// Expression dispatcher: emit each `Expression` variant's wire JSON.

use super::super::super::internal;
use super::declarations::write_class_expression;
use super::functions::{
    write_arrow_function_expression, write_await_expression, write_call_expression,
    write_conditional_expression, write_function_expression, write_member_expression,
    write_new_expression, write_yield_expression,
};
use super::patterns::{
    write_assignment_pattern, write_object_pattern, write_property, write_rest_element,
    write_template_literal,
};
use super::types::{write_type, write_type_parameter_instantiation};
use super::{
    Ctx, JsonWriter, close_node, node_header, write_array, write_bare_node, write_identifier_parts,
    write_identifier_plain, write_literal, write_name, write_type_annotation_field,
    write_type_arguments_field,
};
use tsv_lang::Span;

/// Emission-time expression state.
///
/// `in_chain` marks whether this node emits inside an optional chain. The other
/// two are pre-computed decisions that adjust the `optional` field along a
/// call/member spine:
///
/// - `force_optional` — the acorn `?.<T>(...)` quirk: the callee node of an
///   optional call with type arguments is itself marked optional.
/// - `strip_optional` — the unparenthesized-decorator quirk: `optional` is
///   omitted along the call/member spine.
///
/// Both flags survive a `JsdocCast` unwrap (they act on the unwrapped inner
/// expression) and die on any arm other than call/member — including a
/// `ChainExpression` wrap, which they fall through without touching.
#[derive(Clone, Copy, Default)]
pub(super) struct ExprFlags {
    pub(super) in_chain: bool,
    pub(super) force_optional: bool,
    pub(super) strip_optional: bool,
}

/// Emit an expression's wire JSON in a fresh context (not inside a chain).
pub(super) fn write_expression(w: &mut JsonWriter, expr: &internal::Expression<'_>, ctx: &Ctx<'_>) {
    write_expression_inner(w, expr, ctx, ExprFlags::default());
}

/// Emit a JSON array of expressions (params, arguments, elements without
/// holes) — the writer's most common list shape.
pub(super) fn write_expressions<'a, 'arena: 'a>(
    w: &mut JsonWriter,
    items: impl IntoIterator<Item = &'a internal::Expression<'arena>>,
    ctx: &Ctx<'_>,
) {
    write_array(w, items, |w, e| write_expression(w, e, ctx));
}

/// Emit a JSON array of hole-carrying elements (`[a, , b]` — `None` is
/// `null`), shared by array expressions and array patterns.
fn write_expression_holes<'a, 'arena: 'a>(
    w: &mut JsonWriter,
    items: impl IntoIterator<Item = &'a Option<internal::Expression<'arena>>>,
    ctx: &Ctx<'_>,
) {
    write_array(w, items, |w, e| match e {
        Some(expr) => write_expression(w, expr, ctx),
        None => w.null(),
    });
}

/// Emit an expression's wire JSON, dispatching on its variant (chain flags
/// threaded via `ExprFlags`).
pub(super) fn write_expression_inner(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    ctx: &Ctx<'_>,
    flags: ExprFlags,
) {
    match expr {
        // JSDoc cast is internal-only: emit the inner expression (paren-free
        // public AST). `in_chain = false` — the cast's parens seal any chain;
        // force/strip pass through (they act on the converted inner).
        internal::Expression::JsdocCast(cast) => {
            write_expression_inner(
                w,
                cast.inner,
                ctx,
                ExprFlags {
                    in_chain: false,
                    ..flags
                },
            );
        }
        internal::Expression::Literal(lit) => write_literal(w, lit, ctx),
        internal::Expression::Identifier(id) => {
            write_identifier_parts(
                w,
                id.span,
                id.name,
                id.optional || flags.force_optional,
                id.type_annotation(),
                id.decorators(),
                ctx,
            );
        }
        internal::Expression::PrivateIdentifier(pid) => {
            node_header(w, "PrivateIdentifier", pid.span, ctx);
            w.raw(",\"name\":");
            // The interned name excludes the leading `#` (the public shape).
            write_name(w, pid.name, ctx);
            close_node(w, "PrivateIdentifier", pid.span, ctx);
        }
        internal::Expression::ObjectExpression(obj) => {
            node_header(w, "ObjectExpression", obj.span, ctx);
            w.raw(",\"properties\":");
            write_array(w, obj.properties, |w, p| write_object_property(w, p, ctx));
            close_node(w, "ObjectExpression", obj.span, ctx);
        }
        internal::Expression::ArrayExpression(arr) => {
            node_header(w, "ArrayExpression", arr.span, ctx);
            w.raw(",\"elements\":");
            write_expression_holes(w, arr.elements, ctx);
            close_node(w, "ArrayExpression", arr.span, ctx);
        }
        internal::Expression::UnaryExpression(unary) => {
            node_header(w, "UnaryExpression", unary.span, ctx);
            w.raw(",\"operator\":\"");
            w.raw(unary.operator.as_str());
            w.raw("\",\"prefix\":");
            w.bool(unary.prefix);
            w.raw(",\"argument\":");
            write_expression(w, unary.argument, ctx);
            close_node(w, "UnaryExpression", unary.span, ctx);
        }
        internal::Expression::UpdateExpression(update) => {
            node_header(w, "UpdateExpression", update.span, ctx);
            w.raw(",\"operator\":\"");
            w.raw(update.operator.as_str());
            w.raw("\",\"prefix\":");
            w.bool(update.prefix);
            w.raw(",\"argument\":");
            write_expression(w, update.argument, ctx);
            close_node(w, "UpdateExpression", update.span, ctx);
        }
        internal::Expression::BinaryExpression(binary) => {
            // LogicalExpression for `&&`/`||`/`??`, BinaryExpression otherwise.
            let node_type = match binary.operator {
                internal::BinaryOperator::AmpersandAmpersand
                | internal::BinaryOperator::PipePipe
                | internal::BinaryOperator::QuestionQuestion => "LogicalExpression",
                _ => "BinaryExpression",
            };
            node_header(w, node_type, binary.span, ctx);
            w.raw(",\"left\":");
            write_expression(w, binary.left, ctx);
            w.raw(",\"operator\":\"");
            w.raw(binary.operator.as_str());
            w.raw("\",\"right\":");
            write_expression(w, binary.right, ctx);
            close_node(w, node_type, binary.span, ctx);
        }
        internal::Expression::ArrowFunctionExpression(arrow) => {
            write_arrow_function_expression(w, arrow, ctx);
        }
        internal::Expression::FunctionExpression(func) => {
            write_function_expression(w, func, ctx);
        }
        internal::Expression::ClassExpression(class_expr) => {
            write_class_expression(w, class_expr, ctx);
        }
        internal::Expression::SpreadElement(spread) => {
            node_header(w, "SpreadElement", spread.span, ctx);
            w.raw(",\"argument\":");
            write_expression(w, spread.argument, ctx);
            close_node(w, "SpreadElement", spread.span, ctx);
        }
        internal::Expression::CallExpression(call) => {
            let needs_chain = !flags.in_chain && expr.has_optional_in_chain();
            let callee_in_chain = child_in_chain(
                call.span.start,
                call.callee.span().start,
                needs_chain,
                flags.in_chain,
            );
            let (force, strip) = flags_after_wrap(flags, needs_chain);
            maybe_wrap_chain(w, needs_chain, call.span, ctx, |w| {
                write_call_expression(w, call, ctx, callee_in_chain, force, strip);
            });
        }
        internal::Expression::NewExpression(new_expr) => {
            write_new_expression(w, new_expr, ctx);
        }
        internal::Expression::MemberExpression(member) => {
            let needs_chain = !flags.in_chain && expr.has_optional_in_chain();
            let object_in_chain = child_in_chain(
                member.span.start,
                member.object.span().start,
                needs_chain,
                flags.in_chain,
            );
            let (force, strip) = flags_after_wrap(flags, needs_chain);
            maybe_wrap_chain(w, needs_chain, member.span, ctx, |w| {
                write_member_expression(w, member, ctx, object_in_chain, force, strip);
            });
        }
        internal::Expression::ConditionalExpression(cond) => {
            write_conditional_expression(w, cond, ctx);
        }
        internal::Expression::TemplateLiteral(template) => {
            write_template_literal(w, template, ctx);
        }
        internal::Expression::TaggedTemplateExpression(tagged) => {
            node_header(w, "TaggedTemplateExpression", tagged.span, ctx);
            w.raw(",\"tag\":");
            write_expression(w, tagged.tag, ctx);
            w.raw(",\"quasi\":");
            write_template_literal(w, &tagged.quasi, ctx);
            write_type_arguments_field(w, tagged.type_arguments.as_ref(), ctx);
            close_node(w, "TaggedTemplateExpression", tagged.span, ctx);
        }
        internal::Expression::AwaitExpression(await_expr) => {
            write_await_expression(w, await_expr, ctx);
        }
        internal::Expression::YieldExpression(yield_expr) => {
            write_yield_expression(w, yield_expr, ctx);
        }
        internal::Expression::SequenceExpression(seq) => {
            node_header(w, "SequenceExpression", seq.span, ctx);
            w.raw(",\"expressions\":");
            write_expressions(w, seq.expressions, ctx);
            close_node(w, "SequenceExpression", seq.span, ctx);
        }
        internal::Expression::RegexLiteral(regex) => {
            // Regex uses "Literal" type in acorn/Svelte AST; `value` is `{}`.
            node_header(w, "Literal", regex.span, ctx);
            w.raw(",\"value\":{},\"raw\":");
            w.string(regex.span.extract(ctx.source));
            w.raw(",\"regex\":{\"pattern\":");
            w.string(regex.pattern(ctx.source));
            w.raw(",\"flags\":");
            w.string(regex.flags(ctx.source));
            // Close the inner `regex` object, then the `Literal` node.
            w.raw("}");
            close_node(w, "Literal", regex.span, ctx);
        }
        internal::Expression::ThisExpression(t) => {
            write_bare_node(w, "ThisExpression", t.span, ctx);
        }
        internal::Expression::Super(s) => {
            write_bare_node(w, "Super", s.span, ctx);
        }
        internal::Expression::AssignmentExpression(assign) => {
            // acorn drops a type assertion from a *simple* `=` left.
            let left = if matches!(assign.operator, internal::AssignmentOperator::Assign) {
                assign.left.skip_type_assertions()
            } else {
                assign.left
            };
            node_header(w, "AssignmentExpression", assign.span, ctx);
            w.raw(",\"operator\":\"");
            w.raw(assign.operator.as_str());
            w.raw("\",\"left\":");
            write_expression(w, left, ctx);
            w.raw(",\"right\":");
            write_expression(w, assign.right, ctx);
            close_node(w, "AssignmentExpression", assign.span, ctx);
        }
        internal::Expression::ObjectPattern(obj) => {
            write_object_pattern(w, obj, ctx);
        }
        internal::Expression::ArrayPattern(arr) => {
            node_header(w, "ArrayPattern", arr.span, ctx);
            w.raw(",\"elements\":");
            write_expression_holes(w, arr.elements, ctx);
            if arr.optional {
                w.raw(",\"optional\":true");
            }
            write_type_annotation_field(w, arr.type_annotation.as_ref(), ctx);
            close_node(w, "ArrayPattern", arr.span, ctx);
        }
        internal::Expression::AssignmentPattern(pattern) => {
            write_assignment_pattern(w, pattern, ctx, pattern.span);
        }
        internal::Expression::RestElement(rest) => {
            write_rest_element(w, rest, ctx);
        }
        internal::Expression::TSTypeAssertion(type_assert) => {
            node_header(w, "TSTypeAssertion", type_assert.span, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type(w, type_assert.type_annotation, ctx);
            w.raw(",\"expression\":");
            write_expression(w, type_assert.expression, ctx);
            close_node(w, "TSTypeAssertion", type_assert.span, ctx);
        }
        internal::Expression::TSAsExpression(as_expr) => {
            node_header(w, "TSAsExpression", as_expr.span, ctx);
            w.raw(",\"expression\":");
            write_expression(w, as_expr.expression, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type(w, as_expr.type_annotation, ctx);
            close_node(w, "TSAsExpression", as_expr.span, ctx);
        }
        internal::Expression::TSSatisfiesExpression(sat_expr) => {
            node_header(w, "TSSatisfiesExpression", sat_expr.span, ctx);
            w.raw(",\"expression\":");
            write_expression(w, sat_expr.expression, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type(w, sat_expr.type_annotation, ctx);
            close_node(w, "TSSatisfiesExpression", sat_expr.span, ctx);
        }
        internal::Expression::TSInstantiationExpression(inst_expr) => {
            node_header(w, "TSInstantiationExpression", inst_expr.span, ctx);
            w.raw(",\"expression\":");
            write_expression(w, inst_expr.expression, ctx);
            w.raw(",\"typeArguments\":");
            write_type_parameter_instantiation(w, &inst_expr.type_arguments, ctx);
            close_node(w, "TSInstantiationExpression", inst_expr.span, ctx);
        }
        internal::Expression::TSNonNullExpression(non_null_expr) => {
            let needs_chain = !flags.in_chain && expr.has_optional_in_chain();
            // A parenthesized inner chain seals at the parens, so the
            // paren-aware child check applies like it does for calls/members.
            let inner_in_chain = child_in_chain(
                non_null_expr.span.start,
                non_null_expr.expression.span().start,
                needs_chain,
                flags.in_chain,
            );
            maybe_wrap_chain(w, needs_chain, non_null_expr.span, ctx, |w| {
                node_header(w, "TSNonNullExpression", non_null_expr.span, ctx);
                w.raw(",\"expression\":");
                // force/strip die here: TSNonNullExpression is not a
                // call/member spine node.
                write_expression_inner(
                    w,
                    non_null_expr.expression,
                    ctx,
                    ExprFlags {
                        in_chain: inner_in_chain,
                        ..ExprFlags::default()
                    },
                );
                close_node(w, "TSNonNullExpression", non_null_expr.span, ctx);
            });
        }
        internal::Expression::ImportExpression(import_expr) => {
            node_header(w, "ImportExpression", import_expr.span, ctx);
            w.raw(",\"source\":");
            write_expression(w, import_expr.source, ctx);
            if let Some(phase) = import_expr.phase.as_str() {
                w.raw(",\"phase\":");
                w.token(phase);
            }
            // `arguments` is skip-if-empty: only the import-attributes options
            // object populates it (a single-element array).
            if let Some(opts) = &import_expr.options {
                w.raw(",\"arguments\":[");
                write_expression(w, opts, ctx);
                w.raw("]");
            }
            // Svelte non-`lang="ts"` script quirk: vanilla acorn appends
            // `options: null`; acorn-typescript omits it. Appended last (after
            // `arguments`) so field order matches vanilla acorn.
            if ctx.import_options_null {
                w.raw(",\"options\":null");
            }
            close_node(w, "ImportExpression", import_expr.span, ctx);
        }
        internal::Expression::MetaProperty(meta) => {
            node_header(w, "MetaProperty", meta.span, ctx);
            w.raw(",\"meta\":");
            write_identifier_plain(w, &meta.meta, ctx);
            w.raw(",\"property\":");
            write_identifier_plain(w, &meta.property, ctx);
            close_node(w, "MetaProperty", meta.span, ctx);
        }
        internal::Expression::TSParameterProperty(param_prop) => {
            // acorn quirk: when the parameter is an AssignmentPattern whose left
            // has no type annotation, its span/loc expand to include the
            // accessibility modifier keyword (the whole parameter property).
            let param = param_prop.parameter.unwrap_jsdoc_casts();
            let override_ap = if let internal::Expression::AssignmentPattern(ap) = param {
                let has_type_ann = match ap.left.unwrap_jsdoc_casts() {
                    internal::Expression::Identifier(id) => id.type_annotation().is_some(),
                    internal::Expression::ArrayPattern(arr) => arr.type_annotation.is_some(),
                    internal::Expression::ObjectPattern(obj) => obj.type_annotation.is_some(),
                    _ => false,
                };
                (!has_type_ann).then_some(ap)
            } else {
                None
            };
            node_header(w, "TSParameterProperty", param_prop.span, ctx);
            if let Some(acc) = param_prop.accessibility {
                w.raw(",\"accessibility\":");
                w.token(acc.as_str());
            }
            if param_prop.readonly {
                w.raw(",\"readonly\":true");
            }
            if param_prop.r#override {
                w.raw(",\"override\":true");
            }
            w.raw(",\"parameter\":");
            if let Some(ap) = override_ap {
                write_assignment_pattern(w, ap, ctx, param_prop.span);
            } else {
                write_expression(w, param_prop.parameter, ctx);
            }
            close_node(w, "TSParameterProperty", param_prop.span, ctx);
        }
    }
}

/// Whether a member's object / a call's callee emits as part of the current
/// optional chain: a parenthesized child seals its own chain (the span gap over
/// the stripped `(` is the signal).
fn child_in_chain(parent_start: u32, child_start: u32, needs_chain: bool, in_chain: bool) -> bool {
    let parenthesized = parent_start < child_start;
    !parenthesized && (needs_chain || in_chain)
}

/// Emit `inner` wrapped in a `ChainExpression` node when `needs_chain`, bare
/// otherwise.
fn maybe_wrap_chain(
    w: &mut JsonWriter,
    needs_chain: bool,
    span: Span,
    ctx: &Ctx<'_>,
    inner: impl FnOnce(&mut JsonWriter),
) {
    if needs_chain {
        node_header(w, "ChainExpression", span, ctx);
        w.raw(",\"expression\":");
        inner(w);
        close_node(w, "ChainExpression", span, ctx);
    } else {
        inner(w);
    }
}

/// The force/strip flags a call/member node keeps for itself: a
/// `ChainExpression` wrap kills both (the flags don't reach past the wrapper).
fn flags_after_wrap(flags: ExprFlags, needs_chain: bool) -> (bool, bool) {
    if needs_chain {
        (false, false)
    } else {
        (flags.force_optional, flags.strip_optional)
    }
}

/// Emit an object-literal property (a `Property` or a `SpreadElement`).
fn write_object_property(w: &mut JsonWriter, prop: &internal::ObjectProperty<'_>, ctx: &Ctx<'_>) {
    match prop {
        internal::ObjectProperty::Property(p) => write_property(w, p, ctx),
        internal::ObjectProperty::SpreadElement(s) => {
            node_header(w, "SpreadElement", s.span, ctx);
            w.raw(",\"argument\":");
            write_expression(w, s.argument, ctx);
            close_node(w, "SpreadElement", s.span, ctx);
        }
    }
}
