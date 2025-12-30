// Type annotation printing for TypeScript
//
// Handles printing of TypeScript-specific type syntax:
// - Type annotations (: Type)
// - Type keywords (number, string, boolean, etc.)
// - Future: Complex types (unions, intersections, generics, etc.)

use super::Printer;
use crate::ast::internal::{
    self, TSArrayType, TSIntersectionType, TSLiteralType, TSType, TSTypeParameter,
    TSTypeParameterDeclaration, TSUnionType, TemplateLiteralType,
};
use tsv_lang::SymbolResolver;
use tsv_lang::SymbolToU32;
use tsv_lang::comments_in_range;
use tsv_lang::doc::{self, Doc};

// =============================================================================
// Helper functions
// =============================================================================

/// Find the position of a separator character in the source between start and end,
/// skipping over comments. Returns Some(position) if found, None otherwise.
fn find_separator_position(source: &str, start: u32, end: u32, separator: u8) -> Option<u32> {
    let bytes = source.as_bytes();
    let mut pos = start as usize;
    let end = end as usize;

    while pos < end {
        let b = bytes[pos];
        if b == separator {
            return Some(pos as u32);
        }
        // Skip block comments: /* ... */
        if b == b'/' && pos + 1 < end && bytes[pos + 1] == b'*' {
            pos += 2;
            while pos + 1 < end {
                if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                    pos += 2;
                    break;
                }
                pos += 1;
            }
            continue;
        }
        // Skip line comments: // ...
        if b == b'/' && pos + 1 < end && bytes[pos + 1] == b'/' {
            pos += 2;
            while pos < end && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        pos += 1;
    }
    None
}

/// Find the position of a `|` pipe character, skipping comments.
#[inline]
fn find_pipe_position(source: &str, start: u32, end: u32) -> Option<u32> {
    find_separator_position(source, start, end, b'|')
}

/// Find the position of a `&` ampersand character, skipping comments.
#[inline]
fn find_ampersand_position(source: &str, start: u32, end: u32) -> Option<u32> {
    find_separator_position(source, start, end, b'&')
}

// =============================================================================
// Type parenthesization helpers
// =============================================================================

/// Recursively unwrap TSParenthesizedType to get the inner type.
fn unwrap_parenthesized(ts_type: &TSType) -> &TSType {
    match ts_type {
        TSType::Parenthesized(p) => unwrap_parenthesized(&p.type_annotation),
        _ => ts_type,
    }
}

/// Check if a type needs parentheses when used as the object in indexed access (`T[K]`).
/// Without parens: `A | B[K]` parses as `A | (B[K])`, not `(A | B)[K]`
fn type_needs_parens_for_indexed_access_object(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    // TypeOperator included: `(keyof T)[K]` is valid and different from `keyof T[K]`
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeQuery(_)
            | TSType::TypeOperator(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    )
}

/// Check if a type needs parentheses when used as the element type in an array (`T[]`).
/// Without parens: `A | B[]` parses as `A | (B[])`, not `(A | B)[]`
fn type_needs_parens_for_array_element(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    // TypeOperator excluded: `(readonly T)[]` is invalid TypeScript
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeQuery(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    )
}

/// Check if a type needs parentheses when used as the operand of a prefix type operator
/// (keyof, readonly, unique). Without parens: `keyof A | B` parses as `(keyof A) | B`
fn type_needs_parens_for_prefix_operator(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    matches!(inner, TSType::Union(_) | TSType::Intersection(_))
}

impl<'a> Printer<'a> {
    /// Print a TypeScript type annotation (e.g., `: number`)
    ///
    /// Handles comments between the colon and the type.
    /// For simple types with line comments, the comment is moved to after the type.
    /// For union/intersection types, the comment stays before and the type is indented.
    pub(super) fn print_type_annotation(&mut self, annotation: &internal::TSTypeAnnotation) {
        // Check for comments between `:` and the type
        let colon_end = annotation.span.start + 1; // After the `:`
        let type_start = annotation.type_annotation.span().start;

        // Check if there's a line comment between : and the type
        if self.has_line_comments_between(colon_end, type_start) {
            // Check if type is union (gets indented) or intersection (stays at same level)
            let is_union = matches!(&*annotation.type_annotation, TSType::Union(_));
            let is_intersection = matches!(&*annotation.type_annotation, TSType::Intersection(_));

            if is_union {
                // Line comment stays before union, type on new INDENTED line
                self.write(":");
                self.print_inline_comments_between(colon_end, type_start);
                self.write("\n");
                self.indent_level += 1;
                self.write_indent();
                self.print_type(&annotation.type_annotation);
                self.indent_level -= 1;
            } else if is_intersection {
                // Line comment stays before intersection, type on new line (NOT indented)
                self.write(":");
                self.print_inline_comments_between(colon_end, type_start);
                self.write("\n");
                self.write_indent();
                self.print_type(&annotation.type_annotation);
            } else {
                // Simple type: comment moves to after the type (prettier 3.7 behavior)
                self.write(": ");
                self.print_type(&annotation.type_annotation);
                self.write(";");
                self.print_inline_comments_between(colon_end, type_start);
            }
        } else {
            // Block comments stay inline (if any)
            self.write(": ");
            self.print_inline_comments_between(colon_end, type_start);
            self.print_type(&annotation.type_annotation);
        }
    }

    /// Check if a type annotation has a trailing line comment (between : and type)
    /// that should be moved after the type for simple types.
    ///
    /// Used by callers to know when NOT to print a trailing semicolon
    /// (because print_type_annotation already printed it).
    pub(super) fn type_annotation_has_trailing_comment(
        &self,
        annotation: &internal::TSTypeAnnotation,
    ) -> bool {
        let colon_end = annotation.span.start + 1;
        let type_start = annotation.type_annotation.span().start;
        // Simple types (not union/intersection) have the comment moved to after the type,
        // which includes the semicolon
        self.has_line_comments_between(colon_end, type_start)
            && !matches!(
                &*annotation.type_annotation,
                TSType::Union(_) | TSType::Intersection(_)
            )
    }

