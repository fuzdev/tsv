//! Component invocation: the `Foo($$renderer, {…props})` call, its props object,
//! and its snippet/slot children.
//!
//! A **per-node emitter** in the emission pipeline, and a leaf: it has **zero**
//! outgoing edges to element or `<svelte:element>` emission. The only connection
//! is one dispatch from [`crate::element::emit_element`] on
//! `ElementKind::Component`, plus [`component_is_standalone_eligible`], which
//! [`crate::fragment`]'s `is_standalone` recomputation reads.
//!
//! **Single source of truth** for the component props shape — the plain object
//! literal vs `$.spread_props([…])` fork, the implicit `children` snippet prop for
//! default-slot content, and named `{#snippet}` children as named snippet props
//! with the `$$slots: { key: true, … }` marker beside them. The prop *order* is
//! the oracle's source order across all three kinds, so a second builder that
//! assembled any one of them separately would reorder the object.
//!
//! See [`crate::transform_server`] for the orchestration.

use std::collections::HashMap;

use tsv_lang::Span;
use tsv_svelte::ast::{
    Attribute, AttributeNode, AttributeValue, Element, ElementKind, FragmentNode, SpecialElement,
    SpecialElementKind,
};
use tsv_ts::ast::{
    ArrayExpression, BlockStatement, ExprId, Expression, ExpressionKind, ObjectExpression,
    ObjectProperty, Property, Statement,
};

use crate::analyze::BindingKind;
use crate::attribute::build_mixed_value_expr;
use crate::body_builder::BodyBuilder;
use crate::build::init_property;
use crate::fragment::emit_child_body;
use crate::namespace::{ChildNamespace, FragmentParent, Namespace};
use crate::script_decls::plain_identifier_name;
use crate::snippet_emit::build_snippet_function;
use crate::template_value::wrap_value_expr;
use crate::text_class::is_js_identifier;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// Whether a component is *dynamic* — the oracle's `metadata.dynamic`
/// (`2-analyze/visitors/Component.js`): `binding !== null && (binding.kind !==
/// 'normal' || name.includes('.'))`. A dynamic component compiles to an
/// `if (expr) {…}` truthiness guard with hydration anchors, not a plain call —
/// refused in this slice.
///
/// - A member component (`<Foo.Bar>`) is always dynamic.
/// - A block-local name (each item/index, `{:then}` value, `{@const}`) resolves
///   through an overlay to a non-`normal` binding → dynamic.
/// - A top-level `prop`/`$derived`/`$state` binding → dynamic. A plain
///   declaration/import (`normal`) or an unresolved global (`binding === null`)
///   is **not** dynamic.
fn component_dynamic(env: &EmitEnv<'_>, name: &str) -> bool {
    if name.contains('.') {
        return true;
    }
    if env
        .overlays
        .iter()
        .any(|overlay| overlay.contains_key(name))
    {
        return true;
    }
    match env.bindings.get(name) {
        None => false,
        Some(binding) => match binding.kind {
            BindingKind::Prop | BindingKind::Derived => true,
            BindingKind::Normal | BindingKind::Opaque => env.state_names.contains(name),
        },
    }
}

/// Whether a sole fragment child is a standalone-eligible component (the oracle's
/// `clean_nodes` `is_standalone`: a non-dynamic `Component` with no
/// `--custom-property` attribute — `hmr` is always off here). When true its call
/// reuses the enclosing block's anchor and emits no trailing `<!---->`.
pub(crate) fn component_is_standalone_eligible(env: &EmitEnv<'_>, element: &Element) -> bool {
    if element.kind != ElementKind::Component {
        return false;
    }
    let name = element.name(env.source).to_string();
    if component_dynamic(env, &name) {
        return false;
    }
    !env.sv
        .attributes(element.attributes)
        .iter()
        .any(|attr_node| {
            let AttributeNode::Attribute(attr) = attr_node else {
                return false;
            };
            attr.name(env.source).starts_with("--")
        })
}

