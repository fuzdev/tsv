//! Attribute emission: static inline values, dynamic `$.attr`/`$.attr_class`/
//! `$.attr_style` calls, and mixed text+expression attribute templates.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{InfallibleResolve, Span};
use tsv_svelte::ast::internal::{
    Attribute, AttributeValue, ClassDirective, StyleDirective, StyleDirectiveValue,
};
use tsv_ts::ast::internal::{
    ArrayExpression, Expression, LiteralValue, ObjectExpression, ObjectProperty, Property,
    PropertyKind,
};

use crate::analyze::{evaluate, stringify_value};
use crate::build::escape_template_text;
use crate::css_scope::SCOPE_HASH_CLASS;
use crate::fragment::{BodyBuilder, escape_html_attr, guard_dropped, wrap_value_expr};
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// Elements that emit `load`/`error` events (Svelte's `LOAD_ERROR_ELEMENTS`,
/// `utils.js`): an `onload`/`onerror` handler on one of these injects an
/// `on{name}="this.__e=event"` capture attribute instead of being dropped.
const LOAD_ERROR_ELEMENTS: &[&str] = &[
    "body", "embed", "iframe", "img", "link", "object", "script", "style", "track",
];

/// Whether `name` is a load-error element (see [`LOAD_ERROR_ELEMENTS`]).
pub(crate) fn is_load_error_element(name: &str) -> bool {
    LOAD_ERROR_ELEMENTS.contains(&name)
}

/// The DOM boolean attributes (the oracle's `DOM_BOOLEAN_ATTRIBUTES`): a
/// dynamic value on one of these emits `$.attr(name, value, true)`.
// TODO: consider a home in tsv_html beside the element classification tables.
const DOM_BOOLEAN_ATTRIBUTES: &[&str] = &[
    "allowfullscreen",
    "async",
    "autofocus",
    "autoplay",
    "checked",
    "controls",
    "default",
    "disabled",
    "formnovalidate",
    "indeterminate",
    "inert",
    "ismap",
    "loop",
    "multiple",
    "muted",
    "nomodule",
    "novalidate",
    "open",
    "playsinline",
    "readonly",
    "required",
    "reversed",
    "seamless",
    "selected",
    "webkitdirectory",
    "defer",
    "disablepictureinpicture",
    "disableremoteplayback",
];

