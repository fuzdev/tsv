// Destructuring pattern printing for TypeScript
//
// This module handles all destructuring patterns:
// - Object patterns: `{a, b}` with width-based expansion
// - Array patterns: `[a, b]`
// - Assignment patterns: `a = 1`
// - Assignment expressions: `a = b` with width-based wrapping and chain detection
// - Rest elements: `...rest`

use smallvec::SmallVec;

use super::{PatternContext, Printer, object_pattern_should_expand};
use crate::ast::internal::{self, ArrowFunctionBody, Expression, ObjectPatternProperty};
use tsv_lang::Comment;
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

/// Trailing comments collected for a list element (property or array element)
struct TrailingComments<'a> {
    /// Block comments that go before the comma
    block: SmallVec<[&'a Comment; 2]>,
    /// Line comments that go after the comma (in line_suffix)
    line: SmallVec<[&'a Comment; 2]>,
    /// Position after all trailing comments (for updating prev_end)
    end_pos: u32,
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

        // When JSDoc cast parens are stripped, 2+ block comments may end up
        // after the operator even though prettier places the first one before it.
        // Detect and promote the first comment to the LHS.
        let (left_doc, effective_rhs_start) = if let Some((promoted, new_start)) =
            self.promote_block_comment_before_eq(rhs_comment_start, rhs_comment_end)
        {
            (d.concat(&[left_doc, promoted]), new_start)
        } else {
            (left_doc, rhs_comment_start)
        };

        let rhs_has_line_comment =
            self.has_line_comments_between(effective_rhs_start, rhs_comment_end);
        let rhs_comments = self.build_rhs_comments_opt(effective_rhs_start, rhs_comment_end);

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
            );
        }

        // Build right doc for paths that handle layout directly
        let right_doc = if let Expression::AssignmentExpression(rhs_assign) = assign.right.as_ref()
        {
            self.build_assignment_doc_with_context(rhs_assign, AssignmentContext::Chain)
        } else {
            self.build_expression_doc(&assign.right)
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
            // Expand if: nested patterns, line comments, or blank lines between properties
            let should_expand = object_pattern_should_expand(obj, context);
            let (has_line_comments, has_blank_lines) = self.object_pattern_formatting_hints(obj);

            if should_expand || has_line_comments || has_blank_lines {
                self.build_expanded_object_pattern_doc(obj)
            } else {
                // Use group with line breaks for width-based expansion
                // Include type annotation in the group so its width is considered
                let mut parts = Vec::new();
                let last_is_rest = matches!(
                    obj.properties.last(),
                    Some(ObjectPatternProperty::RestElement(_))
                );

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
                    let trailing = self.collect_trailing_comments(prop_end, upper_bound);

                    // Block comments go before comma
                    parts.push(self.build_block_comments_doc(&trailing.block));

                    // Add comma
                    if !is_last {
                        parts.push(d.text(","));
                    } else if !last_is_rest {
                        parts.push(d.trailing_comma());
                    }

                    // Line comments go after comma
                    parts.push(self.build_line_comments_suffix_doc(&trailing.line));

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

                // Build group contents: { + properties + }
                let mut group_parts = vec![
                    d.text("{"),
                    d.indent_softline(d.concat(&parts)),
                    d.softline(),
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

    /// Build trailing comments doc for object pattern (between last property and `}`)
    ///
    /// Only captures comments on NEW lines (not same-line trailing comments,
    /// which are handled in the main loop).
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

    /// Collect trailing comments for a list element (property or array element)
    ///
    /// Trailing comments are same-line comments after the element:
    /// - Block comments: only if they appear BEFORE the comma
    /// - Line comments: always belong to this element (they consume the rest of the line)
    fn collect_trailing_comments(&self, elem_end: u32, upper_bound: u32) -> TrailingComments<'_> {
        // Find comma position in source (if any)
        let comma_pos = self.source[elem_end as usize..upper_bound as usize]
            .find(',')
            .map(|offset| elem_end + offset as u32);

        // Collect same-line trailing comments
        let all: Vec<_> = comments_in_range(self.comments, elem_end, upper_bound)
            .filter(|c| {
                self.is_same_line(elem_end, c.span.start)
                    && (!c.is_block || comma_pos.is_none_or(|comma| c.span.start < comma))
            })
            .collect();

        let block = all.iter().filter(|c| c.is_block).copied().collect();
        let line = all.iter().filter(|c| !c.is_block).copied().collect();
        let end_pos = all.last().map_or(elem_end, |c| c.span.end);

        TrailingComments {
            block,
            line,
            end_pos,
        }
    }

    /// Build docs for block comments (go before comma)
    fn build_block_comments_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in comments {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
        d.concat(&parts)
    }

    /// Build docs for line comments (go after comma, excluded from width)
    fn build_line_comments_suffix_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in comments {
            parts.push(self.build_trailing_line_comment_doc(comment));
        }
        d.concat(&parts)
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
        let last_is_rest = matches!(
            obj.properties.last(),
            Some(ObjectPatternProperty::RestElement(_))
        );

        // Track previous end for comment detection (start after `{`)
        let mut prev_end = obj.span.start + 1;

        let mut prop_parts = Vec::new();
        for (i, prop) in obj.properties.iter().enumerate() {
            // Handle leading comments before this property (with blank line preservation)
            let prop_start = prop.span().start;
            let leading_comments: Vec<_> =
                comments_in_range(self.comments, prev_end, prop_start).collect();

            prop_parts.extend(
                self.build_leading_comments_with_blank_lines(&leading_comments, prop_start),
            );

            prop_parts.push(self.build_object_pattern_property_doc(prop));

            let prop_end = prop.span().end;
            let is_last = i == obj.properties.len() - 1;

            // Collect trailing comments (stop at next property or type annotation)
            let upper_bound = obj
                .properties
                .get(i + 1)
                .map(|next| next.span().start)
                .or_else(|| obj.type_annotation.as_ref().map(|t| t.span.start))
                .unwrap_or(obj.span.end);
            let trailing = self.collect_trailing_comments(prop_end, upper_bound);

            // Block comments go before comma
            prop_parts.push(self.build_block_comments_doc(&trailing.block));

            // Add trailing comma unless it's a rest element (syntax error)
            if !is_last || !last_is_rest {
                prop_parts.push(d.text(","));
            }

            // Line comments go after comma
            prop_parts.push(self.build_line_comments_suffix_doc(&trailing.line));

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

        // Check for trailing comments after last property
        let trailing = self.build_object_pattern_trailing_comments(obj);
        prop_parts.push(trailing);

        // Structure: { + indent(hardline + props) + hardline + } + type_annotation
        let mut result_parts = vec![
            d.text("{"),
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
                        // Shorthand with default: `{k = /* comment */ 1}`
                        let comments = self.build_inline_comments_between_doc_trailing_space(
                            p.key.span().end,
                            rhs.span().start,
                        );
                        d.concat(&[
                            self.build_expression_doc(&p.key),
                            d.text(" = "),
                            comments,
                            self.build_expression_doc(rhs),
                        ])
                    } else {
                        // Simple shorthand: `{k}`
                        self.build_expression_doc(&p.key)
                    }
                } else {
                    // Handle computed keys: {[key]: value}
                    // For regular keys, use property_key_doc to normalize string keys to identifiers
                    let key_doc = if p.computed {
                        d.brackets(self.build_expression_doc(&p.key))
                    } else {
                        self.build_property_key_doc(&p.key)
                    };
                    // Check for comments between `:` and the value
                    // e.g., `{l: /* comment */ m}`
                    let key_end = p.key.span().end;
                    let value_start = p.value.span().start;
                    let comments =
                        self.build_inline_comments_between_doc_trailing_space(key_end, value_start);
                    d.concat(&[
                        key_doc,
                        d.text(": "),
                        comments,
                        self.build_expression_doc(&p.value),
                    ])
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

        // Check if we need to expand due to line comments
        let has_line_comments = self.array_pattern_has_line_comments(arr);

        if has_line_comments {
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
                let trailing = self.collect_trailing_comments(elem_end, upper_bound);

                // Block comments go before comma (line comments handled in expanded version)
                parts.push(self.build_block_comments_doc(&trailing.block));

                // Add comma
                if !is_last {
                    parts.push(d.text(","));
                    parts.push(d.line());
                } else {
                    parts.push(d.trailing_comma());
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

        // Check if last element is a rest element (no trailing comma allowed)
        let last_is_rest = arr
            .elements
            .last()
            .and_then(|opt| opt.as_ref())
            .is_some_and(|e| matches!(e, Expression::RestElement(_)));

        for (i, elem) in arr.elements.iter().enumerate() {
            let is_last = i == arr.elements.len() - 1;

            if let Some(e) = elem {
                // Check for leading comments before this element (with blank line preservation)
                let elem_start = e.span().start;
                let leading_comments: Vec<_> =
                    comments_in_range(self.comments, prev_end, elem_start).collect();
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
                let trailing = self.collect_trailing_comments(elem_end, upper_bound);

                // Block comments go before comma
                parts.push(self.build_block_comments_doc(&trailing.block));

                // Add comma (unless it's the last element AND it's a rest element)
                if !is_last || !last_is_rest {
                    parts.push(d.text(","));
                }

                // Line comments go after comma
                parts.push(self.build_line_comments_suffix_doc(&trailing.line));

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

        // Structure: [ + indent(hardline + elements) + hardline + ] + type_annotation
        let mut result_parts = vec![
            d.text("["),
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

        // Extract inline block comments between `=` and the default value expression
        // (e.g., `a = /** @type {T} */ (expr)` — the JSDoc type cast comment)
        let left_end = pattern.left.span().end;
        let rhs_start = pattern.right.span().start;
        let inline_comments = self.build_rhs_comments_opt(left_end, rhs_start);

        let rhs_doc = self.build_expression_doc(&pattern.right);
        let value_doc = if let Some(comments_doc) = inline_comments {
            d.concat(&[comments_doc, rhs_doc])
        } else {
            rhs_doc
        };

        d.concat(&[left_doc, d.text(" = "), value_doc])
    }

    /// Build a Doc for a rest element
    pub(super) fn build_rest_element_doc(&self, rest: &internal::RestElement) -> DocId {
        let d = self.d();
        d.concat(&[d.text("..."), self.build_expression_doc(&rest.argument)])
    }
}
