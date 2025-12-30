// Class declaration printing for TypeScript

use super::super::Printer;
use crate::ast::internal;
use tsv_lang::{SymbolResolver, SymbolToU32, doc};

impl<'a> Printer<'a> {
    /// Print a class declaration or anonymous class: `class Foo {}` or `class {}`
    ///
    /// Uses doc-based printing with width-aware wrapping for long lines.
    pub(super) fn print_class_declaration(&mut self, decl: &internal::ClassDeclaration) {
        // Print decorators, each on its own line
        if let Some(decorators) = &decl.decorators {
            for decorator in decorators {
                self.print_decorator(decorator);
                self.write("\n");
                self.write_indent();
            }
        }

        // Build the header doc (everything before the body)
        let header_doc = self.build_class_header_doc(decl);

        // suffix_width accounts for body that follows the header:
        // - Empty body: " {}" = 3 chars
        // - Non-empty body: " {" = 2 chars (newline after brace not counted)
        let suffix_width = if decl.body.body.is_empty() { 3 } else { 2 };
        let header_output = self.render_doc_with_suffix(&header_doc, suffix_width);
        self.write(&header_output);

        // Space before body:
        // - Skip if both extends and implements (header ends with dedent(line))
        // - Skip if implements-only with long type params (header may end with dedent(softline))
        // For simplicity, just check if header ends with newline
        let has_extends = decl.super_class.is_some();
        let has_implements = !decl.implements.is_empty();
        let both_heritage = has_extends && has_implements;
        // Check if header output ends with newline (from break mode)
        let needs_space = !both_heritage && !header_output.ends_with('\n');
        if needs_space {
            self.write(" ");
        }
        self.print_class_body(&decl.body, decl.declare);
    }