/// Emit one plain attribute. Static text values inline (with entity decoding,
/// attribute escaping, `class`/`style` whitespace collapse, and the scope hash
/// on matched classes); dynamic and mixed values emit the oracle's runtime
/// calls (`$.attr` / `$.attr_class` / `$.attr_style`).
pub(crate) fn emit_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attr: &'arena Attribute<'arena>,
    element_name: &str,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // The oracle lowercases attribute names outside foreign namespaces (svg
    // refuses above) — at EMISSION only. The event-handler decision below tests
    // the RAW authored name, so both are kept.
    let raw_name = env
        .b
        .interner
        .borrow()
        .resolve_infallible(attr.name)
        .to_string();
    let name = raw_name.to_ascii_lowercase();

    // `value` on <textarea> becomes child content, on <select> it is omitted
    // with select_value bookkeeping — neither shape is implemented.
    if name == "value" && (element_name == "textarea" || element_name == "select") {
        return Err(unsupported(Refusal::ValueAttribute {
            name: element_name.to_string(),
        }));
    }

    let Some(values) = attr.value else {
        // Boolean attribute: the oracle emits `name=""`.
        out.push_text(&escape_template_text(&format!(" {name}=\"\"")));
        return Ok(());
    };

    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            let mut value = if name == "class" || name == "style" {
                collapse_attr_whitespace(&decoded)
            } else {
                decoded.into_owned()
            };
            // A string-valued `class` that collapses+trims to empty is dropped
            // entirely (oracle-probed: `class=""` and `class="  "` emit no
            // attribute). Class-specific and static-path-specific: a bare
            // `class` (boolean form, handled above) keeps `class=""`, empty
            // `style`/`id` stay, and a *folded* mixed class keeps `class=""`
            // (see `emit_mixed_attribute`).
            if name == "class" && value.is_empty() {
                return Ok(());
            }
            if name == "class"
                && let Some(scope) = &env.scope
            {
                let mut matched = false;
                for class in value.split_ascii_whitespace() {
                    if scope.class_names.contains(class) {
                        env.matched_classes.insert(class.to_string());
                        matched = true;
                    }
                }
                if matched {
                    value.push(' ');
                    value.push_str(SCOPE_HASH_CLASS);
                }
            }
            out.push_text(&escape_template_text(&format!(
                " {name}=\"{}\"",
                escape_html_attr(&value)
            )));
            Ok(())
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // An `on`-prefixed single-expression attribute is an event handler —
            // tested on the RAW authored name, exactly like the oracle
            // (`is_event_attribute`, server `element.js:71`, runs before any
            // lowercasing): `onClick` drops, but `ONCLICK`/`oNclick` are NOT
            // events and emit as regular `$.attr('onclick', …)` attributes
            // (probe-verified). A dropped handler's expression still feeds
            // `needs_context` (walked up front in `needs_context.rs`), so a
            // `new`/prop-rooted member or call inside it still forces the
            // wrapper — only the attribute markup is dropped. Raw `onload`/
            // `onerror` (exact match — `onLoad` on `<img>` is a plain drop,
            // probe-verified) on a load-error element are the exception (the
            // oracle injects an `on{name}="this.__e=event"` capture attribute),
            // refused for now.
            if raw_name.starts_with("on") {
                if (raw_name == "onload" || raw_name == "onerror")
                    && is_load_error_element(element_name)
                {
                    return Err(unsupported(Refusal::EventCaptureAttribute { name }));
                }
                // Dropped, but still guarded: a misplaced rune inside a handler is
                // an oracle analysis-phase error, not an emission one.
                return guard_dropped(env, &tag.expression);
            }
            // Quoted (`class="{a}"`) vs bare (`class={a}`): the oracle's AST
            // represents the quoted form as a one-chunk ARRAY and the bare form
            // as a plain ExpressionTag (the wire writer's `preceded_by_quote`
            // discriminant) — the split the class `$.clsx` rule keys on.
            let quoted = preceded_by_quote(env.source, tag.span.start);
            emit_dynamic_attribute(env, &name, &tag.expression, quoted, out)
        }
        _ => {
            // A mixed-value attribute whose RAW name starts with `on` is an
            // event attribute the oracle rejects as input
            // (`attribute_invalid_event_handler`) — refuse rather than guess.
            // `ONCLICK="a {h}"` is NOT an event (raw test) and emits through the
            // normal mixed path (probe-verified).
            if raw_name.starts_with("on") {
                return Err(unsupported(Refusal::EventAttribute { name }));
            }
            emit_mixed_attribute(env, &name, values, out)
        }
    }
}

/// Whether the byte before `pos` is a quote — the same discriminant the wire
/// writer uses to emit a quoted single-expression attribute value as an array.
fn preceded_by_quote(source: &str, pos: u32) -> bool {
    matches!(
        (pos as usize)
            .checked_sub(1)
            .and_then(|i| source.as_bytes().get(i)),
        Some(b'"' | b'\'')
    )
}

/// The oracle's `needs_clsx` rule (`2-analyze/visitors/Attribute.js`): only a
/// bare `class={expr}` wraps in `$.clsx`, and only when the expression is not a
/// `Literal`, `TemplateLiteral`, or ESTree `BinaryExpression`. tsv's internal
/// AST folds logical operators into `BinaryExpression`, but ESTree types them
/// `LogicalExpression` (`&&`/`||`/`??` DO wrap — oracle-probed), so the
/// exclusion is arithmetic/comparison binaries only. The terminal arm mirrors
/// the oracle's own negative-list rule: everything else wraps.
fn class_needs_clsx(expr: &Expression<'_>, quoted: bool) -> bool {
    if quoted {
        return false;
    }
    match expr {
        Expression::Literal(_) | Expression::TemplateLiteral(_) => false,
        Expression::BinaryExpression(b) => b.operator.is_logical(),
        _ => true,
    }
}

