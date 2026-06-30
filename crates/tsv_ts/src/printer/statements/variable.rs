// Variable declaration printing for TypeScript

use super::Printer;
use crate::ast::internal::{self, Expression};
use crate::printer::layout::{fluid_after_operator, hang_after_operator};
use crate::printer::{
    CommentFilter, CommentSpacing, CommentVec, ParenContext, analysis, class_expr_has_decorators,
    conditional_should_break_after_op, is_call_on_member_chain, is_curried_arrow_chain,
    is_curried_arrow_with_return_type, is_literal_member_chain, is_module_path_fluid_call,
    is_multiline_string_literal, is_poorly_breakable_chain, is_pure_property_chain,
    is_regex_root_chain, is_self_expanding_value, is_simple_self_expanding, is_simple_value,
    is_single_call_on_member_chain, is_string_literal, is_type_assertion_call, needs_parens,
    should_inline_logical_expression,
};
use smallvec::smallvec;
use tsv_lang::SymbolToU32;
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::doc::{DocBuf, GroupId};
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{INDENT, PRINT_WIDTH};

/// Build the fluid assignment layout: break after `=` only when the full line
/// exceeds print_width. Uses indentIfBreak so the RHS is evaluated independently.
/// Matches Prettier's assignment.js lines 59-67.
///
/// Wrapped in its own group so the marker's fits() evaluation doesn't see
/// trailing elements like ";" that would cause incorrect breaking.
fn build_fluid_assignment_doc(d: &DocArena, id_doc: DocId, init_doc: DocId) -> DocId {
    d.group(d.concat(&[
        id_doc,
        d.text(" ="),
        fluid_after_operator(d, init_doc, GroupId::Assignment),
    ]))
}

impl<'a> Printer<'a> {
    /// Build a variable initializer value, wrapping it in parens for the value
    /// position when needed (`const x = (a = b)`) — but NOT double-wrapping when
    /// `build_expression_doc_with_paren_comments` already added its own parens around
    /// a multiline trailing comment (`const y = (a = b // c)` stays single, not
    /// `((a = b // c))`). The single paren then matches the assignment-RHS rendering.
    ///
    /// The paren decision carries the for-header rule via `self.in_for_init`: a
    /// statement-level `const x = b in c` lexically under a for-header init (e.g. in a
    /// nested function body) parenthesizes the `in` like every other position there.
    /// (A for-header's *own* declarator is built in `build_for_init_doc`, not here.)
    fn build_init_value_doc(&self, init: &Expression<'_>, boundary_end: u32) -> DocId {
        let inner = self.build_expression_doc_with_paren_comments(init, boundary_end);
        if needs_parens(init, ParenContext::VariableInit, self.in_for_init.get())
            && !self.init_keeps_own_parens(init, boundary_end)
        {
            self.d().parens(inner)
        } else {
            inner
        }
    }

    /// True when `build_expression_doc_with_paren_comments` wraps `init` in its own
    /// parens (the keep-paren-comments path: a non-sequence with a multiline trailing
    /// paren comment), so the value-position wrap must not add a second pair. A
    /// sequence self-parenthesizes via `build_sequence_doc_value` and already reports
    /// `needs_parens == false`, so it never double-wraps and is excluded here.
    fn init_keeps_own_parens(&self, init: &Expression<'_>, boundary_end: u32) -> bool {
        if matches!(init, Expression::SequenceExpression(_)) {
            return false;
        }
        let expr_end = init.span().end;
        self.has_trailing_paren_comments(expr_end, boundary_end)
            && comments_in_range(self.comments, expr_end, boundary_end)
                .any(|c| !c.is_block || self.has_newline_between(expr_end, c.span.start))
    }

