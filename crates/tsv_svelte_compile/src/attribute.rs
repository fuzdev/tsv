//! Attribute emission: static inline values, dynamic `$.attr`/`$.attr_class`/
//! `$.attr_style` calls, and mixed text+expression attribute templates.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{InfallibleResolve, Span};
use tsv_svelte::ast::internal::{
    Attribute, AttributeNode, AttributeValue, BindDirective, ClassDirective, Element,
    StyleDirective, StyleDirectiveValue,
};
use tsv_ts::ast::internal::{
    ArrayExpression, BinaryOperator, Expression, LiteralValue, ObjectExpression, ObjectProperty,
    Property, PropertyKind,
};

use crate::analyze::{evaluate, stringify_value};
use crate::build::escape_template_text;
use crate::css_scope::SCOPE_HASH_CLASS;
use crate::fragment::{BodyBuilder, escape_html_attr, guard_dropped, wrap_value_expr};
use crate::namespace::{Namespace, element_is_mathml, element_is_svg};
use crate::script_rewrite::plain_identifier_name;
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

/// The `bind:` names Svelte's `binding_properties` marks `omit_in_ssr` — media,
/// dimension, window, and readback bindings (`bindings.js`) that produce **no** SSR
/// output. The oracle's server `BindDirective` handling skips them with an early
/// `continue`, before it visits the target (`shared/element.js`). `this` is
/// `omit_in_ssr` too but is handled separately (it carries `{get, set}`/lvalue
/// validation and skips on both the inline and spread paths), so it is excluded
/// here. Membership is by exact (case-sensitive) name, matching
/// `binding_properties[attribute.name]`.
const OMIT_IN_SSR_BINDS: &[&str] = &[
    "currentTime",
    "duration",
    "paused",
    "buffered",
    "seekable",
    "played",
    "volume",
    "muted",
    "playbackRate",
    "seeking",
    "ended",
    "readyState",
    "videoHeight",
    "videoWidth",
    "naturalWidth",
    "naturalHeight",
    "activeElement",
    "fullscreenElement",
    "pointerLockElement",
    "visibilityState",
    "innerWidth",
    "innerHeight",
    "outerWidth",
    "outerHeight",
    "scrollX",
    "scrollY",
    "online",
    "devicePixelRatio",
    "clientWidth",
    "clientHeight",
    "offsetWidth",
    "offsetHeight",
    "contentRect",
    "contentBoxSize",
    "borderBoxSize",
    "devicePixelContentBoxSize",
    "indeterminate",
    "files",
];

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
    element_scoped: bool,
    namespace: Namespace,
) -> Result<(), CompileError> {
    // The oracle lowercases attribute names outside foreign namespaces
    // (`get_attribute_name`, server `element.js`) — at EMISSION only. The
    // event-handler decision below tests the RAW authored name, so both are kept;
    // `name` (lowercased) drives the `value`/`class`/`style` special-key checks
    // (those keys are lowercase by definition), while `emit_name` is what actually
    // reaches the output — case-preserved on an svg/mathml element (`viewBox`,
    // `preserveAspectRatio`, …).
    let raw_name = env
        .b
        .interner
        .borrow()
        .resolve_infallible(attr.name)
        .to_string();
    let name = raw_name.to_ascii_lowercase();
    let emit_name: &str = if element_is_foreign(element_name, namespace) {
        &raw_name
    } else {
        &name
    };

    // `value` on <textarea> becomes child content, on <select> it is omitted
    // with select_value bookkeeping — neither shape is implemented.
    if name == "value" && (element_name == "textarea" || element_name == "select") {
        return Err(unsupported(Refusal::ValueAttribute {
            name: element_name.to_string(),
        }));
    }

    let Some(values) = attr.value else {
        // Boolean attribute: the oracle emits `name=""`.
        out.push_text(&escape_template_text(&format!(" {emit_name}=\"\"")));
        return Ok(());
    };

    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            let value = if name == "class" || name == "style" {
                collapse_attr_whitespace(&decoded)
            } else {
                decoded.into_owned()
            };
            if name == "class" {
                // CSS scope: a scoped element folds the hash into its `class`
                // value (`(escaped + ' ' + hash).trim()`, the oracle's order —
                // escape, then append the safe hash), so `class=""` on a scoped
                // element becomes `class="svelte-tsvhash"`. A `class` that ends up
                // empty (unscoped, and blank/whitespace-only) is dropped entirely
                // (oracle-probed: `class=""`/`class="  "` emit no attribute) — but
                // a bare boolean `class` keeps `class=""` (handled above), and
                // empty `style`/`id` stay.
                let escaped = escape_html_attr(&value);
                let text = if element_scoped {
                    format!("{escaped} {SCOPE_HASH_CLASS}").trim().to_string()
                } else {
                    escaped
                };
                if text.is_empty() {
                    return Ok(());
                }
                out.push_text(&escape_template_text(&format!(" class=\"{text}\"")));
                return Ok(());
            }
            out.push_text(&escape_template_text(&format!(
                " {emit_name}=\"{}\"",
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
            emit_dynamic_attribute(env, emit_name, &tag.expression, quoted, out)
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
            emit_mixed_attribute(env, emit_name, values, out)
        }
    }
}

/// The oracle's `get_attribute_name` namespace test: an svg/mathml element
/// preserves its attribute-name case; every other element lowercases. `inherited`
/// is the namespace the element sits in — the ancestor signal for the svg
/// `<a>`/`<title>` rule (`namespace::element_is_svg`).
fn element_is_foreign(element_name: &str, inherited: Namespace) -> bool {
    element_is_svg(element_name, inherited) || element_is_mathml(element_name)
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
pub(crate) fn collapse_attr_whitespace(decoded: &str) -> String {
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

/// The value of a mixed text+expression attribute, as the oracle's
/// `build_attribute_value` produces it in its multi-chunk path.
enum MixedAttrValue<'arena> {
    /// Every part folded statically — the raw concatenated value (un-HTML-escaped;
    /// each caller escapes as its own target requires).
    Folded(String),
    /// A template literal `` `t${$.stringify(a)}u` `` (interpolations omitted when
    /// the evaluator proves a defined string).
    Template(Expression<'arena>),
}

/// Build a mixed text+expression attribute value — the fold-or-template logic
/// shared by the element static-attribute path ([`emit_mixed_attribute`]) and the
/// spread object-builder ([`build_attribute_value_expr`]). `trim_whitespace`
/// collapses `class`/`style` runs (no per-chunk trim, matching the oracle's
/// template path). Returns [`MixedAttrValue::Folded`] when every part folds — each
/// caller decides how to escape/emit it — else the assembled template literal.
fn build_mixed_attr_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    trim_whitespace: bool,
    values: &'arena [AttributeValue<'arena>],
) -> Result<MixedAttrValue<'arena>, CompileError> {
    let mut texts: Vec<String> = vec![String::new()];
    // The unescaped folded value, in parallel — consumed only when every part
    // folds statically.
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
        return Ok(MixedAttrValue::Folded(raw));
    }
    Ok(MixedAttrValue::Template(
        env.b.template_literal(&texts, exprs.into_bump_slice()),
    ))
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
    if (name == "class" || name == "style") && env.scope.is_some() {
        return Err(unsupported(Refusal::InterpolatedAttrOnStyled {
            name: name.to_string(),
        }));
    }
    let trim_whitespace = name == "class" || name == "style";

    let template = match build_mixed_attr_value(env, trim_whitespace, values)? {
        MixedAttrValue::Folded(raw) => {
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
        MixedAttrValue::Template(template) => template,
    };
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

/// Build an attribute's value expression the way the oracle's
/// `build_attribute_value` does for a **non-component** element (`is_component`
/// false) — the value shape a spread element's `$.attributes({…})` object property
/// carries. Unlike the static-attribute emitters this returns the bare
/// `Expression`, never pushing an `$.attr(…)` call or inlining HTML:
///
/// - a boolean attribute (`value: None`) → `true`;
/// - a single static text value → the collapsed/trimmed (class/style) then
///   **HTML-escaped** (`[&"<]`) string literal;
/// - a single expression value → the erased + guarded + derived-rewritten
///   expression, wrapped in `$.clsx(…)` for a `class` that needs it
///   (`class_needs_clsx`), **no fold** (the single-chunk path doesn't evaluate);
/// - a mixed text+expression value → a fully-folded string literal (the raw
///   concatenation, **not** HTML-escaped — the runtime escapes the object value)
///   or a `$.stringify` template literal.
pub(crate) fn build_attribute_value_expr<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    value: Option<&'arena [AttributeValue<'arena>]>,
) -> Result<Expression<'arena>, CompileError> {
    let Some(values) = value else {
        // A boolean attribute: `build_attribute_value(true)` → `true`.
        return Ok(env.b.true_literal());
    };
    let trim_whitespace = name == "class" || name == "style";
    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            let collapsed = if trim_whitespace {
                collapse_attr_whitespace(&decoded)
            } else {
                decoded.into_owned()
            };
            Ok(env.b.string_literal_expr(&escape_html_attr(&collapsed)))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // The template borrow point. Both the `class` `$.clsx` gate and the
            // wrap read the ERASED node (an `x as T` value is the assignable `x`).
            let quoted = preceded_by_quote(env.source, tag.span.start);
            let expr = env.erase(&tag.expression)?;
            let wrapped = wrap_value_expr(env, expr)?[0].clone();
            // A `class` single-expression wraps in `$.clsx` per the oracle's
            // `needs_clsx` rule (its `has_spread` branch pre-wraps the expression);
            // every other name (and a non-clsx class) passes the bare value.
            if name == "class" && class_needs_clsx(expr, quoted) {
                let wrapped_alloc = env.b.arena.alloc(wrapped);
                Ok(env
                    .b
                    .member_call("$", "clsx", std::slice::from_ref(wrapped_alloc)))
            } else {
                Ok(wrapped)
            }
        }
        _ => Ok(
            match build_mixed_attr_value(env, trim_whitespace, values)? {
                // The object value is a JS string literal the runtime escapes — no
                // HTML escaping here (unlike the static-attribute full-fold path).
                MixedAttrValue::Folded(raw) => env.b.string_literal_expr(&raw),
                MixedAttrValue::Template(template) => template,
            },
        ),
    }
}