    /// Build doc for class header (declare, name, type params, extends, implements)
    ///
    /// Prettier behavior for classes:
    /// - If everything fits on one line, keep inline
    /// - If type params break, put extends/implements on new lines
    /// - Multiple heritage clauses (extends + implements) each go on their own line
    /// - Brace goes to new line when heritage clauses are on separate lines
    fn build_class_header_doc(&self, decl: &internal::ClassDeclaration) -> doc::Doc {
        let mut parts = Vec::new();

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

        let has_extends = decl.super_class.is_some();
        let has_implements = !decl.implements.is_empty();

        // Type parameters - wrapped in their own group
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Build heritage docs separately first
        // Prettier PR #18325: For extends clause, use expression_to_string to keep the member
        // expression together (no breaks within `eslint.Rule.RuleModule`).
        // The extends keyword can break to a new line, but the expression stays flat.
        let extends_doc = if let Some(super_class) = &decl.super_class {
            let mut ext_parts = vec![doc::text("extends ")];
            // Use expression_to_string for flat output - no breaks in member expression
            ext_parts.push(doc::text_owned(self.expression_to_string(super_class)));
            if let Some(type_params) = &decl.super_type_parameters {
                ext_parts.push(self.build_type_arguments_doc(type_params));
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

        // Add heritage clauses
        // Prettier PR #18094 behavior:
        // - extends: stays inline after `>` (even when type params break)
        // - implements: goes to new line when type params break, with brace on new line
        // - Multiple heritage (extends + implements): each on separate line, brace on new line
        if has_extends && has_implements {
            // Multiple heritage clauses - each on separate line
            if let Some(ext) = extends_doc {
                parts.push(doc::indent_line(ext));
            }
            if let Some(impl_doc) = implements_doc {
                parts.push(doc::indent_line(impl_doc));
            }
            // Brace goes to new line for multiple heritage
            parts.push(doc::dedent(doc::line()));
        } else if let Some(ext) = extends_doc {
            // extends only - behavior depends on type params and super_class type:
            // - With type params: extends stays inline after `>` (Prettier PR #18094)
            // - Without type params AND member expression: can break (Prettier PR #18325)
            // - Without type params AND simple identifier: NEVER breaks (Prettier quirk)
            let has_type_params = decl.type_parameters.is_some();
            let is_member_expression = matches!(
                decl.super_class.as_deref(),
                Some(internal::Expression::MemberExpression(_))
            );
            if has_type_params || !is_member_expression {
                // Type params present OR simple identifier parent - stays inline
                parts.push(doc::text(" "));
                parts.push(ext);
            } else {
                // No type params AND member expression - can break to new line
                parts.push(doc::indent_line(ext));
            }
        } else if let Some(impl_doc) = implements_doc {
            // implements only - line() becomes space in flat mode
            parts.push(doc::indent_line(impl_doc));
            // Brace: use softline which disappears in flat mode, becomes newline in break mode
            parts.push(doc::dedent(doc::softline()));
        }

        doc::group(doc::concat(parts))
    }

    /// Print a class body with blank line preservation
    /// is_declare: true for declare class (members have no implementation)
    pub(in crate::printer) fn print_class_body(
        &mut self,
        body: &internal::ClassBody,
        is_declare: bool,
    ) {
        if body.body.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{\n");
        self.indent_level += 1;

        // Start after the opening '{'
        let mut prev_end = body.span.start + 1;

        for (i, member) in body.body.iter().enumerate() {
            let is_first = i == 0;

            // Preserve blank lines between class members
            // Skip if there are comments - they handle their own blank line preservation
            if !is_first {
                let has_comments = self.has_comments_between(prev_end, member.span().start);
                if !has_comments
                    && tsv_lang::printing::has_blank_line_between(
                        self.source,
                        prev_end,
                        member.span().start,
                    )
                {
                    self.write("\n");
                }
            }

            // Print leading comments before this member
            self.print_block_leading_comments(prev_end, member.span().start, is_first);

            self.write_indent();
            self.print_class_member(member, is_declare);

            // Print trailing inline comments (on same line after member)
            // Upper bound: next member's start, or body end for last member
            let upper_bound = body
                .body
                .get(i + 1)
                .map_or(body.span.end, |next| next.span().start);
            self.print_class_member_trailing_comments(member.span().end, upper_bound);

            self.write("\n");

            prev_end = member.span().end;
        }

        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
    }

    /// Print trailing inline comments for a class member
    /// Handles comments on the same line after the member ends (e.g., `prop: string; // comment`)
    /// `upper_bound` limits the search to avoid including comments inside the next member
    fn print_class_member_trailing_comments(&mut self, member_end: u32, upper_bound: u32) {
        // find_first_comment_from returns index of first comment at or after member_end
        let first_idx = tsv_lang::find_first_comment_from(self.comments, member_end);

        for comment in &self.comments[first_idx..] {
            // Only include comments that are:
            // 1. On the same line as member_end
            // 2. Before the upper bound (next member's start or body end)
            if comment.span.start >= upper_bound {
                break;
            }
            if tsv_lang::printing::is_same_line(self.source, member_end, comment.span.start) {
                self.write(" ");
                self.print_comment(comment);
            } else {
                break;
            }
        }
    }

    /// Print a class member (method, property, or static block)
    fn print_class_member(&mut self, member: &internal::ClassMember, is_declare: bool) {
        match member {
            internal::ClassMember::MethodDefinition(method) => {
                self.print_method_definition(method, is_declare);
            }
            internal::ClassMember::PropertyDefinition(prop) => {
                self.print_property_definition(prop, is_declare);
            }
            internal::ClassMember::StaticBlock(block) => {
                self.print_static_block(block);
            }
            internal::ClassMember::IndexSignature(sig) => {
                self.print_index_signature(sig);
            }
        }
    }

    /// Print an index signature: `[key: Type]: ValueType;`
    fn print_index_signature(&mut self, sig: &internal::TSIndexSignature) {
        if sig.readonly {
            self.write("readonly ");
        }
        self.write("[");
        for (i, param) in sig.parameters.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_identifier(param);
        }
        self.write("]");
        self.print_type_annotation(&sig.type_annotation);
        self.write(";");
    }

    /// Print a static initialization block: `static { ... }`
    fn print_static_block(&mut self, block: &internal::StaticBlock) {
        self.write("static ");
        // Create a BlockStatement wrapper to reuse existing printing logic
        let block_stmt = internal::BlockStatement {
            body: block.body.clone(),
            span: block.span,
        };
        self.print_block_statement(&block_stmt);
    }

    /// Print a property definition
    ///
    /// Uses hybrid approach: prefix printed imperatively, type annotation uses doc-based
    /// width-aware printing so union/intersection types and generic arguments wrap correctly.
    fn print_property_definition(
        &mut self,
        prop: &internal::PropertyDefinition,
        _is_declare: bool,
    ) {
        // Print decorators, each on its own line
        if let Some(decorators) = &prop.decorators {
            for decorator in decorators {
                self.print_decorator(decorator);
                self.write("\n");
                self.write_indent();
            }
        }

        // Print accessibility modifier if applicable
        if let Some(accessibility) = &prop.accessibility {
            self.write(accessibility.as_str());
            self.write(" ");
        }

        // Print static modifier if applicable
        if prop.is_static {
            self.write("static ");
        }

        // Print abstract modifier if applicable
        if prop.r#abstract {
            self.write("abstract ");
        }

        // Print readonly modifier if applicable
        if prop.readonly {
            self.write("readonly ");
        }

        // Print accessor keyword if applicable (ES decorator proposal)
        if prop.accessor {
            self.write("accessor ");
        }

        // Print key
        if prop.computed {
            self.write("[");
            self.print_expression(&prop.key);
            self.write("]");
        } else {
            self.print_expression(&prop.key);
        }

        // Print optional marker (?) or definite assignment assertion (!)
        match prop.modifier {
            internal::PropertyModifier::Optional => self.write("?"),
            internal::PropertyModifier::Definite => self.write("!"),
            internal::PropertyModifier::None => {}
        }

        // Build doc for type annotation + value using width-aware printing
        // suffix_width accounts for trailing ";" = 1 char
        let suffix_doc = self.build_property_suffix_doc(prop);
        let suffix_output = self.render_doc_with_suffix(&suffix_doc, 1);
        self.write(&suffix_output);
        self.write(";");
    }

    /// Build a Doc for property suffix (type annotation + optional value).
    ///
    /// Uses doc-based printing so union/intersection types and generic type
    /// arguments wrap correctly when line exceeds print_width.
    fn build_property_suffix_doc(&self, prop: &internal::PropertyDefinition) -> doc::Doc {
        let mut parts = Vec::new();

        // Type annotation if present - use width-aware wrapping for generic types
        if let Some(type_annotation) = &prop.type_annotation {
            parts.push(self.build_type_annotation_doc_wrapping(type_annotation));
        }

        // Value if present
        if let Some(value) = &prop.value {
            parts.push(doc::text(" = "));
            parts.push(self.build_expression_doc(value));
        }

        doc::concat(parts)
    }

    /// Print a method definition
    ///
    /// Uses hybrid approach: prefix/suffix printed imperatively, signature uses doc-based
    /// width-aware wrapping so params break when total line length exceeds print_width.
    fn print_method_definition(&mut self, method: &internal::MethodDefinition, is_declare: bool) {
        // Print decorators, each on its own line
        if let Some(decorators) = &method.decorators {
            for decorator in decorators {
                self.print_decorator(decorator);
                self.write("\n");
                self.write_indent();
            }
        }

        // Print accessibility modifier if applicable
        if let Some(accessibility) = &method.accessibility {
            self.write(accessibility.as_str());
            self.write(" ");
        }

        // Print static modifier if applicable
        if method.is_static {
            self.write("static ");
        }

        // Print override modifier if applicable
        if method.r#override {
            self.write("override ");
        }

        // Print abstract modifier if applicable
        if method.r#abstract {
            self.write("abstract ");
        }