/// `class`/`style` value whitespace collapse (`[ \t\n\r\f]+` → one space, then
/// trim) — the oracle's `WHITESPACE_INSENSITIVE_ATTRIBUTES` handling.
fn collapse_attr_whitespace(decoded: &str) -> String {
    let mut collapsed = String::with_capacity(decoded.len());
    let mut in_ws = false;
    for c in decoded.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c') {
            in_ws = true;
        } else {
            if in_ws && !collapsed.is_empty() {
                collapsed.push(' ');
            }
            in_ws = false;
            collapsed.push(c);
        }
    }
    collapsed
}

/// A single-expression attribute value: `title={expr}` (or quoted,
/// `title="{expr}"` — `quoted` carries the distinction, which only the class
/// `$.clsx` rule reads).
fn emit_dynamic_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    expr: &'arena Expression<'arena>,
    quoted: bool,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // Event handlers (`on*` single-expression attributes) are dropped in
    // `emit_attribute` before dispatch, so they never reach here.
    // The `$.attr` family interleaves minted (appendix) and borrowed (host)
    // argument spans; with host comments present their windows would sweep.
    if env.has_comments {
        return Err(unsupported(Refusal::CommentsAlongsideExprAttributes));
    }
    // The template borrow point. Both shape predicates below read the node's
    // variant, so they must see the ERASED one: `class={'a' as string}` is a
    // string literal to the oracle (inline-literal path), and a `TSAsExpression`
    // left in place would take the `$.clsx` branch instead.
    let expr = env.erase(expr)?;
    // A string-literal expression value takes the oracle's inline-literal path
    // (pre-escaped static emission) — refuse rather than guess its edge rules.
    if matches!(expr, Expression::Literal(lit)
        if matches!(lit.value, LiteralValue::String(_)))
    {
        return Err(unsupported(Refusal::StringLiteralExprAttribute));
    }

    let wrapped = wrap_value_expr(env, expr)?;
    let call = match name {
        // Dynamic class/style interact with CSS scoping (hash argument,
        // pruning) — supported only on unstyled components.
        "class" => {
            if env.scope.is_some() {
                return Err(unsupported(Refusal::DynamicClassOnStyled));
            }
            if class_needs_clsx(expr, quoted) {
                let clsx = env.b.member_call("$", "clsx", wrapped);
                let clsx_alloc = env.b.arena.alloc(clsx);
                env.b
                    .member_call("$", "attr_class", std::slice::from_ref(clsx_alloc))
            } else {
                env.b
                    .member_call("$", "attr_class", std::slice::from_ref(&wrapped[0]))
            }
        }
        "style" => {
            if env.scope.is_some() {
                return Err(unsupported(Refusal::DynamicStyleOnStyled));
            }
            env.b.member_call("$", "attr_style", wrapped)
        }
        _ => {
            let mut args = BumpVec::new_in(env.b.arena);
            args.push(env.b.string_literal_expr(name));
            args.push(wrapped[0].clone());
            if DOM_BOOLEAN_ATTRIBUTES.contains(&name) {
                args.push(env.b.true_literal());
            }
            env.b.member_call("$", "attr", args.into_bump_slice())
        }
    };
    out.push_expr(call);
    Ok(())
}