/// Build one `key: value` object property for an element `{...spread}`'s
/// `$.attributes({…})` object from a plain attribute (the oracle's
/// `build_spread_object`). Returns `None` for a **dropped** attribute — a
/// single-expression event handler (still guarded), and `defaultValue`/
/// `defaultChecked` (which don't exist as attributes). The key is lowercased
/// (matching `get_attribute_name`) and emitted as a bare identifier when it is
/// identifier-safe, else a quoted string literal; `shorthand` is set when the key
/// is an identifier and the value is the same-named identifier (`{ hidden }`, the
/// oracle's `b.prop` collapse).
pub(crate) fn build_spread_object_property<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attr: &'arena Attribute<'arena>,
    element_name: &str,
    inherited: Namespace,
) -> Result<Option<Property<'arena>>, CompileError> {
    let raw_name = env
        .b
        .interner
        .borrow()
        .resolve_infallible(attr.name)
        .to_string();
    // `defaultValue`/`defaultChecked` are properties, not attributes — the oracle
    // omits them from the object (case-sensitive raw-name test).
    if raw_name == "defaultValue" || raw_name == "defaultChecked" {
        return Ok(None);
    }
    let name = raw_name.to_ascii_lowercase();
    // A `value` on `<textarea>` becomes child content in the oracle (a divergent
    // shape); refuse. (`<select>` refuses the whole spread at the element level.)
    if name == "value" && element_name == "textarea" {
        return Err(unsupported(Refusal::ValueAttribute {
            name: element_name.to_string(),
        }));
    }
    // Event-handler dispatch, mirroring the oracle. A single-expression `on*`
    // attribute is `is_event_attribute` → dropped from the object but still
    // guarded (a stray rune inside it is an analysis-phase error). Any other `on*`
    // value form with name length > 2 is `attribute_invalid_event_handler` (an
    // oracle analysis error) → refuse.
    if raw_name.starts_with("on") {
        if let Some([AttributeValue::ExpressionTag(tag)]) = attr.value {
            guard_dropped(env, &tag.expression)?;
            return Ok(None);
        }
        if raw_name.len() > 2 {
            return Err(unsupported(Refusal::EventAttribute { name }));
        }
        // A bare `on` (length 2) with a non-expression value is a normal attribute.
    }

    let value = build_attribute_value_expr(env, &name, attr.value)?;
    // The object key is the emitted attribute name — case-preserved on an
    // svg/mathml element (`get_attribute_name`), lowercased otherwise. The value
    // builder above still keys the `class` → `$.clsx` decision on the lowercased
    // `name` (`class` is lowercase by definition).
    let emit_name: &str = if element_is_foreign(element_name, inherited) {
        &raw_name
    } else {
        &name
    };
    let key_is_ident = is_js_identifier(emit_name);
    let key = if key_is_ident {
        Expression::Identifier(env.b.ident(emit_name))
    } else {
        env.b.string_literal_expr(emit_name)
    };
    let key_span = key.span();
    // Object shorthand `{ hidden }`: an identifier key whose value is the plain
    // identifier of the same (emitted) name (`hidden={hidden}`, `viewBox={viewBox}`).
    let shorthand = key_is_ident
        && matches!(&value, Expression::Identifier(id)
            if plain_identifier_name(id, env.source).as_deref() == Some(emit_name));
    Ok(Some(Property {
        key,
        value,
        kind: PropertyKind::Init,
        shorthand,
        computed: false,
        method: false,
        span: key_span,
    }))
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

