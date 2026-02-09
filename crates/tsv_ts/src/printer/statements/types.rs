// Type-related statement printing for TypeScript

use super::{
    Printer, build_entity_name_doc, intersection_has_huggable_last_type, unwrap_parenthesized,
};
use crate::ast::internal::{self, TSType};
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::{Comment, SymbolToU32, comments_in_range};

/// Check if a type is "generic" - i.e., has type parameters.
/// This matches prettier's `isGeneric` function in assignment.js.
fn is_generic_type(ts_type: &TSType) -> bool {
    match ts_type {
        TSType::Function(f) => f.type_parameters.is_some(),
        TSType::TypeReference(r) => r.type_arguments.is_some(),
        _ => false,
    }
}

/// Check if we should break before the conditional type in a type alias.
/// Returns true if either checkType or extendsType has type parameters.
/// This matches prettier's `shouldBreakBeforeConditionalType` in assignment.js.
fn should_break_before_conditional_type(conditional: &internal::TSConditionalType) -> bool {
    is_generic_type(&conditional.check_type) || is_generic_type(&conditional.extends_type)
}

/// Returns true if the type has its own internal breaking mechanism
/// (e.g., braces, brackets, parentheses) and should NOT break after `=`.
fn type_has_internal_breaking(ts_type: &TSType) -> bool {
    match ts_type {
        TSType::TypeLiteral(_)
        | TSType::Mapped(_)
        | TSType::Tuple(_)
        | TSType::Function(_)
        | TSType::Constructor(_) => true,
        // TypeReference with type arguments has internal breaking via `<>`
        TSType::TypeReference(r) => r.type_arguments.is_some(),
        _ => false,
    }
}

/// Build a fluid-style doc that can break after `=` when the line is too long.
/// Flat: ` <type>`, Broken: `\n\t<type>`
fn fluid_assignment_doc(d: &DocArena, type_doc: DocId) -> DocId {
    d.group(d.indent_line(type_doc))
}

impl<'a> Printer<'a> {
    /// Check if a comment spans multiple lines (block comment with newlines)
    fn is_multiline_comment(&self, comment: &Comment) -> bool {
        comment.is_block && !self.is_same_line(comment.span.start, comment.span.end)
    }

    /// Build a doc for type alias declaration with proper line breaking
    ///
    /// For union types that don't fit on one line:
    /// ```text
    /// type VeryLongTypeName =
    ///     | Type1
    ///     | Type2
    ///     | Type3;
    /// ```
    ///
    /// For intersection types that don't fit on one line:
    /// ```text
    /// type VeryLongTypeName = FirstType &
    ///     SecondType &
    ///     ThirdType;
    /// ```
    pub(super) fn build_type_alias_declaration_doc(
        &self,
        decl: &internal::TSTypeAliasDeclaration,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text("type ")];
        parts.push(d.symbol(decl.id.name.to_u32()));

        // Check if type parameters are complex (>1 param with constraints/defaults)
        // Complex type params use break-lhs layout: params break, not the RHS
        let has_complex_params = self.type_alias_has_complex_params(decl.type_parameters.as_ref());

        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        parts.push(d.text(" ="));

