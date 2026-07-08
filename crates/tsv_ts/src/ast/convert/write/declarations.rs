// Type alias, function, and class declaration writers.

use super::super::super::internal;
use super::super::Schema;
use super::expressions::{ExprFlags, write_expression, write_expression_inner, write_expressions};
use super::statements::{write_block_statement, write_statement};
use super::types::{write_index_signature, write_type, write_type_parameter_instantiation};
use super::{
    Ctx, JsonWriter, close_node, node_header, write_array, write_identifier_plain,
    write_identifier_with_optional, write_name, write_or_null, write_return_type_field,
    write_type_annotation_field, write_type_parameters_field,
};
use tsv_lang::Span;
use tsv_lang::source_scan::{self, TriviaProfile};

/// Emits a `Decorator` node: an unparenthesized decorator's call/member
/// spine omits `optional`; a parenthesized `@(expr)` rides the full expression
/// parser and keeps it. Parens are stripped from the expression, so the only
/// signal is the span gap.
pub(super) fn write_decorator(
    w: &mut JsonWriter,
    decorator: &internal::Decorator<'_>,
    ctx: &Ctx<'_>,
) {
    let parenthesized = decorator.span.end > decorator.expression.span().end;
    node_header(w, "Decorator", decorator.span, ctx);
    w.raw(",\"expression\":");
    write_expression_inner(
        w,
        &decorator.expression,
        ctx,
        ExprFlags {
            in_chain: false,
            force_optional: false,
            strip_optional: !parenthesized,
        },
    );
    close_node(w, "Decorator", decorator.span, ctx);
}

