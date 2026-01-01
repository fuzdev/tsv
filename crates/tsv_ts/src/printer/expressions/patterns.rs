// Destructuring pattern printing for TypeScript
//
// This module handles all destructuring patterns:
// - Object patterns: `{a, b}` with width-based expansion
// - Array patterns: `[a, b]`
// - Assignment patterns: `a = 1`
// - Assignment expressions: `a = b` with width-based wrapping
// - Rest elements: `...rest`

use super::{PatternContext, Printer, object_pattern_should_expand};
use crate::ast::internal::{self, ArrowFunctionBody, Expression};
use tsv_lang::doc::{self, Doc};

/// Check if an expression has block-like structure that handles its own expansion
///
/// These expressions should NOT have a break after `=` in assignment context
/// because their internal structure (function bodies, etc.) handles expansion.
fn rhs_has_block_structure(expr: &Expression) -> bool {
    match expr {
        // Direct block-structure expressions
        Expression::FunctionExpression(_) | Expression::ClassExpression(_) => true,
        Expression::ArrowFunctionExpression(arrow) => {
            matches!(&arrow.body, ArrowFunctionBody::BlockStatement(_))
        }

        // Call expressions with block-like arguments handle their own expansion
        // e.g., `fork(() => { ... })` - the callback handles its own expansion
        Expression::CallExpression(call) => call.arguments.iter().any(rhs_has_block_structure),

        // New expressions: always handle their own expansion
        // They use grouping for parenthesized callees: `new (a || b)()`
        // and argument wrapping for multi-arg cases
        Expression::NewExpression(_) => true,

        // Objects and arrays handle their own expansion
        Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => true,

        _ => false,
    }
}

impl<'a> Printer<'a> {
    /// Build a Doc for an assignment expression
    pub(super) fn build_assignment_doc(&self, assign: &internal::AssignmentExpression) -> Doc {
        let left_doc = self.build_expression_doc(&assign.left);
        let right_doc = self.build_expression_doc(&assign.right);

        // Expressions with their own blocks (function, class) don't get extra indent
        // They format as: `x = function() {\n\t...\n};`
        // Not: `x =\n\tfunction() {\n\t\t...\n\t};`
        let rhs_uses_own_block = rhs_has_block_structure(assign.right.as_ref());

        // Object patterns on LHS shouldn't use group - they handle their own expansion
        // They format as: `({a, b} = obj)` or `({\n\ta,\n\tb,\n} = obj);`
        // In both cases, `= obj` stays on the same line as the closing `}`
        let lhs_is_object_pattern = matches!(assign.left.as_ref(), Expression::ObjectPattern(_));

        if rhs_uses_own_block || lhs_is_object_pattern {
            // No group wrapping - just space before RHS
            doc::concat(vec![
                left_doc,
                doc::text(" "),
                doc::text(assign.operator.as_str()),
                doc::text(" "),
                right_doc,
            ])
        } else {
            // Standard: group(left + " =" + indent(line + right))
            doc::group(doc::concat(vec![
                left_doc,
                doc::text(" "),
                doc::text(assign.operator.as_str()),
                doc::indent_line(right_doc),
            ]))
        }
    }

    /// Build a Doc for an object pattern
    ///
    /// Prettier expands object patterns when:
    /// 1. Any property has a nested pattern value (always expand)
    /// 2. The pattern exceeds print width (width-based expansion)
    pub(super) fn build_object_pattern_doc(&self, obj: &internal::ObjectPattern) -> Doc {
        self.build_object_pattern_doc_with_context(obj, PatternContext::Standalone)
    }

    /// Build object pattern doc with explicit context
    pub(super) fn build_object_pattern_doc_with_context(
        &self,
        obj: &internal::ObjectPattern,
        context: PatternContext,
    ) -> Doc {
        if obj.properties.is_empty() {
            self.build_empty_object_pattern_doc(obj)
        } else {
            let should_expand = object_pattern_should_expand(obj, context);

            if should_expand {
                // Nested patterns: always expand
                self.build_expanded_object_pattern_doc(obj)
            } else {
                // Use group with line breaks for width-based expansion
                // Include type annotation in the group so its width is considered
                let mut parts = Vec::new();
                let last_is_rest = matches!(
                    obj.properties.last(),
                    Some(internal::ObjectPatternProperty::RestElement(_))
                );

                for (i, prop) in obj.properties.iter().enumerate() {
                    parts.push(self.build_object_pattern_property_doc(prop));
                    if i < obj.properties.len() - 1 {
                        parts.push(doc::text(","));
                        parts.push(doc::line());
                    } else if !last_is_rest {
                        // Last property: trailing comma only when broken
                        // (rest elements can't have trailing commas)
                        parts.push(doc::trailing_comma());
                    }
                }

                // Build group contents: { + properties + }
                let mut group_parts = vec![
                    doc::text("{"),
                    doc::indent_softline(doc::concat(parts)),
                    doc::softline(),
                    doc::text("}"),
                ];

                // Include type annotation in the group for width calculation
                if let Some(type_annotation) = &obj.type_annotation {
                    group_parts.push(self.build_type_annotation_doc(type_annotation));
                }

                doc::group(doc::concat(group_parts))
            }
        }
    }

