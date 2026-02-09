// Array expression printing for TypeScript
//
// Handles printing of array expressions with:
// - Width-based wrapping
// - Fill mode for number-only arrays
// - Forced expansion for multiline content
// - Comment preservation

use super::{Printer, has_multiline_content};
use crate::ast::internal::{self, Expression, LiteralValue};
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Check if array should force break based on Prettier's heuristic
    ///
    /// Returns true when:
    /// - More than 1 element
    /// - ALL elements are arrays (or ALL are objects - no mixing)
    /// - EACH inner array/object has more than 1 item
    ///
    /// This matches prettier's shouldBreak logic in array.js:89-106
    fn should_break_nested_array(&self, arr: &internal::ArrayExpression) -> bool {
        if arr.elements.len() <= 1 {
            return false;
        }

        let mut expect_arrays: Option<bool> = None;

        for elem in &arr.elements {
            let Some(expr) = elem else { return false };

            let (is_array, inner_len) = match expr {
                Expression::ArrayExpression(inner) => (true, inner.elements.len()),
                Expression::ObjectExpression(inner) => (false, inner.properties.len()),
                _ => return false,
            };

            // All elements must be same type (all arrays or all objects)
            if expect_arrays.is_some_and(|expected| expected != is_array) {
                return false;
            }
            expect_arrays = Some(is_array);

            // Each inner must have more than 1 item
            if inner_len <= 1 {
                return false;
            }
        }

        true
    }

    /// Calculate the boundary position for the next element (or array end)
    ///
    /// Used to find the range for trailing comments after an element.
    /// Returns the start position of the next element, or the closing bracket if this is the last element.
    fn next_element_boundary(&self, arr: &internal::ArrayExpression, current_index: usize) -> u32 {
        arr.elements[current_index + 1..]
            .iter()
            .find_map(|e| e.as_ref().map(|e| e.span().start))
            .unwrap_or(arr.span.end - 1)
    }

    /// Format a block comment for inline use (with appropriate spacing)
    ///
    /// - `leading: true` for comments before elements → space after: `/*c*/ elem`
    /// - `leading: false` for comments after elements → space before: `elem /*c*/`
    fn format_inline_block_comment(&self, comment: &tsv_lang::Comment, leading: bool) -> DocId {
        let d = self.d();
        if leading {
            d.text_owned(format!("/*{}*/ ", comment.content))
        } else {
            d.text_owned(format!(" /*{}*/", comment.content))
        }
    }

    /// Build expression doc for array element, wrapping certain expressions in isolated_group
    ///
    /// This prevents internal breaks from propagating to parent groups,
    /// enabling arrays to stay hugged (matching Prettier behavior).
    fn build_array_element_doc(&self, expr: &Expression) -> DocId {
        self.build_huggable_expression_doc(expr)
    }

    /// Add leading comments after opening bracket for the first array element
    fn add_first_element_comments(
        &self,
        arr: &internal::ArrayExpression,
        first_elem: Option<&Expression>,
        parts: &mut Vec<DocId>,
    ) {
        let first_elem_start = first_elem.map_or(arr.span.end - 1, |e| e.span().start);
        for comment in comments_in_range(self.comments, arr.span.start + 1, first_elem_start) {
            if comment.is_block {
                parts.push(self.format_inline_block_comment(comment, true));
            }
        }
    }

    /// Calculate the end position of an element (or fallback for elisions)
    fn element_end_position(
        &self,
        elem: Option<&Expression>,
        arr: &internal::ArrayExpression,
    ) -> u32 {
        elem.map_or(arr.span.start + 1, |e| e.span().end)
    }

    /// Add leading block comments for a non-first array element
    ///
    /// Adds block comments that appear after the previous element's comma and before this element.
    /// These are leading comments on the current element (not trailing on the previous).
    fn add_leading_array_comments(
        &self,
        arr: &internal::ArrayExpression,
        elem_start: u32,
        current_index: usize,
        parts: &mut Vec<DocId>,
    ) {
        let prev_end = self.element_end_position(arr.elements[current_index - 1].as_ref(), arr);

        // Start search after the comma (comments after comma are leading on this element)
        let search_start = self.leading_comment_search_start(prev_end, false);

        for comment in comments_in_range(self.comments, search_start, elem_start) {
            if comment.is_block {
                parts.push(self.format_inline_block_comment(comment, true));
            }
        }
    }

    /// Add trailing block comments for an array element
    ///
    /// Only adds comments that are:
    /// - On the same line as the element
    /// - Block comments (not line comments)
    /// - Before the comma (not after - those are leading on next element)
    fn add_trailing_array_comments(
        &self,
        arr: &internal::ArrayExpression,
        elem_end: u32,
        current_index: usize,
        parts: &mut Vec<DocId>,
    ) {
        let next_boundary = self.next_element_boundary(arr, current_index);
        let comma_pos = self.find_comma_after(elem_end);

        for comment in comments_in_range(self.comments, elem_end, next_boundary) {
            if comment.is_block && self.is_same_line(elem_end, comment.span.start) {
                // Only add if before comma (or no comma found - shouldn't happen in valid arrays with more elements)
                if comma_pos.is_none_or(|pos| comment.span.start < pos) {
                    parts.push(self.format_inline_block_comment(comment, false));
                }
            }
        }
    }

    /// Build a Doc for an array with proper wrapping behavior
    pub(super) fn build_array_doc_with_wrapping(&self, arr: &internal::ArrayExpression) -> DocId {
        if arr.elements.is_empty() {
            return self.build_empty_brackets_with_comments_doc(arr.span);
        }

        // Check for line comments in the array (force expansion - can't be inline)
        let has_line_comments = self.has_line_comments_between(arr.span.start, arr.span.end);

        if has_line_comments {
            // Use comment-aware doc building path (always expands with hardlines)
            return self.build_array_doc_with_line_comments(arr);
        }

        // Check if any element has multiline content (e.g., line continuation strings)
        // Prettier expands arrays containing multiline strings (recursively)
        let has_multiline = arr
            .elements
            .iter()
            .flatten()
            .any(|elem| has_multiline_content(elem, self.source));

        // Check if this is a "numbers-only" array (use fill) vs other (one-per-line)
        let is_numbers_only = self.is_numbers_only_array(arr);

        if has_multiline {
            // Force expansion with hardlines for multiline content
            self.build_array_group_doc_forced(arr)
        } else if is_numbers_only {
            // Use fill for greedy packing of numbers
            self.build_array_fill_doc(arr)
        } else {
            // Use group with one-per-line for other content
            self.build_array_group_doc(arr)
        }
    }

    /// Check if array contains only numeric literals (for fill behavior)
    fn is_numbers_only_array(&self, arr: &internal::ArrayExpression) -> bool {
        arr.elements.iter().all(|elem| match elem {
            Some(Expression::Literal(lit)) => {
                matches!(lit.value, LiteralValue::Number(_))
            }
            Some(Expression::UnaryExpression(unary)) => {
                // -1, +1 are also numeric
                matches!(
                    unary.operator,
                    internal::UnaryOperator::Minus | internal::UnaryOperator::Plus
                ) && matches!(
                    unary.argument.as_ref(),
                    Expression::Literal(lit) if matches!(lit.value, LiteralValue::Number(_))
                )
            }
            _ => false,
        })
    }

    /// Build fill doc for numbers-only arrays (greedy packing)
    ///
    /// Includes inline block comments between elements.
    /// Uses binary search to find comments: O(log n + k)
    fn build_array_fill_doc(&self, arr: &internal::ArrayExpression) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            // Handle comments and element (skip comment collection for elisions)
            if let Some(expr) = elem {
                let elem_start = expr.span().start;
                let elem_end = expr.span().end;

                // Add leading comments
                if i == 0 {
                    // First element: comments between `[` and element
                    self.add_first_element_comments(arr, elem.as_ref(), &mut parts);
                } else {
                    // Other elements: comments between previous element's comma and this element
                    self.add_leading_array_comments(arr, elem_start, i, &mut parts);
                }

                // Add element
                parts.push(self.build_expression_doc(expr));

                // Add trailing block comments (before comma only)
                self.add_trailing_array_comments(arr, elem_end, i, &mut parts);
            }

            if i < arr.elements.len() - 1 {
                parts.push(d.comma_line());
            }
        }

        let inner = d.concat(&[d.softline(), d.fill(&parts), d.trailing_comma()]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.softline());

        d.group(d.concat(&[d.text("["), indented_content, closing_line, d.text("]")]))
    }

    /// Build group doc for non-numeric arrays (one per line when broken)
    ///
    /// Includes inline block comments between elements.
    /// Uses binary search to find comments: O(log n + k)
    ///
    /// Note: Arrays with blank lines between elements use build_array_doc_with_line_comments instead.
    fn build_array_group_doc(&self, arr: &internal::ArrayExpression) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Check if last element is an elision (requires mandatory trailing comma)
        let has_trailing_elision = arr.elements.last().is_some_and(Option::is_none);

        // Check Prettier's shouldBreak heuristic for nested arrays/objects
        let should_break = self.should_break_nested_array(arr);

        for (i, elem) in arr.elements.iter().enumerate() {
            // Calculate elem_end for blank line checking (even for elisions)
            let elem_end = self.element_end_position(elem.as_ref(), arr);

            // Handle comments and element (skip comment collection for elisions)
            if let Some(expr) = elem {
                let elem_start = expr.span().start;

                // Add leading comments
                if i == 0 {
                    // First element: comments between `[` and element
                    self.add_first_element_comments(arr, elem.as_ref(), &mut parts);
                } else {
                    // Other elements: comments between previous element's comma and this element
                    self.add_leading_array_comments(arr, elem_start, i, &mut parts);
                }

                // Add element (templates wrapped in isolated_group)
                parts.push(self.build_array_element_doc(expr));

                // Add trailing block comments (before comma only)
                self.add_trailing_array_comments(arr, elem_end, i, &mut parts);
            }

            let is_last = i == arr.elements.len() - 1;
            if !is_last {
                // Check for blank line after this element (using same boundary logic as comments)
                let next_start = self.next_element_boundary(arr, i);
                let has_blank_after = self.has_blank_line_between(elem_end, next_start);

                // Separator comma between elements
                parts.push(d.text(","));
                if has_blank_after {
                    // Blank line preservation: empty line (no indent) then content line (with indent)
                    // Flat mode: just a space (blank line collapses)
                    // Break mode: literalline (empty) + hardline (indented)
                    parts.push(d.if_break(d.concat(&[d.literalline(), d.hardline()]), d.text(" ")));
                } else {
                    parts.push(d.line());
                }
            } else if has_trailing_elision {
                // Trailing comma for elision - MUST be preserved (semantically significant)
                parts.push(d.text(","));
            }
        }

        // Use trailing_comma() only if last element is NOT an elision
        // (elision trailing comma was already added unconditionally above)
        let trailing = if has_trailing_elision {
            d.empty()
        } else {
            d.trailing_comma()
        };

        let inner = d.concat(&[d.softline(), d.concat(&parts), trailing]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.softline());

        // Build group contents
        let group_contents = d.concat(&[d.text("["), indented_content, closing_line, d.text("]")]);

        // Use group_break() when shouldBreak heuristic matched.
        // This sets shouldBreak on the GROUP ITSELF rather than using break_parent().
        // The difference: shouldBreak is local to this group, while break_parent()
        // propagates up and forces enclosing groups to break.
        // Prettier uses shouldBreak for this heuristic (array.js lines 89-106, 143).
        if should_break {
            d.group_break(group_contents)
        } else {
            d.group(group_contents)
        }
    }

    /// Build group doc for arrays with multiline content (forced expansion with hardlines)
    fn build_array_group_doc_forced(&self, arr: &internal::ArrayExpression) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            // Check for blank line before this element (preserved when wrapped)
            let has_blank_before = if i > 0 {
                let prev_end = arr.elements[i - 1]
                    .as_ref()
                    .map_or(arr.span.start + 1, |e| e.span().end);
                let curr_start = elem.as_ref().map_or(arr.span.end - 1, |e| e.span().start);
                self.has_blank_line_between(prev_end, curr_start)
            } else {
                false
            };

            if has_blank_before {
                // Blank line preservation
                parts.push(d.literalline());
            }

            if let Some(expr) = elem {
                parts.push(self.build_array_element_doc(expr));
            }

            if i < arr.elements.len() - 1 {
                parts.push(d.text(","));
                parts.push(d.hardline());
            }
        }

        let inner = d.concat(&[d.hardline(), d.concat(&parts), d.text(",")]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.hardline());

        d.concat(&[d.text("["), indented_content, closing_line, d.text("]")])
    }

    /// Build a Doc for an array with line comments (forced expansion)
    ///
    /// Arrays with line comments always expand to multiline because line comments
    /// cannot appear on the same line as subsequent content.
    fn build_array_doc_with_line_comments(&self, arr: &internal::ArrayExpression) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end = arr.span.start + 1; // After opening bracket

        for (i, elem) in arr.elements.iter().enumerate() {
            let (elem_start, elem_end) = elem.as_ref().map_or_else(
                || {
                    // Elision: use next element's start or closing bracket
                    let pos = arr.elements[i + 1..]
                        .iter()
                        .find_map(|e| e.as_ref().map(|e| e.span().start))
                        .unwrap_or(arr.span.end - 1);
                    (pos, pos)
                },
                |e| (e.span().start, e.span().end),
            );

            // Collect leading comments before this element
            let leading_comments: Vec<_> = comments_in_range(self.comments, prev_end, elem_start)
                .filter(|c| {
                    // Skip comments that are trailing on the previous line
                    !(i > 0 && self.is_same_line(prev_end, c.span.start))
                })
                .collect();

            // Check for blank line before this element or before leading comments
            if i > 0 {
                // Check blank line to the first leading comment, or to the element if no comments
                let blank_check_end = leading_comments
                    .first()
                    .map_or(elem_start, |c| c.span.start);
                if self.has_blank_line_between(prev_end, blank_check_end) {
                    parts.push(d.literalline());
                    parts.push(d.hardline());
                }
            }

            // Add leading comments
            for comment in leading_comments {
                parts.push(self.build_comment_doc(comment));
                // Line comments always need hardline after
                // Block comments: hardline if NOT on same line as element, space otherwise
                if !comment.is_block || !self.is_same_line(comment.span.end, elem_start) {
                    parts.push(d.hardline());
                } else {
                    parts.push(d.text(" "));
                }
            }

            // Add element (or nothing for elision)
            if let Some(e) = elem {
                parts.push(self.build_array_element_doc(e));
            }

            // Boundary for trailing comments: next element or closing bracket
            let next_boundary = self.next_element_boundary(arr, i);

            // Collect same-line trailing comments (block before comma, line after)
            let trailing: Vec<_> = comments_in_range(self.comments, elem_end, next_boundary)
                .filter(|c| self.is_same_line(elem_end, c.span.start))
                .collect();

            // Block comments go before comma
            for comment in trailing.iter().filter(|c| c.is_block) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            parts.push(d.text(","));

            // Line comments go after comma (excluded from width calculations)
            for comment in trailing.iter().filter(|c| !c.is_block) {
                parts.push(self.build_trailing_line_comment_doc(comment));
            }

            // Check if next element has blank line before it or before its leading comments
            // If so, don't add hardline here (blank line will be added at start of next iteration)
            let next_has_blank_before = if i + 1 < arr.elements.len() {
                let next_elem_start = arr.elements[i + 1].as_ref().map_or_else(
                    || {
                        // Elision: use next element's start or closing bracket
                        arr.elements[i + 2..]
                            .iter()
                            .find_map(|e| e.as_ref().map(|e| e.span().start))
                            .unwrap_or(arr.span.end - 1)
                    },
                    |e| e.span().start,
                );
                // Check for blank line to first leading comment, or to element if no comments
                let first_leading_comment =
                    comments_in_range(self.comments, elem_end, next_elem_start)
                        .find(|c| !self.is_same_line(elem_end, c.span.start));
                let blank_check_boundary =
                    first_leading_comment.map_or(next_elem_start, |c| c.span.start);
                self.has_blank_line_between(elem_end, blank_check_boundary)
            } else {
                false
            };

            if i < arr.elements.len() - 1 && !next_has_blank_before {
                parts.push(d.hardline());
            }

            prev_end = elem_end;
        }

        // Add any final comments before closing bracket
        for comment in comments_in_range(self.comments, prev_end, arr.span.end - 1) {
            if !self.is_same_line(prev_end, comment.span.start) {
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
            }
        }

        let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.hardline());

        d.concat(&[d.text("["), indented_content, closing_line, d.text("]")])
    }

    /// Build a Doc for an array expression (for nested contexts)
    ///
    /// Delegates to `build_array_doc_with_wrapping` to ensure multiline content
    /// triggers proper expansion even in nested contexts.
    pub(super) fn build_array_doc(&self, arr: &internal::ArrayExpression) -> DocId {
        // Use the same wrapping logic as top-level arrays to handle multiline content
        self.build_array_doc_with_wrapping(arr)
    }

    /// Build a Doc for an array expression with forced expansion (hardlines).
    ///
    /// Used by chain arg formatting when we need the array to expand internally
    /// with hardlines so fits() can correctly measure the first line.
    /// Produces: `[\n  elem,\n]` with actual hardlines.
    pub(super) fn build_array_doc_expanded(&self, arr: &internal::ArrayExpression) -> DocId {
        let d = self.d();
        if arr.elements.is_empty() {
            return d.text("[]");
        }

        let mut parts = Vec::new();
        for (i, elem) in arr.elements.iter().enumerate() {
            // Elements are Option<Expression> where None = hole/elision
            if let Some(expr) = elem {
                parts.push(self.build_array_element_doc(expr));
            }
            // Holes are represented by just a comma (no element content)

            if i < arr.elements.len() - 1 {
                parts.push(d.text(","));
                parts.push(d.hardline());
            } else {
                // Trailing comma on last element
                parts.push(d.text(","));
            }
        }

        d.concat(&[
            d.text("["),
            d.indent(d.concat(&[d.hardline(), d.concat(&parts)])),
            d.hardline(),
            d.text("]"),
        ])
    }
}
