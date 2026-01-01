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
    /// Build a Doc for an array with proper wrapping behavior
    pub(super) fn build_array_doc_with_wrapping(&self, arr: &internal::ArrayExpression) -> Doc {
        if arr.elements.is_empty() {
            // Check for comments inside empty array
            let has_inner_comments =
                self.has_comments_between(arr.span.start + 1, arr.span.end - 1);
            if has_inner_comments {
                // Build array with comments inside
                let mut comment_parts = Vec::new();
                for comment in
                    comments_in_range(self.comments, arr.span.start + 1, arr.span.end - 1)
                {
                    comment_parts.push(self.build_comment_doc(comment));
                    if !comment.is_block {
                        comment_parts.push(doc::hardline());
                    }
                }
                return doc::concat(vec![
                    doc::text("["),
                    doc::indent(doc::concat(vec![
                        doc::hardline(),
                        doc::concat(comment_parts),
                    ])),
                    doc::hardline(),
                    doc::text("]"),
                ]);
            }
            return doc::text("[]");
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
            let next_boundary = arr.elements[i + 1..]
                .iter()
                .find_map(|e| e.as_ref().map(|e| e.span().start))
                .unwrap_or(arr.span.end - 1);

            for comment in comments_in_range(self.comments, elem_end, next_boundary) {
                if comment.is_block {
                    parts.push(doc::text_owned(format!(" /*{}*/", comment.content)));
                }
            }

            if i < arr.elements.len() - 1 {
                parts.push(doc::comma_line());
            }
        }

        let inner = doc::concat(vec![
            doc::softline(),
            doc::fill(parts),
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
            let next_boundary = arr.elements[i + 1..]
                .iter()
                .find_map(|e| e.as_ref().map(|e| e.span().start))
                .unwrap_or(arr.span.end - 1);

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

    /// Build a Doc for an array with line comments (forced expansion)
    ///
    /// Arrays with line comments always expand to multiline because line comments
    /// cannot appear on the same line as subsequent content.
    fn build_array_doc_with_line_comments(&self, arr: &internal::ArrayExpression) -> Doc {
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

            // Add leading comments before this element
            for comment in comments_in_range(self.comments, prev_end, elem_start) {
                // Skip comments that are trailing on the previous line
                if i > 0 && is_same_line(self.source, prev_end, comment.span.start) {
                    continue;
                }
                parts.push(self.build_comment_doc(comment));
                // Line comments always need hardline after
                // Block comments: hardline if NOT on same line as element, space otherwise
                if !comment.is_block || !is_same_line(self.source, comment.span.end, elem_start) {
                    parts.push(doc::hardline());
                } else {
                    parts.push(doc::text(" "));
                }
            }

            // Add element (or nothing for elision)
            if let Some(e) = elem {
                parts.push(self.build_expression_doc(e));
            }

            // Boundary for trailing comments: next element or closing bracket
            let next_boundary = arr.elements[i + 1..]
                .iter()
                .find_map(|e| e.as_ref().map(|e| e.span().start))
                .unwrap_or(arr.span.end - 1);

            // Collect same-line trailing comments (block before comma, line after)
            let trailing: Vec<_> = comments_in_range(self.comments, elem_end, next_boundary)
                .filter(|c| is_same_line(self.source, elem_end, c.span.start))
                .collect();

            // Block comments go before comma
            for comment in trailing.iter().filter(|c| c.is_block) {
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            parts.push(doc::text(","));

            // Line comments go after comma
            for comment in trailing.iter().filter(|c| !c.is_block) {
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            if i < arr.elements.len() - 1 {
                parts.push(doc::hardline());
            }

            prev_end = elem_end;
        }

        // Add any final comments before closing bracket
        for comment in comments_in_range(self.comments, prev_end, arr.span.end - 1) {
            if !is_same_line(self.source, prev_end, comment.span.start) {
                parts.push(doc::hardline());
                parts.push(self.build_comment_doc(comment));
            }
        }

        let inner = doc::concat(vec![doc::hardline(), doc::concat(parts)]);
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