        // Print async modifier if applicable
        if method.value.r#async {
            self.write("async ");
        }

        // Print generator marker if applicable
        if method.value.generator {
            self.write("*");
        }

        // Print get/set for accessors
        match method.kind {
            internal::MethodKind::Get => self.write("get "),
            internal::MethodKind::Set => self.write("set "),
            _ => {}
        }

        // Print key
        if method.computed {
            self.write("[");
            self.print_expression(&method.key);
            self.write("]");
        } else {
            self.print_expression(&method.key);
        }

        // Print type parameters if present: method<T>()
        if let Some(type_params) = &method.value.type_parameters {
            self.print_type_parameter_declaration(type_params);
        }

        // Build signature doc (params + return type) using doc-based width-aware wrapping.
        // The suffix_width accounts for the body that follows (` {}` for empty, ` {` for non-empty).
        let is_overload_signature = method.value.body.span.start == method.value.body.span.end;
        let is_bodyless = is_declare || method.r#abstract || is_overload_signature;
        let suffix_width = if is_bodyless {
            1 // Just ";"
        } else if method.value.body.body.is_empty() {
            3 // " {}"
        } else {
            2 // " {"
        };

        let sig_doc = self.build_method_signature_doc(method);
        let sig_output = self.render_doc_with_suffix(&sig_doc, suffix_width);
        self.write(&sig_output);

        // For declare class, abstract methods, or overload signatures, print semicolon instead of body
        if is_bodyless {
            self.write(";");
        } else {
            self.write(" ");
            // Find the end of the signature to check for dangling comments
            // (comments between signature and body that should move inside)
            // We need position AFTER `)` to avoid re-printing param comments.
            let sig_end = if let Some(return_type) = &method.value.return_type {
                return_type.span.end
            } else {
                // Find closing `)` to get accurate boundary
                self.find_closing_paren(method.value.params_start, method.value.body.span.start)
                    .unwrap_or(method.value.body.span.start)
            };
            // Print body with outer comments moved inside
            self.print_block_statement_with_outer_comments(&method.value.body, sig_end);
        }
    }

    /// Build a Doc for method signature (params + return type).
    ///
    /// Prefix (modifiers, name, type params) is printed imperatively before this.
    /// Body is printed separately after. This doc handles width-aware param wrapping.
    fn build_method_signature_doc(&self, method: &internal::MethodDefinition) -> doc::Doc {
        let func = &method.value;

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
        parts.push(doc::group(params_doc));

        // Return type annotation
        if let Some(return_type) = &func.return_type {
            parts.push(self.build_type_annotation_doc(return_type));
        }

        doc::concat(parts)
    }

    /// Build a Doc for a class declaration
    pub(super) fn build_class_declaration_doc(
        &self,
        decl: &internal::ClassDeclaration,
    ) -> doc::Doc {
        let mut parts = vec![];

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
            let id_str = self.resolve_symbol(id.name);
            parts.push(doc::text(" "));
            parts.push(doc::text_owned(id_str));
        }

        // Type parameters
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Handle extends clause
        if let Some(super_class) = &decl.super_class {
            parts.push(doc::text(" extends "));
            parts.push(self.build_expression_doc(super_class));
            if let Some(type_args) = &decl.super_type_parameters {
                parts.push(self.build_type_arguments_doc(type_args));
            }
        }

        // Handle implements clause
        if !decl.implements.is_empty() {
            parts.push(doc::text(" implements "));
            for (i, impl_item) in decl.implements.iter().enumerate() {
                if i > 0 {
                    parts.push(doc::text(", "));
                }
                parts.push(self.build_entity_name_doc(&impl_item.expression));
                if let Some(type_args) = &impl_item.type_arguments {
                    parts.push(self.build_type_arguments_doc(type_args));
                }
            }
        }

        parts.push(doc::text(" "));
        parts.push(self.build_class_body_doc(&decl.body, decl.declare));

        doc::concat(parts)
    }

    /// Build a Doc for a class body
    pub(in crate::printer) fn build_class_body_doc(
        &self,
        body: &internal::ClassBody,
        _is_ambient: bool,
    ) -> doc::Doc {
        if body.body.is_empty() {
            return doc::text("{}");
        }

        // Build member docs joined by hardlines
        let mut member_parts = Vec::new();
        for (i, member) in body.body.iter().enumerate() {
            if i > 0 {
                member_parts.push(doc::hardline());
            }
            member_parts.push(self.build_class_member_doc(member));
        }

        // Wrap body content in indent (like build_interface_declaration_doc)
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
        let param_docs: Vec<_> = sig
            .parameters
            .iter()
            .map(|p| self.build_identifier_doc(p))
            .collect();
        parts.push(doc::join(param_docs, ", "));
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

        // Static modifier
        if prop.is_static {
            parts.push(doc::text("static "));
        }

        // Abstract modifier
        if prop.r#abstract {
            parts.push(doc::text("abstract "));
        }

        // Accessibility modifier
        if let Some(accessibility) = &prop.accessibility {
            match accessibility {
                internal::Accessibility::Public => parts.push(doc::text("public ")),
                internal::Accessibility::Private => parts.push(doc::text("private ")),
                internal::Accessibility::Protected => parts.push(doc::text("protected ")),
            }
        }

        // Readonly modifier
        if prop.readonly {
            parts.push(doc::text("readonly "));
        }

        // Key
        if prop.computed {
            parts.push(doc::text("["));
            parts.push(self.build_expression_doc(&prop.key));
            parts.push(doc::text("]"));
        } else {
            parts.push(self.build_expression_doc(&prop.key));
        }

        // Type annotation
        if let Some(type_ann) = &prop.type_annotation {
            parts.push(doc::text(": "));
            parts.push(self.build_type_doc(&type_ann.type_annotation));
        }

        // Value if present
        if let Some(value) = &prop.value {
            parts.push(doc::text(" = "));
            parts.push(self.build_expression_doc(value));
        }

        parts.push(doc::text(";"));

        doc::concat(parts)
    }

    /// Build a Doc for a method definition
    fn build_method_definition_doc(&self, method: &internal::MethodDefinition) -> doc::Doc {
        let mut parts = vec![];

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

        // Parameters (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        let param_docs: Vec<_> = method
            .value
            .params
            .iter()
            .map(|param| self.build_expression_doc(param))
            .collect();
        parts.push(doc::text("("));
        parts.push(doc::join(param_docs, ", "));

        // For abstract methods, use semicolon instead of body
        if method.r#abstract {
            parts.push(doc::text(");"));
        } else {
            parts.push(doc::text(") "));
            // Body
            parts.push(self.build_block_statement_doc(&method.value.body));
        }

        doc::concat(parts)
    }

    /// Print a decorator: `@expression`
    ///
    /// Uses doc-based printing without margin since decorators don't have
    /// trailing punctuation on the same line.
    pub(in crate::printer) fn print_decorator(&mut self, decorator: &internal::Decorator) {
        self.write("@");
        // Use write_doc (no margin) - decorators don't have trailing punctuation
        let doc = self.build_expression_doc(&decorator.expression);
        self.write_doc(&doc);
    }
}
