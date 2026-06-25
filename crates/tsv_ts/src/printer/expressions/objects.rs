// Object expression printing for TypeScript
//
// Handles printing of object expressions with:
// - Width-based wrapping via doc-builder
// - Comment preservation (block and line comments)
// - Property shorthand detection
// - String key normalization (unquote valid identifiers)
// - Blank line preservation between properties

use crate::ast::internal::{self, Expression, Literal, LiteralValue};
use crate::printer::CommentSpacing;
use crate::printer::Printer;
use crate::printer::expressions::literals::format_string_literal_from_ast;
use crate::printer::expressions::literals::is_valid_js_identifier;
use crate::printer::layout::hang_after_operator;
use smallvec::smallvec;
use tsv_lang::SymbolResolver;
use tsv_lang::TAB_WIDTH;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::visual_width;

impl<'a> Printer<'a> {
    /// Build a Doc for an object expression
    ///
    /// Handles comments between properties, blank line preservation, and trailing comments.
    pub(in crate::printer) fn build_object_doc(&self, obj: &internal::ObjectExpression) -> DocId {
        let d = self.d();
        // Check for comments inside the object
        let has_comments = self.has_comments_between(obj.span.start, obj.span.end);

        // Check if object contains line comments or block comments on their own line (force multiline)
        let has_line_comments = self.has_line_comments_between(obj.span.start, obj.span.end);

        // Check for block comments on their own line (not same line as any property)
        let property_spans: Vec<_> = obj
            .properties
            .iter()
            .map(internal::ObjectProperty::span)
            .collect();
        let has_standalone_block_comment =
            self.has_standalone_block_comment(obj.span.start, obj.span.end, &property_spans);

        if obj.properties.is_empty() {
            // Handle empty object with comments
            return self.build_empty_body_with_comments_doc(obj.span);
        }

        // Check if source has newline after opening brace
        let first_prop_start = obj.properties[0].span().start;
        let has_source_newline = self.has_newline_between(obj.span.start + 1, first_prop_start);

        // Check if any property value has multiline content (e.g., line continuation strings)
        // Prettier expands objects containing multiline strings (recursively)
        let has_multiline = obj.properties.iter().any(|prop| match prop {
            internal::ObjectProperty::Property(p) => {
                crate::printer::has_multiline_content(&p.value, self.source)
            }
            internal::ObjectProperty::SpreadElement(s) => {
                crate::printer::has_multiline_content(&s.argument, self.source)
            }
        });

        // Decide the formatting strategy
        // must_break: conditions that require hardlines (comments, multiline content)
        // has_source_newline: prefers expanded, but uses group_break for proper propagation
        let must_break = has_line_comments || has_standalone_block_comment || has_multiline;

        if has_comments || must_break {
            // Comment-aware path
            // Use hardlines when must_break, use line() when only has_comments
            // This allows inline objects with block comments to stay inline if they fit
            let mut parts = DocBuf::new();
            let mut prev_end = obj.span.start + 1; // After opening brace

            // A comment trailing the opening `{` on its own line is kept on the `{`
            // line when the object expands (divergence from prettier, which relocates
            // it to its own line as the first property's leading comment). See
            // conformance_prettier.md §Comment relocation (Object literal `{`).
            let (brace_line_prefix, brace_pull_pos) =
                self.delimiter_line_comment_prefix(obj.span.start, first_prop_start);

            for (i, prop) in obj.properties.iter().enumerate() {
                let prop_start = prop.span().start;
                let is_first = i == 0;

                // Get comments between previous position and this property
                // For non-first properties, start search after the comma (not after property value)
                let search_start = self.leading_comment_search_start(prev_end, is_first);

                // Collect leading comments (search starts after comma for non-first properties)
                // Skip line comments that are on same line as previous property (those are trailing)
                // Block comments after comma on same line are leading
                let comments: Vec<_> = comments_in_range(self.comments, search_start, prop_start)
                    .filter(|c| {
                        // Brace-line comments pulled onto the `{` line above are emitted
                        // as the prefix, not here (only relevant for the first property).
                        if is_first
                            && let Some(dpos) = brace_pull_pos
                            && self.comment_on_delimiter_line(dpos, c)
                        {
                            return false;
                        }
                        is_first ||
                        c.is_block || // Block comments after comma are always leading
                        !self.is_same_line( prev_end, c.span.start) // Line comments must be on different line
                    })
                    .collect();

                // For non-first properties, add separator
                if !is_first {
                    if must_break {
                        // Must break: check for blank line preservation
                        let check_pos = if comments.is_empty() {
                            prop_start
                        } else {
                            comments[0].span.start
                        };
                        if self.has_blank_line_between(search_start, check_pos) {
                            parts.push(d.literalline());
                        }
                        parts.push(d.hardline());
                    } else {
                        // May stay inline: use line() for group-based breaking
                        parts.push(d.line());
                    }
                }

                // Process comments before this property
                let mut last_pos = search_start;
                for (j, comment) in comments.iter().enumerate() {
                    let is_last_comment = j == comments.len() - 1;

                    // Check if there's a blank line after this comment (for must_break mode)
                    let has_blank_after = must_break
                        && if is_last_comment {
                            self.has_blank_line_between(comment.span.end, prop_start)
                        } else {
                            self.has_blank_line_between(
                                comment.span.end,
                                comments[j + 1].span.start,
                            )
                        };

                    // For subsequent comments, check for blank lines between them
                    if must_break
                        && j > 0
                        && self.has_blank_line_between(last_pos, comment.span.start)
                    {
                        parts.push(d.literalline());
                        parts.push(d.hardline());
                    }

                    parts.push(self.build_comment_doc(comment));
                    if !comment.is_block {
                        // Line comments need a hardline after (unless blank line follows in must_break)
                        if !has_blank_after {
                            parts.push(d.hardline());
                        }
                    } else if must_break && !self.is_same_line(comment.span.end, prop_start) {
                        // Block comment on its own line - hardline after (unless blank line follows)
                        if !has_blank_after {
                            parts.push(d.hardline());
                        }
                    } else {
                        // Block comment on same line as property - space after
                        parts.push(d.text(" "));
                    }
                    last_pos = comment.span.end;
                }

                // Check for blank line after last comment (before property)
                if must_break
                    && !comments.is_empty()
                    && self.has_blank_line_between(last_pos, prop_start)
                {
                    parts.push(d.literalline());
                    parts.push(d.hardline());
                }

                // Build property doc — a preceding format-ignore directive keeps the
                // property's source verbatim (trailing comment/comma handled normally)
                let prop_doc = if self.has_format_ignore_in_range(search_start, prop_start) {
                    self.raw_source_doc(prop.span())
                } else {
                    self.build_object_property_doc(prop)
                };
                parts.push(prop_doc);

                // Trailing comments around the separator comma — block comments
                // before the comma, the comma, an after-comma block on the last
                // property preserved in place (`trailingComma: 'none'`), then line
                // comments as a suffix. Shared with the destructuring-pattern
                // builders via `collect_trailing_comments` /
                // `push_element_comma_trailing`.
                let prop_end = prop.value_end();
                let upper_bound = obj
                    .properties
                    .get(i + 1)
                    .map_or(obj.span.end, |next| next.span().start);

                let is_last = i == obj.properties.len() - 1;
                let trailing = self.collect_trailing_comments(prop_end, upper_bound, is_last);
                let comma = if is_last { d.empty() } else { d.text(",") };
                self.push_element_comma_trailing(&mut parts, &trailing, comma);

                prev_end = prop.value_end();
            }

            // Handle trailing comments before closing brace
            let closing_brace_pos = obj.span.end - 1;
            let trailing_comments: Vec<_> =
                comments_in_range(self.comments, prev_end, closing_brace_pos)
                    .filter(|c| !self.is_same_line(prev_end, c.span.start))
                    .collect();

            if !trailing_comments.is_empty() {
                // Check for blank line before the first trailing comment
                let first_comment = trailing_comments[0];
                if must_break && self.has_blank_line_between(prev_end, first_comment.span.start) {
                    parts.push(d.literalline());
                }

                let mut last_pos = prev_end;
                for (j, comment) in trailing_comments.iter().enumerate() {
                    // Check for blank lines between comments
                    if must_break
                        && j > 0
                        && self.has_blank_line_between(last_pos, comment.span.start)
                    {
                        parts.push(d.literalline());
                    }

                    if must_break {
                        parts.push(d.hardline());
                    } else {
                        parts.push(d.line());
                    }
                    parts.push(self.build_comment_doc(comment));
                    last_pos = comment.span.end;
                }
            }

            if must_break {
                // Forced multiline - use hardlines for predictable formatting
                let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
                let (indented_content, closing_line) =
                    self.wrap_with_decl_indent(inner, d.hardline());

                d.concat(&[
                    d.text("{"),
                    d.concat(&brace_line_prefix),
                    indented_content,
                    closing_line,
                    d.text("}"),
                ])
            } else {
                // May stay inline - use group with bracketSpacing boundaries for
                // width-based breaking: a space when flat (`{ foo }`), a newline when
                // it breaks (brace_line_prefix is empty here — pulling implies must_break).
                let inner = d.concat(&[d.line(), d.concat(&parts)]);
                let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.line());

                self.wrap_object_braces(indented_content, closing_line, has_source_newline)
            }
        } else {
            // No comments, no forced multiline: use width-based wrapping with soft lines
            let mut parts = DocBuf::new();

            for (i, prop) in obj.properties.iter().enumerate() {
                // Check for blank line before this property (preserved in multiline).
                let has_blank_before = if i > 0 {
                    let prev_prop = &obj.properties[i - 1];
                    let prev_end = prev_prop.value_end();
                    let prop_start = prop.span().start;
                    self.has_blank_line_after_comma(prev_end, prop_start)
                } else {
                    false
                };

                if has_blank_before {
                    // Blank line preservation
                    parts.push(d.literalline());
                    parts.push(d.hardline());
                }

                // Build property doc
                let prop_doc = self.build_object_property_doc(prop);
                parts.push(prop_doc);

                // Add comma and line break
                if i < obj.properties.len() - 1 {
                    parts.push(d.text(","));
                    // Only add line break if next property doesn't have blank line before it
                    let next_prop = &obj.properties[i + 1];
                    let curr_end = prop.value_end();
                    let next_start = next_prop.span().start;
                    let next_has_blank = self.has_blank_line_after_comma(curr_end, next_start);

                    if !next_has_blank {
                        parts.push(d.line());
                    }
                }
                // No trailing comma on the last property (trailingComma: 'none').
            }

            // Width-based wrapping: bracketSpacing boundaries (space when flat
            // `{ foo }`, newline when broken).
            let inner = d.concat(&[d.line(), d.concat(&parts)]);
            let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.line());

