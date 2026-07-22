// Object, array, template, and pattern writers.

use super::super::super::internal;
use super::declarations::write_decorators_field;
use super::expressions::{write_expression, write_expressions};
use super::functions::write_function_expression;
use super::{
    Ctx, JsonWriter, close_node, node_header, node_header_wide_end, write_array,
    write_type_annotation_field,
};
use tsv_lang::Span;

/// The header of a destructuring pattern (`ObjectPattern` / `ArrayPattern`).
///
/// An annotation widens the wire `end` but **not** the `loc`
/// ([`node_header_wide_end`]): a signature parameter's span already covers its
/// annotation, so the widening is a no-op there, but a Svelte **block** binding
/// pattern's span stops at the bare pattern — and the oracle's own `end` and `loc`
/// disagree for exactly that reason (`read_pattern` patches `end` and leaves `loc`).
/// Shared so the two pattern writers can't drift.
pub(super) fn pattern_header(
    w: &mut JsonWriter,
    node_type: &'static str,
    span: Span,
    type_annotation: Option<&internal::TSTypeAnnotation<'_>>,
    ctx: &Ctx<'_>,
) {
    match type_annotation {
        Some(ta) => node_header_wide_end(w, node_type, span, ta.span.end, ctx),
        None => node_header(w, node_type, span, ctx),
    }
}

/// Emits a `TemplateLiteral` node. Field order: `expressions`, `quasis`.
pub(super) fn write_template_literal(
    w: &mut JsonWriter,
    template: &internal::TemplateLiteral<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TemplateLiteral", template.span, ctx);
    w.raw(",\"expressions\":");
    write_expressions(w, template.expressions, ctx);
    w.raw(",\"quasis\":");
    write_array(w, template.quasis, |w, q| write_template_element(w, q, ctx));
    close_node(w, "TemplateLiteral", template.span, ctx);
}

/// Emits a `TemplateElement` node: acorn excludes the delimiters from the
/// span (`+1` past the opening `` ` `` or `}`, `-1`/`-2` before the closing
/// `` ` `` or `${`). `cooked` is null for invalid escapes in tagged templates.
pub(super) fn write_template_element(
    w: &mut JsonWriter,
    element: &internal::TemplateElement<'_>,
    ctx: &Ctx<'_>,
) {
    let adjusted_start = element.span.start + 1;
    let adjusted_end = if element.tail {
        element.span.end - 1
    } else {
        element.span.end - 2
    };
    let adjusted_span = Span::new(adjusted_start, adjusted_end);
    node_header(w, "TemplateElement", adjusted_span, ctx);
    w.raw(",\"value\":{\"raw\":");
    w.string(element.raw(ctx.source));
    w.raw(",\"cooked\":");
    match element.cooked {
        internal::TemplateCooked::Verbatim => w.string(element.raw(ctx.source)),
        internal::TemplateCooked::Decoded(decoded) => w.string(decoded),
        internal::TemplateCooked::Invalid => w.null(),
    }
    w.raw("},\"tail\":");
    w.bool(element.tail);
    close_node(w, "TemplateElement", adjusted_span, ctx);
}

/// Emits an `ObjectPattern` node. Field order: `properties`, `optional`
/// (only when true), `typeAnnotation?`, `decorators?` (parameter position).
pub(super) fn write_object_pattern(
    w: &mut JsonWriter,
    obj: &internal::ObjectPattern<'_>,
    ctx: &Ctx<'_>,
) {
    pattern_header(
        w,
        "ObjectPattern",
        obj.span,
        obj.type_annotation.as_ref(),
        ctx,
    );
    w.raw(",\"properties\":");
    write_array(w, obj.properties, |w, p| match p {
        internal::ObjectPatternProperty::Property(p) => write_property(w, p, ctx),
        internal::ObjectPatternProperty::RestElement(r) => write_rest_element(w, r, ctx),
    });
    if obj.optional {
        w.raw(",\"optional\":true");
    }
    write_type_annotation_field(w, obj.type_annotation.as_ref(), ctx);
    write_decorators_field(w, obj.decorators, ctx);
    close_node(w, "ObjectPattern", obj.span, ctx);
}

