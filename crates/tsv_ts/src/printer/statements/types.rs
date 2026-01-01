// Type-related statement printing for TypeScript

use super::{Printer, build_entity_name_doc};
use crate::ast::internal;
use tsv_lang::{SymbolToU32, comments_in_range, doc};

/// Check if a type is "generic" - i.e., has type parameters.
/// This matches prettier's `isGeneric` function in assignment.js.
fn is_generic_type(ts_type: &internal::TSType) -> bool {
    match ts_type {
        internal::TSType::Function(f) => f.type_parameters.is_some(),
        internal::TSType::TypeReference(r) => r.type_arguments.is_some(),
        _ => false,
    }
}

/// Check if we should break before the conditional type in a type alias.
/// Returns true if either checkType or extendsType has type parameters.
/// This matches prettier's `shouldBreakBeforeConditionalType` in assignment.js.
fn should_break_before_conditional_type(conditional: &internal::TSConditionalType) -> bool {
    is_generic_type(&conditional.check_type) || is_generic_type(&conditional.extends_type)
}

impl<'a> Printer<'a> {
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
    ) -> doc::Doc {
        let mut parts = vec![doc::text("type ")];
        parts.push(doc::symbol(decl.id.name.to_u32()));

        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        parts.push(doc::text(" ="));

        // Check the type kind for different formatting rules
        // For union/intersection types, build without their own group so they inherit
        // breaking from this context's group.
        if let internal::TSType::Union(u) = &decl.type_annotation {
            // Union types: break after `=` with leading `| `
            // Don't wrap union in its own group - let this context control breaking
            let type_doc = self.build_union_type_doc(u, false);
            parts.push(doc::group(doc::indent(doc::concat(vec![
                doc::line(), // space when flat, newline when broken
                type_doc,
            ]))));
        } else if let internal::TSType::Intersection(i) = &decl.type_annotation {
            // Intersection types: first element stays inline, subsequent wrap with indent
            // Don't wrap intersection in its own group - let this context control breaking
            // Format: `= Type1 &\n\tType2 &\n\tType3`
            let type_doc = self.build_intersection_type_doc(i, false);
            parts.push(doc::text(" "));
            parts.push(doc::group(doc::indent(type_doc)));
        } else if let internal::TSType::Conditional(cond) = &decl.type_annotation {
            // Conditional types: check if we should break after `=`
            // Break after `=` only if check_type or extends_type has type parameters
            // (this matches prettier's shouldBreakBeforeConditionalType)
            let type_doc = self.build_type_doc(&decl.type_annotation);
            if should_break_before_conditional_type(cond) {
                // Type has generic check/extends - break after `=` with indent
                // type T =
                //     check extends SomeGeneric<X>
                //         ? true
                //         : false
                parts.push(doc::group(doc::indent(doc::concat(vec![
                    doc::line(), // space when flat, newline when broken
                    type_doc,
                ]))));
            } else {
                // Type has non-generic check/extends - keep `=` inline, let conditional handle breaking
                // type T = check extends {a: X}
                //     ? true
                //     : false
                parts.push(doc::text(" "));
                parts.push(type_doc);
            }
        } else {
            // Other types (object literals, etc.): just add space and the type
            let type_doc = self.build_type_doc(&decl.type_annotation);
            parts.push(doc::text(" "));
            parts.push(type_doc);
        }

        parts.push(doc::text(";"));

        doc::concat(parts)
    }

    /// Build doc for interface declaration
    ///
    /// Uses group mode when extends has multiple items - heritage breaks when group breaks.
    pub(super) fn build_interface_declaration_doc(
        &self,
        decl: &internal::TSInterfaceDeclaration,
    ) -> doc::Doc {
        // Group mode: multiple extends items
        let group_mode = decl.extends.len() > 1;

        let mut header_parts = vec![doc::text("interface ")];
        header_parts.push(doc::symbol(decl.id.name.to_u32()));

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
                    doc::concat(h_parts)
                })
                .collect();
            Some(doc::concat(vec![
                doc::text("extends "),
                doc::join(heritage_docs, ", "),
            ]))
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
                header_parts.push(doc::indent(doc::concat(vec![doc::line(), ext_doc])));
            }

            doc::group(doc::concat(header_parts))
        } else {
            // Non-group mode: type params break independently, extends stays inline
            if let Some(type_params) = &decl.type_parameters {
                header_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
            }

            // Extends clause stays inline
            if let Some(ext_doc) = extends_doc {
                header_parts.push(doc::text(" "));
                header_parts.push(ext_doc);
            }

            doc::concat(header_parts)
        };

        // Build body separately (outside the header group)
        let mut parts = vec![header_doc, doc::text(" {")];
        if !decl.body.body.is_empty() {
            parts.push(doc::indent(doc::concat(vec![
                doc::hardline(),
                self.build_type_elements_doc(&decl.body.body),
            ])));
            parts.push(doc::hardline());
        }
        parts.push(doc::text("}"));

        doc::concat(parts)
    }

    /// Build doc for declare function with wrapping support for type parameters
    pub(super) fn build_declare_function_doc(
        &self,
        decl: &internal::TSDeclareFunction,
    ) -> doc::Doc {
        // Only print `declare` for top-level declare functions
        // Inside `declare namespace`, the `declare` is implicit
        let mut parts = if decl.declare {
            vec![
                doc::text("declare function "),
                doc::symbol(decl.id.name.to_u32()),
            ]
        } else {
            vec![doc::text("function "), doc::symbol(decl.id.name.to_u32())]
        };

        // Type parameters with wrapping support
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Function parameters
        parts.push(doc::text("("));
        let param_docs: Vec<_> = decl
            .params
            .iter()
            .map(|param| self.build_expression_doc(param))
            .collect();
        parts.push(doc::join(param_docs, ", "));
        parts.push(doc::text(")"));

        // Return type
        if let Some(return_type) = &decl.return_type {
            parts.push(doc::text(": "));
            parts.push(self.build_type_doc(&return_type.type_annotation));
        }

        parts.push(doc::text(";"));

        doc::group(doc::concat(parts))
    }

    /// Build doc for entity name
    pub(super) fn build_entity_name_doc(&self, name: &internal::TSEntityName) -> doc::Doc {
        // Delegate to standalone function - doesn't need printer state
        build_entity_name_doc(name)
    }

    /// Build doc for type arguments with comment preservation: `</* a */ T /* b */, U>`
    ///
    /// Default version: no independent width-based wrapping (parent context controls breaking).
    /// Use `build_type_arguments_doc_wrapping` when type args should break independently.
    pub(crate) fn build_type_arguments_doc(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> doc::Doc {
        if args.params.is_empty() {
            return doc::text("<>");
        }

        let mut parts = Vec::new();
        let mut prev_end = args.span.start + 1; // After the opening `<`

        for (i, param) in args.params.iter().enumerate() {
            let param_start = param.span().start;

            if i > 0 {
                parts.push(doc::text(", "));
            }

            // Add leading block comments before this type argument
            for comment in comments_in_range(self.comments, prev_end, param_start) {
                if comment.is_block {
                    parts.push(doc::text_owned(format!("/*{}*/ ", comment.content)));
                }
            }

            parts.push(self.build_type_doc(param));

            // Add trailing block comments after this type argument
            let param_end = param.span().end;
            let next_boundary = if i + 1 < args.params.len() {
                args.params[i + 1].span().start
            } else {
                args.span.end - 1 // Before the closing `>`
            };
            for comment in comments_in_range(self.comments, param_end, next_boundary) {
                if comment.is_block {
                    parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                }
            }

            // Update prev_end to next_boundary to avoid double-counting comments
            prev_end = next_boundary;
        }

        doc::concat(vec![doc::text("<"), doc::concat(parts), doc::text(">")])
    }

    /// Build doc for type arguments with width-based wrapping support.
    ///
    /// Inline: `<T, U, V>`
    /// Wrapped: `<\n\tT,\n\tU,\n\tV\n>`
    ///
    /// Use this when type arguments should break independently of parent context,
    /// such as in property type annotations.
    pub(crate) fn build_type_arguments_doc_wrapping(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> doc::Doc {
        if args.params.is_empty() {
            return doc::text("<>");
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
                    arg_parts.push(doc::text_owned(format!("/*{}*/ ", comment.content)));
                }
            }

            arg_parts.push(self.build_type_doc(param));

            // Add trailing block comments after this type argument
            let param_end = param.span().end;
            let next_boundary = if i + 1 < args.params.len() {
                args.params[i + 1].span().start
            } else {
                args.span.end - 1 // Before the closing `>`
            };
            for comment in comments_in_range(self.comments, param_end, next_boundary) {
                if comment.is_block {
                    arg_parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                }
            }

            // Update prev_end to next_boundary to avoid double-counting comments
            prev_end = next_boundary;

            // Add separator before non-first arguments
            if i > 0 {
                inner_parts.push(doc::line());
            }
            inner_parts.push(doc::concat(arg_parts));
            // Add comma separator after non-last elements
            if !is_last {
                inner_parts.push(doc::text(","));
            }
            // Note: type arguments don't get trailing commas (unlike params)
        }

        // Wrap in group with proper indentation for width-based breaking
        doc::group(doc::concat(vec![
            doc::text("<"),
            doc::indent_softline(doc::concat(inner_parts)),
            doc::softline(),
            doc::text(">"),
        ]))
    }

    /// Build doc for type elements
    fn build_type_elements_doc(&self, members: &[internal::TSTypeElement]) -> doc::Doc {
        let mut parts = Vec::new();
        for (i, member) in members.iter().enumerate() {
            if i > 0 {
                parts.push(doc::hardline());
            }
            parts.push(self.build_type_element_doc(member));
        }
        doc::concat(parts)
    }

    /// Build doc for a single type element
    fn build_type_element_doc(&self, elem: &internal::TSTypeElement) -> doc::Doc {
        match elem {
            internal::TSTypeElement::PropertySignature(p) => {
                let mut parts = Vec::new();
                if p.readonly {
                    parts.push(doc::text("readonly "));
                }
                // Handle computed property keys: [key]: type
                if p.computed {
                    parts.push(doc::text("["));
                    parts.push(self.build_expression_doc(&p.key));
                    parts.push(doc::text("]"));
                } else {
                    parts.push(self.build_expression_doc(&p.key));
                }
                if p.optional {
                    parts.push(doc::text("?"));
                }
                if let Some(ta) = &p.type_annotation {
                    // Use width-aware wrapping for generic type arguments
                    parts.push(self.build_type_annotation_doc_wrapping(ta));
                }
                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            internal::TSTypeElement::MethodSignature(m) => {
                let mut parts = Vec::new();
                // Print accessor keyword for get/set signatures
                match m.kind {
                    internal::MethodKind::Get => parts.push(doc::text("get ")),
                    internal::MethodKind::Set => parts.push(doc::text("set ")),
                    _ => {}
                }
                // Handle computed method keys: [key](): type
                if m.computed {
                    parts.push(doc::text("["));
                    parts.push(self.build_expression_doc(&m.key));
                    parts.push(doc::text("]"));
                } else {
                    parts.push(self.build_expression_doc(&m.key));
                }
                if m.optional {
                    parts.push(doc::text("?"));
                }
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &m.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                parts.push(doc::text("("));
                let param_docs: Vec<_> = m
                    .params
                    .iter()
                    .map(|param| self.build_expression_doc(param))
                    .collect();
                parts.push(doc::join(param_docs, ", "));
                parts.push(doc::text(")"));
                if let Some(rt) = &m.return_type {
                    parts.push(doc::text(": "));
                    parts.push(self.build_type_doc(&rt.type_annotation));
                }
                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            internal::TSTypeElement::CallSignature(c) => {
                let mut parts = Vec::new();
                // Type parameters: `<T>` or `<T, U>`
                if let Some(type_params) = &c.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                let param_docs: Vec<_> = c
                    .params
                    .iter()
                    .map(|param| self.build_expression_doc(param))
                    .collect();
                parts.push(doc::text("("));
                parts.push(doc::join(param_docs, ", "));
                parts.push(doc::text(")"));
                if let Some(rt) = &c.return_type {
                    parts.push(doc::text(": "));
                    parts.push(self.build_type_doc(&rt.type_annotation));
                }
                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            internal::TSTypeElement::ConstructSignature(c) => {
                let mut parts = vec![doc::text("new ")];
                // Type parameters: `<T>` or `<T, U>`
                if let Some(type_params) = &c.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                let param_docs: Vec<_> = c
                    .params
                    .iter()
                    .map(|param| self.build_expression_doc(param))
                    .collect();
                parts.push(doc::text("("));
                parts.push(doc::join(param_docs, ", "));
                parts.push(doc::text(")"));
                if let Some(rt) = &c.return_type {
                    parts.push(doc::text(": "));
                    parts.push(self.build_type_doc(&rt.type_annotation));
                }
                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            internal::TSTypeElement::IndexSignature(i) => {
                let mut parts = Vec::new();
                if i.readonly {
                    parts.push(doc::text("readonly "));
                }
                parts.push(doc::text("["));
                for (idx, param) in i.parameters.iter().enumerate() {
                    if idx > 0 {
                        parts.push(doc::text(", "));
                    }
                    parts.push(doc::symbol(param.name.to_u32()));
                    if let Some(ta) = &param.type_annotation {
                        parts.push(doc::text(": "));
                        parts.push(self.build_type_doc(&ta.type_annotation));
                    }
                }
                parts.push(doc::text("]: "));
                parts.push(self.build_type_doc(&i.type_annotation.type_annotation));
                parts.push(doc::text(";"));
                doc::concat(parts)
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
    pub(super) fn build_enum_declaration_doc(
        &self,
        decl: &internal::TSEnumDeclaration,
    ) -> doc::Doc {
        let mut parts = Vec::new();

        // `declare` prefix if ambient declaration
        if decl.declare {
            parts.push(doc::text("declare "));
        }

        // `const` prefix if const enum
        if decl.r#const {
            parts.push(doc::text("const "));
        }

        parts.push(doc::text("enum "));
        parts.push(doc::symbol(decl.id.name.to_u32()));
        parts.push(doc::text(" {"));

        if !decl.members.is_empty() {
            // Build member docs with trailing commas
            // Use join_trailing with comma + hardline separator
            let member_docs: Vec<doc::Doc> = decl
                .members
                .iter()
                .map(|m| self.build_enum_member_doc(m))
                .collect();

            let sep = doc::comma_hardline();
            parts.push(doc::indent(doc::concat(vec![
                doc::hardline(),
                doc::join_trailing(member_docs, sep),
            ])));
            parts.push(doc::hardline());
        }

        parts.push(doc::text("}"));

        doc::concat(parts)
    }

    /// Build doc for a single enum member
    fn build_enum_member_doc(&self, member: &internal::TSEnumMember) -> doc::Doc {
        // Member id (identifier or string literal)
        let id_doc = match &member.id {
            internal::TSEnumMemberId::Identifier(id) => doc::symbol(id.name.to_u32()),
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
                doc::concat(vec![id_doc, doc::text(" = "), doc::indent(init_doc)])
            } else {
                doc::concat(vec![id_doc, doc::text(" = "), init_doc])
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
    ) -> doc::Doc {
        self.build_module_declaration_doc_inner(decl, true)
    }

    /// Inner helper for module declaration doc building
    /// `is_root` is true for the outermost declaration (prints `namespace` keyword)
    fn build_module_declaration_doc_inner(
        &self,
        decl: &internal::TSModuleDeclaration,
        is_root: bool,
    ) -> doc::Doc {
        let mut parts = Vec::new();

        // Only print keywords for root declaration
        if is_root {
            // `declare` prefix if ambient declaration
            if decl.declare {
                parts.push(doc::text("declare "));
            }

            // `global` is special - it replaces namespace/module keyword
            if decl.global {
                parts.push(doc::text("global"));
            } else {
                // Use the original keyword (namespace or module)
                match decl.kind {
                    internal::TSModuleDeclarationKind::Namespace => {
                        parts.push(doc::text("namespace "));
                    }
                    internal::TSModuleDeclarationKind::Module => {
                        parts.push(doc::text("module "));
                    }
                }
            }
        }

        // Module/namespace name (if not global)
        if !decl.global {
            match &decl.id {
                internal::TSModuleName::Identifier(id) => {
                    parts.push(doc::symbol(id.name.to_u32()));
                }
                internal::TSModuleName::Literal(lit) => {
                    parts.push(self.build_literal_doc(lit));
                }
            }
        }

        // Body (may be None for shorthand: `declare module 'name';`)
        match &decl.body {
            Some(internal::TSModuleDeclarationBody::TSModuleBlock(block)) => {
                parts.push(doc::text(" {"));

                if !block.body.is_empty() {
                    // Build statement docs with blank line preservation
                    let mut stmt_parts = Vec::new();
                    let mut prev_end = block.span.start + 1; // After opening '{'

                    for stmt in &block.body {
                        let curr_start = stmt.span().start;

                        // Add separator: literalline + hardline if blank line in source, single hardline otherwise
                        // literalline() produces a bare newline (no indent), preserving truly blank lines
                        if !stmt_parts.is_empty() {
                            if tsv_lang::printing::has_blank_line_between(
                                self.source,
                                prev_end,
                                curr_start,
                            ) {
                                stmt_parts.push(doc::literalline()); // blank line (no indent)
                                stmt_parts.push(doc::hardline()); // next statement with indent
                            } else {
                                stmt_parts.push(doc::hardline());
                            }
                        }

                        stmt_parts.push(self.build_statement_doc(stmt));
                        prev_end = stmt.span().end;
                    }

                    parts.push(doc::indent(doc::concat(vec![
                        doc::hardline(),
                        doc::concat(stmt_parts),
                    ])));
                    parts.push(doc::hardline());
                }

                parts.push(doc::text("}"));
            }
            Some(internal::TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                // Nested namespace: `namespace Outer.Inner { }`
                // Print as `Outer.Inner` (dot-separated)
                parts.push(doc::text("."));
                parts.push(self.build_module_declaration_doc_inner(nested, false));
            }
            None => {
                // Shorthand ambient module: `declare module 'name';`
                parts.push(doc::text(";"));
            }
        }

        doc::concat(parts)
    }
}
