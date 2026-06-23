// Destructuring pattern printing for TypeScript
//
// This module handles all destructuring patterns:
// - Object patterns: `{a, b}` with width-based expansion
// - Array patterns: `[a, b]`
// - Assignment patterns: `a = 1`
// - Assignment expressions: `a = b` with width-based wrapping and chain detection
// - Rest elements: `...rest`

use crate::ast::internal::{self, ArrowFunctionBody, Expression, ObjectPatternProperty};
use crate::printer::CommentSpacing;
use crate::printer::{
    ParenContext, PatternContext, Printer, needs_parens, object_pattern_should_expand,
};
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::DocId;

/// Context for assignment expression printing (chain detection)
///
/// Matches prettier's `path.match()` logic for determining when to use chain formatting.
/// Chain formatting is ONLY used when the parent is another assignment expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssignmentContext {
    /// Default context - parent is unknown or non-assignment
    None,
    /// Parent is ExpressionStatement or VariableDeclaration
    /// → Do NOT use chain formatting (use regular grouped layout)
    TopLevel,
    /// Parent is an assignment expression
    /// → Use chain formatting (ungrouped with line elements)
    Chain,
}

/// Check if an arrow function has a nested arrow function as its body
/// Used for chain-tail-arrow-chain detection: `(x) => (y) => x + y`
fn is_nested_arrow_function(expr: &Expression) -> bool {
    if let Expression::ArrowFunctionExpression(arrow) = expr
        && let ArrowFunctionBody::Expression(body_expr) = &arrow.body
    {
        return matches!(body_expr.as_ref(), Expression::ArrowFunctionExpression(_));
    }
    false
}

/// Build chain formatting doc: [group(left), op, ...right parts]
fn build_chain_doc(
    d: &tsv_lang::doc::arena::DocArena,
    left_doc: DocId,
    operator: &'static str,
    right_doc: DocId,
    is_tail: bool,
    is_arrow_chain: bool,
) -> DocId {
    let mut parts = vec![d.group(left_doc), d.text(operator)];

    if is_tail {
        if is_arrow_chain {
            // Chain-tail-arrow-chain: (x) => (y) => x + y
            parts.push(d.text(" "));
            parts.push(right_doc);
        } else {
            // Standard chain tail: indent the final value
            parts.push(d.indent_line(right_doc));
        }
    } else {
        // Chain middle: soft line break, no indent
        parts.push(d.line());
        parts.push(right_doc);
    }

    d.concat(&parts)
}

impl<'a> Printer<'a> {
    /// Build a Doc for an assignment expression
    pub(super) fn build_assignment_doc(&self, assign: &internal::AssignmentExpression) -> DocId {
        // Determine initial context based on whether we're at top level
        let initial_context = if self.in_top_level_assignment.get() {
            AssignmentContext::TopLevel
        } else {
            AssignmentContext::None
        };
        self.build_assignment_doc_with_context(assign, initial_context)
    }

