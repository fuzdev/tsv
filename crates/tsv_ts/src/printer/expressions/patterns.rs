// Destructuring pattern printing for TypeScript
//
// This module handles all destructuring patterns:
// - Object patterns: `{a, b}` with width-based expansion
// - Array patterns: `[a, b]`
// - Assignment patterns: `a = 1`
// - Assignment expressions: `a = b` with width-based wrapping
// - Rest elements: `...rest`

use super::super::Printer;
use crate::ast::internal::{self, Expression};
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Print an assignment expression with width-based wrapping
    ///
    /// When the assignment exceeds print_width, wraps after `=`:
    /// ```javascript
    /// [long, array, pattern] =
    ///     value;
    /// ```
    ///
    /// However, certain patterns use direct printing without width-based wrapping:
    /// - LHS is ObjectPattern: pattern handles its own expansion, `} = rhs` stays together
    /// - RHS handles internal breaking: callbacks/bodies expand internally, don't break after `=`
    pub(super) fn print_assignment_expression(&mut self, assign: &internal::AssignmentExpression) {
        // Check if we should use direct printing (no width-based break after `=`)
        let use_direct_printing =
            // LHS: ObjectPattern handles its own width-based expansion
            matches!(assign.left.as_ref(), Expression::ObjectPattern(_))
            // RHS: expressions that handle their own internal breaking
            || matches!(
                &*assign.right,
                Expression::CallExpression(_)
                    | Expression::NewExpression(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::FunctionExpression(_)
                    | Expression::ClassExpression(_)
                    | Expression::AwaitExpression(_)
            );

        if use_direct_printing {
            self.print_expression(&assign.left);
            self.write(" ");
            self.write(assign.operator.as_str());
            self.write(" ");
            self.print_expression(&assign.right);
            return;
        }

        // Array patterns and other assignments: use doc-builder for width-based wrapping
        // When the line exceeds print width, wrap after `=`
        let assignment_doc = self.build_assignment_doc(assign);
        self.write_doc(&assignment_doc);
    }

    /// Build a Doc for an assignment expression
    pub(super) fn build_assignment_doc(&self, assign: &internal::AssignmentExpression) -> Doc {
        let left_doc = self.build_expression_doc(&assign.left);
        let right_doc = self.build_expression_doc(&assign.right);

        // Structure: group(left + " =" + indent(line + right))
        doc::group(doc::concat(vec![
            left_doc,
            doc::text(" "),
            doc::text(assign.operator.as_str()),
            doc::indent_line(right_doc),
        ]))
    }

    /// Print an object pattern: `{a, b}` or expanded `{\n\ta,\n\tb,\n}`
    ///
    /// Prettier expands object patterns when:
    /// 1. Any property has a nested pattern value (always expand)
    /// 2. The pattern exceeds print width (width-based expansion)
    pub(super) fn print_object_pattern(&mut self, obj: &internal::ObjectPattern) {
        if obj.properties.is_empty() {
            self.write("{}");
            // Print type annotation if present
            if let Some(type_annotation) = &obj.type_annotation {
                self.print_type_annotation(type_annotation);
            }
            return;
        }

        let should_expand = super::super::object_pattern_should_expand(obj);

        if should_expand {
            // Nested patterns: always expand (imperative path for performance)
            let last_is_rest = matches!(
                obj.properties.last(),
                Some(internal::ObjectPatternProperty::RestElement(_))
            );

            self.write("{\n");
            self.indent_level += 1;
            for (i, prop) in obj.properties.iter().enumerate() {
                self.write_indent();
                self.print_object_pattern_property(prop);
                // Add trailing comma unless it's a rest element (syntax error)
                let is_last = i == obj.properties.len() - 1;
                if !is_last || !last_is_rest {
                    self.write(",");
                }
                if !is_last {
                    self.write("\n");
                }
            }
            self.write("\n");
            self.indent_level -= 1;
            self.write_indent();
            self.write("}");

            // Print type annotation if present
            if let Some(type_annotation) = &obj.type_annotation {
                self.print_type_annotation(type_annotation);
            }
        } else {
            // Use doc-builder for width-based expansion (includes type annotation)
            let doc = self.build_object_pattern_doc(obj);
            self.write_doc(&doc);
        }
    }

    /// Print an object pattern property
    fn print_object_pattern_property(&mut self, prop: &internal::ObjectPatternProperty) {
        match prop {
            internal::ObjectPatternProperty::Property(p) => {
                if p.shorthand {
                    // For shorthand with default value, we need to print key = default
                    // Handle both AssignmentPattern and AssignmentExpression
                    // (parser may produce AssignmentExpression in some contexts)
                    if let Expression::AssignmentPattern(pattern) = &p.value {
                        self.print_expression(&p.key);
                        self.write(" = ");
                        self.print_expression(&pattern.right);
                    } else if let Expression::AssignmentExpression(assign) = &p.value {
                        self.print_expression(&p.key);
                        self.write(" = ");
                        self.print_expression(&assign.right);
                    } else {
                        self.print_expression(&p.key);
                    }
                } else {
                    // Handle computed keys: {[key]: value}
                    // For regular keys, use print_property_key to normalize string keys to identifiers
                    if p.computed {
                        self.write("[");
                        self.print_expression(&p.key);
                        self.write("]");
                    } else {
                        self.print_property_key(&p.key);
                    }
                    self.write(": ");
                    self.print_expression(&p.value);
                }
            }
            internal::ObjectPatternProperty::RestElement(r) => {
                self.print_rest_element(r);
            }
        }
    }

    /// Build a Doc for an object pattern
    ///
    /// Prettier expands object patterns when:
    /// 1. Any property has a nested pattern value (always expand)
    /// 2. The pattern exceeds print width (width-based expansion)
    pub(super) fn build_object_pattern_doc(&self, obj: &internal::ObjectPattern) -> Doc {
        if obj.properties.is_empty() {
            self.build_empty_object_pattern_doc(obj)
        } else {
            let should_expand = super::super::object_pattern_should_expand(obj);

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

    /// Build a Doc for an object pattern that is forced to expand (use hardlines)
    ///
    /// Used when the caller knows the pattern must expand (e.g., line would overflow).
    pub(super) fn build_object_pattern_doc_expanded(&self, obj: &internal::ObjectPattern) -> Doc {
        if obj.properties.is_empty() {
            self.build_empty_object_pattern_doc(obj)
        } else {
            self.build_expanded_object_pattern_doc(obj)
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

    /// Print an array pattern: `[a, b]`
    pub(super) fn print_array_pattern(&mut self, arr: &internal::ArrayPattern) {
        self.write("[");
        for (i, elem) in arr.elements.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            if let Some(e) = elem {
                self.print_expression(e);
            }
        }
        self.write("]");

        // Print type annotation if present
        if let Some(type_annotation) = &arr.type_annotation {
            self.print_type_annotation(type_annotation);
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

    /// Print an assignment pattern: `a = 1`
    pub(super) fn print_assignment_pattern(&mut self, pattern: &internal::AssignmentPattern) {
        self.print_expression(&pattern.left);
        self.write(" = ");
        self.print_expression(&pattern.right);
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

    /// Print a rest element: `...rest`
    pub(super) fn print_rest_element(&mut self, rest: &internal::RestElement) {
        self.write("...");
        self.print_expression(&rest.argument);
    }

    /// Build a Doc for a rest element
    pub(super) fn build_rest_element_doc(&self, rest: &internal::RestElement) -> Doc {
        doc::concat(vec![
            doc::text("..."),
            self.build_expression_doc(&rest.argument),
        ])
    }
}