/// Build the `classes` (3rd) argument of a spread element's `$.attributes(…)` call
/// from its `class:` directives (the oracle's `prepare_element_spread`, which uses
/// `b.init(directive.name, …)`). Unlike the non-spread `$.attr_class` object
/// ([`build_class_directives_object`], string-literal keys), this uses an
/// **identifier key** (a quoted string literal only when the name isn't
/// identifier-safe, e.g. `class:foo-bar` → `{ 'foo-bar': x }`) with the
/// **object-shorthand** collapse the oracle's `b.init` applies: `class:active`
/// (shorthand) and `class:active={active}` both → `{ active }`, `class:active={x}` →
/// `{ active: x }`. Class names are **case-sensitive** (never lowercased). The
/// same-named-identifier fork reads the **raw** directive expression, mirroring the
/// oracle's `directive.expression.type === 'Identifier' && … === directive.name`
/// (a `class:active={active as T}` is not shorthand — the raw node is a
/// `TSAsExpression` — and its value is the erased/guarded expression).
pub(crate) fn build_spread_class_object<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    class_directives: &[&'arena ClassDirective<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for directive in class_directives {
        let name = directive.name_span.extract(env.source).to_string();
        // The oracle's same-named-identifier shorthand fork, read on the RAW node:
        // the value is the bare `b.id(directive.name)` (no transform / derived
        // rewrite), exactly like a `style:` shorthand.
        let same_named = matches!(&directive.expression, Expression::Identifier(id)
            if plain_identifier_name(id, env.source).as_deref() == Some(name.as_str()));
        let (value, is_shorthand_id) = if same_named {
            (Expression::Identifier(env.b.ident(&name)), true)
        } else {
            // The template borrow point: erase once, then guard + rewrite a bare
            // derived read to `d()`.
            let expr = env.erase(&directive.expression)?;
            (wrap_value_expr(env, expr)?[0].clone(), false)
        };
        let key_is_ident = is_js_identifier(&name);
        let shorthand = is_shorthand_id && key_is_ident;
        let (key, key_span) = if key_is_ident {
            let id = env.b.ident(&name);
            let span = id.span;
            (Expression::Identifier(id), span)
        } else {
            let key = env.b.string_literal_expr(&name);
            let span = key.span();
            (key, span)
        };
        properties.push(ObjectProperty::Property(Property {
            key,
            value,
            kind: PropertyKind::Init,
            shorthand,
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

/// Build the `styles` (4th) argument of a spread element's `$.attributes(…)` call
/// from its `style:` directives (the oracle's `prepare_element_spread`). A **flat**
/// object `{ prop: value, … }` in source order — **no** `|important` partitioning
/// (the CRITICAL divergence from the non-spread `$.attr_style` path, which builds
/// the `[ {normal}, {important} ]` array): in spread mode the oracle's
/// `build_attribute_value(directive.value, …, true)` folds every directive into one
/// object regardless of modifier. `|important` is still **validated** (only a single
/// `|important` is legal, else refuse) but does not partition. Reuses
/// [`build_style_property`] for the per-property build (key lowercasing / quoting,
/// shorthand).
pub(crate) fn build_spread_style_object<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    style_directives: &[&'arena StyleDirective<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for directive in style_directives {
        validate_style_modifiers(directive)?;
        properties.push(ObjectProperty::Property(build_style_property(
            env, directive,
        )?));
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
    element_scoped: bool,
) -> Result<(), CompileError> {
    let base = build_class_base(env, class_attr)?;
    let base_is_string = matches!(base, ClassBase::StringLiteral(_));

    // CSS scope: whether any scoped compound matches this element (decided up front
    // by `element_scope`, including type/id/attribute selectors — not only a class
    // token or `class:` name). A string base folds the hash into its text; any
    // other base carries it in the 2nd argument.
    let scoped = element_scoped;

    // The base expression, folding the scope hash into a string-literal base.
    let base_expr = match base {
        ClassBase::StringLiteral(s) => {
            // HTML-escape the static base (`escape_html(data, true)`), after token
            // matching, matching the oracle.
            let escaped = escape_html_attr(&s);
            let text = if scoped {
                format!("{escaped} {SCOPE_HASH_CLASS}").trim().to_string()
            } else {
                escaped
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
    let base = build_style_base(env, style_attr)?;
    let directives = build_style_directives_arg(env, style_directives)?;

    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    args.push(base);
    args.push(directives);
    let call = env.b.member_call("$", "attr_style", args.into_bump_slice());
    out.push_expr(call);
    Ok(())
}

/// Refuse a `bind:` directive with the collapsing `bind: directive {name}` bucket.
fn refuse_bind<T>(name: &str) -> Result<T, CompileError> {
    Err(unsupported(Refusal::BindDirective {
        name: name.to_string(),
    }))
}

/// The `type` attribute's shape on a `bind:`-bearing `<input>` — the split the
/// oracle keys on (`is_text_attribute`): absent, a bare boolean `type`, a static
/// text value (`checkbox`/`file`/…), or a dynamic/mixed expression value.
enum InputType {
    None,
    Bare,
    Static(String),
    Dynamic,
}

/// Classify the `<input>`'s `type` attribute (case-sensitive `type`, mirroring the
/// oracle's `attr.name === 'type'` — a `TYPE` would not be found).
fn classify_input_type(env: &EmitEnv<'_, '_>, element: &Element<'_>) -> InputType {
    for attr_node in element.attributes {
        let AttributeNode::Attribute(attr) = attr_node else {
            continue;
        };
        let is_type = {
            let interner = env.b.interner.borrow();
            interner.resolve_infallible(attr.name) == "type"
        };
        if !is_type {
            continue;
        }
        return match attr.value {
            None => InputType::Bare,
            Some([AttributeValue::Text(text)]) => {
                InputType::Static(text.data(env.source).into_owned())
            }
            Some(_) => InputType::Dynamic,
        };
    }
    InputType::None
}

/// The root identifier name of a bind target that is an `Identifier` or a member
/// chain rooted at one (`v`, `obj.x`, `a[i].b`); `None` for any other shape (a
/// call, literal, binary — not an lvalue root). Mirrors the oracle's `object()`.
///
/// An **optional** link anywhere in the chain (`o?.el`, `o?.a.b`) yields `None`:
/// acorn wraps a chain containing one in a `ChainExpression`, so the oracle's
/// `bind_invalid_expression` test — which admits only `Identifier` /
/// `MemberExpression` / a `{get, set}` `SequenceExpression` — rejects it. tsv's
/// internal AST has no chain wrapper (the flag rides each member), so the refusal
/// is expressed by refusing any optional link; the recursion propagates it from a
/// deeper link up.
fn bind_target_root(expr: &Expression<'_>, source: &str) -> Option<String> {
    match expr {
        Expression::Identifier(id) => plain_identifier_name(id, source),
        Expression::MemberExpression(member) if !member.optional => {
            bind_target_root(member.object, source)
        }
        _ => None,
    }
}

/// A `{get, set}` bind target: a `SequenceExpression` of exactly two expressions
/// (`() => x, (v) => x = v`), the third valid bind form the oracle accepts beside
/// an Identifier / member chain (`SequenceExpression` + `expressions.length === 2`
/// in its `BindDirective` analysis). Recognized only so `bind:this` can OMIT it at
/// parity — the value/checked/group arms still refuse it (no attr value to emit).
fn is_get_set_pair(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::SequenceExpression(seq) if seq.expressions.len() == 2)
}

/// The bind target's root name, but only when that root is something the oracle
/// will let a `bind:` WRITE to. `None` when the expression is not an
/// `Identifier`/member-chain lvalue at all, **or** when its root is one the oracle
/// rejects with `constant_binding`.
///
/// The oracle keys that rejection on the DECLARATION KEYWORD, not on inferred
/// reassignability (`phases/2-analyze/visitors/shared/utils.js:82-105`): an
/// `import` local, or a `const` binding whose kind is not `each`. So a
/// `const c = $state(0)` is refused as a bind target even though it is reactive —
/// which is exactly why `env.unassignable_names` is its own set rather than a
/// filter on `state_names` (that set also drives component-dynamic classification,
/// where the same binding IS dynamic).
///
/// This is the single seam every `bind:` target check shares — the `$state`-gated
/// ones ([`emitted_bind_target`], [`validate_inert_bind_target`]) and the
/// `bind:this` branches, which take any lvalue with no `$state` gate and so would
/// otherwise accept a `const`/import target unchallenged.
///
/// ⚠️ The set it consults ([`crate::transform_server::EmitEnv::unassignable_names`])
/// is keyed on **top-level script statements only**, so the rule is closed for a
/// top-level `const`/import and OPEN for a TEMPLATE-scoped one. A `{@const}` name
/// (`scope.js:1099`/`1111`) and a `{:then}`/`{:catch}` value (`:1310`/`:1324`) are
/// declared `declaration_kind: 'const'` with kind `'template'` — not `'each'` — so
/// the oracle's `validate_no_const_assignment` rejects a bind to one exactly as it
/// rejects a top-level `const`, while tsv compiles it. Closing that half needs tsv
/// to model template scopes; see `../../docs/checklist_svelte_compiler.md`.
fn reassignable_bind_target_root(env: &EmitEnv<'_, '_>, expr: &Expression<'_>) -> Option<String> {
    let name = bind_target_root(expr, env.source)?;
    // The rejection applies to a BARE identifier target only. The oracle's
    // `validate_no_const_assignment` recurses through array/object PATTERNS and
    // tests `argument.type === 'Identifier'`; a `MemberExpression` argument matches
    // no branch and falls through with no error. That is not an oversight — writing
    // through a const binding (`const o = $state({v: ''})` + `bind:value={o.v}`)
    // MUTATES the object and never rebinds the name, so only rebinding the name
    // itself is refused. Walking to the member-chain root here instead would
    // over-refuse a common shape.
    if matches!(expr, Expression::Identifier(_)) && env.unassignable_names.contains(&name) {
        return None;
    }
    Some(name)
}

fn emitted_bind_target<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
    bind_name: &str,
) -> Result<Expression<'arena>, CompileError> {
    // The template borrow point: erase the bind target once, then gate its erased
    // shape (an `x as T` target is `x`, an assignable lvalue).
    let expr = env.erase(&directive.expression)?;
    match reassignable_bind_target_root(env, expr) {
        Some(name) if env.state_names.contains(&name) => {}
        _ => return refuse_bind(bind_name),
    }
    Ok(wrap_value_expr(env, expr)?[0].clone())
}

/// Validate the TARGET of a `bind:` on an SSR-inert special element
/// (`<svelte:window>`/`<svelte:body>`/`<svelte:document>`) — **validation only**
/// (the bind is dropped from SSR output, no `$.attr` is emitted; the bind NAME is
/// already whitelisted by the caller). Reproduces the SAME reassignable-lvalue rule
/// regular elements enforce, reusing the shared primitives so the two never drift:
///
/// - `bind:this` accepts any reassignable `Identifier`/member-chain lvalue or a
///   `{get, set}` pair, no `$state` gate — exactly [`resolve_bind_directive`]'s
///   `this` branch.
/// - every other (whitelisted-name) bind requires a `$state`-rooted
///   `Identifier`/member lvalue — the SAFE gate [`emitted_bind_target`] applies,
///   over-refusing a prop / plain reassignable `let` / each binding (which the
///   oracle accepts) exactly as the regular path does. A non-lvalue (call / literal
///   / logical) and a plain (non-`$state`) `const` or undefined identifier never
///   reach `state_names`, so both refuse — matching the oracle's own
///   `bind_invalid_expression` / `constant_binding` / `bind_invalid_value`.
///
/// The oracle's `constant_binding` rule is enforced on both paths at once by the
/// shared [`reassignable_bind_target_root`], so a **top-level** `const`-declared or
/// imported target refuses here exactly as it does on the regular-element path.
///
/// ⚠️ That closes the top-level half only. A TEMPLATE-scoped const target — a
/// `{@const}` name, or a `{:then}`/`{:catch}` value — is just as `const`-declared to
/// the oracle (`constant_binding`) and still compiles here, because
/// `unassignable_names` is built from top-level script statements alone. Two open
/// over-acceptances, both pre-existing and shared with the regular-element path; see
/// [`reassignable_bind_target_root`] and `../../docs/checklist_svelte_compiler.md`.
///
/// A failure refuses with the collapsing `Refusal::BindDirective { name }` bucket.
pub(crate) fn validate_inert_bind_target<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
) -> Result<(), CompileError> {
    let bind_name = directive.name_span.extract(env.source).to_string();
    // Erase a TypeScript wrapper (`bind:this={x as T}`) first, exactly as the
    // regular fork does before gating the target's shape.
    let expr = env.erase(&directive.expression)?;
    if bind_name == "this" {
        if reassignable_bind_target_root(env, expr).is_some() || is_get_set_pair(expr) {
            return Ok(());
        }
        return refuse_bind(&bind_name);
    }
    match reassignable_bind_target_root(env, expr) {
        Some(name) if env.state_names.contains(&name) => Ok(()),
        _ => refuse_bind(&bind_name),
    }
}

/// Validate a `bind:` on a `<svelte:element>`. The dynamic tag carries no static
/// element identity, so the input-centric core-kind logic (`bind:value`/`checked`/
/// `group`) never applies — the oracle rejects those as `bind_invalid_target`. Only
/// `bind:this` is handled: it OMITS from SSR output (nothing to emit), exactly as on
/// a regular element — any lvalue (`Identifier`/member chain) or `{get, set}` pair
/// target, no `$state` gate (a `TS` wrapper is erased first). Every other bind
/// refuses. Deferred as a **safe over-refusal** (the current alternative is refusing
/// the whole `<svelte:element>`): `bind:focused` (which the oracle emits as
/// `$.attr('focused', …)`), the `omit_in_ssr` dimension family (which it drops),
/// and the `bind:innerHTML`-family content-editable binds.
///
/// Returns `Ok(())` for a valid `bind:this` — the caller emits nothing (inline) or
/// contributes no object property (spread).
pub(crate) fn validate_dynamic_bind<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
) -> Result<(), CompileError> {
    let bind_name = directive.name_span.extract(env.source).to_string();
    if bind_name == "this" {
        let expr = env.erase(&directive.expression)?;
        if reassignable_bind_target_root(env, expr).is_some() || is_get_set_pair(expr) {
            return Ok(());
        }
        return refuse_bind(&bind_name);
    }
    refuse_bind(&bind_name)
}

/// Build and push `$.attr(name, value[, true])` for a synthesized bind attribute.
fn push_bind_attr<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    value: Expression<'arena>,
    boolean: bool,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    args.push(env.b.string_literal_expr(name));
    args.push(value);
    if boolean {
        args.push(env.b.true_literal());
    }
    let call = env.b.member_call("$", "attr", args.into_bump_slice());
    out.push_expr(call);
    Ok(())
}

/// The companion `value` attribute (case-sensitive `value`, the oracle's
/// `attr.name === 'value'`) a `bind:group` reads for its synthesized `checked`.
fn find_value_attribute<'arena>(
    env: &EmitEnv<'_, '_>,
    element: &'arena Element<'arena>,
) -> Option<&'arena Attribute<'arena>> {
    element.attributes.iter().find_map(|attr_node| {
        let AttributeNode::Attribute(attr) = attr_node else {
            return None;
        };
        let interner = env.b.interner.borrow();
        (interner.resolve_infallible(attr.name) == "value").then_some(attr)
    })
}

