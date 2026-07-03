// Object, array, template, and pattern writers.

use super::super::super::internal;
use super::expressions::{write_expression, write_expressions};
use super::{Ctx, JsonWriter, close_node, node_header, write_array, write_type_annotation_field};
use tsv_lang::Span;

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
/// (only when true), `typeAnnotation?`.
pub(super) fn write_object_pattern(
    w: &mut JsonWriter,
    obj: &internal::ObjectPattern<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ObjectPattern", obj.span, ctx);
    w.raw(",\"properties\":");
    write_array(w, obj.properties, |w, p| match p {
        internal::ObjectPatternProperty::Property(p) => write_property(w, p, ctx),
        internal::ObjectPatternProperty::RestElement(r) => write_rest_element(w, r, ctx),
    });
    if obj.optional {
        w.raw(",\"optional\":true");
    }
    write_type_annotation_field(w, obj.type_annotation.as_ref(), ctx);
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
    write_type_annotation_field(w, rest.type_annotation.as_ref(), ctx);
    close_node(w, "RestElement", rest.span, ctx);
}

/// Emits a `Property` node. Field order: `method`, `shorthand`, `computed`,
/// `key`, `kind`, `value`.
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
    w.raw(",\"kind\":\"");
    w.raw(prop.kind.as_str());
    w.raw("\",\"value\":");
    write_expression(w, &prop.value, ctx);
    close_node(w, "Property", prop.span, ctx);
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
    write_expression(w, pattern.left, ctx);
    w.raw(",\"right\":");
    write_expression(w, pattern.right, ctx);
    close_node(w, "AssignmentPattern", span, ctx);
}
