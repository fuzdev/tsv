// Statement printing for TypeScript
//
// Handles printing of different statement types:
// - Expression statements (expression followed by semicolon)
// - Variable declarations (const, let, var)
// - Future: Function declarations, class declarations, import/export, etc.

use super::{Printer, is_multiline_string_literal, is_pure_property_chain};
use crate::ast::internal::{self, Statement};
use tsv_lang::SymbolResolver;
use tsv_lang::doc;

impl<'a> Printer<'a> {
    /// Print a statement
    pub(super) fn print_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::ExpressionStatement(stmt) => self.print_expression_statement(stmt),
            Statement::VariableDeclaration(decl) => self.print_variable_declaration(decl),
            Statement::TSTypeAliasDeclaration(decl) => self.print_type_alias_declaration(decl),
            Statement::ReturnStatement(ret) => self.print_return_statement(ret),
            Statement::BlockStatement(block) => self.print_block_statement(block),
            Statement::FunctionDeclaration(decl) => self.print_function_declaration(decl),
            Statement::ClassDeclaration(decl) => self.print_class_declaration(decl),
        }
    }

    /// Build a Doc for a statement
    pub(super) fn build_statement_doc(&self, statement: &Statement) -> doc::Doc {
        match statement {
            Statement::ExpressionStatement(stmt) => {
                let expr_doc = self.build_expression_doc(&stmt.expression);
                doc::concat(vec![expr_doc, doc::text(";")])
            }
            Statement::VariableDeclaration(decl) => {
                // Simple doc build for variable declarations
                let keyword = decl.kind.as_str();
                let mut parts = vec![doc::text(keyword), doc::text(" ")];

                for (i, declarator) in decl.declarations.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(","));
                        parts.push(doc::hardline());
                        parts.push(doc::text(self.config.indent));
                    }
                    let id_str = self.resolve_symbol(declarator.id.name);
                    parts.push(doc::text(id_str));
                    if let Some(init) = &declarator.init {
                        parts.push(doc::text(" = "));
                        parts.push(self.build_expression_doc(init));
                    }
                }

                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            Statement::TSTypeAliasDeclaration(decl) => {
                let id_str = self.resolve_symbol(decl.id.name);
                // For type alias, extract from source for now (type printing is complex)
                let type_start = decl.type_annotation.span().start as usize;
                let type_end = decl.type_annotation.span().end as usize;
                let type_str = &self.source[type_start..type_end];
                doc::concat(vec![
                    doc::text("type "),
                    doc::text(id_str),
                    doc::text(" = "),
                    doc::text(type_str),
                    doc::text(";"),
                ])
            }
            Statement::ReturnStatement(ret) => {
                let mut parts = vec![doc::text("return")];
                if let Some(arg) = &ret.argument {
                    parts.push(doc::text(" "));
                    parts.push(self.build_expression_doc(arg));
                }
                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            Statement::BlockStatement(block) => self.build_block_statement_doc(block),
            Statement::FunctionDeclaration(decl) => self.build_function_declaration_doc(decl),
            Statement::ClassDeclaration(decl) => self.build_class_declaration_doc(decl),
        }
    }

    /// Print an expression statement (expression followed by semicolon)
    fn print_expression_statement(&mut self, stmt: &internal::ExpressionStatement) {
        self.print_expression(&stmt.expression);
        self.write(";");

        // Print inline comments - includes both:
        // 1. Comments after semicolon (stmt.span.end)
        // 2. Comments before semicolon (between expression.end and stmt.span.end)
        // Prettier moves comments from before semicolon to after it
        self.print_inline_comments_in_statement(stmt.expression.span().end, stmt.span.end);
    }

    /// Print a variable declaration
    fn print_variable_declaration(&mut self, decl: &internal::VariableDeclaration) {
        // Write the keyword (const, let, var)
        let keyword_start = decl.span.start;
        let keyword = decl.kind.as_str();
        self.write(keyword);
        let keyword_len = keyword.len() as u32;

        // Print comments between keyword and first declarator (e.g., `const /* comment */ x`)
        if !decl.declarations.is_empty() {
            let first_declarator_start = decl.declarations[0].span.start;
            let keyword_end = keyword_start + keyword_len;
            self.print_inline_comments_between(keyword_end, first_declarator_start);
        }

        self.write(" ");

        // Print declarators
        // Multiple declarators: one per line with extra indent
        // Example: `const a = 1,\n\t\tb = 2;`
        // When multiple declarators, multiline objects/arrays get extra indentation
        // Use save/restore pattern for nested multi-declarator safety
        let is_multi_declarator = decl.declarations.len() > 1;
        let old_indent_depth = self.declaration_indent_depth;
        if is_multi_declarator {
            self.declaration_indent_depth = old_indent_depth + 1;
        }

        let mut last_declarator_end = 0u32;
        for (i, declarator) in decl.declarations.iter().enumerate() {
            if i > 0 {
                let prev_end = decl.declarations[i - 1].span.end;
                let curr_start = declarator.span.start;

                // Check for line comments after comma (stay on same line)
                // e.g., `const a = 1, // comment\n b = 2`
                let has_line_comment = self.has_line_comments_between(prev_end, curr_start);

                self.write(",");

                if has_line_comment {
                    // Print line comment on same line as comma, then newline
                    self.print_inline_comments_between(prev_end, curr_start);
                }

                self.write("\n");
                // Continuation indent: one extra tab
                // (Svelte printer adds base indent to each line from TypeScript output)
                self.write(self.config.indent);

                if !has_line_comment {
                    // Print block comments on new line (e.g., `const a = 1, /* comment */ b = 2`)
                    // Note: print WITHOUT leading space (already indented) but WITH trailing space
                    if self.print_leading_comments_for_declarator(prev_end, curr_start) {
                        self.write(" ");
                    }
                }
            }
            self.print_variable_declarator(declarator);
            last_declarator_end = declarator.span.end;
        }

        // Restore multi-declarator context
        self.declaration_indent_depth = old_indent_depth;

        self.write(";");

        // Print inline comments - includes both:
        // 1. Comments after semicolon (decl.span.end)
        // 2. Comments before semicolon (between last declarator and decl.span.end)
        // Prettier moves comments from before semicolon to after it
        self.print_inline_comments_in_statement(last_declarator_end, decl.span.end);
    }

    /// Print a variable declarator with "fluid" assignment wrapping
    ///
    /// When the declaration exceeds print_width, wraps after `=`:
    /// ```javascript
    /// const medium =
    ///     obj1.prop1.prop2.prop3...;
    /// ```
    ///
    /// Uses `group(id + " =" + indent(line + rhs))` so the group decision
    /// determines whether to break. Property chains use greedy line packing
    /// via `fill()` for long chains that need internal breaks.
    fn print_variable_declarator(&mut self, declarator: &internal::VariableDeclarator) {
        // Check if we have an initializer - if not, just print the identifier
        let Some(init) = &declarator.init else {
            self.print_identifier(&declarator.id);
            return;
        };

        // Handle comments around the equals sign
        let id_end = declarator.id.span.end;
        let init_start = init.span().start;
        let equals_pos = self.find_equals_position(id_end, init_start);
        let has_comments_before_eq = self.has_comments_between(id_end, equals_pos);
        let has_comments_after_eq = self.has_comments_between(equals_pos + 1, init_start);

        // If there are comments, use direct printing (comment handling with doc IR is complex)
        if has_comments_before_eq || has_comments_after_eq {
            self.print_identifier(&declarator.id);
            let _ = self.print_inline_comments_between(id_end, equals_pos);
            if has_comments_after_eq {
                self.write(" =");
            } else {
                self.write(" = ");
            }
            let _ = self.print_inline_comments_between(equals_pos + 1, init_start);
            if has_comments_after_eq {
                self.write(" ");
            }
            self.print_expression(init);
            return;
        }

        // Check if RHS is a multiline string (line continuations)
        // Prettier ALWAYS wraps these - it's not a width decision, it's mandatory
        let is_multiline_string = is_multiline_string_literal(init, self.source);

        if is_multiline_string {
            // Multiline strings: mandatory break after `=`
            // Structure: id + " =" + hardline + indent + value
            let id_str = declarator.id.span.extract(self.source);
            let init_doc = self.build_expression_doc(init);

            let assignment_doc = doc::concat(vec![
                doc::text(id_str),
                doc::text(" ="),
                doc::indent(doc::concat(vec![doc::hardline(), init_doc])),
            ]);

            let base_offset = self.config.base_indent_offset * self.config.tab_width;
            let current_col = self.current_column() + base_offset;
            let output = doc::print_doc_at_column(&assignment_doc, &self.config, current_col);
            self.write(&output);
        } else if is_pure_property_chain(init) {
            // Property chains: optional break based on width (fluid layout)
            // - If RHS fits after `= `, stay on one line: `id = value`
            // - If RHS doesn't fit, break after `=` and indent: `id =\n\tvalue`
            //
            // Structure: group(id + " =" + indent(line + rhs))
            // When the group decides to break, line() becomes newline + indent
            let id_str = declarator.id.span.extract(self.source);
            let id_doc = doc::text(id_str);
            let init_doc = self.build_expression_doc(init);

            let assignment_doc = doc::group(doc::concat(vec![
                id_doc,
                doc::text(" ="),
                doc::indent(doc::concat(vec![doc::line(), init_doc])),
            ]));

            let base_offset = self.config.base_indent_offset * self.config.tab_width;
            let current_col = self.current_column() + base_offset;
            let output = doc::print_doc_at_column(&assignment_doc, &self.config, current_col);
            self.write(&output);
        } else {
            // Direct printing for expressions that handle their own wrapping
            self.print_identifier(&declarator.id);
            self.write(" = ");
            self.print_expression(init);
        }
    }

    /// Print a type alias declaration: `type X = T`
    fn print_type_alias_declaration(&mut self, decl: &internal::TSTypeAliasDeclaration) {
        self.write("type ");
        self.print_identifier(&decl.id);
        self.write(" = ");
        self.print_type(&decl.type_annotation);
        self.write(";");
    }

    /// Print a return statement: `return expr;` or `return;`
    fn print_return_statement(&mut self, ret: &internal::ReturnStatement) {
        self.write("return");
        if let Some(arg) = &ret.argument {
            self.write(" ");
            self.print_expression(arg);
        }
        self.write(";");
    }

    /// Print a function declaration: `function foo(x) { return x + 1; }`
    fn print_function_declaration(&mut self, decl: &internal::FunctionDeclaration) {
        self.write("function ");
        self.print_identifier(&decl.id);
        self.write("(");

        // Print parameters
        for (i, param) in decl.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_identifier(param);
        }

        self.write(") ");
        self.print_block_statement(&decl.body);
    }

    /// Build a Doc for a function declaration
    fn build_function_declaration_doc(&self, decl: &internal::FunctionDeclaration) -> doc::Doc {
        let id_str = self.resolve_symbol(decl.id.name);
        let mut parts = vec![doc::text("function "), doc::text(id_str), doc::text("(")];

        // Build params
        for (i, param) in decl.params.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            let param_str = self.resolve_symbol(param.name);
            parts.push(doc::text(param_str));
        }

        parts.push(doc::text(") "));
        parts.push(self.build_block_statement_doc(&decl.body));

        doc::concat(parts)
    }

    /// Print a class declaration
    fn print_class_declaration(&mut self, decl: &internal::ClassDeclaration) {
        self.write("class ");
        self.print_identifier(&decl.id);

        // Handle extends clause
        if let Some(super_class) = &decl.super_class {
            self.write(" extends ");
            self.print_expression(super_class);
        }

        self.write(" ");
        self.print_class_body(&decl.body);
    }

    /// Print a class body
    fn print_class_body(&mut self, body: &internal::ClassBody) {
        if body.body.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{\n");
        self.indent_level += 1;

        for method in &body.body {
            self.write_indent();
            self.print_method_definition(method);
            self.write("\n");
        }

        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
    }

    /// Print a method definition
    fn print_method_definition(&mut self, method: &internal::MethodDefinition) {
        // Print static modifier if applicable
        if method.is_static {
            self.write("static ");
        }

        // Print key
        if method.computed {
            self.write("[");
            self.print_expression(&method.key);
            self.write("]");
        } else {
            self.print_expression(&method.key);
        }

        // Print parameters
        self.write("(");
        for (i, param) in method.value.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_identifier(param);
        }
        self.write(") ");

        // Print body
        self.print_block_statement(&method.value.body);
    }

    /// Build a Doc for a class declaration
    fn build_class_declaration_doc(&self, decl: &internal::ClassDeclaration) -> doc::Doc {
        let id_str = self.resolve_symbol(decl.id.name);
        let mut parts = vec![doc::text("class "), doc::text(id_str)];

        // Handle extends clause
        if let Some(super_class) = &decl.super_class {
            parts.push(doc::text(" extends "));
            parts.push(self.build_expression_doc(super_class));
        }

        parts.push(doc::text(" "));
        parts.push(self.build_class_body_doc(&decl.body));

        doc::concat(parts)
    }

    /// Build a Doc for a class body
    fn build_class_body_doc(&self, body: &internal::ClassBody) -> doc::Doc {
        if body.body.is_empty() {
            return doc::text("{}");
        }

        let mut parts = vec![doc::text("{"), doc::hardline()];

        for (i, method) in body.body.iter().enumerate() {
            if i > 0 {
                parts.push(doc::hardline());
            }
            parts.push(doc::indent(self.build_method_definition_doc(method)));
        }

        parts.push(doc::hardline());
        parts.push(doc::text("}"));

        doc::concat(parts)
    }

    /// Build a Doc for a method definition
    fn build_method_definition_doc(&self, method: &internal::MethodDefinition) -> doc::Doc {
        let mut parts = vec![];

        // Static modifier
        if method.is_static {
            parts.push(doc::text("static "));
        }

        // Key
        if method.computed {
            parts.push(doc::text("["));
            parts.push(self.build_expression_doc(&method.key));
            parts.push(doc::text("]"));
        } else {
            parts.push(self.build_expression_doc(&method.key));
        }

        // Parameters
        parts.push(doc::text("("));
        for (i, param) in method.value.params.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            let param_str = self.resolve_symbol(param.name);
            parts.push(doc::text(param_str));
        }
        parts.push(doc::text(") "));

        // Body
        parts.push(self.build_block_statement_doc(&method.value.body));

        doc::concat(parts)
    }
}