    /// Build a doc for a variable binding pattern with optional definite assignment assertion.
    ///
    /// For identifiers with `definite: true`, builds doc for `name!: type` instead of `name: type`.
    /// Uses wrapping type annotations so TypeReference type arguments break internally when needed.
    fn build_variable_binding_doc(&self, id: &Expression<'_>, definite: bool) -> DocId {
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
        ident: &internal::Identifier<'_>,
        definite: bool,
        wrap_type: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = smallvec![d.symbol(ident.name.to_u32())];

        // Compute name_end for comment extraction
        let search_end = ident
            .type_annotation()
            .map_or(ident.span.end, |ta| ta.span.start);
        let raw_name_end = analysis::skip_identifier_at(
            self.source.as_bytes(),
            ident.span.start as usize,
            search_end as usize,
        ) as u32;
        let mut after_modifier = raw_name_end;

        if definite {
            after_modifier = self.push_modifier_marker_doc(&mut parts, after_modifier, b'!');
        }
        if ident.optional {
            after_modifier = self.push_modifier_marker_doc(&mut parts, after_modifier, b'?');
        }
        if let Some(type_ann) = ident.type_annotation() {
            // `: type` annotation, handling a before-`:` comment between the binding
            // name (and any `!`/`?`) and `:` — line → indented continuation, block →
            // inline before `:`.
            parts.push(self.build_binding_type_annotation_doc(after_modifier, type_ann, wrap_type));
        }
        d.concat(&parts)
    }

    /// Build a Doc for a variable declaration statement
    ///
    /// Handles declare, definite assignment (!), type annotations, and multiple declarators.
    /// Follows prettier's rule: if any declarator has an initializer, break to multiple lines.
    /// `emit_semicolon` is `false` only for embedders that supply their own
    /// terminator — Svelte's `{const …}`/`{let …}` tags close with `}` and drop
    /// the `;` (a bare `{let a}` is the lone exception, which passes `true`).
    pub(crate) fn build_variable_declaration_doc(
        &self,
        decl: &internal::VariableDeclaration<'_>,
        emit_semicolon: bool,
    ) -> DocId {
        let d = self.d();
        let mut prefix: DocBuf = DocBuf::new();

        // Declare modifier
        if decl.declare {
            prefix.push(d.text("declare "));
        }

        // Keyword (const, let, var)
        prefix.push(d.text(decl.kind.as_str()));

        // The keyword→first-declarator gap. A *line* comment here indents the whole
        // continuation one level (uniform declaration-header rule); block/no-comment
        // cases stay inline. The leading space is supplied by the gap helper below.
        let keyword_end = if decl.declare {
            decl.span.start + "declare ".len() as u32 + decl.kind.as_str().len() as u32
        } else {
            decl.span.start + decl.kind.as_str().len() as u32
        };
        let first_decl_start = decl.declarations[0].span.start;

        // Everything after the gap is collected into `parts` (the continuation).
        let mut parts = DocBuf::new();

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
        let mut rest_parts = DocBuf::new();

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
                // The declarator-separating comma. A block comment keeps the author's
                // side of it: before → trails the previous init; after → leads the next
                // declarator. (Only consulted when `has_block_comment`.)
                let comma_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    prev_end as usize,
                    curr_start as usize,
                    b',',
                )
                .map_or(curr_start, |p| p as u32);

