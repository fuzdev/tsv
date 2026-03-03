// Variable declaration printing for TypeScript

use super::Printer;
use crate::ast::internal::{self, Expression};
use crate::printer::{
    ParenContext, conditional_needs_fluid_layout, is_call_on_member_chain,
    is_curried_arrow_with_return_type, is_module_path_fluid_call, is_multiline_string_literal,
    is_poorly_breakable_chain, is_pure_property_chain, is_self_expanding_value,
    is_simple_self_expanding, is_single_call_on_member_chain, is_string_literal,
    is_type_assertion_call, needs_parens,
};
use tsv_lang::SymbolToU32;
use tsv_lang::doc::GroupId;
use tsv_lang::doc::arena::{DocArena, DocId};

/// Wrap a doc in parentheses if the expression needs them for variable init context
fn wrap_init_doc(d: &DocArena, init_doc: DocId, init: &Expression) -> DocId {
    if needs_parens(init, ParenContext::VariableInit) {
        d.parens(init_doc)
    } else {
        init_doc
    }
}

impl<'a> Printer<'a> {
    /// Build a doc for a variable binding pattern with optional definite assignment assertion.
    ///
    /// For identifiers with `definite: true`, builds doc for `name!: type` instead of `name: type`.
    /// Uses wrapping type annotations so TypeReference type arguments break internally when needed.
    fn build_variable_binding_doc(&self, id: &Expression, definite: bool) -> DocId {
        if definite {
            if let Expression::Identifier(ident) = id {
                self.build_typed_identifier_doc(ident, true, true)
            } else {
                // Destructuring patterns don't support definite assignment
                self.build_expression_doc(id)
            }
        } else if let Expression::Identifier(ident) = id {
            self.build_identifier_doc_with_wrapping_type(ident)
        } else {
            self.build_expression_doc(id)
        }
    }

    /// Build doc for an identifier with type annotation, configurable wrapping.
    ///
    /// - `definite`: include `!` after name
    /// - `wrap_type`: use wrapping type annotation (breaks internally) vs non-wrapping (stays on one line)
    fn build_typed_identifier_doc(
        &self,
        ident: &internal::Identifier,
        definite: bool,
        wrap_type: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![d.symbol(ident.name.to_u32())];
        if definite {
            parts.push(d.text("!"));
        }
        if ident.optional {
            parts.push(d.text("?"));
        }
        if let Some(type_ann) = &ident.type_annotation {
            if wrap_type {
                parts.push(self.build_type_annotation_doc_wrapping(type_ann));
            } else {
                parts.push(self.build_type_annotation_doc(type_ann));
            }
        }
        d.concat(&parts)
    }

