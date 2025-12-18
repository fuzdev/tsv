// Variable declaration printing for TypeScript

use super::super::Printer;
use crate::ast::internal::{self, Expression};
use crate::printer::{
    ParenContext, is_module_path_fluid_call, is_multiline_string_literal, is_pure_property_chain,
    needs_doc_based_wrapping, needs_parens,
};
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
    pub(super) fn print_variable_declarator(&mut self, declarator: &internal::VariableDeclarator) {
        // Check if we have an initializer - if not, just print the binding pattern
        let Some(init) = &declarator.init else {
            self.print_expression(&declarator.id);
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
            self.print_expression(&declarator.id);
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
            let id_doc = self.build_expression_doc(&declarator.id);
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
            let id_doc = self.build_expression_doc(&declarator.id);
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
            let id_doc = self.build_expression_doc(&declarator.id);
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
            let id_doc = self.build_expression_doc(&declarator.id);
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
            self.print_expression(&declarator.id);
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
}
