// Class declaration printing for TypeScript

use super::Printer;
use crate::ast::internal;
use tsv_lang::{SymbolToU32, comments_in_range, doc};

impl<'a> Printer<'a> {
    /// Check if class should use "group mode" for heritage clauses
    ///
    /// In group mode, heritage clauses break when the class header group breaks.
    /// Returns true when:
    /// 1. Multiple heritage items (extends + implements count > 1)
    /// 2. Member expression superclass without type arguments
    fn should_class_group_mode(&self, decl: &internal::ClassDeclaration) -> bool {
        // Count heritage items
        let mut count = if decl.super_class.is_some() { 1 } else { 0 };
        count += decl.implements.len();
        if count > 1 {
            return true;
        }

        // Check for member expression superclass without type args
        if let Some(super_class) = &decl.super_class
            && decl.super_type_parameters.is_none()
            && matches!(
                super_class.as_ref(),
                internal::Expression::MemberExpression(_)
            )
        {
            return true;
        }

        false
    }

    /// Build a Doc for method signature (params + return type).
    ///
    /// Prefix (modifiers, name, type params) is printed imperatively before this.
    /// Body is printed separately after. This doc handles width-aware param wrapping.
    fn build_method_signature_doc(&self, method: &internal::MethodDefinition) -> doc::Doc {
        let func = &method.value;

        // Check if return type will break on its own (object type or multiline).
        // This matches Prettier's shouldGroupFunctionParameters behavior:
        // - Object types (TypeLiteral) break when they have multiple members
        // - Already multiline types (contains '\n') will obviously break
        // When true, we shouldn't include return type width when deciding if params should break,
        // and we should wrap params in their own group so they break independently.
        let return_type_will_break = func.return_type.as_ref().is_some_and(|rt| {
            // Check if it's an object type (TSTypeLiteral)
            let is_object_type = matches!(
                rt.type_annotation.as_ref(),
                internal::TSType::TypeLiteral(_)
            );
            // Check if it's already multiline in source
            let is_multiline = rt.span.extract(self.source).contains('\n');
            is_object_type || is_multiline
        });

        // Estimate if params should be forced to break based on total signature width.
        // Similar to build_function_signature_doc in function.rs.
        let force_params_break = if let Some(tp) = &func.type_parameters {
            // Type params break if: multiple params OR contains multiline content
            let has_multiple_params = tp.params.len() > 1;
            let span_str = tp.span.extract(self.source);
            let is_multiline = span_str.contains('\n');
            let type_params_will_break = has_multiple_params || is_multiline;

            if type_params_will_break {
                // Type params break → params get fresh line budget → don't force break
                false
            } else if return_type_will_break {
                // Return type is multiline - it breaks on its own, so only check if params fit
                let current_col = self.current_column();
                let params_width: usize = func
                    .params
                    .iter()
                    .map(|p| (p.span().end - p.span().start) as usize + 2)
                    .sum();
                // +4 for (): and opening of return type
                let estimated_params = current_col + params_width + 4;
                estimated_params > self.config.print_width
            } else {
                // Estimate total signature width (current column + remaining content)
                let current_col = self.current_column();
                let params_width: usize = func
                    .params
                    .iter()
                    .map(|p| (p.span().end - p.span().start) as usize + 2)
                    .sum();
                let return_type_width = func
                    .return_type
                    .as_ref()
                    .map_or(0, |rt| (rt.span.end - rt.span.start) as usize);
                // +4 accounts for parens and spaces: "()" around params, " {}" body
                let estimated_total = current_col + params_width + return_type_width + 4;
                estimated_total > self.config.print_width
            }
        } else if return_type_will_break {
            // Return type is multiline - it breaks on its own, so only check if params fit
            let current_col = self.current_column();
            let params_width: usize = func
                .params
                .iter()
                .map(|p| (p.span().end - p.span().start) as usize + 2)
                .sum();
            // +4 for (): and opening of return type
            let estimated_params = current_col + params_width + 4;
            estimated_params > self.config.print_width
        } else {
            // No type params - still need to check if signature fits
            let current_col = self.current_column();
            let params_width: usize = func
                .params
                .iter()
                .map(|p| (p.span().end - p.span().start) as usize + 2)
                .sum();
            let return_type_width = func
                .return_type
                .as_ref()
                .map_or(0, |rt| (rt.span.end - rt.span.start) as usize);
            let estimated_total = current_col + params_width + return_type_width + 4;
            estimated_total > self.config.print_width
        };

        let mut parts = Vec::new();

        // Build params doc with force_break if needed
        let params_start = Some(func.params_start);
        let trailing_comments_end = if let Some(rt) = &func.return_type {
            Some(rt.span.start)
        } else {
            Some(func.body.span.start)
        };
        let params_doc = self.build_params_doc_with_comments_ext(
            &func.params,
            params_start,
            trailing_comments_end,
            force_params_break,
        );

        // Prettier's shouldGroupFunctionParameters: when return type is object/multiline and
        // we have 1 param, wrap params in their own group. This allows params to stay on one
        // line even when the outer group breaks (due to multiline return type).
        // See: printMethodValue in prettier/src/language-js/print/function.js
        let should_group_params =
            func.params.len() == 1 && return_type_will_break && func.return_type.is_some();

        if should_group_params {
            // Wrap params in their own group - params break independently from return type
            parts.push(doc::group(params_doc));
        } else {
            // No nested group - outer signature group controls all breaking
            parts.push(params_doc);
        }

        // Return type annotation
        if let Some(return_type) = &func.return_type {
            parts.push(self.build_type_annotation_doc_for_return_type(return_type));
        }

        // Single outer group for entire signature (params + return type).
        // When this group breaks, params' softlines become newlines while return type stays flat.
        // Matches Prettier's printMethodValue structure.
        doc::group(doc::concat(parts))
    }