/// Build the companion `value` attribute's value for a `bind:group` synthesis —
/// the oracle's `build_attribute_value(value_attribute.value, …)` (no trim, not
/// component mode): a static text → an attr-escaped string literal; a single
/// `{expr}` → the erased/wrapped expression; a bare `value` → `true`; a mixed
/// value → refuse this group for now.
fn build_companion_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    value_attr: &'arena Attribute<'arena>,
) -> Result<Expression<'arena>, CompileError> {
    let Some(values) = value_attr.value else {
        return Ok(env.b.true_literal());
    };
    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            Ok(env.b.string_literal_expr(&escape_html_attr(&decoded)))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            let expr = env.erase(&tag.expression)?;
            Ok(wrap_value_expr(env, expr)?[0].clone())
        }
        _ => refuse_bind("group"),
    }
}

/// The SSR emission a `bind:` **core kind** reduces to, independent of whether the
/// element carries a `{...spread}` — the fork the inline (`emit_bind_directive`)
/// and spread (`build_bind_object_property`) callers share so the two never drift.
enum BindEmission<'arena> {
    /// The synthesized attribute `name = value` (`bind:value`/`bind:checked`, or
    /// `bind:group`'s synthesized `checked`). `boolean` picks the inline
    /// `$.attr(name, value, true)` boolean form; the spread object drops it (the
    /// object property is just `name: value`).
    Attr {
        name: &'static str,
        value: Expression<'arena>,
        boolean: bool,
    },
    /// Emit nothing on BOTH paths: a valid `bind:this`, or a `bind:group` with no
    /// companion `value` attribute (the oracle silently drops it).
    Omit,
    /// An `omit_in_ssr` bind ([`OMIT_IN_SSR_BINDS`]): the oracle drops it from SSR
    /// entirely (its early `continue`). **Both** paths **refuse** it (a safe
    /// over-refusal — the oracle rejects the ill-formed shapes that reach here, and
    /// tsv declines to reproduce the drop for the well-formed ones rather than
    /// silently emit nothing; well-formed `omit_in_ssr`+spread parity is deferred).
    OmitInSsr,
}

