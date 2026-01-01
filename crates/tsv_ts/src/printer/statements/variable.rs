// Variable declaration printing for TypeScript

use super::Printer;
use crate::ast::internal::{self, Expression};
use crate::printer::{
    ParenContext, is_module_path_fluid_call, is_multiline_string_literal, is_pure_property_chain,
    needs_parens,
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

    /// Build a Doc for a variable declaration statement
    ///
    /// Handles declare, definite assignment (!), type annotations, and multiple declarators.
    /// Follows prettier's rule: if any declarator has an initializer, break to multiple lines.
    pub(in crate::printer) fn build_variable_declaration_doc(
        &self,
        decl: &internal::VariableDeclaration,
    ) -> doc::Doc {
        let mut parts = Vec::new();

        // Declare modifier
        if decl.declare {
            parts.push(doc::text("declare "));
        }

        // Keyword (const, let, var)
        parts.push(doc::text(decl.kind.as_str()));

        // Handle comments between keyword and first declarator
        let keyword_end = if decl.declare {
            decl.span.start + 8 + decl.kind.as_str().len() as u32 // "declare " + keyword
        } else {
            decl.span.start + decl.kind.as_str().len() as u32
        };
        let first_decl_start = decl.declarations[0].span.start;

        if self.has_comments_between(keyword_end, first_decl_start) {
            parts.push(self.build_inline_comments_between_doc(keyword_end, first_decl_start));
            parts.push(doc::text(" "));
        } else {
            parts.push(doc::text(" "));
        }

        let is_multi_declarator = decl.declarations.len() > 1;
        let has_any_init = decl.declarations.iter().any(|d| d.init.is_some());
        let should_break = is_multi_declarator && has_any_init;

        // When breaking to multiple lines, multiline objects/arrays get extra indentation
        // Use save/restore pattern for nested multi-declarator safety
        let old_indent_depth = self.declaration_indent_depth.get();
        if should_break {
            self.declaration_indent_depth.set(old_indent_depth + 1);
        }

        // Build continuation declarators for the non-break case (no initializers)
        // These get wrapped in indent() so when the group breaks, they get continuation indent
        let mut rest_parts = Vec::new();

        // Declarators
        for (i, declarator) in decl.declarations.iter().enumerate() {
            if i > 0 {
                let prev_end = decl.declarations[i - 1].span.end;
                let curr_start = declarator.span.start;

                // Check for comments between declarators
                let has_line_comment = self.has_line_comments_between(prev_end, curr_start);
                let has_block_comment = self.has_comments_between(prev_end, curr_start);

                if should_break {
                    parts.push(doc::text(","));
                    if has_line_comment {
                        // Line comment: print on same line as comma, then break
                        parts.push(self.build_inline_comments_between_doc(prev_end, curr_start));
                    }
                    // Break to new line with indentation for initializers
                    parts.push(doc::hardline());
                    parts.push(doc::text(self.config.indent));
                    if has_block_comment && !has_line_comment {
                        // Block comment: print on new line before declarator
                        parts.push(self.build_inline_comments_between_doc_no_leading_space(
                            prev_end, curr_start,
                        ));
                        parts.push(doc::text(" "));
                    }
                } else {
                    // For non-break case: first continuation gets comma in parts,
                    // subsequent continuations get comma in rest_parts
                    if i == 1 {
                        // First continuation: comma goes to parts (after first declarator)
                        parts.push(doc::text(","));
                    } else {
                        // Subsequent: comma goes to rest_parts (after previous continuation)
                        rest_parts.push(doc::text(","));
                    }

                    // Soft break for declarations without initializers
                    rest_parts.push(doc::line());
                    if has_block_comment {
                        rest_parts.push(self.build_inline_comments_between_doc_no_leading_space(
                            prev_end, curr_start,
                        ));
                        rest_parts.push(doc::text(" "));
                    }
                }
            }

            if should_break || i == 0 {
                // Use build_variable_binding_doc which handles definite assignment
                parts.push(self.build_variable_binding_doc(&declarator.id, declarator.definite));
            } else {
                // Non-break continuation declarators go to rest_parts
                rest_parts
                    .push(self.build_variable_binding_doc(&declarator.id, declarator.definite));
            }

            // Initializer with comment handling around =
            if let Some(init) = &declarator.init {
                let id_end = declarator.id.span().end;
                let init_start = init.span().start;
                let equals_pos = self.find_equals_position(id_end, init_start);
                let has_comments_before_eq = self.has_comments_between(id_end, equals_pos);
                let has_comments_after_eq = self.has_comments_between(equals_pos + 1, init_start);

                if has_comments_before_eq {
                    parts.push(self.build_inline_comments_between_doc(id_end, equals_pos));
                }

                // Check if RHS is a multiline string (line continuations)
                let is_multiline_string = is_multiline_string_literal(init, self.source);

                // Check if RHS needs fluid layout (break after = if too long)
                // Skip fluid layout if:
                // - LHS has expanded pattern or multiline type
                // - Multi-declarator with break (hardlines between declarators, no group)
                let interner = self.interner.borrow();
                let needs_fluid_layout = !should_break
                    && (is_module_path_fluid_call(init, &interner)
                        || is_pure_property_chain(init)
                        || matches!(init, Expression::BinaryExpression(_)))
                    && !self.pattern_should_expand(&declarator.id)
                    && !self.id_has_multiline_type(&declarator.id);
                drop(interner);

                if is_multiline_string {
                    // Multiline strings: mandatory break after `=`
                    parts.push(doc::text(" ="));
                    if has_comments_after_eq {
                        parts.push(
                            self.build_inline_comments_between_doc(equals_pos + 1, init_start),
                        );
                    }
                    parts.push(doc::indent(doc::concat(vec![
                        doc::hardline(),
                        wrap_init_doc(self.build_expression_doc(init), init),
                    ])));
                } else if has_comments_after_eq {
                    parts.push(doc::text(" ="));
                    parts.push(self.build_inline_comments_between_doc(equals_pos + 1, init_start));
                    parts.push(doc::text(" "));
                    parts.push(wrap_init_doc(self.build_expression_doc(init), init));
                } else if needs_fluid_layout {
                    // Module path calls, property chains: fluid layout that breaks after = if too long
                    // Structure: group(id + " =" + indent(line + init))
                    // Can't wrap in group here since we're building parts, so use indent_line directly
                    parts.push(doc::text(" ="));
                    parts.push(doc::indent_line(wrap_init_doc(
                        self.build_expression_doc(init),
                        init,
                    )));
                } else {
                    parts.push(doc::text(" = "));
                    parts.push(wrap_init_doc(self.build_expression_doc(init), init));
                }
            }
        }

        // For non-break multi-declarator, add rest_parts wrapped in indent
        if !should_break && !rest_parts.is_empty() {
            parts.push(doc::indent(doc::concat(rest_parts)));
        }

        parts.push(doc::text(";"));

        // Restore multi-declarator context
        self.declaration_indent_depth.set(old_indent_depth);

        if should_break {
            doc::concat(parts)
        } else if is_multi_declarator {
            // Use group for soft line breaks without initializers
            doc::group(doc::concat(parts))
        } else if has_any_init {
            // Single declarator with init: use group for width-based breaking
            doc::group(doc::concat(parts))
        } else {
            doc::concat(parts)
        }
    }
}