    /// Build a Doc for a class declaration
    #[inline]
    pub(super) fn build_class_declaration_doc(
        &self,
        decl: &internal::ClassDeclaration,
    ) -> doc::Doc {
        self.build_class_declaration_doc_inner(decl, true)
    }

    /// Build a Doc for a class declaration without decorators
    ///
    /// Used when exporting decorated classes where decorators are printed
    /// before the export keyword.
    #[inline]
    pub(in crate::printer) fn build_class_declaration_without_decorators_doc(
        &self,
        decl: &internal::ClassDeclaration,
    ) -> doc::Doc {
        self.build_class_declaration_doc_inner(decl, false)
    }

    /// Core implementation for class declaration doc building
    ///
    /// # Arguments
    ///
    /// * `decl` - The class declaration to build a doc for
    /// * `include_decorators` - If true, decorators are included in the output.
    ///   Set to false when decorators are printed separately (e.g., before `export`).
    fn build_class_declaration_doc_inner(
        &self,
        decl: &internal::ClassDeclaration,
        include_decorators: bool,
    ) -> doc::Doc {
        let group_mode = self.should_class_group_mode(decl);
        let mut parts = vec![];

        // Decorators, each on its own line
        if include_decorators
            && let Some(dec_doc) = self.build_decorators_doc(decl.decorators.as_ref())
        {
            parts.push(dec_doc);
        }

        // declare modifier
        if decl.declare {
            parts.push(doc::text("declare "));
        }

        // abstract modifier
        if decl.r#abstract {
            parts.push(doc::text("abstract "));
        }

        parts.push(doc::text("class"));
        if let Some(id) = &decl.id {
            parts.push(doc::text(" "));
            parts.push(doc::symbol(id.name.to_u32()));
        }

        // Build heritage docs
        let extends_doc = if let Some(super_class) = &decl.super_class {
            let mut ext_parts = vec![doc::text("extends ")];
            ext_parts.push(doc::text_owned(self.expression_to_string(super_class)));
            if let Some(type_args) = &decl.super_type_parameters {
                ext_parts.push(self.build_type_arguments_doc(type_args));
            }
            Some(doc::concat(ext_parts))
        } else {
            None
        };

