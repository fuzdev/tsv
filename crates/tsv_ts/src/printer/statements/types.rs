// Type-related statement printing for TypeScript

use super::super::Printer;
use crate::ast::internal;
use string_interner::Symbol;
use tsv_lang::{SymbolResolver, doc};

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
    /// Print a type alias declaration: `type X = T` or `type X<T> = T[]`
    ///
    /// Uses doc-based printing with width-aware wrapping for union/intersection types.
    pub(super) fn print_type_alias_declaration(&mut self, decl: &internal::TSTypeAliasDeclaration) {
        let decl_doc = self.build_type_alias_declaration_doc(decl);
        self.write_doc(&decl_doc);
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
    ) -> doc::Doc {
        let mut parts = vec![doc::text("type ")];
        parts.push(doc::symbol(decl.id.name.to_usize() as u32));

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

    /// Print a return statement: `return expr;` or `return;`
    pub(super) fn print_return_statement(&mut self, ret: &internal::ReturnStatement) {
        self.write("return");
        if let Some(arg) = &ret.argument {
            self.write(" ");
            self.print_expression(arg);
        }
        self.write(";");
    }

    /// Print an interface declaration: `interface Foo { ... }` or `interface Foo<T> { ... }`
    ///
    /// Uses doc-based printing with width-aware wrapping for long lines.
    pub(super) fn print_interface_declaration(&mut self, decl: &internal::TSInterfaceDeclaration) {
        // Build the header doc (everything before the body)
        let header_doc = self.build_interface_header_doc(decl);
        self.write_doc(&header_doc);

        // Print the body
        self.write(" {");
        if !decl.body.body.is_empty() {
            self.write("\n");
            self.indent_level += 1;
            for member in &decl.body.body {
                self.write_indent();
                self.print_type_element(member);
                self.write("\n");
            }
            self.indent_level -= 1;
            self.write_indent();
        }
        self.write("}");
    }

    /// Build doc for interface header (name, type params, extends clause)
    ///
    /// Prettier behavior:
    /// - If everything fits on one line, keep inline
    /// - If type params need to break, put extends on new line with extra indent
    fn build_interface_header_doc(&self, decl: &internal::TSInterfaceDeclaration) -> doc::Doc {
        let mut parts = vec![doc::text("interface ")];
        parts.push(doc::symbol(decl.id.name.to_usize() as u32));

        let has_type_params = decl.type_parameters.is_some();
        let has_extends = !decl.extends.is_empty();

        // Type parameters - wrapped in their own group so they can stay flat
        // even if the outer declaration group breaks
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Extends clause
        if has_extends {
            // Build heritage list
            let mut heritage_parts = Vec::new();
            for (i, heritage) in decl.extends.iter().enumerate() {
                if i > 0 {
                    heritage_parts.push(doc::text(", "));
                }
                heritage_parts.push(self.build_entity_name_doc(&heritage.expression));
                if let Some(type_args) = &heritage.type_arguments {
                    heritage_parts.push(self.build_type_arguments_doc(type_args));
                }
            }

            if has_type_params {
                // When type params are present, extends may break to new line
                // Use line() (not hardline) so will_break() doesn't force breaking
                parts.push(doc::indent(doc::concat(vec![
                    doc::line(), // Becomes space in flat mode, newline in break mode
                    doc::text("extends "),
                    doc::concat(heritage_parts),
                ])));
            } else {
                // No type params - extends stays inline
                parts.push(doc::text(" extends "));
                parts.push(doc::concat(heritage_parts));
            }
        }

        // Wrap everything in a group so if_break works correctly
        doc::group(doc::concat(parts))
    }

    /// Build doc for interface declaration
    pub(super) fn build_interface_declaration_doc(
        &self,
        decl: &internal::TSInterfaceDeclaration,
    ) -> doc::Doc {
        let mut parts = vec![doc::text("interface ")];
        parts.push(doc::symbol(decl.id.name.to_usize() as u32));
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Extends clause
        if !decl.extends.is_empty() {
            parts.push(doc::text(" extends "));
            let heritage_docs: Vec<_> = decl
                .extends
                .iter()
                .map(|heritage| self.build_entity_name_doc(&heritage.expression))
                .collect();
            parts.push(doc::join(heritage_docs, ", "));
        }

        parts.push(doc::text(" {"));
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

    /// Print a declare function: `declare function foo(): void` or `declare function foo<T>(): T`
    ///
    /// Uses doc-based printing with width-aware wrapping for long type parameter lists.
    pub(super) fn print_declare_function(&mut self, decl: &internal::TSDeclareFunction) {
        // Build the full declaration as a doc
        let decl_doc = self.build_declare_function_doc(decl);
        self.write_doc(&decl_doc);
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
                doc::symbol(decl.id.name.to_usize() as u32),
            ]
        } else {
            vec![
                doc::text("function "),
                doc::symbol(decl.id.name.to_usize() as u32),
            ]
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
        super::super::build_entity_name_doc(name)
    }

    /// Build doc for type arguments: `<T, U>`
    pub(super) fn build_type_arguments_doc(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> doc::Doc {
        let arg_docs: Vec<_> = args
            .params
            .iter()
            .map(|arg| self.build_type_doc(arg))
            .collect();
        doc::concat(vec![
            doc::text("<"),
            doc::join(arg_docs, ", "),
            doc::text(">"),
        ])
    }

    /// Print a type element (property signature, method signature, etc.)
    fn print_type_element(&mut self, elem: &internal::TSTypeElement) {
        match elem {
            internal::TSTypeElement::PropertySignature(p) => {
                if p.readonly {
                    self.write("readonly ");
                }
                if p.computed {
                    self.write("[");
                    self.print_expression(&p.key);
                    self.write("]");
                } else {
                    self.print_expression(&p.key);
                }
                if p.optional {
                    self.write("?");
                }
                if let Some(ta) = &p.type_annotation {
                    // For simple types with comments, print_type_annotation includes the semicolon
                    let has_trailing_comment = self.type_annotation_has_trailing_comment(ta);
                    self.print_type_annotation(ta);
                    if !has_trailing_comment {
                        self.write(";");
                    }
                } else {
                    self.write(";");
                }
            }
            internal::TSTypeElement::MethodSignature(m) => {
                if m.computed {
                    self.write("[");
                    self.print_expression(&m.key);
                    self.write("]");
                } else {
                    self.print_expression(&m.key);
                }
                if m.optional {
                    self.write("?");
                }
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &m.type_parameters {
                    self.print_type_parameter_declaration(type_params);
                }
                self.write("(");
                for (i, param) in m.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expression(param);
                }
                self.write(")");
                if let Some(rt) = &m.return_type {
                    self.print_type_annotation(rt);
                }
                self.write(";");
            }
            internal::TSTypeElement::CallSignature(c) => {
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &c.type_parameters {
                    self.print_type_parameter_declaration(type_params);
                }
                self.write("(");
                for (i, param) in c.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expression(param);
                }
                self.write(")");
                if let Some(rt) = &c.return_type {
                    self.print_type_annotation(rt);
                }
                self.write(";");
            }
            internal::TSTypeElement::ConstructSignature(c) => {
                self.write("new ");
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &c.type_parameters {
                    self.print_type_parameter_declaration(type_params);
                }
                self.write("(");
                for (i, param) in c.params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expression(param);
                }
                self.write(")");
                if let Some(rt) = &c.return_type {
                    self.print_type_annotation(rt);
                }
                self.write(";");
            }
            internal::TSTypeElement::IndexSignature(idx_sig) => {
                if idx_sig.readonly {
                    self.write("readonly ");
                }
                self.write("[");
                // Print parameter name and its type (for index signatures, the type is in the parameter)
                for (i, param) in idx_sig.parameters.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    // Just print the identifier name
                    let name = self.resolve_symbol(param.name).to_string();
                    self.write(&name);
                    // Print the parameter's type annotation
                    if let Some(ta) = &param.type_annotation {
                        self.print_type_annotation(ta);
                    }
                }
                self.write("]");
                self.print_type_annotation(&idx_sig.type_annotation);
                self.write(";");
            }
        }
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
                parts.push(self.build_expression_doc(&p.key));
                if p.optional {
                    parts.push(doc::text("?"));
                }
                if let Some(ta) = &p.type_annotation {
                    parts.push(doc::text(": "));
                    parts.push(self.build_type_doc(&ta.type_annotation));
                }
                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            internal::TSTypeElement::MethodSignature(m) => {
                let mut parts = Vec::new();
                parts.push(self.build_expression_doc(&m.key));
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
                let param_docs: Vec<_> = c
                    .params
                    .iter()
                    .map(|param| self.build_expression_doc(param))
                    .collect();
                let mut parts = vec![doc::text("("), doc::join(param_docs, ", "), doc::text(")")];
                if let Some(rt) = &c.return_type {
                    parts.push(doc::text(": "));
                    parts.push(self.build_type_doc(&rt.type_annotation));
                }
                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            internal::TSTypeElement::ConstructSignature(c) => {
                let param_docs: Vec<_> = c
                    .params
                    .iter()
                    .map(|param| self.build_expression_doc(param))
                    .collect();
                let mut parts = vec![
                    doc::text("new ("),
                    doc::join(param_docs, ", "),
                    doc::text(")"),
                ];
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
                    parts.push(doc::symbol(param.name.to_usize() as u32));
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
    /// Prettier formats enums with:
    /// - Each member on its own line
    /// - Trailing comma after the last member
    /// - Indented members
    pub(super) fn print_enum_declaration(&mut self, decl: &internal::TSEnumDeclaration) {
        let decl_doc = self.build_enum_declaration_doc(decl);
        self.write_doc(&decl_doc);
    }

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
        parts.push(doc::symbol(decl.id.name.to_usize() as u32));
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
            internal::TSEnumMemberId::Identifier(id) => doc::symbol(id.name.to_usize() as u32),
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

    /// Print a namespace/module declaration
    pub(super) fn print_module_declaration(&mut self, decl: &internal::TSModuleDeclaration) {
        let decl_doc = self.build_module_declaration_doc(decl);
        self.write_doc(&decl_doc);
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
                    parts.push(doc::symbol(id.name.to_usize() as u32));
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
