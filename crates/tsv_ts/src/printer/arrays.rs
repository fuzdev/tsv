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
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::is_same_line;

impl<'a> Printer<'a> {
    /// Print an array expression: `[1, 2, 3]`
    pub(super) fn print_array_expression(&mut self, arr: &internal::ArrayExpression) {
        if arr.elements.is_empty() {
            // Check for comments inside empty array
            let has_comments = self.has_comments_between(arr.span.start, arr.span.end);
            if has_comments {
                self.print_empty_array_with_comments(arr);
            } else {
                self.write("[]");
            }
            return;
        }

        // Check for line comments in the array (force expansion - can't be inline)
        let has_line_comments = self.has_line_comments_between(arr.span.start, arr.span.end);

        if has_line_comments {
            // Use comment-aware printing path (always expands)
            self.print_array_expression_with_comments(arr);
        } else {
            // Build doc for width-based wrapping (handles block comments inline)
            let doc = self.build_array_doc_with_wrapping(arr);
            self.write_doc_with_margin(&doc);
        }
    }

    /// Print an empty array that contains only comments
    ///
    /// Uses binary search to find comments: O(log n + k)
    fn print_empty_array_with_comments(&mut self, arr: &internal::ArrayExpression) {
        let (content_start, content_end) = super::content_bounds(arr.span);
        self.print_empty_container_with_comments("[", "]", content_start, content_end);
    }

    /// Print an array expression with comments
    ///
    /// Arrays with comments are always expanded to multiline with one element per line.
    fn print_array_expression_with_comments(&mut self, arr: &internal::ArrayExpression) {
        self.write("[");

        if !arr.elements.is_empty() {
            self.write("\n");
            self.indent_level += self.container_indent_increment();

            let mut prev_end = arr.span.start + 1; // After opening bracket

            for (i, elem) in arr.elements.iter().enumerate() {
                let elem_start = if let Some(e) = elem {
                    e.span().start
                } else {
                    // For elision (holes), use next element's start or closing bracket
                    if i + 1 < arr.elements.len() {
                        if let Some(next) = &arr.elements[i + 1] {
                            next.span().start
                        } else {
                            arr.span.end - 1
                        }
                    } else {
                        arr.span.end - 1
                    }
                };

                // Print leading comments before this element
                let is_first = i == 0;
                let had_same_line_comment =
                    self.print_array_leading_comments(prev_end, elem_start, is_first);

                if !had_same_line_comment {
                    self.write_indent();
                }

                // Print element (or nothing for elision)
                if let Some(e) = elem {
                    self.print_expression(e);
                }

                // Get element end position
                let elem_end = if let Some(e) = elem {
                    e.span().end
                } else {
                    elem_start
                };

                // Determine the boundary for trailing comments (next element start or closing bracket)
                let next_boundary = if i + 1 < arr.elements.len() {
                    if let Some(next) = &arr.elements[i + 1] {
                        next.span().start
                    } else {
                        // Next element is a hole - find the following non-hole element
                        arr.elements[i + 1..]
                            .iter()
                            .find_map(|e| e.as_ref().map(|e| e.span().start))
                            .unwrap_or(arr.span.end - 1)
                    }
                } else {
                    arr.span.end - 1 // Before closing bracket
                };

                // Print trailing inline comments (block comments before comma, line comments after)
                // Only consider comments between this element and the next element/closing bracket
                // Uses binary search: O(log n + k)
                let mut has_line_comment = false;
                for comment in comments_in_range(self.comments, elem_end, next_boundary) {
                    if is_same_line(self.source, elem_end, comment.span.start) {
                        if comment.is_block {
                            self.write(" ");
                            self.print_comment(comment);
                        } else {
                            has_line_comment = true;
                        }
                    }
                }

                self.write(",");

                // Print line comments after comma
                if has_line_comment {
                    for comment in comments_in_range(self.comments, elem_end, next_boundary) {
                        if is_same_line(self.source, elem_end, comment.span.start)
                            && !comment.is_block
                        {
                            self.write(" ");
                            self.print_comment(comment);
                        }
                    }
                }

                self.write("\n");
                prev_end = if let Some(e) = elem {
                    e.span().end
                } else {
                    elem_start
                };
            }

            // Print any final comments before closing bracket
            self.print_leading_comments(prev_end, arr.span.end, false);

            self.write_container_closing_indent();
        }

        self.write("]");
    }

    /// Print leading comments before an array element
    ///
    /// Similar to print_object_leading_comments but for arrays.
    /// Returns true if a same-line leading comment was printed.
    ///
    /// Uses binary search to find comments: O(log n + k)
    fn print_array_leading_comments(
        &mut self,
        prev_end: u32,
        curr_start: u32,
        is_first: bool,
    ) -> bool {
        let mut last_comment_end = prev_end;
        let mut printed_same_line = false;

        for comment in comments_in_range(self.comments, prev_end, curr_start) {
            // Skip trailing comments from previous element
            if !is_first && is_same_line(self.source, prev_end, comment.span.start) {
                continue;
            }

            // Same-line leading comment
            if is_same_line(self.source, comment.span.end, curr_start) {
                self.write_indent();
                self.print_comment(comment);
                self.write(" ");
                last_comment_end = comment.span.end;
                printed_same_line = true;
                continue;
            }

            // Comment on its own line
            if comment.span.start > last_comment_end
                && tsv_lang::printing::has_blank_line_between(
                    self.source,
                    last_comment_end,
                    comment.span.start,
                )
            {
                self.write("\n");
            }

            self.write_indent();
            self.print_comment(comment);
            self.write("\n");

            last_comment_end = comment.span.end;
        }

        // Check for blank line after last comment
        if last_comment_end > prev_end
            && last_comment_end < curr_start
            && tsv_lang::printing::has_blank_line_between(self.source, last_comment_end, curr_start)
        {
            self.write("\n");
        }

        printed_same_line
    }