/// Emit a `decorators` field when the internal node carries decorators
/// (`Option<Vec>` with skip-if-none: present ⇒ emitted, even empty).
pub(super) fn write_decorators_field(
    w: &mut JsonWriter,
    decorators: Option<&[internal::Decorator<'_>]>,
    ctx: &Ctx<'_>,
) {
    if let Some(decs) = decorators {
        w.raw(",\"decorators\":");
        write_array(w, decs, |w, d| write_decorator(w, d, ctx));
    }
}

/// Emits a `TSTypeAliasDeclaration` node. Field order: `id`,
/// `typeParameters?`, `typeAnnotation`, `declare` (only when true).
pub(super) fn write_type_alias_declaration(
    w: &mut JsonWriter,
    type_alias: &internal::TSTypeAliasDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSTypeAliasDeclaration", type_alias.span, ctx);
    w.raw(",\"id\":");
    write_identifier_plain(w, &type_alias.id, ctx);
    write_type_parameters_field(w, type_alias.type_parameters.as_ref(), ctx);
    w.raw(",\"typeAnnotation\":");
    write_type(w, &type_alias.type_annotation, ctx);
    if type_alias.declare {
        w.raw(",\"declare\":true");
    }
    close_node(w, "TSTypeAliasDeclaration", type_alias.span, ctx);
}

/// Emits a `FunctionDeclaration` node. Field order: `id` (nullable),
/// `expression` (always false), `generator`, `async`, `typeParameters?`,
/// `params`, `returnType?`, `body`.
pub(super) fn write_function_declaration(
    w: &mut JsonWriter,
    func_decl: &internal::FunctionDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "FunctionDeclaration", func_decl.span, ctx);
    w.raw(",\"id\":");
    write_or_null(w, func_decl.id.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    w.raw(",\"expression\":false,\"generator\":");
    w.bool(func_decl.generator);
    w.raw(",\"async\":");
    w.bool(func_decl.r#async);
    write_type_parameters_field(w, func_decl.type_parameters.as_ref(), ctx);
    w.raw(",\"params\":");
    write_expressions(w, func_decl.params, ctx);
    write_return_type_field(w, func_decl.return_type.as_ref(), ctx);
    w.raw(",\"body\":");
    write_block_statement(w, &func_decl.body, ctx);
    close_node(w, "FunctionDeclaration", func_decl.span, ctx);
}

/// The super-class wrap decision: when
/// `extends Base<T>` sits on a different line from the closing `>` of the type
/// parameters, acorn-typescript emits `superClass` as a
/// `TSInstantiationExpression` consuming `superTypeParameters`. Returns the
/// combined span to wrap with, or `None` for the plain shape. All offsets are
/// internal byte offsets (the same-line scan byte-indexes `source`).
fn super_class_wrap_span(
    type_params_span: Option<Span>,
    super_class: Option<&internal::Expression<'_>>,
    super_type_parameters_end: Option<u32>,
    source: &str,
) -> Option<Span> {
    let tp_span = type_params_span?;
    // The public super-class node starts at the JsdocCast-unwrapped inner
    // expression.
    let sc_start = super_class.map(|e| e.unwrap_jsdoc_casts().span().start)?;
    let stp_end = super_type_parameters_end?;
    if tsv_lang::printing::is_same_line(source, tp_span.end, sc_start) {
        return None;
    }
    Some(Span::new(sc_start, stp_end))
}

/// Emit the `superClass` (nullable) and `superTypeParameters?` fields, applying
/// the wrap decision. Shared by class declarations and expressions.
fn write_super_class_fields(
    w: &mut JsonWriter,
    super_class: Option<&internal::Expression<'_>>,
    super_type_parameters: Option<&internal::TSTypeParameterInstantiation<'_>>,
    type_params_span: Option<Span>,
    ctx: &Ctx<'_>,
) {
    let wrap_span = super_class_wrap_span(
        type_params_span,
        super_class,
        super_type_parameters.map(|tp| tp.span.end),
        ctx.source,
    );
    w.raw(",\"superClass\":");
    if let (Some(e), Some(stp), Some(ws)) = (super_class, super_type_parameters, wrap_span) {
        // Wrapped: `superTypeParameters` is consumed into the wrapper.
        node_header(w, "TSInstantiationExpression", ws, ctx);
        w.raw(",\"expression\":");
        write_expression(w, e, ctx);
        w.raw(",\"typeArguments\":");
        write_type_parameter_instantiation(w, stp, ctx);
        close_node(w, "TSInstantiationExpression", ws, ctx);
    } else {
        write_or_null(w, super_class, |w, e| write_expression(w, e, ctx));
        if let Some(stp) = super_type_parameters {
            w.raw(",\"superTypeParameters\":");
            write_type_parameter_instantiation(w, stp, ctx);
        }
    }
}

/// Emits a `ClassDeclaration` node. Field order: `declare?` (statement
/// position), `abstract?`, `decorators?`, `id` (nullable), `typeParameters?`,
/// `superClass` (nullable), `superTypeParameters?`, `implements?`, `body`,
/// `declare?` (post-hoc position) — acorn-typescript's statement-level
/// `declare class` stamps `declare` before parsing the class
/// (`tsTryParseDeclare`), but `declare abstract class` (the
/// `tsParseAbstractDeclaration` route) and every `export declare` form stamp
/// the finished node, so the field's position depends on `exported`/`abstract`.
/// `abstract` itself is assigned at its keyword, decorators attach after it.
pub(super) fn write_class_declaration(
    w: &mut JsonWriter,
    class_decl: &internal::ClassDeclaration<'_>,
    ctx: &Ctx<'_>,
    exported: bool,
) {
    node_header(w, "ClassDeclaration", class_decl.span, ctx);
    let declare_last = exported || class_decl.r#abstract;
    if class_decl.declare && !declare_last {
        w.raw(",\"declare\":true");
    }
    if class_decl.r#abstract {
        w.raw(",\"abstract\":true");
    }
    write_decorators_field(w, class_decl.decorators, ctx);
    w.raw(",\"id\":");
    write_or_null(w, class_decl.id.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    write_type_parameters_field(w, class_decl.type_parameters.as_ref(), ctx);
    write_super_class_fields(
        w,
        class_decl.super_class,
        class_decl.super_type_parameters.as_ref(),
        class_decl.type_parameters.as_ref().map(|tp| tp.span),
        ctx,
    );
    write_implements_field(w, class_decl.implements, ctx);
    w.raw(",\"body\":");
    write_class_body(w, &class_decl.body, ctx);
    if class_decl.declare && declare_last {
        w.raw(",\"declare\":true");
    }
    close_node(w, "ClassDeclaration", class_decl.span, ctx);
}

/// Emits a `ClassExpression` node (no `declare` field).
pub(super) fn write_class_expression(
    w: &mut JsonWriter,
    class_expr: &internal::ClassExpression<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ClassExpression", class_expr.span, ctx);
    write_decorators_field(w, class_expr.decorators, ctx);
    if class_expr.r#abstract {
        w.raw(",\"abstract\":true");
    }
    w.raw(",\"id\":");
    write_or_null(w, class_expr.id.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    write_type_parameters_field(w, class_expr.type_parameters.as_ref(), ctx);
    write_super_class_fields(
        w,
        class_expr.super_class,
        class_expr.super_type_parameters.as_ref(),
        class_expr.type_parameters.as_ref().map(|tp| tp.span),
        ctx,
    );
    write_implements_field(w, class_expr.implements, ctx);
    w.raw(",\"body\":");
    write_class_body(w, &class_expr.body, ctx);
    close_node(w, "ClassExpression", class_expr.span, ctx);
}

/// The `implements` field is skip-if-empty — an empty internal list emits no
/// field.
fn write_implements_field(
    w: &mut JsonWriter,
    implements: &[internal::TSInterfaceHeritage<'_>],
    ctx: &Ctx<'_>,
) {
    if implements.is_empty() {
        return;
    }
    w.raw(",\"implements\":");
    write_array(w, implements, |w, h| {
        write_expression_with_type_arguments(w, h, ctx);
    });
}

/// Emits a `ClassBody` node (dispatching each `ClassMember`).
fn write_class_body(w: &mut JsonWriter, body: &internal::ClassBody<'_>, ctx: &Ctx<'_>) {
    node_header(w, "ClassBody", body.span, ctx);
    w.raw(",\"body\":");
    write_array(w, body.body, |w, m| match m {
        internal::ClassMember::MethodDefinition(method) => {
            write_method_definition(w, method, ctx);
        }
        internal::ClassMember::PropertyDefinition(prop) => {
            write_property_definition(w, prop, ctx);
        }
        internal::ClassMember::StaticBlock(block) => {
            // Always TypeScript class context.
            node_header(w, "StaticBlock", block.span, ctx);
            w.raw(",\"body\":");
            write_array(w, block.body, |w, s| {
                write_statement(w, s, ctx, Schema::Acorn);
            });
            close_node(w, "StaticBlock", block.span, ctx);
        }
        internal::ClassMember::IndexSignature(sig) => {
            write_index_signature(w, sig, ctx);
        }
    });
    close_node(w, "ClassBody", body.span, ctx);
}

/// A present class-member modifier: its source keyword and the exact field
/// fragment to emit. See `write_modifiers_in_source_order`.
struct MemberModifier {
    keyword: &'static str,
    fragment: &'static str,
}

const fn accessibility_modifier(acc: internal::Accessibility) -> MemberModifier {
    MemberModifier {
        keyword: acc.as_str(),
        fragment: match acc {
            internal::Accessibility::Public => ",\"accessibility\":\"public\"",
            internal::Accessibility::Private => ",\"accessibility\":\"private\"",
            internal::Accessibility::Protected => ",\"accessibility\":\"protected\"",
        },
    }
}

/// Emits a class member's modifier fields (everything before `computed`) in
/// source order, `static` always included.
///
/// acorn-typescript's `tsParseModifiers` assigns each modifier field as it
/// parses the keyword, so the wire order follows the member's source order;
/// `static` is (re-)assigned right after the loop, so when absent from source
/// it still emits (`false`) after the source modifiers. With at most one
/// keyword present there is nothing to order and no scan runs; otherwise the
/// keywords are re-read from the scan window (member start past any
/// decorators, up to the key) with the trivia-aware cursor — comments legally
/// separate modifiers from names (`static /* c */ a = 1`). Every present
/// modifier's keyword precedes the key, so the pending set empties before the
/// walk can reach it; anything unmatched (malformed input) still emits after
/// the walk, in declaration order, so output stays deterministic.
fn write_modifiers_in_source_order(
    w: &mut JsonWriter,
    mods: &mut [Option<MemberModifier>],
    is_static: bool,
    scan_start: u32,
    scan_end: u32,
    ctx: &Ctx<'_>,
) {
    let mut pending = mods.iter().flatten().count();
    let mut static_pending = true;
    if pending + usize::from(is_static) > 1 {
        let bytes = ctx.source.as_bytes();
        let mut i = scan_start as usize;
        let end = (scan_end as usize).min(bytes.len());
        while (pending > 0 || (is_static && static_pending)) && i < end {
            if bytes[i].is_ascii_whitespace() {
                i += 1;
                continue;
            }
            if let Some(next) = source_scan::skip_trivia(bytes, i, end, TriviaProfile::JS) {
                i = next;
                continue;
            }
            let word_start = i;
            while i < end
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
            {
                i += 1;
            }
            if i == word_start {
                break; // non-word byte — the key or punctuation
            }
            let word = &ctx.source[word_start..i];
            if is_static && static_pending && word == "static" {
                w.raw(",\"static\":true");
                static_pending = false;
            } else if let Some(m) = mods.iter_mut().find_map(|slot| {
                slot.as_ref()
                    .is_some_and(|m| m.keyword == word)
                    .then(|| slot.take())
                    .flatten()
            }) {
                w.raw(m.fragment);
                pending -= 1;
            } else {
                break; // not a pending modifier — `async`, `get`, the key, …
            }
        }
    }
    for m in mods.iter_mut().filter_map(Option::take) {
        w.raw(m.fragment);
    }
    if static_pending {
        w.raw(",\"static\":");
        w.bool(is_static);
    }
}

/// The modifier scan window's start: past the decorators when present (they
/// precede the modifiers), else the member's own start.
fn modifier_scan_start(decorators: Option<&[internal::Decorator<'_>]>, span: Span) -> u32 {
    decorators
        .and_then(<[_]>::last)
        .map_or(span.start, |d| d.span.end)
}

/// Emits a `MethodDefinition` node. Field order: source-order modifiers
/// (`abstract?`, `accessibility?`, `override?`, `static` — see
/// `write_modifiers_in_source_order`), `computed`, `key`, `optional?`, `kind`,
/// `typeParameters?` (moved here from the FunctionExpression, acorn
/// convention), `value`, `decorators?` (attached to the finished member, so
/// they serialize last).
fn write_method_definition(
    w: &mut JsonWriter,
    method: &internal::MethodDefinition<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "MethodDefinition", method.span, ctx);
    let mut mods = [
        method.r#abstract.then_some(MemberModifier {
            keyword: "abstract",
            fragment: ",\"abstract\":true",
        }),
        method.accessibility.map(accessibility_modifier),
        method.r#override.then_some(MemberModifier {
            keyword: "override",
            fragment: ",\"override\":true",
        }),
    ];
    write_modifiers_in_source_order(
        w,
        &mut mods,
        method.is_static,
        modifier_scan_start(method.decorators, method.span),
        method.key.span().start,
        ctx,
    );
    w.raw(",\"computed\":");
    w.bool(method.computed);
    w.raw(",\"key\":");
    write_expression(w, &method.key, ctx);
    if method.optional {
        w.raw(",\"optional\":true");
    }
    w.raw(",\"kind\":");
    w.token(method.kind.as_str());
    let func = &method.value;
    write_type_parameters_field(w, func.type_parameters.as_ref(), ctx);
    // Abstract methods and overload signatures emit TSDeclareMethod (no
    // body): abstract flag OR an empty body with a zero-width (synthetic)
    // span. The value span starts at the `(`, not at the method keyword.
    // Both value shapes are otherwise identical — typeParameters stays on
    // the MethodDefinition, never on the value node.
    let is_bodyless = method.r#abstract
        || (func.body.body.is_empty() && func.body.span.start == func.body.span.end);
    let (value_type, body) = if is_bodyless {
        ("TSDeclareMethod", None)
    } else {
        ("FunctionExpression", Some(&func.body))
    };
    w.raw(",\"value\":");
    let value_span = Span::new(func.params_start, func.span.end);
    node_header(w, value_type, value_span, ctx);
    w.raw(",\"id\":");
    write_or_null(w, func.id.as_ref(), |w, id| {
        write_identifier_with_optional(w, id, ctx);
    });
    w.raw(",\"expression\":false,\"generator\":");
    w.bool(func.generator);
    w.raw(",\"async\":");
    w.bool(func.r#async);
    w.raw(",\"params\":");
    write_expressions(w, func.params, ctx);
    write_return_type_field(w, func.return_type.as_ref(), ctx);
    if let Some(body) = body {
        w.raw(",\"body\":");
        write_block_statement(w, body, ctx);
    }
    // Close the `value` function node, then the `MethodDefinition`.
    close_node(w, value_type, value_span, ctx);
    write_decorators_field(w, method.decorators, ctx);
    close_node(w, "MethodDefinition", method.span, ctx);
}