/// A mixed text+expression attribute value: `title="t {a} u"` — an attribute
/// template literal with `$.stringify(expr)` interpolations (omitted when the
/// oracle's evaluator proves a defined string), folded where known.
fn emit_mixed_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    values: &'arena [AttributeValue<'arena>],
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // Event attributes (RAW name starting `on`) are refused by the dispatch in
    // `emit_attribute` before this is reached.
    if env.has_comments {
        return Err(unsupported(Refusal::CommentsAlongsideExprAttributes));
    }
    if (name == "class" || name == "style") && env.scope.is_some() {
        return Err(unsupported(Refusal::InterpolatedAttrOnStyled {
            name: name.to_string(),
        }));
    }
    let trim_whitespace = name == "class" || name == "style";

    let mut texts: Vec<String> = vec![String::new()];
    // The unescaped folded value, in parallel — consumed only when every part
    // folds statically (the full-fold static emission below).
    let mut raw = String::new();
    let mut exprs: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    for value in values {
        match value {
            AttributeValue::Text(text) => {
                let decoded = text.data(env.source);
                let chunk = if trim_whitespace {
                    // Runs collapse but edges are NOT trimmed per-chunk (the
                    // oracle's replace() without trim in the template path).
                    collapse_runs_no_trim(&decoded)
                } else {
                    decoded.into_owned()
                };
                raw.push_str(&chunk);
                // Attribute templates carry no HTML escaping — the runtime
                // escapes; only template metachars are escaped here.
                #[allow(clippy::unwrap_used)]
                texts
                    .last_mut()
                    .unwrap()
                    .push_str(&escape_template_text(&chunk));
            }
            AttributeValue::ExpressionTag(tag) => {
                // The template borrow point: erase once, then guard AND fold the
                // erased node (the fold gate is the silent-divergence trap).
                let expr = env.erase(&tag.expression)?;
                // Guard first — never fold an oracle-invalid expression.
                let wrapped = wrap_value_expr(env, expr)?;
                let evaluated = evaluate(expr, &env.value_scope(), env.source, 0)
                    .map_err(|g| unsupported(Refusal::StaticEvalNotPortable(g.0)))?;
                if let Some(value) = evaluated.known_value() {
                    // Folds into the quasi — plain `(value ?? '') + ''`, no
                    // HTML escaping in the template-value path.
                    let text = stringify_value(value)
                        .map_err(|g| unsupported(Refusal::StaticFoldNotPortable(g.0)))?;
                    raw.push_str(&text);
                    #[allow(clippy::unwrap_used)]
                    texts
                        .last_mut()
                        .unwrap()
                        .push_str(&escape_template_text(&text));
                    continue;
                }
                let piece = if evaluated.is_defined_string() {
                    wrapped[0].clone()
                } else {
                    env.b.member_call("$", "stringify", wrapped)
                };
                exprs.push(piece);
                texts.push(String::new());
            }
        }
    }

    if exprs.is_empty() {
        // Every part folded statically — the oracle emits a *static* attribute
        // (oracle-probed rules): attr-escape `[&"<]`, folded value verbatim (no
        // trim, no empty-class drop, boolean attributes keep the folded value,
        // null/undefined already stringified to '' by the fold above). Only the
        // chunk-array path folds; a single-expression attribute never does
        // (`emit_dynamic_attribute`).
        out.push_text(&escape_template_text(&format!(
            " {name}=\"{}\"",
            escape_html_attr(&raw)
        )));
        return Ok(());
    }

    let template = env.b.template_literal(&texts, exprs.into_bump_slice());
    let template_alloc = env.b.arena.alloc(template);
    let call = match name {
        "class" => env
            .b
            .member_call("$", "attr_class", std::slice::from_ref(template_alloc)),
        "style" => env
            .b
            .member_call("$", "attr_style", std::slice::from_ref(template_alloc)),
        _ => {
            let mut args = BumpVec::new_in(env.b.arena);
            args.push(env.b.string_literal_expr(name));
            args.push(template_alloc.clone());
            if DOM_BOOLEAN_ATTRIBUTES.contains(&name) {
                args.push(env.b.true_literal());
            }
            env.b.member_call("$", "attr", args.into_bump_slice())
        }
    };
    out.push_expr(call);
    Ok(())
}

/// The base (1st) argument of a `$.attr_class(base, hash, directives)` call — the
/// authored `class` attribute value, or the synthetic empty string. The two arms
/// govern CSS-scope handling: a string-literal base folds the hash into its text
/// and contributes its tokens to selector matching; any other base carries the
/// hash in the 2nd argument and contributes no static tokens.
enum ClassBase<'arena> {
    /// A statically-known string base (`class="foo"`, or the synthetic `''`).
    StringLiteral(String),
    /// A non-string-literal base expression (`$.clsx(w)`, a bare identifier, a
    /// template literal, `true`).
    Expr(Expression<'arena>),
}

