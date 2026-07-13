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
//! reference must be counted to match the oracle.

use tsv_svelte::ast::internal::{
    AttributeNode, AttributeValue, Element, ElementKind, StyleDirectiveValue,
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
/// rather than an `Expression`, so an expression traversal cannot reach it; that
/// residual affects only the snippet-hoist decision (a bare name never triggers
/// `needs_context`) and is not covered here.
///
/// `LetDirective` binds a name (a slot prop) and contributes no free reference, so
/// it is excluded.
pub(crate) fn each_reference_bearing_attribute_expression<'a, 'arena>(
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