/// Emits a `PropertyDefinition` node. Field order: source-order modifiers
/// (`declare?`, `abstract?`, `accessor?`, `accessibility?`, `readonly?`,
/// `override?`, `static` — see `write_modifiers_in_source_order`), `computed`,
/// `key`, `optional?`, `definite?`, `typeAnnotation?`, `value` (nullable),
/// `decorators?` (attached to the finished member, so they serialize last).
fn write_property_definition(
    w: &mut JsonWriter,
    prop: &internal::PropertyDefinition<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "PropertyDefinition", prop.span, ctx);
    let mut mods = [
        prop.declare.then_some(MemberModifier {
            keyword: "declare",
            fragment: ",\"declare\":true",
        }),
        prop.r#abstract.then_some(MemberModifier {
            keyword: "abstract",
            fragment: ",\"abstract\":true",
        }),
        prop.accessor.then_some(MemberModifier {
            keyword: "accessor",
            fragment: ",\"accessor\":true",
        }),
        prop.accessibility.map(accessibility_modifier),
        prop.readonly.then_some(MemberModifier {
            keyword: "readonly",
            fragment: ",\"readonly\":true",
        }),
        prop.r#override.then_some(MemberModifier {
            keyword: "override",
            fragment: ",\"override\":true",
        }),
    ];
    write_modifiers_in_source_order(
        w,
        &mut mods,
        prop.is_static,
        modifier_scan_start(prop.decorators, prop.span),
        prop.key.span().start,
        ctx,
    );
    w.raw(",\"computed\":");
    w.bool(prop.computed);
    w.raw(",\"key\":");
    write_expression(w, &prop.key, ctx);
    if matches!(prop.modifier, internal::PropertyModifier::Optional) {
        w.raw(",\"optional\":true");
    }
    if matches!(prop.modifier, internal::PropertyModifier::Definite) {
        w.raw(",\"definite\":true");
    }
    write_type_annotation_field(w, prop.type_annotation.as_ref(), ctx);
    w.raw(",\"value\":");
    write_or_null(w, prop.value.as_ref(), |w, v| write_expression(w, v, ctx));
    write_decorators_field(w, prop.decorators, ctx);
    close_node(w, "PropertyDefinition", prop.span, ctx);
}