/// Build the base expression for an element's `$.attr_class(...)` call from its
/// authored `class` attribute (`None` = no authored class → the oracle's synthetic
/// empty `''`). Mirrors the oracle's `build_attribute_value` + `needs_clsx`
/// handling for the `class` attribute.
fn build_class_base<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    class_attr: Option<&'arena Attribute<'arena>>,
) -> Result<ClassBase<'arena>, CompileError> {
    let Some(attr) = class_attr else {
        // No authored `class` — the oracle injects a synthetic `class=""`
        // (2-analyze/index.js), so the base is the empty string literal.
        return Ok(ClassBase::StringLiteral(String::new()));
    };
    let Some(values) = attr.value else {
        // A bare boolean `class` — the oracle's `build_attribute_value(true)` → `true`.
        return Ok(ClassBase::Expr(env.b.true_literal()));
    };
    match values {
        [AttributeValue::Text(text)] => {
            // Static class value: whitespace-insensitive collapse + trim.
            let decoded = text.data(env.source);
            Ok(ClassBase::StringLiteral(collapse_attr_whitespace(&decoded)))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // Dynamic `class={expr}` / `class="{expr}"`. The oracle wraps in
            // `$.clsx` per the `needs_clsx` rule (`class_needs_clsx`). A
            // string-literal expression takes the oracle's inline-literal path we
            // don't reproduce — refuse, matching the standalone dynamic-attribute
            // path (`emit_dynamic_attribute`).
            let quoted = preceded_by_quote(env.source, tag.span.start);
            let expr = env.erase(&tag.expression)?;
            if matches!(expr, Expression::Literal(lit)
                if matches!(lit.value, LiteralValue::String(_)))
            {
                return Err(unsupported(Refusal::StringLiteralExprAttribute));
            }
            let wrapped = wrap_value_expr(env, expr)?;
            let base = if class_needs_clsx(expr, quoted) {
                env.b.member_call("$", "clsx", wrapped)
            } else {
                wrapped[0].clone()
            };
            Ok(ClassBase::Expr(base))
        }
        // A mixed-value `class="a {b}"` base — deferred (rare).
        _ => Err(unsupported(Refusal::ClassDirectiveWithMixedClass)),
    }
}

/// Build the directives object `{ 'name': expr, … }` (source order) for
/// `$.attr_class`. The key is always a string literal (the oracle's
/// `b.literal(directive.name)`); `format_canonical` drops the quotes for an
/// identifier-safe name, matching the oracle's own canonicalized output. Each
/// value is the directive expression, erased + guarded + derived-rewritten.
fn build_class_directives_object<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    class_directives: &[&'arena ClassDirective<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for directive in class_directives {
        let name = directive.name_span.extract(env.source);
        let key = env.b.string_literal_expr(name);
        let key_span = key.span();
        // The template borrow point: erase once, then guard + rewrite a bare
        // derived read to `d()`.
        let expr = env.erase(&directive.expression)?;
        let value = wrap_value_expr(env, expr)?[0].clone();
        properties.push(ObjectProperty::Property(Property {
            key,
            value,
            kind: PropertyKind::Init,
            shorthand: false,
            computed: false,
            method: false,
            span: key_span,
        }));
    }
    let cbrace = env.b.mint("}").end;
    Ok(Expression::ObjectExpression(ObjectExpression {
        properties: properties.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    }))
}