    /// Print type parameter declaration: `<T, U extends V = W>`
    pub(super) fn print_type_parameter_declaration(&mut self, decl: &TSTypeParameterDeclaration) {
        self.write("<");
        for (i, param) in decl.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_type_parameter(param);
        }
        self.write(">");
    }

    /// Print a single type parameter: `T`, `T extends U`, or `T extends U = V`
    /// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
    fn print_type_parameter(&mut self, param: &TSTypeParameter) {
        // Print modifiers in order: const, in, out
        if param.is_const {
            self.write("const ");
        }
        if param.is_in {
            self.write("in ");
        }
        if param.is_out {
            self.write("out ");
        }

        self.print_identifier(&param.name);

        if let Some(constraint) = &param.constraint {
            self.write(" extends ");
            self.print_type(constraint);
        }

        if let Some(default) = &param.default {
            self.write(" = ");
            self.print_type(default);
        }
    }

    /// Build doc for type parameter declaration: `<T, U extends V = W>`
    /// Non-wrapping version - always inline
    pub(super) fn build_type_parameter_declaration_doc(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        let param_docs: Vec<_> = decl
            .params
            .iter()
            .map(|param| self.build_type_parameter_doc(param))
            .collect();
        doc::concat(vec![
            doc::text("<"),
            doc::join(param_docs, ", "),
            doc::text(">"),
        ])
    }

    /// Build doc for type parameter declaration with wrapping support
    /// When the group breaks, each param goes on its own line with trailing comma
    pub(super) fn build_type_parameter_declaration_doc_wrapping(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        doc::group(self.build_type_parameter_declaration_doc_inner(decl))
    }

    /// Build doc for type parameter declaration - inner version without group wrapper
    /// Used when caller wants to control the group (e.g., interface header)
    pub(super) fn build_type_parameter_declaration_doc_inner(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        if decl.params.is_empty() {
            return doc::text("<>");
        }

        let docs: Vec<_> = decl
            .params
            .iter()
            .map(|param| self.build_type_parameter_doc(param))
            .collect();
        let inner_parts = doc::join_trailing(docs, doc::comma_line());

        doc::concat(vec![
            doc::text("<"),
            doc::indent_softline(inner_parts),
            doc::softline(),
            doc::text(">"),
        ])
    }

    /// Build doc for a single type parameter
    /// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
    pub(super) fn build_type_parameter_doc(&self, param: &TSTypeParameter) -> Doc {
        let mut parts = Vec::new();

        // Add modifiers in order: const, in, out
        if param.is_const {
            parts.push(doc::text("const "));
        }
        if param.is_in {
            parts.push(doc::text("in "));
        }
        if param.is_out {
            parts.push(doc::text("out "));
        }

        parts.push(doc::symbol(param.name.name.to_u32()));

        if let Some(constraint) = &param.constraint {
            parts.push(doc::text(" extends "));
            parts.push(self.build_type_doc(constraint));
        }

        if let Some(default) = &param.default {
            parts.push(doc::text(" = "));
            parts.push(self.build_type_doc(default));
        }

        doc::concat(parts)
    }

    /// Print type parameter instantiation (type arguments): `<T, U>`
    pub(super) fn print_type_parameter_instantiation(
        &mut self,
        inst: &internal::TSTypeParameterInstantiation,
    ) {
        self.write("<");
        for (i, param) in inst.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_type(param);
        }
        self.write(">");
    }

    /// Build doc for type parameter instantiation (type arguments): `<T, U>`
    ///
    /// Supports breaking to multiple lines when content is too long:
    /// ```typescript
    /// new Map<
    ///     VeryLongKeyType,
    ///     VeryLongValueType,
    /// >();
    /// ```
    ///
    /// Also preserves comments: `</* a */ T /* b */, U>`
    pub(super) fn build_type_parameter_instantiation_doc(
        &self,
        inst: &internal::TSTypeParameterInstantiation,
    ) -> Doc {
        if inst.params.is_empty() {
            return doc::text("<>");
        }

        // Build params with commas and line breaks
        // The doc printer's look-ahead (fits_with_lookahead) handles the decision
        // of whether to break based on what follows the type params.
        let mut param_parts = Vec::new();
        let mut prev_end = inst.span.start + 1; // After the opening `<`

        for (i, param) in inst.params.iter().enumerate() {
            let param_start = param.span().start;

            if i > 0 {
                param_parts.push(doc::text(","));
                param_parts.push(doc::line());
            }

            // Add leading block comments before this type argument
            for comment in comments_in_range(self.comments, prev_end, param_start) {
                if comment.is_block {
                    param_parts.push(doc::text_owned(format!("/*{}*/ ", comment.content)));
                }
            }

            param_parts.push(self.build_type_doc(param));

            // Add trailing block comments after this type argument
            let param_end = param.span().end;
            let next_boundary = if i + 1 < inst.params.len() {
                inst.params[i + 1].span().start
            } else {
                inst.span.end - 1 // Before the closing `>`
            };
            for comment in comments_in_range(self.comments, param_end, next_boundary) {
                if comment.is_block {
                    param_parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                }
            }

            // Update prev_end to next_boundary to avoid double-counting comments
            prev_end = next_boundary;
        }

        // Wrap in group with angle brackets and optional breaks
        doc::group(doc::concat(vec![
            doc::text("<"),
            doc::indent_softline(doc::concat(param_parts)),
            doc::softline(),
            doc::text(">"),
        ]))
    }

    /// Build a Doc for a type annotation (e.g., `: number`)
    ///
    /// Handles comments between the colon and the type.
    /// For simple types with line comments, the comment is moved to after the type.
    /// For union types, the comment stays before and the type is INDENTED.
    /// For intersection types, the comment stays before but the type is NOT indented.
    ///
    /// NOTE: For simple types with trailing comments, the caller must NOT add a semicolon
    /// since this function includes it. Use `type_annotation_has_trailing_comment_doc` to check.
    pub(super) fn build_type_annotation_doc(&self, annotation: &internal::TSTypeAnnotation) -> Doc {
        // Check for comments between `:` and the type
        let colon_end = annotation.span.start + 1; // After the `:`
        let type_start = annotation.type_annotation.span().start;

        // Check if there's a line comment between : and the type
        if self.has_line_comments_between(colon_end, type_start) {
            // Check if type is union (gets indented) or intersection (stays at same level)
            let is_union = matches!(&*annotation.type_annotation, TSType::Union(_));
            let is_intersection = matches!(&*annotation.type_annotation, TSType::Intersection(_));

            if is_union {
                // Line comment stays before union, type on new INDENTED line
                let comments_doc = self.build_inline_comments_between_doc(colon_end, type_start);
                doc::concat(vec![
                    doc::text(":"),
                    comments_doc,
                    doc::hardline(),
                    doc::indent(self.build_type_doc(&annotation.type_annotation)),
                ])
            } else if is_intersection {
                // Line comment stays before intersection, type on new line (NOT indented)
                let comments_doc = self.build_inline_comments_between_doc(colon_end, type_start);
                doc::concat(vec![
                    doc::text(":"),
                    comments_doc,
                    doc::hardline(),
                    self.build_type_doc(&annotation.type_annotation),
                ])
            } else {
                // Simple type: comment moves to after the type (prettier 3.7 behavior)
                // Include semicolon here to ensure comment is before it
                let comments_doc = self.build_inline_comments_between_doc(colon_end, type_start);
                doc::concat(vec![
                    doc::text(": "),
                    self.build_type_doc(&annotation.type_annotation),
                    doc::text(";"),
                    comments_doc,
                ])
            }
        } else {
            // Handle unions/intersections with width-based breaking
            // Short: `param: Type1 | Type2`
            // Long: `param:\n\t| Type1\n\t| Type2`
            //
            // This pattern matches index signature type annotation handling.
            // For unions/intersections, wrap in group + indent + line so they break after `:`
            // and inherit breaking from this context's group.
            match annotation.type_annotation.as_ref() {
                TSType::Union(u) => {
                    let type_doc = self.build_union_type_doc(u, false);
                    doc::group(doc::concat(vec![
                        doc::text(":"),
                        doc::indent(doc::concat(vec![
                            doc::line(), // space when flat, newline when broken
                            type_doc,
                        ])),
                    ]))
                }
                TSType::Intersection(i) => {
                    let type_doc = self.build_intersection_type_doc(i, false);
                    doc::group(doc::concat(vec![
                        doc::text(":"),
                        doc::indent(doc::concat(vec![doc::line(), type_doc])),
                    ]))
                }
                _ => {
                    // Block comments stay inline for simple types
                    let comments_doc =
                        self.build_inline_comments_between_doc(colon_end, type_start);
                    doc::concat(vec![
                        doc::text(": "),
                        comments_doc,
                        self.build_type_doc(&annotation.type_annotation),
                    ])
                }
            }
        }
    }

    /// Build type annotation doc with width-aware type argument wrapping.
    ///
    /// For `TypeReference<Args>`, uses `build_type_arguments_doc_wrapping` so
    /// type arguments wrap at width boundary. For other types, delegates to
    /// `build_type_annotation_doc`.
    ///
    /// Returns doc starting with `: ` (the annotation prefix).
    pub(super) fn build_type_annotation_doc_wrapping(
        &self,
        annotation: &internal::TSTypeAnnotation,
    ) -> Doc {
        if let TSType::TypeReference(r) = annotation.type_annotation.as_ref()
            && let Some(type_args) = &r.type_arguments
        {
            return doc::concat(vec![
                doc::text(": "),
                super::build_entity_name_doc(&r.type_name),
                self.build_type_arguments_doc_wrapping(type_args),
            ]);
        }
        self.build_type_annotation_doc(annotation)
    }

    /// Check if a type annotation has a trailing line comment (between : and type)
    /// that should be moved after the type for simple types.
    ///
    /// Used by callers to know when NOT to add a trailing semicolon
    /// (because build_type_annotation_doc already includes it).
    pub(super) fn type_annotation_has_trailing_comment_doc(
        &self,
        annotation: &internal::TSTypeAnnotation,
    ) -> bool {
        let colon_end = annotation.span.start + 1;
        let type_start = annotation.type_annotation.span().start;
        self.has_line_comments_between(colon_end, type_start)
            && !matches!(
                &*annotation.type_annotation,
                TSType::Union(_) | TSType::Intersection(_)
            )
    }

    /// Build a Doc for a TypeScript type expression
    pub(super) fn build_type_doc(&self, ts_type: &TSType) -> Doc {
        match ts_type {
            TSType::Keyword(kw) => doc::text_owned(kw.kind.as_str().to_string()),
            TSType::Literal(lit) => self.build_literal_type_doc(lit),
            TSType::Array(arr) => self.build_array_type_doc(arr),
            TSType::Union(u) => self.build_union_type_doc(u, true),
            TSType::Intersection(i) => self.build_intersection_type_doc(i, true),
            TSType::TypeReference(r) => {
                let mut parts = vec![self.build_type_entity_name_doc(&r.type_name)];
                if let Some(type_args) = &r.type_arguments {
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                doc::concat(parts)
            }
            TSType::TypeLiteral(t) => {
                // Check if original was multi-line (newline immediately after opening brace)
                // This matches prettier's behavior: `{ a: T }` → single-line, `{\n a: T; }` → multi-line
                let source_is_multiline = super::is_type_literal_multiline(self.source, t.span);

                let mut parts = vec![doc::text("{")];
                if !t.members.is_empty() {
                    if source_is_multiline {
                        // Multi-line format with leading comment handling (forced by source)
                        let mut member_parts = vec![];
                        let mut prev_end = t.span.start + 1; // after opening brace
                        for (i, m) in t.members.iter().enumerate() {
                            let is_first = i == 0;
                            // Build leading comments
                            let (comments_doc, has_blank) =
                                self.build_leading_line_comments_doc(prev_end, m.span().start);
                            let has_comments =
                                !matches!(&comments_doc, Doc::Concat(v) if v.is_empty());

                            // Add blank line if present (for blank line preservation)
                            // Use literalline (no indentation) for the blank line itself
                            if has_blank && !is_first {
                                member_parts.push(doc::literalline());
                            }

                            // Add hardline before comments (comments need to start on their own line)
                            if has_comments {
                                member_parts.push(doc::hardline());
                                member_parts.push(comments_doc);
                            }

                            member_parts.push(doc::hardline());
                            member_parts.push(self.build_type_member_doc(m));
                            prev_end = m.span().end;
                        }
                        parts.push(doc::indent(doc::concat(member_parts)));
                        parts.push(doc::hardline());
                    } else {
                        // Width-aware format: stays inline if fits, wraps if too long
                        // Flat: {prop: string; prop2: number}
                        // Broken: {\n\tprop: string;\n\tprop2: number;\n}
                        let mut member_parts = vec![];
                        for (i, m) in t.members.iter().enumerate() {
                            let is_last = i == t.members.len() - 1;

                            // Add line break before each member (softline for width-aware)
                            member_parts.push(doc::softline());
                            member_parts.push(self.build_type_member_doc_inner(m, false));

                            if is_last {
                                // Last member: semicolon only when broken
                                member_parts.push(doc::if_break(doc::text(";"), doc::text("")));
                            } else {
                                // Non-last: semicolon always, space only when flat
                                member_parts.push(doc::if_break(doc::text(";"), doc::text("; ")));
                            }
                        }
                        parts.push(doc::indent(doc::concat(member_parts)));
                        parts.push(doc::softline());
                    }
                }
                parts.push(doc::text("}"));
                // Wrap in group for width-aware breaking
                doc::group(doc::concat(parts))
            }
            TSType::Function(f) => {
                // Function types use width-aware wrapping similar to arrow functions:
                // group([type_params, "(", indent([softline, params...]), ifBreak(","), softline, ")", " => ", return_type])
                let mut parts = Vec::new();

                // Type parameters wrapped in their own group (can break independently)
                if let Some(type_params) = &f.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
                }

                // Function parameters
                parts.extend(self.build_function_params_doc(&f.params));

                parts.push(doc::text(" => "));
                parts.push(self.build_type_doc(&f.return_type.type_annotation));

                // Wrap entire function type in a group for width-aware breaking
                doc::group(doc::concat(parts))
            }
            TSType::Constructor(c) => {
                // Constructor types: `new () => T` or `abstract new <T>() => T`
                let mut parts = Vec::new();

                // Add 'abstract' keyword if present
                if c.abstract_ {
                    parts.push(doc::text("abstract "));
                }

                // Add 'new' keyword
                parts.push(doc::text("new "));

                // Type parameters wrapped in their own group (can break independently)
                if let Some(type_params) = &c.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
                }

                // Constructor parameters
                parts.extend(self.build_function_params_doc(&c.params));

                parts.push(doc::text(" => "));
                parts.push(self.build_type_doc(&c.return_type.type_annotation));

                // Wrap entire constructor type in a group for width-aware breaking
                doc::group(doc::concat(parts))
            }
            TSType::Tuple(t) => {
                if t.element_types.is_empty() {
                    return doc::text("[]");
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
            // Parenthesized types: just unwrap. Parent contexts (IndexedAccess, Array,
            // TypeOperator) add parens when needed based on the inner type.
            TSType::Parenthesized(p) => self.build_type_doc(&p.type_annotation),
            TSType::TypePredicate(p) => {
                let mut parts = vec![];
                if p.asserts {
                    parts.push(doc::text("asserts "));
                }
                parts.push(doc::symbol(p.parameter_name.name.to_u32()));
                if let Some(type_ann) = &p.type_annotation {
                    parts.push(doc::text(" is "));
                    parts.push(self.build_type_doc(type_ann));
                }
                doc::concat(parts)
            }
            TSType::Conditional(c) => {
                // Conditional types use width-aware wrapping:
                // When broken, ternary arms are indented:
                //   check extends extends_type
                //     ? true_type
                //     : false_type
                //
                // The outer-most conditional is wrapped in a group. Nested conditionals
                // (in true_type or false_type) are NOT wrapped in their own group - they
                // inherit breaking from the parent. This matches prettier's behavior.
                doc::group(self.build_conditional_type_doc_inner(c))
            }
            TSType::Mapped(m) => self.build_mapped_type_doc(m),
            TSType::TypeOperator(o) => {
                let needs_parens = type_needs_parens_for_prefix_operator(&o.type_annotation);
                let operand_doc = self.build_type_doc(&o.type_annotation);
                if needs_parens {
                    doc::concat(vec![
                        doc::text(o.operator.as_str()),
                        doc::text(" ("),
                        operand_doc,
                        doc::text(")"),
                    ])
                } else {
                    doc::concat(vec![
                        doc::text(o.operator.as_str()),
                        doc::text(" "),
                        operand_doc,
                    ])
                }
            }
            TSType::Import(i) => {
                let mut parts = vec![doc::text("import(")];
                parts.push(self.build_literal_doc(&i.argument));
                // Import type options
                if let Some(options) = &i.options {
                    parts.push(doc::text(", "));
                    parts.push(self.build_expression_doc(options));
                }
                parts.push(doc::text(")"));
                if let Some(qualifier) = &i.qualifier {
                    parts.push(doc::text("."));
                    parts.push(self.build_type_entity_name_doc(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                doc::concat(parts)
            }
            TSType::TypeQuery(q) => {
                let mut parts = vec![doc::text("typeof ")];
                parts.push(self.build_type_query_expr_name_doc(&q.expr_name));
                if let Some(type_args) = &q.type_arguments {
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                doc::concat(parts)
            }
            TSType::IndexedAccess(i) => {
                let object_doc = self.build_type_doc(&i.object_type);
                let needs_parens = type_needs_parens_for_indexed_access_object(&i.object_type);
                if needs_parens {
                    doc::concat(vec![
                        doc::text("("),
                        object_doc,
                        doc::text(")["),
                        self.build_type_doc(&i.index_type),
                        doc::text("]"),
                    ])
                } else {
                    doc::concat(vec![
                        object_doc,
                        doc::text("["),
                        self.build_type_doc(&i.index_type),
                        doc::text("]"),
                    ])
                }
            }
            TSType::Rest(r) => doc::concat(vec![
                doc::text("..."),
                self.build_type_doc(&r.type_annotation),
            ]),
            TSType::Optional(o) => doc::concat(vec![
                self.build_type_doc(&o.type_annotation),
                doc::text("?"),
            ]),
            TSType::NamedTupleMember(n) => {
                let mut parts = vec![doc::symbol(n.label.name.to_u32())];
                if n.optional {
                    parts.push(doc::text("?"));
                }
                parts.push(doc::text(": "));
                parts.push(self.build_type_doc(&n.element_type));
                doc::concat(parts)
            }
            TSType::Infer(i) => doc::concat(vec![
                doc::text("infer "),
                doc::symbol(i.type_parameter.name.name.to_u32()),
            ]),
        }
    }

    /// Build doc for conditional type WITHOUT the outer group wrapper.
    /// This is used for nested conditionals which should inherit breaking from their parent.
    ///
    /// Structure: `check extends extends_type [indent: line, "? ", true_type, line, ": ", false_type]`
    fn build_conditional_type_doc_inner(&self, c: &internal::TSConditionalType) -> Doc {
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

    /// Build doc for type query expression name
    fn build_type_query_expr_name_doc(&self, expr_name: &internal::TSTypeQueryExprName) -> Doc {
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
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                doc::concat(parts)
            }
        }
    }

    /// Build doc for mapped type: `{ [K in T]: V }`
    ///
    /// Width-aware: stays inline if fits, wraps if too long.
    /// - Flat: `{[K in keyof T]: T[K]}`
    /// - Broken: `{\n\t[K in keyof T]: T[K];\n}`
    fn build_mapped_type_doc(&self, m: &internal::TSMappedType) -> Doc {
        // Build the mapping body starting with softline for indent behavior
        let mut body_parts = vec![doc::softline()];

        // readonly modifier: `readonly` or `-readonly`
        if let Some(readonly) = m.readonly {
            body_parts.push(doc::text(if readonly { "readonly " } else { "-readonly " }));
        }

        // [K in constraint]
        body_parts.push(doc::text("["));
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

        body_parts.push(doc::text(": "));

        // value type
        if let Some(type_ann) = &m.type_annotation {
            body_parts.push(self.build_type_doc(type_ann));
        }

        // Semicolon only when broken (multiline)
        body_parts.push(doc::if_break(doc::text(";"), doc::text("")));

        // Width-aware structure: group(["{", indent([softline, body...]), softline, "}"])
        doc::group(doc::concat(vec![
            doc::text("{"),
            doc::indent(doc::concat(body_parts)),
            doc::softline(),
            doc::text("}"),
        ]))
    }

    /// Build doc for type entity name
    fn build_type_entity_name_doc(&self, name: &internal::TSEntityName) -> Doc {
        // Delegate to standalone function - doesn't need printer state
        super::build_entity_name_doc(name)
    }

    /// Build doc for type member with proper AST-based formatting (with trailing semicolon)
    fn build_type_member_doc(&self, member: &internal::TSTypeElement) -> Doc {
        self.build_type_member_doc_inner(member, true)
    }

    /// Build doc for type member with optional trailing semicolon
    fn build_type_member_doc_inner(
        &self,
        member: &internal::TSTypeElement,
        with_semicolon: bool,
    ) -> Doc {
        match member {
            internal::TSTypeElement::PropertySignature(prop) => {
                let mut parts = vec![];
                if prop.readonly {
                    parts.push(doc::text("readonly "));
                }
                if prop.computed {
                    parts.push(doc::text("["));
                    parts.push(self.build_expression_doc(&prop.key));
                    parts.push(doc::text("]"));
                } else {
                    parts.push(self.build_expression_doc(&prop.key));
                }
                if prop.optional {
                    parts.push(doc::text("?"));
                }
                if let Some(type_ann) = &prop.type_annotation {
                    // Use width-aware wrapping for TypeReference with type arguments
                    parts.push(self.build_type_annotation_doc_wrapping(type_ann));

                    // For simple types with trailing comments, the doc already includes semicolon
                    let has_trailing_comment =
                        self.type_annotation_has_trailing_comment_doc(type_ann);
                    if with_semicolon && !has_trailing_comment {
                        parts.push(doc::text(";"));
                    }
                } else if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::concat(parts)
            }
            internal::TSTypeElement::MethodSignature(method) => {
                let mut parts = vec![];
                // Print accessor keyword for get/set signatures
                match method.kind {
                    internal::MethodKind::Get => parts.push(doc::text("get ")),
                    internal::MethodKind::Set => parts.push(doc::text("set ")),
                    _ => {}
                }
                if method.computed {
                    parts.push(doc::text("["));
                    parts.push(self.build_expression_doc(&method.key));
                    parts.push(doc::text("]"));
                } else {
                    parts.push(self.build_expression_doc(&method.key));
                }
                if method.optional {
                    parts.push(doc::text("?"));
                }
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &method.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                let param_docs: Vec<_> = method
                    .params
                    .iter()
                    .map(|param| self.build_expression_doc(param))
                    .collect();
                parts.push(doc::text("("));
                parts.push(doc::join(param_docs, ", "));
                parts.push(doc::text(")"));
                if let Some(return_type) = &method.return_type {
                    parts.push(self.build_type_annotation_doc(return_type));
                }
                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::concat(parts)
            }
            internal::TSTypeElement::CallSignature(call) => {
                let mut parts = vec![];
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &call.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                let param_docs: Vec<_> = call
                    .params
                    .iter()
                    .map(|param| self.build_expression_doc(param))
                    .collect();
                parts.push(doc::text("("));
                parts.push(doc::join(param_docs, ", "));
                parts.push(doc::text(")"));
                if let Some(return_type) = &call.return_type {
                    parts.push(self.build_type_annotation_doc(return_type));
                }
                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::concat(parts)
            }
            internal::TSTypeElement::ConstructSignature(ctor) => {
                let mut parts = vec![doc::text("new ")];
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &ctor.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                let param_docs: Vec<_> = ctor
                    .params
                    .iter()
                    .map(|param| self.build_expression_doc(param))
                    .collect();
                parts.push(doc::text("("));
                parts.push(doc::join(param_docs, ", "));
                parts.push(doc::text(")"));
                if let Some(return_type) = &ctor.return_type {
                    parts.push(self.build_type_annotation_doc(return_type));
                }
                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::concat(parts)
            }
            internal::TSTypeElement::IndexSignature(idx) => {
                let mut parts = vec![];
                if idx.readonly {
                    parts.push(doc::text("readonly "));
                }

                // Build the key parameter docs
                // For key type annotations with unions/intersections, use special handling
                // so they break properly with leading |/trailing & style.
                let param_docs: Vec<_> =
                    idx.parameters
                        .iter()
                        .map(|param| {
                            let mut param_parts = vec![doc::symbol(param.name.to_u32())];
                            if let Some(type_ann) = &param.type_annotation {
                                match type_ann.type_annotation.as_ref() {
                                    TSType::Union(u) => {
                                        // Union key type: break after `:` with leading `|`
                                        let type_doc = self.build_union_type_doc(u, false);
                                        param_parts.push(doc::text(":"));
                                        param_parts.push(doc::group(doc::indent(doc::concat(
                                            vec![doc::line(), type_doc],
                                        ))));
                                    }
                                    TSType::Intersection(i) => {
                                        // Intersection key type: break after `:` with trailing `&`
                                        let type_doc = self.build_intersection_type_doc(i, false);
                                        param_parts.push(doc::text(":"));
                                        param_parts.push(doc::group(doc::indent(doc::concat(
                                            vec![doc::line(), type_doc],
                                        ))));
                                    }
                                    _ => {
                                        // Regular key type: use standard annotation
                                        param_parts.push(self.build_type_annotation_doc(type_ann));
                                    }
                                }
                            }
                            doc::concat(param_parts)
                        })
                        .collect();

                // Build `[key: type]` as a group that can break when key type is long
                // Flat: [key: type]
                // Break: [\n\tkey: type\n]
                let bracket_contents = doc::join(param_docs, ", ");
                let bracket_group = doc::group(doc::concat(vec![
                    doc::text("["),
                    doc::indent_softline(bracket_contents),
                    doc::softline(),
                    doc::text("]"),
                ]));
                parts.push(bracket_group);

                // Build value type annotation with proper breaking for long unions/intersections
                // Short: `[key: string]: Type`
                // Long union: `[key: string]:\n\t| Type1\n\t| Type2`
                // Long intersection: `[key: string]:\n\tType1 &\n\tType2`
                //
                // For unions/intersections, wrap in group + indent + line so they break after `:`
                // and inherit breaking from this context's group.
                // For other types, use standard `: Type` format.
                match idx.type_annotation.type_annotation.as_ref() {
                    TSType::Union(u) => {
                        let type_doc = self.build_union_type_doc(u, false);
                        parts.push(doc::text(":"));
                        parts.push(doc::group(doc::indent(doc::concat(vec![
                            doc::line(), // space when flat, newline when broken
                            type_doc,
                        ]))));
                    }
                    TSType::Intersection(i) => {
                        let type_doc = self.build_intersection_type_doc(i, false);
                        parts.push(doc::text(":"));
                        parts.push(doc::group(doc::indent(doc::concat(vec![
                            doc::line(),
                            type_doc,
                        ]))));
                    }
                    _ => {
                        parts.push(self.build_type_annotation_doc(&idx.type_annotation));
                    }
                }

                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::concat(parts)
            }
        }
    }

    /// Build a Doc for an array type (e.g., `number[]`)
    fn build_array_type_doc(&self, arr: &TSArrayType) -> Doc {
        let needs_parens = type_needs_parens_for_array_element(&arr.element_type);
        let element_doc = self.build_type_doc(&arr.element_type);
        if needs_parens {
            doc::concat(vec![doc::text("("), element_doc, doc::text(")[]")])
        } else {
            doc::concat(vec![element_doc, doc::text("[]")])
        }
    }

    /// Build parameter list docs for function/constructor types
    /// Returns docs that should be pushed to a parts vector
    fn build_function_params_doc(&self, params: &[internal::Expression]) -> Vec<Doc> {
        let mut parts = Vec::new();
        if params.is_empty() {
            parts.push(doc::text("()"));
        } else {
            let mut param_parts = Vec::new();
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    param_parts.push(doc::text(","));
                    param_parts.push(doc::line());
                }
                param_parts.push(self.build_expression_doc(p));
            }
            parts.push(doc::text("("));
            parts.push(doc::indent(doc::concat(vec![
                doc::softline(),
                doc::concat(param_parts),
            ])));
            // Trailing comma when breaking, UNLESS last param is a rest element
            let last_is_rest = params
                .last()
                .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));
            if !last_is_rest {
                parts.push(doc::trailing_comma());
            }
            parts.push(doc::softline());
            parts.push(doc::text(")"));
        }
        parts
    }

    /// Build a Doc for a literal type
    fn build_literal_type_doc(&self, lit: &TSLiteralType) -> Doc {
        match lit {
            TSLiteralType::TemplateLiteral(template) => {
                self.build_template_literal_type_doc(template)
            }
            TSLiteralType::String(literal) => self.build_literal_doc(literal),
            TSLiteralType::Number(literal) => self.build_literal_doc(literal),
            TSLiteralType::BigInt(literal) => self.build_literal_doc(literal),
            TSLiteralType::UnaryExpression(unary) => {
                // For negative number types like `-1`
                let op = doc::text(unary.operator.as_str());
                let arg = self.build_expression_doc(&unary.argument);
                doc::concat(vec![op, arg])
            }
        }
    }

    /// Build a Doc for a template literal type
    fn build_template_literal_type_doc(&self, template: &TemplateLiteralType) -> Doc {
        let mut parts = vec![doc::text("`")];
        for (i, quasi) in template.quasis.iter().enumerate() {
            parts.push(doc::text_owned(quasi.raw.clone()));
            if i < template.types.len() {
                parts.push(doc::text("${"));
                parts.push(self.build_type_doc(&template.types[i]));
                parts.push(doc::text("}"));
            }
        }
        parts.push(doc::text("`"));
        doc::concat(parts)
    }

    /// Build a Doc for a union type: `A | B | C` or `| A\n| B\n| C`
    ///
    /// When flat: `A | B | C`
    /// When broken: each type on its own line with leading `| `
    ///
    /// When `wrap_in_group` is true (default), wraps the union in its own group
    /// so that it makes independent breaking decisions. This is appropriate for
    /// most contexts (e.g., type parameter constraints should stay flat even
    /// when the parameter list breaks).
    ///
    /// When `wrap_in_group` is false, the union inherits breaking from its parent
    /// group. Use this for type alias values and index signature values where
    /// the union should break together with the parent context.
    pub(super) fn build_union_type_doc(&self, union: &TSUnionType, wrap_in_group: bool) -> Doc {
        if union.types.is_empty() {
            return doc::text("");
        }

        // Build parts: each type prefixed conditionally with `| ` or nothing
        // Flat: T1 | T2 | T3
        // Break: | T1
        //        | T2
        //        | T3
        let mut parts = Vec::new();

        for (i, t) in union.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

            if i > 0 {
                // Between types: newline + "| " when broken, " | " when flat
                // Use if_break with line() instead of hardline() to avoid triggering will_break
                parts.push(doc::if_break(
                    doc::concat(vec![doc::line(), doc::text("| ")]),
                    doc::text(" | "),
                ));

                // Add leading block comments for this type (after the `|` separator)
                // Find the `|` position by scanning backwards from type_start
                let prev_type_end = union.types[i - 1].span().end;
                let pipe_pos = find_pipe_position(self.source, prev_type_end, type_start);
                if let Some(pipe_pos) = pipe_pos {
                    for comment in comments_in_range(self.comments, pipe_pos + 1, type_start) {
                        if comment.is_block {
                            parts.push(doc::text_owned(format!("/*{}*/ ", comment.content)));
                        }
                    }
                }
            } else {
                // First type: "| " when broken, nothing when flat
                parts.push(doc::if_break(doc::text("| "), doc::text("")));
            }

            // Function, constructor, and conditional types have lower precedence than union,
            // so they need parentheses: `T | (() => void)` not `T | () => void`
            let inner = unwrap_parenthesized(t);
            if matches!(
                inner,
                TSType::Function(_) | TSType::Constructor(_) | TSType::Conditional(_)
            ) {
                parts.push(doc::text("("));
                parts.push(self.build_type_doc(t));
                parts.push(doc::text(")"));
            } else {
                parts.push(self.build_type_doc(t));
            }

            // Add trailing block comments after this type (before the next `|` separator)
            if i + 1 < union.types.len() {
                // Find the `|` position between this type and next
                let next_type_start = union.types[i + 1].span().start;
                let pipe_pos = find_pipe_position(self.source, type_end, next_type_start);
                if let Some(pipe_pos) = pipe_pos {
                    // Only include comments before the `|`
                    for comment in comments_in_range(self.comments, type_end, pipe_pos) {
                        if comment.is_block {
                            parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                        }
                    }
                }
            } else {
                // Last type - include all trailing comments up to union span end
                for comment in comments_in_range(self.comments, type_end, union.span.end) {
                    if comment.is_block {
                        parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                    }
                }
            }
        }

        if wrap_in_group {
            // Independent breaking decision
            doc::group(doc::concat(parts))
        } else {
            // Inherit breaking from parent group
            doc::concat(parts)
        }
    }

    /// Build a Doc for an intersection type: `A & B & C` or `A &\nB &\nC`
    ///
    /// Prettier formatting for intersection types differs from union types:
    /// - Flat: `A & B & C`
    /// - Break: `A &\n\tB &\n\tC` (trailing `&`, not leading)
    ///
    /// When `wrap_in_group` is true (default), wraps in its own group for
    /// independent breaking decisions. When false, inherits from parent.
    pub(super) fn build_intersection_type_doc(
        &self,
        intersection: &TSIntersectionType,
        wrap_in_group: bool,
    ) -> Doc {
        if intersection.types.is_empty() {
            return doc::text("");
        }

        // For intersection types, prettier uses trailing `&` when breaking
        // Flat: A & B & C
        // Break: A &
        //        B &
        //        C
        let mut parts = Vec::new();

        for (i, t) in intersection.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

            if i > 0 {
                // After separator: line when broken, space when flat
                parts.push(doc::line());

                // Add leading block comments for this type (after the `&` separator)
                let prev_type_end = intersection.types[i - 1].span().end;
                let amp_pos = find_ampersand_position(self.source, prev_type_end, type_start);
                if let Some(amp_pos) = amp_pos {
                    for comment in comments_in_range(self.comments, amp_pos + 1, type_start) {
                        if comment.is_block {
                            parts.push(doc::text_owned(format!("/*{}*/ ", comment.content)));
                        }
                    }
                }
            }

            // Union, function, constructor, and conditional have lower precedence, wrap in parens
            let inner = unwrap_parenthesized(t);
            if matches!(
                inner,
                TSType::Union(_)
                    | TSType::Function(_)
                    | TSType::Constructor(_)
                    | TSType::Conditional(_)
            ) {
                parts.push(doc::text("("));
                parts.push(self.build_type_doc(t));
                parts.push(doc::text(")"));
            } else {
                parts.push(self.build_type_doc(t));
            }

            // Add trailing block comments after this type (before the next `&` separator)
            if i + 1 < intersection.types.len() {
                let next_type_start = intersection.types[i + 1].span().start;
                let amp_pos = find_ampersand_position(self.source, type_end, next_type_start);
                if let Some(amp_pos) = amp_pos {
                    for comment in comments_in_range(self.comments, type_end, amp_pos) {
                        if comment.is_block {
                            parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                        }
                    }
                }
            } else {
                // Last type - include all trailing comments up to intersection span end
                for comment in comments_in_range(self.comments, type_end, intersection.span.end) {
                    if comment.is_block {
                        parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                    }
                }
            }

            if i < intersection.types.len() - 1 {
                // Separator: " &" after each type except the last
                parts.push(doc::text(" &"));
            }
        }

        if wrap_in_group {
            // Independent breaking decision
            doc::group(doc::concat(parts))
        } else {
            // Inherit breaking from parent group
            doc::concat(parts)
        }
    }

    /// Print a TypeScript type expression
    pub(super) fn print_type(&mut self, ts_type: &TSType) {
        match ts_type {
            TSType::Keyword(kw) => self.write(kw.kind.as_str()),
            TSType::Literal(lit) => self.print_literal_type(lit),
            TSType::Array(arr) => {
                self.print_type(&arr.element_type);
                self.write("[]");
            }
            TSType::Union(u) => {
                for (i, t) in u.types.iter().enumerate() {
                    if i > 0 {
                        self.write(" | ");
                    }
                    self.print_type(t);
                }
            }
            TSType::Intersection(i) => {
                for (idx, t) in i.types.iter().enumerate() {
                    if idx > 0 {
                        self.write(" & ");
                    }
                    self.print_type(t);
                }
            }
            TSType::TypeReference(r) => {
                self.print_type_entity_name(&r.type_name);
                if let Some(type_args) = &r.type_arguments {
                    self.write("<");
                    for (i, arg) in type_args.params.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.print_type(arg);
                    }
                    self.write(">");
                }
            }
            TSType::TypeLiteral(t) => {
                // Check if original was multi-line (newline immediately after opening brace)
                // This matches prettier's behavior: `{ a: T }` → single-line, `{\n a: T; }` → multi-line
                let source_text = t.span.extract(self.source);
                let after_brace = source_text.strip_prefix('{').unwrap_or("");
                let is_single_line = !after_brace.starts_with('\n')
                    && !after_brace.starts_with("\r\n")
                    && !after_brace.trim_start_matches(' ').starts_with('\n');

                self.write("{");
                if !t.members.is_empty() {
                    if is_single_line {
                        // Single-line format: {prop: string; prop2: number} (semicolon separator, no trailing)
                        for (i, m) in t.members.iter().enumerate() {
                            if i > 0 {
                                self.write("; ");
                            }
                            self.print_type_member_inline(m);
                        }
                    } else {
                        // Multi-line format
                        self.indent_level += 1;
                        let mut prev_end = t.span.start + 1; // after opening brace
                        for (i, m) in t.members.iter().enumerate() {
                            let is_first = i == 0;
                            // Print leading comments (including blank line preservation)
                            self.print_block_leading_comments(prev_end, m.span().start, is_first);
                            self.write_indent();
                            self.print_type_member(m);
                            self.write("\n");
                            prev_end = m.span().end;
                        }
                        self.indent_level -= 1;
                        self.write_indent();
                    }
                }
                self.write("}");
            }
            TSType::Function(f) => {
                // Print type parameters if present: <T, U>
                if let Some(type_params) = &f.type_parameters {
                    self.print_type_parameter_declaration(type_params);
                }
                self.write("(");
                for (i, p) in f.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expression(p);
                }
                self.write(") => ");
                self.print_type(&f.return_type.type_annotation);
            }
            TSType::Constructor(c) => {
                // Print 'abstract' if present
                if c.abstract_ {
                    self.write("abstract ");
                }
                // Print 'new'
                self.write("new ");
                // Print type parameters if present: <T, U>
                if let Some(type_params) = &c.type_parameters {
                    self.print_type_parameter_declaration(type_params);
                }
                self.write("(");
                for (i, p) in c.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expression(p);
                }
                self.write(") => ");
                self.print_type(&c.return_type.type_annotation);
            }
            TSType::Tuple(t) => {
                self.write("[");
                for (i, elem) in t.element_types.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_type(elem);
                }
                self.write("]");
            }
            TSType::Parenthesized(p) => {
                self.write("(");
                self.print_type(&p.type_annotation);
                self.write(")");
            }
            TSType::TypePredicate(p) => {
                if p.asserts {
                    self.write("asserts ");
                }
                self.print_identifier(&p.parameter_name);
                if let Some(type_ann) = &p.type_annotation {
                    self.write(" is ");
                    self.print_type(type_ann);
                }
            }
            TSType::Conditional(c) => {
                self.print_type(&c.check_type);
                self.write(" extends ");
                self.print_type(&c.extends_type);
                self.write(" ? ");
                self.print_type(&c.true_type);
                self.write(" : ");
                self.print_type(&c.false_type);
            }
            TSType::Mapped(m) => self.print_mapped_type(m),
            TSType::TypeOperator(o) => {
                self.write(o.operator.as_str());
                self.write(" ");
                self.print_type(&o.type_annotation);
            }
            TSType::Import(i) => {
                self.write("import(");
                self.print_literal(&i.argument);
                // Import type options
                if let Some(options) = &i.options {
                    self.write(", ");
                    self.print_expression(options);
                }
                self.write(")");
                if let Some(qualifier) = &i.qualifier {
                    self.write(".");
                    self.print_type_entity_name(qualifier);
                }
                if let Some(type_args) = &i.type_arguments {
                    self.write("<");
                    for (j, arg) in type_args.params.iter().enumerate() {
                        if j > 0 {
                            self.write(", ");
                        }
                        self.print_type(arg);
                    }
                    self.write(">");
                }
            }
            TSType::TypeQuery(q) => {
                self.write("typeof ");
                self.print_type_query_expr_name(&q.expr_name);
                if let Some(type_args) = &q.type_arguments {
                    self.write("<");
                    for (j, arg) in type_args.params.iter().enumerate() {
                        if j > 0 {
                            self.write(", ");
                        }
                        self.print_type(arg);
                    }
                    self.write(">");
                }
            }
            TSType::IndexedAccess(i) => {
                self.print_type(&i.object_type);
                self.write("[");
                self.print_type(&i.index_type);
                self.write("]");
            }
            TSType::Rest(r) => {
                self.write("...");
                self.print_type(&r.type_annotation);
            }
            TSType::Optional(o) => {
                self.print_type(&o.type_annotation);
                self.write("?");
            }
            TSType::NamedTupleMember(n) => {
                self.write(&self.resolve_symbol(n.label.name));
                if n.optional {
                    self.write("?");
                }
                self.write(": ");
                self.print_type(&n.element_type);
            }
            TSType::Infer(i) => {
                self.write("infer ");
                self.write(&self.resolve_symbol(i.type_parameter.name.name));
            }
        }
    }

    /// Print type query expression name
    fn print_type_query_expr_name(&mut self, expr_name: &internal::TSTypeQueryExprName) {
        match expr_name {
            internal::TSTypeQueryExprName::EntityName(entity) => {
                self.print_type_entity_name(entity);
            }
            internal::TSTypeQueryExprName::Import(i) => {
                self.write("import(");
                self.print_literal(&i.argument);
                self.write(")");
                if let Some(qualifier) = &i.qualifier {
                    self.write(".");
                    self.print_type_entity_name(qualifier);
                }
                if let Some(type_args) = &i.type_arguments {
                    self.write("<");
                    for (j, arg) in type_args.params.iter().enumerate() {
                        if j > 0 {
                            self.write(", ");
                        }
                        self.print_type(arg);
                    }
                    self.write(">");
                }
            }
        }
    }

    /// Print mapped type: `{ [K in T]: V }`
    fn print_mapped_type(&mut self, m: &internal::TSMappedType) {
        self.write("{");

        // readonly modifier
        if let Some(readonly) = m.readonly {
            if readonly {
                self.write("readonly ");
            } else {
                self.write("-readonly ");
            }
        }

        // [K in constraint]
        self.write("[");
        self.write(&m.type_parameter.name);
        self.write(" in ");
        self.print_type(&m.type_parameter.constraint);

        // as clause
        if let Some(name_type) = &m.name_type {
            self.write(" as ");
            self.print_type(name_type);
        }

        self.write("]");

        // optional modifier
        if let Some(optional) = m.optional {
            if optional {
                self.write("?");
            } else {
                self.write("-?");
            }
        }

        self.write(": ");

        // value type
        if let Some(type_ann) = &m.type_annotation {
            self.print_type(type_ann);
        }

        self.write("}");
    }

    /// Print type entity name
    fn print_type_entity_name(&mut self, name: &internal::TSEntityName) {
        match name {
            internal::TSEntityName::Identifier(id) => self.print_identifier(id),
            internal::TSEntityName::QualifiedName(qn) => {
                self.print_type_entity_name(&qn.left);
                self.write(".");
                self.print_identifier(&qn.right);
            }
        }
    }

    /// Print type member with proper AST-based formatting (with trailing semicolon)
    fn print_type_member(&mut self, member: &internal::TSTypeElement) {
        self.print_type_member_inner(member, true);
    }

    /// Print type member inline (without trailing semicolon, for single-line object types)
    fn print_type_member_inline(&mut self, member: &internal::TSTypeElement) {
        self.print_type_member_inner(member, false);
    }

    /// Print type member with optional trailing semicolon
    fn print_type_member_inner(&mut self, member: &internal::TSTypeElement, with_semicolon: bool) {
        match member {
            internal::TSTypeElement::PropertySignature(prop) => {
                if prop.readonly {
                    self.write("readonly ");
                }
                if prop.computed {
                    self.write("[");
                    self.print_expression(&prop.key);
                    self.write("]");
                } else {
                    self.print_expression(&prop.key);
                }
                if prop.optional {
                    self.write("?");
                }
                if let Some(type_ann) = &prop.type_annotation {
                    self.print_type_annotation(type_ann);
                }
                if with_semicolon {
                    self.write(";");
                }
            }
            internal::TSTypeElement::MethodSignature(method) => {
                // Print accessor keyword for get/set signatures
                match method.kind {
                    internal::MethodKind::Get => self.write("get "),
                    internal::MethodKind::Set => self.write("set "),
                    _ => {}
                }
                if method.computed {
                    self.write("[");
                    self.print_expression(&method.key);
                    self.write("]");
                } else {
                    self.print_expression(&method.key);
                }
                if method.optional {
                    self.write("?");
                }
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &method.type_parameters {
                    self.print_type_parameter_declaration(type_params);
                }
                self.write("(");
                for (i, param) in method.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expression(param);
                }
                self.write(")");
                if let Some(return_type) = &method.return_type {
                    self.print_type_annotation(return_type);
                }
                if with_semicolon {
                    self.write(";");
                }
            }
            internal::TSTypeElement::CallSignature(call) => {
                self.write("(");
                for (i, param) in call.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expression(param);
                }
                self.write(")");
                if let Some(return_type) = &call.return_type {
                    self.print_type_annotation(return_type);
                }
                if with_semicolon {
                    self.write(";");
                }
            }
            internal::TSTypeElement::ConstructSignature(ctor) => {
                self.write("new (");
                for (i, param) in ctor.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expression(param);
                }
                self.write(")");
                if let Some(return_type) = &ctor.return_type {
                    self.print_type_annotation(return_type);
                }
                if with_semicolon {
                    self.write(";");
                }
            }
            internal::TSTypeElement::IndexSignature(idx) => {
                if idx.readonly {
                    self.write("readonly ");
                }
                self.write("[");
                for (i, param) in idx.parameters.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    // print_identifier already prints type annotations, so we just call it
                    self.print_identifier(param);
                }
                self.write("]");
                self.print_type_annotation(&idx.type_annotation);
                if with_semicolon {
                    self.write(";");
                }
            }
        }
    }

    /// Print a TypeScript literal type (template literal types, etc.)
    fn print_literal_type(&mut self, lit: &TSLiteralType) {
        match lit {
            TSLiteralType::TemplateLiteral(template) => {
                self.print_template_literal_type(template);
            }
            TSLiteralType::String(literal) => {
                self.print_literal(literal);
            }
            TSLiteralType::Number(literal) => {
                self.print_literal(literal);
            }
            TSLiteralType::BigInt(literal) => {
                self.print_literal(literal);
            }
            TSLiteralType::UnaryExpression(unary) => {
                // For negative number types like `-1`
                self.write(unary.operator.as_str());
                self.print_expression(&unary.argument);
            }
        }
    }

    /// Print a template literal type: `hello ${string} world`
    fn print_template_literal_type(&mut self, template: &TemplateLiteralType) {
        self.write("`");

        for (i, quasi) in template.quasis.iter().enumerate() {
            // Print the raw template content (preserving escapes)
            self.write(&quasi.raw);

            // Print type interpolation if there's a corresponding type
            if i < template.types.len() {
                self.write("${");
                self.print_type(&template.types[i]);
                self.write("}");
            }
        }

        self.write("`");
    }

    /// Convert a type annotation to a string (for inline building)
    pub(super) fn type_annotation_to_string(
        &self,
        annotation: &internal::TSTypeAnnotation,
    ) -> String {
        format!(": {}", self.type_to_string(&annotation.type_annotation))
    }

    /// Convert a type to a string (for inline building)
    pub(super) fn type_to_string(&self, ts_type: &TSType) -> String {
        match ts_type {
            TSType::Keyword(kw) => kw.kind.as_str().to_string(),
            TSType::Literal(lit) => self.literal_type_to_string(lit),
            TSType::Array(arr) => format!("{}[]", self.type_to_string(&arr.element_type)),
            TSType::Union(u) => u
                .types
                .iter()
                .map(|t| self.type_to_string(t))
                .collect::<Vec<_>>()
                .join(" | "),
            TSType::Intersection(i) => i
                .types
                .iter()
                .map(|t| self.type_to_string(t))
                .collect::<Vec<_>>()
                .join(" & "),
            TSType::TypeReference(r) => {
                let name = self.entity_name_to_string(&r.type_name);
                if let Some(type_args) = &r.type_arguments {
                    let args: Vec<_> = type_args
                        .params
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect();
                    format!("{}<{}>", name, args.join(", "))
                } else {
                    name
                }
            }
            TSType::TypeLiteral(t) => t.span.extract(self.source).to_string(),
            TSType::Function(f) => f.span.extract(self.source).to_string(),
            TSType::Constructor(c) => c.span.extract(self.source).to_string(),
            TSType::Tuple(t) => {
                let elems: Vec<_> = t
                    .element_types
                    .iter()
                    .map(|e| self.type_to_string(e))
                    .collect();
                format!("[{}]", elems.join(", "))
            }
            TSType::Parenthesized(p) => {
                format!("({})", self.type_to_string(&p.type_annotation))
            }
            TSType::TypePredicate(p) => {
                let mut result = String::new();
                if p.asserts {
                    result.push_str("asserts ");
                }
                result.push_str(&self.resolve_symbol(p.parameter_name.name));
                if let Some(type_ann) = &p.type_annotation {
                    result.push_str(" is ");
                    result.push_str(&self.type_to_string(type_ann));
                }
                result
            }
            TSType::Conditional(c) => {
                format!(
                    "{} extends {} ? {} : {}",
                    self.type_to_string(&c.check_type),
                    self.type_to_string(&c.extends_type),
                    self.type_to_string(&c.true_type),
                    self.type_to_string(&c.false_type)
                )
            }
            TSType::Mapped(m) => self.mapped_type_to_string(m),
            TSType::TypeOperator(o) => {
                format!(
                    "{} {}",
                    o.operator.as_str(),
                    self.type_to_string(&o.type_annotation)
                )
            }
            TSType::Import(i) => {
                let mut result = format!("import({}", self.literal_to_string(&i.argument));
                // Import type options
                if let Some(options) = &i.options {
                    result.push_str(", ");
                    result.push_str(&self.expression_to_string(options));
                }
                result.push(')');
                if let Some(qualifier) = &i.qualifier {
                    result.push('.');
                    result.push_str(&self.entity_name_to_string(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    let args: Vec<_> = type_args
                        .params
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect();
                    result.push('<');
                    result.push_str(&args.join(", "));
                    result.push('>');
                }
                result
            }
            TSType::TypeQuery(q) => {
                let mut result = String::from("typeof ");
                result.push_str(&self.type_query_expr_name_to_string(&q.expr_name));
                if let Some(type_args) = &q.type_arguments {
                    let args: Vec<_> = type_args
                        .params
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect();
                    result.push('<');
                    result.push_str(&args.join(", "));
                    result.push('>');
                }
                result
            }
            TSType::IndexedAccess(i) => {
                format!(
                    "{}[{}]",
                    self.type_to_string(&i.object_type),
                    self.type_to_string(&i.index_type)
                )
            }
            TSType::Rest(r) => format!("...{}", self.type_to_string(&r.type_annotation)),
            TSType::Optional(o) => format!("{}?", self.type_to_string(&o.type_annotation)),
            TSType::NamedTupleMember(n) => {
                let label = self.resolve_symbol(n.label.name);
                let opt = if n.optional { "?" } else { "" };
                format!("{}{}: {}", label, opt, self.type_to_string(&n.element_type))
            }
            TSType::Infer(i) => {
                format!("infer {}", self.resolve_symbol(i.type_parameter.name.name))
            }
        }
    }

    /// Convert type query expression name to string
    fn type_query_expr_name_to_string(&self, expr_name: &internal::TSTypeQueryExprName) -> String {
        match expr_name {
            internal::TSTypeQueryExprName::EntityName(entity) => self.entity_name_to_string(entity),
            internal::TSTypeQueryExprName::Import(i) => {
                let mut result = format!("import({})", self.literal_to_string(&i.argument));
                if let Some(qualifier) = &i.qualifier {
                    result.push('.');
                    result.push_str(&self.entity_name_to_string(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    let args: Vec<_> = type_args
                        .params
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect();
                    result.push('<');
                    result.push_str(&args.join(", "));
                    result.push('>');
                }
                result
            }
        }
    }

    /// Convert mapped type to string
    fn mapped_type_to_string(&self, m: &internal::TSMappedType) -> String {
        let mut result = String::from("{");

        // readonly modifier
        if let Some(readonly) = m.readonly {
            if readonly {
                result.push_str("readonly ");
            } else {
                result.push_str("-readonly ");
            }
        }

        // [K in constraint]
        result.push('[');
        result.push_str(&m.type_parameter.name);
        result.push_str(" in ");
        result.push_str(&self.type_to_string(&m.type_parameter.constraint));

        // as clause
        if let Some(name_type) = &m.name_type {
            result.push_str(" as ");
            result.push_str(&self.type_to_string(name_type));
        }

        result.push(']');

        // optional modifier
        if let Some(optional) = m.optional {
            if optional {
                result.push('?');
            } else {
                result.push_str("-?");
            }
        }

        result.push_str(": ");

        // value type
        if let Some(type_ann) = &m.type_annotation {
            result.push_str(&self.type_to_string(type_ann));
        }

        result.push('}');
        result
    }

    /// Convert entity name to string
    fn entity_name_to_string(&self, name: &internal::TSEntityName) -> String {
        match name {
            internal::TSEntityName::Identifier(id) => self.resolve_symbol(id.name),
            internal::TSEntityName::QualifiedName(qn) => {
                format!(
                    "{}.{}",
                    self.entity_name_to_string(&qn.left),
                    self.resolve_symbol(qn.right.name)
                )
            }
        }
    }

    /// Convert a literal to a string
    fn literal_to_string(&self, literal: &internal::Literal) -> String {
        use super::expressions::normalize_number_literal;
        use tsv_lang::printing::{StringFormatOptions, format_string_literal};

        match &literal.value {
            internal::LiteralValue::Number(_) => {
                normalize_number_literal(literal.span.extract(self.source))
            }
            internal::LiteralValue::String { content: _, quote } => {
                let raw_literal = literal.span.extract(self.source);
                let raw_content = &raw_literal[1..raw_literal.len() - 1];
                format_string_literal(raw_content, *quote, StringFormatOptions::default())
            }
            internal::LiteralValue::BigInt(_) => {
                normalize_number_literal(literal.span.extract(self.source))
            }
            internal::LiteralValue::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
            internal::LiteralValue::Null => "null".to_string(),
            internal::LiteralValue::Undefined => "undefined".to_string(),
        }
    }

    /// Convert a literal type to a string
    fn literal_type_to_string(&self, lit: &TSLiteralType) -> String {
        match lit {
            TSLiteralType::TemplateLiteral(template) => {
                self.template_literal_type_to_string(template)
            }
            TSLiteralType::String(literal) => self.literal_to_string(literal),
            TSLiteralType::Number(literal) => self.literal_to_string(literal),
            TSLiteralType::BigInt(literal) => self.literal_to_string(literal),
            TSLiteralType::UnaryExpression(unary) => {
                format!(
                    "{}{}",
                    unary.operator.as_str(),
                    self.expression_to_string(&unary.argument)
                )
            }
        }
    }

    /// Convert a template literal type to a string
    fn template_literal_type_to_string(&self, template: &TemplateLiteralType) -> String {
        let mut result = String::from("`");
        for (i, quasi) in template.quasis.iter().enumerate() {
            result.push_str(&quasi.raw);
            if i < template.types.len() {
                result.push_str("${");
                result.push_str(&self.type_to_string(&template.types[i]));
                result.push('}');
            }
        }
        result.push('`');
        result
    }
}
