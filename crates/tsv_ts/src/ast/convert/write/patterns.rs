// Object, array, template, and pattern writers.

use super::declarations::write_decorators_field;
use super::expressions::{write_expression, write_expressions};
use super::functions::write_function_expression;
use super::{
    Ctx, Ent, Sink, close_node, node_header, node_header_wide_end, write_array,
    write_type_annotation_field,
};
use crate::ast;
use tsv_lang::Span;

/// The header of a destructuring pattern (`ObjectPattern` / `ArrayPattern`).
///
/// An annotation widens the wire `end` ([`node_header_wide_end`]): a signature
/// parameter's span already covers its annotation, so the widening is a no-op there,
/// but a Svelte **block** binding pattern's span stops at the bare pattern, and Svelte's
/// `read_pattern` patches `end` to the annotation's. Shared so the two pattern writers
/// can't drift.
pub(super) fn pattern_header(
    w: &mut Sink<'_>,
    node_type: &'static str,
    entry: Ent,
    span: Span,
    type_annotation: Option<&ast::TSTypeAnnotation>,
    ctx: &Ctx<'_>,
) {
    match type_annotation {
        Some(ta) => node_header_wide_end(w, node_type, entry, span, ta.span.end, ctx),
        None => node_header!(w, @dyn node_type => entry, span, ctx),
    }
}

/// Emits a `TemplateLiteral` node. Field order: `expressions`, `quasis`.
pub(super) fn write_template_literal(
    w: &mut Sink<'_>,
    template: &ast::TemplateLiteral,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "TemplateLiteral", span, ctx);
    keyed!(w, "TemplateLiteral", ",\"expressions\":");
    write_expressions(
        w,
        ctx.view
            .expr_list(template.expressions)
            .iter()
            .map(|e| ctx.view.expr(*e)),
        ctx,
    );
    keyed!(w, "TemplateLiteral", ",\"quasis\":");
    write_array(w, ctx.view.quasis(template.quasis), |w, q| {
        write_template_element(w, q, ctx);
    });
    close_node(w, "TemplateLiteral", span, ctx);
}

/// Emits a `TemplateElement` node: acorn excludes the delimiters from the
/// span (`+1` past the opening `` ` `` or `}`, `-1`/`-2` before the closing
/// `` ` `` or `${`). `cooked` is null for invalid escapes in tagged templates.
pub(super) fn write_template_element(
    w: &mut Sink<'_>,
    element: &ast::TemplateElement,
    ctx: &Ctx<'_>,
) {
    let adjusted_start = element.span.start + 1;
    let adjusted_end = if element.tail {
        element.span.end - 1
    } else {
        element.span.end - 2
    };
    let adjusted_span = Span::new(adjusted_start, adjusted_end);
    node_header!(w, "TemplateElement", adjusted_span, ctx);
    // `trv`, not `raw`: the wire reports the element's VALUE, where ECMAScript
    // has already folded `<CR>` / `<CR><LF>` to `<LF>` (§12.9.6). The source
    // slice is the printer's question.
    let trv = element.trv(ctx.source, ctx.view);
    raw!(w, "TemplateElement", ",\"value\":{\"raw\":");
    w.string(trv);
    raw!(w, "~TemplateElementValue", ",\"cooked\":");
    match element.cooked {
        ast::TemplateCooked::Verbatim => w.string(trv),
        ast::TemplateCooked::Decoded(decoded) => w.string(ctx.view.text(decoded)),
        ast::TemplateCooked::Invalid => w.null(),
    }
    pick!(
        w,
        "TemplateElement",
        element.tail,
        "},\"tail\":true",
        "},\"tail\":false"
    );
    close_node(w, "TemplateElement", adjusted_span, ctx);
}

/// Emits an `ObjectPattern` node. Field order: `properties`, `optional`
/// (only when true), `typeAnnotation?`, `decorators?` (parameter position).
pub(super) fn write_object_pattern(
    w: &mut Sink<'_>,
    obj: &ast::ObjectPattern,
    span: Span,
    ctx: &Ctx<'_>,
) {
    pattern_header(
        w,
        "ObjectPattern",
        ent!("ObjectPattern"),
        span,
        obj.type_annotation(ctx.view),
        ctx,
    );
    keyed!(w, "ObjectPattern", ",\"properties\":");
    write_array(w, ctx.view.pat_props(obj.properties), |w, p| match p {
        ast::ObjectPatternProperty::Property(p) => write_property(w, p, ctx),
        ast::ObjectPatternProperty::RestElement(r) => write_rest_element(w, r, ctx),
    });
    if obj.optional {
        raw!(w, "ObjectPattern", ",\"optional\":true");
    }
    write_type_annotation_field::<{ at!("ObjectPattern") }>(w, obj.type_annotation(ctx.view), ctx);
    write_decorators_field::<{ at!("ObjectPattern") }>(w, obj.decorators(ctx.view), ctx);
    close_node(w, "ObjectPattern", span, ctx);
}

/// Emits a `RestElement` node.
pub(super) fn write_rest_element(w: &mut Sink<'_>, rest: &ast::RestElement, ctx: &Ctx<'_>) {
    node_header!(w, "RestElement", rest.span, ctx);
    keyed!(w, "RestElement", ",\"argument\":");
    write_expression(w, ctx.view.expr(rest.argument), ctx);
    // acorn carries a rest parameter's `?` on the rest element (never on
    // `argument`), between `argument` and `typeAnnotation`, only when present.
    if rest.optional {
        raw!(w, "RestElement", ",\"optional\":true");
    }
    write_type_annotation_field::<{ at!("RestElement") }>(w, rest.type_annotation(ctx.view), ctx);
    close_node(w, "RestElement", rest.span, ctx);
}

