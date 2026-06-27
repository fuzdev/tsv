// Class declaration printing for TypeScript

use super::Printer;
use crate::ast::internal;
use crate::printer::CommentSpacing;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::{SymbolToU32, comments_in_range};

/// Printed keyword (with trailing space) for an accessibility modifier.
fn accessibility_keyword(accessibility: &str) -> &'static str {
    match accessibility {
        "private" => "private ",
        "protected" => "protected ",
        _ => "public ",
    }
}

impl<'a> Printer<'a> {
    /// Build a Doc for a class declaration
    #[inline]
    pub(super) fn build_class_declaration_doc(
        &self,
        decl: &internal::ClassDeclaration<'_>,
    ) -> DocId {
        self.build_class_declaration_doc_inner(decl, true)
    }

    /// Build a Doc for a class declaration without decorators
    ///
    /// Used when exporting decorated classes where decorators are printed
    /// before the export keyword.
    #[inline]
    pub(in crate::printer) fn build_class_declaration_without_decorators_doc(
        &self,
        decl: &internal::ClassDeclaration<'_>,
    ) -> DocId {
        self.build_class_declaration_doc_inner(decl, false)
    }

    /// Core implementation for class declaration doc building
    ///
    /// # Arguments
    ///
    /// * `decl` - The class declaration to build a doc for
    /// * `include_decorators` - If true, decorators are included in the output.
    ///   Set to false when decorators are printed separately (e.g., before `export`).
    fn build_class_declaration_doc_inner(
        &self,
        decl: &internal::ClassDeclaration<'_>,
        include_decorators: bool,
    ) -> DocId {
        let d = self.d();

        // Compute heritage positions once (shared with the class-expression printer).
        let positions = self.class_heritage_positions(
            decl.span.start,
            decl.id.as_ref(),
            decl.type_parameters.as_ref(),
            decl.super_class,
            decl.super_type_parameters.as_ref(),
            decl.implements,
        );

        // Determine group mode: structural reasons OR heritage comments
        let has_heritage_comments = positions
            .first_heritage_start
            .is_some_and(|hs| self.has_comments_between(positions.pre_heritage_end, hs))
            || positions.extends_clause_end.is_some_and(|ext_end| {
                !decl.implements.is_empty()
                    && self.has_comments_between(ext_end, decl.implements[0].span.start)
            });
        let group_mode = self.should_class_group_mode(
            decl.super_class,
            decl.super_type_parameters.as_ref(),
            decl.implements,
        ) || has_heritage_comments;

        // Check if heritage line comments force group break.
        // Line comments consume the rest of the line, so heritage must break to new lines.
        // We use group_break() instead of break_parent to avoid polluting fits() lookahead
        // for nested groups (e.g., type params `<T>` would break unnecessarily).
        let has_heritage_line_comments =
            positions
                .first_heritage_start
                .is_some_and(|heritage_start| {
                    self.has_line_comments_between(positions.pre_heritage_end, heritage_start)
                })
                || positions.extends_clause_end.is_some_and(|ext_end| {
                    !decl.implements.is_empty()
                        && self.has_line_comments_between(ext_end, decl.implements[0].span.start)
                });

        let mut parts = smallvec![];

        // Decorators, each on its own line
        // Find the first keyword after decorators (declare/abstract/class)
        let first_keyword = if decl.declare {
            "declare"
        } else if decl.r#abstract {
            "abstract"
        } else {
            "class"
        };
        let keyword_start =
            self.find_keyword_after_decorators(decl.decorators, first_keyword, decl.span.start);

        if include_decorators
            && let Some(dec_doc) = self.build_decorators_doc(decl.decorators, keyword_start)
        {
            parts.push(dec_doc);
        }

        // Emit modifiers with comments preserved between each keyword pair
        // e.g., `abstract/* b */class B` → `abstract /* b */ class B`
        let search_end = decl.id.as_ref().map_or(decl.span.end, |id| id.span.start);
        let mut cursor = keyword_start;

