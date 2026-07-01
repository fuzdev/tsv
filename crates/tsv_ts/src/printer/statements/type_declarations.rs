// Type declaration printing (type aliases, interfaces, enums, namespaces, declare functions)
// plus shared entity-name helpers

use super::{Printer, build_entity_name_doc, should_hug_union_type};
use crate::ast::internal::{self, TSType};
use crate::printer::layout::hang_after_operator;
use crate::printer::{CommentFilter, CommentSpacing, CommentVec, HeritageKeyword};
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{Comment, Span, SymbolToU32, comments_in_range};

/// Check if a type is "generic" - i.e., has type parameters.
/// This matches prettier's `isGeneric` function in assignment.js.
fn is_generic_type(ts_type: &TSType<'_>) -> bool {
    match ts_type {
        TSType::Function(f) => f.type_parameters.is_some(),
        TSType::TypeReference(r) => r.type_arguments.is_some(),
        _ => false,
    }
}

/// Check if we should break before the conditional type in a type alias.
/// Returns true if either checkType or extendsType has type parameters.
/// This matches prettier's `shouldBreakBeforeConditionalType` in assignment.js.
fn should_break_before_conditional_type(conditional: &internal::TSConditionalType<'_>) -> bool {
    is_generic_type(conditional.check_type) || is_generic_type(conditional.extends_type)
}

/// Returns true if the type has its own internal breaking mechanism
/// (e.g., braces, brackets, parentheses) and should NOT break after `=`.
fn type_has_internal_breaking(ts_type: &TSType<'_>) -> bool {
    match ts_type {
        TSType::TypeLiteral(_)
        | TSType::Mapped(_)
        | TSType::Tuple(_)
        | TSType::Function(_)
        | TSType::Constructor(_)
        // `import(...)` hugs the `=` like a call — its specifier path doesn't
        // break, and any comments expand the parens internally.
        | TSType::Import(_) => true,
        // `typeof import(...)` hugs the `=` for the same reason — the import
        // call inside the type query provides the internal break.
        TSType::TypeQuery(q) => {
            matches!(q.expr_name, internal::TSTypeQueryExprName::Import(_))
        }
        // TypeReference with type arguments has internal breaking via `<>`
        TSType::TypeReference(r) => r.type_arguments.is_some(),
        _ => false,
    }
}

impl<'a> Printer<'a> {
    /// Check if a comment spans multiple lines (block comment with newlines)
    fn is_multiline_comment(&self, comment: &Comment) -> bool {
        comment.is_block && !self.is_same_line(comment.span.start, comment.span.end)
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
        decl: &internal::TSTypeAliasDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![];
        if decl.declare {
            parts.push(d.text("declare "));
        }
        parts.push(d.text("type"));
        // Comments between keyword and name: `type /* c */ A = string`
        parts.push(d.text(" "));
        parts.push(
            self.build_inline_comments_between_doc_trailing_space(
                decl.span.start,
                decl.id.span.start,
            ),
        );
        parts.push(d.symbol(decl.id.name.to_u32()));

        // Check if type parameters are complex (>1 param with constraints/defaults)
        // Complex type params use break-lhs layout: params break, not the RHS
        let has_complex_params = self.type_alias_has_complex_params(decl.type_parameters.as_ref());

        // Compute `=` position early so we can use it as comment boundary
        let header_end = decl
            .type_parameters
            .as_ref()
            .map_or(decl.id.span.end, |tp| tp.span.end);
        let type_start = decl.type_annotation.span().start;
        let eq_pos = self.find_equals_position(header_end, type_start);

        // Comments between name and type params: `type A/* c */ <T> = T`. The
        // name→`=` gap (no params) and type-params→`=` gap are handled below as
        // pre-`=` comments so they stay on the head side. Line comments get a
        // hardline to prevent absorbing type params as comment text.
        let comment_end = decl
            .type_parameters
            .as_ref()
            .map_or(decl.id.span.end, |tp| tp.span.start);
        parts.push(self.build_name_to_type_params_comments(
            decl.id.span.end,
            comment_end,
            CommentSpacing::for_type_params(decl.type_parameters.is_some()),
        ));

        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Comments between the head (name + type params) and `=`. A single-line
        // block comment stays inline before `=` (`type A<X> /* c */ = B`); a line
        // comment or multiline block can't share the `=` line, so it stays on its
        // own line before `=` with the value pushed down. tsv keeps these on the
        // head side; prettier relocates them after `=` (see conformance_prettier.md
        // §Comment relocation). They were previously dropped entirely when type
        // parameters were present (content loss).
        let pre_eq_forces_own_line = self.comments_force_own_line_between(header_end, eq_pos);

        if pre_eq_forces_own_line {
            let mut indent_parts: DocBuf = smallvec![d.hardline()];
            for comment in comments_in_range(self.comments, header_end, eq_pos) {
                indent_parts.push(self.build_comment_doc(comment));
                indent_parts.push(d.hardline());
            }
            indent_parts.push(self.build_type_alias_eq_value_doc(
                decl,
                eq_pos,
                type_start,
                has_complex_params,
                false,
            ));
            parts.push(d.indent(d.concat(&indent_parts)));
        } else {
            // Single-line block comments before `=` stay inline: `<head> /* c */ =`
            if let Some(block_doc) = self.build_comments_between_filtered_opt(
                header_end,
                eq_pos,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            ) {
                parts.push(block_doc);
            }
            parts.push(self.build_type_alias_eq_value_doc(
                decl,
                eq_pos,
                type_start,
                has_complex_params,
                true,
            ));
        }

        // Comments between the value and `;`: block comments stay before `;`
        // (`type A = B /* c */;`), matching prettier; line comments move after `;`
        // (`type A = B; // c`) since a line comment can't precede `;` on the same
        // line. These were previously dropped entirely (content loss).
        let value_end = decl.type_annotation.span().end;
        let mut trailing_line_parts = DocBuf::new();
        for comment in comments_in_range(self.comments, value_end, decl.span.end) {
            if comment.is_block {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                trailing_line_parts.push(d.text(" "));
                trailing_line_parts.push(self.build_comment_doc(comment));
            }
        }

        parts.push(d.text(";"));
        parts.extend(trailing_line_parts);

        d.concat(&parts)
    }

