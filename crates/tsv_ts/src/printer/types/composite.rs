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
    self, TSArrayType, TSConditionalType, TSMappedType, TSMappedTypeModifier, TSTupleType, TSType,
};
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    //
    // Conditional Types
    //

    /// Build doc for conditional type WITHOUT the outer group wrapper.
    /// This is used for nested conditionals which should inherit breaking from their parent.
    ///
    /// Structure: `check extends extends_type [indent: line, "? ", true_type, line, ": ", false_type]`
    pub(super) fn build_conditional_type_doc_inner(&self, c: &TSConditionalType) -> DocId {
        let d = self.d();
        // Build true_type doc: if it's a conditional (possibly wrapped in parens), don't wrap in group
        // Add parens for readability only when flat (single-line), not when broken (multi-line)
        let true_type_doc =
            if let TSType::Conditional(inner) = unwrap_parenthesized(c.true_type.as_ref()) {
                // Nested conditional in true position:
                // - Flat: add parens for readability: `T extends A ? (T extends B ? C : D) : E`
                // - Broken: no parens (the line breaks provide clarity)
                let inner_doc = self.build_conditional_type_doc_inner(inner);
                if d.will_break(inner_doc) {
                    // Inner doc forces breaking — use broken layout directly
                    inner_doc
                } else {
                    d.if_break(inner_doc, d.parens(inner_doc))
                }
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
                d.text(" ")
            } else {
                let mut parts = Vec::new();
                for (i, t) in union.types.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.if_break(d.concat(&[d.line(), d.text("| ")]), d.text(" | ")));
                    } else {
                        // First type: line() + "| " when broken, space when flat
                        parts.push(d.if_break(d.concat(&[d.line(), d.text("| ")]), d.text(" ")));
                    }
                    parts.push(self.build_type_doc(t));
                }
                d.group(d.indent(d.concat(&parts)))
            }
        } else {
            d.concat(&[d.text(" "), self.build_type_doc(&c.extends_type)])
        };

        d.concat(&[
            self.build_type_doc(&c.check_type),
            d.text(" extends"),
            extends_type_doc,
            d.indent(d.concat(&[
                d.line(),
                d.text("? "),
                true_type_doc,
                d.line(),
                d.text(": "),
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
    pub(super) fn build_mapped_type_doc(&self, m: &TSMappedType) -> DocId {
        let d = self.d();
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

        // readonly modifier: `readonly`, `+readonly`, or `-readonly`
        if let Some(readonly) = m.readonly {
            body_parts.push(d.text(match readonly {
                TSMappedTypeModifier::True => "readonly ",
                TSMappedTypeModifier::Plus => "+readonly ",
                TSMappedTypeModifier::Minus => "-readonly ",
            }));
        }

        // [K in constraint]
        body_parts.push(d.text("["));

        // For single-line: comments go after `[`
        if !source_is_multiline {
            for comment in &comments_before_mapping {
                body_parts.push(self.build_comment_doc(comment));
                body_parts.push(d.text(" "));
            }
        }

        body_parts.push(d.text_owned(m.type_parameter.name.clone()));
        body_parts.push(d.text(" in "));
        body_parts.push(self.build_type_doc(&m.type_parameter.constraint));

        // as clause: `as NewKeyType`
        if let Some(name_type) = &m.name_type {
            body_parts.push(d.text(" as "));
            body_parts.push(self.build_type_doc(name_type));
        }

        body_parts.push(d.text("]"));

        // optional modifier: `?`, `+?`, or `-?`
        if let Some(optional) = m.optional {
            body_parts.push(d.text(match optional {
                TSMappedTypeModifier::True => "?",
                TSMappedTypeModifier::Plus => "+?",
                TSMappedTypeModifier::Minus => "-?",
            }));
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

            body_parts.push(d.text(":"));
            for comment in &comments_before_value {
                body_parts.push(d.text(" "));
                body_parts.push(self.build_comment_doc(comment));
            }

            // When the value type is a union with line comments between members,
            // break after `:` and indent the union members (matching prettier's
            // `shouldIndent` → `indent(parts)` in `printUnionType`).
            if let TSType::Union(u) = type_ann.as_ref() {
                if self.union_has_line_comments_between_members(u) {
                    let type_doc = self.build_union_type_doc(u, false);
                    body_parts.push(d.group(d.indent(d.concat(&[d.line(), type_doc]))));
                } else {
                    body_parts.push(d.text(" "));
                    body_parts.push(self.build_type_doc(type_ann));
                }
            } else {
                body_parts.push(d.text(" "));
                body_parts.push(self.build_type_doc(type_ann));
            }

            // Trailing comments after value type (before `;` or `}`)
            let body_end = m.span.end.saturating_sub(1); // before `}`
            for comment in comments_in_range(self.comments, type_end, body_end) {
                body_parts.push(d.text(" "));
                body_parts.push(self.build_comment_doc(comment));
            }
        } else {
            body_parts.push(d.text(": "));
        }

        if source_is_multiline {
            // Multi-line source: preserve multi-line format with hardlines
            // Comments before mapping go on their own line
            let mut inner_parts = vec![];
            for comment in &comments_before_mapping {
                inner_parts.push(d.hardline());
                inner_parts.push(self.build_comment_doc(comment));
            }
            inner_parts.push(d.hardline());
            inner_parts.push(d.concat(&body_parts));
            inner_parts.push(d.text(";"));

            d.concat(&[
                d.text("{"),
                d.indent(d.concat(&inner_parts)),
                d.hardline(),
                d.text("}"),
            ])
        } else {
            // One-line source: width-aware (stays inline if fits, wraps if too long)
            let mut all_parts = vec![d.softline()];
            all_parts.extend(body_parts);
            all_parts.push(d.if_break(d.text(";"), d.empty()));

            d.group(d.concat(&[
                d.text("{"),
                d.indent(d.concat(&all_parts)),
                d.softline(),
                d.text("}"),
            ]))
        }
    }

    //
    // Tuple Types
    //

    /// Build a Doc for a tuple type: `[A, B, C]`
    ///
    /// Uses width-aware breaking: inline if fits, one element per line if not.
    pub(super) fn build_tuple_type_doc(&self, t: &TSTupleType) -> DocId {
        let d = self.d();
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
                parts.push(d.text(","));
                parts.push(d.line());
            }
            parts.push(self.build_type_doc(elem));
        }

        // Width-aware breaking: inline if fits, one-per-line if not
        let inner = d.concat(&[d.softline(), d.concat(&parts), d.trailing_comma()]);

        d.group(d.concat(&[d.text("["), d.indent(inner), d.softline(), d.text("]")]))
    }

    /// Build tuple type with line comments between elements
    fn build_tuple_type_doc_with_line_comments(&self, t: &TSTupleType) -> DocId {
        let d = self.d();
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
            inner_parts.push(d.text(","));

            // Trailing comments
            inner_parts.extend(self.build_trailing_comments_multiline(elem_end, next_boundary));

            // Hardline to separate from next element
            if !is_last {
                inner_parts.push(d.hardline());
            }

            prev_end = next_boundary;
        }

        d.concat(&[
            d.text("["),
            d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])),
            d.hardline(),
            d.text("]"),
        ])
    }

    //
    // Array Types
    //

    /// Build a Doc for an array type (e.g., `number[]`)
    pub(super) fn build_array_type_doc(&self, arr: &TSArrayType) -> DocId {
        let d = self.d();
        let needs_parens = type_needs_parens_for_array_element(&arr.element_type);
        let element_doc = self.build_type_doc(&arr.element_type);
        if needs_parens {
            d.concat(&[d.text("("), element_doc, d.text(")[]")])
        } else {
            d.concat(&[element_doc, d.text("[]")])
        }
    }

    //
    // Type Query and Entity Names
    //

    /// Build doc for type query expression name
    pub(super) fn build_type_query_expr_name_doc(
        &self,
        expr_name: &internal::TSTypeQueryExprName,
    ) -> DocId {
        let d = self.d();
        match expr_name {
            internal::TSTypeQueryExprName::EntityName(entity) => {
                self.build_type_entity_name_doc(entity)
            }
            internal::TSTypeQueryExprName::Import(i) => {
                let mut parts = vec![d.text("import(")];
                parts.push(self.build_literal_doc(&i.argument));
                parts.push(d.text(")"));
                if let Some(qualifier) = &i.qualifier {
                    parts.push(d.text("."));
                    parts.push(self.build_type_entity_name_doc(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    parts.push(self.build_type_parameter_instantiation_doc(type_args));
                }
                d.concat(&parts)
            }
        }
    }

    /// Build doc for type entity name
    pub(super) fn build_type_entity_name_doc(&self, name: &internal::TSEntityName) -> DocId {
        // Delegate to standalone function - doesn't need printer state
        super::super::build_entity_name_doc(self.d(), name)
    }
}