/// Emits a `RestElement` node.
pub(super) fn write_rest_element(
    w: &mut JsonWriter,
    rest: &internal::RestElement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "RestElement", rest.span, ctx);
    w.raw(",\"argument\":");
    write_expression(w, rest.argument, ctx);
    // acorn carries a rest parameter's `?` on the rest element (never on
    // `argument`), between `argument` and `typeAnnotation`, only when present.
    if rest.optional {
        w.raw(",\"optional\":true");
    }
    write_type_annotation_field(w, rest.type_annotation.as_ref(), ctx);
    close_node(w, "RestElement", rest.span, ctx);
}

/// Emits a `Property` node. Field order: `method`, `shorthand`, `computed`,
/// `key`, then `value`/`kind` — acorn assigns `value` before `kind` in every
/// shape except two acorn-typescript paths that assign `kind` first: get/set
/// (`parseGetterSetter`) and generic object methods (`m<T>() {}`, the
/// `parsePropertyValue` override). Vanilla acorn (`ctx.vanilla_acorn`) assigns
/// `value` first everywhere.
pub(super) fn write_property(w: &mut JsonWriter, prop: &internal::Property<'_>, ctx: &Ctx<'_>) {
    node_header(w, "Property", prop.span, ctx);
    w.raw(",\"method\":");
    w.bool(prop.method);
    w.raw(",\"shorthand\":");
    w.bool(prop.shorthand);
    w.raw(",\"computed\":");
    w.bool(prop.computed);
    w.raw(",\"key\":");
    write_expression(w, &prop.key, ctx);
    let getset = !matches!(prop.kind, internal::PropertyKind::Init);
    let generic_method = prop.method
        && matches!(&prop.value, internal::Expression::FunctionExpression(f)
            if f.type_parameters.is_some());
    if (getset || generic_method) && !ctx.vanilla_acorn {
        w.raw(",\"kind\":\"");
        w.raw(prop.kind.as_str());
        w.raw("\",\"value\":");
        write_property_value(w, prop, ctx);
    } else {
        w.raw(",\"value\":");
        write_property_value(w, prop, ctx);
        w.raw(",\"kind\":\"");
        w.raw(prop.kind.as_str());
        w.raw("\"");
    }
    close_node(w, "Property", prop.span, ctx);
}

/// A method/get/set value goes through acorn's `parseMethod`, whose
/// `typeParameters` are grafted post-hoc (serialize after `body`).
fn write_property_value(w: &mut JsonWriter, prop: &internal::Property<'_>, ctx: &Ctx<'_>) {
    let method_value = prop.method || !matches!(prop.kind, internal::PropertyKind::Init);
    match (&prop.value, method_value) {
        (internal::Expression::FunctionExpression(f), true) => {
            write_function_expression(w, f, ctx, true);
        }
        _ => write_expression(w, &prop.value, ctx),
    }
}

/// Emits an `AssignmentPattern` node, with the span override the
/// `TSParameterProperty` quirk needs (`span` is normally `pattern.span`; the
/// quirk widens it to the whole parameter property).
pub(super) fn write_assignment_pattern(
    w: &mut JsonWriter,
    pattern: &internal::AssignmentPattern<'_>,
    ctx: &Ctx<'_>,
    span: Span,
) {
    node_header(w, "AssignmentPattern", span, ctx);
    w.raw(",\"left\":");
    // acorn's `=` conversion (`toAssignable`, return value used) peels a
    // type-assertion/paren wrapper off a default's target, so the wire `left`
    // is the bare target (`{ a: (b as T) = 1 }` → `Identifier` `b`); the cast
    // stays in the internal AST for the formatter — same unwrap as the simple
    // `=` left in `write_expression`'s `AssignmentExpression` arm.
    write_expression(w, pattern.left.skip_type_assertions(), ctx);
    w.raw(",\"right\":");
    write_expression(w, pattern.right, ctx);
    write_decorators_field(w, pattern.decorators, ctx);
    close_node(w, "AssignmentPattern", span, ctx);
}