    /// Build a Doc for an assignment expression with chain context
    fn build_assignment_doc_with_context(
        &self,
        assign: &internal::AssignmentExpression,
        context: AssignmentContext,
    ) -> DocId {
        let d = self.d();
        let rhs_is_assignment =
            matches!(assign.right.as_ref(), Expression::AssignmentExpression(_));
        let left_doc = self.build_expression_doc(&assign.left);

        // Extract inline comments between operator and RHS
        // Uses line-comment-safe spacing: block comments get trailing space,
        // line comments get hardline to prevent content absorption.
        let rhs_comment_start = assign.left.span().end;
        let rhs_comment_end = assign.right.span().start;

        // Promote comments that appear before the operator to the LHS.
        // e.g., `a /* comment */ = b` → comment stays before `=`, not after.
        let (left_doc, effective_rhs_start) = if let Some((promoted, new_start)) = self
            .promote_comments_before_operator(
                rhs_comment_start,
                rhs_comment_end,
                assign.operator.as_str(),
            ) {
            (d.concat(&[left_doc, promoted]), new_start)
        } else {
            (left_doc, rhs_comment_start)
        };

        let rhs_has_line_comment =
            self.has_line_comments_between(effective_rhs_start, rhs_comment_end);
        let rhs_comments = self.build_rhs_comments_opt(effective_rhs_start, rhs_comment_end);

        // For 2-segment chains at top level (a = b = value), use unified assignment layout.
        // Prettier only uses chain formatting for 3+ segments (assignment.js:113-125).
        // A 2-segment chain has rhs_is_assignment=true but the inner RHS is NOT an assignment.
        if !matches!(context, AssignmentContext::Chain)
            && rhs_is_assignment
            && !matches!(assign.left.as_ref(), Expression::ObjectPattern(_))
            && let Expression::AssignmentExpression(inner) = assign.right.as_ref()
        {
            let inner_rhs_is_assignment =
                matches!(inner.right.as_ref(), Expression::AssignmentExpression(_));
            if !inner_rhs_is_assignment {
                return self.build_assignment_layout_with_line_comment(
                    left_doc,
                    assign.operator.as_str_with_leading_space(),
                    &assign.right,
                    false,
                    rhs_comments,
                    rhs_has_line_comment,
                    Some(assign.span.end),
                );
            }
        }

        // Use unified assignment layout for simple (non-chain, non-pattern) cases.
        // build_assignment_layout builds right_doc internally and handles rhs_comments.
        if !matches!(context, AssignmentContext::Chain)
            && !matches!(assign.left.as_ref(), Expression::ObjectPattern(_))
            && !rhs_is_assignment
        {
            return self.build_assignment_layout_with_line_comment(
                left_doc,
                assign.operator.as_str_with_leading_space(),
                &assign.right,
                false,
                rhs_comments,
                rhs_has_line_comment,
                Some(assign.span.end),
            );
        }

        // Build right doc for paths that handle layout directly
        let right_doc = if let Expression::AssignmentExpression(rhs_assign) = assign.right.as_ref()
        {
            self.build_assignment_doc_with_context(rhs_assign, AssignmentContext::Chain)
        } else {
            self.build_expression_doc_with_paren_comments(&assign.right, assign.span.end)
        };

        // Prepend inline comments to right doc if present
        let right_doc = if let Some(comments_doc) = rhs_comments {
            d.concat(&[comments_doc, right_doc])
        } else {
            right_doc
        };

        if matches!(context, AssignmentContext::Chain) {
            // Chain formatting - parent is an assignment
            let is_tail = !rhs_is_assignment;
            let is_arrow_chain = is_tail && is_nested_arrow_function(assign.right.as_ref());
            build_chain_doc(
                d,
                left_doc,
                assign.operator.as_str_with_leading_space(),
                right_doc,
                is_tail,
                is_arrow_chain,
            )
        } else if matches!(assign.left.as_ref(), Expression::ObjectPattern(_)) {
            // Object patterns on LHS - never break after operator
            d.concat(&[
                left_doc,
                d.text(assign.operator.as_str_with_leading_space()),
                d.text(" "),
                right_doc,
            ])
        } else {
            // RHS is a chain - group + indent (chain formatting from recursive call)
            d.group(d.concat(&[
                left_doc,
                d.text(assign.operator.as_str_with_leading_space()),
                d.indent_line(right_doc),
            ]))
        }
    }

    /// Build a Doc for an object pattern
    ///
    /// Prettier expands object patterns when:
    /// 1. Any property has a nested pattern value (always expand)
    /// 2. The pattern exceeds print width (width-based expansion)
    pub(super) fn build_object_pattern_doc(&self, obj: &internal::ObjectPattern) -> DocId {
        self.build_object_pattern_doc_with_context(obj, PatternContext::Standalone)
    }