/// The component-children analysis product: the synthetic props to append after
/// the attribute props, plus the snippet-prop function declarations that go into
/// the component's wrapping block.
struct ChildrenPlan {
    /// `function name($$renderer, …) { … }` declarations (source order), placed in
    /// the component's wrapping block so the snippet props can reference them.
    snippet_functions: Vec<Statement>,
    /// Snippet prop names (source order) — each emits a `{ name }` shorthand prop.
    snippet_props: Vec<String>,
    /// `$$slots` entries (slot keys, source order): snippet slots, then `default`.
    slot_keys: Vec<String>,
    /// The default-slot children (direct `{#snippet}` children filtered out — a
    /// subset no run names, so the nodes themselves) → the implicit
    /// `children: ($$renderer) => { … }` arrow, if any.
    default_children: Option<Vec<FragmentNode>>,
}

impl ChildrenPlan {
    /// Whether the plan contributes any synthetic props (a snippet prop, the
    /// `children` arrow, or `$$slots`) — `slot_keys` is non-empty exactly then.
    fn has_content(&self) -> bool {
        !self.slot_keys.is_empty()
    }
}

/// Plan a component's children: build the `{#snippet}` prop functions (in source
/// order) and the synthetic prop shape, refusing two cases — a `slot="…"` child
/// (a named slot: the permanent runes-only fence, not a deferral) and a `children`
/// prop alongside default children (the oracle's `$$slots.default` divergence, a
/// genuine deferred gap). A `{#snippet}` child named `children` keeps the
/// `children` prop name but a `default` slot key (the oracle's rename).
fn plan_component_children(
    env: &mut EmitEnv<'_>,
    element: &Element,
    name: &str,
) -> Result<ChildrenPlan, CompileError> {
    let children = env.sv.nodes(element.fragment.nodes);
    let mut snippet_functions = Vec::new();
    let mut snippet_props = Vec::new();
    let mut slot_keys = Vec::new();
    let mut has_default = false;
    for node in children {
        match node {
            FragmentNode::SnippetBlock(snippet) => {
                let (func, snippet_name) = build_snippet_function(env, snippet)?;
                snippet_functions.push(func);
                // The oracle serializes a snippet named `children` under the
                // `default` slot key, but the prop keeps the `children` name.
                let slot_key = if snippet_name == "children" {
                    "default".to_string()
                } else {
                    snippet_name.clone()
                };
                snippet_props.push(snippet_name);
                slot_keys.push(slot_key);
            }
            FragmentNode::Comment(_) => {}
            FragmentNode::Text(text) if text.is_collapsible_ws_only => {}
            FragmentNode::Element(child) if child_slot_attribute(env, child) => {
                return Err(unsupported(Refusal::ComponentNamedSlot {
                    name: name.to_string(),
                }));
            }
            // A `<svelte:element slot="x">` as a component child routes to a NAMED
            // slot in the oracle, but tsv's named-slot detection above is
            // `FragmentNode::Element`-only — without this arm the node would fall
            // through to `has_default` and MISROUTE into the default `children`.
            // Named slots are a permanent runes-only fence, never a deferral
            // (`Refusal::is_deliberate_fence`), so this arm IS the fence's
            // special-element half rather than a placeholder for future support. A
            // slot-less `<svelte:element>` is ordinary default content (`_` below).
            FragmentNode::SpecialElement(child)
                if matches!(child.kind, SpecialElementKind::SvelteElement { .. })
                    && special_element_slot_attribute(env, child) =>
            {
                return Err(unsupported(Refusal::ComponentNamedSlot {
                    name: name.to_string(),
                }));
            }
            _ => has_default = true,
        }
    }
    // A `children` prop AND default children route through `$$slots.default` with
    // a `children` error in the oracle — a divergent shape; refuse.
    if has_default && component_has_named_attribute(env, element, "children") {
        return Err(unsupported(Refusal::ComponentChildrenPropConflict {
            name: name.to_string(),
        }));
    }
    let default_children = if has_default {
        // The `children` arrow sees only the default-slot children — direct
        // `{#snippet}` children live in the wrapping block, not the arrow body.
        let nodes: Vec<FragmentNode> = children
            .iter()
            .filter(|node| !matches!(node, FragmentNode::SnippetBlock(_)))
            .copied()
            .collect();
        slot_keys.push("default".to_string());
        Some(nodes)
    } else {
        None
    };
    Ok(ChildrenPlan {
        snippet_functions,
        snippet_props,
        slot_keys,
        default_children,
    })
}