        if decl.declare {
            parts.push(d.text("declare"));
            cursor = keyword_start + "declare".len() as u32;
        }
        if decl.r#abstract {
            // Find "abstract" in source after cursor, skipping comments
            let abstract_pos = self.find_keyword_in_range(cursor, search_end, "abstract");
            if let Some(ap) = abstract_pos {
                if let Some(c) = self.build_inline_comments_between_doc_opt(cursor, ap) {
                    parts.push(c);
                }
                if cursor > keyword_start {
                    parts.push(d.text(" "));
                }
                parts.push(d.text("abstract"));
                cursor = ap + "abstract".len() as u32;
            }
        }
        // Find "class" in source after cursor, skipping comments
        let class_pos = self.find_keyword_in_range(cursor, search_end, "class");
        if let Some(cp) = class_pos {
            if let Some(c) = self.build_inline_comments_between_doc_opt(cursor, cp) {
                parts.push(c);
            }
            if cursor > keyword_start {
                parts.push(d.text(" "));
            }
            parts.push(d.text("class"));
            cursor = cp + "class".len() as u32;
        }

        // Build heritage docs (shared with the class-expression printer).
        let extends_doc = self.build_class_extends_doc(
            decl.super_class,
            decl.super_type_parameters.as_ref(),
            positions.extends_keyword_start,
        );
        let implements_doc = self.build_class_implements_doc(
            decl.implements,
            group_mode,
            positions.implements_keyword_start,
        );

        if let Some(id) = &decl.id {
            // Named: collect the name + type params + heritage + body into one
            // continuation so a *line* comment in the `class`→name gap indents the
            // whole declaration one level (uniform declaration-header rule). Block
            // and no-comment cases stay inline.
            let mut header_parts = smallvec![d.symbol(id.name.to_u32())];
            // Comments between name and type params: `class A/* c */ <T> {}`
            // Line comments get a hardline to prevent absorbing type params as comment text
            if let Some(type_params) = &decl.type_parameters {
                header_parts.push(self.build_name_to_type_params_comments(
                    id.span.end,
                    type_params.span.start,
                    CommentSpacing::Trailing,
                ));
                // Type params get their own group - break independently of heritage.
                header_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
            }
            let header_doc = self.build_class_header_doc(
                header_parts,
                &positions,
                extends_doc,
                implements_doc,
                decl.implements,
                decl.body.body.is_empty(),
                decl.body.span.start,
                group_mode,
                has_heritage_line_comments,
                true,
            );
            let continuation = d.concat(&[
                header_doc,
                self.build_class_body_doc(&decl.body, decl.declare),
            ]);
            parts.push(self.build_keyword_to_name_continuation(
                cursor,
                id.span.start,
                continuation,
            ));
            return d.concat(&parts);
        }

        // Anonymous class declaration (`export default class {}`): the keyword→body
        // / →heritage gap is handled by the header builder, unchanged.
        if let Some(type_params) = &decl.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }
        let header_doc = self.build_class_header_doc(
            parts,
            &positions,
            extends_doc,
            implements_doc,
            decl.implements,
            decl.body.body.is_empty(),
            decl.body.span.start,
            group_mode,
            has_heritage_line_comments,
            true,
        );

        d.concat(&[
            header_doc,
            self.build_class_body_doc(&decl.body, decl.declare),
        ])
    }

    /// Build a Doc for a class body
    ///
    /// Handles comments between members, blank line preservation, and trailing comments.
    pub(in crate::printer) fn build_class_body_doc(
        &self,
        body: &internal::ClassBody<'_>,
        _is_ambient: bool,
    ) -> DocId {
        let d = self.d();
        if body.body.is_empty() {
            return self.build_empty_body_with_comments_doc(body.span);
        }

        // A comment trailing the opening `{` on its own line is kept on the `{`
        // line when the body expands (divergence from prettier, which relocates
        // it to its own line as the first member's leading comment). Same
        // mechanism as block/namespace bodies. See conformance_prettier.md
        // §Comment relocation (Class/interface/enum body `{`).
        let first_member_start = body.body[0].span().start;
        let (brace_line_prefix, delimiter_pull_pos) =
            self.delimiter_line_comment_prefix(body.span.start, first_member_start);

        // Build member docs with comments and blank line preservation
        let mut member_parts = DocBuf::new();
        let mut prev_end = body.span.start + 1; // Start after '{'

        for (i, member) in body.body.iter().enumerate() {
            let member_start = member.span().start;
            let is_first = i == 0;

            // Check for comments between previous position and this member
            // Filter out trailing same-line comments from the previous member
            let all_comments: Vec<_> =
                comments_in_range(self.comments, prev_end, member_start).collect();
            let comments: Vec<_> = if !is_first {
                all_comments
                    .iter()
                    .filter(|c| !self.is_same_line(prev_end, c.span.start))
                    .copied()
                    .collect()
            } else {
                // First member: drop comments pulled onto the `{` line (emitted
                // as the brace-line prefix below).
                self.first_member_leading_comments(all_comments, delimiter_pull_pos)
            };

            // For non-first members, determine if we need blank line preservation
            // We either add: hardline (no blank) or literalline + hardline (blank line)
            if !is_first {
                let check_pos = if comments.is_empty() {
                    member_start
                } else {
                    comments[0].span.start
                };
                if self.has_blank_line_between(prev_end, check_pos) {
                    // Blank line before first comment or member
                    member_parts.push(d.literalline());
                }
                member_parts.push(d.hardline());
            }

            // Process comments before this member (with blank line preservation)
            member_parts
                .extend(self.build_leading_comments_with_blank_lines(&comments, member_start));

            // A preceding format-ignore directive keeps the member's source verbatim.
            // The member span includes its trailing `;`.
            let member_doc = if self.has_format_ignore_in_range(prev_end, member_start) {
                self.raw_source_doc(member.span())
            } else {
                self.build_class_member_doc(member)
            };
            member_parts.push(member_doc);

            // Handle trailing inline comments on same line after member
            let upper_bound = body
                .body
                .get(i + 1)
                .map_or(body.span.end, |next| next.span().start);
            member_parts
                .extend(self.build_trailing_same_line_comment_docs(member.span().end, upper_bound));

            // Update prev_end past trailing comments (including comments on the
            // closing */ line of multi-line block comments)
            prev_end = self.find_end_with_trailing_comments(member.span().end);
        }

        // Handle trailing comments after the last member (before closing `}`)
        let body_end = body.span.end.saturating_sub(1); // Before '}'
        member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));

        // Wrap body content in indent
        d.concat(&[
            d.text("{"),
            d.concat(&brace_line_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&member_parts)])),
            d.hardline(),
            d.text("}"),
        ])
    }

    /// Build a Doc for a class member
    fn build_class_member_doc(&self, member: &internal::ClassMember<'_>) -> DocId {
        match member {
            internal::ClassMember::MethodDefinition(method) => {
                self.build_method_definition_doc(method)
            }
            internal::ClassMember::PropertyDefinition(prop) => {
                self.build_property_definition_doc(prop)
            }
            internal::ClassMember::StaticBlock(block) => self.build_static_block_doc(block),
            internal::ClassMember::IndexSignature(sig) => self.build_index_signature_doc(sig),
        }
    }

    /// Build a Doc for a class index signature: `[key: Type]: ValueType;`.
    /// Delegates to the shared `build_index_signature_member_doc` (which handles
    /// the `static`/`readonly` modifiers and every in-bracket comment gap) and
    /// appends the trailing `;`, matching the interface caller.
    fn build_index_signature_doc(&self, sig: &internal::TSIndexSignature<'_>) -> DocId {
        let d = self.d();
        d.concat(&[self.build_index_signature_member_doc(sig), d.text(";")])
    }

    /// Build a Doc for a static initialization block
    fn build_static_block_doc(&self, block: &internal::StaticBlock<'_>) -> DocId {
        let d = self.d();
        // Create a BlockStatement wrapper to reuse existing doc building logic
        let block_stmt = internal::BlockStatement {
            body: block.body,
            span: block.span,
        };
        d.concat(&[
            d.text("static "),
            self.build_block_statement_doc(&block_stmt),
        ])
    }

    /// Build a Doc for a property definition
    fn build_property_definition_doc(&self, prop: &internal::PropertyDefinition<'_>) -> DocId {
        let d = self.d();
        let mut parts = smallvec![];

        // Decorators (inline or own-line depending on original source)
        let next_token_start = prop
            .decorators
            .as_ref()
            .and_then(|decs| decs.last())
            .map_or(prop.span.start, |dec| {
                self.find_first_token_after(dec.span.end)
            });
        if let Some(dec_doc) =
            self.build_class_member_decorators_doc(prop.decorators, next_token_start)
        {
            parts.push(dec_doc);
        }

        // Modifier keywords, preserving comments between them and before the
        // name (e.g., `static /* c */ readonly p`). `cursor` tracks the scan
        // position so each comment is emitted at the user's placement.
        let key_start = prop.key.span().start;
        let mut cursor = next_token_start;

        // Declare modifier (comes first, before accessibility)
        if prop.declare {
            self.push_member_keyword_doc(&mut parts, "declare ", &mut cursor, key_start);
        }

        // Accessibility modifier
        if let Some(accessibility) = &prop.accessibility {
            let kind_text = accessibility_keyword(accessibility.as_str());
            self.push_member_keyword_doc(&mut parts, kind_text, &mut cursor, key_start);
        }

        // Static modifier
        if prop.is_static {
            self.push_member_keyword_doc(&mut parts, "static ", &mut cursor, key_start);
        }

        // Override modifier
        if prop.r#override {
            self.push_member_keyword_doc(&mut parts, "override ", &mut cursor, key_start);
        }

        // Abstract modifier
        if prop.r#abstract {
            self.push_member_keyword_doc(&mut parts, "abstract ", &mut cursor, key_start);
        }

        // Readonly modifier
        if prop.readonly {
            self.push_member_keyword_doc(&mut parts, "readonly ", &mut cursor, key_start);
        }

        // Accessor keyword
        if prop.accessor {
            self.push_member_keyword_doc(&mut parts, "accessor ", &mut cursor, key_start);
        }

        // Key (track key_region_end to avoid double-counting comments inside brackets)
        let key_region_end;
        if prop.computed {
            // Comments before the `[` (inside-bracket comments are handled by
            // the bracket builder)
            let bracket_pos = tsv_lang::source_scan::find_char_skipping_comments(
                self.source.as_bytes(),
                cursor as usize,
                key_start as usize,
                b'[',
            )
            .map_or(key_start, |p| p as u32);
            self.push_pre_name_comments_doc(&mut parts, cursor, bracket_pos);
            // Parenthesize an `in` computed key inside a for-header init
            // (`for (C = class { [(b in c)]() {} };…)`); a no-op elsewhere. The
            // object computed-key path applies this via `needs_parens`; class
            // members build the key directly, so apply it here.
            let key_doc = self.wrap_for_init_in(&prop.key, self.build_expression_doc(&prop.key));
            let (doc, end) = self.build_computed_key_bracket_doc(cursor, &prop.key, key_doc);
            key_region_end = end;
            parts.push(doc);
        } else {
            self.push_pre_name_comments_doc(&mut parts, cursor, key_start);
            key_region_end = prop.key.span().end;
            parts.push(self.build_expression_doc(&prop.key));
        }

        // Optional/definite modifier after key, with comment extraction
        let after_modifier = if prop.modifier != internal::PropertyModifier::None {
            let modifier_char = match prop.modifier {
                internal::PropertyModifier::Optional => b'?',
                internal::PropertyModifier::Definite => b'!',
                internal::PropertyModifier::None => unreachable!(),
            };
            // Comments between key and modifier (e.g., `a /* c */? = 1;`)
            self.push_modifier_marker_doc(&mut parts, key_region_end, modifier_char)
        } else {
            key_region_end
        };

        // Type annotation - width-aware wrapping for generics and union types,
        // handling a before-`:` comment between the modifier (or key) and `:`
        // (`c! /* c */ : number`) — line → indented continuation, block → inline.
        if let Some(type_ann) = &prop.type_annotation {
            parts.push(self.build_binding_type_annotation_doc(after_modifier, type_ann, true));
        }

        // Value if present - use assignment layout (matches prettier's printAssignment)
        if let Some(value) = &prop.value {
            let before_eq = prop
                .type_annotation
                .as_ref()
                .map_or(after_modifier, |ta| ta.span.end);
            let value_start = value.span().start;
            let eq_pos = self.find_equals_position(before_eq, value_start);

            // A line comment between the LHS and `=` keeps the comment in place and
            // drops `= value` to a continuation line indented one level (preserve —
            // lossless when a second comment also trails the member; prettier relocates
            // it to end-of-line and merges the two — conformance_prettier.md §Comment
            // relocation). Bypasses the assignment layout; value built lazily so the
            // common no-comment path is unaffected.
            let preserve = self.build_initializer_line_continuation(before_eq, eq_pos, || {
                let value_doc = if self.needs_parens(value, super::ParenContext::DefaultValue) {
                    d.parens(self.build_expression_doc(value))
                } else {
                    self.build_expression_doc(value)
                };
                self.prepend_rhs_comments(value_doc, eq_pos + 1, value_start)
            });
            if let Some(cont) = preserve {
                parts.push(cont);
            } else {
                // Comments before `=` stay before `=` (e.g., `b /* c */ = 1;`)
                if self.has_comments_between(before_eq, eq_pos) {
                    parts.push(self.build_inline_comments_between_doc(before_eq, eq_pos));
                }

                // Comments after `=`
                if self.has_line_comments_between(eq_pos + 1, value_start) {
                    // A same-line comment stays inline with `=` (line comment via
                    // `line_suffix`, so its width never force-breaks a preceding type
                    // union); own-line comments stay on their own lines (not merged);
                    // the value is indented on the next line. `= // comment\n      c`.
                    parts.push(d.text(" ="));
                    let expr_doc = self.build_expression_doc(value);
                    self.append_keyword_value_line_comments(
                        &mut parts,
                        eq_pos + 1,
                        value_start,
                        expr_doc,
                    );
                } else {
                    // Use assignment layout for proper line-breaking (handles
                    // both no-comment and inline block comment cases).
                    // Inline block comments are passed as rhs_comments so
                    // choose_layout still applies (e.g., ternary with binaryish
                    // test → BreakAfterOperator).
                    let rhs_comments = self.build_rhs_comments_opt(eq_pos + 1, value_start);
                    let left_doc = d.concat(&parts);
                    // An assignment value keeps its parens (`a = (this.a = b);`) —
                    // built manually like object property values, since the layout
                    // chooser takes the bare expression
                    let assignment_doc =
                        if self.needs_parens(value, super::ParenContext::DefaultValue) {
                            let value_doc = d.parens(self.build_expression_doc(value));
                            let value_doc = match rhs_comments {
                                Some(comments_doc) => d.concat(&[comments_doc, value_doc]),
                                None => value_doc,
                            };
                            d.concat(&[left_doc, d.text(" = "), value_doc])
                        } else {
                            self.build_assignment_layout(left_doc, " =", value, false, rhs_comments)
                        };
                    parts = smallvec![assignment_doc];
                }
            }
        }

        // Comments between last content and `;`, with the `;` bound to the member: a
        // same-line block stays inline before it, a same-line line trails after it via
        // `line_suffix`, an own-line comment drops to its own line after it (emitting a
        // line comment before the `;` would swallow it). See `split_separator_gap_comments`.
        let content_end = prop
            .value
            .as_ref()
            .map(|v| v.span().end)
            .or_else(|| prop.type_annotation.as_ref().map(|ta| ta.span.end))
            .unwrap_or(after_modifier);
        let after = self.split_separator_gap_comments(&mut parts, content_end, prop.span.end);
        parts.push(d.text(";"));
        parts.extend(after);

        d.concat(&parts)
    }

    /// Build a Doc for a method definition
    fn build_method_definition_doc(&self, method: &internal::MethodDefinition<'_>) -> DocId {
        let d = self.d();
        let mut parts = smallvec![];

        // Decorators (inline or own-line depending on original source)
        let next_token_start = method
            .decorators
            .as_ref()
            .and_then(|decs| decs.last())
            .map_or(method.span.start, |dec| {
                self.find_first_token_after(dec.span.end)
            });
        if let Some(dec_doc) =
            self.build_class_member_decorators_doc(method.decorators, next_token_start)
        {
            parts.push(dec_doc);
        }

        // Modifier keywords, preserving comments between them and before the
        // name (e.g., `static /* c */ async m()`). `cursor` tracks the scan
        // position so each comment is emitted at the user's placement.
        let key_start = method.key.span().start;
        let mut cursor = next_token_start;

        // Accessibility modifier
        if let Some(accessibility) = &method.accessibility {
            let kind_text = accessibility_keyword(accessibility.as_str());
            self.push_member_keyword_doc(&mut parts, kind_text, &mut cursor, key_start);
        }

        // Static modifier
        if method.is_static {
            self.push_member_keyword_doc(&mut parts, "static ", &mut cursor, key_start);
        }

        // Override modifier
        if method.r#override {
            self.push_member_keyword_doc(&mut parts, "override ", &mut cursor, key_start);
        }

        // Abstract modifier
        if method.r#abstract {
            self.push_member_keyword_doc(&mut parts, "abstract ", &mut cursor, key_start);
        }

        // Async modifier
        if method.value.r#async {
            self.push_member_keyword_doc(&mut parts, "async ", &mut cursor, key_start);
        }

        // Generator marker (owns the `*` and comment handling around it)
        if method.value.generator {
            self.push_generator_star_doc(&mut parts, cursor, key_start, method.computed);
        }

        // Get/set for accessors
        match method.kind {
            internal::MethodKind::Get => {
                self.push_member_keyword_doc(&mut parts, "get ", &mut cursor, key_start);
            }
            internal::MethodKind::Set => {
                self.push_member_keyword_doc(&mut parts, "set ", &mut cursor, key_start);
            }
            _ => {}
        }

        // Key
        let key_region_end;
        if method.computed {
            // Comments before the `[` (inside-bracket comments are handled by
            // the bracket builder); generators handle this span after the `*`.
            if !method.value.generator {
                let bracket_pos = tsv_lang::source_scan::find_char_skipping_comments(
                    self.source.as_bytes(),
                    cursor as usize,
                    key_start as usize,
                    b'[',
                )
                .map_or(key_start, |p| p as u32);
                self.push_pre_name_comments_doc(&mut parts, cursor, bracket_pos);
            }
            // Parenthesize an `in` computed key inside a for-header init (mirrors
            // the object computed-key `needs_parens` wrap); a no-op elsewhere.
            let key_doc =
                self.wrap_for_init_in(&method.key, self.build_expression_doc(&method.key));
            let (doc, end) = self.build_computed_key_bracket_doc(cursor, &method.key, key_doc);
            key_region_end = end;
            parts.push(doc);
        } else {
            if !method.value.generator {
                self.push_pre_name_comments_doc(&mut parts, cursor, key_start);
            }
            key_region_end = method.key.span().end;
            parts.push(self.build_expression_doc(&method.key));
        }

        // Optional marker: `m?()` (abstract / ambient / interface methods),
        // preserving comments between name and `?` (e.g., `m /* c */?()`)
        let after_key = if method.optional {
            self.push_modifier_marker_doc(&mut parts, key_region_end, b'?')
        } else {
            key_region_end
        };

        // Comments between key/`?` and next token: [x] /* c */() or method /* c */ <T>()
        // Line comments get a hardline to prevent absorbing type params as comment text
        let next_after_key = method
            .value
            .type_parameters
            .as_ref()
            .map_or(method.value.params_start, |tp| tp.span.start);
        parts.push(self.build_name_to_type_params_comments(
            after_key,
            next_after_key,
            CommentSpacing::for_type_params(method.value.type_parameters.is_some()),
        ));

        // Type parameters if present: method<T>()
        if let Some(type_params) = &method.value.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));

            // Comments between type_params `>` and `(` go after type_params
            if let Some(pp) = tsv_lang::source_scan::find_char_skipping_comments(
                self.source.as_bytes(),
                type_params.span.end as usize,
                self.source.len(),
                b'(',
            ) {
                self.append_type_params_to_paren_comments(
                    &mut parts,
                    type_params.span.end,
                    pp as u32,
                );
            }
        }

        // Parameters and return type - shared callable-signature builder (same path
        // as function declarations; MethodDefinition.value is field-identical).
        parts.push(self.build_callable_signature_doc(
            method.value.params,
            method.value.type_parameters.as_ref(),
            method.value.return_type.as_ref(),
            method.value.params_start,
            method.value.body.span.start,
        ));

        // Overload signatures have empty body (start == end)
        let is_overload_signature = method.value.body.span.start == method.value.body.span.end;

        // For abstract methods or overload signatures, use semicolon instead of body
        if method.r#abstract || is_overload_signature {
            // Comments between return type (or params) and `;`
            self.append_signature_end_comments(
                &mut parts,
                method.value.return_type.as_ref(),
                Some(method.value.params_start),
                method.span.end,
            );
            parts.push(d.text(";"));
        } else {
            let sig_end = if let Some(rt) = &method.value.return_type {
                rt.span.end
            } else if let Some(paren) =
                self.find_closing_paren(method.value.params_start, method.value.body.span.start)
            {
                paren
            } else {
                method.value.body.span.start
            };
            self.append_body_with_sig_comments(&mut parts, sig_end, &method.value.body);
        }

        d.concat(&parts)
    }
}
