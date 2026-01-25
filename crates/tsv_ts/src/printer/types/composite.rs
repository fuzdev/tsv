// Composite type printing for TypeScript
//
// Handles:
// - Conditional types: `T extends U ? A : B`
// - Mapped types: `{ [K in T]: V }`
// - Tuple types: `[A, B, C]`
// - Array types: `T[]`
// - Type queries: `typeof x`
// - Entity names: `A.B.C`

use super::super::comments_in_range;
use super::Printer;
use super::helpers::{type_needs_parens_for_array_element, unwrap_parenthesized};
use crate::ast::internal::{
    self, TSArrayType, TSConditionalType, TSMappedType, TSTupleType, TSType,
};
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    //
    // Conditional Types
    //

    /// Build doc for conditional type WITHOUT the outer group wrapper.
    /// This is used for nested conditionals which should inherit breaking from their parent.
    ///
    /// Structure: `check extends extends_type [indent: line, "? ", true_type, line, ": ", false_type]`
    pub(super) fn build_conditional_type_doc_inner(&self, c: &TSConditionalType) -> Doc {
        // Build true_type doc: if it's a conditional (possibly wrapped in parens), don't wrap in group
        // Add parens for readability only when flat (single-line), not when broken (multi-line)
        let true_type_doc =
            if let TSType::Conditional(inner) = unwrap_parenthesized(c.true_type.as_ref()) {
                // Nested conditional in true position:
                // - Flat: add parens for readability: `T extends A ? (T extends B ? C : D) : E`
                // - Broken: no parens (the line breaks provide clarity)
                let inner_doc = self.build_conditional_type_doc_inner(inner);
                doc::if_break(
                    inner_doc.clone(),
                    doc::concat(vec![doc::text("("), inner_doc, doc::text(")")]),
                )
            } else {
                self.build_type_doc(&c.true_type)
            };

        // Build false_type doc: if it's a conditional, don't wrap in group
        // No parens needed for nested conditionals in false position (right-associative)
        let false_type_doc =
            if let TSType::Conditional(inner) = unwrap_parenthesized(c.false_type.as_ref()) {
                self.build_conditional_type_doc_inner(inner)
            } else {
                self.build_type_doc(&c.false_type)
            };

        // Build extends_type doc - unions need special handling to avoid trailing space
        // after "extends" when the union breaks (e.g., `T extends\n\t| A\n\t| B`)
        let extends_type_doc = if let TSType::Union(union) = c.extends_type.as_ref() {
            if union.types.is_empty() {
                doc::text(" ")
            } else {
                let mut parts = Vec::new();
                for (i, t) in union.types.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::if_break(
                            doc::concat(vec![doc::line(), doc::text("| ")]),
                            doc::text(" | "),
                        ));
                    } else {
                        // First type: line() + "| " when broken, space when flat
                        parts.push(doc::if_break(
                            doc::concat(vec![doc::line(), doc::text("| ")]),
                            doc::text(" "),
                        ));
                    }
                    parts.push(self.build_type_doc(t));
                }
                doc::group(doc::indent(doc::concat(parts)))
            }
        } else {
            doc::concat(vec![doc::text(" "), self.build_type_doc(&c.extends_type)])
        };

        doc::concat(vec![
            self.build_type_doc(&c.check_type),
            doc::text(" extends"),
            extends_type_doc,
            doc::indent(doc::concat(vec![
                doc::line(),
                doc::text("? "),
                true_type_doc,
                doc::line(),
                doc::text(": "),
                false_type_doc,
            ])),
        ])
    }

    //
    // Mapped Types
    //

    /// Build doc for mapped type: `{ [K in T]: V }`
    ///
    /// Source-fidelity aware: preserves multi-line formatting when source is multi-line.
    /// - Source one-line, fits: `{[K in keyof T]: T[K]}`
    /// - Source one-line, long: `{\n\t[K in keyof T]: T[K];\n}`
    /// - Source multi-line: `{\n\t[K in keyof T]: T[K];\n}` (always)
    pub(super) fn build_mapped_type_doc(&self, m: &TSMappedType) -> Doc {
        // Check if source was multi-line (preserve author's formatting choice)
        let source_is_multiline = super::super::is_brace_block_multiline(self.source, m.span);

        // Find the start of the mapping content (after `{`)
        let content_start = m.span.start + 1; // after `{`
        let param_name_start = m.type_parameter.span.start; // start of `K`

        // Comments between `{` and `K`
        // - Multiline: go on their own line BEFORE `[`
        // - Single-line: go inline AFTER `[`
        let comments_before_mapping: Vec<_> =
            comments_in_range(self.comments, content_start, param_name_start).collect();

        // Build the mapping body (starting from `[`)
        let mut body_parts = vec![];

        // readonly modifier: `readonly` or `-readonly`
        if let Some(readonly) = m.readonly {
            body_parts.push(doc::text(if readonly { "readonly " } else { "-readonly " }));
        }

        // [K in constraint]
        body_parts.push(doc::text("["));

        // For single-line: comments go after `[`
        if !source_is_multiline {
            for comment in &comments_before_mapping {
                body_parts.push(self.build_comment_doc(comment));
                body_parts.push(doc::text(" "));
            }
        }

        body_parts.push(doc::text_owned(m.type_parameter.name.clone()));
        body_parts.push(doc::text(" in "));
        body_parts.push(self.build_type_doc(&m.type_parameter.constraint));

        // as clause: `as NewKeyType`
        if let Some(name_type) = &m.name_type {
            body_parts.push(doc::text(" as "));
            body_parts.push(self.build_type_doc(name_type));
        }

        body_parts.push(doc::text("]"));

        // optional modifier: `?` or `-?`
        if let Some(optional) = m.optional {
            body_parts.push(doc::text(if optional { "?" } else { "-?" }));
        }

        // Comments and value type
        if let Some(type_ann) = &m.type_annotation {
            let type_start = type_ann.span().start;
            let type_end = type_ann.span().end;

            // Find the end of the last syntax element before value type
            // This is either: name_type, or type_parameter constraint
            let after_bracket_area = m
                .name_type
                .as_ref()
                .map_or_else(|| m.type_parameter.constraint.span().end, |n| n.span().end);

            // Comments between `]:` area and value type
            let comments_before_value: Vec<_> =
                comments_in_range(self.comments, after_bracket_area, type_start).collect();

            body_parts.push(doc::text(":"));
            for comment in &comments_before_value {
                body_parts.push(doc::text(" "));
                body_parts.push(self.build_comment_doc(comment));
            }

            body_parts.push(doc::text(" "));
            body_parts.push(self.build_type_doc(type_ann));

            // Trailing comments after value type (before `;` or `}`)
            let body_end = m.span.end.saturating_sub(1); // before `}`
            for comment in comments_in_range(self.comments, type_end, body_end) {
                body_parts.push(doc::text(" "));
                body_parts.push(self.build_comment_doc(comment));
            }
        } else {
            body_parts.push(doc::text(": "));
        }

        if source_is_multiline {
            // Multi-line source: preserve multi-line format with hardlines
            // Comments before mapping go on their own line
            let mut inner_parts = vec![];
            for comment in &comments_before_mapping {
                inner_parts.push(doc::hardline());
                inner_parts.push(self.build_comment_doc(comment));
            }
            inner_parts.push(doc::hardline());
            inner_parts.push(doc::concat(body_parts));
            inner_parts.push(doc::text(";"));

            doc::concat(vec![
                doc::text("{"),
                doc::indent(doc::concat(inner_parts)),
                doc::hardline(),
                doc::text("}"),
            ])
        } else {
            // One-line source: width-aware (stays inline if fits, wraps if too long)
            let mut all_parts = vec![doc::softline()];
            all_parts.extend(body_parts);
            all_parts.push(doc::if_break(doc::text(";"), doc::text("")));

            doc::group(doc::concat(vec![
                doc::text("{"),
                doc::indent(doc::concat(all_parts)),
                doc::softline(),
                doc::text("}"),
            ]))
        }
    }

    //
    // Tuple Types
    //

    /// Build a Doc for a tuple type: `[A, B, C]`
    ///
    /// Uses width-aware breaking: inline if fits, one element per line if not.
    pub(super) fn build_tuple_type_doc(&self, t: &TSTupleType) -> Doc {
        if t.element_types.is_empty() {
            return self.build_empty_brackets_with_comments_doc(t.span);
        }

        // Check for line comments between elements or after last element (force multiline)
        if self.has_line_comments_in_delimited_list(&t.element_types, TSType::span, t.span.end - 1)
        {
            return self.build_tuple_type_doc_with_line_comments(t);
        }

        // Build element docs with commas and line breaks
        let mut parts = Vec::new();
        for (i, elem) in t.element_types.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(","));
                parts.push(doc::line());
            }
            parts.push(self.build_type_doc(elem));
        }

        // Width-aware breaking: inline if fits, one-per-line if not
        let inner = doc::concat(vec![
            doc::softline(),
            doc::concat(parts),
            doc::trailing_comma(),
        ]);

        doc::group(doc::concat(vec![
            doc::text("["),
            doc::indent(inner),
            doc::softline(),
            doc::text("]"),
        ]))
    }

    /// Build tuple type with line comments between elements
    fn build_tuple_type_doc_with_line_comments(&self, t: &TSTupleType) -> Doc {
        let mut inner_parts = Vec::new();
        let mut prev_end = t.span.start + 1; // After the opening `[`

        for (i, elem) in t.element_types.iter().enumerate() {
            let elem_start = elem.span().start;
            let elem_end = elem.span().end;
            let is_last = i == t.element_types.len() - 1;

            // Leading comments
            inner_parts.extend(self.build_leading_comments_multiline(prev_end, elem_start));

            inner_parts.push(self.build_type_doc(elem));

            let next_boundary = if i + 1 < t.element_types.len() {
                t.element_types[i + 1].span().start
            } else {
                t.span.end - 1 // Before the closing `]`
            };

            // Trailing comma for all elements
            inner_parts.push(doc::text(","));

            // Trailing comments
            inner_parts.extend(self.build_trailing_comments_multiline(elem_end, next_boundary));

            // Hardline to separate from next element
            if !is_last {
                inner_parts.push(doc::hardline());
            }

            prev_end = next_boundary;
        }

        doc::concat(vec![
            doc::text("["),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(inner_parts)])),
            doc::hardline(),
            doc::text("]"),
        ])
    }

    //
    // Array Types
    //

    /// Build a Doc for an array type (e.g., `number[]`)
    pub(super) fn build_array_type_doc(&self, arr: &TSArrayType) -> Doc {
        let needs_parens = type_needs_parens_for_array_element(&arr.element_type);
        let element_doc = self.build_type_doc(&arr.element_type);
        if needs_parens {
            doc::concat(vec![doc::text("("), element_doc, doc::text(")[]")])
        } else {
            doc::concat(vec![element_doc, doc::text("[]")])
        }
    }

    //
    // Type Query and Entity Names
    //

    /// Build doc for type query expression name
    pub(super) fn build_type_query_expr_name_doc(
        &self,
        expr_name: &internal::TSTypeQueryExprName,
    ) -> Doc {
        match expr_name {
            internal::TSTypeQueryExprName::EntityName(entity) => {
                self.build_type_entity_name_doc(entity)
            }
            internal::TSTypeQueryExprName::Import(i) => {
                let mut parts = vec![doc::text("import(")];
                parts.push(self.build_literal_doc(&i.argument));
                parts.push(doc::text(")"));
                if let Some(qualifier) = &i.qualifier {
                    parts.push(doc::text("."));
                    parts.push(self.build_type_entity_name_doc(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    parts.push(self.build_type_parameter_instantiation_doc(type_args));
                }
                doc::concat(parts)
            }
        }
    }

    /// Build doc for type entity name
    pub(super) fn build_type_entity_name_doc(&self, name: &internal::TSEntityName) -> Doc {
        // Delegate to standalone function - doesn't need printer state
        super::super::build_entity_name_doc(name)
    }
}