                if should_break {
                    if has_line_comment {
                        // Line comment(s) between declarators: comma must go before
                        // the first line comment, block comments go before the comma.
                        // e.g. `a = 1 /* c1 */,\n// c2\nb = 2` or `a = 1, // c1\n// c2\nb = 2`
                        let comments: CommentVec<'_> =
                            comments_in_range(self.comments, prev_end, curr_start).collect();
                        let first_line_idx = comments.iter().position(|c| !c.is_block).unwrap_or(0);

                        // Block comments before the first line comment
                        for comment in &comments[..first_line_idx] {
                            parts.push(d.text(" "));
                            parts.push(self.build_comment_doc(comment));
                        }

                        // Comma before the first line comment
                        parts.push(d.text(","));

                        // Remaining comments (starting with the first line comment)
                        // `needs_hardline` starts true when block comments precede
                        // (comma sits between block and line, needs newline after)
                        let mut needs_hardline = first_line_idx > 0;
                        for comment in &comments[first_line_idx..] {
                            if needs_hardline {
                                parts.push(d.hardline());
                                parts.push(d.text(INDENT));
                                parts.push(self.build_comment_doc(comment));
                            } else {
                                // Same-line comment trailing the comma: a line comment
                                // goes through `line_suffix` (zero width) so it never
                                // forces the preceding declarator's value to break
                                // (prettier's `lineSuffix`); a block stays inline.
                                parts.push(self.build_trailing_comment_doc(comment));
                            }
                            needs_hardline = !comment.is_block;
                        }
                    } else {
                        // Block comment(s) before the comma trail the previous init
                        // (`a = 1 /* c */,`); after-comma comments lead the next
                        // declarator (below the break). Prettier preserves the side.
                        if has_block_comment {
                            for comment in comments_in_range(self.comments, prev_end, comma_pos) {
                                parts.push(d.text(" "));
                                parts.push(self.build_comment_doc(comment));
                            }
                        }
                        parts.push(d.text(","));
                    }
                    // Break to new line with indentation for next declarator
                    parts.push(d.hardline());
                    parts.push(d.text(INDENT));
                    if has_block_comment && !has_line_comment {
                        // After-comma block comment(s) leading the next declarator: a
                        // block inline-adjacent to the declarator hugs it (`/* c */ b`),
                        // an own-line one keeps its line (with any author blank line
                        // preserved before the declarator/next comment).
                        let comments: CommentVec<'_> =
                            comments_in_range(self.comments, comma_pos, curr_start).collect();
                        for (ci, comment) in comments.iter().enumerate() {
                            parts.push(self.build_comment_doc(comment));
                            if comment.is_block && self.is_same_line(comment.span.end, curr_start) {
                                parts.push(d.text(" "));
                            } else {
                                let next =
                                    comments.get(ci + 1).map_or(curr_start, |c| c.span.start);
                                self.push_blank_preserving_hardline(
                                    &mut parts,
                                    comment.span.end,
                                    next,
                                );
                                parts.push(d.text(INDENT));
                            }
                        }
                    }
                } else {
                    // Non-break case: block comments keep their side of the comma
                    // (preserve position). The first continuation's comma sits in
                    // `parts` (after declarator 0), later commas in `rest_parts`;
                    // after-comma comments lead the next declarator.
                    let comma_target = if i == 1 { &mut parts } else { &mut rest_parts };
                    if has_block_comment {
                        for comment in comments_in_range(self.comments, prev_end, comma_pos) {
                            comma_target.push(d.text(" "));
                            comma_target.push(self.build_comment_doc(comment));
                        }
                    }
                    comma_target.push(d.text(","));
                    // Soft break for declarations without initializers
                    rest_parts.push(d.line());
                    if has_block_comment {
                        for comment in comments_in_range(self.comments, comma_pos, curr_start) {
                            rest_parts.push(self.build_comment_doc(comment));
                            rest_parts.push(d.text(" "));
                        }
                    }
                }
            }

            // Build id doc once for reuse and analysis
            let id_doc = self.build_variable_binding_doc(&declarator.id, declarator.definite);

            // Check if id doc can break (contains line elements like type annotations that wrap)
            // This matches Prettier's `canBreak(leftDoc)` check
            let can_break_left = d.can_break(id_doc);

            if !should_break && i > 0 {
                // Non-break continuation declarators go to rest_parts (never have inits)
                rest_parts.push(id_doc);
            }

            // Initializer with comment handling around =
            if let Some(init) = &declarator.init {
                let mut id_end = declarator.id.span().end;
                let init_start = init.span().start;
                // With definite assignment but no type annotation (`let a! = x`), the id
                // span excludes the `!`; advance past it so comments between the name and
                // `!` (already emitted inside the id doc) aren't re-emitted before `=`.
                if declarator.definite
                    && let Expression::Identifier(ident) = &declarator.id
                    && ident.type_annotation().is_none()
                    && let Some(bang_pos) = find_char_skipping_comments(
                        self.source.as_bytes(),
                        id_end as usize,
                        init_start as usize,
                        b'!',
                    )
                {
                    id_end = bang_pos as u32 + 1;
                }
                let equals_pos = self.find_equals_position(id_end, init_start);
                let has_comments_before_eq = self.has_comments_between(id_end, equals_pos);
                let has_comments_after_eq = self.has_comments_between(equals_pos + 1, init_start);

                // A line comment between the binding and `=` keeps the comment in place
                // and drops `= value` to a continuation line indented one level (preserve
                // — lossless when a second comment also trails the statement; prettier
                // relocates it to end-of-statement and merges the two onto one line —
                // conformance_prettier.md §Comment relocation). Bypasses the
                // assignment-layout selection below; value built lazily so the common
                // no-comment path is unaffected. Init declarators always feed `parts`
                // (the comma/separator is handled above, the `;` after the loop), so a
                // plain push + `continue` is safe.
                if let Some(cont) =
                    self.build_initializer_line_continuation(id_end, equals_pos, || {
                        let value_doc = self
                            .build_expression_doc_with_paren_comments(init, declarator.span.end);
                        self.prepend_rhs_comments(value_doc, equals_pos + 1, init_start)
                    })
                {
                    parts.push(id_doc);
                    parts.push(cont);
                    continue;
                }

                // Comments after `=` all stay after `=`, matching prettier — a JSDoc
                // cast (`= /** @type {T} */ (expr)`) keeps its parens via the
                // `JsdocCast` node, so its comment lives inside the init expression and
                // never reaches this gap.
                let rhs_comments_start = equals_pos + 1;

                // Helpers for LHS doc handling. Most branches use id_doc as-is;
                // some rebuild it (break-lhs wrapping type, fluid non-wrapping type).
                // Comments before `=` are always appended after the LHS. (Only block
                // comments reach here — a before-`=` *line* comment took the
                // continuation `continue` path above.)
                let push_lhs = |parts: &mut DocBuf, lhs_doc: DocId| {
                    parts.push(lhs_doc);
                    if has_comments_before_eq {
                        parts.push(self.build_inline_comments_between_doc(id_end, equals_pos));
                    }
                };
                // For fluid layout, LHS + comments must be a single doc (inside the fluid group)
                let make_fluid_lhs = |lhs_doc: DocId| -> DocId {
                    if has_comments_before_eq {
                        let mut lhs_parts: DocBuf = smallvec![lhs_doc];
                        lhs_parts.push(self.build_inline_comments_between_doc(id_end, equals_pos));
                        d.concat(&lhs_parts)
                    } else {
                        lhs_doc
                    }
                };

                // Build optional inline block comment doc between `=` and init.
                // These are comments like `const x = /* comment */ expr` that should be
                // part of the RHS doc in assignment layout decisions. Line comments are
                // handled separately (mandatory break path).
                let rhs_block_comment_doc = if has_comments_after_eq {
                    self.build_comments_between_filtered_opt(
                        rhs_comments_start,
                        init_start,
                        CommentSpacing::Trailing,
                        CommentFilter::BlockOnly,
                    )
                } else {
                    None
                };

                // Helper: build init doc with optional inline block comments prepended.
                // Comments use Trailing spacing (`/* comment */ `) so no extra space needed.
                let make_init_doc = |init_doc: DocId| -> DocId {
                    if let Some(comment_doc) = rhs_block_comment_doc {
                        d.concat(&[comment_doc, init_doc])
                    } else {
                        init_doc
                    }
                };

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

                // Call chains AND member-only chains with line comments should NOT be
                // treated as fluid / break-after-operator. The chain formatter breaks
                // internally at the comment location, so keep the chain with `=`
                // (otherwise it breaks after `=` too → double indent). E.g.
                // `const a = items // comment\n  .foo()` and `const b = foo.bar // c\n  .baz`.
                let has_line_comments_in_chain = (matches!(init, Expression::CallExpression(_))
                    && self.has_line_comments_in_call_chain(init))
                    || self.has_line_comments_in_member_chain(init);

                // Combined flag for expressions with trailing comments that expand internally
                let has_trailing_comment_expansion =
                    is_call_with_trailing_comments || is_import_with_trailing_comments;

                // Common exclusion: layout strategies don't apply when the init
                // self-expands (object/array), has trailing comment expansion, or
                // has line comments in a chain — those need special handling.
                let is_layout_eligible = !is_self_expanding_value(init)
                    && !has_trailing_comment_expansion
                    && !has_line_comments_in_chain;

                // RHS expressions that should use break-after-operator layout.
                // Matches Prettier's shouldBreakAfterOperator: poorly breakable chains,
                // string literals, etc. These don't break well internally, so the
                // assignment breaks at `=` with group(indent([line, rightDoc])).
                let should_break_after_op_rhs = (is_module_path_fluid_call(init, &interner)
                    || is_pure_property_chain(init)
                    || is_poorly_breakable_chain(init, self.source, PRINT_WIDTH, self.comments)
                    || is_string_literal(init)
                    || matches!(init, Expression::RegexLiteral(_)))
                    && is_layout_eligible;

                // Decorated class expression → break after operator, each decorator
                // on its own line (`const C =\n\t@dec\n\tclass {}`).
                let is_decorated_class_expr = is_layout_eligible
                    && matches!(init, Expression::ClassExpression(c) if class_expr_has_decorators(c));

                // Single-call member chains with complex args (arrows, objects, arrays):
                // Use TRUE fluid layout to break at `=` only when necessary.
                // E.g., `const x = a.b.c.filter((x) => ...)` breaks at `=` if > print_width
                let is_single_call_member_chain =
                    is_call_on_member_chain(init) && is_layout_eligible;

                // Regex-rooted member chain calls: /regex/.exec(b)
                // Prettier returns "fluid" layout (its default) because regex roots are NOT
                // accepted by isPoorlyBreakableMemberOrCallChain (only Identifier/ThisExpression).
                // Our is_poorly_breakable_chain similarly rejects regex roots. Route to fluid
                // so fits() can decide whether to break at `=` or let the call expand args.
                let is_regex_chain_call = is_regex_root_chain(init) && is_layout_eligible;

                // Member-only chains on literal bases: 'string'.length, `template`.length
                // These need Fluid layout so the assignment can break at `=` when the
                // literal base exceeds print_width on the assignment line.
                let is_literal_member = is_literal_member_chain(init) && is_layout_eligible;

                // Expressions that need break-after-operator layout:
                // group([left, " =", indent([line, right])])
                // For binary/logical expressions, breaking happens at operators within the RHS,
                // and the entire RHS is indented together after `=`.
                //
                // Excludes logical expressions with inline-able RHS (non-empty object/array).
                // Those use default layout so the RHS self-expands:
                //   `const x = foo || { a: 1 }` not `const x =\n  foo || {a: 1}`
                // Prettier ref: assignment.js:199, binaryish.js:361
                let is_non_inline_binary = if let Expression::BinaryExpression(binary) = init {
                    !should_inline_logical_expression(binary)
                } else {
                    false
                };
                let needs_break_after_op_layout = (is_non_inline_binary
                    || conditional_should_break_after_op(init))
                    && is_layout_eligible;

                // Member-chain call (a.fn(...)) where the call head fits within print_width:
                // Use default layout and let the call expand its own args rather than breaking
                // at `=`. E.g., `const {a, b} = vi.mocked(longArg)` with short LHS keeps
                // `= vi.mocked(` on line 1 and expands the arg — matching Prettier's behavior.
                // Only fires when call head (decl_start to callee_end + "(") fits in print_width.
                // is_single_call_on_member_chain guarantees CallExpression
                let is_expandable_member_call = if let Expression::CallExpression(call) = init
                    && is_single_call_on_member_chain(init)
                {
                    // Include actual source indentation (JS nesting) in the width check.
                    // Without this, deeply-nested declarations would incorrectly use
                    // default layout even when the call head exceeds print_width.
                    let indent_visual = self.source_indent_visual(decl.span.start);
                    let call_head_width = indent_visual
                        + (call.callee.span().end as usize - decl.span.start as usize)
                        + 1; // +1 for "(" after callee
                    call_head_width < PRINT_WIDTH
                } else {
                    false
                };

                let is_break_after_op_rhs = should_break_after_op_rhs
                    || needs_break_after_op_layout
                    || is_decorated_class_expr;

                // Breakable LHS (destructuring patterns) with non-self-expanding RHS:
                // Use fluid layout so the printer breaks at `=` before expanding the
                // destructuring pattern. Matches Prettier's `canBreak(leftDoc) → "fluid"`.
                // E.g., `const {a, b, c} = resolve(x, y, z)` breaks after `=`, not inside `{}`
                //
                // Excludes break-after-operator RHS (binary, conditional, strings, chains) —
                // those go through needs_break_after_operator with their own layout.
                // In Prettier, shouldBreakAfterOperator() handles those before the canBreak fallback.
                //
                // Excludes is_expandable_member_call: when the call head fits, the call's own
                // arg-expansion handles line breaking via default layout.
                let needs_fluid_for_breakable_lhs = can_break_left
                    && is_layout_eligible
                    && !should_break
                    && !is_break_after_op_rhs
                    && !is_expandable_member_call;

                // Type assertion calls with LHS type annotation need special fluid handling
                // (handled separately below because they need non-wrapping LHS type)
                let is_type_assertion_with_lhs_type = is_type_assertion_call(
                    init,
                    self.source,
                    PRINT_WIDTH,
                ) && matches!(&declarator.id, Expression::Identifier(id) if id.type_annotation().is_some());

                let is_simple_rhs_with_breakable_lhs =
                    can_break_left && is_simple_self_expanding(init);

                let needs_break_after_operator = !should_break
                    && (is_break_after_op_rhs || is_simple_rhs_with_breakable_lhs)
                    && !d.will_break(id_doc)
                    && !has_complex_type_annotation
                    && !has_complex_destructuring
                    && !is_arrow_with_breakable_left;
                drop(interner);

                // Check for line comments after = which force a break
                let has_line_comments_after_eq =
                    self.has_line_comments_between(equals_pos + 1, init_start);

                // Check for a comment after `=` that forces break-after-operator.
                // Prettier ref: hasLeadingOwnLineComment → break-after-operator in
                // chooseLayout. A comment forces the break when it's multiline (its own
                // newlines break the group) or the source put the value on a later line
                // than the comment (an own-line leading comment); a single-line block
                // glued to the value (`= /* c */ v`) stays inline.
                let has_own_line_comment_after_eq = has_comments_after_eq
                    && comments_in_range(self.comments, rhs_comments_start, init_start)
                        .any(|c| c.multiline || !self.is_same_line(c.span.end, init_start));

                // Curried arrows with return type always break after `=`
                let is_curried_arrow = is_curried_arrow_with_return_type(init);

                if is_multiline_string || has_line_comments_after_eq {
                    // Multiline strings or line comments: mandatory break after `=`
                    push_lhs(&mut parts, id_doc);
                    parts.push(d.text(" ="));
                    if has_comments_after_eq {
                        // Single pass: partition comments into same-line (inline) and
                        // different-line (leading) relative to the `=` sign.
                        let mut leading_comments = DocBuf::new();
                        let after_eq: CommentVec<'_> =
                            comments_in_range(self.comments, equals_pos + 1, init_start).collect();
                        for (ci, comment) in after_eq.iter().enumerate() {
                            if self.is_same_line(equals_pos, comment.span.start) {
                                // Inline comment on same line as =
                                parts.push(d.text(" "));
                                parts.push(self.build_comment_doc(comment));
                            } else {
                                leading_comments.push(self.build_comment_doc(comment));
                                // Preserve an author blank line before the next comment
                                // or the value, matching prettier.
                                let next =
                                    after_eq.get(ci + 1).map_or(init_start, |c| c.span.start);
                                self.push_blank_preserving_hardline(
                                    &mut leading_comments,
                                    comment.span.end,
                                    next,
                                );
                            }
                        }
                        parts.push(d.indent(d.concat(&[
                            d.hardline(),
                            d.concat(&leading_comments),
                            self.build_init_value_doc(init, declarator.span.end),
                        ])));
                    } else {
                        parts.push(d.indent(d.concat(&[
                            d.hardline(),
                            self.build_init_value_doc(init, declarator.span.end),
                        ])));
                    }
                } else if has_own_line_comment_after_eq {
                    // A multiline or own-line comment after `=` forces break-after-operator
                    // layout. Prettier ref: hasLeadingOwnLineComment → break-after-operator
                    push_lhs(&mut parts, id_doc);
                    parts.push(d.text(" ="));
                    let comments_doc = self
                        .build_rhs_comments_opt(rhs_comments_start, init_start)
                        .unwrap_or_else(|| d.empty());
                    let init_doc = self.build_init_value_doc(init, declarator.span.end);
                    let rhs_doc = d.concat(&[comments_doc, init_doc]);
                    parts.push(hang_after_operator(d, rhs_doc));
                } else if is_curried_arrow {
                    // Curried arrow with return type: mandatory break after `=`
                    // The arrow expression formatter handles the rest of the breaking
                    push_lhs(&mut parts, id_doc);
                    parts.push(d.text(" ="));
                    parts.push(d.indent(d.concat(&[
                        d.hardline(),
                        self.build_init_value_doc(init, declarator.span.end),
                    ])));
                } else if is_curried_arrow_chain(init) {
                    // Untyped curried arrow chain: fluid break after `=`. The chain's
                    // signature heads break only when they don't fit on the operator
                    // line; a hugging body otherwise expands in place. The context tells
                    // the arrow printer to use the assignment-RHS chain layout.
                    let init_doc = self.build_with_arrow_chain_context(
                        crate::printer::ArrowChainContext::AssignmentRhs,
                        || make_init_doc(self.build_init_value_doc(init, declarator.span.end)),
                    );
                    parts.push(build_fluid_assignment_doc(
                        d,
                        make_fluid_lhs(id_doc),
                        init_doc,
                    ));
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
                        push_lhs(
                            &mut parts,
                            self.build_typed_identifier_doc(
                                ident,
                                declarator.definite,
                                true, // wrap_type
                            ),
                        );
                    } else if has_complex_destructuring {
                        // Strip the outer group from the destructuring id_doc so it
                        // participates in the outer group's fit check. Without this,
                        // the destructuring group evaluates independently via fits()
                        // and stays flat even when the full line exceeds print_width.
                        // Prettier's break-lhs does not wrap leftDoc in an extra group.
                        push_lhs(&mut parts, d.unwrap_group(id_doc));
                    } else {
                        push_lhs(&mut parts, id_doc);
                    }

                    // Add ` = rightDoc` (right side grouped)
                    parts.push(d.text(" = "));
                    let init_doc =
                        make_init_doc(self.build_init_value_doc(init, declarator.span.end));
                    parts.push(d.group(init_doc));
                } else if is_type_assertion_with_lhs_type
                    || is_single_call_member_chain
                    || needs_fluid_for_breakable_lhs
                    || is_regex_chain_call
                    || is_literal_member
                {
                    // Fluid layout for specific RHS patterns: break after `=` only
                    // when the full line exceeds print_width. Type assertion case
                    // rebuilds LHS with non-wrapping type annotation.
                    let fluid_id_doc = if is_type_assertion_with_lhs_type
                        && let Expression::Identifier(ident) = &declarator.id
                    {
                        self.build_typed_identifier_doc(
                            ident,
                            declarator.definite,
                            false, // non-wrapping
                        )
                    } else {
                        id_doc
                    };
                    let init_doc =
                        make_init_doc(self.build_init_value_doc(init, declarator.span.end));
                    parts.push(build_fluid_assignment_doc(
                        d,
                        make_fluid_lhs(fluid_id_doc),
                        init_doc,
                    ));
                } else if needs_break_after_operator {
                    // Break-after-operator layout for binary/conditional expressions:
                    // Structure: [" =", group(indent([line, init]))]
                    //
                    // The init IS inside the group with the line. This allows the binary/conditional
                    // expression to control its own breaking at operators. The entire RHS is
                    // indented together after the `=` break.
                    push_lhs(&mut parts, id_doc);
                    parts.push(d.text(" ="));
                    let init_doc =
                        make_init_doc(self.build_init_value_doc(init, declarator.span.end));
                    parts.push(hang_after_operator(d, init_doc));
                } else if is_layout_eligible && !is_simple_value(init) {
                    // Fluid layout (default for layout-eligible values)
                    //
                    // Matches prettier's chooseLayout default: when no special layout
                    // applies, use fluid so the marker can break at `=` only if needed,
                    // while allowing the RHS to break internally first.
                    let init_doc =
                        make_init_doc(self.build_init_value_doc(init, declarator.span.end));
                    parts.push(build_fluid_assignment_doc(
                        d,
                        make_fluid_lhs(id_doc),
                        init_doc,
                    ));
                } else {
                    push_lhs(&mut parts, id_doc);
                    parts.push(d.text(" = "));
                    let init_doc =
                        make_init_doc(self.build_init_value_doc(init, declarator.span.end));
                    parts.push(init_doc);
                }
            } else if should_break || i == 0 {
                // No initializer: push id_doc directly
                parts.push(id_doc);
            }
        }

        // For non-break multi-declarator, add rest_parts wrapped in indent
        if !should_break && !rest_parts.is_empty() {
            parts.push(d.indent(d.concat(&rest_parts)));
        }

        // Comments between the last declarator and the `;`, with the `;` bound to the
        // declaration: a same-line block trails *after* it (`const x = 1 /* c */;` →
        // `const x = 1; /* c */`, prettier 3.9), a same-line line trails after it via
        // `line_suffix` (`const x = 1; // c`), an own-line comment drops to its own line
        // after it (`const x = 1;⏎// c`). See `split_separator_gap_comments`.
        if emit_semicolon {
            let mut after = DocBuf::new();
            if let Some(last) = decl.declarations.last() {
                let semicolon_pos = decl.span.end.saturating_sub(1);
                after = self.split_separator_gap_comments(
                    &mut parts,
                    last.span.end,
                    semicolon_pos,
                    true,
                );
            }
            parts.push(d.text(";"));
            parts.extend(after);
        }

        // Restore context flags
        self.declaration_indent_depth.set(old_indent_depth);
        self.in_top_level_assignment.set(false);

        let continuation = if should_break {
            // Multi-declarator with initializers: hardline breaks already inserted
            d.concat(&parts)
        } else if is_multi_declarator || has_any_init {
            // Group for width-based breaking (multi-declarator soft breaks or single with init)
            d.group(d.concat(&parts))
        } else {
            d.concat(&parts)
        };
        // A line comment in the keyword→declarator gap indents the continuation.
        prefix.push(self.build_keyword_to_name_continuation(
            keyword_end,
            first_decl_start,
            continuation,
        ));
        d.concat(&prefix)
    }
}
