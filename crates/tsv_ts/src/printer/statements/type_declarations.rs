// Type declaration printing (type aliases, interfaces, enums, namespaces, declare functions)
// plus shared type-argument and entity-name helpers

use super::{Printer, build_entity_name_doc, should_hug_union_type, unwrap_parenthesized};
use crate::ast::internal::{self, TSType};
use crate::printer::analysis::skip_identifier_at;
use crate::printer::layout::hang_after_operator;
use crate::printer::{CommentFilter, CommentSpacing};
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{Comment, SymbolToU32, comments_in_range};

/// Check if a type is "generic" - i.e., has type parameters.
/// This matches prettier's `isGeneric` function in assignment.js.
fn is_generic_type(ts_type: &TSType) -> bool {
    match ts_type {
        TSType::Function(f) => f.type_parameters.is_some(),
        TSType::TypeReference(r) => r.type_arguments.is_some(),
        _ => false,
    }
}

/// Check if we should break before the conditional type in a type alias.
/// Returns true if either checkType or extendsType has type parameters.
/// This matches prettier's `shouldBreakBeforeConditionalType` in assignment.js.
fn should_break_before_conditional_type(conditional: &internal::TSConditionalType) -> bool {
    is_generic_type(&conditional.check_type) || is_generic_type(&conditional.extends_type)
}