/// Emit the fused `$.attr_class(base, css_hash, { name: expr, … })` call for a
/// regular element carrying `class:` directives (the oracle's `build_attr_class`,
/// `shared/element.js`). `class_attr` is the authored `class` attribute when
/// present (its value is the base), else `None` (the oracle's synthetic empty
/// `''`).
///
/// CSS scoping: the element is scoped when any of its candidate classes — the
/// static tokens of a string-literal base ∪ the `class:` directive names — is a
/// scoped selector. Each matched class is recorded (so the no-match post-check
/// passes). A scoped hash folds into a string-literal base (`(value + ' ' +
/// hash).trim()`) or, for any other base, rides the 2nd argument; otherwise the
/// 2nd argument is `void 0` (the directives object is always the 3rd argument, so
/// the middle argument is never elided).
pub(crate) fn emit_class_directives<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    class_attr: Option<&'arena Attribute<'arena>>,
    class_directives: &[&'arena ClassDirective<'arena>],
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // The synthetic `$.attr_class` call interleaves minted (appendix) and borrowed
    // (host) argument spans; with carried script comments their windows would
    // sweep — refuse, matching the dynamic-attribute path.
    if env.has_comments {
        return Err(unsupported(Refusal::CommentsAlongsideExprAttributes));
    }

    let base = build_class_base(env, class_attr)?;
    let base_is_string = matches!(base, ClassBase::StringLiteral(_));

    // Scope matching. `env.scope` and `env.matched_classes` are disjoint fields,
    // so the immutable scope borrow and the mutable insert coexist.
    let mut scoped = false;
    if let Some(scope) = &env.scope {
        if let ClassBase::StringLiteral(s) = &base {
            for token in s.split_ascii_whitespace() {
                if scope.class_names.contains(token) {
                    env.matched_classes.insert(token.to_string());
                    scoped = true;
                }
            }
        }
        for directive in class_directives {
            let dname = directive.name_span.extract(env.source);
            if scope.class_names.contains(dname) {
                env.matched_classes.insert(dname.to_string());
                scoped = true;
            }
        }
    }

    // The base expression, folding the scope hash into a string-literal base.
    let base_expr = match base {
        ClassBase::StringLiteral(s) => {
            let text = if scoped {
                format!("{s} {SCOPE_HASH_CLASS}").trim().to_string()
            } else {
                s
            };
            env.b.string_literal_expr(&text)
        }
        ClassBase::Expr(e) => e,
    };

    // The css-hash 2nd argument: `void 0` unless scoped with a non-string base,
    // where it carries the hash literal (a string base folded it in above).
    let css_hash = if scoped && !base_is_string {
        env.b.string_literal_expr(SCOPE_HASH_CLASS)
    } else {
        env.b.void_zero()
    };

    let directives = build_class_directives_object(env, class_directives)?;

    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    args.push(base_expr);
    args.push(css_hash);
    args.push(directives);
    let call = env.b.member_call("$", "attr_class", args.into_bump_slice());
    out.push_expr(call);
    Ok(())
}

/// Whether `name` is a valid JS identifier (`/^[a-zA-Z_$][a-zA-Z_$0-9]*$/`) — the
/// oracle's `regex_is_valid_identifier` gate (`b.key`), which decides whether a
/// style property key prints as a bare identifier or a quoted string. `format_canonical`
/// applies the same test when dropping quotes off a string-literal key, so a
/// non-shorthand key can always be a string literal; the identifier form matters
/// only for the object-shorthand `{ color }` a `style:color` shorthand builds.
fn is_js_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// The `style:` allowed-modifier gate: the oracle accepts only a single
/// `|important` (or none). Any other modifier, or two or more, is
/// `style_directive_invalid_modifier` — an oracle *error*, so tsv refuses rather
/// than compile it.
fn validate_style_modifiers(directive: &StyleDirective<'_>) -> Result<(), CompileError> {
    match directive.modifiers {
        [] | ["important"] => Ok(()),
        _ => Err(unsupported(Refusal::StyleDirectiveInvalidModifier)),
    }
}