/// Emits a `TSTypeParameterDeclaration` node. Field order: `params`, `extra?`
/// (`{"trailingComma":N}`, emitted like `start`/`end` in the mapper's output
/// space).
pub(super) fn write_type_parameter_declaration(
    w: &mut JsonWriter,
    params: &internal::TSTypeParameterDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSTypeParameterDeclaration", params.span, ctx);
    w.raw(",\"params\":");
    write_array(w, params.params, |w, p| write_type_parameter(w, p, ctx));
    if let Some(pos) = params.trailing_comma {
        w.raw(",\"extra\":{\"trailingComma\":");
        w.u32(ctx.loc.pos(pos));
        w.raw("}");
    }
    close_node(w, "TSTypeParameterDeclaration", params.span, ctx);
}

/// Emits a `TSTypeParameter` node. Field order: `const`/`in`/`out` (each
/// only when true), `name`, `constraint?`, `default?`.
pub(super) fn write_type_parameter(
    w: &mut JsonWriter,
    param: &internal::TSTypeParameter<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSTypeParameter", param.span, ctx);
    if param.is_const {
        w.raw(",\"const\":true");
    }
    if param.is_in {
        w.raw(",\"in\":true");
    }
    if param.is_out {
        w.raw(",\"out\":true");
    }
    w.raw(",\"name\":");
    write_name(w, param.name.ident_name(), param.name.span.start, ctx);
    if let Some(c) = &param.constraint {
        w.raw(",\"constraint\":");
        write_type(w, c, ctx);
    }
    if let Some(d) = &param.default {
        w.raw(",\"default\":");
        write_type(w, d, ctx);
    }
    close_node(w, "TSTypeParameter", param.span, ctx);
}