        // Check the type kind for different formatting rules
        // For union/intersection types, build without their own group so they inherit
        // breaking from this context's group.
        if let TSType::Union(u) = &decl.type_annotation {
            // Union types: break after `=` with leading `| `
            let type_doc = self.build_union_type_doc(u, false);
            parts.push(fluid_assignment_doc(d, type_doc));
        } else if let TSType::Intersection(i) = &decl.type_annotation {
            // Intersection types: first element stays inline, subsequent wrap with indent
            // Special case: when the last type is a TypeLiteral (huggable), don't add indent
            let type_doc = self.build_intersection_type_doc(i, false);
            parts.push(d.text(" "));
            if intersection_has_huggable_last_type(i) {
                parts.push(type_doc);
            } else {
                parts.push(d.group(d.indent(type_doc)));
            }
        } else if let TSType::Conditional(cond) = &decl.type_annotation {
            // Conditional types: break after `=` only if check/extends has type parameters
            let type_doc = self.build_type_doc(&decl.type_annotation);
            if should_break_before_conditional_type(cond) {
                parts.push(fluid_assignment_doc(d, type_doc));
            } else {
                parts.push(d.text(" "));
                parts.push(type_doc);
            }
        } else if type_has_internal_breaking(&decl.type_annotation) {
            // Types with internal breaking (braces, brackets, parens, angle brackets) stay hugged
            // Use wrapping version so TypeReference type args break internally when too long
            let type_doc = self.build_type_doc_with_wrapping_type_args(&decl.type_annotation);
            parts.push(d.text(" "));
            parts.push(type_doc);
        } else if has_complex_params {
            // Complex type parameters: use break-lhs layout
            // Type params break, `=` stays on same line, RHS stays inline
            // Example: type Foo<T extends string, U = number> = SomeLongType;
            // Breaks as:
            //   type Foo<
            //     T extends string,
            //     U = number,
            //   > = SomeLongType;
            let type_doc = self.build_type_doc(&decl.type_annotation);
            parts.push(d.text(" "));
            parts.push(type_doc);
        } else {
            // Other types: fluid layout - can break after `=` when line is too long
            let type_doc = self.build_type_doc(&decl.type_annotation);
            parts.push(fluid_assignment_doc(d, type_doc));
        }

        parts.push(d.text(";"));