/// Build the base (1st) argument of a `$.attr_style(base, directives)` call from
/// the authored `style` attribute (`None` = no authored `style` → the oracle's
/// phase-2 synthetic empty `''`). Mirrors `build_class_base` MINUS `$.clsx` and
/// MINUS any CSS scoping (style is never scoped): a dynamic `style={expr}` is the
/// bare expression, not `$.clsx(expr)`.
fn build_style_base<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    style_attr: Option<&'arena Attribute<'arena>>,
) -> Result<Expression<'arena>, CompileError> {
    let Some(attr) = style_attr else {
        // No authored `style` — the oracle injects a synthetic `style=""`
        // (2-analyze/index.js), so the base is the empty string literal.
        return Ok(env.b.string_literal_expr(""));
    };
    let Some(values) = attr.value else {
        // A bare boolean `style` — the oracle's `build_attribute_value(true)` → `true`.
        return Ok(env.b.true_literal());
    };
    match values {
        [AttributeValue::Text(text)] => {
            // Static style value: whitespace-insensitive collapse + trim, then the
            // attribute HTML-escape the oracle applies to a text base literal
            // (`build_attribute_value`, non-component → `escape_html(data, true)`).
            let decoded = text.data(env.source);
            let collapsed = collapse_attr_whitespace(&decoded);
            Ok(env.b.string_literal_expr(&escape_html_attr(&collapsed)))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // Dynamic `style={expr}` / `style="{expr}"` — the bare expression (NO
            // `$.clsx`, unlike `class`). A string-literal expression takes the
            // oracle's inline-literal path we don't reproduce — refuse, matching the
            // standalone dynamic-attribute path (`emit_dynamic_attribute`).
            let expr = env.erase(&tag.expression)?;
            if matches!(expr, Expression::Literal(lit)
                if matches!(lit.value, LiteralValue::String(_)))
            {
                return Err(unsupported(Refusal::StringLiteralExprAttribute));
            }
            Ok(wrap_value_expr(env, expr)?[0].clone())
        }
        // A mixed-value `style="a {b}"` base — deferred (rare).
        _ => Err(unsupported(Refusal::StyleDirectiveWithMixedStyle)),
    }
}

/// Build one `{ name: value }` property for a `style:` directive (the oracle's
/// `build_attr_style` per-directive `b.init`). The key is the property name,
/// lowercased unless it starts with `--` (custom properties keep case); a
/// shorthand `style:color` prints as object-shorthand `{ color }` when the
/// (lowercased) key coincides with the raw same-name identifier value.
fn build_style_property<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena StyleDirective<'arena>,
) -> Result<Property<'arena>, CompileError> {
    let raw_name = directive.name_span.extract(env.source).to_string();
    // The oracle lowercases the property name unless it is a `--custom-property`.
    let key_name = if raw_name.starts_with("--") {
        raw_name.clone()
    } else {
        raw_name.to_ascii_lowercase()
    };

    // The value, and whether it is the same-name shorthand identifier.
    let (value, is_shorthand_id) = match &directive.value {
        StyleDirectiveValue::True => {
            // Shorthand `style:color` → `b.id(directive.name)` (RAW name).
            (Expression::Identifier(env.b.ident(&raw_name)), true)
        }
        StyleDirectiveValue::ExpressionTag(tag) => {
            // The template borrow point: erase once, then guard + rewrite a bare
            // derived read to `d()`. `|important` does NOT wrap the value.
            let expr = env.erase(&tag.expression)?;
            (wrap_value_expr(env, expr)?[0].clone(), false)
        }
        StyleDirectiveValue::Parts(parts) => match parts {
            // A static `style:color="red"` → the string literal, collapsed +
            // trimmed + attribute-escaped like the base text value.
            [AttributeValue::Text(text)] => {
                let decoded = text.data(env.source);
                let collapsed = collapse_attr_whitespace(&decoded);
                (
                    env.b.string_literal_expr(&escape_html_attr(&collapsed)),
                    false,
                )
            }
            // A mixed `style:color="a {b}"` value — deferred (rare).
            _ => return Err(unsupported(Refusal::StyleDirectiveWithMixedValue)),
        },
    };

    // Object-shorthand `{ color }` requires an identifier key equal to the value
    // identifier's name — i.e. a shorthand whose lowercased name is unchanged and
    // identifier-safe. Otherwise a string-literal key (whose quotes
    // `format_canonical` drops when the name is identifier-safe) with an explicit
    // value.
    let shorthand = is_shorthand_id && key_name == raw_name && is_js_identifier(&key_name);
    let (key, key_span) = if shorthand {
        let id = env.b.ident(&key_name);
        let span = id.span;
        (Expression::Identifier(id), span)
    } else {
        let key = env.b.string_literal_expr(&key_name);
        let span = key.span();
        (key, span)
    };

    Ok(Property {
        key,
        value,
        kind: PropertyKind::Init,
        shorthand,
        computed: false,
        method: false,
        span: key_span,
    })
}