/// Resolve a `bind:` **core kind** on a regular `<input>`/element to its SSR
/// emission (the oracle's server `BindDirective` handling in `shared/element.js`),
/// applying every validity gate. Shared by the inline and spread callers so a bind
/// is validated identically whether or not the element also carries a spread:
///
/// - **`bind:this`** → [`BindEmission::Omit`] (no output). Valid on any variable /
///   any element, no `$state` gate, but the (erased) target must be one of the
///   oracle's three accepted forms — an Identifier, a member chain, or a
///   `{get, set}` pair; any other shape is `bind_invalid_expression` (refuse).
/// - **`omit_in_ssr` binds** → [`BindEmission::OmitInSsr`] (both callers refuse).
/// - **`bind:value`** on `<input>` → `Attr { "value", .. }`. A bare `type` is
///   `attribute_invalid_type` (refuse); a static `type="file"` is the files trap
///   the oracle silently drops (refuse rather than emit a divergent value); a
///   dynamic `type={x}` / static-non-file / no type is fine.
/// - **`bind:checked`** on `<input>` → `Attr { "checked", .., boolean }`; requires
///   a static `type="checkbox"` (else the oracle rejects).
/// - **`bind:group`** on `<input>` with a static `type` → a synthesized
///   `Attr { "checked", <synth>, .. }` where `<synth>` is `group.includes(<value>)`
///   for `type="checkbox"` else `group === <value>`, and `<value>` is the companion
///   `value` attribute's value. No companion `value` → [`BindEmission::Omit`]. The
///   companion `value` still emits normally at its own source slot — it is only
///   READ here.
///
/// Everything else refuses with the collapsing [`Refusal::BindDirective`] bucket: a
/// bind on a non-`<input>` target, `value` on `<textarea>`/`<select>`, the
/// content-editable trio, `open`, `focused`, an invalid target/type, or a bind
/// expression that isn't a `$state`-rooted lvalue.
fn resolve_bind_directive<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
    element: &'arena Element<'arena>,
    element_name: &str,
) -> Result<BindEmission<'arena>, CompileError> {
    let bind_name = directive.name_span.extract(env.source).to_string();

    // `bind:this` (itself an `omit_in_ssr` bind, but with the `{get, set}`/lvalue
    // validation and a skip — not a refusal — on the inline path too). Erase a
    // `bind:this={x as T}` TS wrapper first.
    if bind_name == "this" {
        let expr = env.erase(&directive.expression)?;
        if reassignable_bind_target_root(env, expr).is_some() || is_get_set_pair(expr) {
            return Ok(BindEmission::Omit);
        }
        return refuse_bind(&bind_name);
    }

    // Every other `omit_in_ssr` bind: the oracle skips it before visiting the
    // target. `value`/`checked`/`group`/`open`/`focused`/the content-editable trio
    // are NOT `omit_in_ssr`, so they fall through to the dispatch below.
    if OMIT_IN_SSR_BINDS.contains(&bind_name.as_str()) {
        return Ok(BindEmission::OmitInSsr);
    }

    match (bind_name.as_str(), element_name) {
        ("value", "input") => {
            // A BARE `type` rejects (`attribute_invalid_type`); a static
            // `type="file"` is dropped by the oracle (refuse rather than diverge);
            // dynamic / static-non-file / no type is fine for `value`.
            match classify_input_type(env, element) {
                InputType::Bare => return refuse_bind(&bind_name),
                InputType::Static(t) if t == "file" => return refuse_bind(&bind_name),
                _ => {}
            }
            let value = emitted_bind_target(env, directive, &bind_name)?;
            Ok(BindEmission::Attr {
                name: "value",
                value,
                boolean: false,
            })
        }
        ("checked", "input") => {
            // `bind:checked` requires a static `type="checkbox"` (a dynamic / bare /
            // missing / other type is an oracle error).
            match classify_input_type(env, element) {
                InputType::Static(t) if t == "checkbox" => {}
                _ => return refuse_bind(&bind_name),
            }
            let value = emitted_bind_target(env, directive, &bind_name)?;
            Ok(BindEmission::Attr {
                name: "checked",
                value,
                boolean: true,
            })
        }
        ("group", "input") => {
            // A static `type` is required (a dynamic / bare type is
            // `attribute_invalid_type`; tsv also over-refuses no type). checkbox →
            // `group.includes(value)`, any other static type → `group === value`.
            let is_checkbox = match classify_input_type(env, element) {
                InputType::Static(t) => t == "checkbox",
                InputType::None | InputType::Bare | InputType::Dynamic => {
                    return refuse_bind(&bind_name);
                }
            };
            // Validate the bind TARGET (`g`) even when the bind is later dropped —
            // the oracle rejects an invalid group expression in its analysis phase,
            // before it decides whether to emit.
            let group = emitted_bind_target(env, directive, &bind_name)?;
            // No companion `value` → the oracle silently drops the bind.
            let Some(value_attr) = find_value_attribute(env, element) else {
                return Ok(BindEmission::Omit);
            };
            let value = build_companion_value(env, value_attr)?;
            let arena = env.b.arena;
            let synth = if is_checkbox {
                let group_alloc = arena.alloc(group);
                let callee = env.b.member_prop(group_alloc, "includes");
                let callee_alloc = arena.alloc(callee);
                let args = std::slice::from_ref(arena.alloc(value));
                env.b.call_of(callee_alloc, args, false)
            } else {
                let group_alloc = arena.alloc(group);
                let value_alloc = arena.alloc(value);
                env.b
                    .binary(group_alloc, BinaryOperator::EqualsEqualsEquals, value_alloc)
            };
            Ok(BindEmission::Attr {
                name: "checked",
                value: synth,
                boolean: true,
            })
        }
        _ => refuse_bind(&bind_name),
    }
}