/// Whether a component child element carries a `slot="…"` attribute (a named
/// slot).
///
/// Case-**sensitive**, mirroring its oracle site: the server component visitor
/// matches `node.name === 'slot'` (`build_inline_component` in
/// `server/visitors/shared/component.js`). ⚠️ Do NOT unify this with
/// `element_census.rs`'s `has_slot_attribute`, which is case-**insensitive**
/// because CSS pruning lowercases (`get_possible_element_siblings` in
/// `css-prune.js`) — the oracle is split, so the two predicates are deliberately
/// different, and merging them would break CSS-prune parity.
fn child_slot_attribute(env: &EmitEnv<'_>, element: &Element) -> bool {
    component_has_named_attribute(env, element, "slot")
}

/// Whether a special element carries a plain `slot="…"` attribute (case-sensitive) —
/// the named-slot marker on a `<svelte:element>` component child.
///
/// The case-sensitivity matches `child_slot_attribute` and the same oracle site
/// (`build_inline_component` in `server/visitors/shared/component.js`); see that
/// function for why it must NOT be unified with the census's case-insensitive
/// `has_slot_attribute`.
fn special_element_slot_attribute(env: &EmitEnv<'_>, se: &SpecialElement) -> bool {
    env.sv.attributes(se.attributes).iter().any(|attr_node| {
        let AttributeNode::Attribute(attr) = attr_node else {
            return false;
        };
        attr.name(env.source) == "slot"
    })
}

/// Whether an element carries a plain attribute with the given (case-sensitive)
/// name.
fn component_has_named_attribute(env: &EmitEnv<'_>, element: &Element, name: &str) -> bool {
    env.sv
        .attributes(element.attributes)
        .iter()
        .any(|attr_node| {
            let AttributeNode::Attribute(attr) = attr_node else {
                return false;
            };
            attr.name(env.source) == name
        })
}

/// Emit `Name($$renderer, props)` for a component invocation (`<Foo … />`),
/// followed by a trailing `<!---->` anchor unless the enclosing fragment is
/// standalone (the oracle's `empty_comment` push). Named-snippet children wrap
/// the call in a bare `{ function …; Name(…); }` block. Dynamic components, named
/// slots, `--custom-property` attributes, and directives are refused — see the
/// individual refusal sites.
pub(crate) fn emit_component(
    env: &mut EmitEnv<'_>,
    element: &Element,
    out: &mut BodyBuilder,
    name: &str,
    is_standalone: bool,
) -> Result<(), CompileError> {
    // A dynamic component (member / reactive binding) compiles to the truthiness
    // guard — a later slice.
    if component_dynamic(env, name) {
        return Err(unsupported(Refusal::DynamicComponent {
            name: name.to_string(),
        }));
    }

    // Children plan: the `{#snippet}` prop functions (for the wrapping block) and
    // the synthetic props (snippet props, the `children` arrow, `$$slots`). Built
    // before the props so the snippet functions mint before the props reference
    // them.
    let plan = plan_component_children(env, element, name)?;

    // Build the props/spreads expression (a plain object, or `$.spread_props`),
    // appending the synthetic children props from the plan.
    let props_expr = build_component_props(env, element, name, &plan)?;

    // `Name($$renderer, props)`. The callee is the component reference (a plain
    // identifier — member components refuse above).
    let callee = env.b.ident_expr(name);
    let args = [env.b.ident_expr("$$renderer"), env.b.push(props_expr)];
    let call = env.b.call_of(callee, &args, false);
    let call_stmt = env.b.expression_statement(call);

    // Named-snippet children hoist their `function` declarations into a bare block
    // wrapping the call, so the snippet props resolve (the oracle's
    // `b.block([...snippet_declarations, statement])`).
    let stmt = if plan.snippet_functions.is_empty() {
        call_stmt
    } else {
        let mut block_body = Vec::with_capacity(plan.snippet_functions.len() + 1);
        for func in plan.snippet_functions {
            block_body.push(env.b.push(func));
        }
        block_body.push(env.b.push(call_stmt));
        let block_span = env.b.here();
        let body = env.b.push_list(&block_body);
        Statement::from_block_statement(BlockStatement {
            body,
            span: block_span,
        })
    };
    out.push_statement(&mut env.b, stmt);

    // A non-standalone component keeps the `<!---->` anchor so its output doesn't
    // fuse with the surrounding fragment.
    if !is_standalone {
        out.push_text("<!---->");
    }
    Ok(())
}