/// Build the directives (2nd) argument of `$.attr_style`: a plain object
/// `{ normal… }`, or — when any directive carries `|important` — a 2-element
/// array `[ { normal… }, { important… } ]` (the normal object is `{}` when all are
/// important). Source order is preserved within each group.
fn build_style_directives_arg<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    style_directives: &[&'arena StyleDirective<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut normal: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    let mut important: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for directive in style_directives {
        validate_style_modifiers(directive)?;
        let property = ObjectProperty::Property(build_style_property(env, directive)?);
        if directive.modifiers.contains(&"important") {
            important.push(property);
        } else {
            normal.push(property);
        }
    }
    // An object span is only the printer's newline-scan region for the expansion
    // heuristic — both minted `{…}` regions are appendix-only and newline-free, so
    // each collapses when it fits (same rationale as `build_props_object`). The
    // properties render from their own key/value spans, so the normal span
    // enclosing the (source-order-interleaved) important property text is harmless.
    let cbrace = env.b.mint("}").end;
    let normal_obj = Expression::ObjectExpression(ObjectExpression {
        properties: normal.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    });
    if important.is_empty() {
        return Ok(normal_obj);
    }
    // Any `|important` → the partitioned `[ {normal}, {important} ]` array.
    let normal_alloc = arena.alloc(normal_obj);
    let iobrace = env.b.mint("{").start;
    let icbrace = env.b.mint("}").end;
    let important_obj = Expression::ObjectExpression(ObjectExpression {
        properties: important.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(iobrace, icbrace),
    });
    let important_alloc = arena.alloc(important_obj);
    let lbracket = env.b.mint("[").start;
    let rbracket = env.b.mint("]").end;
    let mut elements: BumpVec<'arena, Option<Expression<'arena>>> = BumpVec::new_in(arena);
    elements.push(Some(normal_alloc.clone()));
    elements.push(Some(important_alloc.clone()));
    Ok(Expression::ArrayExpression(ArrayExpression {
        elements: elements.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(lbracket, rbracket),
    }))
}

/// Emit the fused `$.attr_style(base, { name: value, … })` call for a regular
/// element carrying `style:` directives (the oracle's `build_attr_style`,
/// `shared/element.js`). `style_attr` is the authored `style` attribute when
/// present (its value is the base), else `None` (the oracle's synthetic empty
/// `''`). Unlike `$.attr_class`, there is no css-hash argument (style is never
/// scoped) and the directives argument may be an important-partitioned array.
pub(crate) fn emit_style_directives<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    style_attr: Option<&'arena Attribute<'arena>>,
    style_directives: &[&'arena StyleDirective<'arena>],
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // The synthetic `$.attr_style` call interleaves minted (appendix) and borrowed
    // (host) argument spans; with carried script comments their windows would
    // sweep — refuse, matching the dynamic-attribute path.
    if env.has_comments {
        return Err(unsupported(Refusal::CommentsAlongsideExprAttributes));
    }

    let base = build_style_base(env, style_attr)?;
    let directives = build_style_directives_arg(env, style_directives)?;

    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    args.push(base);
    args.push(directives);
    let call = env.b.member_call("$", "attr_style", args.into_bump_slice());
    out.push_expr(call);
    Ok(())
}

/// Collapse `[ \t\n\r\f]+` runs to one space without trimming (the mixed-value
/// `class`/`style` chunk rule).
fn collapse_runs_no_trim(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c') {
            in_ws = true;
        } else {
            if in_ws {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    if in_ws {
        out.push(' ');
    }
    out
}