    /// Build a Doc for a variable declaration statement
    ///
    /// Handles declare, definite assignment (!), type annotations, and multiple declarators.
    /// Follows prettier's rule: if any declarator has an initializer, break to multiple lines.
    pub(in crate::printer) fn build_variable_declaration_doc(
        &self,
        decl: &internal::VariableDeclaration,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Declare modifier
        if decl.declare {
            parts.push(d.text("declare "));
        }

        // Keyword (const, let, var)
        parts.push(d.text(decl.kind.as_str()));

        // Handle comments between keyword and first declarator
        let keyword_end = if decl.declare {
            decl.span.start + 8 + decl.kind.as_str().len() as u32 // "declare " + keyword
        } else {
            decl.span.start + decl.kind.as_str().len() as u32
        };
        let first_decl_start = decl.declarations[0].span.start;

        // Use _opt variant to avoid redundant binary search
        if let Some(comments_doc) =
            self.build_inline_comments_between_doc_opt(keyword_end, first_decl_start)
        {
            parts.push(comments_doc);
        }
        parts.push(d.text(" "));

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
                    parts.push(d.text(","));
                    if has_line_comment {
                        // Line comment: print on same line as comma, then break
                        parts.push(self.build_inline_comments_between_doc(prev_end, curr_start));
                    }
                    // Break to new line with indentation for initializers
                    parts.push(d.hardline());
                    parts.push(d.text(self.config.indent));
                    if has_block_comment && !has_line_comment {
                        // Block comment: print on new line before declarator
                        parts.push(self.build_inline_comments_between_doc_no_leading_space(
                            prev_end, curr_start,
                        ));
                        parts.push(d.text(" "));
                    }
                } else {
                    // For non-break case: first continuation gets comma in parts,
                    // subsequent continuations get comma in rest_parts
                    if i == 1 {
                        // First continuation: comma goes to parts (after first declarator)
                        parts.push(d.text(","));
                    } else {
                        // Subsequent: comma goes to rest_parts (after previous continuation)
                        rest_parts.push(d.text(","));
                    }

                    // Soft break for declarations without initializers
                    rest_parts.push(d.line());
                    if has_block_comment {
                        rest_parts.push(self.build_inline_comments_between_doc_no_leading_space(
                            prev_end, curr_start,
                        ));
                        rest_parts.push(d.text(" "));
                    }
                }
            }

            // Build id doc once for reuse and analysis
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);

            // Check if id doc can break (contains line elements like type annotations that wrap)
            // This matches Prettier's `canBreak(leftDoc)` check
            let can_break_left = d.can_break(id_doc);

            if should_break || i == 0 {
                parts.push(id_doc);
            } else {
                // Non-break continuation declarators go to rest_parts
                rest_parts.push(id_doc);
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

                // Calls and imports with trailing comments expand internally and should not use fluid layout
                let is_call_with_trailing_comments = if let Expression::CallExpression(call) = init
                {
                    call.arguments.last().is_some_and(|last_arg| {
                        self.has_line_comments_between(last_arg.span().end, call.span.end)
                    })
                } else {
                    false
                };

                // Import expressions with trailing comments also expand internally
                // (handles `await import('./x' // comment)`)
                let is_import_with_trailing_comments = self.has_import_with_trailing_comments(init);

                // Call chains with line comments should NOT be treated as fluid.
                // The chain formatter handles breaking at the comment location.
                // E.g., `const a = items // comment\n  .foo()` keeps `= items // comment` together.
                let has_line_comments_in_chain = matches!(init, Expression::CallExpression(_))
                    && self.has_line_comments_in_call_chain(init);

                // Combined flag for expressions with trailing comments that expand internally
                let has_trailing_comment_expansion =
                    is_call_with_trailing_comments || is_import_with_trailing_comments;

                // Expressions that benefit from FLUID layout (Prettier's indentIfBreak pattern):
                // The printer tries to break at `=` BEFORE evaluating the RHS's internal groups.
                // This gives better results for chains where expanding internal groups is worse
                // than breaking at the assignment.
                let needs_fluid_layout = (is_module_path_fluid_call(init, &interner)
                    || is_pure_property_chain(init)
                    || is_poorly_breakable_chain(init, self.source, self.config.print_width)
                    || is_string_literal(init)
                    || matches!(init, Expression::RegexLiteral(_)))
                    && !is_self_expanding_value(init)
                    && !has_trailing_comment_expansion
                    && !has_line_comments_in_chain;

                // Single-call member chains with complex args (arrows, objects, arrays):
                // Use TRUE fluid layout to break at `=` only when necessary.
                // E.g., `const x = a.b.c.filter((x) => ...)` breaks at `=` if > print_width
                let is_single_call_member_chain = is_call_on_member_chain(init)
                    && !is_self_expanding_value(init)
                    && !has_trailing_comment_expansion
                    && !has_line_comments_in_chain;

                // Expressions that need break-after-operator layout (old style):
                // group([left, " =", indent([line, right])])
                // For binary/logical expressions, breaking happens at operators within the RHS,
                // and the entire RHS is indented together after `=`.
                let needs_break_after_op_layout = (matches!(init, Expression::BinaryExpression(_))
                    || conditional_needs_fluid_layout(init))
                    && !is_self_expanding_value(init)
                    && !has_trailing_comment_expansion
                    && !has_line_comments_in_chain;

                // Combined flag for backward compatibility with existing logic
                let is_fluid_rhs = needs_fluid_layout || needs_break_after_op_layout;

                // Member-chain call (a.fn(...)) where the call head fits within print_width:
                // Use default layout and let the call expand its own args rather than breaking
                // at `=`. E.g., `const {a, b} = vi.mocked(longArg)` with short LHS keeps
                // `= vi.mocked(` on line 1 and expands the arg — matching Prettier's behavior.
                // Only fires when call head (decl_start to callee_end + "(") fits in print_width.
                let is_expandable_member_call = if is_single_call_on_member_chain(init) {
                    if let Expression::CallExpression(call) = init {
                        let indent_visual_width =
                            (self.config.base_indent_offset
                                + self.declaration_indent_depth.get())
                                * self.config.tab_width;
                        // +1 for the "(" after callee
                        let call_head_width = indent_visual_width
                            + (call.callee.span().end as usize - decl.span.start as usize)
                            + 1;
                        call_head_width < self.config.print_width
                    } else {
                        false
                    }
                } else {
                    false
                };

                // Breakable LHS (destructuring patterns) with non-self-expanding RHS:
                // Use fluid layout so the printer breaks at `=` before expanding the
                // destructuring pattern. Matches Prettier's `canBreak(leftDoc) → "fluid"`.
                // E.g., `const {a, b, c} = resolve(x, y, z)` breaks after `=`, not inside `{}`
                //
                // Excludes is_fluid_rhs cases (binary, conditional, strings, chains) — those
                // go through needs_break_after_operator with their own break-after-operator layout.
                // In Prettier, shouldBreakAfterOperator() handles those before the canBreak fallback.
                //
                // Excludes is_expandable_member_call: when the call head fits, the call's own
                // arg-expansion handles line breaking via default layout.
                let needs_fluid_for_breakable_lhs = can_break_left
                    && !is_self_expanding_value(init)
                    && !has_trailing_comment_expansion
                    && !has_line_comments_in_chain
                    && !should_break
                    && !is_fluid_rhs
                    && !is_expandable_member_call;

                // Type assertion calls with LHS type annotation need special fluid handling
                // (handled separately below because they need non-wrapping LHS type)
                let is_type_assertion_with_lhs_type = is_type_assertion_call(
                    init,
                    self.source,
                    self.config.print_width,
                ) && matches!(&declarator.id, Expression::Identifier(id) if id.type_annotation.is_some());

                let is_simple_rhs_with_breakable_lhs =
                    can_break_left && is_simple_self_expanding(init);

                let needs_break_after_operator = !should_break
                    && (is_fluid_rhs || is_simple_rhs_with_breakable_lhs)
                    && !d.will_break(id_doc)
                    && !has_complex_type_annotation
                    && !has_complex_destructuring
                    && !is_arrow_with_breakable_left;
                drop(interner);

                // Check for line comments after = which force a break
                let has_line_comments_after_eq =
                    self.has_line_comments_between(equals_pos + 1, init_start);

                // Curried arrows with return type always break after `=`
                let is_curried_arrow = is_curried_arrow_with_return_type(init);

                if is_multiline_string || has_line_comments_after_eq {
                    // Multiline strings or line comments: mandatory break after `=`
                    parts.push(d.text(" ="));
                    if has_comments_after_eq {
                        // For line comments, print them and break
                        for comment in
                            tsv_lang::comments_in_range(self.comments, equals_pos + 1, init_start)
                        {
                            if self.is_same_line(equals_pos, comment.span.start) {
                                // Inline comment on same line as =
                                parts.push(d.text(" "));
                                parts.push(self.build_comment_doc(comment));
                            }
                        }
                    }
                    parts.push(d.indent(d.concat(&[
                        d.hardline(),
                        // Leading comments (on their own line before value)
                        {
                            let mut leading = Vec::new();
                            for comment in tsv_lang::comments_in_range(
                                self.comments,
                                equals_pos + 1,
                                init_start,
                            ) {
                                if !self.is_same_line(equals_pos, comment.span.start) {
                                    leading.push(self.build_comment_doc(comment));
                                    leading.push(d.hardline());
                                }
                            }
                            d.concat(&leading)
                        },
                        wrap_init_doc(d, self.build_expression_doc(init), init),
                    ])));
                } else if is_curried_arrow {
                    // Curried arrow with return type: mandatory break after `=`
                    // The arrow expression formatter handles the rest of the breaking
                    parts.push(d.text(" ="));
                    parts.push(d.indent(d.concat(&[
                        d.hardline(),
                        wrap_init_doc(d, self.build_expression_doc(init), init),
                    ])));
                } else if (has_complex_type_annotation
                    || has_complex_destructuring
                    || is_arrow_with_breakable_left)
                    && (should_break || i == 0)
                {
                    // Break-lhs layout: LHS breaks internally, `=` stays on same line with RHS
                    // Only applies to first declarator or multi-declarator with breaks
                    //
                    // For complex type annotations, rebuild with wrapping type.
                    // Complex destructuring and arrow with breakable left already have correct id_doc.
                    if has_complex_type_annotation
                        && let Expression::Identifier(ident) = &declarator.id
                    {
                        parts.pop();
                        parts.push(self.build_typed_identifier_doc(
                            ident,
                            declarator.definite,
                            true, // wrap_type
                        ));
                    }

                    // Add ` = rightDoc` (right side grouped)
                    parts.push(d.text(" = "));
                    parts.push(d.group(wrap_init_doc(d, self.build_expression_doc(init), init)));
                } else if has_comments_after_eq {
                    parts.push(d.text(" ="));
                    parts.push(self.build_inline_comments_between_doc(equals_pos + 1, init_start));
                    parts.push(d.text(" "));
                    parts.push(wrap_init_doc(d, self.build_expression_doc(init), init));
                } else if is_type_assertion_with_lhs_type {
                    // Type assertion calls with LHS type annotation: use fluid layout
                    // with non-wrapping type so the LHS type stays together.
                    if let Expression::Identifier(ident) = &declarator.id {
                        parts.pop();
                        parts.push(self.build_typed_identifier_doc(
                            ident,
                            declarator.definite,
                            false, // non-wrapping
                        ));
                    }
                    parts.push(d.text(" ="));
                    parts.push(d.group_with_id(d.indent(d.line()), GroupId::Assignment));
                    parts.push(d.line_suffix_boundary());
                    parts.push(d.indent_if_break(
                        wrap_init_doc(d, self.build_expression_doc(init), init),
                        GroupId::Assignment,
                        false,
                    ));
                } else if is_single_call_member_chain || needs_fluid_for_breakable_lhs {
                    // TRUE fluid layout: [" =", group(indent(line)), lineSuffixBoundary, indentIfBreak(init)]
                    //
                    // The init is NOT inside the line group. This means the printer tries to
                    // break at `=` BEFORE evaluating init's internal groups. This gives
                    // Prettier-style behavior where long chains break at `=` instead of
                    // expanding call arguments.
                    //
                    // Used for:
                    // 1. Single-call member chains with complex args (arrows, objects, arrays)
                    //    E.g., `const x = a.b.c.filter((x) => ...)` breaks at `=` if > print_width
                    // 2. Breakable LHS (destructuring patterns) with non-self-expanding RHS
                    //    E.g., `const {a, b, c} = resolve(x, y, z)` breaks after `=`, not inside `{}`
                    parts.push(d.text(" ="));
                    parts.push(d.group_with_id(d.indent(d.line()), GroupId::Assignment));
                    parts.push(d.line_suffix_boundary());
                    parts.push(d.indent_if_break(
                        wrap_init_doc(d, self.build_expression_doc(init), init),
                        GroupId::Assignment,
                        false,
                    ));
                } else if needs_break_after_operator {
                    // Break-after-operator layout for binary/conditional expressions:
                    // Structure: [" =", group(indent([line, init]))]
                    //
                    // The init IS inside the group with the line. This allows the binary/conditional
                    // expression to control its own breaking at operators. The entire RHS is
                    // indented together after the `=` break.
                    parts.push(d.text(" ="));
                    parts.push(d.group(d.indent(d.concat(&[
                        d.line(),
                        wrap_init_doc(d, self.build_expression_doc(init), init),
                    ]))));
                } else {
                    // Default layout - no line element
                    // The outer group (added at the end) handles whether the whole declaration breaks
                    // Individual expressions (calls, arrays, objects) handle their own internal breaking
                    parts.push(d.text(" = "));
                    parts.push(wrap_init_doc(d, self.build_expression_doc(init), init));
                }
            }
        }

        // For non-break multi-declarator, add rest_parts wrapped in indent
        if !should_break && !rest_parts.is_empty() {
            parts.push(d.indent(d.concat(&rest_parts)));
        }

        parts.push(d.text(";"));

        // Restore context flags
        self.declaration_indent_depth.set(old_indent_depth);
        self.in_top_level_assignment.set(false);

        if should_break {
            d.concat(&parts)
        } else if is_multi_declarator {
            // Use group for soft line breaks without initializers
            d.group(d.concat(&parts))
        } else if has_any_init {
            // Single declarator with init: use group for width-based breaking
            d.group(d.concat(&parts))
        } else {
            d.concat(&parts)
        }
    }
}