/// A group of component attributes: consecutive plain attributes accumulate into
/// one object literal; a `{...spread}` starts a new group (the oracle's
/// `props_and_spreads` grouping in `shared/component.js`).
enum PropGroup<'a> {
    Props(Vec<&'a Attribute>),
    Spread(ExprId),
}

/// Build the component call's props argument: a plain object `{ … }` when there
/// are no spreads (or a single leading props group), otherwise
/// `$.spread_props([ … ])` interleaving objects and spread expressions in source
/// order. Refuses `--custom-property` attributes, `bind:`, and other directives.
fn build_component_props(
    env: &mut EmitEnv<'_>,
    element: &Element,
    name: &str,
    plan: &ChildrenPlan,
) -> Result<Expression, CompileError> {
    // The synthetic children props (snippet props, the `children` arrow,
    // `$$slots`) go into the last props group, or a new one after a trailing
    // spread.
    let synthetic = plan.has_content().then_some(plan);
    let mut groups: Vec<PropGroup<'_>> = Vec::new();
    for attr_node in env.sv.attributes(element.attributes) {
        match attr_node {
            AttributeNode::Attribute(attr) => {
                let attr_name = attr.name(env.source).to_string();
                // A `--custom-property` attribute takes the oracle's `$.css_props`
                // path — a later slice.
                if attr_name.starts_with("--") {
                    return Err(unsupported(Refusal::ComponentCustomProperty {
                        name: name.to_string(),
                    }));
                }
                match groups.last_mut() {
                    Some(PropGroup::Props(props)) => props.push(attr),
                    _ => groups.push(PropGroup::Props(vec![attr])),
                }
            }
            AttributeNode::SpreadAttribute(spread) => {
                groups.push(PropGroup::Spread(spread.expression));
            }
            AttributeNode::BindDirective(_) => {
                return Err(unsupported(Refusal::ComponentBindDirective {
                    name: name.to_string(),
                }));
            }
            _ => {
                return Err(unsupported(Refusal::ComponentDirective {
                    name: name.to_string(),
                }));
            }
        }
    }

    // The oracle emits a plain object when there are no spreads (no groups, or a
    // single props group); otherwise `$.spread_props([...])`. The synthetic props
    // append to the last props group, or a new one when the last group is a spread
    // (the oracle's `push_prop`).
    let single_object =
        groups.is_empty() || (groups.len() == 1 && matches!(groups[0], PropGroup::Props(_)));
    if single_object {
        let attrs: &[&Attribute] = match groups.first() {
            Some(PropGroup::Props(props)) => props,
            _ => &[],
        };
        return build_props_object(env, attrs, synthetic);
    }

    // `$.spread_props([ obj_or_spread, … ])`. Mint the brackets around the
    // element construction so the array span encloses the minted object spans.
    let last_is_props = matches!(groups.last(), Some(PropGroup::Props(_)));
    let lbracket = env.b.mint("[").start;
    let mut elements: Vec<Option<ExprId>> = Vec::with_capacity(groups.len() + 1);
    let group_count = groups.len();
    for (i, group) in groups.iter().enumerate() {
        let element_expr = match group {
            PropGroup::Props(props) => {
                // The synthetic props join the last props group.
                let syn = if i + 1 == group_count {
                    synthetic
                } else {
                    None
                };
                let object = build_props_object(env, props, syn)?;
                env.b.push(object)
            }
            PropGroup::Spread(expr) => {
                // The template borrow point (`<Foo {...(o as any)} />`).
                let expr = env.erase(*expr)?;
                wrap_value_expr(env, expr)?
            }
        };
        elements.push(Some(element_expr));
    }
    // A trailing spread with synthetic props needs its own props object appended.
    if !last_is_props && let Some(plan) = synthetic {
        let object = build_props_object(env, &[], Some(plan))?;
        elements.push(Some(env.b.push(object)));
    }
    let rbracket = env.b.mint("]").end;
    let elements = env.b.push_list(&elements);
    let array = env.b.push(Expression {
        span: Span::new(lbracket, rbracket),
        kind: ExpressionKind::ArrayExpression(ArrayExpression {
            elements,
            spread_trailing_comma: false,
        }),
    });
    Ok(env.b.member_call("$", "spread_props", &[array]))
}