    /// Build a Doc for an array with proper wrapping behavior
    pub(super) fn build_array_doc_with_wrapping(&self, arr: &internal::ArrayExpression) -> Doc {
        if arr.elements.is_empty() {
            return doc::text("[]");
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
    fn build_array_fill_doc(&self, arr: &internal::ArrayExpression) -> Doc {
        let mut parts = Vec::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            // For the first element, check for leading comments after `[`
            if i == 0 {
                let first_elem_start = elem.as_ref().map_or(arr.span.end - 1, |e| e.span().start);
                for comment in
                    comments_in_range(self.comments, arr.span.start + 1, first_elem_start)
                {
                    if comment.is_block {
                        parts.push(doc::text_owned(format!("/*{}*/ ", comment.content)));
                    }
                }
            }

            let elem_end = elem.as_ref().map_or(arr.span.start + 1, |e| e.span().end);

            if let Some(expr) = elem {
                parts.push(self.build_expression_doc(expr));
            }

            // Add trailing block comments (after this element, before next element)
            let next_boundary = if i + 1 < arr.elements.len() {
                arr.elements[i + 1]
                    .as_ref()
                    .map_or(arr.span.end - 1, |e| e.span().start)
            } else {
                arr.span.end - 1
            };

            for comment in comments_in_range(self.comments, elem_end, next_boundary) {
                if comment.is_block {
                    parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                }
            }

            if i < arr.elements.len() - 1 {
                parts.push(doc::comma_line());
            }
        }

        // Reserve 1 char for closing `]` to prevent greedy-fill boundary overflow
        let fill_with_reserve = doc::with_context(
            doc::fill(parts),
            doc::DocContext {
                trailing_reserve: 1,
            },
        );
        let inner = doc::concat(vec![
            doc::softline(),
            fill_with_reserve,
            doc::trailing_comma(),
        ]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, doc::softline());

        doc::group(doc::concat(vec![
            doc::text("["),
            indented_content,
            closing_line,
            doc::text("]"),
        ]))
    }

    /// Build group doc for non-numeric arrays (one per line when broken)
    ///
    /// Includes inline block comments between elements.
    /// Uses binary search to find comments: O(log n + k)
    fn build_array_group_doc(&self, arr: &internal::ArrayExpression) -> Doc {
        let mut parts = Vec::new();

        // Check if last element is an elision (requires mandatory trailing comma)
        let has_trailing_elision = arr.elements.last().is_some_and(Option::is_none);

        for (i, elem) in arr.elements.iter().enumerate() {
            // For the first element, check for leading comments after `[`
            if i == 0 {
                let first_elem_start = elem.as_ref().map_or(arr.span.end - 1, |e| e.span().start);
                for comment in
                    comments_in_range(self.comments, arr.span.start + 1, first_elem_start)
                {
                    if comment.is_block {
                        parts.push(doc::text_owned(format!("/*{}*/ ", comment.content)));
                    }
                }
            }

            let elem_end = elem.as_ref().map_or(arr.span.start + 1, |e| e.span().end);

            // Add element (elisions output nothing - the comma represents them)
            if let Some(expr) = elem {
                parts.push(self.build_expression_doc(expr));
            }

            // Add trailing block comments (after this element, before next element)
            // Comments between elements are treated as trailing comments of the previous element
            let next_boundary = if i + 1 < arr.elements.len() {
                arr.elements[i + 1]
                    .as_ref()
                    .map_or(arr.span.end - 1, |e| e.span().start)
            } else {
                arr.span.end - 1
            };

            for comment in comments_in_range(self.comments, elem_end, next_boundary) {
                if comment.is_block {
                    parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                }
            }

            let is_last = i == arr.elements.len() - 1;
            if !is_last {
                // Separator comma between elements
                parts.push(doc::text(","));
                parts.push(doc::line());
            } else if has_trailing_elision {
                // Trailing comma for elision - MUST be preserved (semantically significant)
                parts.push(doc::text(","));
            }
        }

        // Use trailing_comma() only if last element is NOT an elision
        // (elision trailing comma was already added unconditionally above)
        let trailing = if has_trailing_elision {
            doc::text("")
        } else {
            doc::trailing_comma()
        };

        let inner = doc::concat(vec![doc::softline(), doc::concat(parts), trailing]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, doc::softline());

        doc::group(doc::concat(vec![
            doc::text("["),
            indented_content,
            closing_line,
            doc::text("]"),
        ]))
    }

    /// Build group doc for arrays with multiline content (forced expansion with hardlines)
    fn build_array_group_doc_forced(&self, arr: &internal::ArrayExpression) -> Doc {
        let mut parts = Vec::new();

        for (i, elem) in arr.elements.iter().enumerate() {
            if let Some(expr) = elem {
                parts.push(self.build_expression_doc(expr));
            }

            if i < arr.elements.len() - 1 {
                parts.push(doc::text(","));
                parts.push(doc::hardline());
            }
        }

        let inner = doc::concat(vec![doc::hardline(), doc::concat(parts), doc::text(",")]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, doc::hardline());

        doc::concat(vec![
            doc::text("["),
            indented_content,
            closing_line,
            doc::text("]"),
        ])
    }

    /// Build a Doc for an array expression (for nested contexts)
    ///
    /// Delegates to `build_array_doc_with_wrapping` to ensure multiline content
    /// triggers proper expansion even in nested contexts.
    pub(super) fn build_array_doc(&self, arr: &internal::ArrayExpression) -> Doc {
        // Use the same wrapping logic as top-level arrays to handle multiline content
        self.build_array_doc_with_wrapping(arr)
    }
}