    /// Build object pattern doc with explicit context
    pub(super) fn build_object_pattern_doc_with_context(
        &self,
        obj: &internal::ObjectPattern,
        context: PatternContext,
    ) -> DocId {
        let d = self.d();
        if obj.properties.is_empty() {
            self.build_empty_object_pattern_doc(obj)
        } else {
            // Expand if: nested patterns, line comments, blank lines, or own-line
            // block comments between/around properties
            let should_expand = object_pattern_should_expand(obj, context);
            let (has_line_comments, has_blank_lines) = self.object_pattern_formatting_hints(obj);
            let has_own_line_block = self.object_pattern_has_own_line_block_comments(obj);

            if should_expand || has_line_comments || has_blank_lines || has_own_line_block {
                self.build_expanded_object_pattern_doc(obj)
            } else {
                // Use group with line breaks for width-based expansion
                // Include type annotation in the group so its width is considered
                let mut parts = Vec::new();

                // Track previous end for comment detection (start after `{`)
                let mut prev_end = obj.span.start + 1;

                for (i, prop) in obj.properties.iter().enumerate() {
                    // Check for leading comments before this property
                    let prop_start = prop.span().start;
                    let leading_comments =
                        self.build_inline_comments_between_doc_trailing_space(prev_end, prop_start);
                    parts.push(leading_comments);

                    parts.push(self.build_object_pattern_property_doc(prop));

                    let prop_end = prop.span().end;
                    let is_last = i == obj.properties.len() - 1;

                    // Collect trailing comments (stop at next property or type annotation)
                    let upper_bound = obj
                        .properties
                        .get(i + 1)
                        .map(|next| next.span().start)
                        .or_else(|| obj.type_annotation.as_ref().map(|t| t.span.start))
                        .unwrap_or(obj.span.end);
                    let trailing = self.collect_trailing_comments(prop_end, upper_bound, is_last);

                    // Separator comma between properties; no trailing comma on the last
                    // property (trailingComma: 'none').
                    let comma = if !is_last { d.text(",") } else { d.empty() };
                    self.push_element_comma_trailing(&mut parts, &trailing, comma);

                    // Add line break between properties
                    if !is_last {
                        parts.push(d.line());
                    }

                    prev_end = trailing.end_pos;
                }

                // Check for trailing comments after last property (before closing brace)
                // e.g., `{a /*, b*/}`
                let trailing = self.build_object_pattern_trailing_comments(obj);
                parts.push(trailing);

                // Build group contents: { + properties + } with bracketSpacing
                // boundaries (space when flat `{ a }`, newline when broken).
                let mut group_parts = vec![
                    d.text("{"),
                    d.indent_line(d.concat(&parts)),
                    d.line(),
                    d.text("}"),
                ];

                // Include type annotation in the group for width calculation
                if let Some(type_annotation) = &obj.type_annotation {
                    group_parts.push(self.build_type_annotation_doc(type_annotation));
                }

                d.group(d.concat(&group_parts))
            }
        }
    }

    /// Build trailing comments doc for the inline (single-line) object pattern
    /// path (between last property and `}`), e.g. `{a /*, b*/}`.
    ///
    /// Only captures comments on NEW lines (not same-line trailing comments,
    /// which are handled in the main loop). The expanded paths use
    /// `build_pattern_trailing_dangling_comments` instead, which puts each
    /// comment on its own line.
    fn build_object_pattern_trailing_comments(&self, obj: &internal::ObjectPattern) -> DocId {
        let d = self.d();
        if let Some(last_prop) = obj.properties.last() {
            let prop_end = last_prop.span().end;
            let boundary = obj
                .type_annotation
                .as_ref()
                .map_or(obj.span.end, |t| t.span.start);

            // Only collect comments that are NOT on the same line as the property
            // Same-line comments are handled in the property loop
            let mut parts = Vec::new();
            for comment in comments_in_range(self.comments, prop_end, boundary) {
                if !self.is_same_line(prop_end, comment.span.start) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }
            d.concat(&parts)
        } else {
            d.empty()
        }
    }