        d.concat(&parts)
    }

    /// Build doc for interface declaration
    ///
    /// Uses group mode when extends has multiple items - heritage breaks when group breaks.
    pub(super) fn build_interface_declaration_doc(
        &self,
        decl: &internal::TSInterfaceDeclaration,
    ) -> DocId {
        let d = self.d();
        // Group mode: multiple extends items
        let group_mode = decl.extends.len() > 1;

        let mut header_parts = vec![d.text("interface ")];
        header_parts.push(d.symbol(decl.id.name.to_u32()));

        // Build extends doc
        let extends_doc = if !decl.extends.is_empty() {
            let heritage_docs: Vec<_> = decl
                .extends
                .iter()
                .map(|heritage| {
                    let mut h_parts = vec![self.build_entity_name_doc(&heritage.expression)];
                    if let Some(type_args) = &heritage.type_arguments {
                        h_parts.push(self.build_type_arguments_doc(type_args));
                    }
                    d.concat(&h_parts)
                })
                .collect();
            Some(d.concat(&[d.text("extends "), d.join(heritage_docs, ", ")]))
        } else {
            None
        };

        // Build the header group (without body - body has hardlines that would force breaking)
        let header_doc = if group_mode {
            // Group mode: one unified group - when it breaks, extends breaks too
            if let Some(type_params) = &decl.type_parameters {
                header_parts
                    .push(self.build_type_parameter_declaration_doc_inline_group(type_params));
            }

            // Extends clause with line break
            if let Some(ext_doc) = extends_doc {
                header_parts.push(d.indent_line(ext_doc));
            }

            d.group(d.concat(&header_parts))
        } else {
            // Non-group mode: type params break independently, extends stays inline
            if let Some(type_params) = &decl.type_parameters {
                header_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
            }

            // Extends clause stays inline
            if let Some(ext_doc) = extends_doc {
                header_parts.push(d.text(" "));
                header_parts.push(ext_doc);
            }

            d.concat(&header_parts)
        };

        // Build body separately (outside the header group)
        let mut parts = vec![header_doc, d.text(" ")];

        if decl.body.body.is_empty() {
            parts.push(self.build_empty_body_with_comments_doc(decl.body.span));
        } else {
            parts.push(d.text("{"));
            parts.push(d.indent(d.concat(&[self.build_type_elements_doc(
                &decl.body.body,
                decl.body.span.start,
                decl.body.span.end,
            )])));
            parts.push(d.hardline());
            parts.push(d.text("}"));
        }

        d.concat(&parts)
    }

    /// Build doc for declare function with wrapping support for type parameters
    pub(super) fn build_declare_function_doc(&self, decl: &internal::TSDeclareFunction) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Handle async keyword
        if decl.r#async {
            parts.push(d.text("async "));
        }

        // Handle declare keyword (only for top-level declare functions,
        // not inside `declare namespace` where it's implicit)
        if decl.declare {
            parts.push(d.text("declare "));
        }

        // Handle function/function* keyword
        if decl.generator {
            parts.push(d.text("function* "));
        } else {
            parts.push(d.text("function "));
        }

        parts.push(d.symbol(decl.id.name.to_u32()));

        // Type parameters with wrapping support
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Function parameters with width-based breaking
        // Find paren position for comment handling
        let paren_search_start = decl
            .type_parameters
            .as_ref()
            .map_or(decl.id.span.end, |tp| tp.span.end);
        let paren_pos = self.source[paren_search_start as usize..]
            .find('(')
            .map(|p| paren_search_start + p as u32);
        parts.push(self.build_signature_params_doc(&decl.params, paren_pos));

        // Return type
        if let Some(return_type) = &decl.return_type {
            parts.push(d.text(": "));
            parts.push(self.build_type_doc(&return_type.type_annotation));
        }

        parts.push(d.text(";"));

        d.group(d.concat(&parts))
    }

    /// Build doc for entity name
    pub(super) fn build_entity_name_doc(&self, name: &internal::TSEntityName) -> DocId {
        // Delegate to standalone function - doesn't need printer state
        build_entity_name_doc(self.d(), name)
    }

    /// Build doc for a type used as a type argument.
    ///
    /// For single type arg contexts, uses normal doc (allows object types to break).
    /// For multiple type arg contexts, uses hugging (objects don't break independently).
    fn build_type_arg_doc(&self, param: &TSType, is_multi_arg: bool) -> DocId {
        if is_multi_arg {
            self.build_type_doc_for_type_arg(param)
        } else {
            self.build_type_doc(param)
        }
    }

    /// Build doc for type arguments with comment preservation: `</* a */ T /* b */, U>`
    ///
    /// Default version: no independent width-based wrapping (parent context controls breaking).
    /// Use `build_type_arguments_doc_wrapping` when type args should break independently.
    pub(crate) fn build_type_arguments_doc(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> DocId {
        let d = self.d();
        if args.params.is_empty() {
            return d.text("<>");
        }

        // Check for line comments between arguments or after last argument (force multiline)
        if self.has_line_comments_in_delimited_list(&args.params, TSType::span, args.span.end - 1) {
            return self.build_type_arguments_doc_with_line_comments(args);
        }

        let mut parts = Vec::new();
        let mut prev_end = args.span.start + 1; // After the opening `<`

        for (i, param) in args.params.iter().enumerate() {
            let param_start = param.span().start;

            if i > 0 {
                parts.push(d.text(", "));
            }

            // Add leading block comments before this type argument
            for comment in comments_in_range(self.comments, prev_end, param_start) {
                if comment.is_block {
                    parts.push(d.text_owned(format!("/*{}*/ ", comment.content)));
                }
            }

            parts.push(self.build_type_arg_doc(param, args.params.len() > 1));

            // Add trailing block comments after this type argument
            let param_end = param.span().end;
            let next_boundary = if i + 1 < args.params.len() {
                args.params[i + 1].span().start
            } else {
                args.span.end - 1 // Before the closing `>`
            };
            for comment in comments_in_range(self.comments, param_end, next_boundary) {
                if comment.is_block {
                    parts.push(d.text_owned(format!(" /*{}*/", comment.content)));
                }
            }

            // Update prev_end to next_boundary to avoid double-counting comments
            prev_end = next_boundary;
        }

        d.concat(&[d.text("<"), d.concat(&parts), d.text(">")])
    }

    /// Build doc for type arguments with width-based wrapping support.
    ///
    /// Inline: `<T, U, V>`
    /// Wrapped: `<\n\tT,\n\tU,\n\tV\n>`
    ///
    /// Special case: single TypeLiteral argument hugs the opening `<`:
    /// `Array<{prop: string}>` stays hugged, and when broken:
    /// ```text
    /// Array<{
    ///     prop: string;
    /// }>
    /// ```
    ///
    /// Use this when type arguments should break independently of parent context,
    /// such as in property type annotations.
    pub(crate) fn build_type_arguments_doc_wrapping(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> DocId {
        let d = self.d();
        if args.params.is_empty() {
            return d.text("<>");
        }

        // Check for line comments between arguments or after last argument (force multiline)
        if self.has_line_comments_in_delimited_list(&args.params, TSType::span, args.span.end - 1) {
            return self.build_type_arguments_doc_with_line_comments(args);
        }

        // Special case: single brace-delimited type argument (TypeLiteral or Mapped, possibly
        // parenthesized) - hug `<{` together. These types handle their own internal breaking,
        // so we don't need extra softlines/indents around them.
        if args.params.len() == 1 {
            let is_huggable = matches!(
                unwrap_parenthesized(&args.params[0]),
                TSType::TypeLiteral(_) | TSType::Mapped(_)
            );
            if is_huggable {
                let mut parts = vec![d.text("<")];

                // Include leading comments: `Array</* comment */ {...}>`
                let param_start = args.params[0].span().start;
                let param_end = args.params[0].span().end;
                let after_open = args.span.start + 1; // After the opening `<`
                let before_close = args.span.end - 1; // Before the closing `>`
                for comment in comments_in_range(self.comments, after_open, param_start) {
                    if comment.is_block {
                        parts.push(d.text_owned(format!("/*{}*/ ", comment.content)));
                    }
                }

                parts.push(self.build_type_arg_doc(&args.params[0], false));

                // Include trailing comments: `Array<{...} /* trailing */>`
                for comment in comments_in_range(self.comments, param_end, before_close) {
                    if comment.is_block {
                        parts.push(d.text_owned(format!(" /*{}*/", comment.content)));
                    }
                }

                parts.push(d.text(">"));
                return d.concat(&parts);
            }
        }

        let mut inner_parts = Vec::new();
        let mut prev_end = args.span.start + 1; // After the opening `<`

        for (i, param) in args.params.iter().enumerate() {
            let param_start = param.span().start;
            let is_last = i == args.params.len() - 1;

            // Build parts for this argument
            let mut arg_parts = Vec::new();

            // Add leading block comments before this type argument
            for comment in comments_in_range(self.comments, prev_end, param_start) {
                if comment.is_block {
                    arg_parts.push(d.text_owned(format!("/*{}*/ ", comment.content)));
                }
            }

            arg_parts.push(self.build_type_arg_doc(param, true));

            // Add trailing block comments after this type argument
            let param_end = param.span().end;
            let next_boundary = if i + 1 < args.params.len() {
                args.params[i + 1].span().start
            } else {
                args.span.end - 1 // Before the closing `>`
            };
            for comment in comments_in_range(self.comments, param_end, next_boundary) {
                if comment.is_block {
                    arg_parts.push(d.text_owned(format!(" /*{}*/", comment.content)));
                }
            }

            // Update prev_end to next_boundary to avoid double-counting comments
            prev_end = next_boundary;

            // Add separator before non-first arguments
            if i > 0 {
                inner_parts.push(d.line());
            }
            inner_parts.push(d.concat(&arg_parts));
            // Add comma separator after non-last elements
            if !is_last {
                inner_parts.push(d.text(","));
            }
            // Note: type arguments don't get trailing commas (unlike params)
        }

        // Wrap in group with proper indentation for width-based breaking
        d.group(d.concat(&[
            d.text("<"),
            d.indent_softline(d.concat(&inner_parts)),
            d.softline(),
            d.text(">"),
        ]))
    }

    /// Build doc for type arguments with line comments between them.
    ///
    /// Line comments force multiline because they can't appear inline.
    fn build_type_arguments_doc_with_line_comments(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> DocId {
        let d = self.d();
        let mut inner_parts = Vec::new();
        let mut prev_end = args.span.start + 1; // After the opening `<`

        for (i, param) in args.params.iter().enumerate() {
            let param_start = param.span().start;
            let param_end = param.span().end;
            let is_last = i == args.params.len() - 1;

            // Leading comments
            inner_parts.extend(self.build_leading_comments_multiline(prev_end, param_start));

            inner_parts.push(self.build_type_arg_doc(param, args.params.len() > 1));

            let next_boundary = if i + 1 < args.params.len() {
                args.params[i + 1].span().start
            } else {
                args.span.end - 1 // Before the closing `>`
            };

            // Comma (not for last element in type args)
            if !is_last {
                inner_parts.push(d.text(","));
            }

            // Trailing comments
            inner_parts.extend(self.build_trailing_comments_multiline(param_end, next_boundary));

            // Hardline to separate from next element
            if !is_last {
                inner_parts.push(d.hardline());
            }

            prev_end = next_boundary;
        }

        d.concat(&[
            d.text("<"),
            d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])),
            d.hardline(),
            d.text(">"),
        ])
    }

    /// Build doc for type elements with comment handling
    fn build_type_elements_doc(
        &self,
        members: &[internal::TSTypeElement],
        body_start: u32,
        body_end: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end = body_start + 1; // after opening brace

        for (i, member) in members.iter().enumerate() {
            let member_start = member.span().start;
            let is_first = i == 0;

            // Find comments between previous element and this one
            // Filter out trailing same-line comments from the previous member
            // BUT keep multi-line block comments even if they start on the same line
            let all_comments: Vec<_> =
                comments_in_range(self.comments, prev_end, member_start).collect();
            let leading_comments: Vec<_> = if !is_first {
                all_comments
                    .iter()
                    .filter(|c| {
                        // Keep if not on same line as prev_end
                        if !self.is_same_line(prev_end, c.span.start) {
                            return true;
                        }
                        // Also keep multi-line block comments (they're always leading, never trailing)
                        self.is_multiline_comment(c)
                    })
                    .copied()
                    .collect()
            } else {
                all_comments
            };

            // Add separator before this member
            // For first member: just hardline
            // For other members: literalline + hardline if blank line in source, just hardline otherwise
            if i > 0 {
                let check_pos = if leading_comments.is_empty() {
                    member_start
                } else {
                    leading_comments[0].span.start
                };
                if self.has_blank_line_between(prev_end, check_pos) {
                    parts.push(d.literalline());
                }
            }
            // Always add hardline before member (or its leading comments)
            parts.push(d.hardline());

            // Print leading comments with blank line preservation
            parts.extend(
                self.build_leading_comments_with_blank_lines(&leading_comments, member_start),
            );

            parts.push(self.build_type_element_doc(member));

            // Handle trailing inline comments on same line after member
            // Skip multi-line block comments - they should be leading comments for the next element
            let upper_bound = members
                .get(i + 1)
                .map_or(body_end, |next| next.span().start);
            for comment in comments_in_range(self.comments, member.span().end, upper_bound) {
                if self.is_same_line(member.span().end, comment.span.start) {
                    // Skip multi-line block comments (they're leading comments for next element)
                    if self.is_multiline_comment(comment) {
                        continue;
                    }

                    if comment.is_block {
                        // Single-line block comments are inline, affect width
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                    } else {
                        // Line comments go in line_suffix, don't affect width
                        parts.push(self.build_trailing_line_comment_doc(comment));
                    }
                } else {
                    break; // Only same-line comments
                }
            }

            prev_end = member.span().end;
        }

        // Handle trailing comments after the last member (before closing `}`)
        parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end.saturating_sub(1)));

        d.concat(&parts)
    }

    /// Build doc for a single type element
    fn build_type_element_doc(&self, elem: &internal::TSTypeElement) -> DocId {
        let d = self.d();
        match elem {
            internal::TSTypeElement::PropertySignature(p) => {
                let mut parts = Vec::new();
                if p.readonly {
                    parts.push(d.text("readonly "));
                }
                // Handle computed property keys: [key]: type
                if p.computed {
                    parts.push(d.text("["));
                    parts.push(self.build_expression_doc(&p.key));
                    parts.push(d.text("]"));
                } else {
                    parts.push(self.build_expression_doc(&p.key));
                }
                if p.optional {
                    parts.push(d.text("?"));
                }
                if let Some(ta) = &p.type_annotation {
                    // Use width-aware wrapping for generic type arguments
                    parts.push(self.build_type_annotation_doc_wrapping(ta));
                }
                parts.push(d.text(";"));
                d.concat(&parts)
            }
            internal::TSTypeElement::MethodSignature(m) => {
                let mut parts = Vec::new();
                // Print accessor keyword for get/set signatures
                match m.kind {
                    internal::MethodKind::Get => parts.push(d.text("get ")),
                    internal::MethodKind::Set => parts.push(d.text("set ")),
                    _ => {}
                }
                // Handle computed method keys: [key](): type
                if m.computed {
                    parts.push(d.text("["));
                    parts.push(self.build_expression_doc(&m.key));
                    parts.push(d.text("]"));
                } else {
                    parts.push(self.build_expression_doc(&m.key));
                }
                if m.optional {
                    parts.push(d.text("?"));
                }
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &m.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                // Width-based breaking for params
                parts.push(self.build_signature_params_doc(&m.params, None));
                if let Some(rt) = &m.return_type {
                    parts.push(d.text(": "));
                    parts.push(self.build_type_doc(&rt.type_annotation));
                }
                parts.push(d.text(";"));
                d.group(d.concat(&parts))
            }
            internal::TSTypeElement::CallSignature(c) => {
                let mut parts = Vec::new();
                // Type parameters: `<T>` or `<T, U>`
                if let Some(type_params) = &c.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                // Width-based breaking for params
                parts.push(self.build_signature_params_doc(&c.params, None));
                if let Some(rt) = &c.return_type {
                    parts.push(d.text(": "));
                    parts.push(self.build_type_doc(&rt.type_annotation));
                }
                parts.push(d.text(";"));
                d.group(d.concat(&parts))
            }
            internal::TSTypeElement::ConstructSignature(c) => {
                let mut parts = vec![d.text("new ")];
                // Type parameters: `<T>` or `<T, U>`
                if let Some(type_params) = &c.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                // Width-based breaking for params
                parts.push(self.build_signature_params_doc(&c.params, None));
                if let Some(rt) = &c.return_type {
                    parts.push(d.text(": "));
                    parts.push(self.build_type_doc(&rt.type_annotation));
                }
                parts.push(d.text(";"));
                d.group(d.concat(&parts))
            }
            internal::TSTypeElement::IndexSignature(i) => {
                let mut parts = Vec::new();
                if i.readonly {
                    parts.push(d.text("readonly "));
                }
                parts.push(d.text("["));
                for (idx, param) in i.parameters.iter().enumerate() {
                    if idx > 0 {
                        parts.push(d.text(", "));
                    }
                    parts.push(d.symbol(param.name.to_u32()));
                    if let Some(ta) = &param.type_annotation {
                        parts.push(d.text(": "));
                        parts.push(self.build_type_doc(&ta.type_annotation));
                    }
                }
                parts.push(d.text("]: "));
                parts.push(self.build_type_doc(&i.type_annotation.type_annotation));
                parts.push(d.text(";"));
                d.concat(&parts)
            }
        }
    }

    /// Print an enum declaration: `enum Foo { A, B }` or `const enum Foo { A = 1 }`
    ///
    /// Build doc for enum declaration
    ///
    /// Prettier format:
    /// ```text
    /// enum Color {
    ///     Red,
    ///     Green,
    ///     Blue,
    /// }
    /// ```
    pub(super) fn build_enum_declaration_doc(&self, decl: &internal::TSEnumDeclaration) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // `declare` prefix if ambient declaration
        if decl.declare {
            parts.push(d.text("declare "));
        }

        // `const` prefix if const enum
        if decl.r#const {
            parts.push(d.text("const "));
        }

        parts.push(d.text("enum "));
        parts.push(d.symbol(decl.id.name.to_u32()));
        parts.push(d.text(" "));

        // Find body start (after '{')
        let body_start = self.source[decl.span.start as usize..decl.span.end as usize]
            .find('{')
            .map_or(decl.span.start, |i| decl.span.start + i as u32 + 1);
        let body_end = decl.span.end.saturating_sub(1); // Before '}'
        let body_span = tsv_lang::Span::new(body_start - 1, decl.span.end); // Include '{' and '}'

        if decl.members.is_empty() {
            // Empty enum body - handle comments inside
            parts.push(self.build_empty_body_with_comments_doc(body_span));
        } else {
            parts.push(d.text("{"));
            // Build member docs with comment handling
            let mut member_parts = Vec::new();
            let mut prev_end = body_start;

            for (i, member) in decl.members.iter().enumerate() {
                let member_start = member.span.start;
                let is_first = i == 0;

                // Check for comments between previous position and this member
                let comments: Vec<_> = comments_in_range(self.comments, prev_end, member_start)
                    .filter(|c| is_first || !self.is_same_line(prev_end, c.span.start))
                    .collect();

                // Check for blank lines
                if !is_first {
                    let check_pos = if comments.is_empty() {
                        member_start
                    } else {
                        comments[0].span.start
                    };
                    if self.has_blank_line_between(prev_end, check_pos) {
                        member_parts.push(d.literalline());
                    }
                    member_parts.push(d.hardline());
                }

                // Process leading comments
                for comment in &comments {
                    member_parts.push(self.build_comment_doc(comment));
                    // Block comment on same line as member gets space, otherwise hardline
                    if comment.is_block && self.is_same_line(comment.span.end, member_start) {
                        member_parts.push(d.text(" "));
                    } else {
                        member_parts.push(d.hardline());
                    }
                }

                member_parts.push(self.build_enum_member_doc(member));

                // Add comma (trailing comma for last member)
                member_parts.push(d.text(","));

                // Handle trailing same-line comments
                let upper_bound = decl
                    .members
                    .get(i + 1)
                    .map_or(body_end, |next| next.span.start);
                member_parts.extend(
                    self.build_trailing_same_line_comment_docs(member.span.end, upper_bound),
                );

                prev_end = member.span.end;
            }

            // Handle trailing comments after the last member
            member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));

            parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&member_parts)])));
            parts.push(d.hardline());
            parts.push(d.text("}"));
        }

        d.concat(&parts)
    }

    /// Build doc for a single enum member
    fn build_enum_member_doc(&self, member: &internal::TSEnumMember) -> DocId {
        let d = self.d();
        // Member id (identifier or string literal)
        let id_doc = match &member.id {
            internal::TSEnumMemberId::Identifier(id) => d.symbol(id.name.to_u32()),
            internal::TSEnumMemberId::String(lit) => {
                // String literal member name: `"hello"` in `enum { "hello" = 1 }`
                self.build_literal_doc(lit)
            }
        };

        // Initializer: ` = value`
        if let Some(init) = &member.initializer {
            let init_doc = self.build_expression_doc(init);

            // For binary expressions, use assignment layout with indent for wrapping
            // The binary expression already has a group() with line() elements.
            // We just need to add indent around it so continuations are indented.
            if matches!(init, internal::Expression::BinaryExpression(_)) {
                // Use indent() instead of indent_line() to avoid double-grouping.
                // The binary expression's own group will decide when to break.
                d.concat(&[id_doc, d.text(" = "), d.indent(init_doc)])
            } else {
                d.concat(&[id_doc, d.text(" = "), init_doc])
            }
        } else {
            id_doc
        }
    }

    /// Build doc for namespace/module declaration
    ///
    /// Prettier format:
    /// ```text
    /// namespace Utils {
    ///     export function log() {}
    /// }
    /// ```
    pub(super) fn build_module_declaration_doc(
        &self,
        decl: &internal::TSModuleDeclaration,
    ) -> DocId {
        self.build_module_declaration_doc_inner(decl, true)
    }

    /// Inner helper for module declaration doc building
    /// `is_root` is true for the outermost declaration (prints `namespace` keyword)
    fn build_module_declaration_doc_inner(
        &self,
        decl: &internal::TSModuleDeclaration,
        is_root: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Only print keywords for root declaration
        if is_root {
            // `declare` prefix if ambient declaration
            if decl.declare {
                parts.push(d.text("declare "));
            }

            // `global` is special - it replaces namespace/module keyword
            if decl.global {
                parts.push(d.text("global"));
            } else {
                // Use the original keyword (namespace or module)
                match decl.kind {
                    internal::TSModuleDeclarationKind::Namespace => {
                        parts.push(d.text("namespace "));
                    }
                    internal::TSModuleDeclarationKind::Module => {
                        parts.push(d.text("module "));
                    }
                }
            }
        }

        // Module/namespace name (if not global)
        if !decl.global {
            match &decl.id {
                internal::TSModuleName::Identifier(id) => {
                    parts.push(d.symbol(id.name.to_u32()));
                }
                internal::TSModuleName::Literal(lit) => {
                    parts.push(self.build_literal_doc(lit));
                }
            }
        }

        // Body (may be None for shorthand: `declare module 'name';`)
        match &decl.body {
            Some(internal::TSModuleDeclarationBody::TSModuleBlock(block)) => {
                parts.push(d.text(" "));

                if block.body.is_empty() {
                    // Empty namespace body - handle comments inside
                    parts.push(self.build_empty_body_with_comments_doc(block.span));
                } else {
                    parts.push(d.text("{"));

                    // Build statement docs with blank line preservation
                    let mut stmt_parts = Vec::new();
                    let mut prev_end = block.span.start + 1; // After opening '{'

                    for stmt in &block.body {
                        let curr_start = stmt.span().start;

                        // Add separator: literalline + hardline if blank line in source, single hardline otherwise
                        // literalline() produces a bare newline (no indent), preserving truly blank lines
                        if !stmt_parts.is_empty() {
                            if self.has_blank_line_between(prev_end, curr_start) {
                                stmt_parts.push(d.literalline()); // blank line (no indent)
                                stmt_parts.push(d.hardline()); // next statement with indent
                            } else {
                                stmt_parts.push(d.hardline());
                            }
                        }

                        stmt_parts.push(self.build_statement_doc(stmt));
                        prev_end = stmt.span().end;
                    }

                    // Handle trailing comments after the last statement
                    let body_end = block.span.end.saturating_sub(1);
                    stmt_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));

                    parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&stmt_parts)])));
                    parts.push(d.hardline());
                    parts.push(d.text("}"));
                }
            }
            Some(internal::TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                // Nested namespace: `namespace Outer.Inner { }`
                // Print as `Outer.Inner` (dot-separated)
                parts.push(d.text("."));
                parts.push(self.build_module_declaration_doc_inner(nested, false));
            }
            None => {
                // Shorthand ambient module: `declare module 'name';`
                parts.push(d.text(";"));
            }
        }

        d.concat(&parts)
    }
}
