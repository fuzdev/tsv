// Variable declaration printing for TypeScript

use super::super::Printer;
use crate::ast::internal::{self, Expression};
use crate::printer::{
    ParenContext, is_module_path_fluid_call, is_multiline_string_literal, is_pure_property_chain,
    needs_doc_based_wrapping, needs_parens,
};
use tsv_lang::SymbolResolver;
use tsv_lang::doc;

/// Wrap a doc in parentheses if the expression needs them for variable init context
fn wrap_init_doc(init_doc: doc::Doc, init: &Expression) -> doc::Doc {
    if needs_parens(init, ParenContext::VariableInit) {
        doc::concat(vec![doc::text("("), init_doc, doc::text(")")])
    } else {
        init_doc
    }
}

impl<'a> Printer<'a> {
    /// Build a doc for a variable binding pattern with optional definite assignment assertion
    ///
    /// For identifiers with `definite: true`, builds doc for `name!: type` instead of `name: type`.
    fn build_variable_binding_doc(&self, id: &Expression, definite: bool) -> doc::Doc {
        if definite {
            if let Expression::Identifier(ident) = id {
                let name = self.resolve_symbol(ident.name);
                let mut parts = vec![doc::text_owned(name), doc::text("!")];

                if let Some(type_annotation) = &ident.type_annotation {
                    parts.push(self.build_type_annotation_doc(type_annotation));
                }

                doc::concat(parts)
            } else {
                // Destructuring patterns don't support definite assignment
                self.build_expression_doc(id)
            }
        } else {
            self.build_expression_doc(id)
        }
    }

    /// Print a variable binding pattern with optional definite assignment assertion
    ///
    /// For identifiers with `definite: true`, prints `name!: type` instead of `name: type`.
    fn print_variable_binding(&mut self, id: &Expression, definite: bool) {
        if definite {
            // For definite assignment, we need to insert `!` between name and type annotation
            if let Expression::Identifier(ident) = id {
                // Print decorators (rare for variable declarations, but handle for completeness)
                if let Some(decorators) = &ident.decorators {
                    for decorator in decorators {
                        self.write("@");
                        self.print_expression(&decorator.expression);
                        self.write(" ");
                    }
                }

                // Print the identifier name
                let name = self.resolve_symbol(ident.name);
                self.write(&name);

                // Print definite assignment marker
                self.write("!");

                // Print type annotation if present
                if let Some(type_annotation) = &ident.type_annotation {
                    self.print_type_annotation(type_annotation);
                }
            } else {
                // Destructuring patterns don't support definite assignment
                // (parser sets definite=false for these)
                self.print_expression(id);
            }
        } else {
            // Normal case: just print the expression
            self.print_expression(id);
        }
    }