/// Emit a `bind:` directive on a regular `<input>`/element **inline** at its source
/// slot (the non-spread path) — a handled core kind ([`resolve_bind_directive`])
/// becomes `$.attr(name, value[, true])`; a `bind:this` / no-companion `bind:group`
/// emits nothing; an `omit_in_ssr` bind refuses (a safe over-refusal — the oracle
/// drops it, but the inline path declines rather than reproduce the drop; the spread
/// path skips it instead).
pub(crate) fn emit_bind_directive<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
    element: &'arena Element<'arena>,
    element_name: &str,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    match resolve_bind_directive(env, directive, element, element_name)? {
        BindEmission::Attr {
            name,
            value,
            boolean,
        } => push_bind_attr(env, name, value, boolean, out),
        BindEmission::Omit => Ok(()),
        BindEmission::OmitInSsr => refuse_bind(directive.name_span.extract(env.source)),
    }
}

/// Build the object property a `bind:` **core kind** contributes to a spread
/// element's `$.attributes({…})` object, at the bind's source position (the oracle
/// pre-processes each bind into a synthetic attribute inside `build_spread_object`).
/// Reuses [`resolve_bind_directive`] for every validity gate, then adapts:
///
/// - `Attr { name, value, .. }` → the object property `{ name: value }` (the boolean
///   flag is an `$.attr` concern and is dropped), with the object-shorthand collapse
///   (`bind:value={value}` → `{ value }`);
/// - `Omit` (a valid `bind:this`, a no-companion `bind:group`) → `None`;
/// - `OmitInSsr` → refuse, exactly as the inline path does (a safe over-refusal;
///   well-formed `omit_in_ssr`+spread parity is deferred to a future slice).
pub(crate) fn build_bind_object_property<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
    element: &'arena Element<'arena>,
    element_name: &str,
) -> Result<Option<Property<'arena>>, CompileError> {
    match resolve_bind_directive(env, directive, element, element_name)? {
        BindEmission::Attr {
            name,
            value,
            boolean: _,
        } => {
            // The synthesized bind name (`value`/`checked`) is always a valid
            // identifier key; `shorthand` collapses `{ value: value }` → `{ value }`.
            let key = env.b.ident(name);
            let key_span = key.span;
            let shorthand = matches!(&value, Expression::Identifier(id)
                if plain_identifier_name(id, env.source).as_deref() == Some(name));
            Ok(Some(Property {
                key: Expression::Identifier(key),
                value,
                kind: PropertyKind::Init,
                shorthand,
                computed: false,
                method: false,
                span: key_span,
            }))
        }
        // A valid `bind:this` / no-companion `bind:group` drops with no entry.
        BindEmission::Omit => Ok(None),
        // An `omit_in_ssr` bind refuses here too — consistent with the inline
        // path's `refuse_bind`, and the SAFE side (the oracle rejects these
        // shapes; tsv declines rather than silently drop them).
        BindEmission::OmitInSsr => refuse_bind(directive.name_span.extract(env.source)),
    }
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
