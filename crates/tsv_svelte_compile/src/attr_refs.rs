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

use tsv_svelte::ast::internal::{AttributeNode, AttributeValue, Element, ElementKind};
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