            self.wrap_object_braces(indented_content, closing_line, has_source_newline)
        }
    }

    /// Wrap content in braces with appropriate grouping for object expressions.
    ///
    /// Uses `group_break` when source had newlines (propagates break upward),
    /// otherwise uses `group` for width-based breaking.
    fn wrap_object_braces(
        &self,
        indented_content: DocId,
        closing_line: DocId,
        has_source_newline: bool,
    ) -> DocId {
        let d = self.d();
        let object_doc = d.concat(&[d.text("{"), indented_content, closing_line, d.text("}")]);
        if has_source_newline {
            d.group_break(object_doc)
        } else {
            d.group(object_doc)
        }
    }

    /// Build a Doc for an object expression with forced expansion (hardlines).
    ///
    /// Used by chain arg formatting when we need the object to expand internally
    /// with hardlines so fits() can correctly measure the first line.
    /// Produces: `{\n  prop,\n}` with actual hardlines.
    pub(in crate::printer) fn build_object_doc_expanded(
        &self,
        obj: &internal::ObjectExpression,
    ) -> DocId {
        let d = self.d();
        if obj.properties.is_empty() {
            return d.text("{}");
        }

        let mut parts: DocBuf = DocBuf::new();
        for (i, prop) in obj.properties.iter().enumerate() {
            let prop_doc = self.build_object_property_doc(prop);
            parts.push(prop_doc);

            if i < obj.properties.len() - 1 {
                parts.push(d.text(","));
                parts.push(d.hardline());
            }
            // No trailing comma on the last property under `trailingComma: 'none'`.
        }

        d.concat(&[
            d.text("{"),
            d.indent(d.concat(&[d.hardline(), d.concat(&parts)])),
            d.hardline(),
            d.text("}"),
        ])
    }

    /// Build a Doc for an object property (either Property or SpreadElement)
    fn build_object_property_doc(&self, prop: &internal::ObjectProperty) -> DocId {
        match prop {
            internal::ObjectProperty::Property(p) => self.build_property_doc(p),
            internal::ObjectProperty::SpreadElement(s) => self.build_spread_doc(s),
        }
    }

    /// Build a Doc for a single property
    fn build_property_doc(&self, prop: &internal::Property) -> DocId {
        let d = self.d();
        // For computed keys, use expression doc (preserves string quotes)
        // For regular keys, use property key doc (converts strings to bare identifiers when valid)
        // Track where comments after the key region end (after `]` for computed, after key for normal)
        let key_region_end;
        let key_doc = if prop.computed {
            // Assignment expressions need parens in computed keys: {[(a = b)]: c}
            let key_expr_doc =
                if super::needs_parens(&prop.key, super::ParenContext::ComputedPropertyKey) {
                    d.parens(self.build_expression_doc(&prop.key))
                } else {
                    self.build_expression_doc(&prop.key)
                };
            let (doc, end) =
                self.build_computed_key_bracket_doc(prop.span.start, &prop.key, key_expr_doc);
            key_region_end = end;
            doc
        } else {
            key_region_end = prop.key.span().end;
            self.build_property_key_doc(&prop.key)
        };

        // Add getter/setter prefix if applicable, preserving comments between
        // keyword and name (e.g., `get /* c */ a()`)
        let key_doc = match prop.kind {
            internal::PropertyKind::Get | internal::PropertyKind::Set => {
                let kind_text = if matches!(prop.kind, internal::PropertyKind::Get) {
                    "get "
                } else {
                    "set "
                };
                let mut kw_parts = DocBuf::new();
                self.push_accessor_keyword_doc(
                    &mut kw_parts,
                    kind_text,
                    prop.span.start,
                    prop.key.span().start,
                    prop.computed,
                );
                kw_parts.push(key_doc);
                d.concat(&kw_parts)
            }
            internal::PropertyKind::Init => key_doc,
        };

        // Handle getter/setter vs method vs regular property
        if matches!(
            prop.kind,
            internal::PropertyKind::Get | internal::PropertyKind::Set
        ) {
            // Getter/setter: `get x() {}` or `set x(v) {}`
            if let Expression::FunctionExpression(func) = &prop.value {
                let func_doc = self.build_function_doc_body(func);
                // Comments between key and params: get [x] /* c */() {}
                // Line comments get a hardline to prevent absorbing parens as comment text
                let params_start = func.params_start;
                let comments = self.build_name_to_type_params_comments(
                    key_region_end,
                    params_start,
                    CommentSpacing::Leading,
                );
                d.concat(&[key_doc, comments, func_doc])
            } else {
                key_doc
            }
        } else if prop.method {
            // Method shorthand: `foo() {}`, `async foo() {}`, `*gen() {}`, or `async *gen() {}`
            if let Expression::FunctionExpression(func) = &prop.value {
                let func_doc = self.build_function_doc_body(func);
                // Build prefix: async? + *?, preserving comments after `async`
                // (e.g., `async /* c */ m()`)
                let key_start = prop.key.span().start;
                let mut parts = DocBuf::new();
                let mut cursor = prop.span.start;
                if func.r#async {
                    self.push_member_keyword_doc(&mut parts, "async ", &mut cursor, key_start);
                }
                if func.generator {
                    self.push_generator_star_doc(&mut parts, cursor, key_start, prop.computed);
                } else if func.r#async {
                    // Comments before the name (bounded at `[` for computed keys,
                    // whose inner comments the bracket builder handles)
                    let bound = self.computed_key_name_bound(cursor, key_start, prop.computed);
                    self.push_pre_name_comments_doc(&mut parts, cursor, bound);
                }
                parts.push(key_doc);

                // Handle comments between method name and type params/parameters: foo /* comment */ ()
                // Use key_region_end (after `]` for computed) to avoid re-finding bracket comments
                // Stop at type_params start when present — comments between `>` and `(`
                // are handled by build_function_expression_signature_doc
                // Line comments get a hardline to prevent absorbing type params as comment text
                let comment_search_end = func
                    .type_parameters
                    .as_ref()
                    .map_or(func.params_start, |tp| tp.span.start);
                parts.push(self.build_name_to_type_params_comments(
                    key_region_end,
                    comment_search_end,
                    CommentSpacing::for_type_params(func.type_parameters.is_some()),
                ));

                parts.push(func_doc);
                d.concat(&parts)
            } else {
                // Fallback for malformed AST
                let value_doc = self.build_expression_doc(&prop.value);
                d.concat(&[key_doc, d.text(": "), value_doc])
            }
        } else if prop.shorthand {
            // Handle shorthand with default value: {a = 1}
            // The value is an AssignmentExpression (or AssignmentPattern in proper patterns)
            if let Expression::AssignmentExpression(assign) = &prop.value {
                let default_doc = self.build_expression_doc(&assign.right);
                d.concat(&[key_doc, d.text(" = "), default_doc])
            } else if let Expression::AssignmentPattern(pattern) = &prop.value {
                let default_doc = self.build_expression_doc(&pattern.right);
                d.concat(&[key_doc, d.text(" = "), default_doc])
            } else {
                key_doc
            }
        } else {
            // Regular property: check for comments between key and value
            // Find colon position and check for comments
            // Use key_region_end (after `]` for computed, after key for normal)
            // to avoid double-counting comments already inside brackets
            let colon_pos = self.find_colon_after(key_region_end);
            let value_start = prop.value.span().start;

            // Comments between key region and colon (e.g., {key /* comment */: value})
            let pre_colon_comments: Vec<_> =
                comments_in_range(self.comments, key_region_end, colon_pos).collect();
            // Comments between colon and value (e.g., {key: /* comment */ value})
            let post_colon_comments: Vec<_> =
                comments_in_range(self.comments, colon_pos + 1, value_start).collect();

            // Check if value needs parens (e.g., assignment expressions)
            let needs_parens =
                super::needs_parens(&prop.value, super::ParenContext::ObjectPropertyValue);

            if pre_colon_comments.is_empty() && post_colon_comments.is_empty() {
                if needs_parens {
                    // Build manually with parens
                    let value_doc = d.concat(&[
                        d.text("("),
                        self.build_expression_doc(&prop.value),
                        d.text(")"),
                    ]);
                    d.concat(&[key_doc, d.text(": "), value_doc])
                } else {
                    // No parens needed: use unified assignment layout
                    let is_short_key = self.is_short_property_key(&prop.key, prop.computed);
                    self.build_assignment_layout(key_doc, ":", &prop.value, is_short_key, None)
                }
            } else {
                // Comments around colon: check if any post-colon comment forces a break.
                // Line comments always force break (they extend to end of line).
                // Multiline block comments also force break-after-operator layout.
                // Prettier ref: hasLeadingOwnLineComment → break-after-operator in chooseLayout
                let has_line_comment_post_colon = post_colon_comments.iter().any(|c| !c.is_block);
                let has_multiline_post_colon = has_line_comment_post_colon
                    || self.has_multiline_block_comments_between(colon_pos + 1, value_start);

                if has_multiline_post_colon {
                    // Line comment or multiline block comment after colon: BreakAfterOperator
                    // Structure: group([group(key + pre_colon), ":", group(indent([line, rhs]))])
                    let mut lhs_parts: DocBuf = smallvec![key_doc];
                    for comment in &pre_colon_comments {
                        lhs_parts.push(d.text(" "));
                        lhs_parts.push(self.build_comment_doc(comment));
                    }
                    let lhs_doc = if lhs_parts.len() == 1 {
                        key_doc
                    } else {
                        d.concat(&lhs_parts)
                    };

                    // Build RHS: comments (with proper separators) + value
                    let comments_doc = self
                        .build_rhs_comments_opt(colon_pos + 1, value_start)
                        .unwrap_or_else(|| d.empty());
                    let mut value_parts: DocBuf = smallvec![comments_doc];
                    if needs_parens {
                        value_parts.push(d.text("("));
                    }
                    value_parts.push(self.build_expression_doc(&prop.value));
                    if needs_parens {
                        value_parts.push(d.text(")"));
                    }
                    let rhs_doc = d.concat(&value_parts);

                    // BreakAfterOperator: group([group(left), ":", group(indent([line, rhs]))])
                    d.group(d.concat(&[
                        d.group(lhs_doc),
                        d.text(":"),
                        hang_after_operator(d, rhs_doc),
                    ]))
                } else {
                    // Inline block comments: use assignment layout so choose_layout
                    // applies (e.g., ternary with binaryish test → BreakAfterOperator).
                    // Pre-colon comments become part of the LHS doc.
                    let lhs_doc = if pre_colon_comments.is_empty() {
                        key_doc
                    } else {
                        let mut lhs_parts: DocBuf = smallvec![key_doc];
                        for comment in &pre_colon_comments {
                            lhs_parts.push(d.text(" "));
                            lhs_parts.push(self.build_comment_doc(comment));
                        }
                        d.concat(&lhs_parts)
                    };

                    // Post-colon inline comments become rhs_comments
                    let rhs_comments = if post_colon_comments.is_empty() {
                        None
                    } else {
                        let mut comment_parts: DocBuf = DocBuf::new();
                        for comment in &post_colon_comments {
                            comment_parts.push(self.build_comment_doc(comment));
                            comment_parts.push(d.text(" "));
                        }
                        Some(d.concat(&comment_parts))
                    };

                    if needs_parens {
                        // Rare: assignment expression in object value needs parens
                        let mut parts: DocBuf = smallvec![lhs_doc, d.text(": ")];
                        if let Some(rc) = rhs_comments {
                            parts.push(rc);
                        }
                        parts.push(d.text("("));
                        parts.push(self.build_expression_doc(&prop.value));
                        parts.push(d.text(")"));
                        d.concat(&parts)
                    } else {
                        let is_short_key = self.is_short_property_key(&prop.key, prop.computed);
                        self.build_assignment_layout(
                            lhs_doc,
                            ":",
                            &prop.value,
                            is_short_key,
                            rhs_comments,
                        )
                    }
                }
            }
        }
    }

    /// Check if a property key is "short" for layout decisions.
    ///
    /// Short keys don't benefit from breaking after the colon.
    /// Complex expressions (calls, binary, etc.) are never short - they can't
    /// be reduced to a simple width, matching Prettier's `cleanDoc` behavior.
    ///
    /// Prettier ref: `isObjectPropertyWithShortKey` in print/assignment.js:401
    /// Uses `getStringWidth(cleanDoc(keyDoc)) < tabWidth + MIN_OVERLAP_FOR_BREAK`
    fn is_short_property_key(&self, key: &Expression, computed: bool) -> bool {
        // Prettier: MIN_OVERLAP_FOR_BREAK = 3 (assignment.js:409)
        let threshold = TAB_WIDTH + super::assignment::MIN_OVERLAP_FOR_BREAK;

        let base_width = match key {
            // Prettier: cleanDoc reduces identifier keys to their name string
            Expression::Identifier(id) => {
                self.with_resolved_symbol(id.name, |s| visual_width(s, TAB_WIDTH))
            }
            Expression::Literal(lit) => match &lit.value {
                LiteralValue::String { content, .. } => {
                    // For computed keys, quotes are always preserved: ["x"] prints as ['x']
                    // For non-computed keys, valid identifiers are unquoted: {"x":1} → {x:1}
                    // Escape-bearing keys keep their quotes (see `string_key_unquotes`).
                    if computed || !self.string_key_unquotes(lit, content) {
                        visual_width(content, TAB_WIDTH) + 2 // Include quotes
                    } else {
                        visual_width(content, TAB_WIDTH)
                    }
                }
                LiteralValue::Number(_) => {
                    // Use span to get actual source width
                    (lit.span.end - lit.span.start) as usize
                }
                // Other literals (bool, null, etc.) - rare as keys, not short
                _ => return false,
            },
            // Complex expressions (calls, binary, member, etc.) are never "short".
            // Prettier's cleanDoc can't reduce them to strings, so it returns false.
            _ => return false,
        };

        let total_width = if computed {
            base_width + 2 // Add brackets
        } else {
            base_width
        };

        total_width < threshold
    }

    /// A quoted string key may be unquoted only when its *raw* source (escape
    /// sequences intact) is already a valid identifier. Keys whose raw form
    /// differs from the decoded value carry escapes (`'b'`, `'\a'`,
    /// `'\x66\x69\x73\x6b\x65\x72'`) and keep their quotes so the escapes are
    /// preserved — matching Prettier, which only unquotes when
    /// `rawText.slice(1, -1) === value`. Unquoting from the decoded value would
    /// silently rewrite the source text (data loss).
    pub(in crate::printer) fn string_key_unquotes(&self, lit: &Literal, content: &str) -> bool {
        if !is_valid_js_identifier(content) {
            return false;
        }
        let raw = lit.span.extract(self.source);
        // Strip the surrounding quotes; compare the raw inner text to the
        // decoded value. Equal ⇒ no escapes ⇒ safe to unquote.
        raw.len() >= 2 && raw[1..raw.len() - 1] == *content
    }

    /// Emit a string-literal key with prettier's `quoteProps: as-needed`: drop the
    /// quotes when the raw text is already a valid identifier (`'type'` → `type`),
    /// else keep them and normalize the quote style. Keeping quotes covers
    /// non-identifier keys (`'kebab-case'`) and escape-bearing keys (`'b'`) whose
    /// escapes must be preserved (see [`Self::string_key_unquotes`]). Shared by
    /// object property keys and import-attribute keys.
    pub(in crate::printer) fn build_string_literal_key_doc(
        &self,
        lit: &Literal,
        content: &str,
    ) -> DocId {
        let d = self.d();
        if self.string_key_unquotes(lit, content) {
            d.text_owned(content.to_string())
        } else {
            d.text_owned(format_string_literal_from_ast(lit, self.source))
        }
    }

    /// Build a Doc for a property key
    ///
    /// String literal keys that are valid identifiers are output without quotes.
    /// Example: `{"key": 1}` → `{key: 1}`, but `{"kebab-case": 1}` keeps quotes.
    pub(in crate::printer) fn build_property_key_doc(&self, key: &Expression) -> DocId {
        match key {
            Expression::Literal(
                lit @ Literal {
                    value: LiteralValue::String { content, .. },
                    ..
                },
            ) => self.build_string_literal_key_doc(lit, content),
            _ => self.build_expression_doc(key),
        }
    }

    /// Emit a type-member key (`PropertySignature`/`MethodSignature`), returning
    /// `(doc, key_region_end)` where `key_region_end` is the source offset just
    /// past the key — after the `]` for computed keys — used to anchor the search
    /// for following comments/modifiers.
    ///
    /// `unquote` drops quotes from an identifier-valid string-literal key: `true`
    /// for property signatures (`'plain': T` → `plain: T`), `false` for method
    /// signatures (`'foo'(): void` keeps its quotes — prettier's rule). Computed
    /// keys are always emitted verbatim inside their brackets.
    pub(in crate::printer) fn build_type_member_key_doc(
        &self,
        search_start: u32,
        key: &Expression,
        computed: bool,
        unquote: bool,
    ) -> (DocId, u32) {
        if computed {
            let key_doc = self.build_expression_doc(key);
            self.build_computed_key_bracket_doc(search_start, key, key_doc)
        } else {
            let doc = if unquote {
                self.build_property_key_doc(key)
            } else {
                self.build_expression_doc(key)
            };
            (doc, key.span().end)
        }
    }

    /// Find the position of `:` after a position (for finding colon in property)
    /// Skips over comments to avoid matching colons inside them.
    pub(in crate::printer) fn find_colon_after(&self, start: u32) -> u32 {
        tsv_lang::source_scan::find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            self.source.len(),
            b':',
        )
        .map_or(start, |pos| pos as u32)
    }

    /// Build a `[key]` doc with comments preserved inside brackets.
    /// Returns `(doc, key_region_end)` where key_region_end is the position after `]`.
    /// Used by object properties/methods, class methods/properties, destructuring
    /// patterns, and interface/type-literal members (via `build_type_member_key_doc`).
    ///
    /// `[`→key comment placement: a block comment hugs `[` inline (`[/* c */ foo]`)
    /// and the bracket stays flat; a **line** comment can't sit inline before the
    /// key (a `//` runs to EOL and would swallow it), so it forces the bracket to
    /// break — preserved where the author wrote it (on the `[` line via
    /// `delimiter_line_comment_prefix`, or on its own line) with the key dropped to
    /// an indented continuation. Prettier relocates such a comment (out to the
    /// member's leading line, or glued flush to the key) — a divergence
    /// (conformance_prettier.md §Comment relocation, "Object/array/block
    /// open-delimiter trailing"). A computed key never breaks on width alone
    /// (prettier keeps a long key inline), so the flat, no-line-comment path stays
    /// verbatim — only a `[`→key line comment switches to the breaking layout.
    pub(in crate::printer) fn build_computed_key_bracket_doc(
        &self,
        search_start: u32,
        key: &Expression,
        key_doc: DocId,
    ) -> (DocId, u32) {
        let d = self.d();
        let key_start = key.span().start;
        let key_end = key.span().end;
        let bracket_start = self.find_opening_bracket_after(search_start, key_start);
        let bracket_end = self.find_closing_bracket_after(key_end);

        let bracket_line = self.has_line_comments_between(bracket_start + 1, key_start);
        let after_key_line = self.has_line_comments_between(key_end, bracket_end);

        // Flat path (no line comment in either in-bracket gap): block comments hug
        // inline (`[/* d */ foo]`, `[foo /* c */]`), the key never breaks on width.
        // Byte-identical to the pre-divergence behavior.
        if !bracket_line && !after_key_line {
            let mut parts: DocBuf = smallvec![d.text("[")];
            for comment in comments_in_range(self.comments, bracket_start + 1, key_start) {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            }
            parts.push(key_doc);
            for comment in comments_in_range(self.comments, key_end, bracket_end) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
            parts.push(d.text("]"));
            return (d.concat(&parts), bracket_end + 1);
        }

        // Breaking path: a line comment in either in-bracket gap forces the bracket
        // to break so the `//` can't swallow the key or `]`, preserving each comment
        // in place. `[`→key: a `[`-line comment is pulled onto the `[` line, an
        // own-line one stays on its own line (`build_leading_comments_multiline*`,
        // the shared open-delimiter leading-comment builder, hugging a same-line
        // block to the key). key→`]`: a same-line comment trails the key with a
        // space, an own-line comment keeps its own line. Prettier relocates instead
        // (conformance_prettier.md §Comment relocation).
        let (bracket_line_prefix, bracket_pull_pos) =
            self.delimiter_line_comment_prefix(bracket_start, key_start);
        let mut inner_parts = self.build_leading_comments_multiline_opt(
            bracket_start + 1,
            key_start,
            bracket_pull_pos,
        );
        inner_parts.push(key_doc);
        let mut prev = key_end;
        for comment in comments_in_range(self.comments, key_end, bracket_end) {
            if self.is_same_line(prev, comment.span.start) {
                inner_parts.push(d.text(" "));
            } else {
                inner_parts.push(d.hardline());
            }
            inner_parts.push(self.build_comment_doc(comment));
            prev = comment.span.end;
        }
        let bracket_body = d.concat(&[
            d.text("["),
            d.concat(&bracket_line_prefix),
            d.indent_softline(d.concat(&inner_parts)),
            d.softline(),
            d.text("]"),
        ]);
        (d.group_break(bracket_body), bracket_end + 1)
    }

    /// Find the opening `[` bracket between two positions (for computed properties).
    /// Returns the first `[` found outside comments in the range [start, end).
    pub(in crate::printer) fn find_opening_bracket_after(&self, start: u32, end: u32) -> u32 {
        tsv_lang::source_scan::find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            end as usize,
            b'[',
        )
        .map_or(start, |pos| pos as u32)
    }

    /// Find the closing `]` bracket after a position (for computed properties)
    /// Skips over comments to avoid matching brackets inside them.
    fn find_closing_bracket_after(&self, pos: u32) -> u32 {
        tsv_lang::source_scan::find_char_skipping_comments(
            self.source.as_bytes(),
            pos as usize,
            self.source.len(),
            b']',
        )
        .map_or(pos + 1, |p| p as u32)
    }
}