/// Build a plain object literal `{ … }` from a run of component attributes. The
/// braces are minted around the property construction so the object span encloses
/// the (appendix) key spans (the object printer reads its own span region for the
/// expansion decision — all appendix, no newlines, so it collapses when it fits).
fn build_props_object(
    env: &mut EmitEnv<'_>,
    attrs: &[&Attribute],
    synthetic: Option<&ChildrenPlan>,
) -> Result<Expression, CompileError> {
    let obrace = env.b.mint("{").start;
    let mut properties: Vec<ObjectProperty> = Vec::new();
    for attr in attrs {
        properties.push(ObjectProperty::Property(build_component_property(
            env, attr,
        )?));
    }
    // The synthetic children props, in the oracle's order: snippet props (source
    // order), then the implicit `children` arrow, then `$$slots`.
    if let Some(plan) = synthetic {
        for snippet_name in &plan.snippet_props {
            properties.push(ObjectProperty::Property(build_snippet_prop(
                env,
                snippet_name,
            )));
        }
        if let Some(nodes) = &plan.default_children {
            properties.push(ObjectProperty::Property(build_children_prop(env, nodes)?));
        }
        properties.push(ObjectProperty::Property(build_slots_prop(
            env,
            &plan.slot_keys,
        )));
    }
    let cbrace = env.b.mint("}").end;
    let properties = env.b.push_list(&properties);
    Ok(Expression {
        span: Span::new(obrace, cbrace),
        kind: ExpressionKind::ObjectExpression(ObjectExpression {
            properties,
            spread_trailing_comma: false,
        }),
    })
}

/// A `{ name }` shorthand prop for a named-snippet child — the value references
/// the `function name(…)` declaration in the component's wrapping block.
fn build_snippet_prop(env: &mut EmitEnv<'_>, name: &str) -> Property {
    let key = env.b.ident(name);
    let key_span = key.span;
    let key = env.b.push(Expression::from_identifier(key));
    let value = env.b.ident_expr(name);
    init_property(key, value, true, key_span)
}

/// The implicit `children` prop for a component's default-slot children:
/// `children: ($$renderer) => { …body… }`. The body reuses the fragment
/// machinery (text-first eligible, per the oracle's `is_text_first` Component
/// parent). The key is minted first so the (key-only) property span stays forward.
fn build_children_prop(
    env: &mut EmitEnv<'_>,
    nodes: &[FragmentNode],
) -> Result<Property, CompileError> {
    let key = env.b.ident("children");
    let key_span = key.span;
    // Component children (the default-slot `children` snippet) re-infer their
    // namespace from their own nodes (Svelte's `Component` parent — in the
    // deep-walk special list); the inherited context is not threaded here, so the
    // html document default is the fallback when the slot holds no elements.
    let body = emit_child_body(
        env,
        nodes,
        &[],
        true,
        false,
        ChildNamespace {
            inherited: Namespace::Html,
            parent: FragmentParent::Special,
            in_svg_text: false,
        },
        HashMap::new(),
    )?;
    let renderer_param = Expression::from_identifier(env.b.ident("$$renderer"));
    let block_span = env.b.here();
    let arrow = env.b.arrow_block(&[renderer_param], body, block_span);
    let key = env.b.push(Expression::from_identifier(key));
    let arrow = env.b.push(arrow);
    Ok(init_property(key, arrow, false, key_span))
}