    /// Build dangling comments after the last element of an *expanded* pattern
    /// (between the last element and the closing `}`/`]`).
    ///
    /// Each comment goes on its own line, with blank lines preserved. Same-line
    /// trailing comments are handled in the per-element loop, so they are
    /// skipped here. Mirrors the object/array expression printers' handling of
    /// trailing comments before the closing delimiter (`objects.rs`). Without
    /// this, expanded array patterns dropped these comments entirely (content
    /// loss) and expanded object patterns glued them onto the last property's
    /// line.
    fn build_pattern_trailing_dangling_comments(&self, prev_end: u32, boundary: u32) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut last_pos = prev_end;
        for comment in comments_in_range(self.comments, prev_end, boundary) {
            if self.is_same_line(prev_end, comment.span.start) {
                continue;
            }
            if self.has_blank_line_between(last_pos, comment.span.start) {
                parts.push(d.literalline());
            }
            parts.push(d.hardline());
            parts.push(self.build_comment_doc(comment));
            last_pos = comment.span.end;
        }
        d.concat(&parts)
    }

    /// Generic helper: Check for line comments and blank lines in a collection
    ///
    /// Returns (has_line_comments, has_blank_lines) in a single pass.
    /// Works for any collection with elements that have spans.
    fn collection_formatting_hints<T>(
        &self,
        collection_start: u32,
        collection_end: u32,
        elements: &[T],
        get_span: impl Fn(&T) -> tsv_lang::Span,
    ) -> (bool, bool) {
        let mut has_line_comments = false;
        let mut has_blank_lines = false;
        let mut prev_end = collection_start + 1; // After opening bracket/brace

        for elem in elements {
            let elem_start = get_span(elem).start;

            // Check for blank line (before any comments)
            let first_comment = comments_in_range(self.comments, prev_end, elem_start).next();
            let check_pos = first_comment.map_or(elem_start, |c| c.span.start);
            if self.has_blank_line_between(prev_end, check_pos) {
                has_blank_lines = true;
            }

            // Check for line comments
            for comment in comments_in_range(self.comments, prev_end, elem_start) {
                if !comment.is_block {
                    has_line_comments = true;
                    break;
                }
            }

            // Early exit if both found
            if has_line_comments && has_blank_lines {
                return (true, true);
            }

            prev_end = get_span(elem).end;
        }

        // Check comments after last element
        for comment in comments_in_range(self.comments, prev_end, collection_end) {
            if !comment.is_block {
                return (true, has_blank_lines);
            }
        }

        (has_line_comments, has_blank_lines)
    }

    /// Check if object pattern has line comments or blank lines between properties
    ///
    /// Returns (has_line_comments, has_blank_lines) in a single pass.
    fn object_pattern_formatting_hints(&self, obj: &internal::ObjectPattern) -> (bool, bool) {
        let boundary = obj
            .type_annotation
            .as_ref()
            .map_or(obj.span.end, |t| t.span.start);
        self.collection_formatting_hints(
            obj.span.start,
            boundary,
            &obj.properties,
            ObjectPatternProperty::span,
        )
    }

    /// Check if object pattern has any own-line single-line block comments
    ///
    /// Mirrors `array_pattern_has_own_line_block_comments`: an own-line block
    /// comment (between, before, or after properties) forces expansion, matching
    /// prettier.
    fn object_pattern_has_own_line_block_comments(&self, obj: &internal::ObjectPattern) -> bool {
        let boundary = obj
            .type_annotation
            .as_ref()
            .map_or(obj.span.end, |t| t.span.start);
        let span = tsv_lang::Span::new(obj.span.start, boundary);
        self.has_own_line_block_comments_in_bracket_list(
            span,
            &obj.properties,
            ObjectPatternProperty::span,
        )
    }

    /// Build doc for empty object pattern: `{}` with optional type annotation
    fn build_empty_object_pattern_doc(&self, obj: &internal::ObjectPattern) -> DocId {
        let d = self.d();
        let body_doc = self.build_empty_body_with_comments_doc(obj.span);
        if let Some(type_annotation) = &obj.type_annotation {
            d.concat(&[body_doc, self.build_type_annotation_doc(type_annotation)])
        } else {
            body_doc
        }
    }

    /// Build expanded doc for object pattern with hardlines (always multiline)
    fn build_expanded_object_pattern_doc(&self, obj: &internal::ObjectPattern) -> DocId {
        let d = self.d();

        // A comment trailing the opening `{` on its own line is kept on the `{`
        // line when the pattern expands (divergence from prettier, which relocates
        // it to its own line as the first property's leading comment). See
        // conformance_prettier.md §Comment relocation (Object destructuring `{`).
        let first_prop_start = obj.properties[0].span().start;
        let (brace_line_prefix, brace_pull_pos) =
            self.delimiter_line_comment_prefix(obj.span.start, first_prop_start);

        // Track previous end for comment detection (start after `{`)
        let mut prev_end = obj.span.start + 1;

        let mut prop_parts = Vec::new();
        for (i, prop) in obj.properties.iter().enumerate() {
            // Handle leading comments before this property (with blank line preservation)
            let prop_start = prop.span().start;
            let leading_comments: Vec<_> = comments_in_range(self.comments, prev_end, prop_start)
                .filter(|c| {
                    // The brace-line comment pulled onto the `{` line above is emitted
                    // as the prefix, not here (only relevant for the first property).
                    !(i == 0
                        && brace_pull_pos
                            .is_some_and(|dpos| self.comment_on_delimiter_line(dpos, c)))
                })
                .collect();

            prop_parts.extend(
                self.build_leading_comments_with_blank_lines(&leading_comments, prop_start),
            );

            // A preceding format-ignore directive keeps the property's source verbatim
            // (trailing comment/comma handled normally)
            if self.has_format_ignore_in_range(prev_end, prop_start) {
                prop_parts.push(self.raw_source_doc(prop.span()));
            } else {
                prop_parts.push(self.build_object_pattern_property_doc(prop));
            }

            let prop_end = prop.span().end;
            let is_last = i == obj.properties.len() - 1;

            // Collect trailing comments (stop at next property or type annotation)
            let upper_bound = obj
                .properties
                .get(i + 1)
                .map(|next| next.span().start)
                .or_else(|| obj.type_annotation.as_ref().map(|t| t.span.start))
                .unwrap_or(obj.span.end);
            let trailing = self.collect_trailing_comments(prop_end, upper_bound, is_last);

            // Separator comma between properties; no trailing comma on the last
            // property under `trailingComma: 'none'` (a rest element never takes one
            // either — it is a syntax error there).
            let comma = if !is_last { d.text(",") } else { d.empty() };
            self.push_element_comma_trailing(&mut prop_parts, &trailing, comma);

            if !is_last {
                // Check for blank line before next property
                let next_prop = &obj.properties[i + 1];
                let next_start = next_prop.span().start;

                // Check from after trailing comments to next property (or its leading comment)
                let check_pos = comments_in_range(self.comments, trailing.end_pos, next_start)
                    .next()
                    .map_or(next_start, |c| c.span.start);

                if self.has_blank_line_between(trailing.end_pos, check_pos) {
                    // Preserve blank line: literalline (no indent) + hardline (with indent)
                    prop_parts.push(d.literalline());
                }
                prop_parts.push(d.hardline());
            }

            prev_end = trailing.end_pos;
        }

        // Check for dangling comments after the last property (before `}`)
        let boundary = obj
            .type_annotation
            .as_ref()
            .map_or(obj.span.end, |t| t.span.start);
        prop_parts.push(self.build_pattern_trailing_dangling_comments(prev_end, boundary));

        // Structure: { + brace-line prefix + indent(hardline + props) + hardline + } + type_annotation
        let mut result_parts = vec![
            d.text("{"),
            d.concat(&brace_line_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&prop_parts)])),
            d.hardline(),
            d.text("}"),
        ];

        if let Some(type_annotation) = &obj.type_annotation {
            result_parts.push(self.build_type_annotation_doc(type_annotation));
        }

        d.concat(&result_parts)
    }

    /// Build a Doc for an object pattern property
    ///
    /// String keys that are valid identifiers are normalized to unquoted form:
    /// `{"key": value}` → `{key: value}`
    fn build_object_pattern_property_doc(&self, prop: &ObjectPatternProperty) -> DocId {
        let d = self.d();
        match prop {
            ObjectPatternProperty::Property(p) => {
                if p.shorthand {
                    // Get the default value's right-hand side if present
                    // Parser may produce AssignmentPattern or AssignmentExpression
                    let default_rhs = match &p.value {
                        Expression::AssignmentPattern(pat) => Some(&pat.right),
                        Expression::AssignmentExpression(assign) => Some(&assign.right),
                        _ => None,
                    };

                    if let Some(rhs) = default_rhs {
                        // Shorthand with default: `{k /* c */ = 1}`
                        let key_end = p.key.span().end;
                        let rhs_start = rhs.span().start;
                        let eq_pos = self.find_equals_position(key_end, rhs_start);
                        let mut parts = vec![self.build_expression_doc(&p.key)];
                        // Comments before `=` stay before `=`
                        if self.has_comments_between(key_end, eq_pos) {
                            parts.push(self.build_inline_comments_between_doc(key_end, eq_pos));
                        }
                        parts.push(d.text(" = "));
                        // Comments after `=` stay after `=`
                        if let Some(comment_doc) =
                            self.build_rhs_comments_opt(eq_pos + 1, rhs_start)
                        {
                            parts.push(comment_doc);
                        }
                        parts.push(self.build_expression_doc(rhs));
                        d.concat(&parts)
                    } else {
                        // Simple shorthand: `{k}`
                        self.build_expression_doc(&p.key)
                    }
                } else {
                    // Handle computed keys: {[key]: value}
                    // For regular keys, use property_key_doc to normalize string keys to identifiers
                    let key_region_end;
                    let key_doc = if p.computed {
                        let inner = self.build_expression_doc(&p.key);
                        let (doc, end) =
                            self.build_computed_key_bracket_doc(p.span.start, &p.key, inner);
                        key_region_end = end;
                        doc
                    } else {
                        key_region_end = p.key.span().end;
                        self.build_property_key_doc(&p.key)
                    };
                    // Comments between key and value, split at `:`
                    // e.g., `{[x] /* c1 */: /* c2 */ a}` → before `:` and after `:`
                    let value_start = p.value.span().start;
                    #[allow(clippy::expect_used)]
                    // Parser guarantees `:` exists in destructuring property
                    let colon_pos = tsv_lang::source_scan::find_char_skipping_comments(
                        self.source.as_bytes(),
                        key_region_end as usize,
                        value_start as usize,
                        b':',
                    )
                    .expect(": not found in destructuring property")
                        as u32;
                    let pre_colon_comments =
                        self.build_inline_comments_between_doc(key_region_end, colon_pos);
                    let mut parts = vec![key_doc, pre_colon_comments];
                    parts.push(d.text(": "));
                    // Comments after `:`
                    let after_colon_comments = self
                        .build_inline_comments_between_doc_trailing_space(
                            colon_pos + 1,
                            value_start,
                        );
                    parts.push(after_colon_comments);
                    parts.push(self.build_expression_doc(&p.value));
                    d.concat(&parts)
                }
            }
            ObjectPatternProperty::RestElement(r) => self.build_rest_element_doc(r),
        }
    }

    /// Build a Doc for an array pattern
    pub(super) fn build_array_pattern_doc(&self, arr: &internal::ArrayPattern) -> DocId {
        if arr.elements.is_empty() {
            return self.build_empty_array_pattern_doc(arr);
        }

        // Check if we need to expand due to line comments or own-line block comments
        let has_line_comments = self.array_pattern_has_line_comments(arr);
        let has_own_line_block = self.array_pattern_has_own_line_block_comments(arr);

        if has_line_comments || has_own_line_block {
            self.build_expanded_array_pattern_doc(arr)
        } else {
            self.build_grouped_array_pattern_doc(arr)
        }
    }

    /// Build doc for empty array pattern: `[]` with optional type annotation
    fn build_empty_array_pattern_doc(&self, arr: &internal::ArrayPattern) -> DocId {
        let d = self.d();
        // For array patterns with type annotations, the body ends before the annotation
        let body_end = arr
            .type_annotation
            .as_ref()
            .map_or(arr.span.end, |t| t.span.start);

        let body_doc = self.build_empty_brackets_with_comments_doc_range(arr.span.start, body_end);

        if let Some(type_annotation) = &arr.type_annotation {
            d.concat(&[body_doc, self.build_type_annotation_doc(type_annotation)])
        } else {
            body_doc
        }
    }

    /// Check if array pattern has any line comments
    fn array_pattern_has_line_comments(&self, arr: &internal::ArrayPattern) -> bool {
        let boundary = arr
            .type_annotation
            .as_ref()
            .map_or(arr.span.end, |t| t.span.start);

        // Flatten elements (skip holes) for checking
        let non_null_elements: Vec<_> = arr.elements.iter().flatten().collect();

        self.collection_formatting_hints(arr.span.start, boundary, &non_null_elements, |elem| {
            elem.span()
        })
        .0 // Return just the has_line_comments flag
    }

    /// Check if array pattern has any own-line single-line block comments
    fn array_pattern_has_own_line_block_comments(&self, arr: &internal::ArrayPattern) -> bool {
        let boundary = arr
            .type_annotation
            .as_ref()
            .map_or(arr.span.end, |t| t.span.start);

        let span = tsv_lang::Span::new(arr.span.start, boundary);

        // Collect non-hole element spans for boundary checking
        let non_null_elements: Vec<_> = arr.elements.iter().flatten().collect();

        self.has_own_line_block_comments_in_bracket_list(span, &non_null_elements, |elem| {
            elem.span()
        })
    }

    /// Build grouped array pattern doc (width-based expansion)
    fn build_grouped_array_pattern_doc(&self, arr: &internal::ArrayPattern) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end = arr.span.start + 1;

        for (i, elem) in arr.elements.iter().enumerate() {
            let is_last = i == arr.elements.len() - 1;

            if let Some(e) = elem {
                // Check for leading comments before this element
                let elem_start = e.span().start;
                let leading_comments =
                    self.build_inline_comments_between_doc_trailing_space(prev_end, elem_start);
                parts.push(leading_comments);

                parts.push(self.build_expression_doc(e));

                let elem_end = e.span().end;

                // Collect trailing comments (stop at next element)
                let upper_bound = arr
                    .elements
                    .get(i + 1)
                    .and_then(|opt| opt.as_ref().map(|e| e.span().start))
                    .unwrap_or(arr.span.end);
                let trailing = self.collect_trailing_comments(elem_end, upper_bound, is_last);

                // Block comments around the comma (line comments force the expanded
                // path, so `trailing.line` is empty here). Separator comma between
                // elements; no trailing comma on the last (trailingComma: 'none').
                let comma = if !is_last { d.text(",") } else { d.empty() };
                self.push_element_comma_trailing(&mut parts, &trailing, comma);
                if !is_last {
                    parts.push(d.line());
                }

                prev_end = trailing.end_pos;
            } else {
                // Hole in array pattern
                if !is_last {
                    parts.push(d.text(","));
                    parts.push(d.line());
                }
            }
        }

        // Build group for the array pattern brackets only
        // Type annotation is OUTSIDE the group so it breaks independently.
        // This ensures `[a, b]: [long_tuple]` breaks the tuple type, not the pattern.
        let group_parts = vec![
            d.text("["),
            d.indent_softline(d.concat(&parts)),
            d.softline(),
            d.text("]"),
        ];

        let group_doc = d.group(d.concat(&group_parts));

        if let Some(type_annotation) = &arr.type_annotation {
            let type_doc = self.build_type_annotation_doc(type_annotation);
            d.concat(&[group_doc, type_doc])
        } else {
            group_doc
        }
    }

    /// Build expanded array pattern doc (always multiline)
    fn build_expanded_array_pattern_doc(&self, arr: &internal::ArrayPattern) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end = arr.span.start + 1;

        // A comment trailing the opening `[` on its own line is kept on the `[`
        // line when the pattern expands (divergence from prettier, which relocates
        // it to its own line as the first element's leading comment). See
        // conformance_prettier.md §Comment relocation (Array destructuring `[`).
        // Only applies when the first element is present (a leading hole has no
        // span to anchor the range); otherwise the existing path handles comments.
        let (bracket_line_prefix, bracket_pull_pos) =
            match arr.elements.first().and_then(|opt| opt.as_ref()) {
                Some(first) => {
                    self.delimiter_line_comment_prefix(arr.span.start, first.span().start)
                }
                None => (Vec::new(), None),
            };

        for (i, elem) in arr.elements.iter().enumerate() {
            let is_last = i == arr.elements.len() - 1;

            if let Some(e) = elem {
                // Check for leading comments before this element (with blank line preservation)
                let elem_start = e.span().start;
                let leading_comments: Vec<_> =
                    comments_in_range(self.comments, prev_end, elem_start)
                        .filter(|c| {
                            // The bracket-line comment pulled onto the `[` line above is
                            // emitted as the prefix, not here (only the first element).
                            !(i == 0
                                && bracket_pull_pos
                                    .is_some_and(|dpos| self.comment_on_delimiter_line(dpos, c)))
                        })
                        .collect();
                parts.extend(
                    self.build_leading_comments_with_blank_lines(&leading_comments, elem_start),
                );

                parts.push(self.build_expression_doc(e));

                let elem_end = e.span().end;

                // Collect trailing comments (stop at next element)
                let upper_bound = arr
                    .elements
                    .get(i + 1)
                    .and_then(|opt| opt.as_ref().map(|e| e.span().start))
                    .unwrap_or(arr.span.end);
                let trailing = self.collect_trailing_comments(elem_end, upper_bound, is_last);

                // Separator comma between elements; no trailing comma on the last
                // element under `trailingComma: 'none'` (a rest element never takes one
                // either — it is a syntax error there).
                let comma = if !is_last { d.text(",") } else { d.empty() };
                self.push_element_comma_trailing(&mut parts, &trailing, comma);

                if !is_last {
                    // Check for blank line before next element (or its leading comment)
                    let next_elem = arr.elements.get(i + 1).and_then(|opt| opt.as_ref());
                    if let Some(next) = next_elem {
                        let next_start = next.span().start;
                        let check_pos =
                            comments_in_range(self.comments, trailing.end_pos, next_start)
                                .next()
                                .map_or(next_start, |c| c.span.start);
                        if self.has_blank_line_between(trailing.end_pos, check_pos) {
                            parts.push(d.literalline());
                        }
                    }
                    parts.push(d.hardline());
                }

                prev_end = trailing.end_pos;
            } else {
                // Hole in array pattern
                parts.push(d.text(","));
                if !is_last {
                    parts.push(d.hardline());
                }
            }
        }

        // Check for dangling comments after the last element (before `]`)
        let boundary = arr
            .type_annotation
            .as_ref()
            .map_or(arr.span.end, |t| t.span.start);
        parts.push(self.build_pattern_trailing_dangling_comments(prev_end, boundary));

        // Structure: [ + bracket-line prefix + indent(hardline + elements) + hardline + ] + type_annotation
        let mut result_parts = vec![
            d.text("["),
            d.concat(&bracket_line_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&parts)])),
            d.hardline(),
            d.text("]"),
        ];

        if let Some(type_annotation) = &arr.type_annotation {
            result_parts.push(self.build_type_annotation_doc(type_annotation));
        }

        d.concat(&result_parts)
    }

    /// Build a Doc for an assignment pattern
    pub(super) fn build_assignment_pattern_doc(
        &self,
        pattern: &internal::AssignmentPattern,
    ) -> DocId {
        let d = self.d();
        let left_doc = self.build_expression_doc(&pattern.left);

        let left_end = pattern.left.span().end;
        let rhs_start = pattern.right.span().start;
        let eq_pos = self.find_equals_position(left_end, rhs_start);

        // Comments before `=` stay before `=` (e.g., `{a /* c */ = 1}`)
        let mut parts = vec![left_doc];
        if self.has_comments_between(left_end, eq_pos) {
            parts.push(self.build_inline_comments_between_doc(left_end, eq_pos));
        }

        // Comments after `=` stay after `=`
        let inline_comments = self.build_rhs_comments_opt(eq_pos + 1, rhs_start);

        let rhs_doc = self.build_expression_doc(&pattern.right);
        let rhs_doc = if needs_parens(&pattern.right, ParenContext::DefaultValue) {
            d.parens(rhs_doc)
        } else {
            rhs_doc
        };
        let value_doc = if let Some(comments_doc) = inline_comments {
            d.concat(&[comments_doc, rhs_doc])
        } else {
            rhs_doc
        };

        parts.push(d.text(" = "));
        parts.push(value_doc);
        d.concat(&parts)
    }

    /// Build a Doc for a rest element
    pub(super) fn build_rest_element_doc(&self, rest: &internal::RestElement) -> DocId {
        let d = self.d();
        // Comments between `...` and the argument (e.g., `.../* c */ args`)
        let dots_end = rest.span.start + "...".len() as u32;
        let arg_start = rest.argument.span().start;
        let comments_doc =
            self.build_comments_between(dots_end, arg_start, CommentSpacing::Trailing);
        let mut parts = vec![
            d.text("..."),
            comments_doc,
            self.build_expression_doc(&rest.argument),
        ];
        if let Some(ta) = &rest.type_annotation {
            parts.push(self.build_type_annotation_doc(ta));
        }
        d.concat(&parts)
    }
}