/// Returns true if the type has its own internal breaking mechanism
/// (e.g., braces, brackets, parentheses) and should NOT break after `=`.
fn type_has_internal_breaking(ts_type: &TSType) -> bool {
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
        decl: &internal::TSTypeAliasDeclaration,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![];
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
            let mut indent_parts = vec![d.hardline()];
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
        let mut trailing_line_parts = Vec::new();
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
    /// a line comment. Such a value carries a comment-forced hardline, so the
    /// type-alias RHS keeps the operator on the `=` line (the operand hangs on the
    /// next line via the operator's own layout) instead of breaking after `=` —
    /// matching prettier and the conditional / internal-breaking arms. A long
    /// *comment-free* operator still breaks after `=` (the hanging-indent arm).
    fn type_operator_has_leading_line_comment(&self, ty: &TSType) -> bool {
        match ty {
            TSType::TypeOperator(o) => {
                let kw_end = o.span.start + o.operator.as_str().len() as u32;
                self.has_line_comments_between(kw_end, o.type_annotation.span().start)
            }
            TSType::TypeQuery(q) => {
                let kw_end = q.span.start + 6; // "typeof".len()
                self.has_line_comments_between(kw_end, q.expr_name.span().start)
            }
            _ => false,
        }
    }

    fn build_type_alias_eq_value_doc(
        &self,
        decl: &internal::TSTypeAliasDeclaration,
        eq_pos: u32,
        type_start: u32,
        has_complex_params: bool,
        lead_space: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![d.text(if lead_space { " =" } else { "=" })];

        let force_break = self.comments_force_own_line_between(eq_pos + 1, type_start);

        if force_break {
            // Line/multiline block comments force type to next line with indent.
            // Line comments stay on `=` line; multiline blocks go into the indent.
            // Example: `type A = // comment\n  B;`
            // Example: `type J =\n  /* comment\n   */\n  K | L;`
            let mut inline_parts = Vec::new();
            let mut indent_comment_parts = Vec::new();

            // Only the first single-line comment hugs the `=` line; multiline
            // blocks (any position) and every subsequent comment go on their own
            // line in the indent. Two line comments must not merge onto one line —
            // the second `//` would stop being a delimiter (a boundary loss).
            let mut first = true;
            for comment in comments_in_range(self.comments, eq_pos + 1, type_start) {
                let multiline_block = comment.is_block && self.is_multiline_comment(comment);
                if first && !multiline_block {
                    inline_parts.push(d.text(" "));
                    inline_parts.push(self.build_comment_doc(comment));
                } else {
                    indent_comment_parts.push(self.build_comment_doc(comment));
                    indent_comment_parts.push(d.hardline());
                }
                first = false;
            }

            parts.extend(inline_parts);

            // Type uses its own group (via build_type_doc) so unions/intersections
            // can independently decide whether to break
            let type_doc = self.build_type_doc(&decl.type_annotation);
            let mut indent_content = vec![d.hardline()];
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
                let type_doc = self.build_union_type_doc(u, false);
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
            } else if self.type_operator_has_leading_line_comment(value_type) {
                // keyof/typeof with a line comment after the operator: keep the
                // operator on the `=` line; its operand hangs on the next line
                // (consistent with the conditional / internal-breaking arms).
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
        decl: &internal::TSInterfaceDeclaration,
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

        let mut header_parts = vec![];
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
                "extends",
                &decl.extends,
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
        let mut parts = vec![
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
                &decl.body.body,
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
    pub(super) fn build_declare_function_doc(&self, decl: &internal::TSDeclareFunction) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

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

        // Comments between `function` keyword and name
        parts.push(self.build_keyword_to_name_comments(decl.span.start, decl.id.span.start));
        parts.push(d.symbol(decl.id.name.to_u32()));

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

        // Comments between name and type params/parens: `declare function fn1/* c */ <T>()` or `fn1 /* c */()`
        // Line comments get a hardline to prevent absorbing type params as comment text
        let comment_end = decl
            .type_parameters
            .as_ref()
            .map_or_else(|| paren_pos.unwrap_or(decl.id.span.end), |tp| tp.span.start);
        parts.push(self.build_name_to_type_params_comments(
            decl.id.span.end,
            comment_end,
            CommentSpacing::for_type_params(decl.type_parameters.is_some()),
        ));

        // Type parameters with wrapping support
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Comments between type_params and `(` go after type_params
        if let (Some(tp), Some(pp)) = (decl.type_parameters.as_ref().map(|t| t.span.end), paren_pos)
        {
            self.append_type_params_to_paren_comments(&mut parts, tp, pp);
        }
        parts.push(self.build_signature_params_doc(&decl.params, paren_pos));

        // Return type (preserves a comment between `)` and `:`)
        if let Some(return_type) = &decl.return_type {
            parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
        }

        // Comments between return type (or `)`) and `;`
        self.append_signature_end_comments(
            &mut parts,
            decl.return_type.as_ref(),
            paren_pos,
            decl.span.end,
        );

        parts.push(d.text(";"));

        d.group(d.concat(&parts))
    }

    /// Build doc for entity name
    pub(crate) fn build_entity_name_doc(&self, name: &internal::TSEntityName) -> DocId {
        // Delegate to standalone function - doesn't need printer state
        build_entity_name_doc(self.d(), name)
    }

    /// Build doc for a type used as a type argument.
    ///
    /// For single type arg contexts, uses normal doc (allows object types to break).
    /// For multiple type arg contexts, uses hugging (objects don't break independently).
    fn build_type_arg_doc(&self, param: &TSType, is_multi_arg: bool) -> DocId {
        if is_multi_arg {
            self.build_type_doc_for_type_arg(param)
        } else {
            self.build_type_doc(param)
        }
    }

    /// Comments that force the `<...>` list to the multiline layout: line
    /// comments anywhere (including before the first argument, e.g.
    /// `Foo<// leading\n  a>`) or own-line block comments — neither can render
    /// inline.
    fn type_arguments_force_expansion(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> bool {
        let has_leading_line_comment = args.params.first().is_some_and(|first| {
            self.has_line_comments_between(args.span.start + 1, first.span().start)
        });
        has_leading_line_comment
            || self.has_line_comments_in_delimited_list(
                &args.params,
                TSType::span,
                args.span.end - 1,
            )
            || self.has_own_line_block_comments_in_bracket_list(
                args.span,
                &args.params,
                TSType::span,
            )
    }

    /// Build doc for type arguments: `<T, U>`.
    ///
    /// Single arg: always inline. Multi-arg: group-based breaking via shared helper.
    /// Use `build_type_arguments_doc_wrapping` for single-arg hugging (e.g., `Array<{...}>`).
    pub(crate) fn build_type_arguments_doc(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> DocId {
        let d = self.d();
        if args.params.is_empty() {
            return d.text("<>");
        }

        if self.type_arguments_force_expansion(args) {
            return self.build_type_arguments_doc_with_line_comments(args);
        }

        // Single type argument: inline (matches Prettier's shouldInline for len==1)
        if args.params.len() == 1 {
            let mut parts = Vec::new();
            let prev_end = args.span.start + 1; // After the opening `<`
            let param_start = args.params[0].span().start;
            let param_end = args.params[0].span().end;
            let before_close = args.span.end - 1;

            self.append_leading_inline_block_comments(&mut parts, prev_end, param_start);
            parts.push(self.build_type_arg_doc(&args.params[0], false));
            self.append_trailing_inline_block_comments(&mut parts, param_end, before_close);
            return d.concat(&[d.text("<"), d.concat(&parts), d.text(">")]);
        }

        // Multiple type arguments: use group so they can break at print width.
        // Matches Prettier's group([<, indent([softline, join([",", line], args)]), softline, >])
        self.build_type_arguments_doc_multi_arg(args)
    }

    /// Build doc for type arguments with width-based wrapping support.
    ///
    /// Inline: `<T, U, V>`
    /// Wrapped: `<\n\tT,\n\tU,\n\tV\n>`
    ///
    /// Special case: single TypeLiteral argument hugs the opening `<`:
    /// `Array<{prop: string}>` stays hugged, and when broken:
    /// ```text
    /// Array<{
    ///     prop: string;
    /// }>
    /// ```
    ///
    /// Use this when type arguments should break independently of parent context,
    /// such as in property type annotations.
    pub(crate) fn build_type_arguments_doc_wrapping(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> DocId {
        let d = self.d();
        if args.params.is_empty() {
            return d.text("<>");
        }

        if self.type_arguments_force_expansion(args) {
            return self.build_type_arguments_doc_with_line_comments(args);
        }

        // Single type argument inlining, matching Prettier's `shouldInline` logic.
        // Three categories are inlined (no group/softlines):
        //
        // 1. Simple types: keywords (`string`, `number`) and TypeReference without
        //    type args (`T`, `MyType`). These are atomic and never need breaking.
        // 2. Object types: TypeLiteral and Mapped types handle their own breaking.
        // 3. Hugged unions: unions with a brace-delimited member like `{...} | null`.
        //
        // Without inlining, the group/softlines create Break-mode Line nodes in
        // `fits()` rest_commands, causing upstream groups (like arrays in Fluid
        // assignment layout) to incorrectly appear to "fit" — Line in Break mode
        // returns true from `fits()`, short-circuiting the width check.
        if args.params.len() == 1 {
            let unwrapped = unwrap_parenthesized(&args.params[0]);
            let is_simple = matches!(unwrapped, TSType::Keyword(_))
                || matches!(unwrapped, TSType::TypeReference(r) if r.type_arguments.is_none());
            let is_huggable = is_simple
                || matches!(unwrapped, TSType::TypeLiteral(_) | TSType::Mapped(_))
                || matches!(unwrapped, TSType::Union(u) if
                    should_hug_union_type(u)
                    && u.types.iter().any(|t| matches!(t, TSType::TypeLiteral(_) | TSType::Mapped(_)))
                );
            if is_huggable {
                let mut parts = vec![d.text("<")];

                // Include leading comments: `Array</* comment */ {...}>`
                let param_start = args.params[0].span().start;
                let param_end = args.params[0].span().end;
                let after_open = args.span.start + 1; // After the opening `<`
                let before_close = args.span.end - 1; // Before the closing `>`
                self.append_leading_inline_block_comments(&mut parts, after_open, param_start);

                parts.push(self.build_type_arg_doc(&args.params[0], false));

                // Include trailing comments: `Array<{...} /* trailing */>`
                self.append_trailing_inline_block_comments(&mut parts, param_end, before_close);

                parts.push(d.text(">"));
                return d.concat(&parts);
            }
        }

        self.build_type_arguments_doc_multi_arg(args)
    }

    /// Build multi-arg type arguments with group-based breaking.
    ///
    /// Matches Prettier's `group([<, indent([softline, join([",", line], args)]), softline, >])`.
    /// Used by both `build_type_arguments_doc` and `build_type_arguments_doc_wrapping`
    /// for 2+ type arguments (and non-huggable single args in the wrapping variant).
    fn build_type_arguments_doc_multi_arg(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> DocId {
        let d = self.d();
        let mut inner_parts = Vec::new();
        let mut prev_end = args.span.start + 1; // After the opening `<`

        for (i, param) in args.params.iter().enumerate() {
            let param_start = param.span().start;
            let is_last = i == args.params.len() - 1;

            let mut arg_parts = Vec::new();

            // Add leading block comments before this type argument
            self.append_leading_inline_block_comments(&mut arg_parts, prev_end, param_start);

            arg_parts.push(self.build_type_arg_doc(param, true));

            // Add trailing block comments after this type argument (before comma)
            let param_end = param.span().end;
            prev_end = if i + 1 < args.params.len() {
                let next_start = args.params[i + 1].span().start;
                let comma_pos = self.find_list_comma(param_end, next_start);
                self.append_trailing_inline_block_comments(&mut arg_parts, param_end, comma_pos);
                comma_pos + 1 // After comma — leading comments picked up next iteration
            } else {
                let before_close = args.span.end - 1;
                self.append_trailing_inline_block_comments(&mut arg_parts, param_end, before_close);
                before_close
            };

            if i > 0 {
                inner_parts.push(d.line());
            }
            inner_parts.push(d.concat(&arg_parts));
            if !is_last {
                inner_parts.push(d.text(","));
            }
            // Note: type arguments don't get trailing commas (unlike params)
        }

        d.group(d.concat(&[
            d.text("<"),
            d.indent_softline(d.concat(&inner_parts)),
            d.softline(),
            d.text(">"),
        ]))
    }

    /// Build doc for type arguments with expanding comments (line or own-line block).
    ///
    /// Line comments and own-line block comments force multiline because they can't appear inline.
    fn build_type_arguments_doc_with_line_comments(
        &self,
        args: &internal::TSTypeParameterInstantiation,
    ) -> DocId {
        let d = self.d();

        // Single-arg with only a leading line comment: hug `<` and `>`
        // (`Array<// leading\n  a>`) instead of full multiline.
        if args.params.len() == 1 {
            let param = &args.params[0];
            let param_start = param.span().start;
            let param_end = param.span().end;
            let before_close = args.span.end - 1;
            let has_trailing =
                tsv_lang::has_comments_in_range(self.comments, param_end, before_close);
            if !has_trailing {
                let leading =
                    self.build_leading_comments_multiline(args.span.start + 1, param_start);
                if !leading.is_empty() {
                    let mut parts = vec![d.text("<")];
                    parts.extend(leading);
                    parts.push(self.build_type_arg_doc(param, false));
                    parts.push(d.text(">"));
                    return d.concat(&parts);
                }
            }
        }

        // A comment trailing the opening `<` on its own line is kept on the `<`
        // line (divergence from prettier, which relocates it to its own line as
        // the first argument's leading comment). Multi-argument path only — the
        // single-argument leading-comment case hugs `<`/`>` above and matches
        // prettier. See conformance_prettier.md §Comment relocation
        // (Type-argument `<`).
        let first_param_start = args.params[0].span().start;
        let (angle_line_prefix, delimiter_pull_pos) =
            self.delimiter_line_comment_prefix(args.span.start, first_param_start);

        let mut inner_parts = Vec::new();
        let mut prev_end = args.span.start + 1; // After the opening `<`

        for (i, param) in args.params.iter().enumerate() {
            let param_start = param.span().start;
            let param_end = param.span().end;
            let is_last = i == args.params.len() - 1;

            // Leading comments (after previous comma or `<`). For the first arg,
            // drop comments pulled onto the `<` line (emitted as the angle-line
            // prefix below).
            if i == 0
                && let Some(delim) = delimiter_pull_pos
            {
                inner_parts.extend(self.build_leading_comments_multiline_after_delim(
                    prev_end,
                    param_start,
                    delim,
                ));
            } else {
                inner_parts.extend(self.build_leading_comments_multiline(prev_end, param_start));
            }

            inner_parts.push(self.build_type_arg_doc(param, args.params.len() > 1));

            if !is_last {
                let next_start = args.params[i + 1].span().start;
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    param_end,
                    next_start,
                );
            } else {
                // Last param: trailing comments before `>`
                let before_close = args.span.end - 1;
                inner_parts.extend(self.build_trailing_comments_multiline(param_end, before_close));
                prev_end = before_close;
            }
        }

        d.concat(&[
            d.text("<"),
            d.concat(&angle_line_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])),
            d.hardline(),
            d.text(">"),
        ])
    }

    /// Build doc for type elements with comment handling
    ///
    /// `delimiter_pull_pos`, when `Some(pos)`, drops the first member's leading
    /// comments that share a source line with `pos` (the opening `{`) — the
    /// caller emits those as a prefix on the `{` line instead (the open-brace
    /// trailing-comment divergence). Pass `None` to keep the default behavior.
    fn build_type_elements_doc(
        &self,
        members: &[internal::TSTypeElement],
        body_start: u32,
        body_end: u32,
        delimiter_pull_pos: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end = body_start + 1; // after opening brace

        for (i, member) in members.iter().enumerate() {
            let member_start = member.span().start;
            let is_first = i == 0;

            // Find comments between previous element and this one
            // Filter out trailing same-line comments from the previous member
            // BUT keep multi-line block comments even if they start on the same line
            let all_comments: Vec<_> =
                comments_in_range(self.comments, prev_end, member_start).collect();
            let leading_comments: Vec<_> = if !is_first {
                all_comments
                    .iter()
                    .filter(|c| {
                        // Keep if not on same line as prev_end
                        if !self.is_same_line(prev_end, c.span.start) {
                            return true;
                        }
                        // Also keep multi-line block comments (they're always leading, never trailing)
                        self.is_multiline_comment(c)
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

            // A preceding `// prettier-ignore` keeps the member's source verbatim
            // (matches prettier). The member span includes its trailing `;`.
            let member_doc = if self.has_prettier_ignore_in_range(prev_end, member_start) {
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
            for comment in comments_in_range(self.comments, member.span().end, upper_bound) {
                if self.is_same_line(member.span().end, comment.span.start) {
                    // Skip multi-line block comments (they're leading comments for next element)
                    if self.is_multiline_comment(comment) {
                        continue;
                    }

                    if comment.is_block {
                        // Single-line block comments are inline, affect width
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                    } else {
                        // Line comments go in line_suffix, don't affect width
                        parts.push(self.build_trailing_line_comment_doc(comment));
                    }
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
    fn build_type_element_doc(&self, elem: &internal::TSTypeElement) -> DocId {
        let d = self.d();
        match elem {
            internal::TSTypeElement::PropertySignature(p) => {
                let mut parts = Vec::new();
                if p.readonly {
                    // Preserve comments after the keyword (e.g., `readonly /* c */ a`);
                    // bounded at `[` for computed keys (inner comments are the
                    // bracket builder's)
                    let key_start = p.key.span().start;
                    let mut cursor = p.span.start;
                    self.push_member_keyword_doc(&mut parts, "readonly ", &mut cursor, key_start);
                    let bound = if p.computed {
                        find_char_skipping_comments(
                            self.source.as_bytes(),
                            cursor as usize,
                            key_start as usize,
                            b'[',
                        )
                        .map_or(cursor, |pos| pos as u32)
                    } else {
                        key_start
                    };
                    self.push_pre_name_comments_doc(&mut parts, cursor, bound);
                }
                // Handle computed property keys: [key]: type
                let (key_doc, key_region_end) =
                    self.build_type_member_key_doc(p.span.start, &p.key, p.computed, true);
                parts.push(key_doc);
                // Comments before `?` modifier (e.g., `a /* c */?: number`)
                if p.optional {
                    let after_q = self.push_modifier_marker_doc(&mut parts, key_region_end, b'?');
                    // Comments between `?` and `:` type annotation
                    if let Some(ta) = &p.type_annotation
                        && self.has_comments_between(after_q, ta.span.start)
                    {
                        let comment_doc =
                            self.build_inline_comments_between_doc(after_q, ta.span.start);
                        parts.push(d.concat(&[comment_doc, d.text(" ")]));
                    }
                } else if let Some(ta) = &p.type_annotation {
                    // Comments between key and `:` type annotation (e.g., `[x] /* c */: number`)
                    if self.has_comments_between(key_region_end, ta.span.start) {
                        parts.push(
                            self.build_inline_comments_between_doc(key_region_end, ta.span.start),
                        );
                    }
                }
                if let Some(ta) = &p.type_annotation {
                    // Use width-aware wrapping for generic type arguments
                    parts.push(self.build_type_annotation_doc_wrapping(ta));
                    // Comments between type and `;`
                    for comment in comments_in_range(self.comments, ta.span.end, p.span.end) {
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                }
                parts.push(d.text(";"));
                d.concat(&parts)
            }
            internal::TSTypeElement::MethodSignature(m) => {
                let mut parts = Vec::new();
                // Print accessor keyword for get/set signatures, preserving
                // comments between keyword and name
                match m.kind {
                    internal::MethodKind::Get => self.push_accessor_keyword_doc(
                        &mut parts,
                        "get ",
                        m.span.start,
                        m.key.span().start,
                    ),
                    internal::MethodKind::Set => self.push_accessor_keyword_doc(
                        &mut parts,
                        "set ",
                        m.span.start,
                        m.key.span().start,
                    ),
                    _ => {}
                }
                // Handle computed method keys: [key](): type
                let (key_doc, key_region_end) =
                    self.build_type_member_key_doc(m.span.start, &m.key, m.computed, false);
                parts.push(key_doc);
                // Comments before `?` modifier (e.g., `b /* c */?(x): void`)
                if m.optional {
                    let after_q = self.push_modifier_marker_doc(&mut parts, key_region_end, b'?');
                    // Comments between `?` and next token (type params or parens)
                    let next_token = m.type_parameters.as_ref().map_or_else(
                        || {
                            // Find `(` position skipping comments (not str::find which matches inside comments)
                            find_char_skipping_comments(
                                self.source.as_bytes(),
                                after_q as usize,
                                self.source.len(),
                                b'(',
                            )
                            .map_or(m.span.end, |p| p as u32)
                        },
                        |tp| tp.span.start,
                    );
                    if self.has_comments_between(after_q, next_token) {
                        parts.push(self.build_inline_comments_between_doc(after_q, next_token));
                    }
                } else {
                    // Comments between key and next token (e.g., `[x] /* c */(): void`)
                    // Line comments get a hardline to prevent absorbing type params as comment text
                    let next_token = m.type_parameters.as_ref().map_or_else(
                        || {
                            find_char_skipping_comments(
                                self.source.as_bytes(),
                                key_region_end as usize,
                                self.source.len(),
                                b'(',
                            )
                            .map_or(key_region_end, |p| p as u32)
                        },
                        |tp| tp.span.start,
                    );
                    parts.push(self.build_name_to_type_params_comments(
                        key_region_end,
                        next_token,
                        CommentSpacing::for_type_params(m.type_parameters.is_some()),
                    ));
                }
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &m.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                // Find `(` position for comment handling (skip comments to avoid matching `(` inside them)
                let paren_search_start = m
                    .type_parameters
                    .as_ref()
                    .map_or(key_region_end, |tp| tp.span.end);
                let method_paren_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    paren_search_start as usize,
                    self.source.len(),
                    b'(',
                )
                .map(|p| p as u32);
                // Comments between type_params `>` and `(` go after type_params
                if let (Some(tp), Some(pp)) = (m.type_parameters.as_ref(), method_paren_pos) {
                    self.append_type_params_to_paren_comments(&mut parts, tp.span.end, pp);
                }
                // Width-based breaking for params
                parts.push(self.build_signature_params_doc(&m.params, method_paren_pos));
                if let Some(rt) = &m.return_type {
                    parts.push(self.build_signature_return_type_doc(method_paren_pos, rt));
                }
                // Comments between return type (or params) and `;`
                self.append_signature_end_comments(
                    &mut parts,
                    m.return_type.as_ref(),
                    method_paren_pos,
                    m.span.end,
                );
                parts.push(d.text(";"));
                d.group(d.concat(&parts))
            }
            internal::TSTypeElement::CallSignature(c) => {
                let mut parts = Vec::new();
                // Type parameters: `<T>` or `<T, U>`
                if let Some(type_params) = &c.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                // Find `(` position for comment handling
                let paren_search_start = c
                    .type_parameters
                    .as_ref()
                    .map_or(c.span.start, |tp| tp.span.end);
                let call_paren_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    paren_search_start as usize,
                    self.source.len(),
                    b'(',
                )
                .map(|p| p as u32);
                // Comments between type_params and `(` go after type_params
                if let (Some(tp), Some(pp)) = (
                    c.type_parameters.as_ref().map(|t| t.span.end),
                    call_paren_pos,
                ) {
                    self.append_type_params_to_paren_comments(&mut parts, tp, pp);
                }
                // Width-based breaking for params
                parts.push(self.build_signature_params_doc(&c.params, call_paren_pos));
                if let Some(rt) = &c.return_type {
                    parts.push(self.build_signature_return_type_doc(call_paren_pos, rt));
                }
                // Comments between return type (or params) and `;`
                self.append_signature_end_comments(
                    &mut parts,
                    c.return_type.as_ref(),
                    call_paren_pos,
                    c.span.end,
                );
                parts.push(d.text(";"));
                d.group(d.concat(&parts))
            }
            internal::TSTypeElement::ConstructSignature(c) => {
                let mut parts = vec![d.text("new ")];
                // Type parameters: `<T>` or `<T, U>`
                if let Some(type_params) = &c.type_parameters {
                    // Comments between `new` and `<T>`: `new /* c */ <T>(...)`
                    let new_end = c.span.start + 3;
                    if let Some(doc) = self.build_name_to_type_params_comments_opt(
                        new_end,
                        type_params.span.start,
                        CommentSpacing::Trailing,
                    ) {
                        parts.push(doc);
                    }
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }
                // Find `(` position for comment handling
                let paren_search_start = c
                    .type_parameters
                    .as_ref()
                    .map_or(c.span.start, |tp| tp.span.end);
                let ctor_paren_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    paren_search_start as usize,
                    self.source.len(),
                    b'(',
                )
                .map(|p| p as u32);
                // Comments between type_params and `(` go after type_params
                if let (Some(tp), Some(pp)) = (
                    c.type_parameters.as_ref().map(|t| t.span.end),
                    ctor_paren_pos,
                ) {
                    self.append_type_params_to_paren_comments(&mut parts, tp, pp);
                }
                // Without type params, comments between `new` and `(` stay in
                // place: `new /* c */ (a: number)` (prettier relocates them
                // into the parens). The "new " text already carries the
                // leading space, so blocks get only a trailing space and line
                // comments a hardline.
                if c.type_parameters.is_none()
                    && let Some(pp) = ctor_paren_pos
                {
                    for comment in comments_in_range(self.comments, c.span.start + 3, pp) {
                        parts.push(self.build_comment_doc(comment));
                        if comment.is_block {
                            parts.push(d.text(" "));
                        } else {
                            parts.push(d.hardline());
                        }
                    }
                }
                // Width-based breaking for params
                parts.push(self.build_signature_params_doc(&c.params, ctor_paren_pos));
                if let Some(rt) = &c.return_type {
                    parts.push(self.build_signature_return_type_doc(ctor_paren_pos, rt));
                }
                // Comments between return type (or params) and `;`
                self.append_signature_end_comments(
                    &mut parts,
                    c.return_type.as_ref(),
                    ctor_paren_pos,
                    c.span.end,
                );
                parts.push(d.text(";"));
                d.group(d.concat(&parts))
            }
            internal::TSTypeElement::IndexSignature(i) => {
                let mut parts = Vec::new();
                if i.readonly {
                    // Preserve comments before the `[` (e.g., `readonly /* c */ [k: string]: T`)
                    let bracket_bound = i.parameters.first().map_or(i.span.end, |p| p.span.start);
                    let mut cursor = i.span.start;
                    self.push_member_keyword_doc(
                        &mut parts,
                        "readonly ",
                        &mut cursor,
                        bracket_bound,
                    );
                    let bracket_pos = find_char_skipping_comments(
                        self.source.as_bytes(),
                        cursor as usize,
                        bracket_bound as usize,
                        b'[',
                    )
                    .map_or(cursor, |p| p as u32);
                    self.push_pre_name_comments_doc(&mut parts, cursor, bracket_pos);
                }
                // Build each `key: keyType` param, then wrap `[ … ]` in a group so the
                // bracket breaks (key onto its own line) when the key type breaks — matching
                // prettier's index-signature.js. The type-literal printer
                // (`build_type_element_index_signature_doc`) uses the same structure.
                let param_docs: Vec<DocId> = i
                    .parameters
                    .iter()
                    .map(|param| {
                        let mut param_parts = vec![d.symbol(param.name.to_u32())];
                        if let Some(ta) = &param.type_annotation {
                            // Comments between the key name and the colon: `[key /* c */ : string]`.
                            // Prettier adds a space before `:` when such a comment is present.
                            let colon_pos = ta.span.start;
                            let name_end = skip_identifier_at(
                                self.source.as_bytes(),
                                param.span.start as usize,
                                colon_pos as usize,
                            ) as u32;
                            if let Some(comment_doc) =
                                self.build_inline_comments_between_doc_opt(name_end, colon_pos)
                            {
                                param_parts.push(comment_doc);
                                param_parts.push(d.text(" "));
                            }
                            // Delegate the `: keyType` — colon→type comments (line comments break,
                            // never merge) and the union/intersection break layout — to the shared
                            // annotation printer.
                            param_parts.push(self.build_type_annotation_doc(ta));
                        }
                        d.concat(&param_parts)
                    })
                    .collect();
                parts.push(d.group(d.concat(&[
                    d.text("["),
                    d.indent_softline(d.join(param_docs, ", ")),
                    d.softline(),
                    d.text("]"),
                ])));
                // Value type annotation: use build_type_annotation_doc for comment handling
                parts.push(self.build_type_annotation_doc(&i.type_annotation));
                parts.push(d.text(";"));
                d.concat(&parts)
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
    pub(super) fn build_enum_declaration_doc(&self, decl: &internal::TSEnumDeclaration) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // `declare` prefix if ambient declaration
        if decl.declare {
            parts.push(d.text("declare "));
        }

        // `const` prefix if const enum
        if decl.r#const {
            parts.push(d.text("const "));
        }

        // Comments between keywords and name
        parts.push(d.text("enum"));
        parts.push(self.build_keyword_to_name_comments(decl.span.start, decl.id.span.start));
        parts.push(d.symbol(decl.id.name.to_u32()));

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
        let body_span = tsv_lang::Span::new(body_start - 1, decl.span.end); // Include '{' and '}'

        if decl.members.is_empty() {
            // Empty enum body - handle comments inside
            parts.push(self.build_empty_body_with_comments_doc(body_span));
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
            let mut member_parts = Vec::new();
            let mut prev_end = body_start;

            for (i, member) in decl.members.iter().enumerate() {
                let member_start = member.span.start;
                let is_first = i == 0;

                // Check for comments between previous position and this member.
                // First member: drop comments pulled onto the `{` line (emitted
                // as the brace-line prefix above).
                let comments: Vec<_> = comments_in_range(self.comments, prev_end, member_start)
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
                    if self.has_blank_line_between(prev_end, check_pos) {
                        member_parts.push(d.literalline());
                    }
                    member_parts.push(d.hardline());
                }

                // Process leading comments
                for comment in &comments {
                    member_parts.push(self.build_comment_doc(comment));
                    // Block comment on same line as member gets space, otherwise hardline
                    if comment.is_block && self.is_same_line(comment.span.end, member_start) {
                        member_parts.push(d.text(" "));
                    } else {
                        member_parts.push(d.hardline());
                    }
                }

                // A preceding `// prettier-ignore` keeps the member's source
                // verbatim (matches prettier). The member span excludes the
                // trailing `,`, which the loop still appends below.
                let member_doc = if self.has_prettier_ignore_in_range(prev_end, member_start) {
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

                    // Comma
                    member_parts.push(d.text(","));

                    // Same-line trailing comments after comma (line comments)
                    member_parts
                        .extend(self.build_trailing_same_line_comment_docs(cp + 1, upper_bound));

                    // Update prev_end past trailing comments
                    prev_end = self.find_end_with_trailing_comments(cp + 1);
                } else {
                    // Fallback: no comma found (shouldn't happen in valid enum)
                    member_parts.push(d.text(","));
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

        d.concat(&parts)
    }

    /// Build doc for a single enum member
    fn build_enum_member_doc(&self, member: &internal::TSEnumMember) -> DocId {
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
            // Comments between name and `=` (e.g., `a /* c */ = 1`)
            let id_doc = if self.has_comments_between(id_end, eq_pos) {
                d.concat(&[
                    id_doc,
                    self.build_inline_comments_between_doc(id_end, eq_pos),
                ])
            } else {
                id_doc
            };
            let rhs_comments = self.build_rhs_comments_opt(eq_pos + 1, init_start);

            // For binary expressions, use assignment layout with indent for wrapping
            // The binary expression already has a group() with line() elements.
            // We just need to add indent around it so continuations are indented.
            if let Some(comments) = rhs_comments {
                if matches!(init, internal::Expression::BinaryExpression(_)) {
                    d.concat(&[id_doc, d.text(" = "), comments, d.indent(init_doc)])
                } else {
                    d.concat(&[id_doc, d.text(" = "), comments, init_doc])
                }
            } else if matches!(init, internal::Expression::BinaryExpression(_)) {
                d.concat(&[id_doc, d.text(" = "), d.indent(init_doc)])
            } else {
                d.concat(&[id_doc, d.text(" = "), init_doc])
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
    ) -> DocId {
        self.build_module_declaration_doc_inner(decl, true)
    }

    /// Inner helper for module declaration doc building
    /// `is_root` is true for the outermost declaration (prints `namespace` keyword)
    fn build_module_declaration_doc_inner(
        &self,
        decl: &internal::TSModuleDeclaration,
        is_root: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Only print keywords for root declaration
        if is_root {
            // `declare` prefix if ambient declaration
            if decl.declare {
                parts.push(d.text("declare "));
            }

            // `global` is special - it replaces namespace/module keyword
            if decl.global {
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
                    // separators, prettier-ignore, trailing same-line comments) —
                    // same as block-statement bodies.
                    let body_start = block.span.start + 1; // After opening '{'
                    let body_end = block.span.end.saturating_sub(1); // Before '}'
                    let (mut stmt_parts, prev_end, _prev_stmt_end) = self
                        .build_statement_list_docs(
                            &block.body,
                            body_start,
                            body_end,
                            Vec::new(),
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