/// Emits a `TSExpressionWithTypeArguments` node (an implements clause): the
/// `expression` is the heritage entity name rendered as an expression, plus
/// `typeParameters?`.
fn write_expression_with_type_arguments(
    w: &mut JsonWriter,
    heritage: &internal::TSInterfaceHeritage<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSExpressionWithTypeArguments", heritage.span, ctx);
    w.raw(",\"expression\":");
    write_entity_name_to_expression(w, &heritage.expression, ctx);
    if let Some(ta) = &heritage.type_arguments {
        w.raw(",\"typeParameters\":");
        write_type_parameter_instantiation(w, ta, ctx);
    }
    close_node(w, "TSExpressionWithTypeArguments", heritage.span, ctx);
}

/// Renders an entity name as an expression: `Foo` emits an `Identifier`
/// (carrying the binding's `optional` flag), `Foo.Bar` a `MemberExpression`
/// with `computed:false, optional:false`.
fn write_entity_name_to_expression(
    w: &mut JsonWriter,
    entity: &internal::TSEntityName<'_>,
    ctx: &Ctx<'_>,
) {
    match entity {
        internal::TSEntityName::Identifier(id) => write_identifier_with_optional(w, id, ctx),
        internal::TSEntityName::QualifiedName(qn) => {
            node_header(w, "MemberExpression", qn.span, ctx);
            w.raw(",\"object\":");
            write_entity_name_to_expression(w, &qn.left, ctx);
            w.raw(",\"property\":");
            write_identifier_with_optional(w, &qn.right, ctx);
            w.raw(",\"computed\":false,\"optional\":false");
            close_node(w, "MemberExpression", qn.span, ctx);
        }
    }
}