/// The `$$slots: { key1: true, … }` prop that accompanies component children —
/// one `true` entry per named-snippet slot plus `default` for default children
/// (slot names are always valid identifiers). Named-slot arrow values would live
/// here too, but named slots are refused.
fn build_slots_prop(env: &mut EmitEnv<'_>, slot_keys: &[String]) -> Property {
    let key = env.b.ident("$$slots");
    let key_span = key.span;
    let obrace = env.b.mint("{").start;
    let mut inner_props: Vec<ObjectProperty> = Vec::with_capacity(slot_keys.len());
    for slot_key in slot_keys {
        let entry_key = env.b.ident(slot_key);
        let entry_key_span = entry_key.span;
        let entry_key = env.b.push(Expression::from_identifier(entry_key));
        let entry_val = env.b.true_literal();
        let entry_val = env.b.push(entry_val);
        inner_props.push(ObjectProperty::Property(init_property(
            entry_key,
            entry_val,
            false,
            entry_key_span,
        )));
    }
    let cbrace = env.b.mint("}").end;
    let properties = env.b.push_list(&inner_props);
    let inner = env.b.push(Expression {
        span: Span::new(obrace, cbrace),
        kind: ExpressionKind::ObjectExpression(ObjectExpression {
            properties,
            spread_trailing_comma: false,
        }),
    });
    let key = env.b.push(Expression::from_identifier(key));
    init_property(key, inner, false, key_span)
}

/// Build one `key: value` object property from a component attribute. The key is
/// an identifier when it matches the identifier grammar (else a string literal);
/// `shorthand` is set when the key is an identifier and the value is the plain
/// identifier of the same name (`{ n: n }` prints as `{ n }`). The key is minted
/// before the value, so the (key-only) property span stays forward and in the
/// appendix; the value prints from its own span.
fn build_component_property(
    env: &mut EmitEnv<'_>,
    attr: &Attribute,
) -> Result<Property, CompileError> {
    let name = attr.name(env.source).to_string();
    let key_is_ident = is_js_identifier(&name);
    let key = if key_is_ident {
        Expression::from_identifier(env.b.ident(&name))
    } else {
        env.b.string_literal_expr(&name)
    };
    let key_span = key.span();
    let key = env.b.push(key);
    let value = build_prop_value(env, attr)?;
    let shorthand = key_is_ident
        && matches!(&env.b.view().expr(value).kind, ExpressionKind::Identifier(id)
            if plain_identifier_name(id, env.source).as_deref() == Some(name.as_str()));
    Ok(init_property(key, value, shorthand, key_span))
}

/// Build a component attribute's prop value:
///
/// - a boolean attribute → `true`;
/// - a single static text value → the *decoded* data as a string literal (no
///   HTML escaping, no trim — the oracle's `is_component` branch of
///   `build_attribute_value`);
/// - a single expression value → guarded, bare-derived → `d()`, passed through
///   with **no fold** (the single-chunk component path doesn't evaluate);
/// - a mixed text+expression value → a template literal (or a folded string
///   literal when every part is statically known).
///
/// The value is pushed and its id returned.
fn build_prop_value(env: &mut EmitEnv<'_>, attr: &Attribute) -> Result<ExprId, CompileError> {
    let Some(values) = env.sv.opt_attribute_values(attr.value) else {
        let flag = env.b.true_literal();
        return Ok(env.b.push(flag));
    };
    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            let literal = env.b.string_literal_expr(&decoded);
            Ok(env.b.push(literal))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // The template borrow point. The erased node also decides the caller's
            // `{ n }` shorthand test — `<Foo n={n as T} />` is `{ n }` to the oracle.
            let expr = env.erase(tag.expression)?;
            wrap_value_expr(env, expr)
        }
        // A mixed text+expression value. Unlike the element mixed-attribute path
        // there is no whitespace trim (hence `false`), no HTML escaping, and no
        // `$.attr*` wrapper — the oracle's component `build_attribute_value`
        // returns the bare value.
        _ => build_mixed_value_expr(env, false, values),
    }
}
