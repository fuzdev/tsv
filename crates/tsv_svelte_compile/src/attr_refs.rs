//! The shared element-attribute reference traversal.
//!
//! Both the snippet hoist analysis (`snippet.rs`) and the `needs_context` walk
//! (`needs_context.rs`) must see every attribute expression the compiled output
//! can reference. They previously hand-wrote the same iteration and drifted:
//! the component-spread arm existed in one but not the other, so a top-level
//! snippet whose only instance-binding reference sat in a `<Foo {...binding} />`
//! spread was wrongly module-hoisted — a runtime `ReferenceError` the reparse
//! self-validation cannot catch (a free reference parses fine). This module is
//! the single definition of "reference-bearing attribute expression"; an
//! attribute shape that newly reaches emission must be added HERE so every
//! analysis sees it at once.
//!
//! Two traversals share that definition. `each_attribute_expression` is the
//! **emitted-path** view: it skips the positions refused at emission (element
//! spreads, directives, `{@attach}`), because the emission refusal is what keeps
//! their references out of output. `each_reference_bearing_attribute_expression`
//! is the **dropped-fragment** view (a `{:catch}` branch the emitter discards
//! without walking): there no emission refusal fires, so every attribute
//! reference must be counted to match the oracle. The dropped-fragment view has
//! two more entry points for the same reason — `each_reference_bearing_directive_name`
//! (a directive whose *name* is a value binding) and
//! `special_element_reference_expression` (a `<svelte:element>`/`<svelte:component>`
//! `this={…}`); every special element is refused at emission elsewhere, so its
//! references, too, are only reachable through the dropped-fragment path.

use tsv_svelte::ast::internal::{
    AttributeNode, AttributeValue, Element, ElementKind, SpecialElement, SpecialElementKind,
    StyleDirectiveValue,
};
use tsv_ts::ast::internal::Expression;

/// Visit every attribute expression of `element` that can reach compiled
/// output:
///
/// - plain attribute expression values (single-expression and mixed-value
///   chunks) on any element;
/// - `{...spread}` expressions on **components** (emitted as `$.spread_props`
///   array elements).
///
/// An *element* spread is refused at emission, so its expression never reaches
/// output — and visiting it here would let an analysis refusal fire before the
/// emission refusal, shifting corpus buckets. Directives and `{@attach}` are
/// likewise refused at emission (on elements and components both) and are not
/// visited.
pub(crate) fn each_attribute_expression<'a, 'arena>(
    element: &'a Element<'arena>,
    f: &mut impl FnMut(&'a Expression<'arena>),
) {
    for attr_node in element.attributes {
        match attr_node {
            AttributeNode::Attribute(attr) => {
                if let Some(values) = attr.value {
                    for value in values {
                        if let AttributeValue::ExpressionTag(tag) = value {
                            f(&tag.expression);
                        }
                    }
                }
            }
            AttributeNode::SpreadAttribute(spread) if element.kind == ElementKind::Component => {
                f(&spread.expression);
            }
            _ => {}
        }
    }
}

/// Visit every reference-bearing attribute expression of `element`, **including
/// the positions `each_attribute_expression` deliberately skips** — element
/// `{...spread}`, every directive expression, and `{@attach}`.
///
/// This is the traversal for a **dropped** fragment (a `{:catch}` branch), whose
/// contents the emitter discards without walking. There the emission refusal that
/// `each_attribute_expression`'s exclusions rely on never fires, so the analyses
/// must count these references themselves to match the oracle, which counts every
/// reference inside a dropped `{:catch}` (a snippet whose only instance-binding
/// reference sits in such a position must **not** hoist; a `new`/prop-rooted
/// access there must still trigger the `$$renderer.component` wrapper). On the
/// non-dropped (emitted) path these positions still refuse at emission, so the
/// default `each_attribute_expression` remains correct there.
///
/// A directive's **name** (`use:`/`transition:`/`in:`/`out:`/`animate:`) is also a
/// binding reference the oracle counts, but tsv stores it as a verbatim name span
/// rather than an `Expression`, so an expression traversal cannot reach it — it is
/// surfaced separately by `each_reference_bearing_directive_name`.
///
/// `LetDirective` binds a name (a slot prop) and contributes no free reference, so
/// it is excluded.
///
/// Takes `&[AttributeNode]` (not `&Element`) so it also serves a special element's
/// attributes on the dropped path.
pub(crate) fn each_reference_bearing_attribute_expression<'a, 'arena>(
    attributes: &'a [AttributeNode<'arena>],
    f: &mut impl FnMut(&'a Expression<'arena>),
) {
    for attr_node in attributes {
        match attr_node {
            AttributeNode::Attribute(attr) => {
                if let Some(values) = attr.value {
                    for value in values {
                        if let AttributeValue::ExpressionTag(tag) = value {
                            f(&tag.expression);
                        }
                    }
                }
            }
            AttributeNode::SpreadAttribute(spread) => f(&spread.expression),
            AttributeNode::AttachTag(attach) => f(&attach.expression),
            AttributeNode::OnDirective(d) => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::BindDirective(d) => f(&d.expression),
            AttributeNode::ClassDirective(d) => f(&d.expression),
            AttributeNode::StyleDirective(d) => match &d.value {
                StyleDirectiveValue::ExpressionTag(tag) => f(&tag.expression),
                StyleDirectiveValue::Parts(parts) => {
                    for value in *parts {
                        if let AttributeValue::ExpressionTag(tag) = value {
                            f(&tag.expression);
                        }
                    }
                }
                StyleDirectiveValue::True => {}
            },
            AttributeNode::UseDirective(d) => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::TransitionDirective(d) => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::AnimateDirective(d) => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::LetDirective(_) => {}
        }
    }
}

/// Visit the NAME of every directive whose name is a value-binding reference:
/// `use:` (action), `transition:`/`in:`/`out:` (transition fn), `animate:`
/// (animation fn). These name a binding the oracle counts; the other directives'
/// names are event / DOM-property / class / CSS names (not references) and `let:`
/// binds a name. tsv stores the name as a verbatim `name_span` rather than an
/// `Expression` — and it may be a member path (`use:a.b`) — so the raw slice is
/// surfaced for the consumer to reduce to its root identifier.
///
/// Consumed only by the snippet-hoist analysis on the dropped-`{:catch}` path: a
/// bare name reference never triggers `needs_context` (which fires on `new` /
/// member-call roots only), and on the emitted path these directives refuse.
pub(crate) fn each_reference_bearing_directive_name<'s>(
    attributes: &[AttributeNode<'_>],
    source: &'s str,
    f: &mut impl FnMut(&'s str),
) {
    for attr_node in attributes {
        let name_span = match attr_node {
            AttributeNode::UseDirective(d) => d.name_span,
            AttributeNode::TransitionDirective(d) => d.name_span,
            AttributeNode::AnimateDirective(d) => d.name_span,
            _ => continue,
        };
        f(name_span.extract(source));
    }
}

/// The `this={…}` expression a special element carries as a binding reference:
/// `<svelte:element this={tag}>` and `<svelte:component this={Component}>`. The
/// other special-element kinds carry no such expression (their references live in
/// attributes / children). Used only on the dropped-`{:catch}` path — every
/// special element is refused at emission elsewhere.
pub(crate) fn special_element_reference_expression<'a, 'arena>(
    se: &'a SpecialElement<'arena>,
) -> Option<&'a Expression<'arena>> {
    match &se.kind {
        SpecialElementKind::SvelteElement { tag } => Some(tag),
        SpecialElementKind::SvelteComponent { expression } => Some(expression),
        _ => None,
    }
}