    /// Build the `=` token and the type-alias value, including any comments
    /// between `=` and the value. `lead_space` controls the leading space before
    /// `=` (true for the inline `... =` form, false when the caller has already
    /// emitted a hardline, e.g. after an own-line pre-`=` comment).
    /// A prefix type operator (`keyof` / `typeof`) whose keyword→operand gap holds
    /// a comment that forces the operand onto its own line — a **line** comment or a
    /// **multiline** block comment. Such a value hangs the operand, so the type-alias
    /// RHS keeps the operator on the `=` line (the operand hangs via the operator's
    /// own layout) instead of breaking after `=` — consistent with the conditional /
    /// internal-breaking arms. A **single-line** block comment (own-line, trailing, or
    /// glued) collapses inline, so this returns false and the comment-free `=` layout
    /// applies; a long *comment-free* operator still breaks after `=` (the
    /// hanging-indent arm). Mirrors the printer-side gate
    /// (`comments_force_own_line_between`) so the two stay in lockstep.
    fn type_operator_comment_forces_operand_own_line(&self, ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TypeOperator(o) => {
                let kw_end = o.span.start + o.operator.as_str().len() as u32;
                self.comments_force_own_line_between(kw_end, o.type_annotation.span().start)
            }
            TSType::TypeQuery(q) => {
                let kw_end = q.span.start + "typeof".len() as u32;
                self.comments_force_own_line_between(kw_end, q.expr_name.span().start)
            }
            _ => false,
        }
    }

    fn build_type_alias_eq_value_doc(
        &self,
        decl: &internal::TSTypeAliasDeclaration<'_>,
        eq_pos: u32,
        type_start: u32,
        has_complex_params: bool,
        lead_space: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![d.text(if lead_space { " =" } else { "=" })];

        // A leading comment between `=` and the RHS forces the value onto its own
        // line when it can't share the `=` line: a line comment or multiline block
        // (`comments_force_own_line_between`), OR a single-line block comment that
        // was *authored* on its own line (`type X =⏎/* c */⏎Y`). Prettier breaks
        // after `=` and keeps such a comment on its own line rather than hugging it
        // up to `=` — the union/intersection RHS then renders below it.
        let force_break = self.comments_force_own_line_between(eq_pos + 1, type_start)
            || comments_in_range(self.comments, eq_pos + 1, type_start)
                .any(|c| c.is_block && self.is_own_line_comment(c));

        if force_break {
            // Line/multiline block comments force type to next line with indent.
            // Line comments stay on `=` line; multiline blocks go into the indent.
            // Example: `type A = // comment\n  B;`
            // Example: `type J =\n  /* comment\n   */\n  K | L;`
            let mut inline_parts = DocBuf::new();
            let mut indent_comment_parts = DocBuf::new();

            // Only the first single-line comment hugs the `=` line, and only when
            // it was *authored* on that line (`type A = /* c */ B`). An own-line
            // comment (`type A =⏎/* c */⏎B`) keeps its own line — prettier breaks
            // after `=` and never pulls it up. Multiline blocks (any position) and
            // every subsequent comment go on their own line in the indent. Two line
            // comments must not merge onto one line — the second `//` would stop
            // being a delimiter (a boundary loss).
            let comments: CommentVec<'_> =
                comments_in_range(self.comments, eq_pos + 1, type_start).collect();
            for (idx, comment) in comments.iter().enumerate() {
                let multiline_block = comment.is_block && self.is_multiline_comment(comment);
                let authored_on_eq_line = self.is_same_line(eq_pos, comment.span.start);
                if idx == 0 && !multiline_block && authored_on_eq_line {
                    inline_parts.push(d.text(" "));
                    inline_parts.push(self.build_comment_doc(comment));
                } else {
                    indent_comment_parts.push(self.build_comment_doc(comment));
                    // Preserve an author blank line before the next comment, or before
                    // the value itself (`type X =⏎/* c */⏎⏎Y`), matching prettier.
                    let next = comments.get(idx + 1).map_or(type_start, |c| c.span.start);
                    self.push_blank_preserving_hardline(
                        &mut indent_comment_parts,
                        comment.span.end,
                        next,
                    );
                }
            }

            parts.extend(inline_parts);

            // Type uses its own group (via build_type_doc) so unions/intersections
            // can independently decide whether to break
            let type_doc = self.build_type_doc(&decl.type_annotation);
            let mut indent_content: DocBuf = smallvec![d.hardline()];
            indent_content.extend(indent_comment_parts);
            indent_content.push(type_doc);
            parts.push(d.indent(d.concat(&indent_content)));
        } else {
            // Single-line block comments (or no comments): inline after `=`
            if let Some(comment_doc) = self.build_comments_between_filtered_opt(
                eq_pos + 1,
                type_start,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            ) {
                parts.push(comment_doc); // " /* comment */"
            }

            // Check the type kind for different formatting rules. Redundant
            // comment-free parens around the RHS are stripped (prettier does the
            // same), so a `(union)` / `(intersection)` gets the same break layout
            // as the bare form instead of hanging inline. The doc is built from the
            // unwrapped type — safe, since we only unwrap when no comments are inside
            // the parens (commented parens stay on the preserve-in-place path).
            let value_type = self.unwrap_redundant_parens(&decl.type_annotation);
            // For union/intersection types, build without their own group so they inherit
            // breaking from this context's group.
            if let TSType::Union(u) = value_type {
                let type_doc = self.build_union_type_doc(u);
                if should_hug_union_type(u) {
                    // Hugged unions (e.g., `{ ... } | null`): the object type handles its own
                    // expansion, so keep `= {` together like other internally-breaking types
                    parts.push(d.text(" "));
                    parts.push(type_doc);
                } else {
                    // Normal unions: break after `=` with leading `| `
                    parts.push(hang_after_operator(d, type_doc));
                }
            } else if let TSType::Intersection(i) = value_type {
                // Intersection types: first element stays inline, continuation types
                // wrap with a hanging indent (skipped when a boundary TypeLiteral/Mapped
                // owns its own expansion — see `intersection_hanging_with_indent`).
                parts.push(d.text(" "));
                parts.push(self.intersection_hanging_with_indent(i));
            } else if let TSType::Conditional(cond) = value_type {
                // Conditional types: break after `=` only if check/extends has type parameters
                let type_doc = self.build_type_doc(value_type);
                if should_break_before_conditional_type(cond) {
                    parts.push(hang_after_operator(d, type_doc));
                } else {
                    parts.push(d.text(" "));
                    parts.push(type_doc);
                }
            } else if type_has_internal_breaking(value_type) {
                // Types with internal breaking (braces, brackets, parens, angle brackets) stay hugged
                // Use wrapping version so TypeReference type args break internally when too long
                let type_doc = self.build_type_doc_with_wrapping_type_args(value_type);
                parts.push(d.text(" "));
                parts.push(type_doc);
            } else if has_complex_params {
                // Complex type parameters: use break-lhs layout
                // Type params break, `=` stays on same line, RHS stays inline
                // Example: type Foo<T extends string, U = number> = SomeLongType;
                // Breaks as:
                //   type Foo<
                //     T extends string,
                //     U = number,
                //   > = SomeLongType;
                let type_doc = self.build_type_doc(value_type);
                parts.push(d.text(" "));
                parts.push(type_doc);
            } else if self.type_operator_comment_forces_operand_own_line(value_type) {
                // keyof/typeof with a comment after the operator that forces the
                // operand down: keep the operator on the `=` line; its operand hangs
                // on the next line (consistent with the conditional / internal-breaking
                // arms).
                let type_doc = self.build_type_doc(value_type);
                parts.push(d.text(" "));
                parts.push(type_doc);
            } else {
                // Other types: break after `=` with a hanging indent when too long
                let type_doc = self.build_type_doc(value_type);
                parts.push(hang_after_operator(d, type_doc));
            }
        }

        d.concat(&parts)
    }

    /// Build doc for interface declaration
    ///
    /// Uses group mode when extends has multiple items - heritage breaks when group breaks.
    pub(super) fn build_interface_declaration_doc(
        &self,
        decl: &internal::TSInterfaceDeclaration<'_>,
    ) -> DocId {
        let d = self.d();

        // Compute positions for heritage comment extraction
        let pre_heritage_end = decl
            .type_parameters
            .as_ref()
            .map_or(decl.id.span.end, |tp| tp.span.end);
        // Use `extends` keyword position (not first heritage item start) so
        // heritage leading comments only cover name-to-extends, not extends-to-item
        let extends_keyword_start = decl
            .extends
            .first()
            .and_then(|e| self.find_keyword_in_range(pre_heritage_end, e.span.start, "extends"));
        let first_extends_start =
            extends_keyword_start.or_else(|| decl.extends.first().map(|e| e.span.start));

        // Comments between name/type-params and extends force group mode
        let has_heritage_comments = first_extends_start
            .is_some_and(|ext_start| self.has_comments_between(pre_heritage_end, ext_start));
        let has_heritage_line_comments = first_extends_start
            .is_some_and(|ext_start| self.has_line_comments_between(pre_heritage_end, ext_start));

        // Group mode: multiple extends items OR heritage comments
        let group_mode = decl.extends.len() > 1 || has_heritage_comments;

        let mut header_parts: DocBuf = smallvec![];
        if decl.declare {
            header_parts.push(d.text("declare "));
        }
        header_parts.push(d.text("interface"));
        // Comments between keyword and name: `interface /* c */ A {}`
        header_parts.push(d.text(" "));
        header_parts.push(
            self.build_inline_comments_between_doc_trailing_space(
                decl.span.start,
                decl.id.span.start,
            ),
        );
        header_parts.push(d.symbol(decl.id.name.to_u32()));

        // Comments between name and type params: `interface A/* c */ <T> {}`
        // Line comments get a hardline to prevent absorbing type params as comment text
        if let Some(type_params) = &decl.type_parameters {
            header_parts.push(self.build_name_to_type_params_comments(
                decl.id.span.end,
                type_params.span.start,
                CommentSpacing::Trailing,
            ));
        }

        // Build extends doc, with comments between `extends` keyword and first item
        let extends_doc = if !decl.extends.is_empty() {
            Some(self.build_heritage_clause_doc(
                HeritageKeyword::Extends,
                decl.extends,
                group_mode,
                extends_keyword_start,
            ))
        } else {
            None
        };

        // Build the header group (without body - body has hardlines that would force breaking)
        let header_doc = if group_mode {
            // Group mode: one unified group - when it breaks, extends breaks too
            if let Some(type_params) = &decl.type_parameters {
                // Type params get their own group - break independently of extends
                header_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
            }

            // Comments between name/type-params and extends
            if let Some(ext_start) = first_extends_start {
                let (inline, indent) =
                    self.build_heritage_leading_comment_parts(pre_heritage_end, ext_start);
                header_parts.extend(inline);

                // Extends clause with line break, preceded by any extra heritage comments
                if let Some(ext_doc) = extends_doc {
                    let mut heritage_parts = indent;
                    heritage_parts.push(d.line());
                    heritage_parts.push(ext_doc);
                    header_parts.push(d.indent(d.concat(&heritage_parts)));
                }
            } else if let Some(ext_doc) = extends_doc {
                header_parts.push(d.indent(d.concat(&[d.line(), ext_doc])));
            }

            let parts_doc = d.concat(&header_parts);
            if has_heritage_line_comments {
                d.group_break(parts_doc)
            } else {
                d.group(parts_doc)
            }
        } else {
            // Non-group mode: type params break independently, extends stays inline
            // (No heritage comments in this path - comments force group mode)
            if let Some(type_params) = &decl.type_parameters {
                header_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
            }

            // Extends clause stays inline
            if let Some(ext_doc) = extends_doc {
                header_parts.push(d.text(" "));
                header_parts.push(ext_doc);
            }

            d.concat(&header_parts)
        };

        // Handle comments between header and body: interface B /* comment */ {
        let header_end = if let Some(last_ext) = decl.extends.last() {
            last_ext.span.end
        } else if let Some(tp) = &decl.type_parameters {
            tp.span.end
        } else {
            decl.id.span.end
        };
        let body_start = decl.body.span.start;
        // Comments between the header and body `{`, plus the pre-brace spacing.
        // Shared with the class printer: each comment is kept on its own line (a
        // line comment doesn't absorb a following one), and a line comment forces
        // the brace onto the next line. See heritage_last_item_line_comment.
        let mut parts: DocBuf = smallvec![
            header_doc,
            self.build_header_pre_body_doc(true, header_end, body_start),
        ];

        if decl.body.body.is_empty() {
            parts.push(self.build_empty_body_with_comments_doc(decl.body.span));
        } else {
            // A comment trailing the opening `{` on its own line is kept on the
            // `{` line when the body expands (divergence from prettier, which
            // relocates it to its own line as the first member's leading
            // comment). See conformance_prettier.md §Comment relocation
            // (Class/interface/enum body `{`).
            let first_member_start = decl.body.body[0].span().start;
            let (brace_line_prefix, delimiter_pull_pos) =
                self.delimiter_line_comment_prefix(decl.body.span.start, first_member_start);
            parts.push(d.text("{"));
            parts.push(d.concat(&brace_line_prefix));
            parts.push(d.indent(d.concat(&[self.build_type_elements_doc(
                decl.body.body,
                decl.body.span.start,
                decl.body.span.end,
                delimiter_pull_pos,
            )])));
            parts.push(d.hardline());
            parts.push(d.text("}"));
        }

        d.concat(&parts)
    }

    /// Build doc for declare function with wrapping support for type parameters
    pub(super) fn build_declare_function_doc(
        &self,
        decl: &internal::TSDeclareFunction<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        // Handle async keyword
        if decl.r#async {
            parts.push(d.text("async "));
        }

        // Handle declare keyword (only for top-level declare functions,
        // not inside `declare namespace` where it's implicit)
        if decl.declare {
            parts.push(d.text("declare "));
        }

        // Handle function/function* keyword
        if decl.generator {
            parts.push(d.text("function*"));
        } else {
            parts.push(d.text("function"));
        }

        // Find paren position for comment boundary and later comment handling
        let paren_search_start = decl
            .type_parameters
            .as_ref()
            .map_or(decl.id.span.end, |tp| tp.span.end);
        let paren_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32);

        // Everything after the `function`→name gap is collected into `tail` (the
        // continuation), so a *line* comment in that gap indents the whole
        // signature one level (uniform declaration-header rule).
        let mut tail = smallvec![d.symbol(decl.id.name.to_u32())];

        // Comments between name and type params/parens: `declare function fn1/* c */ <T>()` or `fn1 /* c */()`
        // Line comments get a hardline to prevent absorbing type params as comment text
        let comment_end = decl
            .type_parameters
            .as_ref()
            .map_or_else(|| paren_pos.unwrap_or(decl.id.span.end), |tp| tp.span.start);
        tail.push(self.build_name_to_type_params_comments(
            decl.id.span.end,
            comment_end,
            CommentSpacing::for_type_params(decl.type_parameters.is_some()),
        ));

        // Type parameters with wrapping support
        if let Some(type_params) = &decl.type_parameters {
            tail.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Comments between type_params and `(` go after type_params
        if let (Some(tp), Some(pp)) = (decl.type_parameters.as_ref().map(|t| t.span.end), paren_pos)
        {
            self.append_type_params_to_paren_comments(&mut tail, tp, pp);
        }
        tail.push(self.build_signature_params_doc(decl.params, paren_pos));

        // Return type (preserves a comment between `)` and `:`)
        if let Some(return_type) = &decl.return_type {
            tail.push(self.build_signature_return_type_doc(paren_pos, return_type));
        }

        // Comments between return type (or `)`) and `;`
        self.append_signature_end_comments(
            &mut tail,
            decl.return_type.as_ref(),
            paren_pos,
            decl.span.end,
        );

        tail.push(d.text(";"));

        // Comments between `function` keyword and name; a line comment indents the
        // whole continuation (uniform declaration-header rule).
        parts.push(self.build_keyword_to_name_continuation(
            decl.span.start,
            decl.id.span.start,
            d.concat(&tail),
        ));

        d.group(d.concat(&parts))
    }

    /// Build doc for entity name
    pub(crate) fn build_entity_name_doc(&self, name: &internal::TSEntityName<'_>) -> DocId {
        // Delegate to standalone function - doesn't need printer state
        build_entity_name_doc(self.d(), name)
    }

    /// Build doc for type elements with comment handling
    ///
    /// `delimiter_pull_pos`, when `Some(pos)`, drops the first member's leading
    /// comments that share a source line with `pos` (the opening `{`) — the
    /// caller emits those as a prefix on the `{` line instead (the open-brace
    /// trailing-comment divergence). Pass `None` to keep the default behavior.
    fn build_type_elements_doc(
        &self,
        members: &[internal::TSTypeElement<'_>],
        body_start: u32,
        body_end: u32,
        delimiter_pull_pos: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut prev_end = body_start + 1; // after opening brace

        for (i, member) in members.iter().enumerate() {
            let member_start = member.span().start;
            let is_first = i == 0;

            // Find comments between previous element and this one
            // Filter out trailing same-line comments from the previous member
            // BUT keep multi-line block comments even if they start on the same line
            let all_comments: CommentVec<'_> =
                comments_in_range(self.comments, prev_end, member_start).collect();
            let leading_comments: CommentVec<'_> = if !is_first {
                all_comments
                    .iter()
                    .filter(|c| {
                        // Keep if not on same line as prev_end
                        if !self.is_same_line(prev_end, c.span.start) {
                            return true;
                        }
                        // Also keep multi-line block comments (they're always leading, never trailing)
                        if self.is_multiline_comment(c) {
                            return true;
                        }
                        // An inline comment that hugs this member on its line leads it
                        // (`a: 1, /* c */ b`), even though it shares the previous
                        // member's line — keep it here rather than letting it trail the
                        // previous member. (A comment with a newline after it instead
                        // trails the previous member: the `is_same_line` is false.)
                        self.is_same_line(c.span.end, member_start)
                    })
                    .copied()
                    .collect()
            } else {
                // First member: drop comments pulled onto the `{` line (emitted
                // as the brace-line prefix by the caller).
                self.first_member_leading_comments(all_comments, delimiter_pull_pos)
            };

            // Add separator before this member
            // For first member: just hardline
            // For other members: literalline + hardline if blank line in source, just hardline otherwise
            if i > 0 {
                let check_pos = if leading_comments.is_empty() {
                    member_start
                } else {
                    leading_comments[0].span.start
                };
                if self.has_blank_line_between(prev_end, check_pos) {
                    parts.push(d.literalline());
                }
            }
            // Always add hardline before member (or its leading comments)
            parts.push(d.hardline());

            // Print leading comments with blank line preservation
            parts.extend(
                self.build_leading_comments_with_blank_lines(&leading_comments, member_start),
            );

            // A preceding format-ignore directive keeps the member's source verbatim.
            // The member span includes its trailing `;`.
            let member_doc = if self.has_format_ignore_in_range(prev_end, member_start) {
                self.raw_source_doc(member.span())
            } else {
                self.build_type_element_doc(member)
            };
            parts.push(member_doc);

            // Handle trailing inline comments on same line after member
            // Skip multi-line block comments - they should be leading comments for the next element
            let upper_bound = members
                .get(i + 1)
                .map_or(body_end, |next| next.span().start);
            let next_start = members.get(i + 1).map(|next| next.span().start);
            for comment in comments_in_range(self.comments, member.span().end, upper_bound) {
                if self.is_same_line(member.span().end, comment.span.start) {
                    // Skip multi-line block comments (they're leading comments for next element)
                    if self.is_multiline_comment(comment) {
                        continue;
                    }
                    // An inline comment that hugs the next member on its line leads that
                    // member (`a: 1, /* c */ b`) — the leading filter emits it — so it
                    // must not also trail this one (which would duplicate it). A comment
                    // with a newline after it (next member on a later line) trails here.
                    if next_start.is_some_and(|ns| self.is_same_line(comment.span.end, ns)) {
                        break;
                    }
                    parts.push(self.build_trailing_comment_doc(comment));
                } else {
                    break; // Only same-line comments
                }
            }

            prev_end = member.span().end;
        }

        // Handle trailing comments after the last member (before closing `}`)
        parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end.saturating_sub(1)));

        d.concat(&parts)
    }

    /// Build doc for a single type element
    fn build_type_element_doc(&self, elem: &internal::TSTypeElement<'_>) -> DocId {
        let d = self.d();
        match elem {
            internal::TSTypeElement::PropertySignature(p) => {
                // Shared with the type-literal printer; the only difference is
                // the interface member carries its own `;`.
                d.concat(&[self.build_property_signature_member_doc(p), d.text(";")])
            }
            internal::TSTypeElement::MethodSignature(m) => {
                // Shared with the type-literal printer; the interface member
                // appends its own `;`.
                d.concat(&[self.build_method_signature_member_doc(m), d.text(";")])
            }
            internal::TSTypeElement::CallSignature(c) => {
                // Shared with the type-literal printer; the interface member
                // appends its own `;`.
                d.concat(&[self.build_call_signature_member_doc(c), d.text(";")])
            }
            internal::TSTypeElement::ConstructSignature(c) => {
                // Shared with the type-literal printer; the interface member
                // appends its own `;`.
                d.concat(&[self.build_construct_signature_member_doc(c), d.text(";")])
            }
            internal::TSTypeElement::IndexSignature(i) => {
                // Shared with the type-literal printer; the interface member
                // appends its own `;`.
                d.concat(&[self.build_index_signature_member_doc(i), d.text(";")])
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
        decl: &internal::TSEnumDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        let mut prefix = DocBuf::new();

        // `declare` prefix if ambient declaration
        if decl.declare {
            prefix.push(d.text("declare "));
        }

        // `const` prefix if const enum
        if decl.r#const {
            prefix.push(d.text("const "));
        }

        prefix.push(d.text("enum"));

        // Everything after the `enum`→name gap is collected into `parts` (the
        // continuation), so a *line* comment in that gap indents the whole
        // declaration one level (uniform declaration-header rule).
        let mut parts: DocBuf = smallvec![d.symbol(decl.id.name.to_u32())];

        // Handle comments between name and body: enum C /* comment */ {
        // Use comment-aware search to skip `{` inside comments.
        let enum_body_brace =
            self.find_char_outside_comments(decl.id.span.end, decl.span.end, b'{');
        if let Some(brace) = enum_body_brace
            && self.has_comments_between(decl.id.span.end, brace)
        {
            parts.push(self.build_inline_comments_between_doc(decl.id.span.end, brace));
        }
        parts.push(d.text(" "));

        // Find body start (after '{')
        let body_start = enum_body_brace.map_or(decl.span.start, |b| b + 1);
        let body_end = decl.span.end.saturating_sub(1); // Before '}'
        let body_span = Span::new(body_start - 1, decl.span.end); // Include '{' and '}'

        if decl.members.is_empty() {
            // Empty enum body - handle comments inside (a fitting block comment
            // stays inline as `enum E {/* c */}`).
            parts.push(self.build_empty_braces_inline_with_comments_doc(body_span));
        } else {
            // A comment trailing the opening `{` on its own line is kept on the
            // `{` line when the body expands (divergence from prettier, which
            // relocates it to its own line as the first member's leading
            // comment). See conformance_prettier.md §Comment relocation
            // (Class/interface/enum body `{`). `body_start - 1` is the `{`.
            let first_member_start = decl.members[0].span.start;
            let (brace_line_prefix, delimiter_pull_pos) =
                self.delimiter_line_comment_prefix(body_start - 1, first_member_start);

            parts.push(d.text("{"));
            parts.push(d.concat(&brace_line_prefix));
            // Build member docs with comment handling
            let mut member_parts = DocBuf::new();
            let mut prev_end = body_start;

            for (i, member) in decl.members.iter().enumerate() {
                let member_start = member.span.start;
                let is_first = i == 0;
                let is_last = i == decl.members.len() - 1;

                // Check for comments between previous position and this member.
                // First member: drop comments pulled onto the `{` line (emitted
                // as the brace-line prefix above).
                let comments: CommentVec<'_> =
                    comments_in_range(self.comments, prev_end, member_start)
                        .filter(|c| {
                            if is_first {
                                !delimiter_pull_pos
                                    .is_some_and(|dpos| self.comment_on_delimiter_line(dpos, c))
                            } else {
                                !self.is_same_line(prev_end, c.span.start)
                            }
                        })
                        .collect();

                // Check for blank lines
                if !is_first {
                    let check_pos = if comments.is_empty() {
                        member_start
                    } else {
                        comments[0].span.start
                    };
                    self.push_blank_preserving_hardline(&mut member_parts, prev_end, check_pos);
                }

                // Process leading comments
                self.emit_member_leading_comments(&mut member_parts, &comments, member_start);

                // A preceding format-ignore directive keeps the member's source
                // verbatim. The member span excludes the
                // trailing `,`, which the loop still appends below.
                let member_doc = if self.has_format_ignore_in_range(prev_end, member_start) {
                    self.raw_source_doc(member.span)
                } else {
                    self.build_enum_member_doc(member)
                };
                member_parts.push(member_doc);

                let member_end = member.span.end;
                let upper_bound = decl
                    .members
                    .get(i + 1)
                    .map_or(body_end, |next| next.span.start);

                // Find comma to split: comments before comma = trailing, after = trailing-after-comma
                // Enum members always have trailing commas, so comma must exist
                let comma_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    member_end as usize,
                    upper_bound as usize,
                    b',',
                );

                if let Some(cp) = comma_pos {
                    let cp = cp as u32;
                    // Trailing same-line comments before the comma (block comments
                    // inline, line comments in line_suffix) — same dispatch as the
                    // statement-list / class-member paths.
                    member_parts.extend(self.build_trailing_same_line_comment_docs(member_end, cp));

                    // Separator comma between members; no trailing comma on the last
                    // member under `trailingComma: 'none'`.
                    if !is_last {
                        member_parts.push(d.text(","));
                    }

                    // Same-line trailing comments after comma (line comments)
                    member_parts
                        .extend(self.build_trailing_same_line_comment_docs(cp + 1, upper_bound));

                    // Update prev_end past trailing comments
                    prev_end = self.find_end_with_trailing_comments(cp + 1);
                } else {
                    // Fallback: no comma found (shouldn't happen in valid enum)
                    if !is_last {
                        member_parts.push(d.text(","));
                    }
                    member_parts.extend(
                        self.build_trailing_same_line_comment_docs(member_end, upper_bound),
                    );

                    prev_end = self.find_end_with_trailing_comments(member_end);
                }
            }

            // Handle trailing comments after the last member
            member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));

            parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&member_parts)])));
            parts.push(d.hardline());
            parts.push(d.text("}"));
        }

        // Comments between `enum` and the name; a line comment indents the whole
        // continuation (uniform declaration-header rule).
        prefix.push(self.build_keyword_to_name_continuation(
            decl.span.start,
            decl.id.span.start,
            d.concat(&parts),
        ));
        d.concat(&prefix)
    }

    /// Build doc for a single enum member
    fn build_enum_member_doc(&self, member: &internal::TSEnumMember<'_>) -> DocId {
        let d = self.d();
        // Member id (identifier or string literal)
        let id_doc = match &member.id {
            internal::TSEnumMemberId::Identifier(id) => d.symbol(id.name.to_u32()),
            internal::TSEnumMemberId::String(lit) => {
                // String literal member name: `"hello"` in `enum { "hello" = 1 }`
                self.build_literal_doc(lit)
            }
        };

        // Initializer: ` = value`
        if let Some(init) = &member.initializer {
            let init_doc = self.build_expression_doc(init);

            // Extract comments between `=` and initializer value
            let id_end = match &member.id {
                internal::TSEnumMemberId::Identifier(id) => id.span.end,
                internal::TSEnumMemberId::String(lit) => lit.span.end,
            };
            let init_start = init.span().start;
            let eq_pos = self.find_equals_position(id_end, init_start);

            // The post-`=` value content (shared by the inline and the
            // continuation forms). For binary expressions, indent so wrapped
            // continuations align under the value; any `=`→value block comment
            // leads it.
            let value_doc = {
                let init_with_indent = if matches!(init, internal::Expression::BinaryExpression(_))
                {
                    d.indent(init_doc)
                } else {
                    init_doc
                };
                self.prepend_rhs_comments(init_with_indent, eq_pos + 1, init_start)
            };

            // A line comment between the name and `=` keeps the comment after the
            // name and drops `= value` to a continuation line indented one level
            // (preserve position — lossless when a second comment also trails the
            // member; prettier relocates past the value and merges the two onto one
            // line — see conformance_prettier.md §Comment relocation).
            if let Some(cont) =
                self.build_initializer_line_continuation(id_end, eq_pos, || value_doc)
            {
                d.concat(&[id_doc, cont])
            } else {
                // Comments between name and `=` (block stays inline: `a /* c */ = 1`)
                let id_doc = if self.has_comments_between(id_end, eq_pos) {
                    d.concat(&[
                        id_doc,
                        self.build_inline_comments_between_doc(id_end, eq_pos),
                    ])
                } else {
                    id_doc
                };
                d.concat(&[id_doc, d.text(" = "), value_doc])
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
        decl: &internal::TSModuleDeclaration<'_>,
    ) -> DocId {
        self.build_module_declaration_doc_inner(decl, true)
    }

    /// Inner helper for module declaration doc building
    /// `is_root` is true for the outermost declaration (prints `namespace` keyword)
    fn build_module_declaration_doc_inner(
        &self,
        decl: &internal::TSModuleDeclaration<'_>,
        is_root: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        // Only print keywords for root declaration
        if is_root {
            // `declare` prefix if ambient declaration
            if decl.declare {
                parts.push(d.text("declare "));
            }

            // `global` is special - it replaces namespace/module keyword
            if decl.global {
                // A comment in the `declare`→`global` gap (`declare /* c */ global {}`)
                // stays before `global`, matching prettier — `global` is both keyword
                // and name here, so there is no later name to relocate it onto (the
                // non-global branch handles its keyword→name gap below). For a bare
                // `global {}` the span starts at `global`, so the range is empty.
                let global_start = match &decl.id {
                    internal::TSModuleName::Identifier(id) => id.span.start,
                    internal::TSModuleName::Literal(lit) => lit.span.start,
                };
                parts.push(self.build_inline_comments_between_doc_trailing_space(
                    decl.span.start,
                    global_start,
                ));
                parts.push(d.text("global"));
            } else {
                // Use the original keyword (namespace or module)
                match decl.kind {
                    internal::TSModuleDeclarationKind::Namespace => {
                        parts.push(d.text("namespace "));
                    }
                    internal::TSModuleDeclarationKind::Module => {
                        parts.push(d.text("module "));
                    }
                }
            }
        }

        // Module/namespace name (if not global)
        if !decl.global {
            // Comments between keywords and name: `declare namespace /* c */ A {}`
            let name_start = match &decl.id {
                internal::TSModuleName::Identifier(id) => id.span.start,
                internal::TSModuleName::Literal(lit) => lit.span.start,
            };
            if is_root {
                parts.push(
                    self.build_inline_comments_between_doc_trailing_space(
                        decl.span.start,
                        name_start,
                    ),
                );
            }
            match &decl.id {
                internal::TSModuleName::Identifier(id) => {
                    parts.push(d.symbol(id.name.to_u32()));
                }
                internal::TSModuleName::Literal(lit) => {
                    parts.push(self.build_literal_doc(lit));
                }
            }
        }

        // Body (may be None for shorthand: `declare module 'name';`)
        match &decl.body {
            Some(internal::TSModuleDeclarationBody::TSModuleBlock(block)) => {
                // Handle comments between name and body: namespace D /* comment */ {
                let name_end = match &decl.id {
                    internal::TSModuleName::Identifier(id) => id.span.end,
                    internal::TSModuleName::Literal(lit) => lit.span.end,
                };
                if self.has_comments_between(name_end, block.span.start) {
                    parts.push(self.build_inline_comments_between_doc(name_end, block.span.start));
                }
                parts.push(d.text(" "));

                if block.body.is_empty() {
                    // Empty namespace body - handle comments inside
                    parts.push(self.build_empty_body_with_comments_doc(block.span));
                } else {
                    // A comment trailing the opening `{` on its own line is kept on
                    // the `{` line when the body expands (divergence from prettier,
                    // which relocates it to its own line as the body's leading
                    // comment). Same mechanism as block-statement bodies. See
                    // conformance_prettier.md §Comment relocation (Namespace/module
                    // body `{`).
                    let first_stmt_start = block.body[0].span().start;
                    let (brace_line_prefix, delimiter_pull_pos) =
                        self.delimiter_line_comment_prefix(block.span.start, first_stmt_start);

                    parts.push(d.text("{"));
                    parts.push(d.concat(&brace_line_prefix));

                    // Shared per-statement walk (leading comments, blank-line
                    // separators, format-ignore, trailing same-line comments) —
                    // same as block-statement bodies.
                    let body_start = block.span.start + 1; // After opening '{'
                    let body_end = block.span.end.saturating_sub(1); // Before '}'
                    let (mut stmt_parts, prev_end, _prev_stmt_end) = self
                        .build_statement_list_docs(
                            block.body,
                            body_start,
                            body_end,
                            DocBuf::new(),
                            delimiter_pull_pos,
                        );

                    // Handle own-line trailing comments after the last statement
                    stmt_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));

                    parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&stmt_parts)])));
                    parts.push(d.hardline());
                    parts.push(d.text("}"));
                }
            }
            Some(internal::TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                // Nested namespace: `namespace Outer.Inner { }`
                // Print as `Outer.Inner` (dot-separated)
                parts.push(d.text("."));
                parts.push(self.build_module_declaration_doc_inner(nested, false));
            }
            None => {
                // Shorthand ambient module: `declare module 'name';`
                parts.push(d.text(";"));
            }
        }

        d.concat(&parts)
    }
}