    /// Build doc for empty object pattern: `{}` with optional type annotation
    fn build_empty_object_pattern_doc(&self, obj: &internal::ObjectPattern) -> Doc {
        if let Some(type_annotation) = &obj.type_annotation {
            doc::concat(vec![
                doc::text("{}"),
                self.build_type_annotation_doc(type_annotation),
            ])
        } else {
            doc::text("{}")
        }
    }

    /// Build expanded doc for object pattern with hardlines (always multiline)
    fn build_expanded_object_pattern_doc(&self, obj: &internal::ObjectPattern) -> Doc {
        let last_is_rest = matches!(
            obj.properties.last(),
            Some(internal::ObjectPatternProperty::RestElement(_))
        );

        let mut prop_parts = Vec::new();
        for (i, prop) in obj.properties.iter().enumerate() {
            prop_parts.push(self.build_object_pattern_property_doc(prop));
            let is_last = i == obj.properties.len() - 1;
            // Add trailing comma unless it's a rest element (syntax error)
            if !is_last || !last_is_rest {
                prop_parts.push(doc::text(","));
            }
            if !is_last {
                prop_parts.push(doc::hardline());
            }
        }

        // Structure: { + indent(hardline + props) + hardline + } + type_annotation
        let mut result_parts = vec![
            doc::text("{"),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(prop_parts)])),
            doc::hardline(),
            doc::text("}"),
        ];

        if let Some(type_annotation) = &obj.type_annotation {
            result_parts.push(self.build_type_annotation_doc(type_annotation));
        }

        doc::concat(result_parts)
    }

    /// Build a Doc for an object pattern property
    ///
    /// String keys that are valid identifiers are normalized to unquoted form:
    /// `{"key": value}` → `{key: value}`
    fn build_object_pattern_property_doc(&self, prop: &internal::ObjectPatternProperty) -> Doc {
        match prop {
            internal::ObjectPatternProperty::Property(p) => {
                if p.shorthand {
                    // Handle both AssignmentPattern and AssignmentExpression
                    // (parser may produce AssignmentExpression in some contexts)
                    if let Expression::AssignmentPattern(pattern) = &p.value {
                        doc::concat(vec![
                            self.build_expression_doc(&p.key),
                            doc::text(" = "),
                            self.build_expression_doc(&pattern.right),
                        ])
                    } else if let Expression::AssignmentExpression(assign) = &p.value {
                        doc::concat(vec![
                            self.build_expression_doc(&p.key),
                            doc::text(" = "),
                            self.build_expression_doc(&assign.right),
                        ])
                    } else {
                        self.build_expression_doc(&p.key)
                    }
                } else {
                    // Handle computed keys: {[key]: value}
                    // For regular keys, use property_key_doc to normalize string keys to identifiers
                    let key_doc = if p.computed {
                        doc::concat(vec![
                            doc::text("["),
                            self.build_expression_doc(&p.key),
                            doc::text("]"),
                        ])
                    } else {
                        self.build_property_key_doc(&p.key)
                    };
                    doc::concat(vec![
                        key_doc,
                        doc::text(": "),
                        self.build_expression_doc(&p.value),
                    ])
                }
            }
            internal::ObjectPatternProperty::RestElement(r) => self.build_rest_element_doc(r),
        }
    }

    /// Build a Doc for an array pattern
    pub(super) fn build_array_pattern_doc(&self, arr: &internal::ArrayPattern) -> Doc {
        let mut parts = Vec::new();
        parts.push(doc::text("["));
        for (i, elem) in arr.elements.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            if let Some(e) = elem {
                parts.push(self.build_expression_doc(e));
            }
        }
        parts.push(doc::text("]"));

        // Include type annotation if present
        if let Some(type_annotation) = &arr.type_annotation {
            parts.push(self.build_type_annotation_doc(type_annotation));
        }

        doc::concat(parts)
    }

    /// Build a Doc for an assignment pattern
    pub(super) fn build_assignment_pattern_doc(
        &self,
        pattern: &internal::AssignmentPattern,
    ) -> Doc {
        doc::concat(vec![
            self.build_expression_doc(&pattern.left),
            doc::text(" = "),
            self.build_expression_doc(&pattern.right),
        ])
    }

    /// Build a Doc for a rest element
    pub(super) fn build_rest_element_doc(&self, rest: &internal::RestElement) -> Doc {
        doc::concat(vec![
            doc::text("..."),
            self.build_expression_doc(&rest.argument),
        ])
    }
}