    /// Print a variable declaration
    pub(super) fn print_variable_declaration(&mut self, decl: &internal::VariableDeclaration) {
        // Print declare modifier if present
        if decl.declare {
            self.write("declare ");
        }

        // Write the keyword (const, let, var)
        let keyword_start = if decl.declare {
            // Skip "declare " (8 chars) to find actual keyword start
            decl.span.start + 8
        } else {
            decl.span.start
        };
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
        // Prettier's rule: if ANY declarator has an initializer, break to multiple lines
        // If no initializers, use soft breaks that break at print_width
        let is_multi_declarator = decl.declarations.len() > 1;
        let has_any_init = decl.declarations.iter().any(|d| d.init.is_some());
        let should_break = is_multi_declarator && has_any_init;

        // When no initializers: use doc-based printing with line separators
        // that break at print_width (prettier uses `line` separator)
        if is_multi_declarator && !has_any_init {
            // Check for comments between declarators - fall back to direct printing if present
            let has_comments = decl.declarations.windows(2).any(|pair| {
                let prev_end = pair[0].span.end;
                let curr_start = pair[1].span.start;
                self.has_comments_between(prev_end, curr_start)
            });

            if !has_comments {
                // Use doc-based printing with soft breaks
                // Structure: group(d1 + "," + indent(line + d2 + "," + line + d3 + ...))
                let mut parts = vec![];
                let mut rest_parts = vec![];
                let mut last_span_end = 0u32;

                for (i, declarator) in decl.declarations.iter().enumerate() {
                    last_span_end = declarator.span.end;
                    let decl_doc =
                        self.build_variable_binding_doc(&declarator.id, declarator.definite);
                    if i == 0 {
                        parts.push(decl_doc);
                        parts.push(doc::text(","));
                    } else {
                        rest_parts.push(doc::line());
                        rest_parts.push(decl_doc);
                        if i < decl.declarations.len() - 1 {
                            rest_parts.push(doc::text(","));
                        }
                    }
                }

                parts.push(doc::indent(doc::concat(rest_parts)));
                parts.push(doc::text(";"));

                let declarator_doc = doc::group(doc::concat(parts));

                // Print using doc system with current column position
                let base_offset = self.config.base_indent_offset * self.config.tab_width;
                let current_col = self.current_column() + base_offset;
                let output = {
                    let interner = self.interner.borrow();
                    doc::print_doc_at_column_resolved(
                        &declarator_doc,
                        &self.config,
                        current_col,
                        &*interner,
                    )
                };
                self.write(&output);

                // Handle trailing comments
                self.print_inline_comments_in_statement(last_span_end, decl.span.end);

                return;
            }
        }

        // For single declarators, we can use doc-based printing for the whole statement
        // This ensures the semicolon is included in the width calculation
        if decl.declarations.len() == 1 {
            let declarator = &decl.declarations[0];
            if let Some(stmt_doc) = self.try_build_single_declarator_stmt_doc(declarator) {
                self.write_doc(&stmt_doc);
                // Handle trailing comments
                self.print_inline_comments_in_statement(declarator.span.end, decl.span.end);
                return;
            }
        }

        // When breaking to multiple lines, multiline objects/arrays get extra indentation
        // Use save/restore pattern for nested multi-declarator safety
        let old_indent_depth = self.declaration_indent_depth;
        if should_break {
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

                if should_break || has_line_comment {
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
                } else {
                    // Keep on same line: just add space after comma
                    self.write(" ");
                    // Print any block comments inline
                    self.print_inline_comments_between(prev_end, curr_start);
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
    pub(super) fn print_variable_declarator(&mut self, declarator: &internal::VariableDeclarator) {
        // Check if we have an initializer - if not, just print the binding pattern
        let Some(init) = &declarator.init else {
            self.print_variable_binding(&declarator.id, declarator.definite);
            return;
        };

        // Handle comments around the equals sign
        let id_end = declarator.id.span().end;
        let init_start = init.span().start;
        let equals_pos = self.find_equals_position(id_end, init_start);
        let has_comments_before_eq = self.has_comments_between(id_end, equals_pos);
        let has_comments_after_eq = self.has_comments_between(equals_pos + 1, init_start);

        // If there are comments, use direct printing (comment handling with doc IR is complex)
        if has_comments_before_eq || has_comments_after_eq {
            self.print_variable_binding(&declarator.id, declarator.definite);
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
            // Wrap assignment expressions in parens for clarity
            if needs_parens(init, ParenContext::VariableInit) {
                self.write("(");
                self.print_expression(init);
                self.write(")");
            } else {
                self.print_expression(init);
            }
            return;
        }

        // Check if RHS is a multiline string (line continuations)
        // Prettier ALWAYS wraps these - it's not a width decision, it's mandatory
        let is_multiline_string = is_multiline_string_literal(init, self.source);

        if is_multiline_string {
            // Multiline strings: mandatory break after `=`
            // Structure: id + " =" + hardline + indent + value
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);
            let init_doc = wrap_init_doc(self.build_expression_doc(init), init);

            let assignment_doc = doc::concat(vec![
                id_doc,
                doc::text(" ="),
                doc::indent(doc::concat(vec![doc::hardline(), init_doc])),
            ]);

            self.write_doc(&assignment_doc);
        } else if (is_pure_property_chain(init)
            || is_module_path_fluid_call(init, &self.interner.borrow())
            || matches!(init, Expression::BinaryExpression(_)))
            && !self.pattern_should_expand(&declarator.id)
            && !self.id_has_multiline_type(&declarator.id)
        {
            // Property chains, module path calls, and binary expressions: optional break based on width
            // - If RHS fits after `= `, stay on one line: `id = value`
            // - If RHS doesn't fit, break after `=` and indent: `id =\n\tvalue`
            //
            // Structure: group(id + " =" + indent(line + rhs))
            // When the group decides to break, line() becomes newline + indent
            //
            // Binary expressions have internal groups with line() elements that allow
            // operand-level breaking when the expression is too long.
            //
            // Note: Skip this for expanded patterns and multiline type annotations -
            // the group-based approach causes unwanted line breaks after `=`.
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);
            let init_doc = wrap_init_doc(self.build_expression_doc(init), init);

            let assignment_doc = doc::group(doc::concat(vec![
                id_doc,
                doc::text(" ="),
                doc::indent_line(init_doc),
            ]));

            self.write_doc(&assignment_doc);
        } else if needs_doc_based_wrapping(init) {
            // Expressions with internal groups that need width-based evaluation
            // e.g., `!!(a || b || c)`, `new (a || b || c)()`
            //
            // These expressions have doc groups inside them that decide whether
            // to break based on line width. We need to use the doc system to
            // evaluate those groups.
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);
            let init_doc = wrap_init_doc(self.build_expression_doc(init), init);

            let assignment_doc = doc::concat(vec![id_doc, doc::text(" = "), init_doc]);

            self.write_doc(&assignment_doc);
        } else if matches!(
            declarator.id,
            Expression::ObjectPattern(_) | Expression::ArrayPattern(_)
        ) {
            // Destructuring patterns with groups that need width-based evaluation
            // e.g., `let {a, b}: {a: number; b: string} = obj`
            //
            // ObjectPattern and ArrayPattern contain groups for width-based expansion.
            // We need to check if the FULL statement would exceed print width, not just
            // the pattern, because the group makes its break decision in isolation.
            //
            // Pre-calculate if expanding is needed by measuring flat width of full statement.
            // Note: Destructuring patterns don't support definite assignment, so we can use
            // build_expression_doc directly (definite is always false for patterns).
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);
            let init_doc = wrap_init_doc(self.build_expression_doc(init), init);

            let assignment_doc = doc::concat(vec![id_doc, doc::text(" = "), init_doc]);

            let base_offset = self.config.base_indent_offset * self.config.tab_width;
            let current_col = self.current_column() + base_offset;

            // Check if the flat representation would fit
            let remaining = self.config.print_width.saturating_sub(current_col);
            let would_fit = {
                let interner = self.interner.borrow();
                doc::fits_resolved(
                    &assignment_doc,
                    remaining,
                    doc::Mode::Flat,
                    &self.config,
                    &*interner,
                )
            };

            if !would_fit {
                // "break-lhs" pattern: LHS breaks independently (has hardlines),
                // but " = value" stays together on one line
                // Don't wrap in group - hardlines in LHS would force group to break
                let id_doc = self.build_expression_doc_forced_expand(&declarator.id);
                let init_doc = wrap_init_doc(self.build_expression_doc(init), init);
                let forced_doc = doc::concat(vec![id_doc, doc::text(" = "), init_doc]);
                let output = {
                    let interner = self.interner.borrow();
                    doc::print_doc_at_column_resolved(
                        &forced_doc,
                        &self.config,
                        current_col,
                        &*interner,
                    )
                };
                self.write(&output);
            } else {
                // Use doc-based printing for width-based decisions
                let output = {
                    let interner = self.interner.borrow();
                    doc::print_doc_at_column_resolved(
                        &assignment_doc,
                        &self.config,
                        current_col,
                        &*interner,
                    )
                };
                self.write(&output);
            }
        } else {
            // Direct printing for expressions that handle their own wrapping
            self.print_variable_binding(&declarator.id, declarator.definite);
            self.write(" = ");
            // Wrap assignment expressions in parens for clarity
            if needs_parens(init, ParenContext::VariableInit) {
                self.write("(");
                self.print_expression(init);
                self.write(")");
            } else {
                self.print_expression(init);
            }
        }
    }

    /// Try to build a doc for a single variable declarator statement (including semicolon)
    ///
    /// Returns `Some(doc)` if the declarator needs doc-based wrapping (binary expressions,
    /// property chains, etc.), `None` otherwise to fall back to direct printing.
    ///
    /// This includes the semicolon in the doc so the width calculation is accurate.
    fn try_build_single_declarator_stmt_doc(
        &self,
        declarator: &internal::VariableDeclarator,
    ) -> Option<doc::Doc> {
        let init = declarator.init.as_ref()?;

        // Check for comments around the equals sign - use direct printing for those
        let id_end = declarator.id.span().end;
        let init_start = init.span().start;
        let equals_pos = self.find_equals_position(id_end, init_start);
        if self.has_comments_between(id_end, equals_pos)
            || self.has_comments_between(equals_pos + 1, init_start)
        {
            return None;
        }

        // Check if RHS is a multiline string (line continuations)
        let is_multiline_string = is_multiline_string_literal(init, self.source);

        if is_multiline_string {
            // Multiline strings: mandatory break after `=`
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);
            let init_doc = wrap_init_doc(self.build_expression_doc(init), init);

            Some(doc::concat(vec![
                id_doc,
                doc::text(" ="),
                doc::indent(doc::concat(vec![doc::hardline(), init_doc])),
                doc::text(";"),
            ]))
        } else if (is_pure_property_chain(init)
            || is_module_path_fluid_call(init, &self.interner.borrow())
            || matches!(init, Expression::BinaryExpression(_)))
            && !self.pattern_should_expand(&declarator.id)
            && !self.id_has_multiline_type(&declarator.id)
        {
            // Property chains, module path calls, and binary expressions: optional break
            // Include semicolon in the doc so width calculation is accurate
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);
            let init_doc = wrap_init_doc(self.build_expression_doc(init), init);

            Some(doc::group(doc::concat(vec![
                id_doc,
                doc::text(" ="),
                doc::indent_line(init_doc),
                doc::text(";"),
            ])))
        } else if needs_doc_based_wrapping(init) {
            // Expressions with internal groups that need width-based evaluation
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);
            let init_doc = wrap_init_doc(self.build_expression_doc(init), init);

            Some(doc::concat(vec![
                id_doc,
                doc::text(" = "),
                init_doc,
                doc::text(";"),
            ]))
        } else {
            // Fall back to direct printing
            None
        }
    }
}