        let implements_doc = if !decl.implements.is_empty() {
            let mut impl_parts = vec![doc::text("implements ")];
            for (i, heritage) in decl.implements.iter().enumerate() {
                if i > 0 {
                    impl_parts.push(doc::text(", "));
                }
                impl_parts.push(self.build_entity_name_doc(&heritage.expression));
                if let Some(type_args) = &heritage.type_arguments {
                    impl_parts.push(self.build_type_arguments_doc(type_args));
                }
            }
            Some(doc::concat(impl_parts))
        } else {
            None
        };

        // Build the header group
        // The pre-brace line break is inside the group so it's affected by group break
        // But the body itself is outside (its hardlines don't affect fit check)
        let header_doc = if group_mode {
            // Group mode: one unified group - when it breaks, heritage breaks too
            if let Some(type_params) = &decl.type_parameters {
                parts.push(self.build_type_parameter_declaration_doc_inline_group(type_params));
            }

            // Heritage clauses with line breaks
            let mut heritage_parts = Vec::new();
            if let Some(ext) = extends_doc {
                heritage_parts.push(doc::line());
                heritage_parts.push(ext);
            }
            if let Some(impl_doc) = implements_doc {
                heritage_parts.push(doc::line());
                heritage_parts.push(impl_doc);
            }
            if !heritage_parts.is_empty() {
                parts.push(doc::indent(doc::concat(heritage_parts)));
            }

            // Pre-brace behavior depends on body content:
            // - Empty body: always space (` {}` stays inline)
            // - Non-empty body: line() becomes space if fits, newline if breaks
            if decl.body.body.is_empty() {
                parts.push(doc::text(" "));
            } else {
                // line() at base indent (heritage is indented, brace is at class level)
                parts.push(doc::line());
            }

            doc::group(doc::concat(parts))
        } else {
            // Non-group mode: type params break independently, heritage stays inline
            if let Some(type_params) = &decl.type_parameters {
                parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
            }

            // Heritage clauses stay inline
            if let Some(ext) = extends_doc {
                parts.push(doc::text(" "));
                parts.push(ext);
            }
            if let Some(impl_doc) = implements_doc {
                parts.push(doc::text(" "));
                parts.push(impl_doc);
            }

            // Always space before brace in non-group mode
            parts.push(doc::text(" "));

            doc::concat(parts)
        };

