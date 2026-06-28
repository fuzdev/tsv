// Array expression printing for TypeScript
//
// Handles printing of array expressions with:
// - Width-based wrapping
// - Fill mode for number-only arrays
// - Forced expansion for multiline content
// - Comment preservation

use crate::ast::internal::{self, Expression, LiteralValue};
use crate::printer::calls::skip_stripped_open_paren;
use crate::printer::{Printer, has_multiline_content};
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::{comments_in_range, has_multiline_block_comments_in_range};

impl<'a> Printer<'a> {
    /// Check if array should force break based on Prettier's heuristic
    ///
    /// Returns true when:
    /// - More than 1 element
    /// - ALL elements are arrays (or ALL are objects - no mixing)
    /// - EACH inner array/object has more than 1 item
    ///
    /// This matches prettier's shouldBreak logic in array.js:89-106
    fn should_break_nested_array(&self, arr: &internal::ArrayExpression<'_>) -> bool {
        if arr.elements.len() <= 1 {
            return false;
        }

        let mut expect_arrays: Option<bool> = None;

        for elem in arr.elements {
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
    fn next_element_boundary(
        &self,
        arr: &internal::ArrayExpression<'_>,
        current_index: usize,
    ) -> u32 {
        arr.elements[current_index + 1..]
            .iter()
            .find_map(|e| e.as_ref().map(|e| e.span().start))
            .unwrap_or(arr.span.end - 1)
    }

    /// Check for a blank line at an array boundary, walking back across any
    /// intervening hole commas (elision-aware version of
    /// `has_blank_line_after_comma`).
    ///
    /// `i` is the destination iter index; `upper` is the position of the next
    /// real token (typically the next element's start, or its first own-line
    /// leading comment).
    fn has_blank_line_at_array_boundary(
        &self,
        arr: &internal::ArrayExpression<'_>,
        i: usize,
        upper: u32,
    ) -> bool {
        let check_start = self.blank_scan_start_before(arr, i, upper);
        let check_end = skip_stripped_open_paren(self.source, check_start, upper);
        self.has_blank_line_between(check_start, check_end)
    }

    /// Compute the scan start for a blank-line check between array element
    /// boundaries — the position immediately before `upper` after stepping past
    /// every comma and every comment in (`scan_from`, `upper`).
    ///
    /// `scan_from` is the end of the last real element before index `i` (or
    /// just inside `[`). `i` may be `arr.elements.len()` for end-of-iter checks
    /// where the "next" boundary is the closing bracket.
    ///
    /// Why both: elision commas live between consecutive holes; line and
    /// multi-line block comments contribute newlines that `has_blank_line_between`
    /// would otherwise count as a blank line. Returning the position past all of
    /// them leaves a pure-whitespace gap to `upper` that the binary-search
    /// blank-line check can interpret correctly.
    ///
    /// Result is clamped to `<= upper`.
    fn blank_scan_start_before(
        &self,
        arr: &internal::ArrayExpression<'_>,
        i: usize,
        upper: u32,
    ) -> u32 {
        let scan_from = arr.elements[..i]
            .iter()
            .rev()
            .find_map(|e| e.as_ref().map(|e| e.span().end))
            .unwrap_or(arr.span.start + 1);
        let mut pos = self
            .find_last_comma_before(scan_from, upper)
            .map_or(scan_from, |c| c + 1);
        for c in comments_in_range(self.comments, pos, upper) {
            if c.span.end > pos {
                pos = c.span.end;
            }
        }
        pos.min(upper)
    }

    /// Format a block comment for inline use (with appropriate spacing)
    ///
    /// - `leading: true` for comments before elements → space after: `/*c*/ elem`
    /// - `leading: false` for comments after elements → space before: `elem /*c*/`
    fn format_inline_block_comment(&self, comment: &tsv_lang::Comment, leading: bool) -> DocId {
        let d = self.d();
        let content = comment.content(self.source);
        if leading {
            d.text_owned(format!("/*{content}*/ "))
        } else {
            d.text_owned(format!(" /*{content}*/"))
        }
    }

    /// Build expression doc for array element, wrapping certain expressions in isolated_group
    ///
    /// This prevents internal breaks from propagating to parent groups,
    /// enabling arrays to stay hugged (matching Prettier behavior).
    fn build_array_element_doc(&self, expr: &Expression<'_>) -> DocId {
        self.build_huggable_expression_doc(expr)
    }

    /// Calculate the end position of an element (or fallback for elisions)
    fn element_end_position(
        &self,
        elem: Option<&Expression<'_>>,
        arr: &internal::ArrayExpression<'_>,
    ) -> u32 {
        elem.map_or(arr.span.start + 1, |e| e.span().end)
    }

    /// Emit block comments in `[search_start, elem_start)` as inline-leading
    /// (`/*c*/ elem`). Used by both the first-element and subsequent-element
    /// paths in the non-expanding array printers.
    fn add_inline_leading_block_comments(
        &self,
        search_start: u32,
        elem_start: u32,
        parts: &mut DocBuf,
    ) {
        for comment in comments_in_range(self.comments, search_start, elem_start) {
            if comment.is_block {
                parts.push(self.format_inline_block_comment(comment, true));
            }
        }
    }

    /// Compute the search start for leading comments on the array element at
    /// `i`. For the first element, that's just inside `[`; for later elements,
    /// just past the comma after the previous element.
    fn leading_comment_search_start_for(
        &self,
        arr: &internal::ArrayExpression<'_>,
        i: usize,
    ) -> u32 {
        if i == 0 {
            arr.span.start + 1
        } else {
            let prev_end = self.element_end_position(arr.elements[i - 1].as_ref(), arr);
            self.leading_comment_search_start(prev_end, false)
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
        arr: &internal::ArrayExpression<'_>,
        elem_end: u32,
        current_index: usize,
        parts: &mut DocBuf,
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
    pub(in crate::printer) fn build_array_doc_with_wrapping(
        &self,
        arr: &internal::ArrayExpression<'_>,
    ) -> DocId {
        if arr.elements.is_empty() {
            return self.build_empty_brackets_inline_with_comments_doc(arr.span);
        }

        // Check for comments that force expansion: line comments (can't be inline),
        // multi-line block comments (contain hardlines that must propagate),
        // or own-line single-line block comments (on a separate line from adjacent tokens)
        let has_expanding_comments = self.has_line_comments_between(arr.span.start, arr.span.end)
            || has_multiline_block_comments_in_range(self.comments, arr.span.start, arr.span.end)
            || self.has_own_line_block_comments_in_array(arr);

        if has_expanding_comments {
            return self.build_array_doc_with_expanding_comments(arr);
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

    /// Check if array contains own-line single-line block comments that force expansion.
    ///
    /// Delegates to the generic `has_own_line_block_comments_in_bracket_list` helper,
    /// filtering out holes (elisions) from the element list.
    fn has_own_line_block_comments_in_array(&self, arr: &internal::ArrayExpression<'_>) -> bool {
        let non_null: Vec<_> = arr.elements.iter().flatten().collect();
        self.has_own_line_block_comments_in_bracket_list(arr.span, &non_null, |e| e.span())
    }

    /// Check if array contains only numeric literals (for fill behavior)
    fn is_numbers_only_array(&self, arr: &internal::ArrayExpression<'_>) -> bool {
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
                    unary.argument,
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
    fn build_array_fill_doc(&self, arr: &internal::ArrayExpression<'_>) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            // Handle comments and element (skip comment collection for elisions)
            if let Some(expr) = elem {
                let elem_start = expr.span().start;
                let elem_end = expr.span().end;

                let search_start = self.leading_comment_search_start_for(arr, i);
                self.add_inline_leading_block_comments(search_start, elem_start, &mut parts);

                parts.push(self.build_expression_doc(expr));

                // Trailing block comments (before comma only)
                self.add_trailing_array_comments(arr, elem_end, i, &mut parts);
            }

            if i < arr.elements.len() - 1 {
                parts.push(d.comma_line());
            }
        }

        let inner = d.concat(&[d.softline(), d.fill(&parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.softline());

        d.group(d.concat(&[d.text("["), indented_content, closing_line, d.text("]")]))
    }

    /// Build group doc for non-numeric arrays (one per line when broken)
    ///
    /// Includes inline block comments between elements.
    /// Uses binary search to find comments: O(log n + k)
    ///
    /// Note: Arrays with expanding comments use build_array_doc_with_expanding_comments instead.
    fn build_array_group_doc(&self, arr: &internal::ArrayExpression<'_>) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        // Check if last element is an elision (requires mandatory trailing comma)
        let has_trailing_elision = arr.elements.last().is_some_and(Option::is_none);

        // Check Prettier's shouldBreak heuristic for nested arrays/objects
        let mut should_break = self.should_break_nested_array(arr);

        for (i, elem) in arr.elements.iter().enumerate() {
            // Calculate elem_end for blank line checking (even for elisions)
            let elem_end = self.element_end_position(elem.as_ref(), arr);

            // Handle comments and element (skip comment collection for elisions)
            if let Some(expr) = elem {
                let elem_start = expr.span().start;

                let search_start = self.leading_comment_search_start_for(arr, i);
                self.add_inline_leading_block_comments(search_start, elem_start, &mut parts);

                // Add element (templates wrapped in isolated_group)
                parts.push(self.build_array_element_doc(expr));

                // Trailing block comments (before comma only)
                self.add_trailing_array_comments(arr, elem_end, i, &mut parts);
            }

            let is_last = i == arr.elements.len() - 1;
            if !is_last {
                // Check for blank line after this element (using same boundary logic as comments).
                let next_start = self.next_element_boundary(arr, i);
                let has_blank_after = self.has_blank_line_after_comma(elem_end, next_start);

                // Separator comma between elements
                parts.push(d.text(","));

                // Own-line block comments from spread with stripped parens:
                // placed after the comma as siblings in the array.
                if let Some(expr) = elem {
                    let spread_comments = self.spread_own_line_block_comments(expr);
                    if !spread_comments.is_empty() {
                        for comment in &spread_comments {
                            parts.push(d.line());
                            parts.push(self.build_comment_doc(comment));
                        }
                        should_break = true;
                    }
                }

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

                // Block comments past the trailing-elision comma (e.g., `[, , ,/* c */]`)
                // aren't picked up by add_leading/add_trailing, which only run for real
                // elements. Anchor on the last comma in the array, then emit any block
                // comments after it inline.
                let scan_start = arr
                    .elements
                    .iter()
                    .flatten()
                    .next_back()
                    .map_or(arr.span.start + 1, |e| e.span().end);
                if let Some(lc) = self.find_last_comma_before(scan_start, arr.span.end - 1) {
                    for comment in comments_in_range(self.comments, lc + 1, arr.span.end - 1) {
                        if comment.is_block {
                            parts.push(self.build_comment_doc(comment));
                        }
                    }
                }
            }
        }

        // Own-line block comments after the last element (before closing bracket).
        // These appear as siblings after the last element, forcing the array to break.
        // Also picks up comments from spread with stripped parens that
        // build_spread_doc intentionally skips.
        let last_elem_end = arr.elements.last().and_then(|e| e.as_ref()).map(|e| {
            // For spread elements, also check inside the spread span for
            // comments from stripped parens (argument.end to spread.end)
            if let Expression::SpreadElement(spread) = e {
                let has_inner =
                    self.has_comments_between(spread.argument.span().end, spread.span.end);
                if has_inner {
                    return spread.argument.span().end;
                }
            }
            e.span().end
        });
        let mut trailing_own_line_comments = Vec::new();
        // Same-line block comment trailing the LAST element's comma — preserved
        // after the comma (prettier relocates before; see conformance_prettier.md
        // §Comment relocation). Own-line comments are handled below as siblings.
        let mut trailing_same_line_after_comma = Vec::new();
        if let Some(search_start) = last_elem_end {
            let comma_pos = self.find_comma_after(search_start);
            for comment in comments_in_range(self.comments, search_start, arr.span.end - 1) {
                if !comment.is_block {
                    continue;
                }
                if !self.is_same_line(search_start, comment.span.start) {
                    trailing_own_line_comments.push(comment);
                } else if comma_pos.is_some_and(|pos| comment.span.start > pos) {
                    trailing_same_line_after_comma.push(comment);
                }
            }
        }

        let mut inner_parts: DocBuf = smallvec![d.softline(), d.concat(&parts)];
        for comment in &trailing_same_line_after_comma {
            inner_parts.push(d.text(" "));
            inner_parts.push(self.build_comment_doc(comment));
        }
        if !trailing_own_line_comments.is_empty() {
            for comment in &trailing_own_line_comments {
                inner_parts.push(d.line());
                inner_parts.push(self.build_comment_doc(comment));
            }
            should_break = true;
        }

        let inner = d.concat(&inner_parts);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.softline());

        // Build group contents
        let group_contents = d.concat(&[d.text("["), indented_content, closing_line, d.text("]")]);

        // Use group_break() when shouldBreak heuristic matched or spread comments force it.
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
    fn build_array_group_doc_forced(&self, arr: &internal::ArrayExpression<'_>) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            if i > 0 {
                parts.push(d.text(","));

                // Check for blank line before this element (preserved when wrapped).
                // Use literalline() for the blank line (no trailing whitespace) then
                // hardline() for the indented content line — matches the non-forced path.
                let prev_end = arr.elements[i - 1]
                    .as_ref()
                    .map_or(arr.span.start + 1, |e| e.span().end);
                let curr_start = elem.as_ref().map_or(arr.span.end - 1, |e| e.span().start);
                if self.has_blank_line_after_comma(prev_end, curr_start) {
                    parts.push(d.literalline());
                }

                parts.push(d.hardline());
            }

            if let Some(expr) = elem {
                parts.push(self.build_array_element_doc(expr));
            }
        }

        // Own-line block comments after the last element (before closing bracket).
        // These appear after the last element (no trailing comma), each on its own line.
        let mut trailing_comments = Vec::new();
        if let Some(last) = arr.elements.last().and_then(|e| e.as_ref()) {
            let search_start = last.span().end;
            for comment in comments_in_range(self.comments, search_start, arr.span.end - 1) {
                if comment.is_block && !self.is_same_line(search_start, comment.span.start) {
                    trailing_comments.push(comment);
                }
            }
        }

        if trailing_comments.is_empty() {
            // No trailing comma after the last element under `trailingComma: 'none'`.
            let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
            let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.hardline());
            d.concat(&[d.text("["), indented_content, closing_line, d.text("]")])
        } else {
            // No trailing comma after the last element under `trailingComma: 'none'`,
            // then comments on own lines.
            for comment in &trailing_comments {
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
            }
            let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
            let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.hardline());
            d.concat(&[d.text("["), indented_content, closing_line, d.text("]")])
        }
    }

    /// Build a Doc for an array with comments that force expansion.
    ///
    /// Used for arrays containing line comments (can't be inline) or multi-line
    /// block comments (hardlines must propagate). Always expands to multiline.
    fn build_array_doc_with_expanding_comments(
        &self,
        arr: &internal::ArrayExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        // A comment trailing the opening `[` on its own line is kept on the `[`
        // line (divergence from prettier, which relocates it to its own line as the
        // first element's leading comment). See conformance_prettier.md §Comment
        // relocation (Array literal `[`).
        let first_elem_start = arr
            .elements
            .iter()
            .flatten()
            .next()
            .map_or(arr.span.end - 1, |e| e.span().start);
        let (bracket_line_prefix, bracket_pull_pos) =
            self.delimiter_line_comment_prefix(arr.span.start, first_elem_start);

        // End of the most recently emitted REAL element. Holes don't advance it;
        // this lets the next real element's leading-comment range walk back across
        // any intervening hole commas to claim the comments between them and the
        // previous real element. Also drives the post-loop trailing-comments scan.
        let mut last_real_emit_end = arr.span.start + 1;

        // End position of the last trailing-on-array comment emitted by the
        // trailing-hole iteration (when present). The post-loop scan starts here
        // to avoid re-emitting those comments.
        let mut trailing_hole_comments_end: Option<u32> = None;

        for (i, elem) in arr.elements.iter().enumerate() {
            // O(remaining elements) — compute once and reuse below.
            let next_boundary = self.next_element_boundary(arr, i);
            let (elem_start, elem_end) = match elem {
                Some(e) => (e.span().start, e.span().end),
                None => (next_boundary, next_boundary),
            };

            // Hole at the LAST element index: its leading comments are trailing
            // on the array as a whole (no future real element to attach to).
            // Collect them and emit inline after the hole's comma below.
            let is_trailing_hole = elem.is_none() && i + 1 == arr.elements.len();

            // For real elements: comments in (last_real_emit_end, elem_start).
            // For a trailing hole: same range, but extended to the closing `]`.
            // Other holes contribute nothing — their comments belong to the next
            // real element's leading-comment range.
            //
            // Filter rule: comments same-line with the previous real element are
            // trailing on it, EXCEPT block comments past its comma, which are
            // leading on this element.
            let leading_upper = if elem.is_some() {
                Some(elem_start)
            } else if is_trailing_hole {
                Some(arr.span.end - 1)
            } else {
                None
            };
            let leading_comments: Vec<_> = if let Some(upper) = leading_upper {
                let prev_comma_pos = (i > 0)
                    .then(|| self.find_comma_after(last_real_emit_end))
                    .flatten();
                comments_in_range(self.comments, last_real_emit_end, upper)
                    .filter(|c| {
                        // Bracket-line comments pulled onto the `[` line above are
                        // emitted as the prefix, not as leading on the first element.
                        // (Only the first element's gap can be same-line as `[`.)
                        if let Some(dpos) = bracket_pull_pos
                            && self.comment_on_delimiter_line(dpos, c)
                        {
                            return false;
                        }
                        if i > 0 && self.is_same_line(last_real_emit_end, c.span.start) {
                            c.is_block && prev_comma_pos.is_some_and(|pos| c.span.start > pos)
                        } else {
                            true
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };

            // Blank-line check before this element / its leading comments
            // (walks back across intervening hole commas).
            if i > 0 {
                let blank_check_end = leading_comments
                    .first()
                    .map_or(elem_start, |c| c.span.start);
                if self.has_blank_line_at_array_boundary(arr, i, blank_check_end) {
                    parts.push(d.literalline());
                    parts.push(d.hardline());
                }
            }

            // Emit leading comments BEFORE real element.
            if elem.is_some() {
                for (ci, comment) in leading_comments.iter().enumerate() {
                    if ci > 0 {
                        let prev_comment_end = leading_comments[ci - 1].span.end;
                        if self.has_blank_line_between(prev_comment_end, comment.span.start) {
                            parts.push(d.literalline());
                            parts.push(d.hardline());
                        }
                    }
                    parts.push(self.build_comment_doc(comment));
                    // If a blank line follows, the next iter's literalline+hardline
                    // handles the separator — emit nothing here.
                    let next_is_separated_by_blank =
                        leading_comments.get(ci + 1).is_some_and(|next| {
                            self.has_blank_line_between(comment.span.end, next.span.start)
                        });
                    if !next_is_separated_by_blank {
                        let inline_block =
                            comment.is_block && self.is_same_line(comment.span.end, elem_start);
                        parts.push(if inline_block {
                            d.text(" ")
                        } else {
                            d.hardline()
                        });
                    }
                }
            }

            if let Some(e) = elem {
                parts.push(self.build_array_element_doc(e));
            }

            // Same-line trailing comments (real elements only).
            let trailing: Vec<_> = if elem.is_some() {
                let comma_pos = self.find_comma_after(elem_end);
                comments_in_range(self.comments, elem_end, next_boundary)
                    .filter(|c| self.is_same_line(elem_end, c.span.start))
                    .filter(|c| {
                        if c.is_block {
                            comma_pos.is_none_or(|pos| c.span.start < pos)
                        } else {
                            true
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };

            for comment in trailing.iter().filter(|c| c.is_block) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            // Separator comma between elements; under `trailingComma: 'none'` the last
            // REAL element gets no trailing comma, but a trailing-elision hole keeps its
            // (syntactically significant) comma.
            let is_last = i + 1 == arr.elements.len();
            if !is_last || elem.is_none() {
                parts.push(d.text(","));
            }

            for comment in trailing.iter().filter(|c| !c.is_block) {
                parts.push(self.build_trailing_line_comment_doc(comment));
            }

            // Trailing-hole iter: emit collected trailing-on-array comments inline
            // after this hole's comma. First same-line block comment hugs the comma
            // (no separator); subsequent or own-line comments use hardline.
            if is_trailing_hole && !leading_comments.is_empty() {
                // Source position of the LAST comma before `]` (the comma we just
                // emitted for this hole). Used as the same-line anchor for the
                // first comment.
                let last_comma = self.find_last_comma_before(last_real_emit_end, arr.span.end - 1);

                for (ci, comment) in leading_comments.iter().enumerate() {
                    let same_line_inline = if ci == 0 {
                        comment.is_block
                            && last_comma.is_some_and(|c| self.is_same_line(c, comment.span.start))
                    } else {
                        let prev_comment_end = leading_comments[ci - 1].span.end;
                        comment.is_block
                            && self.is_same_line(prev_comment_end, comment.span.start)
                            && !self.has_blank_line_between(prev_comment_end, comment.span.start)
                    };
                    if same_line_inline {
                        parts.push(self.build_comment_doc(comment));
                    } else {
                        if ci > 0 {
                            let prev_comment_end = leading_comments[ci - 1].span.end;
                            if self.has_blank_line_between(prev_comment_end, comment.span.start) {
                                parts.push(d.literalline());
                            }
                        }
                        parts.push(d.hardline());
                        parts.push(self.build_comment_doc(comment));
                    }
                }
                trailing_hole_comments_end = leading_comments.last().map(|c| c.span.end);
            }

            // Suppress trailing hardline if the next iter has a blank line before it
            // (the blank check at start of that iter will emit it).
            let next_has_blank_before = if i + 1 < arr.elements.len() {
                let first_leading_comment =
                    comments_in_range(self.comments, elem_end, next_boundary)
                        .find(|c| !self.is_same_line(elem_end, c.span.start));
                let blank_check_boundary =
                    first_leading_comment.map_or(next_boundary, |c| c.span.start);
                self.has_blank_line_at_array_boundary(arr, i + 1, blank_check_boundary)
            } else {
                false
            };

            if i < arr.elements.len() - 1 && !next_has_blank_before {
                parts.push(d.hardline());
            }

            if elem.is_some() {
                last_real_emit_end = elem_end;

                // Spread elements with own-line trailing comments from stripped parens:
                // expose them to subsequent leading-comment searches and the final scan.
                if let Some(Expression::SpreadElement(spread)) = elem {
                    let arg_end = spread.argument.span().end;
                    let has_own_line = comments_in_range(self.comments, arg_end, spread.span.end)
                        .any(|c| c.is_block && self.has_newline_between(arg_end, c.span.start));
                    if has_own_line {
                        last_real_emit_end = arg_end;
                    }
                }
            }
        }

        // Final comments before closing bracket. Skip what trailing-hole emission
        // already handled.
        let final_scan_start = trailing_hole_comments_end.unwrap_or(last_real_emit_end);
        for comment in comments_in_range(self.comments, final_scan_start, arr.span.end - 1) {
            if !self.is_same_line(final_scan_start, comment.span.start) {
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
            }
        }

        let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.hardline());

        d.concat(&[
            d.text("["),
            d.concat(&bracket_line_prefix),
            indented_content,
            closing_line,
            d.text("]"),
        ])
    }

    /// Build a Doc for an array expression (for nested contexts)
    ///
    /// Delegates to `build_array_doc_with_wrapping` to ensure multiline content
    /// triggers proper expansion even in nested contexts.
    pub(in crate::printer) fn build_array_doc(&self, arr: &internal::ArrayExpression<'_>) -> DocId {
        // Use the same wrapping logic as top-level arrays to handle multiline content
        self.build_array_doc_with_wrapping(arr)
    }

    /// Build a Doc for an array expression with forced expansion (hardlines).
    ///
    /// Used by chain arg formatting when we need the array to expand internally
    /// with hardlines so fits() can correctly measure the first line.
    /// Produces: `[\n  elem,\n]` with actual hardlines.
    pub(in crate::printer) fn build_array_doc_expanded(
        &self,
        arr: &internal::ArrayExpression<'_>,
    ) -> DocId {
        let d = self.d();
        if arr.elements.is_empty() {
            return d.text("[]");
        }

        let mut parts = DocBuf::new();
        for (i, elem) in arr.elements.iter().enumerate() {
            // Elements are Option<Expression> where None = hole/elision
            if let Some(expr) = elem {
                parts.push(self.build_array_element_doc(expr));
            }
            // Holes are represented by just a comma (no element content)

            if i < arr.elements.len() - 1 {
                parts.push(d.text(","));
                parts.push(d.hardline());
            } else if elem.is_none() {
                // Trailing-elision hole keeps its (syntactically significant) comma;
                // a real last element gets no trailing comma under `trailingComma: 'none'`.
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
