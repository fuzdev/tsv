// Variable declaration printing for TypeScript

use super::Printer;
use crate::ast::internal::{self, Expression};
use crate::printer::{
    ParenContext, conditional_needs_fluid_layout, is_module_path_fluid_call,
    is_multiline_string_literal, is_plain_require_call, is_poorly_breakable_chain,
    is_pure_property_chain, is_self_expanding_value, is_simple_self_expanding, needs_parens,
};
use tsv_lang::SymbolToU32;
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
    /// Build a doc for a variable binding pattern with optional definite assignment assertion.
    ///
    /// For identifiers with `definite: true`, builds doc for `name!: type` instead of `name: type`.
    /// Uses wrapping type annotations so TypeReference type arguments break internally.
    fn build_variable_binding_doc(&self, id: &Expression, definite: bool) -> doc::Doc {
        if definite {
            if let Expression::Identifier(ident) = id {
                let mut parts = vec![doc::symbol(ident.name.to_u32()), doc::text("!")];

                if let Some(type_annotation) = &ident.type_annotation {
                    // Use wrapping version for TypeReference type args
                    parts.push(self.build_type_annotation_doc_wrapping(type_annotation));
                }

                doc::concat(parts)
            } else {
                // Destructuring patterns don't support definite assignment
                self.build_expression_doc(id)
            }
        } else if let Expression::Identifier(ident) = id {
            // Use wrapping version for identifiers so TypeReference type args break internally
            self.build_identifier_doc_with_wrapping_type(ident)
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

        // Set top-level assignment flag for chain detection
        // Short 2-segment assignment chains in variable declarations should not use chain formatting
        self.in_top_level_assignment.set(true);

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

            // Build id doc once for reuse and analysis
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);

            // Check if id doc can break (contains line elements like type annotations that wrap)
            // This matches Prettier's `canBreak(leftDoc)` check
            let can_break_left = doc::can_break(&id_doc);

            if should_break || i == 0 {
                parts.push(id_doc.clone());
            } else {
                // Non-break continuation declarators go to rest_parts
                rest_parts.push(id_doc.clone());
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

                // Check if LHS triggers break-lhs layout:
                // 1. Complex type annotation - nested generics that should break internally
                // 2. Complex destructuring - >2 properties with defaults/non-shorthand
                // 3. Arrow function with breakable LHS (long type annotation)
                //
                // Example type annotation: `const x: Map<string, Array<number>> = getLongValue()`
                // Should break as:
                //   const x: Map<
                //     string,
                //     Array<number>
                //   > = getLongValue();
                //
                // Example destructuring: `const { a, b = 1, c } = obj`
                // Should break as:
                //   const {
                //     a,
                //     b = 1,
                //     c,
                //   } = obj;
                //
                // Example arrow with long type: `const fn: (x: number) => void = (x) => {}`
                // When type is long enough to wrap:
                //   const fn: (
                //     x: number,
                //   ) => void = (x) => {};
                let has_complex_type_annotation =
                    self.id_has_complex_type_annotation(&declarator.id);
                let has_complex_destructuring = self.id_has_complex_destructuring(&declarator.id);
                let is_arrow_with_breakable_left =
                    matches!(init, Expression::ArrowFunctionExpression(_)) && can_break_left;

                // Break-after-operator layout: group([left, " =", group(indent([line, right]))])
                // Used for fluid RHS or simple RHS when LHS can break.
                let interner = self.interner.borrow();
                let is_plain_require = is_plain_require_call(init, &interner);

                let is_fluid_rhs = (is_module_path_fluid_call(init, &interner)
                    || is_pure_property_chain(init)
                    || is_poorly_breakable_chain(init, self.source, self.config.print_width)
                    || matches!(init, Expression::BinaryExpression(_))
                    || conditional_needs_fluid_layout(init))
                    && !is_self_expanding_value(init);

                let is_simple_rhs_with_breakable_lhs =
                    can_break_left && is_simple_self_expanding(init);

                let needs_break_after_operator = !should_break
                    && !is_plain_require
                    && (is_fluid_rhs || is_simple_rhs_with_breakable_lhs)
                    && !doc::will_break(&id_doc)
                    && !has_complex_type_annotation
                    && !has_complex_destructuring
                    && !is_arrow_with_breakable_left;
                drop(interner);

                // Check for line comments after = which force a break
                let has_line_comments_after_eq =
                    self.has_line_comments_between(equals_pos + 1, init_start);

                if is_multiline_string || has_line_comments_after_eq {
                    // Multiline strings or line comments: mandatory break after `=`
                    parts.push(doc::text(" ="));
                    if has_comments_after_eq {
                        // For line comments, print them and break
                        for comment in
                            tsv_lang::comments_in_range(self.comments, equals_pos + 1, init_start)
                        {
                            if tsv_lang::printing::is_same_line(
                                self.source,
                                equals_pos,
                                comment.span.start,
                            ) {
                                // Inline comment on same line as =
                                parts.push(doc::text(" "));
                                parts.push(self.build_comment_doc(comment));
                            }
                        }
                    }
                    parts.push(doc::indent(doc::concat(vec![
                        doc::hardline(),
                        // Leading comments (on their own line before value)
                        {
                            let mut leading = Vec::new();
                            for comment in tsv_lang::comments_in_range(
                                self.comments,
                                equals_pos + 1,
                                init_start,
                            ) {
                                if !tsv_lang::printing::is_same_line(
                                    self.source,
                                    equals_pos,
                                    comment.span.start,
                                ) {
                                    leading.push(self.build_comment_doc(comment));
                                    leading.push(doc::hardline());
                                }
                            }
                            doc::concat(leading)
                        },
                        wrap_init_doc(self.build_expression_doc(init), init),
                    ])));
                } else if (has_complex_type_annotation
                    || has_complex_destructuring
                    || is_arrow_with_breakable_left)
                    && (should_break || i == 0)
                {
                    // Break-lhs layout: LHS breaks internally, `=` stays on same line with RHS
                    // Only applies to first declarator or multi-declarator with breaks
                    //
                    // Three cases:
                    // 1. Complex type annotations: Need wrapping version - rebuild with `build_type_annotation_doc_wrapping()`
                    // 2. Complex destructuring: Regular doc already correct (pattern handles breaking)
                    // 3. Arrow function with breakable LHS: Regular doc already correct (has line elements)

                    // Only rebuild for complex type annotations - others use the id_doc we already built
                    if has_complex_type_annotation
                        && matches!(&declarator.id, Expression::Identifier(_))
                    {
                        // Complex type annotation: build custom doc with wrapping
                        if let Expression::Identifier(ident) = &declarator.id {
                            let mut id_parts = vec![doc::symbol(ident.name.to_u32())];
                            if declarator.definite {
                                id_parts.push(doc::text("!"));
                            }
                            if ident.optional {
                                id_parts.push(doc::text("?"));
                            }
                            if let Some(type_ann) = &ident.type_annotation {
                                id_parts.push(self.build_type_annotation_doc_wrapping(type_ann));
                            }
                            let id_doc_wrapping = doc::concat(id_parts);

                            // Replace the regular id_doc with wrapping version
                            parts.pop();
                            parts.push(id_doc_wrapping);
                        }
                    }
                    // else: Complex destructuring or arrow with breakable left already have correct id_doc in parts

                    // Add ` = rightDoc` (right side grouped)
                    parts.push(doc::text(" = "));
                    parts.push(doc::group(wrap_init_doc(
                        self.build_expression_doc(init),
                        init,
                    )));
                } else if has_comments_after_eq {
                    parts.push(doc::text(" ="));
                    parts.push(self.build_inline_comments_between_doc(equals_pos + 1, init_start));
                    parts.push(doc::text(" "));
                    parts.push(wrap_init_doc(self.build_expression_doc(init), init));
                } else if needs_break_after_operator {
                    // Break-after-operator layout with nested groups - matches prettier exactly
                    // Structure: group([leftParts, " =", group(indent([line, init]))])
                    // The inner group for init evaluates independently based on remaining width.
                    // This allows the type annotation to expand while keeping `} = init` together.
                    parts.push(doc::text(" ="));
                    parts.push(doc::group(doc::indent(doc::concat(vec![
                        doc::line(),
                        wrap_init_doc(self.build_expression_doc(init), init),
                    ]))));
                } else {
                    // Default layout - no line element
                    // The outer group (added at the end) handles whether the whole declaration breaks
                    // Individual expressions (calls, arrays, objects) handle their own internal breaking
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

        // Restore context flags
        self.declaration_indent_depth.set(old_indent_depth);
        self.in_top_level_assignment.set(false);

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