/// Emits a `Property` node. Field order: `method`, `shorthand`, `computed`, `key`,
/// `value`, `kind` — acorn's order. (Two acorn-typescript paths assign `kind` before
/// `value` — get/set, `parseGetterSetter`, and generic object methods, the
/// `parsePropertyValue` override; tsv's wire keeps the one order.)
pub(super) fn write_property(w: &mut Sink<'_>, prop: &ast::Property, ctx: &Ctx<'_>) {
    node_header!(w, "Property", prop.span, ctx);
    // `method` and `computed` are nearly always `false`, so the common shape is
    // one literal per `shorthand` value, the four keys folded in.
    if prop.method || prop.computed {
        pick!(
            w,
            "Property",
            prop.method,
            ",\"method\":true",
            ",\"method\":false"
        );
        pick!(
            w,
            "Property",
            prop.shorthand,
            ",\"shorthand\":true",
            ",\"shorthand\":false"
        );
        pick!(
            w,
            "Property",
            prop.computed,
            ",\"computed\":true,\"key\":",
            ",\"computed\":false,\"key\":"
        );
    } else {
        pick!(
            w,
            "Property",
            prop.shorthand,
            ",\"method\":false,\"shorthand\":true,\"computed\":false,\"key\":",
            ",\"method\":false,\"shorthand\":false,\"computed\":false,\"key\":"
        );
    }
    write_expression(w, ctx.view.expr(prop.key), ctx);
    keyed!(w, "Property", ",\"value\":");
    write_property_value(w, prop, ctx);
    // Nearly every property is `init`, so that kind appends the whole
    // field as one literal rather than a runtime-length copy of `as_str()`.
    match prop.kind {
        ast::PropertyKind::Init => raw!(w, "Property", ",\"kind\":\"init\""),
        kind => {
            keyword!(
                w,
                "Property",
                ",\"kind\":\"",
                quoted,
                kind => as_str,
                ast::PropertyKind => [Init, Get, Set]
            );
            raw!(w, "\"");
        }
    }
    close_node(w, "Property", prop.span, ctx);
}

/// A method/get/set value goes through acorn's `parseMethod`; its `FunctionExpression`
/// takes the one field order every `FunctionExpression` does.
///
/// A shorthand default an object LITERAL holds (`({ a = 1 } += x)`, the
/// `CoverInitializedName` on a compound operator's left, which stays a literal) is an
/// `AssignmentPattern` on the wire: acorn reads every `{ a = 1 }` through
/// `parseMaybeDefault`, whatever the literal becomes. The internal literal keeps the
/// `=` assignment it was parsed as; a pattern property already holds the pattern.
fn write_property_value(w: &mut Sink<'_>, prop: &ast::Property, ctx: &Ctx<'_>) {
    let method_value = prop.method || !matches!(prop.kind, ast::PropertyKind::Init);
    match (ctx.view.expr(prop.value), method_value) {
        (
            &ast::Expression {
                kind: ast::ExpressionKind::FunctionExpression(f),
                ..
            },
            true,
        ) => {
            write_function_expression(w, ctx.view.fn_expr(f), ctx);
        }
        (
            ast::Expression {
                kind: ast::ExpressionKind::AssignmentExpression(assign),
                span,
            },
            false,
        ) if prop.shorthand => {
            debug_assert_eq!(assign.operator, ast::AssignmentOperator::Assign);
            node_header!(w, "AssignmentPattern", *span, ctx);
            keyed!(w, "AssignmentPattern", ",\"left\":");
            write_expression(w, ctx.view.expr(assign.left), ctx);
            keyed!(w, "AssignmentPattern", ",\"right\":");
            write_expression(w, ctx.view.expr(assign.right), ctx);
            close_node(w, "AssignmentPattern", *span, ctx);
        }
        _ => write_expression(w, ctx.view.expr(prop.value), ctx),
    }
}

/// Emits an `AssignmentPattern` node, with the span override the
/// `TSParameterProperty` quirk needs (`span` is normally `pattern.span`; the
/// quirk widens it to the whole parameter property).
pub(super) fn write_assignment_pattern(
    w: &mut Sink<'_>,
    pattern: &ast::AssignmentPattern,
    ctx: &Ctx<'_>,
    span: Span,
) {
    node_header!(w, "AssignmentPattern", span, ctx);
    keyed!(w, "AssignmentPattern", ",\"left\":");
    // A default's target keeps its type-assertion wrapper (`{ a: (b as T) = 1 }` →
    // `TSAsExpression`): acorn-typescript's `toAssignable` validates through the
    // assertion but preserves the node — same as the simple `=` left in
    // `write_expression`'s `AssignmentExpression` arm. The internal-only JSDoc cast is
    // still peeled, by `write_expression` itself.
    write_expression(w, ctx.view.expr(pattern.left), ctx);
    keyed!(w, "AssignmentPattern", ",\"right\":");
    write_expression(w, ctx.view.expr(pattern.right), ctx);
    write_decorators_field::<{ at!("AssignmentPattern") }>(
        w,
        ctx.view.opt_decorators(pattern.decorators),
        ctx,
    );
    close_node(w, "AssignmentPattern", span, ctx);
}
