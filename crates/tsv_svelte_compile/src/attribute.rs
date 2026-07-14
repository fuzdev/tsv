//! Attribute emission: static inline values, dynamic `$.attr`/`$.attr_class`/
//! `$.attr_style` calls, and mixed text+expression attribute templates.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::InfallibleResolve;
use tsv_svelte::ast::internal::{Attribute, AttributeValue};
use tsv_ts::ast::internal::{Expression, LiteralValue};

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
fn is_load_error_element(name: &str) -> bool {
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