        // Body is outside the header group (hardlines in body don't affect header fit check)
        doc::concat(vec![
            header_doc,
            self.build_class_body_doc(&decl.body, decl.declare),
        ])
    }

    /// Build a Doc for a class body
    ///
    /// Handles comments between members, blank line preservation, and trailing comments.
    pub(in crate::printer) fn build_class_body_doc(
        &self,
        body: &internal::ClassBody,
        _is_ambient: bool,
    ) -> doc::Doc {
        if body.body.is_empty() {
            return self.build_empty_body_with_comments_doc(body.span);
        }

        // Build member docs with comments and blank line preservation
        let mut member_parts = Vec::new();
        let mut prev_end = body.span.start + 1; // Start after '{'

        for (i, member) in body.body.iter().enumerate() {
            let member_start = member.span().start;
            let is_first = i == 0;

            // Check for comments between previous position and this member
            // Filter out trailing same-line comments from the previous member
            let all_comments: Vec<_> =
                comments_in_range(self.comments, prev_end, member_start).collect();
            let comments: Vec<_> = if !is_first {
                all_comments
                    .iter()
                    .filter(|c| !self.is_same_line(prev_end, c.span.start))
                    .copied()
                    .collect()
            } else {
                all_comments
            };

            // For non-first members, determine if we need blank line preservation
            // We either add: hardline (no blank) or literalline + hardline (blank line)
            if !is_first {
                let check_pos = if comments.is_empty() {
                    member_start
                } else {
                    comments[0].span.start
                };
                if self.has_blank_line_between(prev_end, check_pos) {
                    // Blank line before first comment or member
                    member_parts.push(doc::literalline());
                }
                member_parts.push(doc::hardline());
            }

            // Process comments before this member (with blank line preservation)
            member_parts
                .extend(self.build_leading_comments_with_blank_lines(&comments, member_start));

            member_parts.push(self.build_class_member_doc(member));

            // Handle trailing inline comments on same line after member
            let upper_bound = body
                .body
                .get(i + 1)
                .map_or(body.span.end, |next| next.span().start);
            member_parts
                .extend(self.build_trailing_same_line_comment_docs(member.span().end, upper_bound));

            prev_end = member.span().end;
        }

        // Handle trailing comments after the last member (before closing `}`)
        let body_end = body.span.end.saturating_sub(1); // Before '}'
        member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));

        // Wrap body content in indent
        doc::concat(vec![
            doc::text("{"),
            doc::indent(doc::concat(vec![
                doc::hardline(),
                doc::concat(member_parts),
            ])),
            doc::hardline(),
            doc::text("}"),
        ])
    }

    /// Build a Doc for a class member
    fn build_class_member_doc(&self, member: &internal::ClassMember) -> doc::Doc {
        match member {
            internal::ClassMember::MethodDefinition(method) => {
                self.build_method_definition_doc(method)
            }
            internal::ClassMember::PropertyDefinition(prop) => {
                self.build_property_definition_doc(prop)
            }
            internal::ClassMember::StaticBlock(block) => self.build_static_block_doc(block),
            internal::ClassMember::IndexSignature(sig) => self.build_index_signature_doc(sig),
        }
    }

    /// Build a Doc for an index signature: `[key: Type]: ValueType;`
    fn build_index_signature_doc(&self, sig: &internal::TSIndexSignature) -> doc::Doc {
        let mut parts = Vec::new();

        if sig.readonly {
            parts.push(doc::text("readonly "));
        }

        parts.push(doc::text("["));
        parts.push(doc::join(
            sig.parameters.iter().map(|p| self.build_identifier_doc(p)),
            ", ",
        ));
        parts.push(doc::text("]"));
        parts.push(self.build_type_annotation_doc(&sig.type_annotation));
        parts.push(doc::text(";"));

        doc::concat(parts)
    }

    /// Build a Doc for a static initialization block
    fn build_static_block_doc(&self, block: &internal::StaticBlock) -> doc::Doc {
        // Create a BlockStatement wrapper to reuse existing doc building logic
        let block_stmt = internal::BlockStatement {
            body: block.body.clone(),
            span: block.span,
        };
        doc::concat(vec![
            doc::text("static "),
            self.build_block_statement_doc(&block_stmt),
        ])
    }

    /// Build a Doc for a property definition
    fn build_property_definition_doc(&self, prop: &internal::PropertyDefinition) -> doc::Doc {
        let mut parts = vec![];

        // Decorators
        if let Some(dec_doc) = self.build_decorators_doc(prop.decorators.as_ref()) {
            parts.push(dec_doc);
        }

        // Declare modifier (comes first, before accessibility)
        if prop.declare {
            parts.push(doc::text("declare "));
        }

        // Accessibility modifier
        if let Some(accessibility) = &prop.accessibility {
            parts.push(doc::text(accessibility.as_str()));
            parts.push(doc::text(" "));
        }

        // Static modifier
        if prop.is_static {
            parts.push(doc::text("static "));
        }

        // Override modifier
        if prop.r#override {
            parts.push(doc::text("override "));
        }

        // Abstract modifier
        if prop.r#abstract {
            parts.push(doc::text("abstract "));
        }

        // Readonly modifier
        if prop.readonly {
            parts.push(doc::text("readonly "));
        }

        // Accessor keyword
        if prop.accessor {
            parts.push(doc::text("accessor "));
        }

        // Key
        if prop.computed {
            parts.push(doc::text("["));
            parts.push(self.build_expression_doc(&prop.key));
            parts.push(doc::text("]"));
        } else {
            parts.push(self.build_expression_doc(&prop.key));
        }

        // Optional/definite modifier after key
        match prop.modifier {
            internal::PropertyModifier::Optional => parts.push(doc::text("?")),
            internal::PropertyModifier::Definite => parts.push(doc::text("!")),
            internal::PropertyModifier::None => {}
        }

        // Type annotation - use width-aware wrapping for generics and union types
        if let Some(type_ann) = &prop.type_annotation {
            parts.push(self.build_type_annotation_doc_wrapping(type_ann));
        }

        // Value if present
        if let Some(value) = &prop.value {
            parts.push(doc::text(" = "));

            // Check for comments between = and value (e.g., /* @__PURE__ */ annotations)
            // The = comes after the key (and type annotation if present)
            let before_value = prop
                .type_annotation
                .as_ref()
                .map_or_else(|| prop.key.span().end, |ta| ta.span.end);
            if let Some(comments) = self.build_inline_comments_between_doc_trailing_space_opt(
                before_value,
                value.span().start,
            ) {
                parts.push(comments);
            }

            parts.push(self.build_expression_doc(value));
        }

        parts.push(doc::text(";"));

        doc::concat(parts)
    }

    /// Build a Doc for a method definition
    fn build_method_definition_doc(&self, method: &internal::MethodDefinition) -> doc::Doc {
        let mut parts = vec![];

        // Decorators
        if let Some(dec_doc) = self.build_decorators_doc(method.decorators.as_ref()) {
            parts.push(dec_doc);
        }

        // Accessibility modifier
        if let Some(accessibility) = &method.accessibility {
            parts.push(doc::text(accessibility.as_str()));
            parts.push(doc::text(" "));
        }

        // Static modifier
        if method.is_static {
            parts.push(doc::text("static "));
        }

        // Override modifier
        if method.r#override {
            parts.push(doc::text("override "));
        }

        // Abstract modifier
        if method.r#abstract {
            parts.push(doc::text("abstract "));
        }

        // Async modifier
        if method.value.r#async {
            parts.push(doc::text("async "));
        }

        // Generator marker
        if method.value.generator {
            parts.push(doc::text("*"));
        }

        // Get/set for accessors
        match method.kind {
            internal::MethodKind::Get => parts.push(doc::text("get ")),
            internal::MethodKind::Set => parts.push(doc::text("set ")),
            _ => {}
        }

        // Key
        if method.computed {
            parts.push(doc::text("["));
            parts.push(self.build_expression_doc(&method.key));
            parts.push(doc::text("]"));
        } else {
            parts.push(self.build_expression_doc(&method.key));
        }

        // Type parameters if present: method<T>()
        if let Some(type_params) = &method.value.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Parameters and return type - use the signature builder
        parts.push(self.build_method_signature_doc(method));

        // Overload signatures have empty body (start == end)
        let is_overload_signature = method.value.body.span.start == method.value.body.span.end;

        // For abstract methods or overload signatures, use semicolon instead of body
        if method.r#abstract || is_overload_signature {
            parts.push(doc::text(";"));
        } else {
            parts.push(doc::text(" "));
            // Check for comments between signature and body (outer comments)
            // These need to be moved inside the block body
            let sig_end = if let Some(rt) = &method.value.return_type {
                rt.span.end
            } else if let Some(paren) =
                self.find_closing_paren(method.value.params_start, method.value.body.span.start)
            {
                paren
            } else {
                method.value.body.span.start
            };
            let outer_comments = self.build_outer_comments_for_block(sig_end, &method.value.body);
            parts.push(
                self.build_block_statement_with_outer_comments_doc(
                    &method.value.body,
                    outer_comments,
                ),
            );
        }

        doc::concat(parts)
    }
}
